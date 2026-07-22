# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

> **Implementation status:** this README documents the implemented public API.

`gpui-form` is a typed form-state, validation, and submit-preparation library
for GPUI applications. One generated form store owns the current Rust model;
controls are synchronized projections of that model, not additional business
state.

## Quick start

Declare the exact model that the application will submit:

```rust,ignore
use gpui_form::FormStore;

#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[form(validation(adapter = "garde"))]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    name: String,

    #[form(validate(on_submit))]
    #[garde(range(min = 0, max = 10))]
    retry_limit: u32,
}
```

Create one form entity and create each bound control from its typed field:

```rust,ignore
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::InputState;
use gpui_form::{FormControl as _, SubmitError};
use gpui_form_gpui_component::{
    FormControlError, FormInput, FormIntegerInput, IntegerInputState,
};

struct ProviderPage {
    form_subscription: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
    form: Entity<ProviderInputFormStore>,
}

impl ProviderPage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Result<Self, FormControlError> {
        let form = cx.new(|cx| {
            ProviderInputFormStore::from_value(
                ProviderInput {
                    name: String::new(),
                    retry_limit: 3,
                },
                cx,
            )
        });

        let name_input = FormInput::new(
            ProviderInputFormStore::name_field(&form),
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        )?;
        let retry_limit_input = FormIntegerInput::new(
            ProviderInputFormStore::retry_limit_field(&form),
            |window, cx| {
                IntegerInputState::new(window, cx)
                    .min(0u32)
                    .max(10u32)
                    .step(1u32)
            },
            window,
            cx,
        )?;
        let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self {
            form_subscription,
            name_input,
            retry_limit_input,
            form,
        })
    }
}
```

Render the native controls and read validation state from their typed fields.
`validation_text` below is the application's localization helper for
`ValidationMessage`:

```rust,ignore
use gpui::{
    Context, IntoElement, ParentElement as _, Render, Window,
    prelude::FluentBuilder as _,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    form::{field, v_form},
    h_flex,
    input::Input,
    label::Label,
    spinner::Spinner,
    v_flex,
};
use gpui_form::{FormFieldId as _, FormStore as _};
use gpui_form_gpui_component::IntegerInput;

impl Render for ProviderPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name_field = ProviderInputFormStore::name_field(&self.form);
        let name_error = name_field
            .errors(cx)
            .expect("ProviderPage owns the form while rendering")
            .into_iter()
            .next()
            .map(|issue| validation_text(&issue.message, cx));
        let name_is_validating = name_field
            .is_validating(cx)
            .expect("ProviderPage owns the form while rendering");

        v_form()
            .child(
                field()
                    .label("Provider name")
                    .required(ProviderInputField::Name.schema().is_required())
                    .child(
                        v_flex()
                            .child(
                                h_flex()
                                    .child(Input::new(&self.name_input))
                                    .when(name_is_validating, |row| {
                                        row.child(Spinner::new())
                                    }),
                            )
                            .when_some(name_error, |this, error| {
                                this.child(Label::new(error))
                            }),
                    ),
            )
            .child(
                field()
                    .label("Retry limit")
                    .child(IntegerInput::new(&self.retry_limit_input)),
            )
            .child(
                Button::new("save-provider")
                    .primary()
                    .label("Save")
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
    }
}
```

Bound controls are plain Rust handles. They dereference to their native GPUI
component entities and retain only the entity plus synchronization
subscriptions. Focus, IME state, selection, popup state, options, and temporary
editor text remain component or application concerns.

Submit the model already stored in the form. Capture its revision in the same
entity update, keep persistence state on the page or application store, and
conditionally rebase the saved value when the request completes:

```rust,ignore
let prepared = self.form.update(cx, |form, cx| {
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((form.revision(), output))
});

match prepared {
    Ok((revision, output)) => self.start_save(revision, output, cx),
    Err(error) => self.show_submit_error(error, cx),
}

// In the save task's completion callback:
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(submitted_revision, saved_value, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`prepare_submit` performs synchronous submit validation and one pure transform.
It does not start persistence and the form has no submit task, busy flag, retry
policy, or submission-attempt counter. Active asynchronous validation returns
`SubmitError::ValidationPending`.

Every `FieldChanged` and `ModelReplaced` event silently reprojects every mounted
bound control, including the control that initiated a field write. Adapters
never depend on skipping an origin echo or treating component state as
authoritative.

Nested groups and stable-ID arrays remain inside the same top-level model.
Generated field accessors compose without creating child form entities, and
`FormField::project_value` exposes a computed typed value without creating a
parallel business value. Every accessor is a pure lens over a cloned model
candidate; `FormField` owns the one commit, scoped `on_change` validation pass,
event, and notification for a successful write. Nested adapter issues are
matched against their complete stable path, so group and array leaves use
their own schema rather than an ancestor's validation triggers.

## Crates

- `gpui-form`: typed form state, revision/baseline tracking, validation, and
  submit preparation;
- `gpui-form-macros`: `#[derive(FormStore)]` and typed field accessors;
- `gpui-form-gpui-component`: owning bound controls for `gpui-component`.

## Documentation

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
