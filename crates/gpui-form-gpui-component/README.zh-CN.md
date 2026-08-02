# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-gpui-component` 把类型化 `gpui-form` 字段连接到
`gpui-component` state entity。Form 始终是业务值与提交数据的唯一来源。每个有状态
bound control 都只是一个小型 Rust handle，只持有原生 entity 与同步 subscriptions，并
deref 到该 entity：

```rust,ignore
use gpui_component::input::{Input, InputState};
use gpui_form_gpui_component::FormInput;

let name_input = FormInput::new(
    &form,
    ProviderForm::NAME,
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
);

let element = Input::new(&name_input);
```

`ProviderForm::NAME` 以 associated `const` descriptor 暴露该字段唯一的 schema-level definition，
可复用且不产生 allocation：它只提供静态 schema 与 field access，不持有本次 form entity、value 或 subscription。显式传入 `&form`
才建立所有权边界。静态确定存在的 total field 使用
`FormInput::new`，直接返回 `Self` 而不是 `Result`；projected 或 identified 的 partial
field 使用 `FormInput::try_new`，路径已不存在时返回 `FieldAccessError`。

构造闭包负责配置原生 state。Adapter 不再提供 `Config`，也不保存 delegate 副本、
binding 字段、focus flag 或 error-visibility state。`FormSelect<D>` 绑定
`Option<D::Item::Value>`，通过 `SelectEvent::Confirm` 写入；`FormCombobox<D>`
绑定 `Vec<D::Item::Value>`，通过 `ComboboxEvent::Change` 写入。程序化 form
变更使用原生 value setter 静默投影到所有已挂载实例。

精确整数使用 `FormIntegerInput<N>` 与 `IntegerInputState<N>`，不会把 `u64`、
`i64` 或其他整数绕经 `String` 或 `f64`。不完整或无效的编辑文本只保留在原生
state 中，并产生临时 control issue；它不会覆盖 form 中最后一个合法的类型化值。

Options、delegate、placeholder、disabled state、catalog refresh、dynamic
validation、focus 选择与持久化都属于应用。配置变化时，应用修改暴露出来的原生 state 后，
必须立即通过更新后的 items/options 静默重投影当前 form value；原生 API 无法原地完成时，
直接重建整个 bound handle，不能等待后续 form event。

`Checkbox` 与 `Switch` 没有公开 state entity，因此直接按 controlled element
使用，不制造假的 bound wrapper：

```rust,ignore
use gpui_component::{checkbox::Checkbox, switch::Switch};

let enabled = ProviderForm::ENABLED.value(&self.form, cx);

let checkbox_field = ProviderForm::ENABLED;
let checkbox_form = self.form.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field.set(&checkbox_form, *checked, cx);
    });

let switch_form = self.form.clone();
let switch_field = ProviderForm::ENABLED;
let switch = Switch::new("provider-enabled-switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        switch_field.set(&switch_form, *checked, cx);
    });
```

total descriptor 在调用者显式提供 `Entity<Form>` 后，同步读写不会失败。只有 partial
descriptor 使用 `try_value` 与 `try_set`，并按正常 `FieldAccessError` 处理。

详见[使用指南](docs/guide.zh-CN.md)与[英文指南](docs/guide.md)。
