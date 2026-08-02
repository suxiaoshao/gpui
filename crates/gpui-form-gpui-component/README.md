# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-gpui-component` connects typed `gpui-form` fields to
`gpui-component` state entities. The form remains the only business-value and
submit source. Each stateful bound control is a small Rust handle that owns only
the native entity and its synchronization subscriptions, and dereferences to
that entity:

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

`ProviderForm::NAME` exposes the field's single schema-level definition as an
associated `const` descriptor. It is allocation-free and reusable: it provides
only static schema and field access, and never retains this form entity, a value,
or a subscription. Passing `&form` makes the ownership boundary explicit.
For a statically present (total) field, `FormInput::new` returns `Self`, not a
`Result`. A projected or identified (partial) field uses `FormInput::try_new`
and returns `FieldAccessError` if the path no longer exists.

The construction closure configures the native state. There is no adapter
`Config`, delegate copy, binding field, focus flag, or error-visibility state.
`FormSelect<D>` binds `Option<D::Item::Value>` and confirms through
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

Total descriptors have infallible synchronous reads and writes once the caller
supplies an `Entity<Form>`. Only partial descriptors use `try_value` and
`try_set` with normal `FieldAccessError` handling.

See the [user guide](docs/guide.md) and the
[Chinese guide](docs/guide.zh-CN.md).
