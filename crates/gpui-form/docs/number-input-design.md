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

`gpui-form` core 只看到通用的 `ComponentFieldStore<Value, Binding>`。dirty/default 以
`Binding::Draft` 为基准，因此这些场景都由同一套逻辑覆盖：

- 输入 `12x`：typed value 保持上一次成功解析值，field 写入 internal parse error，dirty 为 true。
- 输入 `012`：typed value 仍可能等于 `12`，但 draft 与 default draft 不同，dirty 为 true。
- reset：typed value、draft、component state、parse error 和 dirty/default 一起回到默认状态。
- normalize writeback：submit transform 写回 canonical typed value 时，adapter 重新生成 draft 并同步 state。

## 文件和模块

| 文件 | 职责 |
| --- | --- |
| `crates/gpui-form/src/component/fields/component.rs` | 保存 typed value、draft、default draft、component state 和 parse error。 |
| `crates/gpui-form-gpui-component/src/number.rs` | 提供 `NumberInputBinding<N>` 和 `number_input::<N>(&state)` render helper。 |
| `crates/gpui-form-macros/src/expand/fields.rs` | 对所有 binding 统一订阅事件并调用 `sync_from_state(...)`。 |
| `crates/gpui-form-macros/src/expand/validation.rs` | submit preflight 对所有 binding 调 `prepare_submit(...)`，number parse error 作为 internal field error 返回。 |
| `crates/gpui-form/tests/derive.rs` | 覆盖 invalid draft、typed-equal draft dirty、reset、normalize writeback。 |

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
