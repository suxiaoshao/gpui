# gpui-form-gpui-component user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

> **Implementation status:** this guide documents the implemented public API.

`gpui-form-gpui-component` adapts native `gpui-component` state entities to
typed `gpui-form` fields. It does not create another business-value store and
does not own application configuration.

## Create and render a control

Pass a generated field and a closure that constructs the native state:

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

`FormInput` is a plain Rust value, not `Entity<FormInput>`. It contains only
ordinary subscriptions and the native entity, and dereferences to that entity:

```rust,ignore
use std::ops::Deref;
use gpui::{Entity, Subscription};

pub struct FormInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}

impl Deref for FormInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}
```

Subscriptions are declared before the entity so Rust drops them first. The
other stateful adapters use the same layout. They do not store a field,
`ControlAttachment`, delegate, `Config`, focus flag, blur flag, or validation
report. Binding details live only in subscription closures.

Calling a generated field accessor repeatedly is safe. A `FormField` is a
cheap typed handle to one form path, not another value or another subscription.

## Synchronization and lifetime

The form owns the authoritative typed value. Native state owns only the current
presentation projection and interaction details such as focus, IME, selection,
query, popup, and highlighted item.

Every stateful adapter follows one synchronization rule:

1. the constructor reads the field, builds the native state, silently projects
   the initial value, and installs both subscriptions;
2. a component event defers its typed write until the emitting entity's update
   has ended;
3. the form subscription reprojects on every `FieldChanged` and
   `ModelReplaced`, including an equal-value whole-form lifecycle and an event
   for another path that changes this field's projection; it ignores only
   `RuntimeChanged`;
4. native silent setters do not emit a second user event, so the round trip
   terminates without an origin-echo skip or a value read-back API.

The only public attachment creation entry point on `FormField` is:

```rust,ignore
pub fn attach_control(
    &self,
    cx: &mut App,
) -> Result<ControlAttachment<Form, T>, FormFieldError>;
```

`ControlAttachment` is `Clone`; every clone shares one private lease and
liveness state. Component-event subscription callbacks capture a clone and
call its narrow `defer_set_user_value`, `defer_blur`, `defer_set_issue`, or
`defer_clear_issue` intent. These are its only public mutation methods. Weak
lifetime state, source IDs, and control IDs remain private to core.

A normal form-to-control projection closure captures only the typed field and
a weak native entity. A typed editor that owns a lifecycle-scoped control draft
issue may also capture a clone of the same attachment, only to call
`defer_clear_issue` after a programmatic silent projection succeeds. Among the
built-in adapters, only the exact-integer control uses this exception. The bound
wrapper still stores exactly its subscriptions first and native state entity
second; it never stores the field or attachment as another field. Dropping the
wrapper drops all subscription-held clones. After the last clone drops, queued
intents are inert and the control-scoped issue is inactive.

A missing projected or identified path returns `FormFieldError::ValueUnavailable`.
The callback does not invent a fallback or preserve the component value as a
second source of truth; it notifies the owner so the structural view can drop
or rebuild the stale control.

## Validation and errors

Adapters forward only events that the concrete component can represent:

| Control | User write | Blur |
| --- | --- | --- |
| `FormInput` | `InputEvent::Change` with a deferred `String` write | `InputEvent::Blur` runs field blur validation |
| `FormIntegerInput<N>` | valid typed integer edit; invalid text creates a control issue | native input blur runs field blur validation |
| `FormSelect<D>` | `SelectEvent::Confirm(Option<Value>)` | unsupported: upstream exposes no reliable final composite blur |
| `FormCombobox<D>` | `ComboboxEvent::Change(Vec<Value>)` | unsupported: upstream exposes no reliable final composite blur |

`ComboboxEvent::Confirm` is intentionally ignored: `Change` already commits
each toggle and listening to both would write the same selection twice.

A non-equal typed field write changes the model and revision, then clears only
intersecting required, structural, and generated synchronous field buckets and
intersecting asynchronous validation. Adapter-wide issues and active control
issues are retained. Change validation then runs and emits `FieldChanged`.
An equal field write is a complete no-op. Whole-form lifecycle operations use
`ModelReplaced`; mounted controls still reproject even when the replacement
model compares equal.

The bound handle never stores `focused`, `blurred`, `touched`, `show_error`, or
a validation-report copy. Read data-level status from the generated field:

```rust,ignore
use gpui_form::FormFieldId as _;

let field = ProviderInputFormStore::name_field(&form);
let is_validating = field.is_validating(cx)?;
let error = field.errors(cx)?.into_iter().next();
let required = ProviderInputField::Name.schema().is_required();
```

If the same field is rendered more than once, every instance sees the same
data-level issue. After a failed submit, the active page chooses which visible
control to focus; neither the form nor the adapter owns that choice.

## Select

`FormSelect<D>` binds exactly `Option<D::Item::Value>`. Build the state with
application-owned items and configuration:

```rust,ignore
use gpui_component::select::{Select, SelectState};
use gpui_form::FormControl as _;
use gpui_form_gpui_component::FormSelect;

let model_select = FormSelect::new(
    ProviderInputFormStore::model_id_field(&form),
    move |window, cx| {
        SelectState::new(ModelDelegate::new(models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Select::new(&model_select);
```

A user confirmation defers the event's `Option<Value>` directly to the form.
Form projection calls `set_selected_value` for `Some` and
`set_selected_index(None)` for `None`. Both resolve against the native state's
current delegate and are silent.

The adapter stores no delegate and exposes no adapter-specific item updater.
An unavailable `Some(value)` clears only the native selection; the typed form
value remains unchanged for application-owned dynamic validation.

## Combobox

`FormCombobox<D>` binds exactly `Vec<D::Item::Value>`:

```rust,ignore
use gpui_component::combobox::{Combobox, ComboboxState};
use gpui_form::FormControl as _;
use gpui_form_gpui_component::FormCombobox;

let tags = FormCombobox::new(
    JobInputFormStore::tag_ids_field(&form),
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Combobox::new(&tags);
```

`ComboboxEvent::Change(values)` defers `values` to the form. Every form change
calls upstream `ComboboxState::set_selected_values`, which resolves values with
the current delegate, ignores values that cannot be resolved, preserves input
order, updates the committed selection and snapshot, and emits no
`ComboboxEvent`. No captured delegate or value/index map can become stale.

## Exact integer input

`FormIntegerInput<N>` binds standard signed and unsigned integer primitives to
`IntegerInputState<N>`. The native state owns typed `N`, its private text
editor, and typed minimum, maximum, and step policy:

```rust,ignore
use gpui_form::FormControl as _;
use gpui_form_gpui_component::{
    FormIntegerInput, IntegerInput, IntegerInputState,
};

let budget = FormIntegerInput::new(
    JobInputFormStore::budget_field(&form),
    |window, cx| {
        IntegerInputState::new(window, cx)
            .min(1_024u64)
            .max(1_000_000u64)
            .step(1_024u64)
    },
    window,
    cx,
)?;

let element = IntegerInput::new(&budget);
```

The wrapper validates construction policy before installing subscriptions:

- `step <= 0` returns `IntegerInputPolicyError::NonPositiveStep`;
- `min > max` returns `IntegerInputPolicyError::ReversedRange`.

Editor changes are classified as `Incomplete`, `InvalidSyntax`, `Overflow`, or
`OutOfRange { min, max }`. Those states keep the raw text, publish a
lifecycle-scoped validation issue, and do not write the form. A valid edit
clears the issue and defers typed `N`; the resulting form event silently
reprojects the canonical text to every instance. A programmatic form write is
authoritative: after the field read, weak-entity upgrade, and silent projection
all succeed, it replaces stale raw text and clears the old editor issue. A
failed projection does not clear that issue. Business validation remains
responsible for rejecting an application-written value that violates the
model's domain rules.

Increment and decrement use `checked_add` and `checked_sub` with typed bounds.
They never use `f64`, clamp overflow, or lose values above `2^53`. Blur runs
field blur validation; invalid text remains visible for correction.

The adapter emits stable message keys and string parameters, while the
application owns translations:

- `gpui-form-error-integer-incomplete`;
- `gpui-form-error-integer-invalid`;
- `gpui-form-error-integer-overflow`;
- `gpui-form-error-integer-min` with `min`;
- `gpui-form-error-integer-max` with `max`;
- `gpui-form-error-integer-range` with `min` and `max`.

## Stateless boolean elements

Upstream `Checkbox` and `Switch` are `RenderOnce` elements without a public
state entity. They do not get a fake `FormBool` wrapper. Render them as
controlled elements and write the user value to `FormField<bool>`:

```rust,ignore
use gpui_component::{checkbox::Checkbox, switch::Switch};

let enabled_field = ProviderInputFormStore::enabled_field(&self.form);
let enabled = enabled_field
    .value(cx)
    .expect("ProviderPage owns the form while rendering");

let checkbox_field = enabled_field.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .label("Enabled with checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field
            .set_user_value(*checked, cx)
            .expect("ProviderPage owns the form while this element is mounted");
    });

let switch = Switch::new("provider-enabled-switch")
    .label("Enabled with switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        enabled_field
            .set_user_value(*checked, cx)
            .expect("ProviderPage owns the form while this element is mounted");
    });
```

These element callbacks are not emitted from a component-state entity update,
so they can write the field directly. The page's form observation rerenders
both controlled values. Change and submit validation work normally; native
blur validation is unavailable because these elements expose no public focus
handle.

The `expect` calls are appropriate only because rendering structurally owns the
form and path. Use `?` or explicit error handling for projected or dynamic paths
that can legitimately disappear.

## Change options and component configuration

Options, delegates, placeholder, disabled state, size, accessibility, focus,
and catalog refresh belong to the application. Configure native state in the
construction closure or through the dereferenced entity; configure
element-only presentation while rendering.

After replacing items, explicitly reproject the current form value with native
setters or replace the whole bound handle:

```rust,ignore
use gpui_form::{FormFieldId as _, ValidationScope, ValidationTrigger};

let selected_model =
    ProviderInputFormStore::model_id_field(&form).value(cx)?;
model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags =
    ProviderInputFormStore::tag_ids_field(&form).value(cx)?;
tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});

form.update(cx, |form, cx| {
    form.validate(
        ValidationTrigger::Dynamic,
        ValidationScope::Field(ProviderInputField::ModelId.path()),
        cx,
    );
});
```

Run the native item update and value re-projection immediately as one refresh
operation. Do not wait for a later form value event: changing options does not
itself write the form, so no `FieldChanged` is guaranteed.

The adapter never chooses a fallback, changes form data, persists configuration,
or starts dynamic validation as a side effect of an item refresh. Direct native
setters are presentation operations; business writes use `FormField::set`,
`replace`, `reset`, or `rebase`.

## Implement another stateful adapter

Core `FormControl<T>` standardizes one-call construction without standardizing
component configuration:

```rust,ignore
use std::ops::Deref;
use gpui::{Context, Entity, Window};
use gpui_form::{FormField, FormStore};

pub trait FormControl<T>: Deref<Target = Entity<Self::State>> + Sized
where
    T: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Error;

    fn new<Form, Owner, Build>(
        field: FormField<Form, T>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State;
}
```

An implementation returns a plain handle containing only
`Vec<Subscription>` and `Entity<State>`. It captures field and attachment data
inside subscriptions, uses the attachment's deferred intent methods for
component-to-form writes, and silently reprojects every `FieldChanged` and
`ModelReplaced` while ignoring `RuntimeChanged`. Normal projection closures
capture only the field and weak native entity. A typed editor with a
lifecycle-scoped control draft issue may additionally capture the same
attachment clone, solely to clear that issue after successful programmatic
projection; among the built-in adapters, only the exact-integer control uses
this exception. Temporary editor data stays in native state, and weak lifetime
handling stays inside core. Do not add adapter `Config`, field or attachment
fields, focus mirrors, delegate copies, origin-echo skipping,
authoritative-value read-back APIs, or public source/control IDs.

## Related documentation

- [gpui-form user guide](../../gpui-form/docs/guide.md)
- [gpui-form-macros user guide](../../gpui-form-macros/docs/guide.md)
- [implementation plan](../dev/typed-bound-controls.md)
