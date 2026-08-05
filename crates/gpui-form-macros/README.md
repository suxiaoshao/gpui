# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` provides `#[derive(FormSchema)]`: the compile-time half of
the greenfield `gpui-form` API. Derive it on the model you edit, then create a
single `Form<M>` session that owns the draft, topology, validation, and commit
boundary.

Applications normally depend on `gpui-form`, which re-exports `FormSchema`:

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## Start with a flat model

A flat struct produces total root fields. Create one form session, retain it as
`Entity<Form<M>>`, and use the generated root definition to read or update a
field.

```rust,ignore
use gpui::{App, Entity};
use gpui_form::{Form, FormRevision, FormSchema, Prepared};

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    name: String,
    retry_limit: u32,
}

struct SaveProvider {
    name: String,
    retry_limit: u32,
}

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self { name: draft.name, retry_limit: draft.retry_limit }
    }
}

let runtime = Form::try_new(ProviderDraft {
    name: "primary".to_owned(),
    retry_limit: 3,
})?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| runtime);

let name = ProviderDraft::NAME;
let current: String = name.value(&form, cx);
name.set(&form, "backup".to_owned(), cx);

let prepared: Prepared<ProviderDraft> =
    form.update(cx, |form, cx| form.prepare(cx))?;
let revision: FormRevision = prepared.revision();
let request = prepared.map(SaveProvider::from);
// Give `(revision, request)` to the application's persistence task.

fn apply_saved_provider(
    form: &Entity<Form<ProviderDraft>>,
    revision: FormRevision,
    saved_provider: ProviderDraft,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| {
        form.rebase_if_revision(revision, saved_provider, cx)
    })
}
```

`ProviderDraft::NAME` is a generated `FieldDef<ProviderDraft, String>`. Since
there is no topology boundary between the root and `name`, its path is total:
`value` and `set` can operate directly on the form session.

`prepare` is the explicit handoff from an editable session to a submit-ready
snapshot. It runs the session validation policy and captures a revision; `map`
then transforms that accepted snapshot without reopening mutation. Capture the
revision before `map`; after persistence, apply the canonical `ProviderDraft`
only through `rebase_if_revision` so an older response cannot overwrite newer
edits.

## Add structure with attributes

`FormSchema` derives static definitions from Rust fields and variants:

| Attribute | Intended model shape | Generated definition |
| --- | --- | --- |
| `#[form(child)]` | a nested schema, optionally `Option<Child>` | `ChildDef` |
| `#[form(items)]` | `Vec<Item>` whose item has a form schema | `ItemsDef` |

Definitions compose from root to leaf. For example, an item property below a
collection starts from an `ItemPath` returned by the Form runtime, then composes
with `item_path.then(...)`. Crossing an item, optional value, or enum case makes
the resulting path dynamic; it must be read or written with `try_value` /
`try_set`. Models never declare or store a form-only item ID.

The full tutorial covers total child paths, `try_some(&Form)`, recursive arrays,
`try_case(&Form, CaseDef)`, topology mutations, validators, and commit/rebase
handling:

- [English guide](docs/guide.md)
- [中文指南](docs/guide.zh-CN.md)

## Generated names are the contract

The derive expands schema metadata rather than an editing runtime. The runtime
crate supplies `Form<M>`, paths, validators, topology operations, and prepared
snapshots. The macro supplies typed, static entry points such as:

```rust,ignore
ProfileDraft::DISPLAY_NAME; // FieldDef<ProfileDraft, String>
ProfileDraft::ADDRESS;      // ChildDef<ProfileDraft, AddressDraft>
ProfileDraft::RULES;        // ItemsDef<ProfileDraft, RuleDraft>
ModeDraft::REMOTE;          // CaseDef<ModeDraft, RemoteDraft>
```

The definition types are re-exported by `gpui-form`; the names above are the
derive output contract.

## Validation attributes

`child` and `items` describe structure. Leaf fields also accept validation
metadata:

| Attribute | Meaning |
| --- | --- |
| `#[form(required)]` | mark a field as required; without an explicit trigger list it enables mount/change/blur/submit |
| `#[form(validate(...))]` | enable any of `on_mount`, `on_change`, `on_blur`, `on_dynamic`, and `on_submit` |

Required values use `RequiredValue`; strings are trimmed, options and supported
collections must be nonempty, and booleans must be true.

## Compile-time diagnostics

The derive should fail close to the model declaration for unsupported schema
shapes: generic schema types, tuple structs, unions, struct-like enum variants,
and enum variants with more than one payload field. An `items` field must be a
supported collection whose item exposes the required form schema. The removed
`#[form(identity)]` attribute is an error: item identity is generated and owned
by the Form runtime, not by a model field.

See the guide for diagnostics and the supported recursive shape.
