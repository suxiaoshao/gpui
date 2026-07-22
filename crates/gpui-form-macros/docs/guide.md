# gpui-form-macros user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form-macros` provides the `#[derive(FormStore)]` entry point used by
`gpui-form` applications.

> **Implementation status:** this guide documents the implemented public
> contract.

## Derive a store

```rust,ignore
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
struct ServerInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,

    #[form(group)]
    auth: AuthInput,

    #[form(array(id = "row_id"))]
    headers: Vec<HeaderRowInput>,
}
```

For `ServerInput`, the macro generates:

- `ServerInputFormStore`;
- `ServerInputField` and static field schema;
- typed field access such as `ServerInputFormStore::name_field(&form)`;
- validation and submit traversal;
- nested group and stable-ID array paths.

## Store name and generics

By default, `Model` generates `ModelFormStore` and `ModelField`. Override the
store name when a domain-specific public name is clearer:

```rust,ignore
#[derive(Clone, PartialEq, gpui_form::FormStore)]
#[form(store = GenericValueStore)]
struct ValueEditor<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
}
```

This generates `GenericValueStore<T>` and `ValueEditorField`. `store = ...`
overrides only the store name; the field enum is always derived from the model
name. The generated store declaration and implementations preserve the model's
lifetimes, type parameters, const generics, defaults where legal, and `where`
clause. Implementations omit type defaults as required by Rust.

## Type attributes and canonical grammar

The rewrite is intentionally breaking. A model may have at most one
`#[form(...)]` helper attribute, and every option may appear at most once. Type
options are comma-separated and may appear in any order:

```text
store = StoreIdent
validation(adapter = "garde"[, i18n = ProviderType])
validation(adapter = CustomValidatorType[, context = ContextType])
transform(adapter = "validify")
transform(adapter = CustomTransformType)
```

`StoreIdent` is an unquoted identifier. Custom adapters, contexts, and I18n
providers are unquoted Rust type paths. Only the built-in adapter names
`"garde"` and `"validify"` are string literals. Quoted custom types, unknown
built-in names, duplicate options, empty clauses, and a second helper attribute
are compile errors rather than compatibility aliases.

Select built-in validation and transform adapters like this:

```rust,ignore
#[form(
    validation(adapter = "garde", i18n = AppGardeI18nProvider),
    transform(adapter = "validify")
)]
```

For Garde, the validation context is always
`<Model as garde::Validate>::Context`. Declare it with Garde itself:

```rust,ignore
#[derive(gpui_form::FormStore, garde::Validate)]
#[garde(context(ServerValidationContext))]
#[form(validation(
    adapter = "garde",
    i18n = AppGardeI18nProvider
))]
struct ServerInput {
    // ...
}
```

The macro selects
`GardeAdapter<ServerInput, AppGardeI18nProvider>`. Omitting `i18n` selects
`DefaultGardeI18nProvider`. The macro does not generate translations, Fluent
keys, Garde rules, or locale observers.

Select application-defined adapter types without quoting them:

```rust,ignore
#[form(
    validation(
        adapter = ServerValidator,
        context = ServerValidationContext
    ),
    transform(adapter = ServerTransform)
)]
```

The combinations are strict:

| Adapter | `context` | `i18n` |
| --- | --- | --- |
| no validation adapter | forbidden | forbidden |
| `"garde"` | forbidden; Garde owns `Validate::Context` | optional |
| custom validation type | optional; otherwise use the adapter's associated context | forbidden |

The generated store implements the `FormStore` constructor contract:
`from_value_with_validation_context`, `validation_context`, and
`set_validation_context` are always available. `FormStore::from_value` is a
trait-provided method with the bound `Self::ValidationContext: Default`; the
derive does not conditionally generate a different inherent API.

Both constructors install the initial model and validation context before
running `on_mount` validation exactly once. `set_validation_context` only
replaces the context and notifies observers. The caller explicitly chooses a
trigger and scope when the new context requires validation.

Custom validation and transform adapter types implement `Default + 'static`.
They are selected through `FormStore` associated types and constructed with
`default()` only when validation or transformation runs; the generated store
does not retain either instance. Runtime dependencies belong in the validation
context, not in an adapter or transform value. A custom
`SubmitTransform<Model>` declares an associated `Output` and one `transform`
method. The generated `FormStore::Output` is
`<Transform as SubmitTransform<Model>>::Output`; identity and Validify
transforms use the model itself.

## Field attributes

| Attribute | Purpose |
| --- | --- |
| `required` | built-in submit-time required rule and static required schema; `validate(...)` adds earlier triggers |
| `validate(on_mount, ...)` | validation triggers for the field |
| `group` | nested typed form model |
| `array(id = "row_id")` | typed array with stable item paths |

Each field may have at most one `#[form(...)]` helper attribute. `required` and
`group` are bare flags; `required = true`, `group()`, and nested configuration
such as `group(store = ...)` are invalid. `validate(...)` contains one or more
unique triggers. `array` accepts exactly one string-literal field name through
`id = "..."`; a bare identifier, missing ID, additional option, or non-`Vec<T>`
field is invalid. `group` and `array` are mutually exclusive.

Validation triggers are `on_mount`, `on_change`, `on_blur`, `on_dynamic`, and
`on_submit`. `on_mount` runs once inside either generated constructor, after the
initial value and validation context are installed.

Attributes describe form data and rules. Component type, options, layout,
focus, and persistence are configured by the application or adapter crate.
Legacy field options such as `component`, `codec`, `binding`, `state`, `focus`,
`touched`, `blurred`, `show_error`, and nested `store` are rejected with a
targeted migration diagnostic. The derive never silently ignores an unknown or
removed option.

## Generated ownership and lifecycle boundary

The generated store has exactly one private, doc-hidden
`FormRuntime<Model, ValidationContext>` field. It is macro/core plumbing rather
than a caller-facing API. That runtime owns the current model, baseline,
monotonic revision, validation context, and validation state. The validation
adapter and submit transform are associated types, not stored instances. The
generated implementation delegates their lifecycle to the core `FormStore`
contract and emits typed field or runtime notifications.

It does **not** own or generate any of the following:

- raw or string drafts, codecs, or per-field copies of business values;
- component entities, options, configuration, subscriptions, or focus handles;
- touched, blurred, focused, or error-visibility flags;
- `SubmitRuntime`, request tasks, busy flags, submission-attempt counters,
  retry policy, or persistence calls.

`prepare_submit` is only the synchronous validation and transform boundary.
The page or application store owns the asynchronous request lifecycle and
conditionally rebases a saved value through the core revision API.

## Typed field access

Generated field access preserves the declared Rust type:

```rust,ignore
use gpui_form::{FormFieldId as _, FormStore as _};

let name = ServerInputFormStore::name_field(&form);
name.set("api.example.com".to_owned(), cx)?;

let path = ServerInputField::Name.path();
let required = ServerInputField::Name.schema().is_required();

let report: gpui_form::ValidationReport =
    form.read(cx).validation_report();
let errors: Vec<gpui_form::ValidationIssue> =
    form.read(cx).errors_at(&path);
```

No generated API converts integers or enums through a String draft.

Every non-equal typed write follows one core-owned transaction: project and
store the typed value while advancing the revision; clear only intersecting
required, structural, and generated synchronous field buckets; cancel and clear
intersecting asynchronous validation; preserve the adapter-wide form bucket and
all active control issues; run `on_change` for the field's validation path; emit
one typed form event; and notify observers once. When the projected value equals
the current field value, the whole transaction is a no-op. The macro supplies
pure field projections over a cloned `Model` candidate and recursive schema;
those projections cannot access the runtime, validation, `Context`, events, or
notification. It therefore cannot duplicate or prematurely invoke the
lifecycle inside a root or nested accessor. Validation query methods return
owned snapshots:
`validation_report() -> ValidationReport` and
`errors_at(path) -> Vec<ValidationIssue>`.

## Validation and submit generation

`required` always participates in submit validation, even when the field does
not list `on_submit`. Its declared `validate(...)` triggers only add earlier or
explicit validation times. Nested leaf rules are selected from the leaf schema,
not copied onto every ancestor group or array.

Synchronous adapter validation always reads the model snapshot owned by the
store. A Garde-backed model uses its associated context and the selected I18n
provider. The derive implements `GardePathMapper` so external vector indices
can be converted to generated stable paths. Custom adapters receive the same
model, trigger, scope, and typed validation context through the core trait.

The derive also implements recursive model schema resolution for every full
stable path. The core normalizes each adapter issue in the fixed order
`schema resolver -> scope -> exact owner trigger`; generated code does not use
root-prefix filtering or copy leaf triggers onto ancestors. A resolver failure
becomes a blocking internal issue before scope filtering, so an invalid adapter
path cannot be hidden by a narrow validation run.

`prepare_submit` uses one model snapshot and has a fixed order:

1. run submit validation, including required and structural checks;
2. reject validation issues or pending blocking async validation;
3. invoke the selected `SubmitTransform<Model>::transform` exactly once;
4. return the associated output without mutating the model or starting I/O.

For a custom output type:

```rust,ignore
#[derive(Default)]
struct ServerTransform;

struct SaveServer {
    name: String,
}

impl gpui_form::SubmitTransform<ServerInput> for ServerTransform {
    type Output = SaveServer;

    fn transform(
        &self,
        model: &ServerInput,
    ) -> Result<Self::Output, gpui_form::TransformReport> {
        Ok(SaveServer {
            name: model.name.trim().to_owned(),
        })
    }
}
```

There is no transform preview method or transform context. A transform failure
is returned to the caller and does not become validation state.

## Groups and arrays

`#[form(group)]` reuses the child model schema while keeping one top-level form
store. Derive `FormStore` for the child model as well. Its generated store type
acts as the namespace for `*_in(parent_field)` accessors; calling one does not
construct a child store entity:

```rust,ignore
let username = AuthInputFormStore::username_in(
    ServerInputFormStore::auth_field(&form),
);
```

`#[form(array(id = "row_id"))]` requires every item to expose a stable ID of
the declared field. Errors and bound fields use the stable ID rather than the
current vector index. The ID field type implements `ToFormItemId`; generated
item accessors accept `FormItemId`.

The parent store exposes `*_item(form, id)` for an identified array. Compose
that item handle with the child model's `*_in` accessor to reach a leaf:

```rust,ignore
let header_name = HeaderRowInputFormStore::name_in(
    ServerInputFormStore::headers_item(
        &form,
        gpui_form::FormItemId::new(row_id),
    ),
);
```

The resulting type is `FormField<ServerInputFormStore, String>`: it retains the
top-level store type and carries the nested stable-ID path. Repeating either
accessor creates only another cheap typed handle.

If the identified array belongs to a nested model, use its generated
`*_item_in(parent_field, id)` accessor. The complete traversal API is:

```rust,ignore
RootFormStore::field_field(&form);
ChildFormStore::field_in(parent_field);
RootFormStore::items_item(&form, item_id);
ChildFormStore::items_item_in(parent_field, item_id);
```

These are typed lenses over the root form. They do not allocate another form
store, own subscriptions, or copy business state.

Stable IDs are unique within the current array and immutable through an
identified-item handle. A missing or ambiguous item causes reads and writes
through its generated handle to return `FormFieldError::ValueUnavailable`;
accessors never choose the first duplicate. Replacing an identified item, or
writing its generated ID leaf, with a different or unconvertible ID returns
`FormFieldError::ItemIdentityChanged`. Either error is rejected on the cloned
candidate and is a complete no-op. Duplicate or unconvertible IDs become
blocking internal structural issues, and submit checks this invariant without
requiring `validate(...)` on the array. Whole-array writes remain available for
explicit add, remove, reorder, and replacement operations.

Stable identity is nominal rather than historical. During one form session,
the same `(array path, stable ID)` denotes the same logical item. The runtime
does not track retired IDs; preserving an ID in a whole-array write updates that
nominal item, while changing it is remove plus insert. Applications must assign
a new ID to a new logical item. Reordering preserves addressing, but the
whole-array write invalidates descendant synchronous issues and asynchronous
checks so the next validation run maps fresh issues to the current IDs.

Nested leaf validation is controlled by the leaf's generated schema. Ancestor
groups and arrays do not repeat those triggers, and a nested `required` rule
always propagates to submit. Exact ownership is recursive:

| Stable path shape | Generated schema owner |
| --- | --- |
| `auth` | the declared `auth` group field |
| `auth.username` | the child model's `username` field |
| `headers` | the declared `headers` array field |
| `headers[#id]` | the directly owning `headers` array field |
| `headers[#id].name` | the item model's `name` field |

The same rule applies to arrays inside groups, arrays inside identified items,
and deeper combinations. An item root has no synthetic schema and does not
grant the array ownership of item descendants. `validate(...)` on a group or
array therefore applies only to an issue attached to that exact parent path—or
for an array, its direct item root—not to a nested leaf.

Garde recursion is a separate opt-in. When Garde owns nested rules, mark the
group and array with `#[garde(dive)]`; the nested types also implement
`garde::Validate` with a compatible context:

```rust,ignore
#[derive(gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
struct ServerInput {
    #[form(group)]
    #[garde(dive)]
    auth: AuthInput,

    #[form(array(id = "row_id"))]
    #[garde(dive)]
    headers: Vec<HeaderRowInput>,
}
```

The derive implements `GardePathMapper` for all array shapes: container
(`headers`), direct item root (`headers[2]`), and item leaf
(`headers[2].name`), recursively through nested groups and arrays. Indexed
paths are bounds-checked against the validated model and converted to stable
item IDs. Mapping does not inspect schema triggers; the built-in Garde adapter
returns the complete mapped report, and the core separately performs exact
schema resolution, scope filtering, and trigger filtering. Unknown fields,
invalid indices, out-of-bounds indices, duplicate IDs, and invalid item IDs
return a typed `GardePathError`; the runtime turns the failure into a blocking
internal issue.

## Compile-time diagnostics

The derive reports unsupported attributes, invalid validation triggers,
incorrect group types, `array` on non-array fields, missing stable IDs,
unresolved adapter types, custom context on Garde, and Garde I18n on another
adapter at compile time. It also rejects duplicate helper attributes/options,
quoted custom types, empty clauses, noncanonical `required`/`group`/`array`
spellings, and every removed draft/component/focus option. Diagnostics point to
the offending option or value and name the canonical replacement; invalid
configuration is never overwritten or ignored.

## Related documentation

- [gpui-form user guide](../../gpui-form/docs/guide.md)
- [gpui-form-gpui-component user guide](../../gpui-form-gpui-component/docs/guide.md)
