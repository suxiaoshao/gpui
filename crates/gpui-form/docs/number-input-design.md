# Number Input Design

本文记录 `gpui-form` number 字段的组件选择、raw input dirty/default 语义和后续修复计划。

## 当前问题

`NumberInputBinding<N>` 目前只创建 `Entity<InputState>`，generated accessor 也只暴露
`field_input_state()`。接入 app 可以把这个 state 渲染成普通 `Input::new(...)`，从而绕过
`gpui_component::input::NumberInput` 的 number mask、全角数字归一化、step/min/max 和加减按钮行为。

更重要的是，number 字段当前把 `FieldCore<N>` 的 typed value 当作 dirty/default 基准。用户把输入框改成
typed value 无法解析的文本时，宏只写入 internal parse error，`FieldCore<N>` 的 value、dirty 和 revision
仍停留在上一次可解析值。结果是可见输入已经变化，但 `form.meta().is_dirty` 可能仍为 false。

这不是单纯 UI 问题，而是 number field 同时存在 raw UI draft 和 typed domain draft，却只把 typed draft
建模为字段状态。

## 设计目标

- `component = "number"` 必须以 `gpui_component::input::NumberInput` 作为默认渲染组件；app 不应把 number
  字段的 state 渲染成普通 `Input`。
- dirty/default 判断以 raw input 文本为基准，而不是以最后一次成功解析的 typed value 为基准。
- parse 成功时同步 typed draft；parse 失败时保留上一次 typed draft，但 raw draft、dirty、revision 和
  internal parse error 必须同步更新。
- reset、programmatic setter 和 submit normalize 必须同时写回 raw draft、typed draft 和 component state。
- `NumberInput` 的 mask/step/min/max 是 UI 约束，不替代 submit preflight；submit 仍必须从当前 raw input
  重新 parse，失败时返回 internal field error。
- 不新增 app-specific 数字规则；range、capability clamp、token budget 等业务规则仍由接入 app validator 或
  app binding 负责。

## 文件和模块结构

| 文件 | 计划 |
| --- | --- |
| `crates/gpui-form/src/component/fields/number.rs` | 调整 `NumberInputBinding` 和 `NumberFieldStore`：number state 仍是 `InputState`，但内置 render helper 使用 `NumberInput`；store 新增 raw baseline/draft 语义，dirty/default 从 raw 文本计算。 |
| `crates/gpui-form/src/component/fields.rs` | 导出 number render helper 所需类型；不新增模块入口 `mod.rs`。 |
| `crates/gpui-form/src/core/field.rs` | 给 `FieldCore<T>` 增加由 owner 传入 dirty/default snapshot 的 meta 刷新方法，供 number/array 这类非 typed-value dirty 源使用。 |
| `crates/gpui-form/src/core/meta.rs` | 不新增 number 专用字段；`FieldMeta.is_dirty` / `is_default_value` 继续作为 owner 计算后的 render snapshot。 |
| `crates/gpui-form-macros/src/expand/fields.rs` | number `InputEvent::Change` 调用 `NumberFieldStore::sync_raw_input(...)`，parse 成功再触发 change validation；parse 失败仍刷新 meta、emit typed form event、notify。 |
| `crates/gpui-form-macros/src/expand/accessors.rs` | number 字段继续保留 `field_input_state()`，并新增明确的 `field_number_input()` 或等价 helper，生成 `NumberInput::new(&state)`，引导 app render 使用正确组件。 |
| `crates/gpui-form-macros/src/expand/validation.rs` | submit preflight 使用 number store 的 raw parse API；不要直接从 stale typed value 构造 candidate。 |
| `crates/gpui-form/tests/derive.rs` | 增加 number raw dirty 回归测试：invalid raw edit、parse 成功但 typed value 相同的 raw edit、reset、normalize 写回。 |

禁止新增 `mod.rs`。

## 所用组件

| 用途 | 组件 / 类型 | 说明 |
| --- | --- | --- |
| number 字段渲染 | `gpui_component::input::NumberInput` | 默认 number 视觉组件，提供 number mask、全角数字归一化、step/min/max 和加减按钮。 |
| number 字段状态 | `gpui_component::input::InputState` | `NumberInput::new(&state)` 的底层 state；仍由 `NumberInputBinding` 创建和订阅。 |
| 普通文本字段 | `gpui_component::input::Input` | 只用于 `component = "input"`；不作为 number 字段默认渲染组件。 |
| 错误渲染 | `gpui_component::form::field().error(...)` | parse error 继续作为 field error 暴露给 app render。 |

`NumberInput` 文档确认其底层仍是 `Entity<InputState>`，所以类型层不需要引入新的 component state；关键是
binding/render helper 必须把 number 字段的 state 放进 `NumberInput::new(&state)`。

## 自定义类型结构

`NumberFieldStore<N>` 继续暴露 typed `Value = N`，但 dirty/default 的字段基准改为 raw input：

```rust
pub struct NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    core: FieldCore<N>,
    input_state: Entity<InputState>,
    raw_default: String,
    raw_value: String,
    raw_revision: u64,
    parse_error: Option<FieldError>,
}
```

语义：

- `raw_default`：初始化或 reset/rebase 后的 raw 文本基线，通常是 `initial.to_string()`。
- `raw_value`：最近一次由 binding 处理过的 raw input 文本；它是 form store 的 raw draft，不是独立 UI
  事实源。更新入口只能是 generated subscription、generated setter、reset 或 submit normalize。
- `core.value`：最后一次成功解析的 typed draft，用于 `draft()`、validation preview 和 submit candidate。
- `raw_revision`：raw 文本变化次数；当 raw parse 失败但可见文本变化时也递增，避免 revision 只跟 typed
  value 变化绑定。
- `parse_error`：internal parse error 缓存；parse 成功、reset、clear errors 或 normalize 写回时清除。

新增或调整的方法目标：

```rust
impl<N> NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    pub fn number_input(&self) -> NumberInput;

    pub fn sync_raw_input(
        &mut self,
        raw_text: String,
        path: FieldPath,
        trigger: ValidationTrigger,
        cause: FieldChangeCause,
    ) -> NumberInputSync<N>;

    pub fn parse_raw_for_submit(&mut self, path: FieldPath, cx: &App) -> Result<N, FieldError>;

    pub fn write_component_value(
        &mut self,
        value: &N,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    );

    pub fn reset(&mut self, window: &mut Window, cx: &mut App);
}

pub enum NumberInputSync<N> {
    Parsed { value: N, raw_changed: bool },
    ParseError { raw_changed: bool, error: FieldError },
}
```

`FieldCore<T>` 需要一个 owner-driven meta 刷新入口，避免 number store 调 `set_value` 后又被 typed equality 覆盖：

```rust
impl<T> FieldCore<T>
where
    T: Clone + PartialEq + 'static,
{
    pub fn refresh_meta_from_default_state(
        &mut self,
        is_default_value: bool,
        cause: FieldChangeCause,
        changed: bool,
    );
}
```

number store 调用时使用：

```text
is_default_value = raw_value == raw_default
changed = raw_changed || typed_value_changed || cause == NormalizeOnSubmit
```

## Dirty 和默认值规则

```text
raw_dirty = raw_value != raw_default
typed_changed = parsed_value != core.value

meta.is_dirty = raw_dirty
meta.is_default_value = !raw_dirty
meta.is_touched = previous_touched || cause marks touched || raw_changed
revision increments when raw_changed || typed_changed || NormalizeOnSubmit
```

典型场景：

- 初始值 `12`，用户输入 `12x`：parse 失败，typed value 仍是 `12`，但 `raw_value = "12x"`，
  `is_dirty = true`，submit 返回 parse error。
- 初始值 `1`，用户输入 `01`：parse 成功且 typed value 仍可能等价为 `1`，但 raw 不等于默认 raw，
  `is_dirty = true`；submit normalize 如果写回 `"1"`，dirty 才回 false。
- 用户输入 `-`、`+`、`.`：`NumberInput` mask 可能允许这些编辑态继续存在；parse 失败时仍按 raw dirty 处理。
- reset：`raw_value = raw_default`，typed value 回默认值，component state 写回 raw default，errors 清空，
  dirty false。

## 数据流

初始化：

```text
domain N
  -> raw_default = N::to_string()
  -> raw_value = raw_default.clone()
  -> FieldCore<N>::new(N)
  -> NumberInputBinding::new_state creates InputState::default_value(raw_default)
  -> generated accessor renders NumberInput::new(&input_state)
```

用户输入：

```text
NumberInput/InputState emits InputEvent::Change
  -> generated subscription reads raw text from InputState
  -> NumberFieldStore::sync_raw_input(raw_text, Change, UserInput)
  -> if parse ok: update typed FieldCore<N>, clear parse error
  -> if parse err: keep typed FieldCore<N>, set internal parse error
  -> refresh field meta from raw_value/raw_default
  -> refresh form meta and emit typed form event
```

submit：

```text
prepare_submit(cx)
  -> number field parse_raw_for_submit(current raw)
  -> parse failure returns preflight FormValidationReport
  -> parse success updates typed draft if needed
  -> transform/normalize runs on typed candidate
  -> normalized value writes raw + typed + InputState
  -> final validation report decides Ok/Err
```

programmatic setter：

```text
set_<field>_value(N, cause, window, cx)
  -> raw_text = N::to_string()
  -> update typed FieldCore<N>
  -> raw_value = raw_text
  -> InputState::set_value(raw_text)
  -> dirty/default from raw_value/raw_default
```

## 全局数据管理、数据库和数据获取

- 全局数据管理：无。number field 状态只存在于 generated form store 和 `InputState` entity 内。
- 数据库变更：无。raw number input 不写入 SQLite、config 或 credentials；提交输出仍是 typed `N`。
- 数据获取方式：无网络/DB 读取；只读取当前 `InputState` raw text 和 form store typed/raw draft。
- icon：复用 `NumberInput` 内部的 `IconName::Minus` / `IconName::Plus`；`gpui-form` 不新增 icon。
- i18n：不新增用户可见 key；parse error 继续使用 `gpui-form-error-number-parse`，错误参数保留 raw value。
- 新增依赖：无。继续使用 workspace 里的 `gpui-component`。

## ai-chat2 接入约束

- `app/ai-chat2` 如果后续把 token budget 或其他数字字段迁移进 generated form，必须渲染 generated
  `field_number_input()`；如果直接使用 state，也必须先取出 `let state = form.field_input_state();`，再渲染
  `NumberInput::new(&state)`，不能渲染普通 `Input`。
- token budget 的 min/max/default clamp 仍由 `thinking_effort::token_budget_bounds` 和 app validator 决定；
  `gpui-form` 只负责 raw parse、dirty/default 和 field error。
- 现有 Provider/MCP/Prompt/Shortcut 不因为 number design 新增数据库字段、i18n key 或依赖。

## 测试计划

- `invalid_number_raw_edit_marks_form_dirty`：raw 输入 parse 失败时 field/form dirty true，submit 返回 parse error。
- `number_raw_edit_with_same_typed_value_stays_dirty`：例如 `1 -> 01`，typed value 相等但 raw dirty true。
- `number_reset_restores_raw_default`：reset 同步 typed、raw、InputState、errors 和 dirty。
- `number_normalize_writeback_recomputes_raw_dirty`：normalize 写回 canonical raw 后重新计算 dirty。
- `number_accessor_renders_number_input`：生成的 number render helper 返回 `NumberInput`，不是要求 app 手动用
  `Input::new` 包装 state。
