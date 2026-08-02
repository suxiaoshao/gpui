# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form` 面向 GPUI 应用提供类型化 draft、验证和提交准备能力。生成的 form state 持有一份
可编辑的 Rust model；field 是该 model 路径的可复用类型化描述符，绝不持有 `Entity` 或
`WeakEntity`。

边界明确如下：

- `gpui-form` 持有本地 draft、baseline、revision、验证和 prepared submit snapshot；
- `gpui-store` 持有 catalog、已加载记录、选择项等共享应用状态；
- `gpui-operation` 持有远程保存、刷新、重试的生命周期和 task。

## 快速开始

声明应用最终提交的精确 model。`FormModel` 会生成指定名称、实现 `FormState` 的
`ProviderForm` entity state：

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

每个编辑会话创建一个 entity。每个静态声明的 model field 都是一个无 allocation 的 schema 层
`SCREAMING_SNAKE_CASE` associated const，例如
`ProviderForm::NAME: FormField<ProviderForm, String>`。它可直接复用为只包含静态
schema/access 信息的微小 descriptor；访问它绝不构造每个 form 或 field 的 state、进行 allocation、
捕获 value 或建立 subscription。所有同步操作都显式传入自己要操作的 form：

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

`ProviderForm::NAME` 是 total 的 `FormField<ProviderForm, String>`。它是轻量的静态类型化 lens，
可供多个 control 共用；其中没有 entity、value 或 subscription。调用者提供强
`&Entity<ProviderForm>`，因此不存在 liveness `Result`：

```rust,ignore
let value = ProviderForm::NAME.value(&self.form, cx);
let issues = ProviderForm::NAME.errors(&self.form, cx);
ProviderForm::NAME.set(&self.form, "OpenAI".to_owned(), cx);
ProviderForm::NAME.validate(&self.form, ValidationTrigger::Dynamic, cx);
```

渲染原生 control 与字段 runtime state 时不再需要 `expect` 或无意义的 `Result`：

```rust,ignore
let name_error = ProviderForm::NAME.errors(&self.form, cx)
    .first()
    .map(|issue| validation_text(&issue.message, cx));

field()
    .label("Provider name")
    .required(ProviderForm::NAME.schema().is_required())
    .child(Input::new(&self.name_input));
```

Bound control 仍是很小的 Rust handle：先放 subscription，后放 native component entity。
native component event 通过内部 `ControlBinding` defer；weak lifetime 的边界属于 binding，
不属于 `FormField`。wrapper 不保存 form state、field value、option、focus 或 editor text。

## 动态路径

identified array item 或计算 projection 可能消失，因此返回 `PartialFormField<Form, T>`，并在
调用点显式处理不确定性：

```rust,ignore
let header_parent = ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(header_parent);

let name = header_name.try_value(&form, cx)?;
header_name.try_set(&form, "Authorization".to_owned(), cx)?;
```

`FieldAccessError` 描述 projection 不可用、item 缺失或重复；`FieldMutationError` 额外描述试图
改写已捕获 stable ID。没有 `FormReleased`：同步调用者已经持有强 form entity。

`AuthForm::USERNAME.within(ServerForm::AUTH)` 仍是 total，因为 parent 和 child 都是静态的。
`ServerForm::HEADERS.item(id)` 创建运行时寻址的 `PartialFormField`；因此
`HeaderRowForm::NAME.within(partial_parent)` 仍是 partial。无论调用者是静态 descriptor 还是组合后
的 descriptor，`project_value` 都是 partial。`within` 与 `item` 创建的是轻量定位 descriptor，
不是新的 schema 定义。

## 准备、保存与 rebase

`prepare_submit` 对同一个 model snapshot 验证并只转换一次，返回 output 及其 revision。
transform 是静态且不可失败的；业务内联失败属于 validation，持久化失败属于页面的 operation：

```rust,ignore
use gpui_form::{PreparedSubmit, SubmitError};

let PreparedSubmit { revision, output } = self.form.update(cx, |form, cx| {
    form.prepare_submit(cx)
})?;

self.save_provider(revision, output, cx); // 应用持有的 operation

// operation completion callback：
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(revision, saved_value, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`gpui-form` 不启动持久化、不保存 busy/retry state，也不写入 `gpui-store`。`gpui-operation`
可以协调保存生命周期，但不会替代 form 的本地 draft 或 revision-CAS rebase 边界。

## 验证与 event

验证是类型化且 path-scoped 的。一次 scoped adapter run 只替换 scope 内的 adapter issue bucket；
sibling field 的 issue 会保留。form-level issue 只参与 form-wide run，因此修改一个字段不会意外
清空无关的 adapter error。

`FormEvent` 不再带泛型。当 `ValueChanged { path, revision }` 可能影响 descriptor 的值时，
以及每次 `ModelReplaced { revision }` 后，descriptor subscription 会进行重投影，其中包括
发起写入的 control。`ValidationChanged` 不重投影 value。不存在 origin echo 协议或从
component 回读权威 value。

## Crate 与文档

- `gpui-form`：类型化 form state、验证、revision 与 submit preparation；
- `gpui-form-macros`：`#[derive(FormModel)]` 与类型化 descriptor；
- `gpui-form-gpui-component`：owning control 与 `ControlBinding` 集成。

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
