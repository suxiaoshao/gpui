# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

## Bind and render a flat input

Define a typed draft, create one form session, bind the root field, and render
the returned handle as the native input entity:

```rust,ignore
use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, Subscription, Window,
    div,
};
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
    fn new(window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let runtime = Form::try_new(ProviderDraft {
            name: String::new(),
        })?;
        let form = cx.new(|_| runtime);

        // A root FieldDef is accepted directly through the sealed
        // IntoTotalPath conversion. There is no path-resolution Result here.
        let name_input = FormInput::new(
            &form,
            ProviderDraft::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self { form, form_observer, name_input })
    }
}

impl Render for ProviderEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let errors = ProviderDraft::NAME.errors(&self.form, cx);
        let feedback = errors
            .first()
            .map(|issue| validation_text(issue, cx))
            .unwrap_or_default();

        div().child(
            field()
                .label("Provider name")
                .required(ProviderDraft::NAME.schema().is_required())
                .description(feedback)
                .child(Input::new(&self.name_input)),
        )
    }
}
```

Keep both `form` and `name_input` in the page. Typing updates the typed draft;
blur asks the form to validate that field; `set`, `reset`, or `rebase` on the
form silently updates the visible input. `validation_text` is the application's
localization helper for a validation issue; the observer rerenders the label,
feedback, and surrounding buttons whenever form state changes.

Submission still uses the same core form session; controls do not need a
separate collection step:

```rust,ignore
let prepared = self.form.update(cx, |form, cx| form.prepare(cx))?;
let revision = prepared.revision();
let request: ProviderDraft = prepared.map(|draft| draft);
self.start_save(revision, request, cx);
```

Use `FormInput` for `String`, `FormIntegerInput` for integer primitives,
`FormSelect` for one optional selection, and `FormCombobox` for multiple typed
values. Render `Checkbox` and `Switch` as controlled elements. Your own
component can either use the same controlled pattern or expose a small stateful
adapter; both recipes are shown in the guide.

See the [user guide](docs/guide.md) or its
[Chinese version](docs/guide.zh-CN.md) for total and dynamic Input, Integer,
Select, Combobox, controlled boolean, options-refresh, error, and custom-adapter
recipes.
