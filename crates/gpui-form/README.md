# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form` is a typed draft, validation, and submit-preparation library for
GPUI applications. A generated form state owns one editable Rust model. A
field is a reusable typed descriptor of a path in that model; it never owns an
`Entity` or `WeakEntity`.

The separation is intentional:

- `gpui-form` owns the local draft, baseline, revision, validation, and a
  prepared submit snapshot;
- `gpui-store` owns shared application state such as catalogs, selected
  resources, and loaded records;
- `gpui-operation` owns remote save/refresh/retry lifecycles and their tasks.

## Quick start

Declare the exact model that the application will submit. `FormModel` generates
the named `ProviderForm` entity state, which implements `FormState`:

```rust,ignore
use gpui_form::FormModel;

#[derive(Clone, Debug, PartialEq, FormModel, garde::Validate)]
#[form(state = ProviderForm, validation(adapter = "garde"))]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    name: String,

    #[form(validate(on_submit))]
    #[garde(range(min = 0, max = 10))]
    retry_limit: u32,
}
```

Create one entity for one editing session. Every statically declared model
field is one allocation-free schema-level associated constant in
`SCREAMING_SNAKE_CASE`, such as
`ProviderForm::NAME: FormField<ProviderForm, String>`. It can be reused
directly as a tiny descriptor containing only static schema/access information;
accessing it never constructs per-form or per-field state, allocates, captures
a value, or subscribes. Every synchronous operation explicitly receives the
form it uses:

```rust,ignore
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::InputState;
use gpui_form::FormState as _;
use gpui_form_gpui_component::{FormInput, FormIntegerInput, IntegerInputState};

struct ProviderPage {
    form_subscription: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
    form: Entity<ProviderForm>,
}

impl ProviderPage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let form = cx.new(|cx| ProviderForm::from_value(
            ProviderInput { name: String::new(), retry_limit: 3 },
            cx,
        ));

        let name_input = FormInput::new(
            &form,
            ProviderForm::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );
        let retry_limit_input = FormIntegerInput::new(
            &form,
            ProviderForm::RETRY_LIMIT,
            |window, cx| IntegerInputState::new(window, cx)
                .min(0u32).max(10u32).step(1u32),
            window,
            cx,
        )?;

        let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());
        Ok(Self { form_subscription, name_input, retry_limit_input, form })
    }
}
```

`ProviderForm::NAME` is a total `FormField<ProviderForm, String>`. It is a
lightweight static typed lens that can be shared by many controls;
it contains neither an entity, a value, nor a subscription. It has no liveness
error because the caller supplies a strong `&Entity<ProviderForm>`:

```rust,ignore
let value = ProviderForm::NAME.value(&self.form, cx);
let issues = ProviderForm::NAME.errors(&self.form, cx);
ProviderForm::NAME.set(&self.form, "OpenAI".to_owned(), cx);
ProviderForm::NAME.validate(&self.form, ValidationTrigger::Dynamic, cx);
```

Render the native controls and field runtime state without `expect` or a
spurious `Result`:

```rust,ignore
let name_error = ProviderForm::NAME.errors(&self.form, cx)
    .first()
    .map(|issue| validation_text(&issue.message, cx));

field()
    .label("Provider name")
    .required(ProviderForm::NAME.schema().is_required())
    .child(Input::new(&self.name_input));
```

Bound controls remain small Rust handles: subscriptions first, native component
entity second. They defer native-component events through an internal
`ControlBinding`; that binding, not `FormField`, is the deferred weak-lifetime
boundary. The wrapper does not retain form state, field values, options, focus,
or editor text.

## Dynamic paths

An identified array item or computed projection can legitimately disappear. It
returns `PartialFormField<Form, T>` and makes the uncertainty visible at the
call site:

```rust,ignore
let header_parent = ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(header_parent);

let name = header_name.try_value(&form, cx)?;
header_name.try_set(&form, "Authorization".to_owned(), cx)?;
```

`FieldAccessError` describes an unavailable projection or missing/duplicate
item. `FieldMutationError` additionally describes an attempt to change a
captured stable ID. There is no `FormReleased`: a synchronous caller already
holds the strong form entity.

`AuthForm::USERNAME.within(ServerForm::AUTH)` stays total because both parent
and child are static. `ServerForm::HEADERS.item(id)` creates a runtime-addressed
`PartialFormField`; `HeaderRowForm::NAME.within(partial_parent)` consequently
stays partial. `project_value` is also partial, whether called on a static or a
composed descriptor. `within` and `item` create lightweight located descriptors,
not new schema definitions.

## Prepare, save, and rebase

`prepare_submit` validates one model snapshot, transforms it once, and returns
its revision with the output. Transformations are static and infallible;
inline business failures belong in validation, while persistence failures belong
to the page's operation:

```rust,ignore
use gpui_form::{PreparedSubmit, SubmitError};

let PreparedSubmit { revision, output } = self.form.update(cx, |form, cx| {
    form.prepare_submit(cx)
})?;

self.save_provider(revision, output, cx); // application-owned operation

// In the operation completion callback:
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(revision, saved_value, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`gpui-form` never starts persistence, owns no busy/retry state, and does not
write `gpui-store`. `gpui-operation` may coordinate the save lifecycle, but it
does not replace the form's local draft or revision-CAS rebase boundary.

## Validation and events

Validation is typed and path-scoped. A scoped adapter run replaces only the
adapter issue buckets inside that scope; sibling field issues remain intact.
Form-level issues participate only in a form-wide run. This prevents changing
one field from accidentally clearing an unrelated adapter error.

`FormEvent` is non-generic. A descriptor subscription reprojects when a
`ValueChanged { path, revision }` can affect its value, and after every
`ModelReplaced { revision }`, including the originating control.
`ValidationChanged` does not reproject values. There is no origin-echo protocol
or authoritative component read-back.

## Crates and documentation

- `gpui-form`: typed form state, validation, revisions, and submit preparation;
- `gpui-form-macros`: `#[derive(FormModel)]` and typed descriptors;
- `gpui-form-gpui-component`: owning controls and `ControlBinding` integration.

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
