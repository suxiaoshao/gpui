# `FormSchema` derive guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

This guide teaches the confirmed greenfield derive design from a model
declaration to a prepared submission. It is intentionally task-oriented:
derive static definitions first, then use those definitions against one
`Form<M>` session.

Applications normally consume the derive through `gpui-form`:

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## 1. Derive a flat `FormSchema`

Begin with a model that has only leaf fields. `FormSchema` gives each field a
typed root definition.

```rust,ignore
use gpui::Entity;
use gpui_form::{Form, FormSchema};

#[derive(Clone, FormSchema)]
struct AccountDraft {
    email: String,
    max_projects: u32,
}

let runtime = Form::try_new(AccountDraft {
    email: "owner@example.com".to_owned(),
    max_projects: 5,
})?;
let form: Entity<Form<AccountDraft>> = cx.new(|_| runtime);

let email = AccountDraft::EMAIL;
let current = email.value(&form, cx);
email.set(&form, "team@example.com".to_owned(), cx);
```

`AccountDraft::EMAIL` is a `FieldDef<AccountDraft, String>`. A root field is a
total path: it has no item, optional-value, or case boundary to resolve.

## 2. Helper attribute grammar

| Attribute | Accepted target shape | Derive output |
| --- | --- | --- |
| `#[form(child)]` | a field whose type implements `FormSchema`, including `Option<Child>` | `ChildDef<Parent, Child>` |
| `#[form(items)]` | `Vec<Item>`, where `Item` implements `FormSchema` | `ItemsDef<Parent, Item>` |
| `#[form(required)]` | a leaf field | required metadata |
| `#[form(validate(...))]` | a leaf field | mount/change/blur/dynamic/submit trigger metadata |

`child` and `items` are about shape. Item identity is not macro input: Form
generates an opaque session-local identity for each item occurrence and returns
it only through a typed `ItemPath`. A model does not implement a key trait or
declare an ID field for form navigation.

## 3. Compose a total static child path

A required nested child remains total. `then` joins the parent-child edge with
one of the child schema's definitions, so the result can read and write without
runtime selection.

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

let runtime = Form::try_new(ProfileDraft {
    address: AddressDraft { city: String::new() },
})?;
let form: Entity<Form<ProfileDraft>> = cx.new(|_| runtime);

let city = ProfileDraft::ADDRESS.then(AddressDraft::CITY);
let current = city.value(&form, cx);
city.set(&form, "Shanghai".to_owned(), cx);
```

The composed type is a `TotalPath<ProfileDraft, String>`: `ADDRESS` always
exists, so `CITY` always has one target below it.

## 4. Enter an optional child with `try_some`

An optional child has no target while its value is `None`. Set the total option
field when the application wants to create the child; `try_some(&Form)` locates
the payload in the current session and therefore yields a dynamic path.

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

let runtime = Form::try_new(ConnectionDraft { credentials: None })?;
let form: Entity<Form<ConnectionDraft>> = cx.new(|_| runtime);

ConnectionDraft::CREDENTIALS.set(
    &form,
    Some(CredentialsDraft::default()),
    cx,
);

let token: DynamicPath<ConnectionDraft, String> = ConnectionDraft::CREDENTIALS
    .try_some(form.read(cx))?
    .then(CredentialsDraft::TOKEN);

let current = token.try_value(&form, cx)?;
token.try_set(&form, "secret".to_owned(), cx)?;
```

The detailed option-replacement error fields remain API-design work. The
resolver contract is fixed: `try_some` explicitly receives the current
`&Form<ConnectionDraft>`, captures the active `Some` incarnation without
storing the form entity, and returns a dynamic path. Once a path crosses this
boundary, it and every descendant remain dynamic.

## 5. Model recursive items and enum cases

This self-contained example has a recursive group and an enum whose concrete
payload is selected with `case`. The item model contains no form-only ID.

```rust,ignore
use gpui::Entity;
use gpui_form::{DynamicPath, Form, FormSchema};

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
}

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
let form: Entity<Form<QueryDraft>> = cx.new(|_| runtime);

let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let group_node = children
    .items(&form, cx)?
    .into_iter()
    .next()
    .expect("the example contains one root node");
let nested_children = group_node
    .then(FilterNode::KIND)
    .try_case(form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);
let condition_node = nested_children
    .try_items(&form, cx)?
    .into_iter()
    .next()
    .expect("the example contains one nested node");

let value: DynamicPath<QueryDraft, String> = condition_node
    .then(FilterNode::KIND)
    .try_case(form.read(cx), FilterNodeKind::CONDITION)?
    .then(FilterCondition::VALUE);

let current = value.try_value(&form, cx)?;
value.try_set(&form, "Rust".to_owned(), cx)?;
```

Form generates and owns the opaque identity for each item occurrence. `items`
and `try_items` return typed `ItemPath` values selected from the current runtime
topology; the caller cannot construct or recover one from a model value. An item
path is already dynamic; `try_case(&Form, CaseDef)` resolves the active case and
captures its current incarnation. A retired item or inactive case is a
recoverable path-resolution error, not a silent fallback.

Supported enum variants are unit variants and variants with one concrete tuple
payload that implements `FormSchema`. Generic schema models, struct-like
variants, and variants with multiple payload fields are compile-time errors.

## 6. Use generated definitions as building blocks

The derive exposes static definitions; applications compose them rather than
constructing field names or paths from strings.

```rust,ignore
ProfileDraft::DISPLAY_NAME; // FieldDef<ProfileDraft, String>
ProfileDraft::ADDRESS;      // ChildDef<ProfileDraft, AddressDraft>
FilterGroup::CHILDREN;      // ItemsDef<FilterGroup, FilterNode>
FilterNodeKind::GROUP;      // CaseDef<FilterNodeKind, FilterGroup>
```

The important generated families are `FieldDef`, `ChildDef`, `ItemsDef`, and
`CaseDef`. The runtime, not the derive output, creates typed `ItemPath` values.
Concrete module paths and non-resolver helper details remain subject to the
runtime API decision. The resolver shape is fixed: paths call
`try_case(&Form, CaseDef)` or `try_some(&Form)`; neither generated definitions
nor located paths retain a form entity.

## 7. Change items through topology methods

Collections are a topology boundary, not a mutable `Vec` borrowed from the
model. The form session owns item creation, deletion, ordering, opaque identity,
and movement so it can preserve revision tracking and validation scope.

```rust,ignore
use gpui_form::Position;

let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let anchor = children.items(&form, cx)?.into_iter().next().unwrap();
let appended = children.append(
    &form,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
let inserted = children.insert_before(
    &form,
    &anchor,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
children.move_before(&form, &appended, &anchor, cx)?;
children.remove(&form, inserted, cx)?;

let fresh = children.replace_all(
    &form,
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
```

Moving between different parents is explicit because it changes ownership and
retires the source path:

```rust,ignore
let parent = fresh[0].clone();
let source = fresh[1].clone();
let destination_children = parent
    .then(FilterNode::KIND)
    .try_case(form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);

let moved = source.move_to(
    &form,
    destination_children,
    Position::End,
    cx,
)?;
```

`items`, `append`, `insert_before`, `move_before`, `remove`, and `replace_all`
are the topology vocabulary. They exchange typed item paths, never raw
IDs. The runtime rejects stale or wrong-session source, destination, and anchor
paths, as well as an attempted move cycle. `TopologyIndex` is private runtime
state: callers pass neither it nor a topology snapshot to these APIs.

## 8. Close the session with a validator, prepare, and rebase

Validation belongs to the `Form<M>` session, not to a separate model state. A
validator receives the requested scope and reports into a sink; it can validate
a leaf, a subtree, or the whole model as the runtime defines.

```rust,ignore
use gpui::{App, Entity};
use gpui_form::{
    Form, FormRevision, ValidationMessage, ValidationRequest, ValidationSink,
    Validator,
};

#[derive(Clone)]
struct QueryContext {
    allow_empty: bool,
}

struct QueryValidator {
    context: QueryContext,
}

impl Validator<QueryDraft> for QueryValidator {
    fn validate(
        &self,
        model: &QueryDraft,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<QueryDraft>,
    ) {
        let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
        if request.includes(&children)
            && !self.context.allow_empty
            && model.root.children.is_empty()
        {
            out.at(children).error(
                "query-empty",
                ValidationMessage::key("query-empty"),
            );
        }
    }
}

struct SaveQuery(QueryDraft);

impl From<QueryDraft> for SaveQuery {
    fn from(query: QueryDraft) -> Self {
        Self(query)
    }
}

let context = QueryContext { allow_empty: false };
let initial_query = QueryDraft {
    root: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: "Rust".to_owned(),
            }),
        }],
    },
};
let runtime = Form::try_new_with_validator(initial_query, QueryValidator { context })?;
let form: Entity<Form<QueryDraft>> = cx.new(|_| runtime);

let prepared = form.update(cx, |form, cx| form.prepare(cx))?;
let revision: FormRevision = prepared.revision();
let request = prepared.map(SaveQuery::from);
// Give `(revision, request)` to the application's persistence task.

fn apply_saved_query(
    form: &Entity<Form<QueryDraft>>,
    revision: FormRevision,
    canonical_saved_model: QueryDraft,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| {
        form.rebase_if_revision(revision, canonical_saved_model, cx)
    })
}
```

`prepare` validates and freezes an accepted snapshot. `map` consumes that
snapshot into a request. After persistence, a conditional rebase prevents an
older response from overwriting edits made while the request was in flight.
`Validator`, `ValidationRequest`, `ValidationSink`, and the error types are
provided by the runtime crate.

## 9. Expect diagnostics at the declaration site

The macro should make invalid schemas obvious where they are declared:

| Invalid declaration | Expected diagnostic direction |
| --- | --- |
| generic struct or enum | schemas must be monomorphic |
| tuple struct or union | only supported struct/enum shapes can expose named definitions |
| `#[form(items)]` on a non-`Vec` field | item collections use the supported `Vec<Item>` shape |
| `#[form(items)]` item without `FormSchema` | structured items must expose a schema |
| removed `#[form(identity)]` attribute | Form owns item identity; remove the attribute and keep the field only if it is business data |
| struct-like or multi-payload enum variant | a case must be unit or one concrete tuple payload |

Diagnostics should point to the offending attribute, field, or variant and say
which supported model shape is expected. This keeps recursive runtime failures
out of application code.
