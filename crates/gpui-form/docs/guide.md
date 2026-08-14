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
selection, popup state, and incomplete text.

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
    .with_validator(ProviderValidator::new(reserved_names))
});
```

`Form::new` and `with_validator` are infallible. The descriptor generated for
`name` is a static value such as `ProviderDraft::NAME`; every read or mutation
explicitly receives `&form`.

## 2. Total paths for ordinary fields

A root field descriptor, or a path composed only through `#[form(child)]`
fields, is a `TotalPath<Root, T>`. It exists throughout the editing session,
including after `replace`, `reset`, or `rebase`:

```rust,ignore
let name = ProviderDraft::NAME;

let current: String = name.get(&form, cx);
let changed: bool = name.set(&form, "primary".into(), cx);

let enabled = ProviderDraft::ENABLED;
enabled.set(&form, true, cx);
```

`set` returns whether the model changed. An equal normal write is a model
no-op. Do not make a second page-owned copy of the value; reread the form when
rendering.

Static nesting keeps the same property:

```rust,ignore
#[derive(Clone, FormSchema)]
struct AuthDraft { username: String }

#[derive(Clone, FormSchema)]
struct RedirectDraft { callback_url: String }

#[derive(Clone, FormSchema)]
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
#[derive(Clone, FormSchema)]
struct HeaderDraft { name: String }

#[derive(Clone, FormSchema)]
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
#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
enum FilterNode {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
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
impl Validator<ProviderDraft> for ProviderValidator {
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

impl Validator<QueryDraft> for QueryValidator {
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

`Submit` is the default business-validation trigger. Add schema validation
metadata for `Mount`, `Change`, or `Blur` when immediate feedback is desirable.
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
self.form.update(cx, |form, cx| form.replace(next_draft, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved_draft, cx));
```

- `replace` installs a new current draft and retains the baseline.
- `reset` restores the baseline as the current draft.
- `rebase` installs one model as both current draft and baseline.

Each lifecycle operation is a semantic model change even when its Rust values
compare equal. Total paths remain valid. Old dynamic paths and their pending
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
