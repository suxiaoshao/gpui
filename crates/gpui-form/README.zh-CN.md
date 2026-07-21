# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

> **实现状态：** 本 README 描述已经实现的公开 API。

`gpui-form` 是面向 GPUI 应用的类型化表单状态、验证和提交准备库。一个生成的
form store 持有当前 Rust model；control 只是这个 model 的同步投影，不是另一份
业务状态。

## 快速开始

声明应用最终要提交的精确 model：

```rust,ignore
use gpui_form::FormStore;

#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[form(validation(adapter = "garde"))]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    name: String,

    #[form(validate(on_submit))]
    #[garde(range(min = 0, max = 10))]
    retry_limit: u32,
}
```

创建一个 form entity，并从对应的类型化字段创建每个 bound control：

```rust,ignore
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::InputState;
use gpui_form::{FormControl as _, SubmitError};
use gpui_form_gpui_component::{
    FormControlError, FormInput, FormIntegerInput, IntegerInputState,
};

struct ProviderPage {
    form_subscription: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
    form: Entity<ProviderInputFormStore>,
}

impl ProviderPage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Result<Self, FormControlError> {
        let form = cx.new(|cx| {
            ProviderInputFormStore::from_value(
                ProviderInput {
                    name: String::new(),
                    retry_limit: 3,
                },
                cx,
            )
        });

        let name_input = FormInput::new(
            ProviderInputFormStore::name_field(&form),
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        )?;
        let retry_limit_input = FormIntegerInput::new(
            ProviderInputFormStore::retry_limit_field(&form),
            |window, cx| {
                IntegerInputState::new(window, cx)
                    .min(0u32)
                    .max(10u32)
                    .step(1u32)
            },
            window,
            cx,
        )?;
        let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self {
            form_subscription,
            name_input,
            retry_limit_input,
            form,
        })
    }
}
```

渲染原生 control，并从类型化字段读取验证状态。下面的 `validation_text` 是应用用于处理
`ValidationMessage` 的本地化 helper：

```rust,ignore
use gpui::{
    Context, IntoElement, ParentElement as _, Render, Window,
    prelude::FluentBuilder as _,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    form::{field, v_form},
    h_flex,
    input::Input,
    label::Label,
    spinner::Spinner,
    v_flex,
};
use gpui_form::{FormFieldId as _, FormStore as _};
use gpui_form_gpui_component::IntegerInput;

impl Render for ProviderPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name_field = ProviderInputFormStore::name_field(&self.form);
        let name_error = name_field
            .errors(cx)
            .expect("ProviderPage owns the form while rendering")
            .into_iter()
            .next()
            .map(|issue| validation_text(&issue.message, cx));
        let name_is_validating = name_field
            .is_validating(cx)
            .expect("ProviderPage owns the form while rendering");

        v_form()
            .child(
                field()
                    .label("Provider name")
                    .required(ProviderInputField::Name.schema().is_required())
                    .child(
                        v_flex()
                            .child(
                                h_flex()
                                    .child(Input::new(&self.name_input))
                                    .when(name_is_validating, |row| {
                                        row.child(Spinner::new())
                                    }),
                            )
                            .when_some(name_error, |this, error| {
                                this.child(Label::new(error))
                            }),
                    ),
            )
            .child(
                field()
                    .label("Retry limit")
                    .child(IntegerInput::new(&self.retry_limit_input)),
            )
            .child(
                Button::new("save-provider")
                    .primary()
                    .label("Save")
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
    }
}
```

Bound control 是普通 Rust handle。它会解引用到原生 GPUI component entity，并且只
保存 entity 和同步 subscription。Focus、IME、selection、popup 状态、options 和临时
editor text 仍由 component 或应用负责。

提交 form 中已经存储的 model。在同一次 entity update 中捕获 revision，把持久化状态
保存在页面或应用 store 上，并在请求完成后有条件地 rebase 已保存值：

```rust,ignore
let prepared = self.form.update(cx, |form, cx| {
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((form.revision(), output))
});

match prepared {
    Ok((revision, output)) => self.start_save(revision, output, cx),
    Err(error) => self.show_submit_error(error, cx),
}

// 在 save task 的完成回调中：
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(submitted_revision, saved_value, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`prepare_submit` 只执行同步 submit validation 和一次纯 transform。它不会启动持久化；
form 不保存 submit task、busy flag、retry policy 或 submission-attempt counter。存在
active async validation 时返回 `SubmitError::ValidationPending`。

每个 `FieldChanged` 和 `ModelReplaced` event 都会把值静默重投影到所有已挂载的 bound
control，包括发起字段写入的 control。Adapter 不依赖跳过 origin echo，也不会把 component
state 当作权威值。

Nested group 和 stable-ID array 仍属于同一个顶层 model。生成的字段 accessor 可以直接
组合而不会创建 child form entity；`FormField::project_value` 可以暴露计算得到的类型化值，
而不会创建平行业务值。

## Crate

- `gpui-form`：类型化 form state、revision/baseline tracking、validation 和 submit
  preparation；
- `gpui-form-macros`：`#[derive(FormStore)]` 和类型化字段 accessor；
- `gpui-form-gpui-component`：面向 `gpui-component` 的 owning bound control。

## 文档

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
