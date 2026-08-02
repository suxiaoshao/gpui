# gpui-form-gpui-component user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form-gpui-component` adapts native `gpui-component` state entities to
typed `gpui-form` descriptors. It does not create another business-value store
and does not own application configuration.

## Create and render a total control

Every generated static field has one schema-level definition exposed as an
associated `const` descriptor, such as `ProviderForm::NAME`,
`ProviderForm::MODEL_ID`, and `ProviderForm::ENABLED`. These descriptors are
allocation-free and reusable: `FormField<Form, T>` identifies a typed path and
provides static schema and field access, but never owns a particular form entity,
value, subscription, or per-form allocation. Pass the form explicitly when
reading, writing, validating, or binding it:

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

`ProviderForm::NAME` is a total descriptor: the generated model guarantees
that its path exists. `FormInput::new` therefore returns `Self`, not a
`Result`.

`FormInput` is a plain Rust value, not `Entity<FormInput>`. It contains only
ordinary subscriptions and the native entity, and dereferences to that entity:

```rust,ignore
pub struct FormInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}
```

Subscriptions are declared before the entity so Rust drops them first. Other
stateful adapters use the same layout. They do not store a form, field,
`ControlBinding`, delegate, `Config`, focus/blur flag, or validation report.
Binding details live only in subscription closures.

## Partial descriptors

Projection through an optional value or an identified array item is partial: the
descriptor remains cheap and reusable, while the path may be absent in a
specific `Entity<Form>`. `item` and `within` create lightweight located
descriptors, not new schema definitions. Bind the resulting `PartialFormField`
with `try_new`:

```rust,ignore
let header = ServerForm::HEADERS.item(header_id);
let header_name = HeaderRowForm::NAME.within(header);

let header_input = FormInput::try_new(
    &form,
    header_name,
    |window, cx| InputState::new(window, cx).placeholder("Header name"),
    window,
    cx,
)?;
```

`try_new` returns `FieldAccessError` only when the projected or identified
path does not exist. It does not report a released form: the caller supplied a
strong `Entity<Form>`. Use `try_value`, `try_set`, `try_errors`, and similar
`try_*` operations for the same partial boundary. Do not add `Result`
handling to total descriptors merely because another descriptor is partial.

## Synchronization and lifetime

The form owns the authoritative typed value. Native state owns only the current
presentation projection and interaction details such as focus, IME, selection,
query, popup, and highlighted item.

Every stateful adapter follows one synchronization rule:

1. The constructor reads the descriptor through its explicit form, builds the
   native state, silently projects the initial value, and installs both
   subscriptions.
2. A component event defers its typed write until the emitting entity's update
   has ended.
3. The form subscription reprojects on every value or whole-model replacement
   event, including equal-value lifecycle replacement and another path that
   changes this descriptor's projection.
4. Native silent setters do not emit a second user event, so the round trip
   terminates without origin-echo suppression or a value read-back API.

`ControlBinding` is the low-level adapter boundary. It is created from an
explicit strong form and descriptor while mounting a stateful control. It owns
the weak form used by deferred intents; `FormField` itself never contains an
`Entity<Form>` or `WeakEntity<Form>`.

```rust,ignore
let binding = ProviderForm::NAME.bind_control(&form, cx);
let binding = header_name.try_bind_control(&form, cx)?;
```

`ControlBinding` is cloneable. Subscription callbacks capture a clone and call
only `defer_set`, `defer_blur`, `defer_set_issue`, or `defer_clear_issue`.
Deferred callbacks and async work are the only other weak-form boundaries. If
the weak form cannot be upgraded, queued work is cancelled silently.

A normal form-to-control projection closure may capture a static descriptor, or
clone/capture a lightweight located descriptor, together with a weak native
entity. A partial adapter, or a typed editor with a lifecycle-scoped control
issue, may additionally capture the binding to invalidate itself or clear the
issue after a successful programmatic projection. The wrapper still stores
exactly subscriptions first and native state second; it never stores the
descriptor or binding.

## Validation and errors

Adapters forward only events that the concrete component can represent:

| Control | User write | Blur |
| --- | --- | --- |
| `FormInput` | `InputEvent::Change` defers a typed `String` write | `InputEvent::Blur` defers field blur validation |
| `FormIntegerInput<N>` | valid typed integer edit; invalid text creates a control issue | native input blur runs field blur validation |
| `FormSelect<D>` | `SelectEvent::Confirm(Option<Value>)` | unsupported: upstream exposes no reliable final composite blur |
| `FormCombobox<D>` | `ComboboxEvent::Change(Vec<Value>)` | unsupported: upstream exposes no reliable final composite blur |

A non-equal typed field write changes the model and revision, clears only
intersecting validation work, and retains active control issues. An equal write
is a complete no-op. Whole-form lifecycle replacement still reprojects mounted
controls even when the replacement model compares equal.

The bound handle never stores interaction flags or a validation-report copy.
Read data-level status through the explicit form:

```rust,ignore
let field = ProviderForm::NAME;
let is_validating = field.is_validating(&form, cx);
let error = field.errors(&form, cx).into_iter().next();
let required = field.schema().is_required();
```

For a partial descriptor, use the corresponding `try_*` method and handle
`FieldAccessError`. After a failed submit, the active page chooses which visible
control to focus; neither the form nor the adapter owns that choice.

## Select and combobox

`FormSelect<D>` binds exactly `Option<D::Item::Value>`:

```rust,ignore
let model_select = FormSelect::new(
    &form,
    ProviderForm::MODEL_ID,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
);
```

A user confirmation defers its `Option<Value>` directly to the form. Projection
uses the native state's current delegate and is silent. The adapter stores no
delegate and exposes no adapter-specific item updater.

`FormCombobox<D>` binds exactly `Vec<D::Item::Value>`:

```rust,ignore
let tags = FormCombobox::new(
    &form,
    JobForm::TAG_IDS,
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
);
```

`ComboboxEvent::Change(values)` defers `values` to the form. Each form update
uses upstream `set_selected_values` against the current delegate, so a captured
delegate or value/index map cannot become stale.

## Exact integer input

`FormIntegerInput<N>` binds standard signed and unsigned integer primitives to
`IntegerInputState<N>`. The native state owns typed `N`, private editor text,
and typed minimum, maximum, and step policy:

```rust,ignore
let budget = FormIntegerInput::new(
    &form,
    JobForm::BUDGET,
    |window, cx| {
        IntegerInputState::new(window, cx)
            .min(1_024u64)
            .max(1_000_000u64)
            .step(1_024u64)
    },
    window,
    cx,
)?;
```

For a total descriptor, this `Result` contains only
`IntegerInputPolicyError`: `step <= 0` yields `NonPositiveStep` and `min > max`
yields `ReversedRange`. A partial integer descriptor uses `try_new`; its build
error is `FormIntegerInputBuildError::{Field, Policy}`, preserving either
`FieldAccessError` or `IntegerInputPolicyError` rather than erasing the cause.

Incomplete, invalid, overflowed, or out-of-range editor text stays in native
state and publishes a lifecycle-scoped control issue. Only a valid `N` defers
a form write. Increment and decrement use checked arithmetic with typed bounds;
they never use `f64`, clamp overflow, or lose values above `2^53`.

## Stateless boolean elements

Upstream `Checkbox` and `Switch` are `RenderOnce` elements without a public
state entity. They do not get a fake wrapper. Render them as controlled elements
and pass the form explicitly to the descriptor:

```rust,ignore
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

These callbacks are not emitted from a component-state entity update, so they
can write this total field directly. A partial descriptor instead uses
`try_value` and `try_set`. Native blur validation is unavailable because these
elements expose no public focus handle.

## Change options and component configuration

Options, delegates, placeholder, disabled state, size, accessibility, focus,
and catalog refresh belong to the application. Configure native state in the
construction closure or through the dereferenced entity; configure element-only
presentation while rendering.

After replacing items, explicitly reproject the current form value with native
setters or replace the whole bound handle:

```rust,ignore
let selected_model = ProviderForm::MODEL_ID.value(&form, cx);
model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags = JobForm::TAG_IDS.value(&form, cx);
tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});
```

Run the native item update and value re-projection immediately as one refresh
operation. Changing options does not itself write the form, so no value event
is guaranteed. The adapter never chooses a fallback, changes form data,
persists configuration, or starts dynamic validation as an item-refresh effect.

## Implement another stateful adapter

There is no `FormControl` trait. A third-party adapter exposes inherent `new`
and `try_new` constructors appropriate to its native state. Callers pass the
static associated descriptor or a lightweight located descriptor together with
the explicit `&Entity<Form>`:

```rust,ignore
pub struct FormDateInput {
    subscriptions: Vec<Subscription>,
    state: Entity<DateInputState>,
}

impl FormDateInput {
    pub fn new<Form>(
        form: &Entity<Form>,
        field: FormField<Form, Date>,
        cx: &mut App,
    ) -> Self
    where
        Form: FormState,
    {
        let binding = field.bind_control(form, cx);
        // Build state and capture `binding` in subscriptions.
    }

    pub fn try_new<Form>(
        form: &Entity<Form>,
        field: PartialFormField<Form, Date>,
        cx: &mut App,
    ) -> Result<Self, FieldAccessError>
    where
        Form: FormState,
    {
        let binding = field.try_bind_control(form, cx)?;
        // Build state and capture `binding` in subscriptions.
    }
}
```

The example omits component-specific construction parameters. The real handle
still contains only `Vec<Subscription>` and `Entity<State>`. Capture the
binding inside subscriptions, use its deferred intents for component-to-form
writes, and silently reproject form changes. Do not add adapter `Config`,
descriptor/binding fields, focus mirrors, delegate copies, origin-echo skipping,
authoritative read-back APIs, or public source/control IDs.

## Related documentation

- [gpui-form user guide](../../gpui-form/docs/guide.md)
- [gpui-form-macros user guide](../../gpui-form-macros/docs/guide.md)
