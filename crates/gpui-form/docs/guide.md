# gpui-form vNext user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

## 1. Add the pieces a page needs

An application normally uses the core crate and a native-control adapter. A
validator adapter such as Garde is optional; it is never the path authority.

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
# garde.workspace = true # optional validator adapter
```

The examples use this common prelude:

```rust,ignore
use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    form::field,
    input::{Input, InputState},
};
use gpui_form::{
    DynamicPath, Form, FormRevision, FormSchema, Position, Prepared, TotalPath,
    ValidationMessage, ValidationRequest, ValidationSink, ValidationTrigger,
    Validator,
};
use gpui_form_gpui_component::{
    FormInput, FormIntegerInput, IntegerInputState,
};
```

Use one `Entity<Form<M>>` per editing session. The form owns the draft and its
editing runtime; the page owns persistence and presentation; native controls own
focus, IME, popup state, selection, and incomplete editor text.

## 2. Declare a draft and create its session

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

`required` and `validate(...)` are static leaf-schema metadata, not component
configuration or a second value copy.

`FormSchema` produces static definitions on the model type:

```rust,ignore
ProviderDraft::NAME: FieldDef<ProviderDraft, String>
ProviderDraft::RETRY_LIMIT: FieldDef<ProviderDraft, u32>
```

Create a session by supplying the initial draft and a validator. Use
`Form::try_new` when the session does not need an injected validator.

```rust,ignore
let runtime = Form::try_new_with_validator(
    ProviderDraft { name: String::new(), retry_limit: 3, enabled: true },
    ProviderValidator::new(reserved_names),
)?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| runtime);
```

Section 6 implements `ProviderValidator`; `reserved_names` is an
`Arc<HashSet<String>>` supplied by the page.

The same schema may be used in another session with a different validator or
validator context. Updating an options catalog does not mutate the form; its
owner explicitly refreshes validator data and requests dynamic validation when
product rules require it.

## 3. Put form, controls, and observation in the page

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

A root `FieldDef<M, T>` is a total-path shorthand. Static nested paths are also
total; an item, enum case, or optional payload produces a dynamic path and uses
the adapter's `try_new` constructor instead.

Do not reproduce every native-control rule in this guide. The
[component adapter guide](../../gpui-form-gpui-component/docs/guide.md) explains
deferred Input/Integer/Select/Combobox events, options refreshes, teardown, and
custom stateful adapters.

## 4. Render fields and session status

Static schema answers questions such as whether a label is required. The
explicit form answers questions about the live editing session.

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let pending = self.form.read(cx).is_validating();
let dirty = self.form.read(cx).is_dirty();
let valid = self.form.read(cx).is_valid();
let feedback = errors
    .first()
    .map(|issue| validation_text(issue, cx))
    .unwrap_or_else(|| if pending { "Checking…".into() } else { String::new() });

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(feedback)
    .child(Input::new(&self.name_input));
```

The form-level query family includes dirty, valid, pending, validation report,
errors at a path, first blocking error path, and revision. The form never owns touched/blurred/error-visibility mirrors;
after failed submit, the page chooses a visible native control to focus.

Stateless controls use the same total shorthand when their callback is not
running inside a native state-entity update:

```rust,ignore
let enabled = ProviderDraft::ENABLED;
let checked = enabled.value(&self.form, cx);
let form = self.form.clone();

Checkbox::new("provider-enabled")
    .checked(checked)
    .on_click(move |checked, _, cx| enabled.set(&form, *checked, cx));
```

## 5. Replace, reset, rebase, and revisions

Use whole-form lifecycle operations when application data is installed or a
save has produced a canonical model:

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved, cx));
```

- `replace` installs a new current draft and retains the baseline.
- `reset` restores the baseline as the current draft.
- `rebase` installs one model as both current draft and baseline.

Each lifecycle operation advances revision and reprojects mounted controls even
when Rust values compare equal. It invalidates old async and topology lifetime
work, clears stale validation state, and does not pretend that every leaf was a
user change. `rebase_if_revision` is the only async-save merge primitive: a
failed comparison changes no draft, baseline, report, task, or control.

## 6. Validate synchronously, dynamically, and asynchronously

### Validation rules and scoped results

The runtime supports mount, change, blur, dynamic, and submit triggers.
Required semantics are fixed: trimmed-empty strings, `None`, empty supported
collections, and `false` are missing; numeric and enum values are not
implicitly missing.

A change scope includes the changed path, its descendants, and structural
ancestors, but not unrelated siblings. A validation run replaces only the
source/trigger/path buckets it owns. This is why changing one field cannot clear
another field's error. Topology errors, such as using a stale item path or
creating a move cycle, reject a transaction before validation; they are not
validation issues.

### Write a session validator

This complete sketch shows the validator flow.

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
        let path = ProviderDraft::NAME;
        if request.includes(&path)
            && !model.name.is_empty()
            && self.reserved_names.contains(model.name.trim())
        {
            out.at(path).error(
                "provider-name-unavailable",
                ValidationMessage::key("provider-name-unavailable"),
            );
        }
    }
}
```

Native validators emit typed or canonical paths through the same sink. A Garde
adapter, when used, maps positional collection reports through the topology of
the same form snapshot to runtime-located item paths and active cases. A stale,
unknown, inactive, or unresolvable adapter path becomes a blocking internal
form issue rather than being dropped or attached to another field.

### Request dynamic and async validation deliberately

An owner explicitly requests dynamic validation after an external dependency
change:

```rust,ignore
ProviderDraft::NAME.validate(&self.form, ValidationTrigger::Dynamic, cx);
```

The page also decides when a remote check is worthwhile. It starts one against a
path and input snapshot; after that the form owns cancellation, generation,
address/incarnation freshness, and result publication:

```rust,ignore
self.form.update(cx, |form, cx| {
    form.start_async_validation(
        ProviderDraft::NAME,
        "provider-name",
        |name| async move { directory.check_name(name).await },
        cx,
    )
})?;
```

An intersecting write, lifecycle replacement, subtree removal, or stale
completion cannot publish an old result. Pending form-owned async validation
blocks `prepare`. A native editor that cannot currently produce typed `T` keeps
its raw text locally and publishes a lifecycle-scoped control issue through its
binding.

## 7. Prepare, persist, and conditionally rebase

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

let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;
let revision = prepared.revision();
let request = prepared.map(SaveProvider::from);

self.start_save(revision, request, cx);

// In the page-owned async completion callback:
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(revision, canonical_saved_model, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`prepare` runs submit validation on one snapshot, rejects blocking data/control
issues and pending async work, and atomically captures that snapshot plus
revision. `Prepared<M>::map` consumes it and transforms it once. Persistence,
loading, retry, and notifications remain application work.

## 8. Compose static, optional, and recursive paths

### Static and optional children

```rust,ignore
#[derive(Clone, FormSchema)]
struct AuthDraft {
    username: String,
}

#[derive(Clone, FormSchema)]
struct RedirectDraft {
    callback_url: String,
}

#[derive(Clone, FormSchema)]
struct ServerDraft {
    #[form(child)]
    auth: AuthDraft,
    #[form(child)]
    redirect: Option<RedirectDraft>,
}

let username: TotalPath<ServerDraft, String> =
    ServerDraft::AUTH.then(AuthDraft::USERNAME);
// `server_form: Entity<Form<ServerDraft>>` owns this editing session.
let callback: DynamicPath<ServerDraft, String> = ServerDraft::REDIRECT
    .try_some(server_form.read(cx))?
    .then(RedirectDraft::CALLBACK_URL);
```

Static composition remains total. `try_some(form)` and
`try_case(form, case_def)` explicitly resolve the current optional/case
incarnation and return a dynamic path; an `ItemPath` returned by Form is already
dynamic. All descendants stay dynamic and use `try_*` operations. The returned
path stores no form entity, and callers never supply an item ID.

### Recursive runtime-located trees

The business model contains only query data. It does not carry IDs added for
form navigation:

```rust,ignore
#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    root: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
struct FilterNode {
    #[form(child)]
    kind: FilterNodeKind,
}

#[derive(Clone, FormSchema)]
enum FilterNodeKind {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
}

let runtime = Form::try_new(QueryDraft {
    root: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Group(FilterGroup {
                children: vec![FilterNode {
                    kind: FilterNodeKind::Condition(FilterCondition {
                        value: String::new(),
                    }),
                }],
            }),
        }],
    },
})?;
let query_form: Entity<Form<QueryDraft>> = cx.new(|_| runtime);

let root_children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let group_node = root_children
    .items(&query_form, cx)?
    .into_iter()
    .next()
    .expect("the example contains one root node");

let nested_children = group_node
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);
let condition_node = nested_children
    .try_items(&query_form, cx)?
    .into_iter()
    .next()
    .expect("the example contains one nested node");

let value = condition_node
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::CONDITION)?
    .then(FilterCondition::VALUE);

let current: String = value.try_value(&query_form, cx)?;
value.try_set(&query_form, "Rust".to_owned(), cx)?;
```

`Form` assigns an opaque session-local identity to every item occurrence while
building the session. `items` and `try_items` return ordered typed `ItemPath`
values carrying that identity. Choosing an entry from the returned `Vec` uses
its current position only for selection; the resulting path never stores or
later resolves by that index. The token cannot be constructed, read, serialized,
or placed in the business model by the caller.

Collections are not writable `Vec<T>` leaves. Topology methods accept and return
the same typed item paths instead of raw IDs:

```rust,ignore
let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let anchor = children.items(&query_form, cx)?.into_iter().next().unwrap();

let appended = children.append(
    &query_form,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
let inserted = children.insert_before(
    &query_form,
    &anchor,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
children.move_before(&query_form, &appended, &anchor, cx)?;
children.remove(&query_form, inserted, cx)?;

let fresh = children.replace_all(
    &query_form,
    vec![
        FilterNode {
            kind: FilterNodeKind::Group(FilterGroup { children: Vec::new() }),
        },
        FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
        },
    ],
    cx,
)?;

let parent = fresh[0].clone();
let source = fresh[1].clone();
let destination = parent
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);
let moved = source.move_to(&query_form, destination, Position::End, cx)?;
```

`append` and `insert_before` return the new occurrence; `replace_all` returns
fresh paths for every replacement item. Same-parent reorder preserves an item
path. Removal, whole-collection replacement, case reconstruction, whole-model
replacement/rebase, or a cross-parent move retires affected old paths. A
cross-parent move is one cycle-checked root transaction and returns the new
destination path; an old binding never follows it implicitly.

## 9. Ownership and lifetime rules

Static definitions own schema edges and typed accessors. Located paths own a
typed access plan, canonical address, and static target. `Form<M>` owns the
mutable draft runtime. No path or definition owns an entity, value, validation
report, subscription, or native control.

A canonical address identifies a position, while an incarnation identifies the
current object at that position. `try_case` and `try_some` read the current
`&Form<M>` and capture that incarnation in an entity-free path. Only deferred
bindings and async work hold weak form ownership; they also capture address,
incarnation, generation, and a control-issue lease. Stale callbacks become
no-ops. The renderer destroys native entities and subscriptions when a dynamic
subtree disappears.

## 10. Contract summary

Static `then` needs no form. `try_case(&Form, CaseDef)` and `try_some(&Form)`
capture the current incarnation, while Form keeps topology snapshots private.
Form owns item identity and returns typed located paths rather than raw IDs.
Lifecycle replacement retires old paths, bindings, issues, and async
completions instead of reviving them at the same structural location.

## Related documentation

- [Documentation index](README.md)
- [Macro guide](../../gpui-form-macros/docs/guide.md)
- [Component adapter guide](../../gpui-form-gpui-component/docs/guide.md)
