# gpui-form-macros user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form-macros` provides `#[derive(FormModel)]`. It turns a typed Rust
model into one root form state type and a family of reusable field descriptors.
The generated state implements `gpui_form::FormState`; descriptors are pure
lenses and never contain a strong or weak GPUI entity handle.

## Derive a form model

```rust,ignore
#[derive(Clone, Debug, PartialEq, gpui_form::FormModel)]
#[form(state = ServerForm)]
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

- `ServerForm`, the root GPUI entity state implementing `FormState`;
- one schema-level `SCREAMING_SNAKE_CASE` associated const per statically
  declared field, such as `ServerForm::NAME`, `ServerForm::AUTH`, and
  `ServerForm::HEADERS`;
- validation, schema traversal, revision, and submit-preparation glue.

Each associated const is one allocation-free, reusable schema definition and a
`FormField<ServerForm, T>` typed lens backed only by static schema and access
functions. It has no form instance, `Entity`, `WeakEntity`, value, subscription,
or per-form allocation. Accessing it creates no per-form field state.
Composition may create a lightweight located descriptor, but it never creates
another field or schema definition.

It does **not** expose a `ServerInputField` enum or `FormFieldId`. Static schema
is obtained from the descriptor that actually identifies the field:

```rust,ignore
let name = ServerForm::NAME;
assert!(name.schema().is_required());
assert_eq!(name.path(), FieldPath::field("name"));
```

The macro never creates child form entities. A nested model has its own
descriptor namespace, but all descriptors read and write the one root entity.

## State name and generics

By default, `Model` generates `ModelForm`. Use `state = ...` when a more
specific state name reads better at call sites:

```rust,ignore
#[derive(Clone, PartialEq, gpui_form::FormModel)]
#[form(state = GenericValueForm)]
struct ValueEditor<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
}
```

This generates `GenericValueForm<T>`. The generated declaration and its
implementations preserve the model's lifetimes, type parameters, const generics,
legal defaults, and `where` clause. Implementations omit type defaults where
Rust requires them.

## Type attributes and canonical grammar

A model has at most one `#[form(...)]` helper attribute. Each option appears at
most once and options are comma-separated in any order:

```text
state = StateIdent
validation(adapter = "garde"[, messages = ProviderType])
validation(adapter = CustomValidatorType[, context = ContextType])
transform(adapter = "validify")
transform(adapter = CustomTransformType)
```

`StateIdent` is an unquoted identifier. Custom adapters, contexts, and Garde
message providers are unquoted Rust type paths. Only the built-in adapter names
`"garde"` and `"validify"` are string literals. Quoted custom types, unknown
built-in names, duplicate options, empty clauses, and a second helper attribute
are compile errors.

Select built-in validation and transformation policy like this:

```rust,ignore
#[form(
    validation(adapter = "garde", messages = AppGardeMessageProvider),
    transform(adapter = "validify")
)]
```

For Garde, the validation context is always
`<Model as garde::Validate>::Context` and is declared on the Garde model:

```rust,ignore
#[derive(gpui_form::FormModel, garde::Validate)]
#[garde(context(ServerValidationContext))]
#[form(validation(adapter = "garde", messages = AppGardeMessageProvider))]
struct ServerInput {
    // ...
}
```

Application-defined policies use their type names directly:

```rust,ignore
#[form(
    validation(adapter = ServerValidator, context = ServerValidationContext),
    transform(adapter = ServerTransform)
)]
```

Validation is a type-level policy selected by the generated `FormState`
implementation. It is neither retained as a state field nor constructed through
`Default`; runtime dependencies belong in the typed validation context or in
application state.

The combinations remain strict:

| Adapter | `context` | `messages` |
| --- | --- | --- |
| no validation adapter | forbidden | forbidden |
| `"garde"` | forbidden; Garde owns `Validate::Context` | optional |
| custom validation type | optional; otherwise use its associated context | forbidden |

## Field attributes

| Attribute | Purpose |
| --- | --- |
| `required` | built-in submit-time required rule and static schema metadata |
| `validate(on_mount, ...)` | validation triggers for this exact schema path |
| `group` | nested typed form model |
| `array(id = "row_id")` | typed `Vec<T>` with caller-owned stable item IDs |

Each field has at most one `#[form(...)]` helper attribute. `required` and
`group` are bare flags. `validate(...)` contains one or more unique triggers.
`array` accepts exactly one string-literal ID field name and requires `Vec<T>`.
`group` and `array` are mutually exclusive.

Supported triggers are `on_mount`, `on_change`, `on_blur`, `on_dynamic`, and
`on_submit`. `on_mount` runs once after the generated state has installed its
initial value and validation context.

Attributes describe model data and validation rules. Component type, options,
layout, focus, persistence, and operation lifecycle remain outside the derive.

## Create and use the state

Create one `Entity<State>` for one editing session:

```rust,ignore
use gpui::AppContext as _;
use gpui_form::FormState as _;

let form = cx.new(|cx| {
    ServerForm::from_value(
        ServerInput {
            name: String::new(),
            auth: AuthInput::default(),
            headers: Vec::new(),
        },
        cx,
    )
});

let name = ServerForm::NAME;

let current: String = name.value(&form, cx);
name.set(&form, "api.example.com".into(), cx);

let errors = name.errors(&form, cx);
let validating = name.is_validating(&form, cx);
```

`from_value` is available when the validation context implements `Default`.
Use `from_value_with_validation_context(value, context, cx)` when validation
needs application-owned dependencies.

For a total descriptor, these APIs do not return `Result`. The entity is strong
and the generated root path is statically known to exist. A same-value `set` is
a no-op; a changed value advances the revision once, performs the applicable
validation work, emits one event, and notifies once.

`FormField` constructors are core-private. Applications use generated
schema-level constants and composition rather than making paths or model
projections by hand.

## Availability through groups and arrays

`FormField<Form, T>` is a total descriptor. It represents a declared root path
or a projection whose parent is total. `PartialFormField<Form, T>` represents a
path that can cease to resolve against a live model.

Groups preserve their parent's availability through `within`:

```rust,ignore
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
username.set(&form, "alice".into(), cx);
```

Identified items become partial because the item may be removed between creating
the descriptor and using it:

```rust,ignore
let header = ServerForm::HEADERS.item(gpui_form::FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(header);

let value = header_name.try_value(&form, cx)?;
header_name.try_set(&form, "Authorization".into(), cx)?;
```

`project_value(...)` is also partial. It may describe an optional or computed
projection, so it and every derived child expose `try_*` methods. The
corresponding errors describe only a real unresolved projection or missing/
ambiguous stable item; there is no synchronous `FormReleased` error.

The complete generated traversal vocabulary is:

```rust,ignore
RootForm::FIELD;
ChildForm::FIELD.within(parent);
RootForm::ITEMS;
RootForm::ITEMS.item(id);
```

Whole-array writes remain total and are the explicit API for adding, removing,
reordering, and replacing items. Stable IDs are unique within the current array
and cannot change through an identified-item descriptor.

## Validation, schema, and submit

The generated state owns current value, baseline, monotonic revision,
validation context, reports, and validation task bookkeeping. Descriptors only
identify typed paths; the core performs a successful write transaction and
notifies exactly once.

Nested schema ownership is exact: a group owns its direct path, an array owns
its direct path and item root, and a child model owns its descendants. Garde
models can opt into recursion with `#[garde(dive)]` on a `group` or `array`;
the derive maps external vector indexes to stable item paths.

`SubmitTransform` is static, pure, and infallible. It converts a validated
model to the application output without an adapter instance, I/O, or mutable
form state:

```rust,ignore
struct SaveServer {
    name: String,
}

struct ServerTransform;

impl gpui_form::SubmitTransform<ServerInput> for ServerTransform {
    type Output = SaveServer;

    fn transform(model: &ServerInput) -> Self::Output {
        SaveServer {
            name: model.name.trim().to_owned(),
        }
    }
}
```

`prepare_submit` first runs synchronous submit validation, rejects blocking
issues or pending validation, applies the transform once, and returns:

```rust,ignore
let prepared = form.update(cx, |form, cx| form.prepare_submit(cx))?;
let gpui_form::PreparedSubmit { revision, output } = prepared;
self.start_save(revision, output, cx);
```

The page or controller owns persistence and uses `rebase_if_revision` with the
returned revision after a successful save. The form has no busy flag, save task,
retry state, persistence callback, or operation runtime.

## Compile-time diagnostics

The derive reports unsupported attributes, invalid validation triggers,
incorrect group types, `array` on non-`Vec` fields, missing stable IDs,
unresolved adapter types, custom context on Garde, and a Garde message provider on another
adapter at compile time.

It also rejects duplicate helper attributes/options, quoted custom types, empty
clauses, and every removed draft/component/focus option. `store = ...` is a
removed type option; its diagnostic directs the caller to `state = ...`.
`FormStore` is a removed derive name; diagnostics direct the caller to
`FormModel`. Invalid configuration is never overwritten, ignored, or accepted
through a compatibility alias.

## Related documentation

- [gpui-form user guide](../../gpui-form/docs/guide.md)
- [gpui-form-gpui-component user guide](../../gpui-form-gpui-component/docs/guide.md)
