# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` provides `#[derive(FormStore)]` for `gpui-form`. It generates
a typed form store, field identities and schema, typed field access, validation
traversal, and submit-preparation glue from an ordinary Rust model.

> **Implementation status:** this README documents the implemented public API.
> See the [implementation plan](dev/form-store-derive.md) for verification
> evidence.

## Usage

```rust,ignore
use gpui::AppContext as _;
use gpui_form::FormStore as _;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,
    retry_limit: u32,
}

let form = cx.new(|cx| {
    ProviderInputFormStore::from_value(
        ProviderInput {
            name: String::new(),
            retry_limit: 3,
        },
        cx,
    )
});

let name = ProviderInputFormStore::name_field(&form);
name.set("OpenAI".to_owned(), cx)?;

let output = form.update(cx, |form, cx| form.prepare_submit(cx))?;
assert_eq!(output.name, "OpenAI");
```

This generates `ProviderInputFormStore`, `ProviderInputField`, static field
schema and paths, and type-preserving accessors. `required` always participates
in submit validation; `validate(...)` adds earlier triggers.

The generated store contains one internal, doc-hidden
`FormRuntime<Model, ValidationContext>`; callers do not access that runtime
directly. Validation and submit behavior are selected by the
`FormStore::ValidationAdapter` and
`FormStore::SubmitTransform` associated types; both require `Default + 'static`
and are constructed only when validation or transformation runs, rather than
being stored as instances. Validation queries such as `validation_report()` and
`errors_at(path)` return owned snapshots.

The derive also supports Garde or custom validation adapters, submit
transforms, generic models, nested groups, and stable-ID arrays. Nested models
derive `FormStore` independently but do not create child form entities;
generated `*_in`, `*_item`, and `*_item_in` accessors remain typed lenses over
the one root form value.

Use `#[form(store = ValueEditorStore)]` when the generated store needs an
explicit name. This overrides only the store name; the field enum remains named
after the model (`ModelField`).

The macro does not generate UI controls, component configuration, raw drafts,
codecs, focus/touched/blurred state, submit tasks, busy flags, retry policy, or
persistence. Controls belong to adapter crates, while request lifecycle and
persistence belong to the application.

## Documentation

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
