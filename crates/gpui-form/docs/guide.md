# gpui-form user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

This guide explains the public contract of a form editing session. It uses a
provider settings page first, then expands it to optional data and recursive
collections. For declaration syntax, see the [macro guide]; for the concrete
`gpui-component` adapters and custom controls, see the [component adapter
guide].

[macro guide]: ../../gpui-form-macros/docs/guide.md
[component adapter guide]: ../../gpui-form-gpui-component/docs/guide.md

The snippets assume the dependencies and imports in the [README]. They omit
unrelated page methods such as `start_save` and `reconcile_query_rows`.

[README]: ../README.md

## 1. Ownership: one form session, explicit at every access

Create one `Entity<Form<M>>` for one editing session. It owns the current
typed model, its baseline, validation facts, and dynamic field locations. A
page owns save/load operations, button policy, error visibility, focus, and
presentation. A native control owns its editing state such as IME, cursor,
selection, and popup state. The Form owns unresolved display drafts under stable editor keys.

Schema descriptors never own the session. That makes each descriptor reusable:

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_blur, on_submit))]
    name: String,
    #[form(validate(on_submit))]
    retry_limit: u32,
    enabled: bool,
}

let form = cx.new(|_| {
    Form::new(ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    })
    .with_rules(ProviderValidator::new(reserved_names).rules()).expect("valid rules")
});
```

`Form::new` is infallible; `with_rules` rejects duplicate IDs, missing dependencies, and invalid instances. The descriptor generated for
`name` is a static value such as `ProviderDraft::NAME`; every read or mutation
explicitly receives `&form`.

## 2. Total paths for ordinary fields

A root field descriptor, or a path composed only through `#[form(child)]`
fields, is a `TotalPath<Root, T>`. It exists throughout the editing session,
including after `replace`, `reset`, or `rebase`:

```rust,ignore
let name = ProviderDraft::NAME;

let current: String = name.get(&form, cx);
let changed: bool = name.set(&form, "primary".into(), cx).unwrap();

let enabled = ProviderDraft::ENABLED;
enabled.set(&form, true, cx).unwrap();
```

`set` returns whether the model changed. An equal normal write is a model
no-op. Do not make a second page-owned copy of the value; reread the form when
rendering.

Static nesting keeps the same property:

```rust,ignore
#[derive(Clone, PartialEq, FormSchema)]
struct AuthDraft { username: String }

#[derive(Clone, PartialEq, FormSchema)]
struct RedirectDraft { callback_url: String }

#[derive(Clone, PartialEq, FormSchema)]
struct ServerDraft {
    #[form(child)]
    auth: AuthDraft,
    #[form(child)]
    redirect: Option<RedirectDraft>,
}

let server_form = cx.new(|_| Form::new(ServerDraft {
    auth: AuthDraft { username: String::new() },
    redirect: None,
}));
let username = ServerDraft::AUTH.then(AuthDraft::USERNAME);
let current: String = username.get(&server_form, cx);
```

## 3. Dynamic paths for items, cases, and optional payloads

An item, enum case, or `Option::Some` boundary produces a
`DynamicPath<Root, T>`. Its `try_get`, `try_set`, and adapter constructors return
`ResolveError` when the path belongs to another form session or has retired.

Resolving an inactive enum case or an absent optional child is normal and
returns `Ok(None)`. It is not an error. A retired starting path returns
`Err(ResolveError)` instead:

```rust,ignore
let condition = node
    .then(FilterNode::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&query_form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterCondition::VALUE);
    let current: String = value.try_get(&query_form, cx)?;
    value.try_set(&query_form, "Rust".to_owned(), cx)?;
}

let redirect = ServerDraft::REDIRECT
    .some()
    .resolve(&server_form, cx)?;

if let Some(redirect) = redirect {
    redirect
        .then(RedirectDraft::CALLBACK_URL)
        .try_set(&server_form, "https://example.com/callback".into(), cx)?;
}
```

The return type of the resolved path determines the value type. Passing the
wrong Rust type to `set` or `try_set` is rejected by the type system.

## 4. Collections and recursive typed trees

Use `#[form(items)]` for a structured `Vec<T>`. The business model contains
only business data: Form creates the identity of each item occurrence. Obtain
`ItemPath` values only by enumerating the form or by a collection mutation:

```rust,ignore
#[derive(Clone, PartialEq, FormSchema)]
struct HeaderDraft { name: String }

#[derive(Clone, PartialEq, FormSchema)]
struct RequestDraft {
    #[form(items)]
    headers: Vec<HeaderDraft>,
}

let request_form = cx.new(|_| Form::new(RequestDraft { headers: Vec::new() }));
let headers = RequestDraft::HEADERS;
let header = headers.append(
    &request_form,
    HeaderDraft { name: String::new() },
    cx,
)?;

header
    .then(HeaderDraft::NAME)
    .try_set(&request_form, "Authorization".into(), cx)?;
```

Collection methods are `items`, `try_items`, `append`, `insert_before`,
`move_before`, `remove`, `replace_all`, and `ItemPath::move_to`. They use
typed item paths, not raw IDs, business IDs, or indexes. `ItemPath::key()` is
an opaque stable UI key while that item remains active.

This scales to a recursive filter tree without adding IDs to the model:

```rust,ignore
#[derive(Clone, PartialEq, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, PartialEq, FormSchema)]
enum FilterNode {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterCondition { value: String }

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode::Condition(FilterCondition {
            value: String::new(),
        })],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
for node in children.items(&query_form, cx) {
    let condition = node
        .case(FilterNode::CONDITION)
        .resolve(&query_form, cx)?;

    if let Some(condition) = condition {
        let value = condition.then(FilterCondition::VALUE);
        render_condition(node.key(), value.try_get(&query_form, cx)?);
    }
}
```

Same-parent reordering preserves an item path. Removing and reinserting an
item, reconstructing a case or optional payload, replacing a collection,
replacing/resetting/rebasing the form, and moving across parents retire the old
dynamic path. A path never silently resolves to a later value at the same
apparent location.

## 5. Connect controls and render the page

Use the built-in adapters for normal `gpui-component` controls:

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

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
    "budget-editor",
    |window, cx| IntegerInputState::new(window, cx).min(0u32).max(10u32),
    window,
    cx,
)?;
let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
```

`new` takes a total path. `try_new` takes a resolved dynamic path and can fail
because that location may already be retired. Built-in controls retain their
own binding and subscribe to form changes; keep `form_observer` only when the
page itself must rerender.

Use schema metadata and live form facts separately in render code:

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let pending = self.form.read(cx).is_validating();

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(error_text(errors.first(), pending))
    .child(Input::new(&self.name_input));
```

The page decides whether errors are visible and which visible control receives
focus after a failed submit. Form reports facts; it does not own touched state,
focus, or layout.

For a custom stateful control, use the `ControlBinding`, `ControlWriter`, and
`ControlProjection` protocol in the [component adapter guide]. The control
projects `Value` silently, writes through its writer from native events, and
handles a one-time `Retired` projection. It does not manually subscribe to
`FormEvent` or use a local two-way-binding flag.

## 6. Validation

Write validators against one `ValidationRequest`, which supplies both the
current model and snapshot-bound path resolution:

```rust,ignore
impl ProviderValidator {
    fn rules(self) -> Vec<ValidationRule<ProviderDraft>> {
        vec![ValidationRule::new("ProviderValidator", move |request, out| self.validate(request, out))
            .dependency(ProviderDraft::ROOT)]
    }

    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        let model = request.model();

        if request.includes(&ProviderDraft::NAME) && model.name.trim().is_empty() {
            out.at(ProviderDraft::NAME).error(
                "provider-name-empty",
                ValidationMessage::key("provider-name-empty"),
            );
        }
    }
}
```

Recursive validation uses paths created by that same request. These paths can
be composed and passed to `out.at`, but have no mutation or control-binding
methods and cannot outlive the validation snapshot:

```rust,ignore
fn validate_filter_nodes<'a>(
    request: &ValidationRequest<'a, QueryDraft>,
    nodes: Vec<ValidationItemPath<'a, QueryDraft, FilterNode>>,
    out: &mut ValidationSink<'_, QueryDraft>,
) {
    for node in nodes {
        if let Ok(Some(condition)) = node
            .clone()
            .case(FilterNode::CONDITION)
            .resolve(request)
        {
            let value = condition.then(FilterCondition::VALUE);
            if request
                .try_get(&value)
                .is_ok_and(|value| value.trim().is_empty())
            {
                out.at(value).error(
                    "filter-value-empty",
                    ValidationMessage::key("filter-value-empty"),
                );
            }
            continue;
        }

        if let Ok(Some(group)) = node.case(FilterNode::GROUP).resolve(request) {
            let children = group.then(FilterGroup::CHILDREN);
            if let Ok(children) = request.try_items(&children) {
                validate_filter_nodes(request, children, out);
            }
        }
    }
}

impl QueryValidator {
    fn rules(self) -> Vec<ValidationRule<QueryDraft>> {
        vec![ValidationRule::new("QueryValidator", move |request, out| self.validate(request, out))
            .dependency(QueryDraft::ROOT)]
    }

    fn validate(
        &self,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<'_, QueryDraft>,
    ) {
        let children = QueryDraft::ROOT
            .then(QueryDraft::FILTERS)
            .then(FilterGroup::CHILDREN);
        validate_filter_nodes(&request, request.items(&children), out);
    }
}
```

`Submit` is the default business-validation trigger. Set the rule’s `.triggers(ValidationTriggers { ... })` for `Mount`, `Change`, or `Blur` when immediate feedback is desirable. Schema metadata configures required-field rules.
After a catalog, permission, or other external dependency changes, explicitly
request a scoped validation pass:

```rust,ignore
ProviderDraft::NAME.validate(&form, ValidationTrigger::External, cx);
```

Async validation is likewise started deliberately for a total or currently
resolved dynamic path. The form cancels or discards a result after an
intersecting write, retirement, lifecycle change, or a newer validation run.
Pending form-owned async validation prevents `prepare` from succeeding.

## 7. Replace, reset, prepare, and save

Use lifecycle methods when application data changes:

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next_draft, DraftPolicy::Preserve, cx)?);
self.form.update(cx, |form, cx| form.reset(DraftPolicy::Discard, cx)?);
self.form.update(cx, |form, cx| form.rebase(saved_draft, DraftPolicy::Discard, cx)?);
```

- `replace` installs a new current draft and retains the baseline.
- `reset` restores the baseline as the current draft.
- `rebase` installs one model as both current draft and baseline.

Lifecycle changes advance the editing revision when baseline or topology changes. `ModelChanged` is emitted only when the stored model changes; `SessionChanged` reports other lifecycle changes. Total paths remain valid. Old dynamic paths and their pending
work retire.

Prepare the value before application-owned I/O, and retain its version rather
than a bare revision number:

```rust,ignore
let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;

let (version, request) = prepared
    .map(SaveProvider::from)
    .into_parts();
self.start_save(version, request, cx);

// Later, after a successful save:
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_saved_model, cx)
});
```

`prepare` runs submit validation for one snapshot and rejects blocking issues
or pending async validation. `Prepared::map` keeps its session-bound
`FormVersion`. If `rebase_if_current` returns `false`, it has not changed the
draft: show application UI appropriate for “saved while editing”.

## 8. Observe semantic changes selectively

Use ordinary entity observation for page rendering. Subscribe to `FormEvent`
only when an owner needs a selective side effect, such as reconciling a
recursive row tree:

```rust,ignore
let subscription = cx.subscribe(&query_form, |_, _, event, cx| {
    match event {
        FormEvent::ModelChanged(change) => {
            let target = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroup::CHILDREN);
            let impact = change.impact(&target);

            if impact.structure_changed() || impact.retired() {
                reconcile_query_rows(cx);
            } else if impact.value_changed() {
                cx.notify();
            }
        }
        FormEvent::ValidationChanged { .. } => cx.notify(),
    }
});
```

`ModelChangeKind` distinguishes edit, replace, reset, and rebase. `PathImpact`
answers whether the selected target's value changed, its structure changed, or
it retired. The event has no control origin or implementation identity: controls
already handle their own synchronization, and other owners only consume the
semantic effect relevant to their target.

## Related documentation

- [README](../README.md)
- [Macro guide]
- [Component adapter guide]


## Editing session and breaking API changes (#248)

`Validator` / `with_validator` / `replace_validator` have been removed. Register
explicit rules with `Form::with_rules(Vec<ValidationRule<M>>) -> Result<Self,
RuleRegistrationError>` or `replace_rules(..., cx) -> Result<(), ...>`. Each rule
has a `RuleId`, one or more typed `.dependency(path)` subtrees, optional dynamic
`.instance(path)`, and `.triggers(ValidationTriggers)`. IDs cannot be duplicated
within an instance. The execution request covers the whole snapshot: output
locations do not determine scheduling. Results atomically replace all previous
results owned by that rule, including an empty result and across triggers.
Started rules rerun on dependency edits; unrelated changes leave them alone.
External runs recheck all registered rules. Unresolved dependent drafts suppress
old synchronous results until parsing succeeds. Garde adapters become explicit
rules with `GardeValidator::new(context).into_rule("garde", Model::ROOT)`.

Native writers submit parsed values with `defer_set` or unresolved display data
with `defer_draft(display, code, message, window, cx)`. Bindings use a stable
`EditorKey`; `with_editor_key(key, cx)` reconnects a low-level writer. The Form
retains drafts after unmount. Different editors at the same path keep independent
drafts. Successful parsing clears only that editor. `is_dirty` includes drafts;
format-equivalent parsed values compare by stored value. An edit ticket is
reserved synchronously before deferring native writes: pending admission blocks
`prepare`, and even equal-value input invalidates older save versions. Dropping a
binding/window settles queued tickets; accepted drafts remain in the Form.

`DraftChanged(ModelChange<M>)` reports draft content changes with path impact.
A combined parsed model/draft change shares one revision and notification.
Message-only changes do not advance editing revision. `SessionChanged` reports
baseline/topology lifecycle changes without falsely emitting `ModelChanged`.
Exhaustive event matches must handle both new variants. Value setters are not
called for ordinary draft/validation notifications.

Total paths expose `baseline(&form, cx)` and `is_dirty(&form, cx)`; dynamic paths
and item paths expose `try_baseline -> Result<Option<T>, ResolveError>` and
`try_is_dirty`. New occurrences have no baseline; retired paths return an error.
Collection dirty includes additions, deletion, order and child drafts.
Total-path and descriptor `set` now return `Result<bool, ResolveError>` so a
structural write can reject `WouldRetireDrafts` before changing anything.
Dynamic set/remove/replace-all/cross-parent moves also reject retirement of a
draft. Explicitly call `form.discard_drafts(typed_subtree, cx)?` before a deliberate
destructive structural mutation; scalar programmatic writes preserve drafts.

`replace(value, policy, cx)`, `reset(policy, cx)` and `rebase(value, policy, cx)`
require `DraftPolicy::Preserve` or `Discard` and return
`Result<LifecycleReport, LifecycleError>`. Preserve retains valid drafts and
atomically rejects retirement of dynamic drafts. Preserve rejects pending
native admission with `EditsPending`; Discard settles those tickets and clears
drafts. Reset with Preserve can remain dirty. Discard denotes a deliberate
abandon/load action; ordinary refresh should use preserving rebase.

`rebase_preserving_edits(remote, cx)` atomically merges old baseline, local and
remote values, installs the remote baseline, and returns conflict paths plus a
version. Ordinary fields and direct child leaves merge independently; opaque
fields, collections, Option and enum/case structures merge as whole units.
Concurrent changes retain local values and report conflicts. Drafts conflict
with external changes even when the stored local value equals the old baseline.
Retained local occurrences retain identity; an uncorrelated external baseline
returns None for their dynamic baseline. No index-based matching or generic
keyed collection framework is provided. Reports can be checked using
`form.is_current(report.version())`. They do not lock external persistence.
`rebase_if_current(prepared.version(), saved, cx)` remains guarded and never
accepts pending input or unresolved drafts. Do not turn a stale save result into
a refresh using the old baseline: merging edits made during save requires the
actual submitted snapshot as ancestor and remains application policy.

### Domain field merges

Use `#[form(merge = function_path)]` when a field has domain keys or merge rules.
The function has signature `fn(&T, &T, &T) -> FieldMerge<T>`: old baseline B,
local stored value L, then remote value R. It receives complete typed values,
including unknown or unreadable content retained by the application. It must
be pure: no Form access, GPUI context, I/O or events. For example, an opaque map
can merge independent keys and keep local values on conflicting keys:

```rust
use std::collections::BTreeMap;
use gpui_form::{FieldMerge, FormSchema};

type Overrides = BTreeMap<String, String>;

#[derive(Clone, PartialEq, FormSchema)]
struct Settings {
    #[form(merge = merge_overrides)]
    model_overrides: Overrides,
}

fn merge_overrides(base: &Overrides, local: &Overrides, remote: &Overrides)
    -> FieldMerge<Overrides>
{
    let mut candidate = remote.clone();
    let mut conflict = false;
    for key in base.keys().chain(local.keys()) {
        let (b, l, r) = (base.get(key), local.get(key), remote.get(key));
        if l == b { continue; }
        conflict |= r != b && l != r;
        match l {
            Some(value) => { candidate.insert(key.clone(), value.clone()); }
            None => { candidate.remove(key); }
        }
    }
    if conflict { FieldMerge::conflicted(candidate) }
    else { FieldMerge::merged(candidate) }
}
```

The hook replaces the field's default recursive or whole-unit merge, while
schema traversal, validation and typed paths stay the same. It works with leaf,
`child` and `items` fields. Declare it at the merge boundary: existing traversal
stops at collections, Option and enum units, so hooks inside those units are
not visited automatically. Duplicate hooks are rejected; Rust checks the function
and return types. See the [macro guide](../../gpui-form-macros/docs/guide.md#field-merge-declarations).

Pending native edit tickets reject the entire refresh with `EditsPending`
before any hook runs. Each draft-free hook produces a candidate within the same
transaction as ordinary fields. `conflicted` adds the field's `PathKey` to the
deduplicated `LifecycleReport::conflicts()`; the report version is the installed
version. Domain keys are application data, not dynamic PathKeys.

If any unresolved draft intersects a hook field, Form skips that hook and keeps
the whole local field, original draft text and active dynamic identities.
It reports a field conflict when R differs from B, even if L equals R; R=B only
retains the edit. Other fields continue merging. This deliberately postpones
even independent remote keys in that field until the user resolves or explicitly
discards the draft and refreshes again. A hook cannot authorize draft loss.

Without drafts, a candidate equal to L preserves that field's dynamic identities.
A different candidate retires all its dynamic descendants; rebuild bindings from
new active paths. Other fields retain their identities. There is no matching by
index, value or business key. The stored field baseline is always the original R,
so an unsaved candidate remains dirty. Dynamic baseline correspondence uses the
installed topology when candidate=R, or the previous baseline when R=B and
candidate=L. Otherwise `try_baseline` may return `None`; read the total field's
baseline and project domain keys there. `None` does not mean a domain Unset value.
These hooks apply to `rebase_preserving_edits`; the guarded save-install behavior
of `rebase_if_current` is unchanged.

### Control mappings

`ControlMapping<S, D>` formats storage with a cloned application context and
parses display data with current storage, optional baseline and that same
context snapshot into `EditResult::Parsed`, `Update` or `Draft`. Use
`bind_mapped_control_in` / `try_bind_mapped_control_in` and their returned
`MappedControlWriter::defer_edit`. `MappedControlBinding::refresh(context, cx)`
reprojects clean editors without changing model/revision; drafts retain original
text and are not reparsed. Parsing happens before enqueue, so queued text is
not reinterpreted under a later context. Reusing a draft key with a different
display/mapping type returns `MappingBindError::EditorType`. The native adapter
provides `FormMappedInput`; integer constructors require an explicit editor key.
Applications own inheritance/Unreadable/Unset semantics and business validation.
Native controls continue to own focus, cursor, selection and IME composition;
only committed native Change events enter the Form.

A mapping can restore an inherited baseline without inventing a storage override:

```rust,ignore
#[derive(Clone, PartialEq)]
enum Slot { Unset, Set(i32), Unreadable(String) }
struct NumberMapping;
impl ControlMapping<Slot, String> for NumberMapping {
    type Context = i32; // application's inherited value snapshot
    fn format(&self, stored: &Slot, inherited: &i32) -> String {
        match stored {
            Slot::Unset => inherited.to_string(),
            Slot::Set(n) => n.to_string(),
            Slot::Unreadable(raw) => raw.clone(),
        }
    }
    fn parse(&self, display: String, current: &Slot, baseline: Option<&Slot>, inherited: &i32)
        -> EditResult<Slot, String>
    {
        if let Some(base) = baseline.filter(|b| self.format(b, inherited) == display) {
            return EditResult::Parsed(base.clone());
        }
        if self.format(current, inherited) == display {
            return EditResult::Parsed(current.clone());
        }
        match display.parse() {
            Ok(n) => EditResult::Parsed(Slot::Set(n)),
            Err(_) => EditResult::draft(display, "number", "Enter an integer"),
        }
    }
}
```

### Admission-time mapped updates

`Parsed(value)` explicitly replaces the complete bound value. Independent editors
of one map or composite field should return `EditResult::update(...)`: the
one-shot callback receives the latest storage when its native edit is admitted.
For example, two queued key edits then compose instead of replacing each other's
copies of the map:

```rust,ignore
use std::collections::BTreeMap;
use gpui_form::{ControlMapping, EditProblem, EditResult};

type Overrides = BTreeMap<String, u64>;
struct TokenMapping { key: String }
impl ControlMapping<Overrides, String> for TokenMapping {
    type Context = u64;
    fn format(&self, stored: &Overrides, inherited: &u64) -> String {
        stored.get(&self.key).unwrap_or(inherited).to_string()
    }
    fn parse(&self, display: String, _: &Overrides, baseline: Option<&Overrides>,
             inherited: &u64) -> EditResult<Overrides, String> {
        let Ok(value) = display.parse::<u64>() else {
            return EditResult::draft(display, "number", "Enter an integer");
        };
        let Some(baseline) = baseline else {
            return EditResult::draft(display, "baseline-unavailable", "Refresh before editing");
        };
        let key = self.key.clone();
        let base = baseline.get(&key).copied();
        let target = if value == base.unwrap_or(*inherited) { base } else { Some(value) };
        EditResult::update(move |latest: &Overrides| {
            if latest.contains_key("locked") {
                return Err(EditProblem::new("locked", "Overrides are locked"));
            }
            let mut next = latest.clone();
            match target {
                Some(value) => { next.insert(key, value); }
                None => { next.remove(&key); }
            }
            Ok(next)
        })
    }
}
```

Parse runs once at native event entry, with that entry's storage, baseline and
context. Capture an owned key, parsed value and any required baseline/context
fragments. The update is `FnOnce(&S) -> Result<S, EditProblem> + 'static`, with no
Clone or Send requirement. It must not reparse text, read external state, perform
side effects or panic. Do not capture the entry's whole map as the update base,
or omit an edit because its display equals entry storage: an earlier queued edit
may still need to be undone. Restore only the target baseline leaf on latest.

`Update(StorageUpdate<S>)` and `StorageUpdate::new` expose the same payload;
`EditProblem::new(code, message)` accepts a `ValidationMessage` and has `code()` /
`message()` readers. Exhaustive `EditResult` matches must handle the new variant.
Mapping and adapter constructor signatures are unchanged.

Each call reserves one native ticket before returning. Admissions run in order;
an Update sees earlier accepted edits, and a later Parsed can replace their full
result. Pending input blocks prepare and preserving rebase, and invalidates older
save versions. Cancelled bindings/windows, discarded tickets and retired paths
never execute the callback or create orphan drafts. Context refresh only projects
clean, admitted editors and never changes captured intent or retries an update.

On admission failure, Form retains the original display under the source
`(PathKey, EditorKey)` as a Draft and settles its ticket. The stored value,
baseline and topology stay unchanged. A candidate that would retire dynamic
drafts produces code `form-edit-would-retire-drafts` with a displayable problem;
this also applies to a rejected mapped Parsed replacement. Failures never retry
automatically. Remount with the same key to restore the text; corrected new input
may try again. Success clears only that editor's draft and issue, even for an
equal candidate, while preserving peers' drafts and pending native text. Model
and source-draft changes share one revision; an equal edit that only clears a
draft advances it once. Updates do not change the baseline.

Composite updates keep existing whole-value installation rules: a changed
candidate retires dynamic descendants, and unresolved dynamic drafts cause
refusal. Use typed `discard_drafts` only for an explicit discard. There is no
business-key path or identity reconciliation. Field merge hooks still apply
only to external refresh, including their conservative draft-skipping rule.

### Per-editor drafts

Several editors can share a total map path and use different stable EditorKeys.
Read their accepted unresolved state directly from Form, including after unmount:

```rust,ignore
let status = form.read(cx).editor_status(Settings::MODEL_OVERRIDES, "reserve")?;
let invalid = status.has_draft();
let source_issues = status.issues();

form.update(cx, |form, cx| {
    form.discard_editor_draft(Settings::MODEL_OVERRIDES, "reserve", cx)
})?;
```

`EditorStatus` is an owned read-only snapshot. `has_draft()` means an accepted
unresolved draft; queued native input is not included. Explicit discard still
cancels queued input when `has_draft()` is false. `issues()` contains only
that editor's current source issues. Keep using path errors for field-wide
business validation. Queries neither parse native text nor advance revision,
allocate path identity or require a mounted control. A retained draft remains
discoverable even when its inherited business key disappears from mapping context.
No private control ID is exposed.

`discard_editor_draft(path, editor, cx)` targets exactly that path and EditorKey.
It clears that editor's draft/issues and cancels its pending native tickets;
cancelled Update callbacks never run. Other editors at the same path and editors
under descendant paths keep their drafts and tickets. A peer's pending input still
blocks prepare. The model, baseline and topology stay unchanged. A changed discard
that removes a draft or cancels tickets advances one editing revision and invalidates older save credentials; DraftChanged
shares that revision when a draft was removed. Issue-only changes stay validation-only;
an empty discard is a no-op.
Retired or foreign-session targets return `LifecycleError::InvalidTarget`.
Use `discard_drafts(path, cx)` when explicitly discarding a whole subtree.

`MappedControlBinding`, `MappedControlWriter`, `ControlWriter`, `FormMappedInput`
and the text, integer, select and combobox adapters expose `discard_draft(cx)` /
`editor_status(cx)` for their source editor. A raw `ControlBinding` instead takes
the key explicitly: `discard_editor_draft(editor, cx)` / `editor_status(editor, cx)`.
Known EditorKeys can also be passed by reference; generated control identities stay private.
These APIs are additive; existing constructors retain their signatures.
