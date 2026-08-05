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

## 绑定并渲染 flat input

定义 typed draft，创建唯一 form session，绑定 root field，然后把返回的 handle 作为原生
input entity 渲染：

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

        // root FieldDef 通过 sealed IntoTotalPath conversion 直接传入。
        // 这里没有 path resolution Result。
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

页面同时保留 `form` 与 `name_input`。输入会更新 typed draft；blur 会请求 form 校验该字段；
对 form 执行 `set`、`reset` 或 `rebase` 时，可见 input 会被静默更新。`validation_text` 是应用把
validation issue 本地化为文案的 helper；observer 会在 form state 变化后重新渲染 label、feedback
与外围按钮。

提交仍使用同一个 core form session；无需从 controls 另行收集一次值：

```rust,ignore
let prepared = self.form.update(cx, |form, cx| form.prepare(cx))?;
let revision = prepared.revision();
let request: ProviderDraft = prepared.map(|draft| draft);
self.start_save(revision, request, cx);
```

`String` 使用 `FormInput`，整数 primitive 使用 `FormIntegerInput`，单个可选选择使用
`FormSelect`，多个 typed value 使用 `FormCombobox`。`Checkbox` 与 `Switch` 作为 controlled
element 渲染。自己的组件既可以使用同样的 controlled pattern，也可以提供一个很小的
stateful adapter；guide 展示了两种 recipe。

参见[使用指南](docs/guide.zh-CN.md)或其[英文版本](docs/guide.md)，其中包含
total/dynamic Input、Integer、Select、Combobox、controlled boolean、options refresh、错误与
custom adapter recipe。
