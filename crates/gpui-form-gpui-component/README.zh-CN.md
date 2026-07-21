# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

> **实现状态：**这份 README 描述已经实现的公开 API。

`gpui-form-gpui-component` 把类型化 `gpui-form` 字段连接到
`gpui-component` state entity。Form 始终是业务值与提交数据的唯一来源。每个有状态
bound control 都只是一个小型 Rust handle，只持有原生 entity 与同步 subscriptions，并
deref 到该 entity：

```rust,ignore
use gpui_component::input::{Input, InputState};
use gpui_form::FormControl as _;
use gpui_form_gpui_component::FormInput;

let name_input = FormInput::new(
    ProviderInputFormStore::name_field(&form),
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
)?;

let element = Input::new(&name_input);
```

构造闭包负责配置原生 state。Adapter 不再提供 `Config`，也不保存 delegate 副本、
attachment 字段、focus flag 或 error-visibility state。`FormSelect<D>` 绑定
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

let enabled_field = ProviderInputFormStore::enabled_field(&self.form);
let enabled = enabled_field
    .value(cx)
    .expect("ProviderPage 在 render 期间持有 form");

let checkbox_field = enabled_field.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field
            .set_user_value(*checked, cx)
            .expect("element 挂载期间 ProviderPage 持有 form");
    });

let switch = Switch::new("provider-enabled-switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        enabled_field
            .set_user_value(*checked, cx)
            .expect("element 挂载期间 ProviderPage 持有 form");
    });
```

这里的 `expect` 用来记录 render 期间的结构生命周期不变量；如果 form 或 projected
path 在正常业务流程中确实可能消失，应使用普通 `Result` 处理。

详见[使用指南](docs/guide.zh-CN.md)、[英文指南](docs/guide.md)与
[实施计划](dev/typed-bound-controls.md)。
