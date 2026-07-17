# gpui-form draft、组件 adapter 与外部依赖分离计划

状态：已完成。纯 draft core、typed field handle、generic form events、macro 的 replace/required 生成和
gpui-component 的 caller-owned adapter 已落地；Jaco 的 ChatInput/RunSettings/Prompt/Shortcut、provider
secret 与 MCP 动态数组均已迁移，workspace 不再保留 state-owning binding surface。
本文取代旧的“FormSelection + catalog rebase”方案，是 `gpui-form`、`gpui-form-macros` 和
`gpui-form-gpui-component` 的实施入口。

## 1. 状态与范围

### 目标

1. `gpui-form` 只拥有 typed/raw draft、meta、validation、transform 和 submit；不拥有任何 UI
   component entity。
2. 将组件中的业务值、外部配置和交互状态拆成三个独立通道。
3. options/catalog 更新只更新组件配置和派生合法性，不 hydrate/rebase form，也不制造 dirty conflict。
4. 用户输入只通过 adapter 写入 form；submit 只读取 form draft，永不回读组件 state。
5. 删除当前把 component state 当作 draft source 的 API、trait、类型和宏属性。
6. 保留显式 committed-value reset/replace；不建设通用 form/store 双向同步或 form selector 系统。

### 非目标

- 不修改 `gpui-component`；适配其现有 `InputState`、`SelectState`、`ComboboxState` 和事件 API。
- 不把 provider/project catalog、repository、config 或业务 fallback policy 放入 `gpui-form`。
- 不增加 `gpui-form` 对 `gpui-component` 或 `gpui-store` 的依赖。
- 不为旧 binding API 保留兼容层；workspace 调用方与旧 core 类型一起迁移/删除。
- 不改变 `garde` / `validify` 的验证和 submit normalize 职责。
- 数据库、schema、migration、icon、i18n、网络和平台行为无变化。

### 用户决策

- 以最优、最清晰、最简单的最终结构为目标，允许大规模重构和删除不必要 API。
- `gpui-component` state 物理上可以同时保存 value/options/interaction，但语义 owner 必须分离。
- options 变化不是 form external value，不触发 form rebase。

## 2. 证据快照

### Current fact：错误的耦合边界

| 当前位置 | 当前行为 | 问题 |
| --- | --- | --- |
| `src/component/binding.rs` | `FormComponentBinding` 创建 state、读取 draft、写 value、配置 required/disabled、安装事件 | 一个 trait 同时承担 form、组件配置和交互生命周期 |
| `src/component/fields/component.rs` | `ComponentFieldStore` 持有 component entity，并在 change/blur/submit 回读 state | component state 成为第二个 draft owner；submit 事实源不再唯一 |
| `gpui-form-macros/src/expand/*` | derive 根据 `binding = "..."` 创建 component state 和 subscriptions | form schema 被 UI library state 构造方式污染 |
| `gpui-form-gpui-component/src/select.rs` | `set_items` 和 selected value 都写入同一个 `SelectState` | options 更新可能改变 selected index/presentation，却没有独立同步语义 |
| Jaco `run_settings.rs` | catalog reload 同时写 choices、selected、capability、picker 和 form | options 变化被误建模为 form value 同步，形成重复 owner 和 nested update 风险 |

### Upstream fact：gpui-component state 是混合容器

当前 pinned `gpui-component` 的 `SelectState<D>` 持有 searchable list state；`ListState<D>` 同时
持有 delegate/options、query/focus/scroll/highlight/selected index 和 task/subscription。该结构适合组件
内部实现，但不能直接成为 form domain owner。

### Dependency evidence

| Dependency | Current | Target | Decision |
| --- | --- | --- | --- |
| `gpui` | `0.2.2`, Zed rev `1d217ee39d381ac101b7cf49d3d22451ac1093fe` | unchanged | 复用 Entity/event/subscription；不新增框架 API |
| `gpui-component` | `0.5.2`, commit `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` | unchanged | 适配现有 state；不 fork、不修改上游 |
| `garde` / `validify` | workspace current | unchanged | validation/normalize 边界不变 |
| `gpui-store` | no dependency | none | catalog 由 app/store 拥有，form 不依赖它 |

## 3. 决策

### FORM-D-01：三个通道、三个 owner

| 通道 | 内容 | 唯一可写 owner |
| --- | --- | --- |
| Form draft | raw draft、parsed value、dirty/touched/errors | generated form entity |
| Component config | options/items、capability、disabled、placeholder、mask | app/controller/catalog projection |
| Component interaction | focus、open、query、highlight、scroll、IME、component tasks | component entity |

component state 中的 selected/text 只是 form draft 的 UI mirror。它只能在用户事件入口被读取，不能被
submit、validation、repository 或 fallback policy 当作事实源。

### FORM-D-02：删除 state-owning form binding

删除：

- `FormComponentBinding<Value>`；
- `ComponentStateOptions`；
- `FormComponentEvent` / `FormComponentEventSink`；
- `ComponentFieldStore<Value, Binding>`；
- `ComponentFieldEventKind` / `ComponentFieldEventOutcome` / `FieldDraftSync`；
- `NoComponentBinding` / `NoComponentState`；
- `FormField::ComponentState`、`component_state()`；
- derive 的 `#[form(binding = "...")]`、label/description/placeholder/masked/disabled component
  construction 属性；
- generated `<field>_state()`、`set_<field>_disabled()` 和带 `Window`/component side effect 的
  `set_<field>_required(...)`。

这些 API 没有兼容层。静态 label/description 仍由 app render/i18n 负责；required 只保留 validation/meta
语义，并由纯 form `set_<field>_required(required, cx)` 动态修改，不再尝试配置任意组件 state。

### FORM-D-03：core 使用纯 `DraftFieldStore`

```rust
pub trait FieldCodec<Value>: 'static
where
    Value: Clone + PartialEq + 'static,
{
    type Draft: Clone + PartialEq + 'static;

    fn draft_from_value(value: &Value) -> Self::Draft;

    fn parse(draft: &Self::Draft) -> Result<Value, FieldCodecError>;
}

pub struct FieldCodecError {
    pub code: Cow<'static, str>,
    pub message_key: Cow<'static, str>,
    pub params: ErrorParams,
}

pub struct IdentityCodec<Value>(PhantomData<fn() -> Value>);

pub struct DraftFieldStore<Value, Codec>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    core: FieldCore<Value>,
    baseline: Codec::Draft,
    draft: Codec::Draft,
    parse_error: Option<FieldError>,
    _codec: PhantomData<fn() -> Codec>,
}
```

核心方法固定为：

```rust
impl<Value, Codec> DraftFieldStore<Value, Codec>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    pub fn new(value: Value) -> Self;
    pub fn draft(&self) -> &Codec::Draft;
    pub fn value(&self) -> &Value;
    pub fn set_user_draft(&mut self, draft: Codec::Draft) -> DraftUpdate<Value>;
    pub fn set_value(&mut self, value: Value, cause: FieldChangeCause);
    pub fn replace_baseline(&mut self, value: Value);
    pub fn prepare_submit(
        &mut self,
        path: FieldPath,
        trigger: ValidationTrigger,
    ) -> Result<Value, Box<FieldError>>;
}
```

codec 只转换 draft/value 并返回无 path/trigger 的 `FieldCodecError`；`DraftFieldStore` 在当前 field/trigger
边界补齐 `FieldPath`、`ValidationSource::Internal` 和 visibility routing。`prepare_submit` 解析已保存的
`draft`，不访问任何 component entity。默认 value field 使用
`IdentityCodec<Value>`；input/number 等 raw draft codec 由 adapter crate 或 app 定义，但 codec 本身不依赖
component state。

### FORM-D-04：宏生成 typed field handle，不生成组件 state

每个 leaf field 生成：

```rust
pub fn model_draft(&self) -> ModelKey;
pub fn set_model_draft(
    &mut self,
    draft: ModelKey,
    cause: FieldChangeCause,
    cx: &mut Context<Self>,
);
pub fn set_model_value(
    &mut self,
    value: ModelKey,
    cause: FieldChangeCause,
    window: &mut Window,
    cx: &mut Context<Self>,
);
```

同时生成一个可复制的 entity handle，供 adapter 使用：

```rust
pub struct FormFieldHandle<Form, Draft> {
    form: WeakEntity<Form>,
    path: FieldPath,
    read: fn(&Form) -> Draft,
    write: fn(&mut Form, Draft, FieldChangeCause, &mut Context<Form>),
}

pub(crate) struct FormDraftEvent {
    path: FieldPath,
    draft: Rc<dyn Any>,
    cause: FieldChangeCause,
}

pub struct FieldDraftEvent<Draft> {
    pub draft: Draft,
    pub cause: FieldChangeCause,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConnectionFormField {
    Model,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormStoreEvent<Field> {
    FieldChanged {
        field: Field,
        cause: FieldChangeCause,
    },
}

impl ConnectionFormStore {
    pub fn model_handle(form: &Entity<Self>) -> FormFieldHandle<Self, ModelKey>;

    pub fn model_required(&self) -> bool;
    pub fn set_model_required(
        &mut self,
        required: bool,
        cx: &mut Context<Self>,
    );
}
```

public handle API 固定为：

```rust
impl<Form, Draft> FormFieldHandle<Form, Draft> {
    pub fn draft<C: AppContext>(&self, cx: &C) -> Result<Draft, FormFieldHandleError>;
    pub fn set_user_draft(
        &self,
        draft: Draft,
        cx: &mut impl AppContext,
    ) -> Result<(), FormFieldHandleError>;
    pub fn set_draft(
        &self,
        draft: Draft,
        cause: FieldChangeCause,
        cx: &mut impl AppContext,
    ) -> Result<(), FormFieldHandleError>;

    pub fn subscribe_in<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
        listener: impl Fn(&mut Owner, &FieldDraftEvent<Draft>, &mut Window, &mut Context<Owner>)
            + 'static,
    ) -> Result<Subscription, FormFieldHandleError>;
}
```

generated form 对 adapter emit internal `FormDraftEvent`：带 `FieldPath`、type-erased owned draft 和
`FieldChangeCause`。handle 内部按 generated path 过滤，并将宏保证类型正确的 payload downcast 后，只向 adapter
暴露该字段的 typed `FieldDraftEvent<Draft>`。页面级 observer 使用 runtime 统一的
`FormStoreEvent<ConnectionFormField>`；宏保留 generated `ConnectionFormField`，但删除每个 form 独立生成的
`ConnectionFormEvent` 以及 `FieldFocused`/`FieldBlurred` variants。adapter 不需要订阅整个 form 后匹配字符串，
也不需要再次读取 form。`set_user_draft` 只更新 form entity；handle 不持有组件、不读取 catalog、不提供 config
setter。

动态 required setter 是纯 validation command：相同值 no-op；变化时更新 required meta，删除已失效的 internal
required error，按 `validate(on_dynamic)` 决定是否立即以 `ValidationTrigger::Dynamic` 重算，refresh form meta 并
notify。它不接收 `Window`，也不产生 draft/form change event。

### FORM-D-05：adapter 返回订阅，调用方拥有生命周期

`gpui-form` 保留组件库中立的生命周期集合：

```rust
pub struct SubscriptionSet {
    subscriptions: Vec<Subscription>,
}
```

它只提供 `new`、`push`、`extend`、`clear` 等集合操作，不提供 component-specific `bind` API。调用方按
controller/page/mounted subtree 的实际生命周期保存一个或多个 set。

`gpui-form-gpui-component` 不定义 public binding handle、handle alias、`ComponentBindingSet` 或
`FormComponentAdapter` trait。component-specific bind function 接收已经由调用方创建和配置的 state，并返回一个
局部构造完成的 `SubscriptionSet`：

```rust
pub fn bind_select<Form, Value, Delegate, Owner>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<SelectState<Delegate>>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>;
```

调用方只需要合并结果，不保存逐字段 binding：

```rust
subscriptions.extend(bind_select(
    ProviderFormStore::kind_handle(&form),
    &kind_state,
    window,
    cx,
)?);
```

bind function 先读取 form draft 完成 initial projection，再创建 guard 和两侧 subscriptions；只有全部成功才返回
局部 set。失败时局部 subscriptions 自动 drop，调用方已有 set 不变。guard 是两个 closure 共同捕获的私有状态：

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ComponentSyncState {
    #[default]
    Idle,
    PublishingUserDraft,
    ApplyingFormDraft,
}
```

adapter subscriptions 只做：

1. user event 时读取 component 当前 draft，一次性写入 form；
2. form field event 时把 form draft写入 component mirror；
3. `PublishingUserDraft` 时忽略由本次写入同步产生的 form event；
4. `ApplyingFormDraft` 时忽略 component 的程序化 change event；
5. caller clear/drop 对应 `SubscriptionSet` 时释放两侧 subscriptions。

options/config 不进入 value adapter protocol。简单 config 由调用方直接使用 component API，例如
`InputState::set_disabled`。items 会使 selected item/label projection 失效，因此 adapter 提供
component-specific `set_select_items` / `set_combobox_items` 命令。命令先读取 authoritative form draft，再在
一个 component update 中替换 delegate 并按新 items 重投影 selection；找不到 draft 时只清空 UI selection 并
展示 unavailable，不修改 form、不选择业务 fallback、不产生 form field event。

```rust
pub fn set_select_items<Form, Value, Delegate>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<SelectState<Delegate>>,
    delegate: Delegate,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), ComponentBindError>;
```

当前 pinned `SelectState` / `ComboboxState` 的 items 与 programmatic selection setter 不发 user event，因此 config
command 不需要 retained handle 或 guard。自定义组件和其他组件库在 app/自己的 adapter crate 中实现相同的
`bind_custom_* -> Result<SubscriptionSet, _>` 协议即可，不依赖 `gpui-form-gpui-component`。

### FORM-D-06：只保留两个外部值操作

不实现 `FormSelection`、`draft_revision`、`FormExternalConflict` 或自动 observer rebase。它们不是解决组件
config 问题所必需的最小能力。

generated form 只提供：

```rust
fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>);

fn replace_from_value(
    &mut self,
    value: Input,
    cx: &mut Context<Self>,
);
```

`replace_from_value` 是调用方明确授权丢弃当前 draft 的 command：重建 baseline/draft、清空 errors 和 submit
meta，并通过 internal `FormDraftEvent` 让 adapters 同步实际变化的 UI mirror，同时用 public
`FormStoreEvent<Field>` 通知页面级 observer。需要 conflict 的产品先在 app 比较
`form.meta().is_dirty`，决定是否调用；core 不保存 incoming snapshot 或 conflict UI 状态。

对 dynamic array，已存在行会按位置重新建立 baseline；如果 incoming value 改变了行数，core 不会静默截断或
伪造行，而是在 array path 保留 `array_length_changed` internal error，调用方应使用 array 的显式 append/remove/
replace 操作完成结构变更。

### FORM-D-07：catalog/config 永远不走 replace

```text
committed domain value changed -> app decides -> replace_from_value
component options changed      -> app updates component config only
user input                     -> component adapter -> form draft
programmatic form value        -> form event -> component adapter mirror
submit                         -> form draft -> normalize/validate -> output
```

## 4. 目标模块

### `crates/gpui-form`

| 文件 | 变更 |
| --- | --- |
| `src/core/field.rs` | `FormField` 移除 component associated type/state accessor |
| `src/core/codec.rs` | 新增 `FieldCodec`、`FieldCodecError`、`IdentityCodec`、`DraftFieldStore`、`DraftUpdate` |
| `src/core/subscriptions.rs` | 保留通用 `SubscriptionSet` 集合语义；不得添加 component/adapter 专用 API |
| `src/component.rs`、`src/component/*` | 删除；core 不再有 component 概念 |
| `src/macro_support.rs` | 只暴露 draft/validation/group/array glue |
| `src/lib.rs` | 删除旧 binding exports，导出 draft/handle API 与中立 `SubscriptionSet` |
| `src/core/field_handle.rs` | 新增 `FormFieldHandle`、internal `FormDraftEvent`、typed `FieldDraftEvent` 和 released-form error |
| `src/core/events.rs` | 新增 public generic `FormStoreEvent<Field>`；只表达 field/cause，不保存 heterogeneous draft |
| `tests/derive.rs` | 删除 component state assertions，改测 pure draft |

### `crates/gpui-form-macros`

| 文件 | 变更 |
| --- | --- |
| `src/attributes.rs` | 删除 binding/component construction 属性；解析 `codec`、`group(store)`、`array(store)` 和 form validation 属性 |
| `src/field_kind.rs` | leaf 只区分 identity draft、explicit codec、group、array；group/array 不再叫 component |
| `src/expand/fields.rs` | 生成 `DraftFieldStore`，不创建 Entity/Subscription |
| `src/expand/accessors.rs` | 生成 draft/value、纯 required setter 和 `FormFieldHandle` constructor |
| `src/expand/events.rs` | 保留 generated `<Form>Field`；改为 runtime `FormStoreEvent<Field>`，删除 generated `<Form>Event` |
| `src/expand/validation.rs` | submit 只解析 field-owned draft |

目标属性：

```rust
#[derive(FormStore)]
pub struct ConnectionInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub name: String,

    #[form(codec = "PortCodec", validate(on_blur, on_submit))]
    pub port: u16,

    #[form(group(store = "AuthFormStore"))]
    pub auth: AuthInput,

    #[form(array(store = "HeaderFormStore"))]
    pub headers: Vec<HeaderInput>,
}
```

### `crates/gpui-form-gpui-component`

| 文件 | 变更 |
| --- | --- |
| `src/binding.rs` | 新增 `ComponentBindError` 和私有 guard/install helper；不导出 binding handle/set/trait |
| `src/input.rs` | `bind_input` + `OptionalTextCodec`；plain String 使用 core identity codec |
| `src/number.rs` | `bind_number` + number codecs/policy |
| `src/bool.rs` | `bind_bool` |
| `src/select.rs` | `bind_select` + `set_select_items`，items 后重投影 form draft mirror |
| `src/combobox.rs` | `bind_combobox` + `set_combobox_items`，items 后重投影 form draft mirror |
| `src/lib.rs` | 删除旧 `*Binding` trait impl/handle exports，只导出 bind/config functions 和 codecs |

## 5. 数据流与 GPUI 约束

用户输入：

```text
component event
  -> adapter reads component draft
  -> guard = PublishingUserDraft
  -> FormFieldHandle::set_user_draft
  -> form parses/stores draft and validates
  -> form emits internal FormDraftEvent + public FormStoreEvent<Field>
  -> same adapter sees guard and ignores mirror write
  -> guard = Idle
```

程序化 form 更新：

```text
form setter/replace/normalize
  -> form emits internal FormDraftEvent + public FormStoreEvent<Field>
  -> adapter guard = ApplyingFormDraft
  -> component state receives mirror value
  -> programmatic component event is ignored
  -> guard = Idle
```

config 更新：

```text
catalog/app command -> component-specific config command
                    -> replace items/config + reproject form draft mirror
                    -> no form update and no fallback
```

同一 entity 在 update scope 内不得再次 update。guard 的作用是消除同步回路，不是延迟或吞掉真实用户事件；
不得用无条件 `defer_in` 代替正确的方向建模。

## 6. 删除优先审计

| 当前实现 | 决定 | 替代 |
| --- | --- | --- |
| form-owned component entity | Delete | app-created state + adapter-returned subscriptions |
| submit-time `read_draft(state)` | Delete | field-owned draft |
| `ComponentStateOptions` | Delete | app render/component configuration |
| generic component `set_disabled/set_required` binding methods | Delete | component-specific config API；required 使用纯 form setter |
| generated `<Form>Event` + focus/blur variants | Delete | generated `<Form>Field` + runtime `FormStoreEvent<Field>`；blur 使用 cause |
| `component = "group"/"array"` | Delete | `group(store = ...)` / `array(store = ...)` |
| `FormSelection` proposal | Delete | form typed events/field handles；app直接读 draft |
| draft revision proposal | Delete | `PartialEq` + field events |
| conflict value types proposal | Delete | app dirty check + explicit `replace_from_value` |
| `gpui-component` controls | Reuse directly | adapter 只做事件/value mirror |

## 7. 工作包

### FORM-10：纯 draft core

**文件**：删除 `src/component*`；新增 `src/core/codec.rs`、`src/core/field_handle.rs`、`src/core/events.rs`；修改
field/lib/macro_support。

**实现**：先引入 `FieldCodec`/`DraftFieldStore`，迁移 identity/raw draft、parse error、dirty/default、submit
normalize；所有测试在没有 component entity 的情况下通过，再删除旧类型。

**测试**：`crates/gpui-form/tests/draft.rs`：

- `raw_draft_is_form_owned`；
- `invalid_draft_survives_until_submit`；
- `typed_equal_raw_draft_stays_dirty`；
- `replace_from_value_replaces_baseline_and_clears_meta`；
- `form_store_event_carries_field_and_cause`。

### FORM-20：derive 生成边界

**前置**：FORM-10。

**文件**：macro attributes/field kind/fields/accessors/events/validation 和 compile tests。

**实现**：删除 binding 属性和 component construction；将 group/array 改为结构属性；生成 codec field、typed
setters/handles、纯 required setter、generated field enum 和 runtime generic form event；group/array child-store
生命周期保持现状。

**测试**：`crates/gpui-form-macros/tests/compile.rs` 覆盖 identity/codec/group/array/dynamic required；旧 binding、
component construction 和 `component = "group"/"array"` 必须 compile-fail 并给出对应迁移提示。

### FORM-30：gpui-component adapter subscriptions

**前置**：FORM-20。

**文件**：`crates/gpui-form-gpui-component/src/{binding,input,number,bool,select,combobox}.rs`。

**实现**：调用方创建 state 和持有 core `SubscriptionSet`；bind function 原子安装两侧 subscription 和私有 guard，
返回局部 set 供调用方 extend；config API 与 value mirror 分开。删除 public binding handle/aliases、
`ComponentBindingSet`、universal adapter trait 和 component-specific `SubscriptionSet` extension。

**测试**：

- `user_event_updates_form_once`；
- `form_setter_updates_component_once`；
- `select_items_change_does_not_change_form_draft`；
- `disabled_change_does_not_change_form_draft`；
- `caller_subscription_set_drop_releases_subscriptions`；
- `bind_failure_leaves_caller_subscription_set_unchanged`；
- `custom_adapter_requires_only_form_handle_and_subscription_set`；
- `programmatic_component_event_does_not_reenter_state`。

### FORM-40：workspace 调用方迁移

**前置**：FORM-30。

Jaco prompt/shortcut、ChatInput/RunSettings、provider secret、MCP 动态字段/数组行和测试 fixture 已全部迁移。
所有调用方显式创建 component state、保存一个或多个 core `SubscriptionSet`，并从 catalog/config 独立更新
component config。旧 trait/type/属性和 `*BindingHandle` 已删除，workspace-wide 搜索不再有实现引用。

## 8. 系统面与验证

| Surface | 决定 |
| --- | --- |
| UI/focus/keyboard | 复用现有 component state；调用方 set 保存 adapter event subscriptions |
| Data | form draft、component config、interaction state 三 owner |
| Form events | generated field identity + runtime generic event；adapter draft mirror 使用 internal typed handle event |
| Dynamic required | pure form validation/meta command；component affordance 由 app 独立配置 |
| Persistence | app/repository 不变；form 无 store dependency |
| Errors | parse/validation 留 form；catalog unavailable/fallback 留 app |
| Cancellation | component tasks 仍由 component state；binding subscriptions 随 caller set clear/drop |
| DB/schema | No change |
| i18n/icons/assets | No change；label/description/placeholder 由 app UI 配置 |
| Dependencies/MSRV/platform | No change |

```bash
cargo fmt --all
cargo test -p gpui-form
cargo test -p gpui-form-macros
cargo test -p gpui-form-gpui-component --all-features
cargo test -p gpui-form-gpui-component --test adapters --all-features
cargo test -p gpui-store --all-features
cargo check -p jaco
cargo test -p jaco --no-fail-fast
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component --all-targets --all-features -- -D warnings
cargo clippy -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

本阶段完成条件已满足：core public API 的新路径不存在 component state/config 或 component-specific subscription
API；通用 `SubscriptionSet` 保持组件库中立；options 更新不会触发 form mutation；submit 不读取组件 state；
workspace-wide 已删除旧 binding API，provider/MCP 调用方与测试 fixture 均使用显式 adapter 生命周期。
