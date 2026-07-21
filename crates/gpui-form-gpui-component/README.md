# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

> **Implementation status:** this README documents the implemented public API.

`gpui-form-gpui-component` connects typed `gpui-form` fields to
`gpui-component` state entities. The form remains the only business-value and
submit source. Each stateful bound control is a small Rust handle that owns only
the native entity and its synchronization subscriptions, and dereferences to
that entity:

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

The construction closure configures the native state. There is no adapter
`Config`, delegate copy, attachment field, focus flag, or error-visibility
state. `FormSelect<D>` binds `Option<D::Item::Value>` and confirms through
`SelectEvent::Confirm`; `FormCombobox<D>` binds `Vec<D::Item::Value>` and writes
on `ComboboxEvent::Change`. Programmatic form changes are silently projected to
every mounted instance through the native value setters.

Exact integers use `FormIntegerInput<N>` and `IntegerInputState<N>` instead of
routing `u64`, `i64`, or another integer through `String` or `f64`. Incomplete
or invalid editor text stays inside the native state and creates a temporary
control issue; it never replaces the last valid typed form value.

Options, delegates, placeholders, disabled state, catalog refresh, dynamic
validation, focus choice, and persistence remain application concerns. Change
the exposed native state when configuration changes, then immediately silently
reproject the current form value through the updated items/options. If the
native API cannot do both in place, rebuild the bound handle; do not wait for a
later form event.

`Checkbox` and `Switch` have no public state entity, so use them as controlled
elements rather than creating an artificial bound wrapper:

```rust,ignore
use gpui_component::{checkbox::Checkbox, switch::Switch};

let enabled_field = ProviderInputFormStore::enabled_field(&self.form);
let enabled = enabled_field
    .value(cx)
    .expect("ProviderPage owns the form while rendering");

let checkbox_field = enabled_field.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field
            .set_user_value(*checked, cx)
            .expect("ProviderPage owns the form while this element is mounted");
    });

let switch = Switch::new("provider-enabled-switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        enabled_field
            .set_user_value(*checked, cx)
            .expect("ProviderPage owns the form while this element is mounted");
    });
```

The `expect` calls document a structural lifetime invariant during rendering;
use normal `Result` handling where the form or projected path can legitimately
disappear.

See the [user guide](docs/guide.md), the
[Chinese guide](docs/guide.zh-CN.md), and the
[implementation plan](dev/typed-bound-controls.md).
