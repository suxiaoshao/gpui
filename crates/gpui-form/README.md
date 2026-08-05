# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form` lets a GPUI page edit one typed Rust draft, validate it, prepare one
snapshot for saving, and safely apply the canonical saved result. Start with one
ordinary page; recursive trees use the same session and path rules later.

## Add dependencies and imports

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

The complete example below uses this prelude; application-owned save types and
I/O methods are introduced where they are called.

```rust,ignore
use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    form::field,
    input::{Input, InputState},
};
use gpui_form::{
    Form, FormRevision, FormSchema, PrepareError, Prepared, ValidationMessage,
    ValidationRequest, ValidationSink, Validator,
};
use gpui_form_gpui_component::{
    FormInput, FormIntegerInput, IntegerInputState,
};
```

## A complete provider form

### 1. Describe the draft

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_change, on_blur, on_submit))]
    name: String,

    #[form(validate(on_change, on_submit))]
    retry_limit: u32,

    enabled: bool,
}
```

`FormSchema` creates reusable static definitions such as
`ProviderDraft::NAME: FieldDef<ProviderDraft, String>`. A definition contains
schema metadata and typed access only; it never holds a value, form entity,
subscription, or control state.

`required` and `validate(...)` configure the generated leaf schema and its
validation triggers.

### 2. Inject validation into one editing session

The validator belongs to the session, so the same model can be edited with
different application dependencies in different pages.

```rust,ignore
struct ProviderValidator {
    reserved_names: Arc<HashSet<String>>,
}

impl ProviderValidator {
    fn new(reserved_names: Arc<HashSet<String>>) -> Self {
        Self { reserved_names }
    }
}

impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        model: &ProviderDraft,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<ProviderDraft>,
    ) {
        if request.includes(&ProviderDraft::NAME)
            && self.reserved_names.contains(model.name.trim())
        {
            out.at(ProviderDraft::NAME).error(
                "provider-name-reserved",
                ValidationMessage::key("provider-name-reserved"),
            );
        }

        if request.includes(&ProviderDraft::RETRY_LIMIT) && model.retry_limit > 10 {
            out.at(ProviderDraft::RETRY_LIMIT).error(
                "retry-limit-too-large",
                ValidationMessage::key("retry-limit-too-large"),
            );
        }
    }
}

let reserved_names = Arc::new(HashSet::from(["default".to_owned()]));
let runtime = Form::try_new_with_validator(
    ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    },
    ProviderValidator::new(reserved_names),
)?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| runtime);
```

`Form<M>` owns the current draft, baseline, revision, validation report, and
form-owned async validation work for this one editing session.
`#[form(required)]` handles the missing-name rule; the injected validator adds
business rules that depend on this page's catalog.

### 3. Let the page own controls and one observation

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

impl ProviderPage {
    fn new(
        reserved_names: Arc<HashSet<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let runtime = Form::try_new_with_validator(
            ProviderDraft { name: String::new(), retry_limit: 3, enabled: true },
            ProviderValidator::new(reserved_names),
        )?;
        let form = cx.new(|_| runtime);

        let name_input = FormInput::new(
            &form,
            ProviderDraft::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );
        let retry_limit_input = FormIntegerInput::new(
            &form,
            ProviderDraft::RETRY_LIMIT,
            |window, cx| IntegerInputState::new(window, cx)
                .min(0u32).max(10u32).step(1u32),
            window,
            cx,
        )?;
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self { form, form_observer, name_input, retry_limit_input })
    }
}
```

`FieldDef` at the root is the intended total-path shorthand, so `FormInput::new`
does not need a path-resolution result. Stateful-control construction and native
event synchronization are covered in the
[component adapter guide](../gpui-form-gpui-component/docs/guide.md).

### 4. Render schema metadata and live form status

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let is_pending = self.form.read(cx).is_validating();
let feedback = errors
    .first()
    .map(|issue| validation_text(issue, cx))
    .unwrap_or_else(|| if is_pending { "Checking…".into() } else { String::new() });

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(feedback)
    .child(Input::new(&self.name_input));
```

The page also reads form-level dirty, valid, pending, report, and revision state
to render buttons and summaries. The form does not decide when an error becomes
visible or which control receives focus after a failed submit; the active page
does.

### 5. Prepare, save, and conditionally rebase

`Prepared<M>::map` consumes the prepared snapshot, so capture its revision
before mapping it into a request:

```rust,ignore
struct SaveProvider {
    name: String,
    retry_limit: u32,
    enabled: bool,
}

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self {
            name: draft.name,
            retry_limit: draft.retry_limit,
            enabled: draft.enabled,
        }
    }
}

fn save(&mut self, cx: &mut Context<Self>) -> Result<(), PrepareError> {
    let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
        form.prepare(cx)
    })?;

    let revision = prepared.revision();
    let request = prepared.map(SaveProvider::from);
    self.start_save(revision, request, cx); // page-owned async operation
    Ok(())
}

fn save_finished(
    &mut self,
    submitted_revision: FormRevision,
    saved: ProviderDraft,
    cx: &mut Context<Self>,
) {
    let applied = self.form.update(cx, |form, cx| {
        form.rebase_if_revision(submitted_revision, saved, cx)
    });
    if !applied {
        self.show_saved_while_editing_notice(cx);
    }
}
```

`prepare` validates one snapshot, rejects blocking data/control issues and
pending async validation, then captures that snapshot with its revision. Saving,
loading, retry, and notifications remain page or controller work. A failed CAS
never overwrites edits made while saving.

## Dynamic items do not need model IDs

Mark a structured collection with `#[form(items)]`, but keep form navigation
identity out of the business draft:

```rust,ignore
#[derive(Clone, FormSchema)]
struct HeaderDraft {
    name: String,
}

#[derive(Clone, FormSchema)]
struct RequestDraft {
    #[form(items)]
    headers: Vec<HeaderDraft>,
}

let headers = RequestDraft::HEADERS;
let header = headers.append(
    &request_form,
    HeaderDraft { name: String::new() },
    cx,
)?;
let name = header.then(HeaderDraft::NAME);
name.try_set(&request_form, "Authorization".to_owned(), cx)?;
```

Form generates the item's stable session-local identity and returns it inside a
typed `ItemPath`. `items`, `append`, `insert_before`, and `replace_all` produce
these paths; remove and move operations consume or compare them. Callers never
declare `#[form(identity)]`, construct a raw item ID, or persist form identity
in `RequestDraft`.

Dynamic enum and optional locations are resolved against the current session:

```rust,ignore
let payload = enum_path.try_case(form_entity.read(cx), EnumDraft::PAYLOAD)?;
let child = optional_path.try_some(form_entity.read(cx))?;
```

`try_case` and `try_some` capture the active incarnation without storing the
form entity in the returned `DynamicPath`. Static `.then(...)` composition does
not require a form. If a case changes `A -> B -> A`, or an option changes
`Some -> None -> Some`, the old dynamic path remains retired. Callers never see
or pass `TopologyIndex`; Form uses one private topology snapshot for each
resolve, validation, or mutation transaction.

## What comes next

- [User guide](docs/guide.md): validation workflow, lifecycle replacement,
  optional and recursive paths, runtime-located collection topology, and
  lifetime rules.
- [Macro guide](../gpui-form-macros/docs/guide.md): `FormSchema`, schema
  fragments, enum cases, runtime item paths, and compile-time diagnostics.
- [Component adapter guide](../gpui-form-gpui-component/docs/guide.md): native
  controls, deferred bindings, integer/select/combobox behavior, and custom
  adapters.
