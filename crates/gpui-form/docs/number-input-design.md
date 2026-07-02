# Number Input Design

本文记录 number 字段在当前架构中的实现结果。详细 binding 架构见
`binding-architecture.md`。

## 当前实现

number 不再是 `gpui-form` core 的特殊 field store。它由
`crates/gpui-form-gpui-component::NumberInputBinding<N>` 提供：

- `State = gpui_component::input::InputState`
- `Draft = String`
- `Value = N`
- `parse_draft(String) -> Result<N, Box<FieldError>>`
- `NumberFieldValue::input_policy() -> NumberInputPolicy`

`gpui-form` core 只看到通用的 `ComponentFieldStore<Value, Binding>`。dirty/default 以
`Binding::Draft` 为基准，因此这些场景都由同一套逻辑覆盖：

- 输入 `-` 到 signed integer：这是允许的编辑中间态；typed value 保持上一次成功解析值，field 写入 internal parse
  error，dirty 为 true。
- 输入 `012`：typed value 仍可能等于 `12`，但 draft 与 default draft 不同，dirty 为 true。
- reset：typed value、draft、component state、parse error 和 dirty/default 一起回到默认状态。
- normalize writeback：submit transform 写回 canonical typed value 时，adapter 重新生成 draft 并同步 state。

`gpui-component::NumberInput` 本身不是 typed number control。当前依赖版本的真实 API 是：

- `NumberInput::new(&Entity<InputState>)`，state 仍是 text input state。
- number mask 只限制“像数字”：可选正负号、数字、单个小数点和全角数字 normalize。
- `InputState::min(...)`、`max(...)`、`step(...)` 都是 `f64`。
- story/example 中整数语义通过 caller 自己的 `pattern(...)` / `parse::<i64>()` / 手写 step 处理。
- `gpui-component` setting `NumberField` 只读写 `f64`。

因此 `gpui-form-gpui-component` 必须在 binding 的 `new_state(...)` 中按 Rust 目标类型设置 input policy，
不能把所有 `FromStr + ToString` 类型都当作同一种 number。

## 类型策略

`NumberFieldValue` 不再是空 blanket trait。adapter 只为 Rust primitive number 类型提供默认实现；
自定义 number-like 类型必须显式实现 trait 并返回自己的 `NumberInputPolicy`。

| 类型 | `new_state(...)` 行为 |
| --- | --- |
| `i8` / `i16` / `i32` | signed integer：允许 `+` / `-` 中间态，不允许小数；设置类型 `min/max`；组件内部 step 为 `1`。 |
| `u8` / `u16` / `u32` | unsigned integer：不允许正负号，不允许小数；设置 `min = 0` 和类型 `max`；组件内部 step 为 `1`。 |
| `i64` / `isize` | signed integer：允许符号，不允许小数；不把大范围映射到 `f64 min/max`；禁用组件内部 step，改由 binding 订阅 `NumberInputEvent::Step` 并用 Rust checked arithmetic 更新 draft。 |
| `u64` / `usize` | unsigned integer：不允许正负号和小数；设置 `min = 0`，不设置 `f64 max`；禁用组件内部 step，改由 binding 用 Rust checked arithmetic 更新 draft。 |
| `f32` / `f64` | float：允许符号和小数；不设置类型范围；组件内部 step 为 `1.0`。 |

`new_state(...)` 应用策略的顺序：

```text
InputState::new(...)
  -> default_value(N::to_string())
  -> NumberFieldValue::input_policy().apply_to(InputState)
  -> placeholder / masked / required 等 ComponentStateOptions
  -> 若 component_step 为 None，再 set_step(None) 让 NumberInputEvent::Step 回到 binding
```

`parse_draft(...)` 仍是最终保护。UI policy 只改善输入体验和 step 行为，不能替代 Rust 类型边界、app-specific
range、DB/config 约束或 submit-time validator。

## 文件和模块

| 文件 | 职责 |
| --- | --- |
| `crates/gpui-form/src/component/fields/component.rs` | 保存 typed value、draft、default draft、component state 和 parse error。 |
| `crates/gpui-form-gpui-component/src/number.rs` | 提供 `NumberInputBinding<N>`、`NumberInputPolicy`、primitive `NumberFieldValue` impl、typed step fallback 和 `number_input::<N>(&state)` render helper。 |
| `crates/gpui-form-macros/src/expand/fields.rs` | 为 binding 字段创建 `FormComponentEventSink`；具体 `InputEvent` / `NumberInputEvent` 订阅由 binding 自己安装。 |
| `crates/gpui-form-macros/src/expand/validation.rs` | submit preflight 对所有 binding 调 `prepare_submit(...)`，number parse error 作为 internal field error 返回。 |
| `crates/gpui-form/tests/derive.rs` | 覆盖 invalid draft、typed-equal draft dirty、type-specific `new_state` policy、reset、normalize writeback。 |

## 所用组件

| 用途 | 组件 / 类型 |
| --- | --- |
| number 视觉控件 | `gpui_component::input::NumberInput` |
| number state | `gpui_component::input::InputState` |
| text draft | `String` |
| parse error | `FieldError { source: ValidationSource::Internal, code: "parse" }` |

app 渲染 number 字段时不再依赖 core 宏生成 `field_number_input()`。应从 generated
`<field>_state()` 取出 `InputState`，再使用 adapter helper：

```rust
let state = form.amount_state();
let input = gpui_form_gpui_component::number_input::<i32>(&state);
```

## 非目标

- 不在 `gpui-form` core 中依赖 `gpui-component`。
- 不新增数据库字段、config key 或 keychain 数据。
- 不新增 icon；使用 `NumberInput` 内部已有加减按钮资源。
- 不新增 i18n key；parse error 继续使用 `gpui-form-error-number-parse`。
- 不下沉 app-specific range、capability clamp 或 token budget 规则。
