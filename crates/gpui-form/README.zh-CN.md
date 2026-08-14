# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form` 为一个 GPUI 编辑页面提供一份类型化 Rust draft、校验，以及可安全保存的 snapshot。`Form<M>`
是一次编辑 session，而不是第二份应用 store：页面持有持久化、加载和展示；Form 持有 current draft、baseline、
validation fact 与动态字段的 session-local 位置。

## 添加 crate

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

component crate 是可选的，但通常用它连接标准 `gpui-component` input。以下示例使用这些 import：

```rust,ignore
use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    form::field,
    input::{Input, InputState},
};
use gpui_form::{
    Form, FormEvent, FormSchema, Prepared, ValidationMessage,
    ValidationItemPath, ValidationRequest, ValidationSink, ValidationTrigger, Validator,
};
use gpui_form_gpui_component::{FormInput, FormIntegerInput, IntegerInputState};
```

## 一张完整的小表单

### 描述 draft

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_blur, on_submit))]
    name: String,

    #[form(validate(on_submit))]
    retry_limit: u32,

    enabled: bool,
}
```

`FormSchema` 会创建可复用的静态 descriptor，例如 `ProviderDraft::NAME`。descriptor 只包含 schema
metadata 与 typed access；它不会保留 form entity、value、subscription 或 native control。root descriptor
也是 total path，所以它的 `get` 与 `set` 操作不会失败。

### 创建编辑 session

构造函数是 infallible。先提供初始 draft，再按需为本次编辑 session 附加 validator：

```rust,ignore
let form: Entity<Form<ProviderDraft>> = cx.new(|_| {
    Form::new(ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    })
    .with_validator(ProviderValidator::new(reserved_names))
});
```

同一份 schema 可以创建多个独立 session，并拥有不同 validator data。更新应用 catalog 不会改写 Form；当
外部事实会影响规则时，catalog owner 显式请求校验。

### 编写 validator

校验会收到一个自洽的 snapshot。通过 request 读取 model，并把 issue 附着到精确的 typed path：

```rust,ignore
struct ProviderValidator {
    reserved_names: Arc<HashSet<String>>,
}

impl ProviderValidator {
    fn new(reserved_names: Arc<HashSet<String>>) -> Self {
        Self { reserved_names }
    }
}

impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        let model = request.model();

        if request.includes(&ProviderDraft::NAME)
            && self.reserved_names.contains(model.name.trim())
        {
            out.at(ProviderDraft::NAME).error(
                "provider-name-reserved",
                ValidationMessage::key("provider-name-reserved"),
            );
        }
    }
}
```

业务校验默认在 `Submit` 时运行。schema 声明 `Mount`、`Change` 或 `Blur` 规则后，才会启用相应 trigger。
catalog 或其他外部依赖变化时使用 `ValidationTrigger::External`；它和 `DynamicPath` 没有关系。

### 连接控件并重绘页面

普通控件直接使用内置 adapter。它们自行订阅 Form，并处理两个方向的同步：

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

impl ProviderPage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let form = cx.new(|_| Form::new(ProviderDraft {
            name: String::new(),
            retry_limit: 3,
            enabled: true,
        }));

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

        // 这里只负责重绘页面，不是让控件保持同步所必需的订阅。
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self { form, form_observer, name_input, retry_limit_input })
    }
}
```

`FormInput`、`FormIntegerInput`、`FormSelect` 和 `FormCombobox` 自己持有 binding 与 native subscription。
页面 observer 只用于渲染 error、dirty state 或按钮可用性等页面状态。

无状态 callback 也显式把 Form 传给同一个 descriptor：

```rust,ignore
let enabled = ProviderDraft::ENABLED;
let checked = enabled.get(&self.form, cx);
let form = self.form.clone();

Checkbox::new("provider-enabled")
    .checked(checked)
    .on_click(move |checked, _, cx| {
        enabled.set(&form, *checked, cx);
    });
```

## 校验、保存并有条件地 rebase

`prepare` 会对一个 snapshot 运行 submit validation。成功时返回 `Prepared<M>`，其中同时包含值和
session-bound `FormVersion`：

```rust,ignore
struct SaveProvider(ProviderDraft);

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self(draft)
    }
}

let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;

let (version, request) = prepared
    .map(SaveProvider::from)
    .into_parts();
self.start_save(version, request, cx);

// 页面持有的 async completion callback 中：
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_saved_model, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`Prepared::map` 会保留同一个 version。用户在 prepare 后继续编辑，或 version 属于另一个 session 时，
`rebase_if_current` 不会改变任何内容。保存、retry、notification 和错误展示仍由应用负责。

## 嵌套与动态数据

嵌套 schema 使用 `#[form(child)]`，结构化 collection 使用 `#[form(items)]`。下面这个完整的递归
model 不包含 Form 专用 ID：

```rust,ignore
#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    title: String,
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
enum FilterNode {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
}

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        title: "All articles".into(),
        children: Vec::new(),
    },
}));

let title = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::TITLE);
let value: String = title.get(&query_form, cx);
title.set(&query_form, "Recent articles".into(), cx);
```

item、enum case 和 `Option::Some` 位置是 dynamic。Form 生成它们的 identity；调用方不生成 ID，也不按数组
下标导航。针对当前 Form resolve case 或 optional payload。未激活的 case/option 返回 `Ok(None)`；起点已
retire 则返回 `Err(ResolveError)`：

```rust,ignore
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
let node = children.append(
    &query_form,
    FilterNode::Condition(FilterCondition { value: String::new() }),
    cx,
)?;
let key = node.key(); // item 仍 active 时稳定的 UI identity

let condition = node
    .case(FilterNode::CONDITION)
    .resolve(&query_form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterCondition::VALUE);
    let current: String = value.try_get(&query_form, cx)?;
    value.try_set(&query_form, "Rust".into(), cx)?;
}
```

使用 `children.items(&query_form, cx)` 枚举已有 item。每个返回的 `ItemPath` 都是类型化的，并携带
Form 持有的当前位置。

同父级重排保留 item path。删除、替换、case/optional 重建、whole-form replacement 与 cross-parent move 会使
受影响的 dynamic path retire；它们不会在看起来相同的位置复活。

## 需要时观察语义变化

大多数页面只需 `cx.observe` 来重绘。tree reconciler 或其他 cross-field owner 可以订阅语义化 Form event，
并查询自身的 typed target 是否受影响：

```rust,ignore
let subscription = cx.subscribe(&form, |_, _, event, cx| {
    if let FormEvent::ModelChanged(change) = event {
        let children = QueryDraft::ROOT
            .then(QueryDraft::FILTERS)
            .then(FilterGroup::CHILDREN);
        let impact = change.impact(&children);

        if impact.structure_changed() {
            cx.notify(); // 重新枚举 row
        } else if impact.value_changed() {
            cx.notify(); // 重新读取已有 value
        }
    }
});
```

`PathImpact` 也会报告 retirement。仅 validation 的变化以 `FormEvent::ValidationChanged` 单独到达；它不表示
必须重新设置 native control value。

## 继续阅读

- [使用指南](docs/guide.zh-CN.md)：lifecycle operation、validation、submission、recursive collection 与
  event handling。
- [宏使用指南](../gpui-form-macros/docs/guide.zh-CN.md)：schema declaration、enum case 与编译期
  diagnostic。
- [Component adapter 使用指南](../gpui-form-gpui-component/docs/guide.zh-CN.md)：内置控件及 custom
  control binding API。
