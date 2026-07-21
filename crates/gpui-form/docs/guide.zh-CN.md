# gpui-form 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

> **实现状态：** 本指南描述已经实现的公开 API。

`gpui-form` 为 GPUI 应用提供类型化表单数据、验证、版本跟踪和提交准备能力。
本文从库使用者的角度说明公开契约。

## 1. Crate 与 feature

应用通常同时依赖表单 crate 和组件适配器。derive macro 已由 `gpui-form` 重新导出：

```toml
[dependencies]
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
garde.workspace = true
```

在 `gpui-form` 上启用可选集成：

```toml
gpui-form = { workspace = true, features = ["garde-adapter", "validify-transform"] }
```

- `garde-adapter` 验证实现了 `garde::Validate` 的模型；
- `validify-transform` 克隆待提交模型，并对副本应用 `validify::Modify`；
- `form-pipeline` 同时启用这两项集成。

## 2. 声明一个类型化模型

表单模型直接使用应用真正要提交的 Rust 类型：

```rust,ignore
use gpui_form::FormStore;

#[derive(Clone, Debug, PartialEq, FormStore)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,

    #[form(validate(on_submit))]
    retry_limit: u32,

    #[form(validate(on_dynamic, on_submit))]
    model_id: Option<String>,
}
```

`#[derive(FormStore)]` 会生成：

- `ProviderInputFormStore`：持有当前模型的 GPUI entity state；
- `ProviderInputField`：静态字段 identity 和 schema enum；
- `ProviderInputFormStore::name_field(&form)` 这类类型化字段访问器；
- 验证遍历、嵌套访问器、版本处理和提交衔接代码。

generated store 恰好只保存一个内部 `FormRuntime`，由它持有 current value、baseline、
revision、validation context 和 validation state。validation adapter 与 submit transform
只作为 associated type 存在：form 在操作需要时构造其无状态 `Default` 值，不保存 adapter
或 transform instance。运行时依赖应放入类型化 validation context 或由应用 state 持有。

表单不会为整数、枚举或其他类型另存一份 String draft，也不需要 codec。具体组件可以在
内部保存尚未完成的编辑文本，但该文本永远不会替代表单里的类型化业务值。

## 3. 创建 form

每个编辑会话创建一个 form entity：

```rust,ignore
use gpui::AppContext as _;

let form = cx.new(|cx| {
    ProviderInputFormStore::from_value(
        ProviderInput {
            name: String::new(),
            retry_limit: 3,
            model_id: None,
        },
        cx,
    )
});
```

当 `Self::ValidationContext: Default` 时可以使用 `FormStore::from_value`。它先安装模型
和 context，再在返回 store 前恰好执行一次 mount validation。

验证需要应用持有的依赖时，传入类型化 context：

```rust,ignore
let form = cx.new(|cx| {
    ProviderInputFormStore::from_value_with_validation_context(
        initial,
        ProviderValidationContext { catalog: catalog.clone() },
        cx,
    )
});
```

`from_value_with_validation_context` 始终可用。它会在唯一一次 mount validation 前安装
初始模型和给定 context。

`set_validation_context(next, cx)` 只替换 context 并通知 form observer，不会自行选择
validation trigger。新 context 需要影响报告时，由调用者显式运行 dynamic 或 submit
validation。

## 4. 创建绑定控件

`gpui-form-gpui-component` 用一次调用创建原生组件 state，并把它绑定到类型化字段：

```rust,ignore
use gpui_component::input::InputState;
use gpui_form::FormControl as _;
use gpui_form_gpui_component::{
    FormInput, FormIntegerInput, IntegerInputState,
};

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
```

返回的控件是普通 Rust handle，不会再套一层 GPUI entity。它 deref 到原生 state entity，
自身只保留该 entity 和绑定订阅。组件配置放在构造闭包或原生组件 API 中；只影响 element
展示的配置放在渲染时 builder 上。

`FormField::subscribe_in` 会处理每一个 `FormEvent::FieldChanged` 和
`FormEvent::ModelReplaced`，不按来源或 event path 过滤，并忽略
`FormEvent::RuntimeChanged`。每次 callback 都重新读取自己的类型化字段并静默投影到已挂载
控件，包括发起这次用户写入的控件。组件的 silent setter 不得再次发出用户事件。因此
binding 只需要这一条规则，不公开“跳过来源控件”或“权威值回读”等协议。

组件用户事件会把类型化字段写入 defer 到 emitter 当前 update 结束之后，从而避免 GPUI
entity 重入；应用代码无需自行完成这次 defer。

页面或 controller 生命周期内只需观察一次 form，即可在表单 runtime 状态变化时重新渲染
label、验证反馈和按钮：

```rust,ignore
let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());
```

## 5. 渲染字段和 form 状态

generated schema 提供静态元数据，form 提供数据层 runtime 状态，原生控件则提供自己的
交互状态：

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
            .expect("the page owns the form while rendering")
            .into_iter()
            .next()
            .map(|issue| validation_text(&issue.message, cx));
        let name_is_validating = name_field
            .is_validating(cx)
            .expect("the page owns the form while rendering");

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

常用查询包括：

- `is_dirty()` 和 `is_valid()`；
- `is_validating()` 和 `is_validating_at(path)`；
- `validation_report()`、`errors_at(path)` 和 `first_error_path()`；
- `revision()`。

form 不持有 `FocusHandle`、focused/touched/blurred 标记或错误可见性标记。validation
trigger 决定 issue 何时进入报告。提交失败后，由当前页面通过 `first_error_path()` 决定
应该聚焦哪个可见控件实例。

## 6. 修改数据并保护异步保存

### 类型化字段写入

应用通过类型化 handle 修改一个字段：

```rust,ignore
ProviderInputFormStore::retry_limit_field(&self.form).set(5, cx)?;
```

有状态 bound control 在内部使用 attachment。Checkbox 这类无状态受控 element 使用明确的
用户写入方法：

```rust,ignore
ProviderInputFormStore::enabled_field(&self.form)
    .set_user_value(true, cx)?;
```

两种方法都会先保存类型化值，再执行 change validation。相等字段写入是 no-op：不会推进
revision、重跑验证或发出新的投影事件。

### 整表生命周期

安装应用数据时使用 whole-form 操作：

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next_value, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved_value, cx));
```

- `replace` 安装新的 current model，但保留原 baseline；
- `reset` 把 baseline 恢复为 current model；
- `rebase` 把同一个 model 同时安装为 current value 和 baseline；
- 只有 current revision 等于 expected revision 时，`rebase_if_revision` 才执行与
  `rebase` 相同的操作。

这些都是显式生命周期操作。每次调用 `replace`、`reset` 或 `rebase`，以及每次成功的
`rebase_if_revision`，即使安装的 Rust value 比较相等，也会推进 revision。每项操作都会
取消活跃异步验证、清除数据层 validation issue，并把值静默重投影到所有已挂载控件。它只
在组件 silent setter 能支持的范围内保留组件自身交互状态，也不会伪造逐字段 change
validation。

### Revision 与条件 rebase

`FormRevision` 是业务值状态的单调递增 token。字段写入和 whole-form 生命周期操作会推进
它；验证执行、pending 状态、control issue 和 validation-context 更新不会。

在同一次 entity update 中取得 revision 和 prepared output：

```rust,ignore
let (submitted_revision, output) = self.form.update(cx, |form, cx| {
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((form.revision(), output))
})?;
```

持久化 task 和 loading 状态由页面或应用 store 持有。成功后，仅当用户期间没有继续编辑
form 时，才安装 repository 返回的规范化值：

```rust,ignore
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(submitted_revision, saved_value, cx)
});

if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

被拒绝的条件 rebase 返回 `false`，且完全没有副作用：不改变 value、baseline、revision、
验证状态、异步任务或控件。成功的条件 rebase 会推进 revision，因此针对同一个 submitted
revision 的两个响应不可能都应用。只有应用在请求整个生命周期内阻止了所有业务值写入时，
才能在请求后使用无条件 `rebase`。

持久化代码消费 `prepare_submit` 的输出，不应读取字段或组件 state 来拼装模型。

## 7. 验证

### Trigger 与 scope

支持以下 trigger：

| Attribute | Runtime trigger |
| --- | --- |
| `on_mount` | 构造器安装初始模型和 context 后恰好执行一次 |
| `on_change` | 类型化字段写入提交后执行 |
| `on_blur` | 具体 bound control 报告最终 blur 时执行 |
| `on_dynamic` | 应用显式刷新外部依赖时执行 |
| `on_submit` | `prepare_submit` 期间执行 |

`ValidationScope::Field(path)` 包含变更 path、其后代和其祖先 group/array path，但不包含
兄弟叶子。Group 和 identified array-item scope 包含自己的子树和祖先。
`ValidationScope::Form` 包含所有数据 path。

一次 validation run 只会替换同时被 trigger 和 scope 选中的同步字段 issue。adapter 产生的
form-level issue 使用一个 adapter 全局 bucket；即使运行的是 field scope，该 bucket 也会在
每次 adapter 执行时整体替换。不参与本次执行的字段 issue 保持不变。

成功的类型化字段写入会先推进 model 和 revision，然后只清除相交的 required、structural
和 generated synchronous field bucket，取消并清除相交的 async validation，保留
adapter-wide form bucket 和所有 active control issue，再执行 change validation，最后发出一个
`FormEvent::FieldChanged`。这里的“保留”描述的是 invalidation 阶段：如果 adapter 参与随后
的 change-validation run，该次 run 仍会按正常语义整批替换 adapter-wide bucket；change
validation 永远不拥有或清除 control issue。相等写入完全是 no-op。

### Required value

`required` 同时是静态 schema 元数据和内置验证规则：

```rust,ignore
#[form(required, validate(on_change, on_blur))]
name: String,
```

required rule 始终参与 submit validation；列出的 trigger 只负责更早反馈。
`RequiredValue::is_missing` 精确定义缺失语义：

- `String` 的 `trim()` 为空时缺失；
- `Option<T>` 为 `None` 时缺失；
- `Vec<T>`、`HashMap`、`BTreeMap`、`HashSet` 和 `BTreeSet` 为空时缺失；
- `bool` 为 `false` 时缺失，可用于必须同意的控件；
- 自定义类型通过实现 `RequiredValue` 主动加入。

数字和 enum 没有通用“缺失”值，因此库不提供内置实现。在不支持的类型上使用 `required`
是编译错误。领域特定的数字或 enum 约束应使用显式验证规则。

内置 issue 使用稳定 key `gpui-form-error-required`。应用在字段的本地化 label 旁渲染时
翻译该 key。

### Garde

同步模型和业务规则优先使用 Garde：

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(validation(adapter = "garde"))]
struct AccountInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    display_name: String,

    #[form(validate(on_change, on_blur, on_dynamic, on_submit))]
    #[garde(email)]
    email: Option<String>,
}
```

`#[form(required)]` 负责空值语义，不要用 Garde 重复同一约束。需要 Garde 递归验证的 group
或 array 必须显式加 `#[garde(dive)]`。

带 Garde context 的模型要在 Garde 上声明 context，并把它传给 generated form 构造器：

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[garde(context(AccountValidationContext))]
#[form(validation(
    adapter = "garde",
    i18n = AppGardeI18nProvider
))]
struct AccountInput {
    #[form(validate(on_dynamic, on_submit))]
    #[garde(custom(validate_account_plan))]
    plan_id: Option<String>,
}
```

adapter 会使用完全相同的类型化 context 调用 `garde::Validate::validate_with`；非默认
context 不会 fallback 到 `validate()`。

Garde 0.23.0 通过 `garde::i18n::with_i18n` 本地化内置错误。provider 返回实现上游精确
trait 的 handler：

```rust,ignore
use std::borrow::Cow;
use garde::i18n::{I18n, InvalidEmail};

struct AppGardeI18n<'a> {
    i18n: &'a AppI18nSnapshot,
}

impl I18n for AppGardeI18n<'_> {
    fn length_lower_than(&self, min: usize) -> Cow<'static, str> {
        self.i18n
            .translate("validation-length-lower-than", [("min", min.to_string())])
            .into()
    }

    fn email_invalid(&self, reason: InvalidEmail) -> Cow<'static, str> {
        self.i18n
            .translate(
                "validation-email-invalid",
                [("reason", reason.to_string())],
            )
            .into()
    }

    // Implement every other method required by garde::i18n::I18n 0.23.0.
}
```

0.23.0 的签名只接收上游 trait 中列出的规则参数，例如
`length_lower_than(min)`，并返回 `Cow<'static, str>`；它不接收 `actual` length 参数。

`GardeI18nProvider<C>` 从 form validation context 创建 handler。handler 仅安装在当前线程
和同步调用栈中，绝不能跨越 `await`。Garde 在每个 error 中保存最终字符串，因此 adapter
将其保留为 `ValidationMessage::Localized`。省略 `i18n` 时选择
`DefaultGardeI18nProvider`。

Garde 的显示 path 使用 vector position。generated `GardePathMapper` 会在 scope 过滤前，
把当前 index 映射到被验证模型中的稳定 `FormItemId`。未知字段、格式错误或越界 index、
无法转换的 ID 和重复 ID 都会变成阻止提交的 internal form issue。adapter 不使用 Garde
doc-hidden path iterator，也绝不会在最终 `FieldPath` 中保留可变 vector index。

语言变化时，应用更新 validation context，并为需要重新生成的消息显式请求 dynamic
validation：

```rust,ignore
self.form.update(cx, |form, cx| {
    form.set_validation_context(next_context, cx);
    form.validate(
        ValidationTrigger::Dynamic,
        ValidationScope::Form,
        cx,
    );
});
```

### 自定义同步 adapter

任何同步验证库都通过 `ValidationAdapter<Model>` 接入：

```rust,ignore
use gpui::App;
use gpui_form::{
    FormFieldId as _, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationIssue, ValidationMessage, ValidationScope,
    ValidationSource, ValidationTrigger,
};

#[derive(Default)]
struct ProviderValidator;

impl ValidationAdapter<ProviderInput> for ProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        &self,
        value: &ProviderInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let mut report = ValidationAdapterReport::default();
        let path = ProviderInputField::ModelId.path();

        if scope.includes(Some(&path))
            && value.model_id.as_ref().is_some_and(|id| {
                !context.external.model_ids.contains(id)
            })
        {
            report.push(ValidationIssue::field(
                path,
                trigger,
                ValidationSource::App("provider".into()),
                "model_unavailable",
                ValidationMessage::key("provider-model-unavailable"),
            ));
        }

        report
    }
}
```

adapter 会直接收到 trigger；`ValidationContext` 只包含类型化 external context，没有冗余的
`submitted` 标记。自定义 adapter 实现 `Default + 'static`。每次 validation run 都构造
`Self::ValidationAdapter::default()`；form 不保存 adapter instance，因此 runtime 依赖放入
context。

外部验证库的 path 必须映射为 generated `FieldPath`。未知 path 是阻止提交的 internal
form-level issue，而不是被忽略的字符串。渲染、focus、持久化和库特有 global state 均留在
adapter 外部。

### 异步验证

页面持有决定何时启动远程检查的 subscription；检查一旦启动，就由 form 持有：

```rust,ignore
use gpui_form::{
    AsyncValidationIssue, ValidationMessage, ValidationTrigger,
};

let field = ProviderInputFormStore::name_field(&self.form);
let validation_field = field.clone();
let service = self.provider_service.clone();

let validation_subscription = field.subscribe_in(
    window,
    cx,
    move |_owner, window, cx| {
        let field = validation_field.clone();
        let service = service.clone();

        // This callback is emitted by the same form. Start the next form
        // update only after the current update scope has ended.
        cx.defer_in(window, move |_owner, _window, cx| {
            field.start_async_validation(
                "provider-name",
                ValidationTrigger::Change,
                move |name| async move {
                    if service.name_available(&name).await {
                        Ok(())
                    } else {
                        Err(AsyncValidationIssue::new(
                            "name_taken",
                            ValidationMessage::key("provider-name-taken"),
                        ))
                    }
                },
                cx,
            )?;
            anyhow::Ok(())
        });
    },
)?;
```

`start_async_validation` 会 snapshot 当前类型化字段值，并在 `(field path, source)` 下保留
一个 `Task<()>` 和单调递增 generation。再次启动相同 key 或写入相交值时，会取消旧 task、
清除旧 issue 并安装新 generation。过期 completion 没有任何效果。

`cancel_async_validation(source, cx)`、whole-form 生命周期操作或 form 被 drop 时，都会
取消 task 并清除 pending 状态。页面 subscription 被 drop 只会阻止未来检查，不会丢弃已经
启动的检查，因为该 task 由 form 保留。

所有通过该 API 注册的活跃异步验证都会阻止提交。在全部检查完成前，`prepare_submit`
返回 `SubmitError::ValidationPending`。不阻止提交的远程提示属于普通应用 UI state，不应
使用 form async-validation API。

### Control issue 与错误消息

类型化控件暂时无法产生字段类型时，例如整数编辑器只有 `-`，会在内部保留该文本并发布
control issue。只有 bound control 生命周期仍然活跃时，该 issue 才阻止提交。该 control
共享 attachment lease 的最后一个 clone 被 drop（通常是 binding subscriptions 被 drop）时，
或 dynamic path 消失后发生 internal invalidation 时，issue 都会失效。

`ValidationMessage::Key { key, params }` 由应用在渲染时翻译；
`ValidationMessage::Localized` 已经是最终文本（例如 Garde 错误），不能再次翻译。form
不持有 locale global 或 error renderer。

## 8. 提交与转换

`prepare_submit` 是同步的验证与转换边界：

```rust,ignore
let prepared = self.form.update(cx, |form, cx| {
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((form.revision(), output))
});
```

它只克隆一次 current model snapshot，并按固定顺序执行：

1. 对该 snapshot 运行同步 submit validation；
2. 存在数据 issue 或活跃 control issue 时返回 `SubmitError::Validation(report)`；
3. 存在 form-owned async validation 时返回 `SubmitError::ValidationPending`；
4. 对同一个 snapshot 运行一次 submit transform，并返回 output 或
   `SubmitError::Transform(report)`。

该操作不会启动持久化。save task、loading 状态、取消、重试策略、provider/database 错误和
用户通知均由页面或应用 store 持有。form 不公开 submit task、busy flag、提交尝试计数或
`SubmitError::Busy`。

为自定义 output 实现 `SubmitTransform<Model>`：

```rust,ignore
use gpui_form::{SubmitTransform, TransformReport};

#[derive(Default)]
struct ProviderTransform;

struct SaveProvider {
    name: String,
    retry_limit: u32,
}

impl SubmitTransform<ProviderInput> for ProviderTransform {
    type Output = SaveProvider;

    fn transform(
        &self,
        model: &ProviderInput,
    ) -> Result<Self::Output, TransformReport> {
        Ok(SaveProvider {
            name: model.name.trim().to_owned(),
            retry_limit: model.retry_limit,
        })
    }
}
```

`SubmitTransform<Model>` 要求 `Default + 'static`，只有一个 associated `Output` 和一个
`transform` 方法，没有 preview path 或 transform context。`prepare_submit` 只有在 validation
和 pending 检查都通过后才构造 `Self::SubmitTransform::default()`，并恰好调用一次
`transform`。Identity 和 Validify transform 使用 `Output = Model`。转换是纯函数：不会修改
form value、baseline、revision、validation report 或控件。

transform failure 属于 submit result，不属于 validation state。需要影响 inline error 或
`is_valid()` 的规则应放进 `ValidationAdapter`。

## 9. 实现自定义有状态控件

`FormControl<T>` 统一“一步构造并绑定”，但不持有组件配置：

```rust,ignore
use std::ops::Deref;
use gpui::{App, Context, Entity, Subscription, Window};
use gpui_form::{
    ControlAttachment, FormField, FormFieldError, FormStore,
};

pub trait FormControl<T>: Deref<Target = Entity<Self::State>> + Sized
where
    T: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Error;

    fn new<Form, Owner, Build>(
        field: FormField<Form, T>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State;
}

impl<Form, T> FormField<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub fn attach_control(
        &self,
        cx: &mut App,
    ) -> Result<ControlAttachment<Form, T>, FormFieldError>;
}
```

stateful handle 只保存 subscriptions 和原生 entity；subscriptions 必须声明在前，以便先
drop：

```rust,ignore
pub struct FormRating {
    subscriptions: Vec<Subscription>,
    rating: Entity<RatingState>,
}

impl Deref for FormRating {
    type Target = Entity<RatingState>;

    fn deref(&self) -> &Self::Target {
        &self.rating
    }
}
```

binding 首先创建一个 attachment。构造会立即确认 form 仍存在且 field/path 可读取，否则分别
返回 `FormReleased` 或 `ValueUnavailable`：

```rust,ignore
let attachment = field.attach_control(cx)?;
let component_attachment = attachment.clone();
let projection_attachment = attachment;
```

`ControlAttachment` 实现 `Clone`。所有 clone 共享一个 private control identity 与 lease；clone
不会注册第二个 control。把这些 clone 移入 component-event 和 form-projection subscription
closure，而不是在 wrapper 上增加 attachment 字段。callback 通过 attachment 的 deferred
方法表达组件 intent：

```rust,ignore
attachment.defer_set_user_value(next, window, cx);
attachment.defer_blur(window, cx);
attachment.defer_set_issue("invalid_rating", message, window, cx);
attachment.defer_clear_issue(window, cx);
```

这四个方法是 attachment 唯一的 public mutation API。它们返回 `()`，并把 form update 安排
到当前 owner update 结束之后。`attach_control` 本身只构造并验证 lease，不修改业务值、
revision 或 validation report。weak lifetime state 和内部 control identity 都是 crate-private
实现细节。drop 某一个 clone 时，只要还有其他 clone，control 仍然 active；最后一个 clone
drop 后，private lease 无法再 upgrade，队列中的 intent 变成 no-op，control issue 立即视为
inactive。runtime 中的 weak lease 不会延长该生命周期。调用方既不 upgrade weak attachment，
也不解释 control ID。

自定义实现遵守以下规则：

1. 读取当前类型化字段，创建原生 state，并静默投影初始值；
2. 所有组件来源的字段写入都通过 `defer_set_user_value` 发送，由它 defer 到 emitter update
   结束后；
3. 使用 `FormField::subscribe_in` 处理每个 `FieldChanged` 和 `ModelReplaced`，只忽略
   `RuntimeChanged`，并把每次产生的值静默投影到当前组件，包括自己的写入；
4. 不跳过 origin echo，也不把原生 component state 当作第二个权威值；
5. 只有组件能提供可靠的最终 blur signal 时才调用 `defer_blur`，不在 form 中存 focus 或
   blur 状态；
6. 不完整 editor state 留在原生 state 中，并通过 `defer_set_issue` 发布生命周期受控的
   issue；integer 或自定义 projection 可以捕获同一个 attachment clone，在成功完成 silent
   authoritative projection 后调用 `defer_clear_issue`；
7. projected 或 identified path 消失时，由 deferred attachment 操作在内部处理
   `FormFieldError::ValueUnavailable`：它会使私有 lifetime 失效并通知 owner，由 owner drop
   或重建控件；form-to-control 投影遇到同一错误时也通知 owner，不虚构 fallback value；
8. options、disabled state、placeholder、accessibility 配置和 presentation 均放在 form
   field 外部。

attachment 的 public mutation surface 仅包含上面的四个 deferred intent 方法，不暴露
immediate write、authoritative read-back、weak handle、control ID 或 origin token。deferred
写入成功后仍执行正常的静默 form 投影。wrapper 字段仍然只有前置的
`Vec<Subscription>` 和原生 `Entity<State>`；captured attachment clone 存活在这些
subscription closure 内。

## 10. 无状态控件与组件配置

无状态 `Checkbox` 和 `Switch` element 不需要伪造 state wrapper。直接从
`FormField<bool>` 把它们渲染为 controlled element，并在 click callback 中调用
`set_user_value`。页面对 form 的观察会让该字段的所有 consumer 重新渲染。

```rust,ignore
use gpui_component::{checkbox::Checkbox, switch::Switch};

let enabled_field = ProviderInputFormStore::enabled_field(&self.form);
let enabled = enabled_field
    .value(cx)
    .expect("ProviderPage 在 render 期间持有 form");

let checkbox_field = enabled_field.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .label("Enabled with checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field
            .set_user_value(*checked, cx)
            .expect("element 挂载期间 ProviderPage 持有 form");
    });

let switch = Switch::new("provider-enabled-switch")
    .label("Enabled with switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        enabled_field
            .set_user_value(*checked, cx)
            .expect("element 挂载期间 ProviderPage 持有 form");
    });
```

这些 element callback 并不是从 component-state entity update 中发出，因此可以安全地
直接写 field。只有当页面在 element 的整个挂载生命周期内从结构上确定持有 form 和 path
时才使用 `expect`；projected 或 dynamic path 可能正常消失时，应显式处理
`FormFieldError`。

options 和 catalog 属于配置，不属于 form data：

1. 更新应用 catalog store；
2. 更新或重建原生组件 state；
3. 通过当前原生 delegate 静默重投影权威 form value；
4. 当前值不可用时显式执行 dynamic validation。

options 刷新绝不会选择第一项、修改 form value、rebase、持久化或隐式读取数据库。Select
和 Combobox 的准确 API 见组件适配器指南。

## 11. 嵌套模型、数组与 projection

嵌套数据仍然保存在一个 root model 中：

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormStore)]
struct AuthInput {
    #[form(required, validate(on_change, on_blur))]
    username: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
struct HeaderRowInput {
    row_id: u64,
    #[form(required, validate(on_change, on_blur))]
    name: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
struct ServerInput {
    #[form(group)]
    auth: AuthInput,

    #[form(array(id = "row_id"))]
    headers: Vec<HeaderRowInput>,
}
```

generated accessor 会在 root store 上组合类型化 lens：

```rust,ignore
let username = AuthInputFormStore::username_in(
    ServerInputFormStore::auth_field(&form),
);

let header_name = HeaderRowInputFormStore::name_in(
    ServerInputFormStore::headers_item(
        &form,
        FormItemId::new(row_id),
    ),
);
```

重复调用 accessor 只会创建廉价 handle，不会创建 value、subscription 或 child entity。
多个控件可以安全消费同一个字段。

稳定 ID 在数组内必须唯一，在一个 form session 中对同一个逻辑 item 保持不变，也不能被
另一个 item 复用。ID 缺失、重复或无法转换时，受影响 handle 返回
`FormFieldError::ValueUnavailable`，并创建阻止提交的 structural issue。库绝不会选择
第一个重复项。重排会保留寻址，但会让旧的后代验证结果失效；新的验证会把 issue 映射到
当前稳定 ID。

只有计算值或条件存在的类型化值才使用 `project_value`：

```rust,ignore
let budget = JobInputFormStore::run_settings_field(&form).project_value(
    "token_budget",
    |settings| settings.custom_token_budget(),
    |settings, value| settings.set_custom_token_budget(value),
);
```

技术名称会创建独立 `FieldPathSegment::Projection`，永远不会与真实 model field 冲突。
projected path 用于标识 control 和 async issue，而 `validation_path` 保留最近的真实 parent
path，因此写入会执行 parent field 的验证规则。projection 上继续 projection 时也保留同一个
真实 validation path。

projection 不再存在时，读写返回 `FormFieldError::ValueUnavailable`。结构 owner 应 drop
或重建控件，而不是虚构 fallback value。

## 12. 跨页面共享一个 form

同一个 `Entity<GeneratedFormStore>` 可以由多个页面共享。每个页面分别创建自己的 bound
handle 和 page-level observation。所有控件收到同一个类型化值投影，而 focus、selection、
popup/query state、私有编辑文本和 subscriptions 都留在当前控件实例。

drop 一个页面只会移除该页面的 binding。form、其他页面、current value、validation state
和应用持久化 task 仍由各自 owner 管理。

## 13. 职责表

| 职责 | Owner |
| --- | --- |
| 当前类型化值、baseline、revision、验证报告/任务、提交准备 | `gpui-form` generated store |
| 类型化 field/schema/嵌套遍历生成 | `gpui-form-macros` |
| 原生组件 entity 与 binding subscriptions | bound control owner |
| Focus、IME、selection、popup/query/highlight、不完整编辑文本 | 具体 component state |
| Options/catalog/capability/disabled/presentation | 应用和组件 |
| Save task/loading/retry/provider/database 错误 | 页面、controller 或应用 store |
| 错误渲染、locale observation、提交后 focus | 应用 |

## 14. 相关文档

- [gpui-form-macros 使用指南](../../gpui-form-macros/docs/guide.zh-CN.md)
- [gpui-form-gpui-component 使用指南](../../gpui-form-gpui-component/docs/guide.zh-CN.md)
