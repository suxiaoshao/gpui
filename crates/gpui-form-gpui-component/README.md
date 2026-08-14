# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-gpui-component` connects a typed `gpui_form::Form<M>` session to
stateful controls from `gpui-component`. It supplies adapters for text,
integers, single selection, and multiple selection; the Form remains the only
authority for business values.

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

## Bind a field

Define a schema, create one form for one editing session, then retain both the
form and the adapter in the page:

```rust,ignore
use gpui::{Context, Entity, IntoElement, Render, Subscription, Window, div};
use gpui_component::{form::field, input::{Input, InputState}};
use gpui_form::{Form, FormSchema};
use gpui_form_gpui_component::FormInput;

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    #[form(required)]
    name: String,
}

struct ProviderEditor {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
}

impl ProviderEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| Form::new(ProviderDraft {
            name: String::new(),
        }));

        let name_input = FormInput::new(
            &form,
            ProviderDraft::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );

        // This redraws page-owned labels, errors, and buttons. It is not
        // required to keep FormInput synchronized with the Form.
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Self { form, form_observer, name_input }
    }
}

impl Render for ProviderEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let feedback = ProviderDraft::NAME
            .errors(&self.form, cx)
            .first()
            .map(|issue| validation_text(issue, cx))
            .unwrap_or_default();

        div().child(
            field()
                .label("Provider name")
                .required(true)
                .description(feedback)
                .child(Input::new(&self.name_input)),
        )
    }
}
```

`FormInput` defers native edits into the Form, silently projects relevant Form
value changes back to the native input, and owns that synchronization for its
entire lifetime. A write from this input is not immediately projected back to
the same input, while another control bound to the same path does receive the
new value.

## Submit the same session

Controls do not collect a second copy of the draft. Prepare the existing Form,
perform application-owned I/O, then only rebase if the saved version is still
current:

```rust,ignore
let prepared = self.form.update(cx, |form, cx| form.prepare(cx))?;
let (version, request): (_, ProviderDraft) = prepared.into_parts();

let canonical_model = self.save_provider(request).await?;
self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_model, cx);
});
```

## Choose the right adapter

- `FormInput` binds `String`.
- `FormIntegerInput` binds integer primitives while keeping incomplete text in
  the native editor.
- `FormSelect` binds `Option<D::Item::Value>`.
- `FormCombobox` binds `Vec<D::Item::Value>`.
- `Checkbox` and `Switch` are rendered as controlled elements because they do
  not expose a state entity.

Pass a total path to `new`. A path through a collection item, enum case, or
`Option::Some` is dynamic: resolve it first and pass the resulting path to
`try_new`. A dynamic adapter is retired with its location; create a fresh one
for a new location instead of retargeting the old control.

The page observer is for page rendering only. Built-in adapters synchronize
without a page-level `FormEvent` subscription. For catalogs and delegates,
the application owns option refreshes; refreshing options never changes Form
values or selects a fallback implicitly.

See the [user guide](docs/guide.md) or its [Chinese version](docs/guide.zh-CN.md)
for total and dynamic paths, all adapters, validation, option refreshes, and a
custom stateful-adapter recipe.
