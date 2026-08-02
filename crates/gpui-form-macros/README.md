# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` provides `#[derive(FormModel)]` for `gpui-form`. It
generates one GPUI form state type, reusable typed field descriptors, schema
metadata, validation traversal, and submit-preparation glue from an ordinary
Rust model.

## Example

```rust,ignore
use gpui::AppContext as _;
use gpui_form::FormState as _;

#[derive(Clone, Debug, PartialEq, gpui_form::FormModel)]
#[form(state = ProviderForm)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,
    retry_limit: u32,
}

let form = cx.new(|cx| {
    ProviderForm::from_value(
        ProviderInput {
            name: String::new(),
            retry_limit: 3,
        },
        cx,
    )
});

let name = ProviderForm::NAME;
name.set(&form, "OpenAI".to_owned(), cx);

let prepared = form.update(cx, |form, cx| form.prepare_submit(cx))?;
assert_eq!(prepared.output.name, "OpenAI");
```

By default `Model` generates `ModelForm`; `#[form(state = ProviderForm)]`
overrides that state type name. The generated type implements `FormState` and
owns exactly one internal runtime with the current model, baseline, revision,
validation context, and validation state.

For every statically declared field, the derive generates one schema-level
associated constant in `SCREAMING_SNAKE_CASE`, such as `ProviderForm::NAME`.
Each constant is one allocation-free schema definition and exposes a lightweight
`FormField<ProviderForm, T>` typed lens backed only by static schema and access
functions. It neither owns nor weakly references an `Entity<ProviderForm>`, and
accessing it creates no per-form field state or subscription and performs no
allocation.
Callers pass the `&Entity<ProviderForm>` explicitly to every data operation.

The macro does not expose a `ProviderInputField` enum or `FormFieldId` API.
Schema belongs to the descriptor:

```rust,ignore
let name = ProviderForm::NAME;
assert!(name.schema().is_required());
let errors = name.errors(&form, cx);
```

## Total and partial descriptors

Root fields and ordinary group projections are `FormField<Form, T>` (a total
descriptor). Their `value`, `set`, validation, and error APIs are infallible:
their path is statically present whenever the supplied form entity exists.

An identified array item and a computed `project_value` are
`PartialFormField<Form, T>`. They use explicit `try_*` APIs because a stable-ID
item may have been removed or a computed projection may be unavailable:

```rust,ignore
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
username.set(&form, "alice".into(), cx);

let header = ServerForm::HEADERS.item(row_id);
let header_name = HeaderRowForm::NAME.within(header);
let row = header_name.try_value(&form, cx)?;
```

Availability propagates through composition: `within` keeps its parent's
availability; `HEADERS.item(id)` and `project_value(...)` turn it partial; every
descendant of a partial descriptor remains partial. The macro keeps descriptor
construction private, so every public descriptor carries a generated path and
schema contract.

## Validation and submit

The derive supports Garde or application-defined validation adapters, generic
models, nested groups, and stable-ID arrays. A validation adapter is selected
as a type-level associated policy; it is not a stored value and does not require
`Default`. Runtime dependencies belong in the typed validation context or in
application-owned state.

`SubmitTransform` is a static, pure, infallible transformation from the
validated model to the application output. `prepare_submit` returns a
`PreparedSubmit<Output>` containing both that output and the form revision that
produced it. Persistence, request tasks, retry, and conditional rebase remain
application responsibilities.

Nested models also derive `FormModel`, but never create child form entities.
`within` composes a child lens over a parent lens from the same root form;
`item(id)` creates a `PartialFormField`, and every descendant remains partial.
`project_value` is likewise partial. `FormField` constructors remain
core-private. The macro does not generate controls,
component configuration, raw drafts, codecs, focus/touched/blurred state,
persistence, or operation lifecycle.

## Documentation

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
