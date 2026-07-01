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
合法 `Value`，但 number input 的真实编辑态是 raw text，例如 `""`、`"-"`、`"12x"`、`"01"`。这些是合法
UI draft，不一定是合法 `i32` / `u64` / `f64`。

## 当前结果

- `crates/gpui-form` 不再依赖 `gpui-component`。
- leaf field 只使用一个 store：`ComponentFieldStore<Value, Binding>`。
- 用户只实现 binding，不实现 field store；宏只依赖 `gpui-form` 自己的通用 store constructor。
- binding 必须显式建模 `Draft`，数据流统一为 `State -> Draft -> Result<Value, Box<FieldError>>`。
- number 不再需要专门 store；它只是 `Draft = String` 且 `parse_draft(...)` 可能失败的 binding。
- `gpui-component` 相关 binding 拆到新 crate：`crates/gpui-form-gpui-component`。
- group 和 array 继续是组合字段，不纳入 leaf field 统一 store。

## 文件和模块结构

### `crates/gpui-form`

| 文件 | 当前职责 |
| --- | --- |
| `Cargo.toml` | 移除 `gpui-component` 依赖；保留 `gpui`、`gpui-form-macros`、`garde`、`validify`。 |
| `src/component/binding.rs` | 定义新的 draft-aware `FormComponentBinding<Value>`、`ComponentStateOptions`、`FormComponentEvent`。 |
| `src/component/fields/component.rs` | 作为唯一 leaf field store：`ComponentFieldStore<Value, Binding>`，持有 typed value、draft、component state、parse error 和 subscriptions。 |
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
| `src/expand/fields.rs` | leaf 初始化统一生成 `ComponentFieldStore::<Value, Binding>::new(...)`；不再调用任何用户或 adapter store 的 inherent `new`。 |
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
| `src/number.rs` | `NumberInputBinding<N>`，state 为 `InputState`，draft 为 `String`，parse 失败返回 internal field error。 |
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
    type Event: 'static;
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

    fn event_kind(event: &Self::Event) -> Option<FormComponentEvent>;

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool;
}
```

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
}
```

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

用户输入 "12x":
  draft = "12x"
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
Binding::Event
  -> Binding::event_kind(event)
  -> store.sync_from_state(path, trigger, cause, cx)
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

## 测试计划

- `component_field_store_tracks_draft_dirty_when_parse_fails`。
- `component_field_store_tracks_draft_dirty_when_typed_value_is_equal`。
- `component_field_store_normalize_writeback_rebases_draft`。
- `derive_leaf_binding_does_not_reference_gpui_component`，用 `cargo expand` 或 trybuild 编译错误快照验证。
- `gpui_component_number_binding_rejects_unparsable_draft_on_submit`。
- `ai-chat2` focused compile check：`cargo test -p ai-chat2 --no-run`。
