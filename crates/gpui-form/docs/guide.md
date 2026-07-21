# gpui-form user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

> **Implementation status:** this guide documents the implemented public API.

`gpui-form` provides typed form data, validation, revision tracking, and submit
preparation for GPUI applications. This document describes the public contract
from a library user's perspective.

## 1. Crates and features

Applications normally depend on the form crate and the component adapter. The
derive macro is re-exported by `gpui-form`:

```toml
[dependencies]
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
garde.workspace = true
```

Optional integrations are enabled on `gpui-form`:

```toml
gpui-form = { workspace = true, features = ["garde-adapter", "validify-transform"] }
```

- `garde-adapter` validates a model that implements `garde::Validate`;
- `validify-transform` clones the submitted model and applies
  `validify::Modify` to the clone;
- `form-pipeline` enables both integrations.

## 2. Declare one typed model

The form model uses the exact Rust types that the application wants to submit:

```rust,ignore
use gpui_form::FormStore;

#[derive(Clone, Debug, PartialEq, FormStore)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,

    #[form(validate(on_submit))]
    retry_limit: u32,

    #[form(validate(on_dynamic, on_submit))]
    model_id: Option<String>,
}
```

`#[derive(FormStore)]` generates:

- `ProviderInputFormStore`, the GPUI entity state that owns the current model;
- `ProviderInputField`, the static field identity and schema enum;
- typed field access such as `ProviderInputFormStore::name_field(&form)`;
- validation traversal, nested accessors, revision handling, and submit glue.

The generated store has exactly one internal `FormRuntime`, which owns the
current value, baseline, revision, validation context, and validation state.
Validation adapters and submit transforms are associated types only: the form
constructs their stateless `Default` value when an operation needs one and never
stores an adapter or transform instance. Put runtime dependencies in the typed
validation context or in application-owned state.

There is no form-owned String draft or codec for integers, enums, or other
typed values. A concrete component may privately keep incomplete editing text,
but that text never replaces the form's typed business value.

## 3. Create the form

Create one form entity for one editing session:

```rust,ignore
use gpui::AppContext as _;

let form = cx.new(|cx| {
    ProviderInputFormStore::from_value(
        ProviderInput {
            name: String::new(),
            retry_limit: 3,
            model_id: None,
        },
        cx,
    )
});
```

`FormStore::from_value` is available when
`Self::ValidationContext: Default`. It installs the model and context, then
runs mount validation exactly once before returning the store.

Use a typed context when validation needs application-owned dependencies:

```rust,ignore
let form = cx.new(|cx| {
    ProviderInputFormStore::from_value_with_validation_context(
        initial,
        ProviderValidationContext { catalog: catalog.clone() },
        cx,
    )
});
```

`from_value_with_validation_context` is always available. It installs both the
initial model and supplied context before the single mount-validation pass.

`set_validation_context(next, cx)` only replaces the context and notifies form
observers. It does not select a validation trigger. The caller explicitly runs
dynamic or submit validation when the new context should affect the report.

## 4. Create bound controls

`gpui-form-gpui-component` creates a native component state and binds it to a
typed field in one call:

```rust,ignore
use gpui_component::input::InputState;
use gpui_form::FormControl as _;
use gpui_form_gpui_component::{
    FormInput, FormIntegerInput, IntegerInputState,
};

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
```

The returned control is a plain Rust handle, not another GPUI entity layer. It
dereferences to its native state entity and retains only that entity plus its
binding subscriptions. Component configuration belongs in the construction
closure or native component API; element-only presentation belongs on the
render-time builder.

`FormField::subscribe_in` reacts to every `FormEvent::FieldChanged` and
`FormEvent::ModelReplaced`, regardless of source or event path, and ignores
`FormEvent::RuntimeChanged`. Each callback rereads its own typed field and
silently reprojects it to the mounted control, including the control whose user
event initiated the write. Silent component setters must not emit another user
event. Bindings therefore have one simple rule and do not expose
origin-skipping or authoritative-readback protocols.

User component events defer their typed field write until the emitter's active
update has ended. This prevents GPUI entity reentrancy. Application code does
not need to perform that deferral itself.

Observe the form once at the page or controller lifetime so labels, validation
feedback, and buttons rerender when form runtime state changes:

```rust,ignore
let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());
```

## 5. Render fields and form state

The generated schema supplies static metadata. The form supplies data-level
runtime state. Native controls supply their own interaction state:

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
            .expect("the page owns the form while rendering")
            .into_iter()
            .next()
            .map(|issue| validation_text(&issue.message, cx));
        let name_is_validating = name_field
            .is_validating(cx)
            .expect("the page owns the form while rendering");

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

Useful queries include:

- `is_dirty()` and `is_valid()`;
- `is_validating()` and `is_validating_at(path)`;
- `validation_report()`, `errors_at(path)`, and `first_error_path()`;
- `revision()`.

The form owns no `FocusHandle`, focused/touched/blurred flag, or error-visibility
flag. Validation triggers decide when issues enter the report. After a failed
submit, the active page uses `first_error_path()` to choose which visible
control instance to focus.

## 6. Change data and protect asynchronous saves

### Typed field writes

Application code changes one field through its typed handle:

```rust,ignore
ProviderInputFormStore::retry_limit_field(&self.form).set(5, cx)?;
```

Stateful bound controls use their attachment internally. Stateless controlled
elements such as a checkbox use the explicit user-originated method:

```rust,ignore
ProviderInputFormStore::enabled_field(&self.form)
    .set_user_value(true, cx)?;
```

Both methods store the typed value before running change validation. An equal
field write is a no-op: it does not advance the revision, rerun validation, or
emit another projection event.

### Whole-form lifecycle

Use whole-form operations when installing application data:

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next_value, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved_value, cx));
```

- `replace` installs a new current model and keeps the existing baseline;
- `reset` restores the baseline as the current model;
- `rebase` installs one model as both current value and baseline;
- `rebase_if_revision` performs the same operation as `rebase` only when the
  current revision equals the expected revision.

These are explicit lifecycle operations. Every call to `replace`, `reset`, or
`rebase`, and every successful `rebase_if_revision`, advances the revision even
when the installed Rust value compares equal. Each operation cancels active
asynchronous validation, clears data-level validation issues, and silently
reprojects all mounted controls. It preserves component-owned interaction state
only to the degree supported by the component's silent setter, and it does not
synthesize per-field change validation.

### Revisions and conditional rebase

`FormRevision` is a monotonically increasing token for business-value state.
Field writes and whole-form lifecycle operations advance it. Validation runs,
pending state, control issues, and validation-context changes do not.

Capture the revision and prepared output in the same entity update:

```rust,ignore
let (submitted_revision, output) = self.form.update(cx, |form, cx| {
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((form.revision(), output))
})?;
```

The page or application store owns the persistence task and loading state. On
success, install the repository's canonical saved value only if the user has
not edited the form meanwhile:

```rust,ignore
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(submitted_revision, saved_value, cx)
});

if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

A rejected conditional rebase returns `false` with no side effects: it changes
neither value, baseline, revision, validation state, async work, nor controls.
A successful conditional rebase advances the revision, so two responses for
the same submitted revision cannot both apply. Use unconditional `rebase` after
a request only when the application prevented every business-value write for
the request's entire lifetime.

Persistence code consumes `prepare_submit` output. It does not assemble a
model by reading fields or component states.

## 7. Validation

### Triggers and scopes

Supported triggers are:

| Attribute | Runtime trigger |
| --- | --- |
| `on_mount` | once after construction has installed the initial model and context |
| `on_change` | after a typed field write has been committed |
| `on_blur` | when a concrete bound control reports final blur |
| `on_dynamic` | when the application explicitly refreshes external dependencies |
| `on_submit` | during `prepare_submit` |

`ValidationScope::Field(path)` includes the changed path, its descendants, and
its ancestor group/array paths, but not sibling leaves. Group and identified
array-item scopes include their subtree plus ancestors. `ValidationScope::Form`
includes every data path.

A validation run replaces synchronous field issues only for paths selected by
both its trigger and scope. Adapter-produced form-level issues use one
adapter-wide bucket that is replaced on every adapter run, even for a field
scope. Issues for non-participating fields remain intact.

A successful typed field write first advances the model and revision. It then
clears only the intersecting required, structural, and generated synchronous
field buckets; cancels and clears intersecting async validation; preserves the
adapter-wide form bucket and every active control issue; runs change validation;
and finally emits one `FormEvent::FieldChanged`. Equal writes are complete
no-ops. Preservation here describes the invalidation phase: if the adapter
participates in the subsequent change-validation run, that run replaces its
adapter-wide bucket normally. Change validation never owns or clears a control
issue.

### Required values

`required` is both static schema metadata and a built-in validation rule:

```rust,ignore
#[form(required, validate(on_change, on_blur))]
name: String,
```

The required rule always participates in submit validation. The listed
triggers add earlier feedback. `RequiredValue::is_missing` defines the exact
semantics:

- `String` is missing when `trim()` is empty;
- `Option<T>` is missing for `None`;
- `Vec<T>`, `HashMap`, `BTreeMap`, `HashSet`, and `BTreeSet` are missing when
  empty;
- `bool` is missing when `false`, which supports required consent controls;
- a custom type opts in by implementing `RequiredValue`.

Numeric and enum types do not have a universal missing value and therefore do
not receive a built-in implementation. Applying `required` to an unsupported
type is a compile error. Use an explicit validation rule for domain-specific
numeric or enum constraints.

The built-in issue uses the stable key `gpui-form-error-required`. The
application localizes that key while rendering beside its localized field
label.

### Garde

Use Garde for synchronous model and business rules:

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(validation(adapter = "garde"))]
struct AccountInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    display_name: String,

    #[form(validate(on_change, on_blur, on_dynamic, on_submit))]
    #[garde(email)]
    email: Option<String>,
}
```

`#[form(required)]` owns empty-value semantics. Do not duplicate the same
constraint with Garde. Add `#[garde(dive)]` explicitly to groups or arrays that
Garde should recurse into.

For a model with a Garde context, declare the context on Garde and pass it to
the generated form constructor:

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[garde(context(AccountValidationContext))]
#[form(validation(
    adapter = "garde",
    i18n = AppGardeI18nProvider
))]
struct AccountInput {
    #[form(validate(on_dynamic, on_submit))]
    #[garde(custom(validate_account_plan))]
    plan_id: Option<String>,
}
```

The adapter calls `garde::Validate::validate_with` with the exact typed context.
It never falls back to `validate()` for a non-default context.

Garde 0.23.0 localizes built-in errors through
`garde::i18n::with_i18n`. A provider returns a handler implementing the exact
upstream trait:

```rust,ignore
use std::borrow::Cow;
use garde::i18n::{I18n, InvalidEmail};

struct AppGardeI18n<'a> {
    i18n: &'a AppI18nSnapshot,
}

impl I18n for AppGardeI18n<'_> {
    fn length_lower_than(&self, min: usize) -> Cow<'static, str> {
        self.i18n
            .translate("validation-length-lower-than", [("min", min.to_string())])
            .into()
    }

    fn email_invalid(&self, reason: InvalidEmail) -> Cow<'static, str> {
        self.i18n
            .translate(
                "validation-email-invalid",
                [("reason", reason.to_string())],
            )
            .into()
    }

    // Implement every other method required by garde::i18n::I18n 0.23.0.
}
```

The 0.23.0 signatures take only the rule parameters shown by the upstream
trait—for example, `length_lower_than(min)`—and return
`Cow<'static, str>`. They do not receive an `actual` length parameter.

`GardeI18nProvider<C>` creates the handler from the form's validation context.
The handler is installed only for the current thread and synchronous stack
frame; it must never cross an `await`. Garde stores a final string in each
error, so the adapter preserves it as `ValidationMessage::Localized`.
Omitting `i18n` selects `DefaultGardeI18nProvider`.

Garde exposes vector positions in its displayed paths. Generated
`GardePathMapper` implementations map each current index to the validated
model's stable `FormItemId` before scope filtering. Unknown fields, malformed
or out-of-bounds indices, unconvertible IDs, and duplicate IDs become blocking
internal form issues. The adapter does not use Garde's doc-hidden path iterator
and never leaves a mutable vector index in a final `FieldPath`.

When the locale changes, the application updates the validation context and
explicitly requests dynamic validation for messages that must be regenerated:

```rust,ignore
self.form.update(cx, |form, cx| {
    form.set_validation_context(next_context, cx);
    form.validate(
        ValidationTrigger::Dynamic,
        ValidationScope::Form,
        cx,
    );
});
```

### Custom synchronous adapters

Any synchronous validation library integrates through
`ValidationAdapter<Model>`:

```rust,ignore
use gpui::App;
use gpui_form::{
    FormFieldId as _, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationIssue, ValidationMessage, ValidationScope,
    ValidationSource, ValidationTrigger,
};

#[derive(Default)]
struct ProviderValidator;

impl ValidationAdapter<ProviderInput> for ProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        &self,
        value: &ProviderInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let mut report = ValidationAdapterReport::default();
        let path = ProviderInputField::ModelId.path();

        if scope.includes(Some(&path))
            && value.model_id.as_ref().is_some_and(|id| {
                !context.external.model_ids.contains(id)
            })
        {
            report.push(ValidationIssue::field(
                path,
                trigger,
                ValidationSource::App("provider".into()),
                "model_unavailable",
                ValidationMessage::key("provider-model-unavailable"),
            ));
        }

        report
    }
}
```

The adapter receives the trigger directly; `ValidationContext` contains only
the typed external context and has no redundant `submitted` flag. A custom
adapter implements `Default + 'static`. Each validation run constructs
`Self::ValidationAdapter::default()`; the form never stores an adapter instance,
so runtime dependencies belong in its context.

Map external library paths to generated `FieldPath`s. An unknown path is a
blocking internal form-level issue rather than an ignored string. Rendering,
focus, persistence, and library-specific global state remain outside the
adapter.

### Asynchronous validation

The page owns the subscription that decides when to start a remote check. The
form owns each check after it starts:

```rust,ignore
use gpui_form::{
    AsyncValidationIssue, ValidationMessage, ValidationTrigger,
};

let field = ProviderInputFormStore::name_field(&self.form);
let validation_field = field.clone();
let service = self.provider_service.clone();

let validation_subscription = field.subscribe_in(
    window,
    cx,
    move |_owner, window, cx| {
        let field = validation_field.clone();
        let service = service.clone();

        // This callback is emitted by the same form. Start the next form
        // update only after the current update scope has ended.
        cx.defer_in(window, move |_owner, _window, cx| {
            field.start_async_validation(
                "provider-name",
                ValidationTrigger::Change,
                move |name| async move {
                    if service.name_available(&name).await {
                        Ok(())
                    } else {
                        Err(AsyncValidationIssue::new(
                            "name_taken",
                            ValidationMessage::key("provider-name-taken"),
                        ))
                    }
                },
                cx,
            )?;
            anyhow::Ok(())
        });
    },
)?;
```

`start_async_validation` snapshots the current typed field value and retains a
`Task<()>` plus a monotonically increasing generation under `(field path,
source)`. Starting the same key again, or writing an intersecting value,
cancels the prior task, clears its prior issue, and installs a new generation.
A stale completion has no effect.

`cancel_async_validation(source, cx)`, a whole-form lifecycle operation, or
dropping the form cancels the task and clears pending state. Dropping the page
subscription prevents future checks but does not abandon an already-started
check, because that task is retained by the form.

Every active async validation registered through this API blocks submit.
`prepare_submit` returns `SubmitError::ValidationPending` until all such checks
finish. A non-blocking remote hint is ordinary application UI state and must not
use the form async-validation API.

### Control issues and error messages

A typed control that temporarily cannot produce its field type—for example, an
integer editor containing only `-`—keeps that text privately and publishes a
control issue. The issue blocks submit only while the bound control's lifetime
is active. Dropping the last clone of that control's shared attachment lease
(normally by dropping its binding subscriptions), or an internal invalidation
after its dynamic path disappears, makes the issue inactive.

`ValidationMessage::Key { key, params }` is translated by the application at
render time. `ValidationMessage::Localized` is already final, as with Garde,
and must not be translated again. The form does not own a locale global or an
error renderer.

## 8. Submit and transformation

`prepare_submit` is a synchronous validation-and-transformation boundary:

```rust,ignore
let prepared = self.form.update(cx, |form, cx| {
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((form.revision(), output))
});
```

It clones one current model snapshot and follows this fixed order:

1. run synchronous submit validation against that snapshot;
2. return `SubmitError::Validation(report)` for data or active control issues;
3. return `SubmitError::ValidationPending` when form-owned async validation is
   active;
4. run the submit transform once against the same snapshot, returning its
   output or `SubmitError::Transform(report)`.

The operation never starts persistence. The page or application store owns the
save task, loading state, cancellation, retry policy, provider/database errors,
and user notifications. The form exposes no submit task, busy flag,
submission-attempt counter, or `SubmitError::Busy`.

Implement `SubmitTransform<Model>` for a custom output:

```rust,ignore
use gpui_form::{SubmitTransform, TransformReport};

#[derive(Default)]
struct ProviderTransform;

struct SaveProvider {
    name: String,
    retry_limit: u32,
}

impl SubmitTransform<ProviderInput> for ProviderTransform {
    type Output = SaveProvider;

    fn transform(
        &self,
        model: &ProviderInput,
    ) -> Result<Self::Output, TransformReport> {
        Ok(SaveProvider {
            name: model.name.trim().to_owned(),
            retry_limit: model.retry_limit,
        })
    }
}
```

`SubmitTransform<Model>` requires `Default + 'static` and has one associated
`Output` and one `transform` method. There is no preview path or transform
context. `prepare_submit` constructs `Self::SubmitTransform::default()` only
after validation and pending checks pass, then calls `transform` exactly once.
Identity and Validify transforms use `Output = Model`. Transformation is pure:
it does not mutate the form value, baseline, revision, validation report, or
controls.

Transform failures are submit results, not validation state. Put a rule in
`ValidationAdapter` instead when it should affect inline errors or
`is_valid()`.

## 9. Implement a custom stateful control

`FormControl<T>` standardizes one-step construction and binding without owning
component configuration:

```rust,ignore
use std::ops::Deref;
use gpui::{App, Context, Entity, Subscription, Window};
use gpui_form::{
    ControlAttachment, FormField, FormFieldError, FormStore,
};

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

impl<Form, T> FormField<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub fn attach_control(
        &self,
        cx: &mut App,
    ) -> Result<ControlAttachment<Form, T>, FormFieldError>;
}
```

A stateful handle contains only subscriptions and its native entity, with
subscriptions declared first so they drop first:

```rust,ignore
pub struct FormRating {
    subscriptions: Vec<Subscription>,
    rating: Entity<RatingState>,
}

impl Deref for FormRating {
    type Target = Entity<RatingState>;

    fn deref(&self) -> &Self::Target {
        &self.rating
    }
}
```

The binding first creates one attachment. Construction immediately checks that
the form is alive and the field/path is readable, returning `FormReleased` or
`ValueUnavailable` otherwise:

```rust,ignore
let attachment = field.attach_control(cx)?;
let component_attachment = attachment.clone();
let projection_attachment = attachment;
```

`ControlAttachment` is `Clone`. All clones share one private control identity
and lease; cloning never registers a second control. Move the clones into the
component-event and form-projection subscription closures rather than storing
an attachment field on the wrapper. Those callbacks express component intents
through the attachment's deferred methods:

```rust,ignore
attachment.defer_set_user_value(next, window, cx);
attachment.defer_blur(window, cx);
attachment.defer_set_issue("invalid_rating", message, window, cx);
attachment.defer_clear_issue(window, cx);
```

These four methods are the attachment's only public mutation API. They return
`()`, and schedule the form update after the current owner update has ended.
`attach_control` itself only constructs and validates the lease; it does not
change the business value, revision, or validation report. Weak lifetime state
and the internal control identity are crate-private implementation details.
Dropping one clone leaves the control active while another clone exists. When
the last clone drops, the private lease can no longer be upgraded: queued
intents become no-ops and the control issue is immediately treated as inactive.
The runtime's weak lease does not keep it alive. Callers never upgrade a weak
attachment or interpret a control ID.

A custom implementation follows these rules:

1. read the current typed field, build the native state, and silently project
   the initial value;
2. send every component-originated field write through
   `defer_set_user_value`, which defers it until the emitter update ends;
3. use `FormField::subscribe_in`, which handles every `FieldChanged` and
   `ModelReplaced`, ignores only `RuntimeChanged`, and silently projects every
   resulting value to this component, including its own write;
4. never skip an origin echo and never treat native component state as a second
   authoritative value;
5. invoke `defer_blur` only when the component exposes a reliable final-blur
   signal; do not store focus or blur state in the form;
6. keep incomplete editor state in the native state and publish its
   lifecycle-scoped issue through `defer_set_issue`; an integer or custom
   projection may capture the same attachment clone and call
   `defer_clear_issue` after a successful silent authoritative projection;
7. if a projected or identified path disappears, let the deferred attachment
   operation handle `FormFieldError::ValueUnavailable`: it invalidates its
   private lifetime and notifies the owner so the owner can drop or rebuild the
   control; a form-to-control projection that detects the same error also
   notifies the owner instead of inventing a fallback value;
8. keep options, disabled state, placeholders, accessibility configuration,
   and presentation outside the form field.

The attachment's public mutation surface is limited to the four deferred intent
methods above. It exposes no immediate write, authoritative read-back, weak
handle, control ID, or origin token. A successful deferred write is followed by
the normal silent form projection. The wrapper still contains only
`Vec<Subscription>` followed by the native `Entity<State>`; captured attachment
clones live inside those subscriptions.

## 10. Stateless controls and component configuration

Stateless `Checkbox` and `Switch` elements do not need a fake state wrapper.
Render them as controlled elements from `FormField<bool>` and call
`set_user_value` from their click callback. The page's form observation
rerenders every consumer of that field.

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
so direct field writes are safe. Use `expect` only when the page structurally
owns the form and path for the element's entire mounted lifetime. Handle
`FormFieldError` explicitly for projected or dynamic paths that can legitimately
disappear.

Options and catalogs are configuration, not form data:

1. update the application catalog store;
2. update or rebuild the native component state;
3. silently reproject the authoritative form value through the current native
   delegate;
4. explicitly run dynamic validation if the value is unavailable.

An options refresh never chooses the first item, changes the form value,
rebases, persists, or reads the database implicitly. See the component adapter
guide for the exact Select and Combobox APIs.

## 11. Nested models, arrays, and projections

Nested data stays in one root model:

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormStore)]
struct AuthInput {
    #[form(required, validate(on_change, on_blur))]
    username: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
struct HeaderRowInput {
    row_id: u64,
    #[form(required, validate(on_change, on_blur))]
    name: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
struct ServerInput {
    #[form(group)]
    auth: AuthInput,

    #[form(array(id = "row_id"))]
    headers: Vec<HeaderRowInput>,
}
```

Generated accessors compose typed lenses over the root store:

```rust,ignore
let username = AuthInputFormStore::username_in(
    ServerInputFormStore::auth_field(&form),
);

let header_name = HeaderRowInputFormStore::name_in(
    ServerInputFormStore::headers_item(
        &form,
        FormItemId::new(row_id),
    ),
);
```

Repeated accessor calls create cheap handles, not values, subscriptions, or
child entities. Multiple controls may consume one field safely.

Stable IDs are unique within the array, immutable for a logical item, and not
reused for another item during one form session. A missing, duplicate, or
unconvertible ID makes the affected handle return
`FormFieldError::ValueUnavailable` and creates a blocking structural issue for
submit. The library never picks the first duplicate. Reordering preserves
addressing but invalidates old descendant validation results; fresh validation
maps issues to the current stable IDs.

Use `project_value` only for a computed or conditionally available typed value:

```rust,ignore
let budget = JobInputFormStore::run_settings_field(&form).project_value(
    "token_budget",
    |settings| settings.custom_token_budget(),
    |settings, value| settings.set_custom_token_budget(value),
);
```

The technical name creates a distinct `FieldPathSegment::Projection`; it can
never collide with a real model field. The projected path identifies control
and async issues, while `validation_path` remains the nearest real parent path.
Writes therefore run the parent field's validation rules. A projection of a
projection keeps that same real validation path.

If the projection no longer exists, reads and writes return
`FormFieldError::ValueUnavailable`. The structural owner drops or rebuilds the
control instead of inventing a fallback value.

## 12. Share one form across pages

An `Entity<GeneratedFormStore>` may be shared by multiple pages. Each page
creates its own bound handles and page-level observation. Every control receives
the same typed value projection, while focus, selection, popup/query state,
private editor text, and subscriptions remain local to that control instance.

Dropping one page removes only its bindings. The form, other pages, current
value, validation state, and application persistence tasks remain governed by
their own owners.

## 13. Responsibility map

| Responsibility | Owner |
| --- | --- |
| Current typed value, baseline, revision, validation report/tasks, submit preparation | `gpui-form` generated store |
| Typed field/schema/nested traversal generation | `gpui-form-macros` |
| Native component entity and binding subscriptions | owning bound control |
| Focus, IME, selection, popup/query/highlight, incomplete editor text | concrete component state |
| Options/catalogs/capabilities/disabled/presentation | application and component |
| Save task/loading/retry/provider/database errors | page, controller, or application store |
| Error rendering, locale observation, post-submit focus | application |

## 14. Related documentation

- [gpui-form-macros user guide](../../gpui-form-macros/docs/guide.md)
- [gpui-form-gpui-component user guide](../../gpui-form-gpui-component/docs/guide.md)
