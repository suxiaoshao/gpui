# gpui-form v2 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form` 持有一份可编辑的类型化 draft，负责验证和提交准备；它不持有远程 I/O、共享应用
状态、组件交互状态，也不保存第二份业务值。

## 1. Crate 与边界

应用通常同时使用 core crate 和组件适配器：

```toml
[dependencies]
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
garde.workspace = true
```

三层的所有权彼此独立：

| 事实 | owner |
| --- | --- |
| 类型化 draft、baseline、revision、验证、prepared output | 生成的 `FormState` |
| catalog、选中记录、共享的已加载数据 | `gpui-store` 或其他应用 state owner |
| 保存/刷新/重试 task、loading 与通知 | page/controller，需要时配合 `gpui-operation` |
| Focus、IME、selection、popup/query、未完成 editor text | native component state |
| binding subscription 与 deferred control intent | owning control 与 `ControlBinding` |

编辑 form 不会顺带写入 shared store。operation 成功后，应用显式以
`rebase_if_revision` 应用 canonical result。

## 2. 声明 model 与生成的 state

model 直接使用应用最终提交的精确 Rust 类型。`FormModel` 生成指定名称的 entity state，并为每个
静态声明的 field 生成一个无 allocation 的 schema 层 `SCREAMING_SNAKE_CASE` associated const：

```rust,ignore
use gpui_form::FormModel;

#[derive(Clone, Debug, PartialEq, FormModel)]
#[form(state = ProviderForm)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,

    #[form(validate(on_submit))]
    retry_limit: u32,

    #[form(validate(on_dynamic, on_submit))]
    model_id: Option<String>,
}
```

生成的 `ProviderForm` 实现 `FormState`，并且只持有一个 runtime：current model、baseline、
单调递增的 `FormRevision`、类型化 validation context、validation report 和已经启动的
async-validation task。

`ProviderForm::NAME: FormField<ProviderForm, String>` 是纯类型化 lens 与 schema descriptor，
而不是 form handle。每个静态声明的 model field 都有一个这样的 associated const。它可直接复用
为只包含静态 schema/access 信息的微小 descriptor。访问它不会构造每个 form 或 field 的 state，也
不会进行 allocation；descriptor 不保存 value、subscription、`Entity<ProviderForm>` 或
`WeakEntity<ProviderForm>`。

一个编辑会话只创建这一个 state entity：

```rust,ignore
use gpui::{AppContext as _, Entity};
use gpui_form::FormState as _;

let form: Entity<ProviderForm> = cx.new(|cx| ProviderForm::from_value(
    ProviderInput {
        name: String::new(),
        retry_limit: 3,
        model_id: None,
    },
    cx,
));
```

当 `ValidationContext: Default` 时使用 `from_value`；否则使用
`from_value_with_validation_context(initial, context, cx)`。构造过程先安装 model 与 context，
随后恰好运行一次 mount validation。

## 3. Total descriptor 与 partial descriptor

绝大多数 field path 在类型上就确定存在。普通 `FormField<Form, T>` 是 **total**：每个同步 API
都显式接收强 form entity，并且没有结构性 `Result`。

```rust,ignore
let current: String = ProviderForm::NAME.value(&form, cx);
let issues = ProviderForm::NAME.errors(&form, cx);
let validating = ProviderForm::NAME.is_validating(&form, cx);

ProviderForm::NAME.set(&form, "OpenAI".to_owned(), cx);
ProviderForm::NAME.validate(&form, ValidationTrigger::Dynamic, cx);
```

写入只有一个动词：`set`。值变化时，它存入类型化值、只推进一次 revision、失效相交的验证、
运行该字段的 change validation、发出一个 event 并 notify 一次；相等写入是完整 no-op。
库不再公开 `set_user_value`。

identified item 或计算 projection 可能消失，因此返回 `PartialFormField<Form, T>`，只提供
`try_*` API：

```rust,ignore
let partial_parent = ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(partial_parent);

let name = header_name.try_value(&form, cx)?;
header_name.try_set(&form, "Authorization".to_owned(), cx)?;
let issues = header_name.try_errors(&form, cx)?;
```

`FieldAccessError` 表示 projection 不可用，或 stable item 缺失/重复。`FieldMutationError`
包装 access error，并拒绝把已捕获 stable ID 改成其他值的写入。两者都没有 `FormReleased`：
同步调用者已经传入 `&Entity<Form>`，form liveness 已经得到保证。

`AuthForm::USERNAME.within(ServerForm::AUTH)` 是 total：组合保留静态 parent 的 total 可用性。
`ServerForm::HEADERS.item(id)` 是运行时寻址的 `PartialFormField`；因此
`HeaderRowForm::NAME.within(partial_parent)` 仍是 partial。`within` 保留 parent descriptor 的
total/partial 可用性；`project_value` 和 `item` 创建 partial descriptor；partial descriptor 的
所有 child 仍然是 partial。即使输入是静态 descriptor 或 total 的组合，`project_value` 仍是 partial。
`within` 与 `item` 创建轻量定位 descriptor，而不是新的 schema 定义。实现这一规则的 marker 是内部
细节；公开文档只使用 `FormField` 与 `PartialFormField`，不要求用户书写 marker 泛型。

## 4. 创建绑定控件

组件适配器一次创建 native component state 并完成绑定。form 显式传入；total field 的构造除组件
本身的领域错误外是 infallible：

```rust,ignore
use gpui_component::input::InputState;
use gpui_form_gpui_component::{FormInput, FormIntegerInput, IntegerInputState};

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
)?; // 不合法 integer bounds 仍是实际的组件错误
```

`FormInput::try_new(&form, partial_field, ...)` 对应 `PartialFormField`，可以返回
`FieldAccessError`。bound control 是普通 Rust newtype，字段严格为先
`subscriptions: Vec<Subscription>`、后 native `Entity<State>`，并 deref 到该 entity。它不保存
form、descriptor、`ControlBinding`、value snapshot、option、focus flag 或 editor text。

binding 在 native entity event 已开始之后，才拥有所需的 weak relation：它把类型化写入 defer 到
emitter update 结束后，届时升级 form；若 form 或 partial path 已消失便静默取消。直接同步的页面代码
始终传强 entity，不会看到这条 weak-lifetime 路径。

页面/controller 生命周期内只需 observe form 一次，即可重渲染 label、验证反馈和按钮：

```rust,ignore
let form_subscription = cx.observe(&form, |_, _, cx| cx.notify());
```

## 5. 渲染 field 与 form runtime

schema 位于静态 descriptor，runtime state 由显式 form 提供：

```rust,ignore
let error = ProviderForm::NAME.errors(&self.form, cx)
    .first()
    .map(|issue| validation_text(&issue.message, cx));
let is_validating = ProviderForm::NAME.is_validating(&self.form, cx);

field()
    .label("Provider name")
    .required(ProviderForm::NAME.schema().is_required())
    .child(Input::new(&self.name_input));
```

form-level query 仍包括 `is_dirty`、`is_valid`、`is_validating`、`validation_report`、
`errors_at`、`first_error_path` 和 `revision`。form 不持有 focus、touched、blurred 或 error-
visibility state。submit 失败后，由 active page 选择要 focus 的可见 native control。

无状态的类型化 element 使用同一个 total descriptor：

```rust,ignore
let form = self.form.clone();
let checked = ProviderForm::ENABLED.value(&form, cx);

Checkbox::new("provider-enabled")
    .checked(checked)
    .on_click(move |checked, _, cx| ProviderForm::ENABLED.set(&form, *checked, cx));
```

仅当 callback 不在 native state entity 的 active update 内时，才能直接写入。所有有状态 component
event 都必须通过 adapter 的 deferred `ControlBinding`，避免 GPUI reentrancy。

Options/catalog 是配置而非 form data：更新应用 store，更新或重建 native delegate，静默投影权威
form value；必要时显式执行 dynamic validation。options refresh 绝不选择第一个值、改写/rebase form
或隐式持久化。

## 6. Whole-form lifecycle 与 revision

安装应用数据时使用 lifecycle operation：

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved, cx));
```

`replace` 改 current value 而保留 baseline；`reset` 恢复 baseline；`rebase` 同时安装 current value
与 baseline。每次 lifecycle operation 即使写入相等 Rust value 也会推进 revision、取消受影响的
async work、清理过期数据验证并触发 value reproject；但不会伪造逐字段 change validation。

`rebase_if_revision(expected, saved, cx)` 是唯一的异步保存 merge primitive。比较失败不会影响
draft、baseline、revision、report、task 或 control；成功会推进 revision，因此相同 submitted revision
的两个结果不可能都生效。

## 7. Validation

### Trigger、path 与 scoped bucket

支持 mount、change、blur、dynamic、submit trigger。`required` 总会参与 submit，也可选择更早的
trigger。trim 后为空的字符串、`None`、空的受支持 collection、`false` 是 missing；数字与 enum
没有隐式 missing 语义。

`ValidationScope::Field(path)` 包含变更 path、其 descendant 和 ancestor group/array path，但不含
sibling leaf。group/identified-item scope 包含自己的 subtree 加 ancestor；`Form` 包含全部 data path。

runtime 按已经 normalize 的 source-and-path bucket 存储 adapter result。一轮 scoped validation
只替换同时被 scope 与 trigger 选中的 bucket，并保留 sibling adapter issue。adapter 的 form-level
issue 只可在 `ValidationScope::Form` 中出现，并且只由另一轮 form-wide run 替换。control issue
仍使用自己生命周期相关的 bucket。

core 总是在 scope/trigger filtering 之前，根据 model snapshot 解析 adapter path。未知、畸形、重复
或无法转换的 stable path 会变为 blocking internal form issue，不能静默消失。schema ownership 精确：
array 拥有 container 与 direct item root；nested item leaf 使用自己的 schema trigger。

### Garde 与 custom adapter

Garde 用于同步 model/business rule；空值语义保留在 `#[form(required)]`，不要用 Garde 重复：

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormModel, garde::Validate)]
#[form(state = AccountForm, validation(adapter = "garde"))]
#[garde(allow_unvalidated)]
struct AccountInput {
    #[form(required, validate(on_change, on_blur))]
    #[garde(skip)]
    display_name: String,

    #[form(validate(on_change, on_blur, on_dynamic, on_submit))]
    #[garde(email)]
    email: Option<String>,
}
```

外部依赖放入类型化 validation context，并显式替换该 context。Garde message provider 是
static type-level policy：把 Garde rule 映射为 `ValidationMessage`，既不保存实例，也不通过
`Default` 构造。应用可以返回稳定 key/params，并在渲染时使用当前 locale 翻译。

custom `ValidationAdapter<Model>` 是生成 state 选择的静态 associated policy，不会保存为实例，
也不通过 `Default` 构造。它直接接收类型化 validation context，并必须把
`ValidationScope` 作为自己报告 bucket 的边界：

```rust,ignore
impl ValidationAdapter<ProviderInput> for ProviderValidator {
    type Context = ProviderValidationContext;

    fn validate(
        value: &ProviderInput,
        trigger: ValidationTrigger,
        scope: &ValidationScope,
        context: &Self::Context,
        cx: &App,
    ) -> ValidationAdapterReport {
        let mut report = ValidationAdapterReport::default();
        let path = ProviderForm::MODEL_ID.path().clone();
        if scope.includes(&path)
            && value.model_id.as_ref().is_some_and(|id| !context.model_ids.contains(id))
        {
            report.push(ValidationIssue::field(
                path, trigger,
                ValidationSource::App("provider".into()),
                "model_unavailable", ValidationMessage::key("provider-model-unavailable"),
            ));
        }
        report
    }
}
```

core 仍会 normalize 返回 path，并强制 scope/trigger ownership；使用 scope 既能优化性能，也能让
adapter 不报告无关 sibling。

### Async validation 与 control issue

page 决定何时发起 remote check；一旦开始，form 持有 task、generation 和 scope check：

```rust,ignore
ProviderForm::NAME.start_async_validation(
    &self.form,
    "provider-name",
    ValidationTrigger::Change,
    move |name| async move { service.check_name(name).await },
    cx,
);
```

相交写入、取消、lifecycle operation 或 form drop 会取消 task；过期 completion 无法覆盖新的 state。
活跃的 form-owned async validation 阻塞 `prepare_submit`；非阻塞 remote hint 属于应用 UI。

native editor 暂时无法产生 `T` 时，将 raw text 保存在本地，并通过 `ControlBinding` 发布一个
lifecycle-scoped control issue。该 issue 只在 binding mounted 期间阻塞提交。

## 8. Prepare submit 与持久化

`prepare_submit` 对一个 snapshot 验证，拒绝 validation/control/pending async issue，并恰好转换该
snapshot 一次：

```rust,ignore
use gpui_form::PreparedSubmit;

let PreparedSubmit { revision, output } = self.form.update(cx, |form, cx| {
    form.prepare_submit(cx)
})?;
```

`PreparedSubmit` 避免调用者从不同 model version 读取 revision 与 output。
`SubmitTransform<Model>` 是由生成 state 选择的静态、不可失败 transform：

```rust,ignore
struct ProviderTransform;

impl SubmitTransform<ProviderInput> for ProviderTransform {
    type Output = SaveProvider;

    fn transform(model: &ProviderInput) -> SaveProvider {
        SaveProvider {
            name: model.name.trim().to_owned(),
            retry_limit: model.retry_limit,
        }
    }
}
```

不存在 `TransformReport` 或 transform failure variant。应内联显示的条件属于 validation；
remote/provider/database failure 属于应用 operation。持久化成功后，以
`rebase_if_revision(revision, saved, cx)` 应用 canonical saved value。

## 9. Nested model、array 与 projection

Nested data 保留在一个 root entity 中；静态 associated-const descriptor 可以组合，不创建 child form、
不捕获 root entity，也不实例化 field state：

```rust,ignore
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
let username = username.value(&form, cx);

let partial_parent = ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(partial_parent);
let header_name = header_name.try_value(&form, cx)?;
```

stable ID 在 array 内唯一，并且通过 identified-item descriptor 不可改写。whole-array `set` 是插入、
删除、替换和排序的显式操作。库不会选择第一个 duplicate，也不会静默修复 ID。

`project_value` 为 control 和 async issue 生成独立 projection path，同时保留最近的真实 model path
作为 validation path；它总是 partial，包括直接在静态 descriptor 或 total 组合上调用时：

```rust,ignore
let budget = JobForm::RUN_SETTINGS.project_value(
    "token_budget",
    |settings| settings.custom_token_budget(),
    |settings, value| settings.set_custom_token_budget(value),
);
let budget = budget.try_value(&form, cx)?;
```

## 10. 实现 custom stateful control

不再有 `FormControl` trait。custom control 的固有构造函数显式接收 form，并在长期 callback 边界
创建 `ControlBinding`：

```rust,ignore
pub struct FormRating {
    subscriptions: Vec<Subscription>,
    rating: Entity<RatingState>,
}

impl FormRating {
    pub fn new<Owner>(
        form: &Entity<ReviewForm>,
        field: FormField<ReviewForm, Rating>,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self {
        let binding = field.bind_control(form, cx);
        // Build and silently project initial value. Capture binding clones only
        // in event/projection subscriptions; do not retain it as a struct field.
        todo!()
    }
}
```

`ControlBinding` 可 clone，并持有内部 control lease。其公开 deferred intent 为 `defer_set`、
`defer_blur`、`defer_set_issue` 和 `defer_clear_issue`。它不暴露 weak form handle、immediate
mutation、control ID 或 component read-back。partial field 使用 `try_bind_control`。

descriptor subscription 会在 `ValueChanged` path 可能影响该 descriptor 时，以及每次
`ModelReplaced` 后静默重投影当前 field value，其中也包括该 control 自己编辑之后；
`ValidationChanged` 不参与 value projection。不要加入 origin skipping。

## 11. 相关文档

- [项目文档索引](README.md)
- [gpui-form-macros guide](../../gpui-form-macros/docs/guide.md)
- [gpui-form-gpui-component guide](../../gpui-form-gpui-component/docs/guide.md)
