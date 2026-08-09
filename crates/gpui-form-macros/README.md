# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` provides `#[derive(FormSchema)]`, the schema declaration
layer of `gpui-form`. Derive it for an editable Rust model; the runtime crate
then creates one `Entity<Form<M>>` editing session for that model.

Applications normally depend on `gpui-form`, which re-exports the derive:

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## Static descriptors, explicit form ownership

The derive creates one static, typed descriptor per field. A descriptor is
schema data only: it never owns a `Form`, a weak form reference, a value, or a
control. Pass the current strong `Entity<Form<M>>` explicitly whenever a path
is read or changed.

```rust,ignore
use gpui::Entity;
use gpui_form::{Form, FormSchema};

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    name: String,
    retry_limit: u32,
}

let form: Entity<Form<ProviderDraft>> = cx.new(|_| {
    Form::new(ProviderDraft {
        name: "primary".to_owned(),
        retry_limit: 3,
    })
});

let name: String = ProviderDraft::NAME.get(&form, cx);
let changed: bool = ProviderDraft::NAME.set(&form, "backup".to_owned(), cx);
```

`ProviderDraft::NAME` is a `FieldDef<ProviderDraft, String>`, which is also a
total path. A total path has no runtime-dependent boundary, so `get` and `set`
are infallible. `set` returns whether the model actually changed.

## Describe nested shapes

Use attributes only where a field introduces structure:

| Attribute | Model shape | Generated descriptor |
| --- | --- | --- |
| `#[form(child)]` | nested `FormSchema`, including `Option<Child>` | `ChildDef` |
| `#[form(items)]` | `Vec<Item>` where `Item: FormSchema` | `ItemsDef` |
| `#[form(required)]` | leaf field | required validation metadata |
| `#[form(validate(...))]` | leaf field | validation trigger metadata |

Required children compose into total paths with `then`. Collection items,
active enum cases, and active optional children produce dynamic paths. Dynamic
paths use `try_get` and `try_set`, because the runtime location can retire.

```rust,ignore
let city = ProfileDraft::ADDRESS.then(AddressDraft::CITY);
let city: String = city.get(&form, cx);
```

The full guide covers optional and enum resolvers, recursive collections,
validation, and submission:

- [English guide](docs/guide.md)
- [中文指南](docs/guide.zh-CN.md)

## Runtime-owned item identity

Models do not carry form navigation IDs and do not implement an identity trait
for this derive. The Form session creates an opaque occurrence whenever an item,
enum case, or optional payload becomes active. Applications obtain typed item
paths only by enumerating a collection or by using collection mutation methods.

This makes recursive typed trees possible without string paths or application
counters. Reordering an item in the same parent preserves its identity; removal
and re-insertion, reactivating a case or optional payload, and cross-parent
movement create a fresh occurrence. A retired dynamic path reports a resolution
error instead of silently targeting a later value.

## Validation and accepted snapshots

`#[form(validate(...))]` may opt a field into `on_mount`, `on_change`,
`on_blur`, `on_external`, or `on_submit`. `on_external` is for a changed
catalog or other application-owned dependency; it is not related to a dynamic
path. Unless a trigger is declared, normal business validation runs at submit.

Validators receive one snapshot-bound request and read its model through
`request.model()`. A prepared submission carries an opaque, session-bound
`FormVersion`. After application-owned I/O completes, use that version with
`rebase_if_current`; an old response then cannot overwrite newer edits or a
different form session.

## Compile-time diagnostics

The derive rejects unsupported declarations near the model definition:

- generic structs or enums, tuple structs, and unions;
- `#[form(items)]` on anything other than a supported `Vec<Item>` schema;
- struct-like enum variants or variants with multiple payload fields; and
- the removed `#[form(identity)]` attribute.

Supported enum variants are unit variants or variants with one concrete tuple
payload implementing `FormSchema`. See the guide for complete examples.
