# gpui-form-gpui-component 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

## 开始之前

添加 runtime、native components 与 adapters：

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

下文片段使用这个公共 prelude。后文的 `ModelDelegate`、`TagDelegate` 与自定义
`SlugInput*` 类型属于应用。

```rust,ignore
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    combobox::{Combobox, ComboboxState},
    input::{Input, InputState},
    select::{Select, SelectState},
    switch::Switch,
};
use gpui_form::{DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError};
use gpui_form_gpui_component::{
    FormCombobox, FormInput, FormIntegerInput, FormSelect, IntegerInput,
    IntegerInputState,
};
```

下文示例共用这些普通 Rust draft 类型：

```rust,ignore
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ModelId(String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TagId(String);

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    #[form(required)]
    name: String,
    model_id: Option<ModelId>,
    enabled: bool,
}

#[derive(Clone, FormSchema)]
struct JobDraft {
    budget: u64,
    tag_ids: Vec<TagId>,
}

#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    root: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
struct FilterNode {
    #[form(child)]
    kind: FilterNodeKind,
}

#[derive(Clone, FormSchema)]
enum FilterNodeKind {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
    limit: u64,
    model_id: Option<ModelId>,
    tag_ids: Vec<TagId>,
}
```

为每次编辑会话创建一个 strong form entity。下文分别使用 `form`、`job_form` 与
`query_form`：

```rust,ignore
let provider_runtime = Form::try_new(ProviderDraft {
    name: String::new(),
    model_id: None,
    enabled: true,
})?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| provider_runtime);

let job_runtime = Form::try_new(JobDraft {
    budget: 1_024,
    tag_ids: Vec::new(),
})?;
let job_form: Entity<Form<JobDraft>> = cx.new(|_| job_runtime);

let query_runtime = Form::try_new(QueryDraft {
    root: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: String::new(),
                limit: 10,
                model_id: None,
                tag_ids: Vec::new(),
            }),
        }],
    },
})?;
let query_form: Entity<Form<QueryDraft>> = cx.new(|_| query_runtime);

let condition_node = QueryDraft::ROOT
    .then(FilterGroup::CHILDREN)
    .items(&query_form, cx)?
    .into_iter()
    .next()
    .expect("示例包含一个 condition");
let condition: DynamicPath<QueryDraft, FilterCondition> = condition_node
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::CONDITION)?;
```

target 始终存在时使用 `new`。直接传入 `ProviderDraft::NAME` 这样的 root definition，或
已经组合好的 `TotalPath<M, T>`。

root `FieldDef<M, T>` 本身也直接提供 total-path façade，包括 `value`、`set`、`errors` 与
validation query。

路径经过 item、enum case 或 optional child 后，target 在 current form 中可能不存在；此时用
`DynamicPath<M, T>` 调用 `try_new`。case 与 option 先通过
`try_case(form_entity.read(cx), case_def)` 或 `try_some(form_entity.read(cx))` 定位；adapter 永远不接收
`TopologyIndex`。

Form 为每个 item occurrence 生成并持有 identity。遍历与 topology operation 返回 typed located
item path，renderer 再从中组合出上面的 `condition`。model、page 与 adapter 都不创建或查询 item ID。
located path 只对 Form 返回它时对应的 occurrence 与 active case 有效。

## 把 Input 绑定到 total path

```rust,ignore
let name_input = FormInput::new(
    &form,
    ProviderDraft::NAME,
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
);

let element = Input::new(&name_input);
```

`InputEvent::Change` defer typed `String` write；`InputEvent::Blur` defer blur
validation。Form commit 静默调用原生 value setter。

由于 `ProviderDraft::NAME` 是 total，`FormInput::new` 没有 path resolution `Result`。

## 把 Input 绑定到 dynamic path

```rust,ignore
let value: DynamicPath<QueryDraft, String> =
    condition.clone().then(FilterCondition::VALUE);

let value_input = FormInput::try_new(
    &query_form,
    value,
    |window, cx| InputState::new(window, cx).placeholder("Condition value"),
    window,
    cx,
)?;
```

item 已 retire 或 condition case 不活跃时 mount 失败。renderer 应以 dynamic location 的 UI key
持有 adapter；renderer 不再收到该 location 时 drop adapter，新 location 出现后重新调用 `try_new`，
不要把旧 control 重定向过去。retired path 的 queued work 会静默 no-op。

## 把 Integer 绑定到 total path

```rust,ignore
let budget_input = FormIntegerInput::new(
    &job_form,
    JobDraft::BUDGET,
    |window, cx| {
        IntegerInputState::new(window, cx)
            .min(1_024u64)
            .max(1_000_000u64)
            .step(1_024u64)
    },
    window,
    cx,
)?;

let element = IntegerInput::new(&budget_input);
```

form value 始终为 `u64`。原生 entity 持有私有 editor text。不完整、非法、溢出或超范围 text
留在 native state 并发布 leased control issue；只有合法 typed input 才 defer 到 form。
checked arithmetic 不会把 integer 路由经过 `String` 或 `f64`。

total constructor 可以返回 native integer-policy error，但不会返回 path resolution error。

## 把 Integer 绑定到 dynamic path

```rust,ignore
let limit: DynamicPath<QueryDraft, u64> =
    condition.clone().then(FilterCondition::LIMIT);

let limit_input = FormIntegerInput::try_new(
    &query_form,
    limit,
    |window, cx| IntegerInputState::new(window, cx).min(0u64).step(1u64),
    window,
    cx,
)?;

let element = IntegerInput::new(&limit_input);
```

`FormIntegerInputBuildError` 用 `Resolve` 与 `Policy` 区分 path 无法解析和 integer policy 非法。

## 把 Select 绑定到 total path

`FormSelect<D>` 绑定 `Option<D::Item::Value>`，并且只在 `SelectEvent::Confirm` 后写入：

`ModelDelegate` 是应用定义的 native select delegate，其 item value 为 `ModelId`。
下文 `provider_models` 与 `condition_models` 是应用分别持有的 options snapshot。

```rust,ignore
let provider_model_select = FormSelect::new(
    &form,
    ProviderDraft::MODEL_ID,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(provider_models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
);

let element = Select::new(&provider_model_select);
```

每次 silent projection 都使用 native state 当前的 delegate。adapter 不保留第二份 delegate 或
value/index map。

## 把 Select 绑定到 dynamic path

```rust,ignore
let model_id: DynamicPath<QueryDraft, Option<ModelId>> =
    condition.clone().then(FilterCondition::MODEL_ID);

let condition_model_select = FormSelect::try_new(
    &query_form,
    model_id,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(condition_models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Select::new(&condition_model_select);
```

`try_new` 在创建 native state 前解析 current item 与 case。dynamic location 后续消失时，
旧 binding 不能写入同一地址新建的 object。

## 把 Combobox 绑定到 total path

`FormCombobox<D>` 绑定 `Vec<D::Item::Value>`，并在 `ComboboxEvent::Change` 时写入：

`TagDelegate` 是应用定义的 native combobox delegate，其 item value 为 `TagId`。
下文两个 options 变量是相互独立的 application snapshot。

```rust,ignore
let job_tags = FormCombobox::new(
    &job_form,
    JobDraft::TAG_IDS,
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(job_tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
);

let element = Combobox::new(&job_tags);
```

程序化投影根据 native state 当前 delegate 调用 `set_selected_values`。

## 把 Combobox 绑定到 dynamic path

```rust,ignore
let tag_ids: DynamicPath<QueryDraft, Vec<TagId>> =
    condition.clone().then(FilterCondition::TAG_IDS);

let condition_tags = FormCombobox::try_new(
    &query_form,
    tag_ids,
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(condition_tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Combobox::new(&condition_tags);
```

即使外层 item 或 case 是 dynamic，selected values 仍保持 typed。

## 渲染 Checkbox 与 Switch

`Checkbox` 与 `Switch` 没有公开 native state entity，因此使用 controlled element，不创建
adapter wrapper：

```rust,ignore
let enabled = ProviderDraft::ENABLED;
let checked = enabled.value(&form, cx);

let checkbox_form = form.clone();
let checkbox_path = enabled.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        checkbox_path.set(&checkbox_form, *checked, cx);
    });

let switch_form = form.clone();
let switch_path = enabled.clone();
let switch = Switch::new("provider-enabled-switch")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        switch_path.set(&switch_form, *checked, cx);
    });
```

这些 callback 并非从另一个 state entity 的 active update 发出，因此 total path 通过显式 strong
form 同步写入。dynamic boolean 在 render 时使用 `try_value`，在 callback 中使用 `try_set`。

## 刷新 Select 或 Combobox options

Options 与 delegate 属于 application，不属于 `Form<M>`。更新 native items 后立即重投影
form 中的权威值：

```rust,ignore
let selected_model = ProviderDraft::MODEL_ID.value(&form, cx);
provider_model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags = JobDraft::TAG_IDS.value(&job_form, cx);
job_tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});
```

如果 native API 不能原地连续更新 items 并 silent reproject，则重建 adapter。options refresh
绝不隐式选择 fallback、写 form data、开始 validation 或持久化配置。

dynamic path 使用 `try_value`；如果它不再解析，应 teardown adapter，而不是选择 replacement value。

## 处理错误

保持 total 与 dynamic failure 的区别：

```rust,ignore
// Total path：没有 ResolveError。
let errors = ProviderDraft::NAME.errors(&form, cx);

// Dynamic path：检查当前可用性。
let condition_value = condition.clone().then(FilterCondition::VALUE);
match condition_value.try_errors(&query_form, cx) {
    Ok(errors) => render_errors(errors),
    Err(error) => teardown_missing_control(error),
}
```

- `ResolveError` 报告 missing item、inactive case、retired path、wrong session 或其他 dynamic
  resolution failure。
- item identity 由 runtime 持有且对调用方不透明。located path 携带 Form 选定的 occurrence 与
  freshness；model 与 adapter 不管理 item ID，也不在多个 item 之间选择。
- integer-policy error 与 resolution error 保持可区分。
- leased control issue 表示 native editor state，不是第二份 form value。
- page 决定何时展示 error，以及 focus 哪个可见 control。

## 接入自己的组件

### Controlled element 不需要 adapter

如果组件直接参与渲染、没有独立 state entity，就在 render 时读取 typed value，并在 callback
中写回：

```rust,ignore
let enabled_path = ProviderDraft::ENABLED;
let enabled = enabled_path.value(&form, cx);
let form_for_change = form.clone();

TogglePill::new("provider-enabled")
    .selected(enabled)
    .on_change(move |enabled, _window, cx| {
        enabled_path.set(&form_for_change, enabled, cx);
    });
```

组件指向 item、case 或 optional payload 时，改用 `try_value` 与 `try_set`。

### 把 stateful component 封装一次

调用方应获得与内置 adapter 一致的用法：

```rust,ignore
let slug_input = FormSlugInput::new(
    &form,
    ProviderDraft::NAME,
    |initial, window, cx| SlugInputState::new(initial, window, cx),
    window,
    cx,
);

let element = SlugInput::new(&slug_input);
```

adapter 作者只需要完成四件事：读取初始 typed value；把 native change/blur event defer
到 form；把 form commit 静默投影回 native state；持有 control lease 与 subscriptions。

```rust,ignore
use std::ops::Deref;

pub struct FormSlugInput {
    subscriptions: Vec<Subscription>,
    _lease: ControlLease,
    state: Entity<SlugInputState>,
}

impl Deref for FormSlugInput {
    type Target = Entity<SlugInputState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl FormSlugInput {
    pub fn new<M, P, Owner>(
        form: &Entity<Form<M>>,
        path: P,
        build: impl FnOnce(String, &mut Window, &mut Context<SlugInputState>)
            -> SlugInputState,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        M: FormSchema,
        P: IntoTotalPath<M, String>,
        Owner: 'static,
    {
        let path = path.into_total_path();
        let initial = path.value(form, cx);
        let state = cx.new(|state_cx| build(initial, window, state_cx));
        let binding = path.bind_control(form, cx);
        let lease = binding.lease();

        let native_binding = binding.clone();
        let native_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SlugInputEvent, window, cx| match event {
                SlugInputEvent::Change(value) => {
                    native_binding.defer_set(value.clone(), window, cx);
                }
                SlugInputEvent::Blur => {
                    native_binding.defer_blur(window, cx);
                }
            },
        );

        let weak_form = form.downgrade();
        let weak_state = state.downgrade();
        let form_subscription = cx.subscribe_in(
            form,
            window,
            move |_, _, _: &FormEvent, window, cx| {
                let (Some(form), Some(state)) =
                    (weak_form.upgrade(), weak_state.upgrade())
                else { return };
                let value = path.value(&form, cx);
                state.update(cx, |state, cx| {
                    state.set_value_silent(value, window, cx);
                });
            },
        );

        Self {
            subscriptions: vec![native_subscription, form_subscription],
            _lease: lease,
            state,
        }
    }
}
```

dynamic path 使用 `try_value` 与 `try_bind_control`，constructor 返回
`Result<Self, ResolveError>`；path retired 后忽略 form projection。`ControlLease` 必须由 adapter 持有：
drop adapter 会让 queued binding callback 与 control issue 一并退休。owning handle 不保存 strong form
或 authoritative value。

## 相关文档

- [gpui-form 使用指南](../../gpui-form/docs/guide.zh-CN.md)
- [gpui-form-macros 使用指南](../../gpui-form-macros/docs/guide.zh-CN.md)
