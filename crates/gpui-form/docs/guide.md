# gpui-form v2 user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form` owns an editable typed draft. It validates and prepares that draft,
but it does not own remote I/O, shared application state, component interaction
state, or a second copy of the business value.

## 1. Crates and boundaries

Applications normally use the core crate plus a component adapter:

```toml
[dependencies]
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
garde.workspace = true
```

The three relevant layers have distinct ownership:

| Fact | Owner |
| --- | --- |
| Typed draft, baseline, revision, validation, prepared output | generated `FormState` |
| Catalogs, selected records, shared loaded data | `gpui-store` or another application state owner |
| Save/refresh/retry task, loading and notifications | page/controller plus `gpui-operation` when applicable |
| Focus, IME, selection, popup/query, incomplete editor text | native component state |
| Binding subscriptions and deferred control intent | owning control and `ControlBinding` |

The form never writes a shared store as a side effect of editing. A successful
operation explicitly applies its canonical result through
`rebase_if_revision`.

## 2. Declare a model and generated state

The model uses the exact Rust types that the application will submit.
`FormModel` generates the named entity state and one allocation-free
schema-level `SCREAMING_SNAKE_CASE` associated const for every statically
declared field:

```rust,ignore
use gpui_form::FormModel;

#[derive(Clone, Debug, PartialEq, FormModel)]
#[form(state = ProviderForm)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,

    #[form(validate(on_submit))]
    retry_limit: u32,

    #[form(validate(on_dynamic, on_submit))]
    model_id: Option<String>,
}
```

The generated `ProviderForm` implements `FormState` and owns exactly one
runtime containing the current model, baseline, monotonic `FormRevision`, typed
validation context, validation report, and started async-validation tasks.

`ProviderForm::NAME: FormField<ProviderForm, String>` is a pure typed lens and
schema descriptor—not a form handle. Every statically declared model field has
one such associated const. It can be reused directly as a tiny descriptor with
only static schema/access information. Accessing it constructs no per-form or
per-field state and performs no allocation; the descriptor stores no value,
subscription, `Entity<ProviderForm>`, or `WeakEntity<ProviderForm>`.

The generated state is the only entity required for an editing session:

```rust,ignore
use gpui::{AppContext as _, Entity};
use gpui_form::FormState as _;

let form: Entity<ProviderForm> = cx.new(|cx| ProviderForm::from_value(
    ProviderInput {
        name: String::new(),
        retry_limit: 3,
        model_id: None,
    },
    cx,
));
```

`from_value` is available when `ValidationContext: Default`. Otherwise use
`from_value_with_validation_context(initial, context, cx)`. Construction installs
the model and context, then runs mount validation exactly once.

## 3. Total descriptors and partial descriptors

Most field paths are statically present. A normal `FormField<Form, T>` is
**total**: every synchronous API explicitly takes a strong form entity and has
no structural `Result`.

```rust,ignore
let current: String = ProviderForm::NAME.value(&form, cx);
let issues = ProviderForm::NAME.errors(&form, cx);
let validating = ProviderForm::NAME.is_validating(&form, cx);

ProviderForm::NAME.set(&form, "OpenAI".to_owned(), cx);
ProviderForm::NAME.validate(&form, ValidationTrigger::Dynamic, cx);
```

There is only one write verb: `set`. It stores the typed value, advances the
revision once when the value changed, invalidates intersecting validation, runs
the field's change validation, emits one event, and notifies once. Equal writes
are complete no-ops. The library does not expose a separate `set_user_value`.

An identified item or computed projection can disappear. It returns a
`PartialFormField<Form, T>` and exposes only `try_*` operations:

```rust,ignore
let partial_parent = ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(partial_parent);

let name = header_name.try_value(&form, cx)?;
header_name.try_set(&form, "Authorization".to_owned(), cx)?;
let issues = header_name.try_errors(&form, cx)?;
```

`FieldAccessError` represents an unavailable projection or a missing/duplicate
stable item. `FieldMutationError` wraps access errors and also rejects a write
that changes a captured stable ID. Neither includes `FormReleased`: a
synchronous caller passed `&Entity<Form>` and therefore already established
form liveness.

`AuthForm::USERNAME.within(ServerForm::AUTH)` is total: composition preserves
the total availability of its static parent. `ServerForm::HEADERS.item(id)` is a
runtime-addressed `PartialFormField`; `HeaderRowForm::NAME.within(partial_parent)`
therefore remains partial. `within` preserves the parent descriptor's
total/partial availability, while `project_value` and `item` create a partial
descriptor; every child of a partial descriptor remains partial. `project_value`
is partial even when its input descriptor is static or total-by-composition.
`within` and `item` create lightweight located descriptors rather than new
schema definitions. The marker that implements this rule is an implementation
detail—the public guide uses `FormField` and `PartialFormField`, not marker
generics.

## 4. Create bound controls

The component adapter creates a native component state and binds it in one
call. The form is explicit; a total field constructor is infallible except for
the component's own domain errors:

```rust,ignore
use gpui_component::input::InputState;
use gpui_form_gpui_component::{FormInput, FormIntegerInput, IntegerInputState};

let name_input = FormInput::new(
    &form,
    ProviderForm::NAME,
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
);

let retry_limit_input = FormIntegerInput::new(
    &form,
    ProviderForm::RETRY_LIMIT,
    |window, cx| IntegerInputState::new(window, cx)
        .min(0u32).max(10u32).step(1u32),
    window,
    cx,
)?; // invalid integer bounds remain a real component error
```

`FormInput::try_new(&form, partial_field, ...)` is the corresponding constructor
for a `PartialFormField`; it can return `FieldAccessError`. A bound control is a
plain Rust newtype with exactly `subscriptions: Vec<Subscription>` followed by
its native `Entity<State>`, and dereferences to that entity. It does not store a
form, descriptor, `ControlBinding`, value snapshot, options, focus flag, or
temporary editor text.

The binding internally owns the weak relation needed after a native entity event
has started. It defers the typed write until the emitter update ends, upgrades
the form only then, and silently cancels if the form or a partial path no longer
exists. Direct synchronous page code passes the strong entity and never sees
that weak-lifetime path.

Observe the form once at the page/controller lifetime to rerender labels,
validation feedback, and buttons:

```rust,ignore
let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());
```

## 5. Render fields and form runtime

Schema is static on the descriptor; runtime state comes from the explicit form:

```rust,ignore
let error = ProviderForm::NAME.errors(&self.form, cx)
    .first()
    .map(|issue| validation_text(&issue.message, cx));
let is_validating = ProviderForm::NAME.is_validating(&self.form, cx);

field()
    .label("Provider name")
    .required(ProviderForm::NAME.schema().is_required())
    .child(Input::new(&self.name_input));
```

Useful form-level queries remain `is_dirty`, `is_valid`, `is_validating`,
`validation_report`, `errors_at`, `first_error_path`, and `revision`. The form
owns no focus, touched, blurred, or error-visibility state. After a failed
submit, the active page chooses which visible native control to focus.

Stateless typed elements use the same total descriptor:

```rust,ignore
let form = self.form.clone();
let checked = ProviderForm::ENABLED.value(&form, cx);

Checkbox::new("provider-enabled")
    .checked(checked)
    .on_click(move |checked, _, cx| ProviderForm::ENABLED.set(&form, *checked, cx));
```

This direct write is appropriate only when the callback is not inside a native
state entity's active update. Stateful component events always go through the
adapter's deferred `ControlBinding` path to avoid GPUI reentrancy.

Options and catalogs are configuration, not form data: update the application
store, update or rebuild the native delegate, silently project the authoritative
form value, then explicitly request dynamic validation if necessary. Never pick
the first option, rewrite/rebase a value, or persist as an options-refresh side
effect.

## 6. Whole-form lifecycle and revisions

Use lifecycle operations when installing application data:

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved, cx));
```

`replace` changes current value and retains baseline; `reset` restores baseline;
`rebase` installs both current value and baseline. Each lifecycle operation
advances the revision even for an equal Rust value, cancels affected async work,
clears stale data validation, and causes value reprojection. It does not
synthesize individual field change validation.

`rebase_if_revision(expected, saved, cx)` is the sole async-save merge primitive.
A failed comparison changes no draft, baseline, revision, report, task, or
control. A success advances the revision, so two results for the same submitted
revision cannot both apply.

## 7. Validation

### Triggers, paths, and scoped buckets

Supported triggers are mount, change, blur, dynamic, and submit. `required`
always participates in submit and may opt into earlier triggers. Strings are
missing when trimmed-empty; `None`, empty supported collections, and `false`
are missing. Numeric and enum values have no implicit missing semantics.

`ValidationScope::Field(path)` includes the changed path, descendants, and
ancestor group/array paths, but not sibling leaves. Group and identified-item
scopes include their subtree plus ancestors; `Form` includes every data path.

The runtime stores adapter results in normalized, source-and-path buckets. A
scoped validation run replaces only buckets selected by both scope and trigger;
it preserves sibling adapter issues. An adapter's form-level issue is valid only
for `ValidationScope::Form` and is replaced only by another form-wide run.
Control issues retain their separate lifecycle-scoped buckets.

The core resolves every adapter path against the model snapshot before scope and
trigger filtering. Unknown, malformed, duplicate, or unconvertible stable paths
become blocking internal form issues rather than silently disappearing. Schema
ownership is exact: an array owns its container and direct item roots; a nested
item leaf uses that leaf's own schema triggers.

### Garde and custom adapters

Garde validates synchronous model/business rules; keep empty-value semantics in
`#[form(required)]` rather than duplicating them in Garde:

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormModel, garde::Validate)]
#[form(state = AccountForm, validation(adapter = "garde"))]
#[garde(allow_unvalidated)]
struct AccountInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    display_name: String,

    #[form(validate(on_change, on_blur, on_dynamic, on_submit))]
    #[garde(email)]
    email: Option<String>,
}
```

For external dependencies, put typed data in the validation context and replace
that context explicitly. A Garde message provider is a static type-level policy
that maps Garde rules to `ValidationMessage`; it is neither stored nor
`Default`-constructed. Applications can return stable keys and parameters and
translate them with the current locale while rendering.

A custom `ValidationAdapter<Model>` is a static associated policy of the
generated state, not a stored or `Default`-constructed value. It receives the
typed validation context directly and must treat `ValidationScope` as the
boundary of the bucket set it reports:

```rust,ignore
impl ValidationAdapter<ProviderInput> for ProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        value: &ProviderInput,
        trigger: ValidationTrigger,
        scope: &ValidationScope,
        context: &Self::Context,
        cx: &App,
    ) -> ValidationAdapterReport {
        let mut report = ValidationAdapterReport::default();
        let path = ProviderForm::MODEL_ID.path().clone();
        if scope.includes(&path)
            && value.model_id.as_ref().is_some_and(|id| !context.model_ids.contains(id))
        {
            report.push(ValidationIssue::field(
                path, trigger,
                ValidationSource::App("provider".into()),
                "model_unavailable", ValidationMessage::key("provider-model-unavailable"),
            ));
        }
        report
    }
}
```

The core still normalizes returned paths and enforces scope/trigger ownership;
using the scope is both a performance opportunity and a way for an adapter to
avoid reporting unrelated siblings.

### Asynchronous validation and control issues

The page decides when to begin a remote check. Once started, the form owns its
task, generation, and scope checks:

```rust,ignore
ProviderForm::NAME.start_async_validation(
    &self.form,
    "provider-name",
    ValidationTrigger::Change,
    move |name| async move { service.check_name(name).await },
    cx,
);
```

An intersecting write, cancellation, lifecycle operation, or form drop cancels
the task. Stale completion cannot replace newer state. Active form-owned async
validation blocks `prepare_submit`; nonblocking remote hints belong to
application UI.

A native editor that temporarily cannot produce `T` keeps its raw text locally
and publishes a lifecycle-scoped control issue through `ControlBinding`. That
issue blocks submit only while its binding remains mounted.

## 8. Prepare submit and persist

`prepare_submit` validates one snapshot, rejects validation/control/pending
async issues, and transforms the same snapshot exactly once:

```rust,ignore
use gpui_form::PreparedSubmit;

let PreparedSubmit { revision, output } = self.form.update(cx, |form, cx| {
    form.prepare_submit(cx)
})?;
```

`PreparedSubmit` prevents callers from accidentally reading revision and output
from different model versions. `SubmitTransform<Model>` is a static,
infallible transform selected by the generated state:

```rust,ignore
struct ProviderTransform;

impl SubmitTransform<ProviderInput> for ProviderTransform {
    type Output = SaveProvider;

    fn transform(model: &ProviderInput) -> SaveProvider {
        SaveProvider {
            name: model.name.trim().to_owned(),
            retry_limit: model.retry_limit,
        }
    }
}
```

There is no `TransformReport` or transform failure variant. A condition that
should render inline belongs in validation; remote/provider/database failures
belong to the application operation. On successful persistence, apply the
canonical saved value with `rebase_if_revision(revision, saved, cx)`.

## 9. Nested models, arrays, and projections

Nested data remains in one root entity; static associated-const descriptors
compose without creating child forms, capturing a root entity, or instantiating
field state:

```rust,ignore
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
let username = username.value(&form, cx);

let partial_parent = ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(partial_parent);
let header_name = header_name.try_value(&form, cx)?;
```

Stable IDs are unique within an array and immutable through an identified-item
descriptor. Whole-array `set` is the explicit operation for insertion, removal,
replacement, and reorder. The library neither selects the first duplicate nor
silently repairs IDs.

`project_value` creates a distinct projection path for controls and async issues
while retaining its nearest real model path for validation. It is always partial,
including when called directly on a static descriptor or a total composition:

```rust,ignore
let budget = JobForm::RUN_SETTINGS.project_value(
    "token_budget",
    |settings| settings.custom_token_budget(),
    |settings, value| settings.set_custom_token_budget(value),
);
let budget = budget.try_value(&form, cx)?;
```

## 10. Implement a custom stateful control

There is no `FormControl` trait. A custom control has an inherent constructor
that accepts the form explicitly and creates a `ControlBinding` at the
long-lived callback boundary:

```rust,ignore
pub struct FormRating {
    subscriptions: Vec<Subscription>,
    rating: Entity<RatingState>,
}

impl FormRating {
    pub fn new<Owner>(
        form: &Entity<ReviewForm>,
        field: FormField<ReviewForm, Rating>,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self {
        let binding = field.bind_control(form, cx);
        // Build and silently project initial value. Capture binding clones only
        // in event/projection subscriptions; do not retain it as a struct field.
        todo!()
    }
}
```

`ControlBinding` is cloneable and owns the internal control lease. Its public
deferred intents are `defer_set`, `defer_blur`, `defer_set_issue`, and
`defer_clear_issue`. It exposes neither a weak form handle, immediate mutation,
control ID, nor component read-back. A partial field uses `try_bind_control`.

A descriptor subscription silently reprojects when a `ValueChanged` path can
affect that descriptor, and after every `ModelReplaced`, including after that
control's own edit. `ValidationChanged` is ignored for value projection. Do
not add origin skipping.

## 11. Related documentation

- [Project documentation index](README.md)
- [gpui-form-macros guide](../../gpui-form-macros/docs/guide.md)
- [gpui-form-gpui-component guide](../../gpui-form-gpui-component/docs/guide.md)
