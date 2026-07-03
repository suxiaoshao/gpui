# Derive Macro Generation Boundary

状态：已实现。本文记录 `FormStore` derive 宏的瘦身边界：宏只生成 Rust
类型系统无法从普通数据结构推导出的 glue code；组件订阅、字段事件处理和重复的 runtime 分支已下沉到
`gpui-form` runtime / binding trait。

## 问题

以旧 `ApiKeyProviderFormStore` 展开代码为例，原宏生成了下面几类本可以由普通 Rust 抽象承载的逻辑：

- 在 `impl where` 中为每个 binding 字段添加隐藏约束：
  `<Binding as FormComponentBinding<Value>>::State: gpui::EventEmitter<Binding::Event>`。
- 同时调用 `Binding::install_subscriptions(...)` 和宏内联 `cx.subscribe_in(&state, ...)`，形成两套
  subscription 入口。
- 为每个 binding 字段重复展开完整的 `Change` / `Focus` / `Blur` match、`sync_from_state`、
  validation trigger、`refresh_meta`、`cx.emit(...)` 和 `cx.notify()`。
- 对相同类型字段重复生成 where predicate，例如多个 `StringInputBinding` 字段重复出现相同 bound。
- 用 form 级 `is_normalizing_on_submit` 抑制 programmatic write 触发的组件事件；这个状态和具体字段
  writeback 相关，不应该作为整个 form store 的长期同步事实源。

## 实现边界

宏保留“只有 derive 才知道”的信息：

- 输入 struct 的字段列表、字段名和字段类型。
- generated field enum / event enum 的 variant。
- 字段路径和 validator report 的路由。
- `draft()` / `write_draft(...)` 的 struct destructuring / construction。
- group / array child store 的具体类型参数和 array helper。
- app 可直接调用的 field accessor、setter、required setter 和 error helper。

宏不再生成具体组件订阅逻辑：

- binding 自己决定订阅哪个 entity、使用 `subscribe` 还是 `subscribe_in`、订阅一个还是多个事件源。
- binding 自己把 UI library 的事件翻译为 `FormComponentEvent`。
- form store 只接收 binding 回传的 `FormComponentEvent`，并把它映射到具体字段的表单副作用。

## 文件和模块结果

### `crates/gpui-form`

| 文件 | 结果 |
| --- | --- |
| `src/component/binding.rs` | 新增 `FormComponentEventSink<Form>`；调整 `FormComponentBinding<Value>`，删除 `type Event` 和 `event_kind(...)`，让 `install_subscriptions(...)` 成为唯一订阅入口。 |
| `src/component/fields/component.rs` | 新增 `apply_component_event(...) -> ComponentFieldEventOutcome`，集中处理 `sync_from_state`、focus/blur meta、parse error 和 dirty/default 更新。 |
| `src/core/subscriptions.rs` | 保留 `SubscriptionSet` 作为订阅生命周期容器；补 `Extend<Subscription>` / `FromIterator<Subscription>`，不引入新依赖。 |
| `src/macro_support.rs` | 增加 `component_field(...)` 和 `component_field_event_trigger(...)` 这类 derive 专用 helper，承载 component field 初始化、field path 构造和字段事件 outcome 辅助查询。 |

### `crates/gpui-form-macros`

| 文件 | 结果 |
| --- | --- |
| `src/expand.rs` | 删除 binding 字段的 `State: EventEmitter<Event>` where predicate；对重复 where predicate 做去重；继续生成 field enum、event enum、store struct 和 trait impl；不再生成额外的“字段 -> validation triggers”match。 |
| `src/expand/fields.rs` | 删除内联 `cx.subscribe_in(...)`；binding 字段初始化时创建 `FormComponentEventSink`，调用 `Binding::install_subscriptions(state, sink, window, cx)` 并把返回的 `SubscriptionSet` 存入 field core。 |
| `src/expand/accessors.rs` | 继续生成具体字段 accessor/setter/required/error helper；setter 中重复的 change/blur validation 判断改为调用 runtime helper。 |
| `src/expand/validation.rs` | 继续生成字段路径到 report 的路由，因为这依赖具体字段名、group store 和 array item store 类型。 |
| `src/expand/arrays.rs` | 保留 array helper 生成；array 结构性操作仍需要具体字段名、item store 类型和 generated form event。 |

### `crates/gpui-form-gpui-component`

| 文件 | 结果 |
| --- | --- |
| `src/input.rs` | `TextInputBinding<T>::install_subscriptions(...)` 订阅 `InputState` 的 `InputEvent`，映射 `Change` / `Focus` / `Blur` 后调用 sink。 |
| `src/number.rs` | 同 `input.rs`，继续使用 `NumberInput`/raw input 作为 UI 基准；parse 仍在 `parse_draft(...)`。 |
| `src/select.rs` | `SelectBinding<T, D>` 在 adapter 内订阅 `SelectEvent<D>`，只把会改变 selected value 的事件映射为 `Change(UserInput)`。 |
| `src/combobox.rs` | `ComboboxBinding<T, D>` 在 adapter 内订阅 combobox selection/input 事件，按现有 draft 规则映射 change/focus/blur。 |
| `src/bool.rs` | 当前 `BoolBinding` 是 passive state，`install_subscriptions(...)` 返回空 `SubscriptionSet`；如果后续 bool state 变成事件源，由 adapter 自己添加订阅，不影响宏。 |

### `app/ai-chat2`

| 文件 | 结果 |
| --- | --- |
| `features/settings/prompts/form_state.rs` | `PromptContentInputBinding` 删除 `type Event` / `event_kind(...)`，在 `install_subscriptions(...)` 内订阅 `InputState`。 |
| `features/settings/shortcuts/form_state.rs` | `ShortcutHotkeyBinding`、`ShortcutPromptSelectBinding`、`ShortcutModelSelectBinding` 删除宏依赖的 `EventEmitter` 约束，改为 binding-owned subscriptions。 |
| `features/settings/provider/forms/custom_openai.rs` | `ProviderApiModeSelectBinding` 在 binding 内订阅 `SelectState` 事件并调用 sink。 |

## 当前类型结构

当前 `FormComponentBinding`：

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

`FormComponentEventSink<Form>`：

```rust
pub struct FormComponentEventSink<Form> {
    callback: Rc<dyn Fn(&mut Form, FormComponentEvent, &mut Window, &mut Context<Form>)>,
}

impl<Form> FormComponentEventSink<Form> {
    pub fn new(
        callback: impl Fn(&mut Form, FormComponentEvent, &mut Window, &mut Context<Form>)
            + 'static,
    ) -> Self;

    pub fn emit(
        &self,
        form: &mut Form,
        event: FormComponentEvent,
        window: &mut Window,
        cx: &mut Context<Form>,
    );
}
```

`ComponentFieldEventOutcome`：

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

`ComponentFieldStore<Value, Binding>` 新增方法：

```rust
impl<Value, Binding> ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    pub fn apply_component_event(
        &mut self,
        path: FieldPath,
        event: FormComponentEvent,
        cx: &App,
    ) -> ComponentFieldEventOutcome;
}
```

宏生成的 sink closure 只保留字段专属部分：

```rust
let sink = FormComponentEventSink::new(|this, event, _window, cx| {
    let outcome = this.api_key.apply_component_event(field_path("api_key"), event, cx);
    let field_allows_validation_trigger = outcome
        .validation_trigger()
        .is_some_and(|trigger| this.api_key.core().validation_triggers().contains(trigger));
    this.finish_component_field_event(
        ApiKeyProviderFormField::ApiKey,
        field_path("api_key"),
        outcome,
        field_allows_validation_trigger,
        cx,
    );
});
```

`finish_component_field_event(...)` 由宏为每个 generated store 生成一次，不为每个字段复制完整
change/focus/blur match，也不生成额外的字段枚举 match 来查询 validation triggers。具体字段 sink closure 直接从
`this.<field>.core().validation_triggers()` 计算当前 trigger 是否启用，再把 bool 传给收尾函数。收尾函数只做
generated 类型相关的事情：按 `outcome.validation_trigger()` 调用 `apply_validation_for_scope(...)`，按
`outcome.field_event_kind()` emit `ApiKeyProviderFormEvent::FieldChanged` / `FieldFocused` / `FieldBlurred`，
然后 `refresh_meta()` 和 `cx.notify()`。runtime helper 不反向依赖 generated field enum 或 event enum。

## 数据流

初始化：

```text
domain Value
  -> Binding::new_state(Value, ComponentStateOptions)
  -> ComponentFieldStore::new(Value, state)
  -> macro builds FormComponentEventSink for concrete field
  -> Binding::install_subscriptions(state, sink, window, cx)
  -> field core owns returned SubscriptionSet
```

用户输入：

```text
UI component emits UI-library event
  -> Binding-owned subscription receives concrete event
  -> Binding maps event to FormComponentEvent
  -> sink.emit(form, FormComponentEvent, window, cx)
  -> generated field handler calls ComponentFieldStore::apply_component_event(...)
  -> runtime syncs State -> Draft -> Result<Value, FieldError>
  -> generated handler runs field-scoped validation if trigger requires it
  -> generated handler emits typed FormEvent and notifies
```

programmatic setter / normalize：

```text
generated set_<field>_value(...) or write_draft(...)
  -> ComponentFieldStore::write_component_value(...)
  -> Binding::write_value(state, value, cause, window, cx)
  -> field store updates draft/default/parse error
  -> generated handler refreshes meta and emits FieldChanged
```

如果 `Binding::write_value(...)` 同步触发 UI `Change` 事件，抑制逻辑收口到字段级 runtime 状态：
`ComponentFieldStore` 内部的 writeback guard，而不是继续使用 form 级
`is_normalizing_on_submit` 作为全局同步开关。

## 宏生成职责清单

必须保留生成：

- `ApiKeyProviderFormField` 这类 field enum、`key()`、`from_key()`。
- `ApiKeyProviderFormEvent` 这类 typed form event enum。
- generated store struct 的具体字段和 visibility。
- `from_value(...)` 中的 input destructuring、每个 field 的 attribute-derived options/triggers/required。
- `draft()`、`write_draft(...)`、`reset(...)` 对具体 struct 字段的拆装。
- `<field>_value()`、`<field>_state()`、`set_<field>_value(...)`、`<field>_required()`、
  `set_<field>_required(...)`、field error helpers。
- group / array 的具体 child store 调用、array item helper 和路径前缀路由。
- validator / transform adapter 的调用点和 `FormStore` / `GeneratedFormStore` impl。

应该下沉到 runtime 或 binding：

- 组件订阅安装方式，包括 `subscribe` / `subscribe_in` 的选择。
- UI-library event 到 `FormComponentEvent` 的映射。
- `State: EventEmitter<Event>` 约束和 `Binding::Event` 关联类型。
- 每个 binding 字段重复展开的 change/focus/blur sync 分支。
- change/blur validation trigger 的重复判断，可由 `ComponentFieldEventOutcome` 或
  `macro_support` helper 表达。
- “字段枚举 -> validation triggers”的额外 match；字段自己的 sink closure 已经知道具体字段。
- 重复 where predicate 去重。
- form 级 `is_normalizing_on_submit`，优先替换为字段级 writeback guard。

暂不优化：

- `field_paths: Vec<FieldPath>` 仍保留。当前 `FieldPath` 内部持有 `Vec<FieldPathSegment>`，无法直接变成
  const slice；要改成静态路径描述需要先调整 `FieldPath` 表示，不属于本次 subscription 边界修复。
- generated field enum / event enum 不下沉到 runtime。它们是 app 可订阅的 typed API，必须保留具体类型。
- array helper 不下沉。array helper 依赖 item store 类型、字段名和 generated event，普通 trait 难以保持同等可读性。

## 所用组件

- `gpui-form` core 不新增 UI 组件。
- `gpui-form-gpui-component` 继续使用 `gpui-component` 的 `Input` / `NumberInput` / `Select` /
  `Combobox` / `Checkbox` / `Switch`。
- app-local binding 继续使用各自已有组件，例如 prompt multiline input、shortcut hotkey input 和 model select。

## 全局数据管理、数据库和数据获取

- 全局数据管理：无新增 `Global`，不接 `gpui-store`。
- 数据库变更：无。宏生成边界不改变 submit output 和 app persistence。
- 数据获取方式：无新增网络、DB 或 config 读取；动态 options 仍由 app 构造 state 或 draft 时注入。
- icon：无新增 icon。
- i18n：无新增文案；required label、placeholder、validation message 的 i18n 仍按现有 key 流转。
- 新增依赖库：无。`FormComponentEventSink` 如需共享 callback，使用 `std::rc::Rc`。

## 验证覆盖

- `RequiredFlagState` 不实现 `EventEmitter`，但 `RequiredBindingInput` derive 编译通过，覆盖“binding state 不再
  需要隐藏 `State: EventEmitter<Binding::Event>` 约束”。
- `derive_installs_binding_component_subscriptions` 覆盖 binding 在 `install_subscriptions(...)` 内订阅事件；
  事件触发后 form typed value 会从 component state 同步。
- `derive_emits_typed_field_events` 覆盖 adapter-owned input subscription 回传 focus/change/blur 后，generated
  form 仍 emit typed field event。
- `submit_rejects_unparsable_number_input`、`number_raw_edit_with_same_typed_value_stays_dirty`、
  `number_normalize_writeback_recomputes_raw_dirty` 和 `number_reset_restores_raw_default` 覆盖 number raw draft
  仍由 `ComponentFieldStore` 统一管理。
- `cargo check -p ai-chat2` 覆盖 app-local `PromptContentInputBinding`、`ShortcutHotkeyBinding`、
  `ShortcutPromptSelectBinding`、`ShortcutModelSelectBinding` 和 `ProviderApiModeSelectBinding` 都已适配新
  trait surface。
- `cargo clippy -p gpui-form -p gpui-form-gpui-component -p ai-chat2 --all-targets --all-features -- -D warnings`
  覆盖宏 helper、adapter subscription 和 app-local binding 没有新增 lint。
