# Binding Architecture

本文记录 `gpui-form` 已落地的 leaf field binding 架构。目标是删除 core crate 对具体 UI 组件库的默认假设，
并用一个通用 `ComponentFieldStore<Value, Binding>` 覆盖 text、number、bool、select、combobox 和 app
自定义控件。

## 原问题

旧实现把三层职责混在 `crates/gpui-form` 中：

- 表单核心：`FieldCore`、`FieldMeta`、`FormMeta`、errors、validation、submit transform。
- binding 抽象：`FormComponentBinding<Value>`、`ComponentFieldStore<Value, Binding>`。
- `gpui-component` 适配：`TextInputBinding`、`NumberInputBinding`、`BoolBinding`、`SelectBinding`、
  `ComboboxBinding` 和对应 field store。

这会产生两个设计问题：

- `gpui-form` 默认依赖 `gpui-component`，但库使用者不一定使用这套 UI 组件。
- 旧 derive 宏会为内置 binding 生成专用 field store 的 inherent constructor 调用；这些 `new` 不是
  trait contract，用户无法提供等价 store 而让宏稳定调用。

number dirty bug 是同一个抽象问题的表现：当前 `FormComponentBinding<Value>` 假设组件 state 能随时读出
合法 `Value`，但 number input 的真实编辑态是 raw text，例如 `""`、`"-"`、`"."`、`"01"`。这些是否合法还要
看目标 Rust 类型；其中一部分是合法 UI draft，但不一定是合法 `i32` / `u64` / `f64`。

## 当前结果

- `crates/gpui-form` 不再依赖 `gpui-component`。
- leaf field 只使用一个 store：`ComponentFieldStore<Value, Binding>`。
- 用户只实现 binding，不实现 field store；宏只依赖 `gpui-form` 自己的通用 store constructor。
- binding 必须显式建模 `Draft`，数据流统一为 `State -> Draft -> Result<Value, Box<FieldError>>`。
- number 不再需要专门 store；它是 `Draft = String` 且 `parse_draft(...)` 可能失败的 binding，同时
  `NumberFieldValue` 必须按 Rust 目标类型提供 `NumberInputPolicy`，让 `new_state(...)` 对整数、无符号整数和
  浮点数配置不同的 `InputState` 行为。
- `gpui-component` 相关 binding 拆到新 crate：`crates/gpui-form-gpui-component`。
- group 和 array 继续是组合字段，不纳入 leaf field 统一 store。
- binding-owned subscriptions 已落地：derive 宏不再内联 `cx.subscribe_in(&state, ...)`，也不再隐含要求
  `Binding::State: EventEmitter<Binding::Event>`。binding 通过 `install_subscriptions(state, sink, window, cx)`
  自己安装订阅并回传 `FormComponentEvent`。

## 文件和模块结构

### `crates/gpui-form`

| 文件 | 当前职责 |
| --- | --- |
| `Cargo.toml` | 移除 `gpui-component` 依赖；保留 `gpui`、`gpui-form-macros`、`garde`、`validify`。 |
| `src/component/binding.rs` | 定义 draft-aware `FormComponentBinding<Value>`、`ComponentStateOptions`、`FormComponentEvent`、`FormComponentEventSink<Form>`；binding 负责安装 subscriptions。 |
| `src/component/fields/component.rs` | 作为唯一 leaf field store：`ComponentFieldStore<Value, Binding>`，持有 typed value、draft、component state、parse error、writeback guard 和 subscriptions；集中处理 `FormComponentEvent -> ComponentFieldEventOutcome`。 |
| `src/component/fields.rs` | 只 re-export 通用 store 和 core binding；不再 re-export `gpui-component` bindings。 |
| `src/component/fields/{input,number,select,combobox,bool}.rs` | 已从 core 删除，相关 binding 移到 `gpui-form-gpui-component`。 |
| `src/core/field.rs` | 保留 `FieldCore<Value>` 作为 typed value/meta/error holder；dirty/default 由 owner store 传入 draft snapshot。 |
| `src/core/error.rs` | 保留 `FieldError` / `ValidationSource::Internal`，供 binding parse error 使用。 |
| `src/view/render.rs` | 继续只输出 semantic view state，不引用具体 UI 组件。 |

### `crates/gpui-form-macros`

| 文件 | 当前职责 |
| --- | --- |
| `src/field_kind.rs` | leaf field 只区分 `Value` / `Binding`；`input`、`number`、`select`、`combobox`、`bool` 会给出 compile error，提示改用 adapter binding。 |
| `src/attributes.rs` | `binding = "TypePath"` 是 leaf UI 控件唯一扩展点；`component = "group"` / `"array"` 仍用于组合字段。 |
| `src/expand/fields.rs` | leaf 初始化统一生成 `ComponentFieldStore::<Value, Binding>` 和 field event sink；不再调用任何用户或 adapter store 的 inherent `new`，也不再内联 component subscription。 |
| `src/expand/accessors.rs` | leaf 统一生成 `<field>_value()` 和 `<field>_state()`；具体 render helper 属于 adapter crate 或 app。 |
| `src/expand/validation.rs` | submit preflight 对所有 binding 调 `prepare_submit(...)`，由 binding 的 `parse_draft(...)` 产出 typed value 或 internal field error。 |
| `src/expand/arrays.rs` | 不变；array item 仍创建 child generated store。 |

### `crates/gpui-form-gpui-component`

workspace crate，专门承载 `gpui-component` 适配：

| 文件 | 当前职责 |
| --- | --- |
| `Cargo.toml` | 依赖 `gpui`、`gpui-form`、`gpui-component`。 |
| `src/lib.rs` | re-export bindings。 |
| `src/input.rs` | `TextInputBinding<T>`，state 为 `InputState`，draft 为 `String`。 |
| `src/number.rs` | `NumberInputBinding<N>`，state 为 `InputState`，draft 为 `String`，按 `N::input_policy()` 配置 typed number input 行为，parse 失败返回 internal field error。 |
| `src/bool.rs` | `BoolBinding`，draft 为 `bool`。 |
| `src/select.rs` | `SelectBinding<T, D>`，state 为 `SelectState<D>`，draft 由 selected value 或 binding snapshot 表达。 |
| `src/combobox.rs` | `ComboboxBinding<T, D>`，state 为 `ComboboxState<D>`，draft 由 selected values 或 binding snapshot 表达。 |
| `src/number.rs` | 同时提供 `number_input::<N>(state) -> NumberInput` render helper；不进入 `gpui-form` core。 |

禁止新增 `mod.rs`。

## 自定义类型结构

`FormComponentBinding` 目标形态：

```rust
pub trait FormComponentBinding<Value>: Sized + 'static
where
    Value: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Draft: Clone + PartialEq + 'static;

    fn new_state(
        initial: &Value,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State>;

    fn draft_from_value(value: &Value) -> Self::Draft;

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft;

    fn parse_draft(
        draft: &Self::Draft,
        path: FieldPath,
        trigger: ValidationTrigger,
        cx: &App,
    ) -> Result<Value, Box<FieldError>>;

    fn write_value(
        state: &Entity<Self::State>,
        value: &Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    );

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let _ = (state, sink, window, cx);
        SubscriptionSet::default()
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool;
}
```

`FormComponentEventSink<Form>` 是 binding-owned subscription 和 generated form handler 之间的边界：

```rust
pub struct FormComponentEventSink<Form> {
    callback: Rc<dyn Fn(&mut Form, FormComponentEvent, &mut Window, &mut Context<Form>)>,
}
```

binding 只调用 sink，不直接知道 generated field enum / event enum；宏只生成 sink 的 callback，不直接知道
UI library 的事件源。

`ComponentFieldStore` 目标形态：

```rust
pub struct ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    core: FieldCore<Value>,
    state: Entity<Binding::State>,
    default_draft: Binding::Draft,
    draft: Binding::Draft,
    parse_error: Option<FieldError>,
}
```

核心方法：

```rust
impl<Value, Binding> ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    pub fn new(value: Value, state: Entity<Binding::State>) -> Self;
    pub fn state(&self) -> Entity<Binding::State>;
    pub fn sync_from_state(
        &mut self,
        path: FieldPath,
        trigger: ValidationTrigger,
        cause: FieldChangeCause,
        cx: &App,
    ) -> FieldDraftSync<Value>;
    pub fn prepare_submit(&mut self, path: FieldPath, cx: &App) -> Result<Value, Box<FieldError>>;
    pub fn write_component_value(
        &mut self,
        value: &Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    );
    pub fn apply_component_event(
        &mut self,
        path: FieldPath,
        event: FormComponentEvent,
        cx: &App,
    ) -> ComponentFieldEventOutcome;
}
```

`ComponentFieldEventOutcome` 用来让 generated form handler 做字段级 validation 和 typed event emit，但不重复
实现 draft parsing：

```rust
pub enum ComponentFieldEventOutcome {
    Changed { parsed: bool, cause: FieldChangeCause },
    Focused,
    Blurred { parsed: bool },
    Ignored,
}

impl ComponentFieldEventOutcome {
    pub fn validation_trigger(self) -> Option<ValidationTrigger>;
    pub fn field_event_kind(self) -> Option<ComponentFieldEventKind>;
}

pub enum ComponentFieldEventKind {
    Changed,
    Focused,
    Blurred,
}
```

`NumberFieldValue` 是 adapter crate 的 typed number 边界，不属于 `gpui-form` core：

```rust
pub trait NumberFieldValue: Clone + PartialEq + ToString + FromStr + 'static {
    fn input_policy() -> NumberInputPolicy;
    fn step_draft(draft: &str, action: StepAction) -> Option<String> {
        None
    }
}
```

`gpui-component::NumberInput` 的真实 state 是 `InputState`，并且 min/max/step 都以 `f64` 表达。因此
`NumberInputBinding<N>::new_state(...)` 必须按 `N` 配置 input policy：`i32` 类 signed integer 允许符号但不允许
小数，`u32` 类 unsigned integer 不允许符号和小数，`f32/f64` 允许小数。`i64/u64/isize/usize` 这类可能超过
`f64` 安全整数范围的类型不把 max 映射成 `f64`，并把 step 交给 binding 的 `NumberInputEvent::Step`
订阅，用 Rust checked arithmetic 更新 draft。

`FieldDraftSync`：

```rust
pub enum FieldDraftSync<Value> {
    Parsed { value: Value, draft_changed: bool },
    ParseError { error: FieldError, draft_changed: bool },
}
```

`NoComponentBinding<Value>` 是给自定义场景使用的无 UI state binding：

```rust
pub struct NoComponentBinding<Value>(PhantomData<fn() -> Value>);
```

它的 `Draft = Value`，`State = NoComponentState`，`parse_draft` 永远成功。当前默认 value field 仍使用
`ValueFieldStore<T>`，因为它没有 component state 和 event subscription；binding-backed leaf field 才使用
`ComponentFieldStore<Value, Binding>`。

## Dirty 和默认值规则

leaf field 的 dirty/default 只由 draft 决定：

```text
draft_dirty = draft != default_draft
meta.is_dirty = draft_dirty
meta.is_default_value = !draft_dirty
typed value = parse_draft(draft) 成功后的最后一次 Value
parse error = parse_draft(draft) 失败时的 internal FieldError
revision increments when draft_changed || typed_changed || NormalizeOnSubmit
```

number 示例：

```text
Value = i32
Draft = String
default_draft = "12"

用户输入 "-":
  draft = "-"
  parse_draft -> Err(parse)
  typed value 仍是 12
  dirty true
  submit Err(field parse error)

用户输入 "012":
  draft = "012"
  parse_draft -> Ok(12)
  typed value 仍可能等于 12
  dirty true，因为 draft != default_draft
```

## 数据流

初始化：

```text
domain Value
  -> Binding::new_state(Value, ComponentStateOptions)
  -> ComponentFieldStore::new(Value, state)
  -> default_draft = Binding::draft_from_value(Value)
  -> draft = default_draft
```

用户输入：

```text
UI component event
  -> Binding-owned subscription
  -> FormComponentEventSink::emit(FormComponentEvent)
  -> generated field handler
  -> store.apply_component_event(path, event, cx)
  -> Binding::read_draft(state)
  -> compare draft/default_draft for dirty
  -> Binding::parse_draft(draft)
  -> Ok(Value): update FieldCore<Value>, clear parse error
  -> Err(FieldError): keep previous typed value, set parse error
  -> refresh form meta, emit typed form event, notify
```

submit：

```text
prepare_submit
  -> each leaf store reads latest draft from state
  -> parse failure returns preflight FormValidationReport
  -> parse success builds typed candidate
  -> transform/normalize runs on typed candidate
  -> write_draft(normalized, NormalizeOnSubmit)
  -> Binding::write_value(state, normalized)
  -> draft = Binding::draft_from_value(normalized)
  -> dirty/default recomputed from draft
  -> validation final report decides Ok/Err
```

programmatic setter：

```text
set_<field>_value(Value, cause)
  -> FormField::set_value updates typed core
  -> Binding::write_value writes component state
  -> draft = Binding::draft_from_value(Value)
  -> dirty/default recomputed from draft
```

## Derive 宏边界

宏继续生成字段结构 glue code，但不继续拥有组件订阅：

- 保留生成：field enum、typed form event enum、store struct、`draft()` / `write_draft(...)`、
  field accessor/setter/required/error helper、group/array helper、validation report 路由和 `FormStore` impl。
- 下沉到 binding：订阅哪个 state/entity、订阅几个事件源、`subscribe` / `subscribe_in` 的选择、UI event 到
  `FormComponentEvent` 的映射。
- 下沉到 runtime：`FormComponentEvent` 到 `ComponentFieldStore` 的 draft sync、parse error、dirty/default 和
  focus/blur meta 更新。
- 保留少量 generated 事件收尾：每个 generated store 生成一次 `finish_component_field_event(...)`，
  用 runtime outcome 调用 field-scoped validation 并 emit typed form event；具体字段是否启用当前 validation
  trigger 由字段自己的 sink closure 直接读取，不额外生成“字段枚举 -> triggers”的 match。
- 暂不优化：`field_paths: Vec<FieldPath>`，因为当前 `FieldPath` 内部持有 `Vec<FieldPathSegment>`，不能直接用
  const slice 表达。

完整文件结构、类型结构和验证覆盖见 `macro-generation-boundary.md`。

## 所用组件

- `gpui-form` core：无具体 UI 组件。
- `gpui-form-gpui-component` adapter：使用 `gpui_component::input::Input` / `NumberInput`、
  `gpui_component::select::Select`、`gpui_component::combobox::Combobox`、`gpui_component::checkbox::Checkbox`
  或 `Switch` 对应 state。
- app 自定义 binding：由 app 自己选择 `HotkeyInput`、editor、segmented control、model picker 等组件。

## 全局数据管理、数据库和数据获取

- 全局数据管理：无。form store 仍是 `Entity<GeneratedFormStore>`，binding state 仍由字段 store 持有。
- 数据库变更：无。binding draft 不进入 SQLite/config/keychain；submit output 仍是 typed domain struct。
- 数据获取方式：无网络/DB 读取。动态 options、provider/model snapshot、prompt list 等仍由 app 构造 binding
  state 或 field value 时注入。
- icon：`gpui-form` core 不新增 icon；adapter 复用 `gpui-component` 控件内部 icon；app row action icon 仍归 app。
- i18n：`gpui-form` core 继续只保存 key/params；adapter 只消费 `ComponentStateOptions`，不引入 app 文案。
- 新增依赖库：新增 workspace crate `gpui-form-gpui-component`；把 `gpui-component` 从 `gpui-form` dependency
  移到该 adapter crate。`gpui-form` 不新增第三方依赖。

## 已完成的迁移阶段

### Phase A: 引入 draft-aware binding

- 已修改 `FormComponentBinding`，增加 `Draft`、`draft_from_value`、`read_draft`、`parse_draft`。
- 已扩展 `ComponentFieldStore<Value, Binding>`，让 binding-backed leaf field 共用同一个 store。
- 已删除旧内置 field store。
- 测试覆盖 text、number invalid draft、number typed-equal raw dirty、bool、custom binding。

### Phase B: 拆出 `gpui-form-gpui-component`

- 已新建 adapter crate。
- 已移动 `TextInputBinding`、`NumberInputBinding`、`BoolBinding`、`SelectBinding`、`ComboboxBinding` 到 adapter。
- `gpui-form` 已移除 `gpui-component` runtime dependency 和相关 public re-export；测试仍以 dev-dependency 使用
  `gpui-component` 触发 adapter event。
- 宏生成代码不再引用 `::gpui_component::*` 路径。

### Phase C: 收敛 derive 宏语法

- leaf UI 字段统一使用 `#[form(binding = "...")]`。
- 默认无 UI 字段继续使用 `ValueFieldStore<T>`。
- `component = "input"` / `"number"` / `"select"` / `"combobox"` / `"bool"` 已改为明确 compile error，
  提示使用 adapter binding。
- accessors 统一为 `<field>_state()`；adapter-specific render helper 不由 core 宏生成。

### Phase D: 迁移 app

- `app/ai-chat2` 已增加 `gpui-form-gpui-component` dependency。
- Provider/MCP/Prompt/Shortcut form input 已把内置 component 语法改成显式 adapter binding 或 app-local type alias。
- `PromptContentInputBinding`、`ShortcutHotkeyBinding`、`ShortcutPromptSelectBinding`、`ShortcutModelSelectBinding`
  已实现新的 draft-aware binding。
- 保存、validator、DB/config/keychain 写回不变。

### Phase E: binding-owned subscriptions

- 已修改 `FormComponentBinding`，删除 `type Event` 和 `event_kind(...)`。
- 已新增 `FormComponentEventSink<Form>`，让 binding 在 `install_subscriptions(state, sink, window, cx)` 内自行订阅。
- 已删除 derive 宏中的 `State: EventEmitter<Event>` where predicate 和内联 `cx.subscribe_in(...)`。
- 已把 `TextInputBinding`、`NumberInputBinding`、`SelectBinding`、`ComboboxBinding` 和 app-local binding
  的事件订阅迁移到各自 binding 实现内。
- `BoolBinding` 保持 passive state，默认返回空 `SubscriptionSet`；如果未来 bool 组件 state 发事件，只修改
  adapter crate。
- 已把每个字段重复展开的 `Change` / `Focus` / `Blur` 同步逻辑收敛到 `ComponentFieldStore` runtime helper。
- 已删除额外的“字段枚举 -> validation triggers”生成 match，改由具体字段 sink closure 直接读取自己的
  `FieldCore`。
- 已对 macro where predicate 做去重，减少展开代码和错误信息噪音。

## 验证覆盖

- `RequiredFlagState` 不实现 `EventEmitter`，但 `RequiredBindingInput` derive 编译通过，覆盖 binding state
  不再需要隐藏 `State: EventEmitter<Binding::Event>` 约束。
- `derive_installs_binding_component_subscriptions` 覆盖 binding-owned subscription 能把 component state 事件同步回
  form typed value。
- `derive_emits_typed_field_events` 覆盖 adapter-owned input subscription 回传 focus/change/blur 后，generated
  form 仍 emit typed field event。
- `submit_rejects_unparsable_number_input`、`number_raw_edit_with_same_typed_value_stays_dirty`、
  `number_normalize_writeback_recomputes_raw_dirty` 和 `number_reset_restores_raw_default` 覆盖 number raw draft
  仍由 `ComponentFieldStore` 统一管理。
- 后续如要继续降低宏展开噪音，可补 trybuild 或 expand snapshot，专门断言生成代码中不出现
  `Binding::event_kind(...)` 和宏内联 `cx.subscribe_in(&__gpui_form_*_state, ...)`。
- focused 验证命令：`cargo check -p gpui-form -p gpui-form-gpui-component`、`cargo check -p ai-chat2`、
  `cargo test -p gpui-form`、`cargo test -p ai-chat2`。
