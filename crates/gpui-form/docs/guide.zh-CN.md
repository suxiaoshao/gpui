# gpui-form 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

本指南说明 form 编辑 session 的公开契约。先以 provider settings 页面为例，再扩展到 optional data 与
recursive collection。声明语法见[宏使用指南]；具体的 `gpui-component` adapter 与 custom control 见
[component adapter 使用指南]。

[宏使用指南]: ../../gpui-form-macros/docs/guide.zh-CN.md
[component adapter 使用指南]: ../../gpui-form-gpui-component/docs/guide.zh-CN.md

代码片段沿用 [README] 中的依赖与 import，并省略 `start_save`、`reconcile_query_rows` 等与示例无关的
页面 method。

[README]: ../README.zh-CN.md

## 1. 所有权：一个 form session，每次访问都显式传入

每次编辑 session 创建一个 `Entity<Form<M>>`。它持有 current typed model、baseline、validation fact 与
dynamic field location。页面持有 save/load operation、button policy、error visibility、focus 与展示。native
control 持有 IME、cursor、selection、popup state 和 incomplete text 等编辑器状态。

schema descriptor 不持有 session，因此每个 descriptor 可以复用：

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_blur, on_submit))]
    name: String,
    #[form(validate(on_submit))]
    retry_limit: u32,
    enabled: bool,
}

let form = cx.new(|_| {
    Form::new(ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    })
    .with_validator(ProviderValidator::new(reserved_names))
});
```

`Form::new` 和 `with_validator` 都是 infallible。为 `name` 生成的 descriptor 是
`ProviderDraft::NAME` 这类静态值；每次读取或修改都显式传入 `&form`。

## 2. 普通字段使用 total path

root field descriptor，或只穿过 `#[form(child)]` field 组合出的 path，都是 `TotalPath<Root, T>`。它在整个
编辑 session 内存在，包括 `replace`、`reset` 或 `rebase` 之后：

```rust,ignore
let name = ProviderDraft::NAME;

let current: String = name.get(&form, cx);
let changed: bool = name.set(&form, "primary".into(), cx);

let enabled = ProviderDraft::ENABLED;
enabled.set(&form, true, cx);
```

`set` 返回 model 是否变化。普通等值写入是 model no-op。不要在页面中再保存一份 value；render 时重新读取
Form。

静态嵌套也保持这个性质：

```rust,ignore
#[derive(Clone, FormSchema)]
struct AuthDraft { username: String }

#[derive(Clone, FormSchema)]
struct RedirectDraft { callback_url: String }

#[derive(Clone, FormSchema)]
struct ServerDraft {
    #[form(child)]
    auth: AuthDraft,
    #[form(child)]
    redirect: Option<RedirectDraft>,
}

let server_form = cx.new(|_| Form::new(ServerDraft {
    auth: AuthDraft { username: String::new() },
    redirect: None,
}));
let username = ServerDraft::AUTH.then(AuthDraft::USERNAME);
let current: String = username.get(&server_form, cx);
```

## 3. item、case 与 optional payload 使用 dynamic path

item、enum case 或 `Option::Some` 边界会产生 `DynamicPath<Root, T>`。它的 `try_get`、`try_set` 与 adapter
构造函数在 path 属于另一个 form session 或已经 retire 时返回 `ResolveError`。

resolve 未激活的 enum case 或不存在的 optional child 是正常情况，返回 `Ok(None)`，不是 error。起始 path
已经 retire 时才返回 `Err(ResolveError)`：

```rust,ignore
let condition = node
    .then(FilterNode::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&query_form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterCondition::VALUE);
    let current: String = value.try_get(&query_form, cx)?;
    value.try_set(&query_form, "Rust".to_owned(), cx)?;
}

let redirect = ServerDraft::REDIRECT
    .some()
    .resolve(&server_form, cx)?;

if let Some(redirect) = redirect {
    redirect
        .then(RedirectDraft::CALLBACK_URL)
        .try_set(&server_form, "https://example.com/callback".into(), cx)?;
}
```

resolved path 的返回类型决定 value type。对 `set` 或 `try_set` 传入错误的 Rust type 会被类型系统拒绝。

## 4. collection 与 recursive typed tree

结构化 `Vec<T>` 使用 `#[form(items)]`。业务 model 只包含业务数据：每个 item occurrence 的 identity 由 Form
生成。`ItemPath` 只能通过枚举 Form 或 collection mutation 获得：

```rust,ignore
#[derive(Clone, FormSchema)]
struct HeaderDraft { name: String }

#[derive(Clone, FormSchema)]
struct RequestDraft {
    #[form(items)]
    headers: Vec<HeaderDraft>,
}

let request_form = cx.new(|_| Form::new(RequestDraft { headers: Vec::new() }));
let headers = RequestDraft::HEADERS;
let header = headers.append(
    &request_form,
    HeaderDraft { name: String::new() },
    cx,
)?;

header
    .then(HeaderDraft::NAME)
    .try_set(&request_form, "Authorization".into(), cx)?;
```

collection method 包括 `items`、`try_items`、`append`、`insert_before`、`move_before`、`remove`、
`replace_all` 与 `ItemPath::move_to`。它们使用 typed item path，不使用 raw ID、business ID 或 index。
`ItemPath::key()` 在 item 保持 active 时是稳定的 opaque UI key。

这套方式可以扩展到 recursive filter tree，而无需向 model 添加 ID：

```rust,ignore
#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
enum FilterNode {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
struct FilterCondition { value: String }

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode::Condition(FilterCondition {
            value: String::new(),
        })],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
for node in children.items(&query_form, cx) {
    let condition = node
        .case(FilterNode::CONDITION)
        .resolve(&query_form, cx)?;

    if let Some(condition) = condition {
        let value = condition.then(FilterCondition::VALUE);
        render_condition(node.key(), value.try_get(&query_form, cx)?);
    }
}
```

同父级重排保留 item path。删除并重新插入 item、重建 case 或 optional payload、替换 collection、
replace/reset/rebase 整个 Form，以及跨父级移动，都会使旧 dynamic path retire。path 不会悄悄解析到之后位于
同一表面位置的 value。

## 5. 连接控件并渲染页面

普通 `gpui-component` control 使用内置 adapter：

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

let name_input = FormInput::new(
    &form,
    ProviderDraft::NAME,
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
);
let retry_limit_input = FormIntegerInput::new(
    &form,
    ProviderDraft::RETRY_LIMIT,
    |window, cx| IntegerInputState::new(window, cx).min(0u32).max(10u32),
    window,
    cx,
)?;
let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
```

`new` 接收 total path。`try_new` 接收已经 resolve 的 dynamic path，且可能因 location 已 retire 而失败。
内置控件自行保存 binding 并订阅 form change；只有页面本身需要 rerender 时，才保存 `form_observer`。

render code 分别使用 schema metadata 与 live form fact：

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let pending = self.form.read(cx).is_validating();

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(error_text(errors.first(), pending))
    .child(Input::new(&self.name_input));
```

页面决定 error 是否可见，以及 submit 失败后 focus 哪个可见 control。Form 只报告事实，不持有 touched state、
focus 或 layout。

自定义的有状态控件使用 [component adapter 使用指南]中的 `ControlBinding`、`ControlWriter` 与
`ControlProjection` 协议。控件静默投影 `Value`，从 native event 通过 writer 写回，并处理一次性的
`Retired` projection。它不手工订阅 `FormEvent`，也不使用本地的双向绑定布尔开关。

## 6. Validation

validator 面向单个 `ValidationRequest` 编写，它同时提供 current model 与 snapshot-bound path resolution：

```rust,ignore
impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        let model = request.model();

        if request.includes(&ProviderDraft::NAME) && model.name.trim().is_empty() {
            out.at(ProviderDraft::NAME).error(
                "provider-name-empty",
                ValidationMessage::key("provider-name-empty"),
            );
        }
    }
}
```

递归 validation 只使用同一个 request 创建的 path。这些 path 可以继续类型化组合并传给 `out.at`，但不提供
mutation 或 control binding 方法，也不能逃逸出本次 validation snapshot：

```rust,ignore
fn validate_filter_nodes<'a>(
    request: &ValidationRequest<'a, QueryDraft>,
    nodes: Vec<ValidationItemPath<'a, QueryDraft, FilterNode>>,
    out: &mut ValidationSink<'_, QueryDraft>,
) {
    for node in nodes {
        if let Ok(Some(condition)) = node
            .clone()
            .case(FilterNode::CONDITION)
            .resolve(request)
        {
            let value = condition.then(FilterCondition::VALUE);
            if request
                .try_get(&value)
                .is_ok_and(|value| value.trim().is_empty())
            {
                out.at(value).error(
                    "filter-value-empty",
                    ValidationMessage::key("filter-value-empty"),
                );
            }
            continue;
        }

        if let Ok(Some(group)) = node.case(FilterNode::GROUP).resolve(request) {
            let children = group.then(FilterGroup::CHILDREN);
            if let Ok(children) = request.try_items(&children) {
                validate_filter_nodes(request, children, out);
            }
        }
    }
}

impl Validator<QueryDraft> for QueryValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<'_, QueryDraft>,
    ) {
        let children = QueryDraft::ROOT
            .then(QueryDraft::FILTERS)
            .then(FilterGroup::CHILDREN);
        validate_filter_nodes(&request, request.items(&children), out);
    }
}
```

业务 validation 默认 trigger 是 `Submit`。需要即时反馈时，再通过 schema validation metadata 添加
`Mount`、`Change` 或 `Blur`。catalog、permission 或其他外部依赖变化后，显式请求 scope 化的 validation：

```rust,ignore
ProviderDraft::NAME.validate(&form, ValidationTrigger::External, cx);
```

异步 validation 也只会针对 total path 或当前已 resolve 的 dynamic path 被显式发起。相交写入、retirement、
lifecycle change 或更新的 validation run 发生后，Form 会取消或丢弃旧 result。Form own 的 pending async
validation 会让 `prepare` 不能成功。

## 7. Replace、reset、prepare 与 save

应用数据改变时使用 lifecycle method：

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next_draft, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved_draft, cx));
```

- `replace` 安装新的 current draft，并保留 baseline。
- `reset` 把 baseline 恢复为 current draft。
- `rebase` 把一个 model 同时安装为 current draft 与 baseline。

每个 lifecycle operation 都是语义上的 model change，即使 Rust value 相等。total path 保持有效；旧 dynamic
path 及其 pending work 会 retire。

在应用 own 的 I/O 前 prepare value，并保存其 version，而不是裸 revision number：

```rust,ignore
let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;

let (version, request) = prepared
    .map(SaveProvider::from)
    .into_parts();
self.start_save(version, request, cx);

// save 成功后的稍后时刻：
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_saved_model, cx)
});
```

`prepare` 对一个 snapshot 运行 submit validation，并拒绝 blocking issue 或 pending async validation。
`Prepared::map` 保留 session-bound `FormVersion`。`rebase_if_current` 返回 `false` 时，draft 未被改变：应由
应用展示“保存期间仍在编辑”的相应 UI。

## 8. 有选择地观察语义变化

页面重绘使用普通 entity observation。只有 owner 需要选择性 side effect（如 reconcile recursive row tree）时，
才订阅 `FormEvent`：

```rust,ignore
let subscription = cx.subscribe(&query_form, |_, _, event, cx| {
    match event {
        FormEvent::ModelChanged(change) => {
            let target = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroup::CHILDREN);
            let impact = change.impact(&target);

            if impact.structure_changed() || impact.retired() {
                reconcile_query_rows(cx);
            } else if impact.value_changed() {
                cx.notify();
            }
        }
        FormEvent::ValidationChanged { .. } => cx.notify(),
    }
});
```

`ModelChangeKind` 区分 edit、replace、reset 与 rebase。`PathImpact` 回答选择的 target 的 value 是否变化、
structure 是否变化、是否 retire。event 不带 control origin 或实现 identity：控件已经自行处理同步，其他 owner
只消费与自己 target 有关的语义效果。

## 相关文档

- [README](../README.zh-CN.md)
- [宏使用指南]
- [Component adapter 使用指南]
