# `FormSchema` derive guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

`FormSchema` describes an editable Rust model at compile time. The generated
definitions are static and typed; `gpui-form` supplies the one explicit
`Entity<Form<M>>` session that they operate on. This guide uses the current public API.

Applications normally consume the derive through `gpui-form`:

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## 1. Derive fields and create one form session

Every supported named field receives one static descriptor. The descriptor is
not a field instance and does not retain a form. A page or view owns the form
entity and passes it explicitly to descriptors and composed paths.

```rust,ignore
use gpui::Entity;
use gpui_form::{Form, FormSchema};

#[derive(Clone, FormSchema)]
struct AccountDraft {
    email: String,
    max_projects: u32,
}

let form: Entity<Form<AccountDraft>> = cx.new(|_| {
    Form::new(AccountDraft {
        email: "owner@example.com".to_owned(),
        max_projects: 5,
    })
});

let email: String = AccountDraft::EMAIL.get(&form, cx);
let changed: bool = AccountDraft::EMAIL.set(
    &form,
    "team@example.com".to_owned(),
    cx,
);
```

`AccountDraft::EMAIL` is `FieldDef<AccountDraft, String>`. It is a total path:
there is always exactly one `email` value in this form session. Total paths use
`get` and `set`, never `Result`; an equal `set` returns `false`.

## 2. Mark nested children and collections

Use `#[form(child)]` for a nested schema and `#[form(items)]` for a collection
of nested schemas. A required child is statically present, so composing through
it keeps a total path.

```rust,ignore
#[derive(Clone, FormSchema)]
struct AddressDraft {
    city: String,
}

#[derive(Clone, FormSchema)]
struct ProfileDraft {
    #[form(child)]
    address: AddressDraft,
}

let profile_form = cx.new(|_| Form::new(ProfileDraft {
    address: AddressDraft { city: String::new() },
}));
let city = ProfileDraft::ADDRESS.then(AddressDraft::CITY);
let value: String = city.get(&profile_form, cx);
city.set(&profile_form, "Shanghai".to_owned(), cx);
```

The derive creates `ChildDef<ProfileDraft, AddressDraft>` for `ADDRESS`. The
composed `city` path remains `TotalPath<ProfileDraft, String>` because it did
not cross a runtime-selected boundary.

For a collection, declare only business data. Do not add an ID field or a key
trait for Form navigation:

```rust,ignore
#[derive(Clone, FormSchema)]
struct RuleDraft {
    label: String,
}

#[derive(Clone, FormSchema)]
struct PolicyDraft {
    #[form(items)]
    rules: Vec<RuleDraft>,
}
```

The Form runtime creates the opaque occurrence identity for every active item.
Applications receive its typed `ItemPath` from `items`, `try_items`, or a
collection mutation; they do not reconstruct it from an array index, a business
ID, or a serialized token.

## 3. Resolve optional children and enum cases

An optional payload or an enum case is not always active. First build a typed
resolver, then resolve it against the explicit form. Both kinds return
`Result<Option<_>, ResolveError>`:

- `Ok(Some(path))` means the requested payload is active now;
- `Ok(None)` means the optional is `None` or the enum currently has another
  case; and
- `Err(_)` means the resolver's dynamic starting point belongs to another
  session or has retired.

```rust,ignore
#[derive(Clone, Default, FormSchema)]
struct CredentialsDraft {
    token: String,
}

#[derive(Clone, FormSchema)]
struct ConnectionDraft {
    #[form(child)]
    credentials: Option<CredentialsDraft>,
}

let connection_form = cx.new(|_| Form::new(ConnectionDraft {
    credentials: None,
}));
ConnectionDraft::CREDENTIALS.set(
    &connection_form,
    Some(CredentialsDraft::default()),
    cx,
);

let credentials = ConnectionDraft::CREDENTIALS
    .some()
    .resolve(&connection_form, cx)?;

if let Some(credentials) = credentials {
    let token = credentials.then(CredentialsDraft::TOKEN);
    let value: String = token.try_get(&connection_form, cx)?;
    token.try_set(&connection_form, "secret".to_owned(), cx)?;
}
```

Once a path crosses an item, optional, or case boundary it is dynamic. Use
`try_get` and `try_set`; their error reports that this particular runtime
location is no longer usable. It does not expose a session, token, or topology
implementation detail.

## 4. Build recursive typed trees

Recursive forms use the same Rust types as the business model. The collection
and resolver APIs carry the exact payload type through every level.

```rust,ignore
#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
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

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: String::new(),
            }),
        }],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);

for node in children.items(&query_form, cx) {
    let condition = node
        .then(FilterNode::KIND)
        .case(FilterNodeKind::CONDITION)
        .resolve(&query_form, cx)?;

    if let Some(condition) = condition {
        let value = condition.then(FilterCondition::VALUE);
        let current: String = value.try_get(&query_form, cx)?;
        value.try_set(&query_form, current.trim().to_owned(), cx)?;
    }
}
```

`FilterNodeKind::CONDITION` is a generated `CaseDef<FilterNodeKind,
FilterCondition>`. The resolver preserves that payload type, so writing a value
of the wrong Rust type is rejected at compile time. Calling `case(...).resolve`
on an inactive case returns `Ok(None)`, not a stale-path error.

The runtime gives every item, active case, and active optional payload a fresh
occurrence. Same-parent reordering preserves an item's occurrence. Removing
and re-inserting an item, changing away from and back to a case, or recreating
an optional payload creates a new occurrence; old dynamic paths stay retired.

## 5. Change collections with paths, not IDs

Use collection methods to create, order, remove, and replace items. They accept
and return typed item paths rather than indexes or application IDs.

```rust,ignore
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
let node = children.append(
    &query_form,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition {
            value: String::new(),
        }),
    },
    cx,
)?;

let first = children.items(&query_form, cx).into_iter().next();
if let Some(first) = first {
    children.move_before(&query_form, &node, &first, cx)?;
}

children.remove(&query_form, node, cx)?;
```

`append`, `insert_before`, `move_before`, `remove`, and `replace_all` form the
collection vocabulary. `ItemPath::move_to` performs an explicit cross-parent
move and returns the new destination path. A deleted or otherwise retired path
cannot be reused.

## 6. Add validation metadata and a validator

Leaf fields may use `#[form(required)]` and `#[form(validate(...))]`.
`validate` selects `on_mount`, `on_change`, `on_blur`, `on_external`, and
`on_submit`. `on_external` is for application-owned facts such as a refreshed
catalog. It is deliberately separate from the meaning of `DynamicPath`.

Without an explicitly selected non-submit trigger, business validation runs at
submit. A validator receives one consistent snapshot through
`ValidationRequest`; read the model with `request.model()` rather than taking a
second model parameter or rereading the live Form.

```rust,ignore
use gpui_form::{
    ValidationMessage, ValidationRequest, ValidationSink, Validator,
};

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_submit, on_external))]
    name: String,
}

struct ProviderValidator;

impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        if request.includes(&ProviderDraft::NAME)
            && request.model().name.trim().is_empty()
        {
            out.at(ProviderDraft::NAME).error(
                "provider-name-empty",
                ValidationMessage::key("provider-name-empty"),
            );
        }
    }
}

let form = cx.new(|_| {
    Form::new(ProviderDraft { name: String::new() })
        .with_validator(ProviderValidator)
});
```

The application may explicitly request `ValidationTrigger::External` after an
external dependency changes. Form stores validation facts; the page still owns
when and where to show them.

## 7. Prepare, save, and conditionally rebase

`prepare` runs submit validation and returns an accepted `Prepared<M>` value.
It contains an opaque `FormVersion` bound to this editing session. `map` keeps
that version when the application converts the draft into a request.

```rust,ignore
use gpui::{App, Entity};
use gpui_form::{Form, FormVersion, Prepared};

struct SaveProvider(ProviderDraft);

let prepared: Prepared<ProviderDraft> =
    form.update(cx, |form, cx| form.prepare(cx))?;
let request: Prepared<SaveProvider> = prepared.map(SaveProvider);
let version: FormVersion = request.version();

// Run application-owned I/O with `request.into_parts().1`.

fn apply_saved_provider(
    form: &Entity<Form<ProviderDraft>>,
    version: FormVersion,
    canonical: ProviderDraft,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| {
        form.rebase_if_current(version, canonical, cx)
    })
}
```

`rebase_if_current` returns `false` without changing the form if the user has
edited it since preparation, or if the version came from another session.

## 8. Expect declaration-site diagnostics

The derive diagnoses unsupported schema shapes at the declaration:

| Invalid declaration | Expected direction |
| --- | --- |
| generic struct or enum | schemas are monomorphic |
| tuple struct or union | only supported named struct and enum shapes expose definitions |
| `#[form(items)]` on a non-`Vec` field | collections use supported `Vec<Item>` items |
| item without `FormSchema` | structured items expose a schema |
| `#[form(identity)]` | removed; Form owns occurrence identity |
| struct-like or multi-payload enum variant | a variant is unit or has one concrete tuple payload |

The macro keeps unsupported shapes out of application runtime code. Runtime
resolution errors remain limited to dynamic paths that once referred to an
active item, case, or optional payload.
