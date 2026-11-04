# gpui-form-gpui-component 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

## 开始之前

添加 Form runtime、gpui-component 与本 adapter crate：

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

下文片段使用以下 import。`ModelDelegate`、`TagDelegate` 与 `SlugInputState` 是应用类型。

```rust,ignore
use gpui::{Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    combobox::{Combobox, ComboboxState},
    input::{Input, InputState},
    select::{Select, SelectState},
    switch::Switch,
};
use gpui_form::{DynamicPath, Form, FormSchema, ResolveError};
use gpui_form_gpui_component::{
    FormCombobox, FormInput, FormIntegerInput, FormSelect, IntegerInput,
    IntegerInputState,
};
```

示例使用普通的 typed draft。schema annotation 只描述一次嵌套；调用点不使用字符串 path，也不管理应用侧
item ID。

```rust,ignore
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ModelId(String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TagId(String);

#[derive(Clone, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required)]
    name: String,
    model_id: Option<ModelId>,
    enabled: bool,
}

#[derive(Clone, PartialEq, FormSchema)]
struct JobDraft {
    budget: u64,
    tag_ids: Vec<TagId>,
}

#[derive(Clone, PartialEq, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterNode {
    #[form(child)]
    kind: FilterNodeKind,
}

#[derive(Clone, PartialEq, FormSchema)]
enum FilterNodeKind {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterCondition {
    value: String,
    limit: u64,
    model_id: Option<ModelId>,
    tag_ids: Vec<TagId>,
}
```

## 创建并定位 form

每个编辑 session 创建一个 strong `Entity<Form<M>>`。`Form::new` 不会失败。静态字段是 total path：直接使用
`get` 与 `set`。

```rust,ignore
let form = cx.new(|_| Form::new(ProviderDraft {
    name: String::new(),
    model_id: None,
    enabled: true,
}));

let name: String = ProviderDraft::NAME.get(&form, cx);
let changed: bool = ProviderDraft::NAME.set(&form, "Local provider".into(), cx).unwrap();
```

collection item、活跃 enum case 与 `Option::Some` value 是 dynamic location。Form 生成它们的 identity。应从
Form 枚举，再在同一 session 中解析 case 或 optional boundary：

```rust,ignore
let job_form = cx.new(|_| Form::new(JobDraft {
    budget: 1_024,
    tag_ids: Vec::new(),
}));
let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: String::new(),
                limit: 10,
                model_id: None,
                tag_ids: Vec::new(),
            }),
        }],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
let node = children.items(&query_form, cx).into_iter().next().unwrap();
let condition = node
    .then(FilterNode::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&query_form, cx)?
    .expect("示例以 condition 开始");
let value: DynamicPath<QueryDraft, String> =
    condition.clone().then(FilterCondition::VALUE);
let current = value.try_get(&query_form, cx)?;
```

`Ok(None)` 表示当前 enum case 或 optional value 未激活。`ResolveError` 表示 dynamic 起点已经无法使用，例如
被删除或替换后。两种情况都不能转换成按 index 或业务 ID 查询。

## 绑定 Input

将 total path 传给 `FormInput::new`：

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

`InputEvent::Change` defer typed write；`InputEvent::Blur` 请求配置好的 blur validation。相关 Form value change
会静默投影回 input；由这个 input 发起的 write 不会回声给它自己。

对已解析的 dynamic path 使用 `try_new`：

```rust,ignore
let value = condition.clone().then(FilterCondition::VALUE);
let value_input = FormInput::try_new(
    &query_form,
    value,
    |window, cx| InputState::new(window, cx).placeholder("Condition value"),
    window,
    cx,
)?;
```

在 renderer 中按 dynamic `PathKey` 保存 dynamic adapter。该 location 退休时 drop adapter。如果后续 model
change 在相同 schema position 新建了另一个 condition，应创建新 adapter；绝不能重定向旧 adapter。

## 绑定多行正文与代码

`FormTextarea`、`FormEditor` 的 `new`、`try_new`、`Deref` 契约与 `FormInput` 相同，
原生类型分别为 `TextareaState`、`EditorState`。三者都消费 `InputEvent::Change` / `Blur`，
通过静默 `set_value` 投影；动态路径退役和绑定释放遵循上文规则。

```rust,ignore
let body = FormEditor::new(
    &request_form,
    RequestDraft::BODY,
    |window, cx| EditorState::new(window, cx).language("json"),
    window,
    cx,
);
// 使用 Editor::new(&body) 渲染；正文则使用 FormTextarea、
// TextareaState::new(window, cx) 与 Textarea::new(&body)。
```

适配器从 `gpui_form_gpui_component` 导入，原生类型从 `gpui_component::input` 导入；
workspace 应用也可通过 `gpui_kit::component::input` 使用同一类型。
只读响应查看器使用 `Editor::readonly(true)` 保留选择和复制能力，无须 Form 绑定。

## 绑定整数 input

`FormIntegerInput` 将未完成或非法的展示文本按 editor key 保留在 Form；只有合法的 typed integer 才写入模型。

```rust,ignore
let budget_input = FormIntegerInput::new(
    &job_form,
    JobDraft::BUDGET,
    "budget-editor",
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

构造函数可以拒绝不合法的 native integer policy，但不会把 total path 变成 resolution error。对 dynamic
integer path 使用 `FormIntegerInput::try_new`；它的 build error 区分 unavailable path 与非法 integer policy。

```rust,ignore
let limit = condition.clone().then(FilterCondition::LIMIT);
let limit_input = FormIntegerInput::try_new(
    &query_form,
    limit,
    "budget-editor",
    |window, cx| IntegerInputState::new(window, cx).min(0u64).step(1u64),
    window,
    cx,
)?;
```

## 绑定 Select 与 Combobox

`FormSelect<D>` 绑定 `Option<D::Item::Value>`，并在 `SelectEvent::Confirm` 后写入：

```rust,ignore
let model_select = FormSelect::new(
    &form,
    ProviderDraft::MODEL_ID,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(provider_models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
);

let element = Select::new(&model_select);
```

`FormCombobox<D>` 绑定 `Vec<D::Item::Value>`，并在 `ComboboxEvent::Change` 时写入：

```rust,ignore
let tags = FormCombobox::new(
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

let element = Combobox::new(&tags);
```

解析出 dynamic `model_id` 或 `tag_ids` path 后使用对应的 `try_new` constructor。即使外层 item 或 case 是
dynamic，selection value 仍保持 typed。

## 渲染 Checkbox 与 Switch

`Checkbox` 与 `Switch` 没有 state entity，因此应作为 controlled element 渲染。total-path callback 可以通过
显式 Form 直接写入：

```rust,ignore
let enabled_path = ProviderDraft::ENABLED;
let checked = enabled_path.get(&form, cx);
let form_for_change = form.clone();

let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        enabled_path.set(&form_for_change, *checked, cx).unwrap();
    });

let switch_form = form.clone();
let switch_path = ProviderDraft::ENABLED;
let switch = Switch::new("provider-enabled-switch")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        switch_path.set(&switch_form, *checked, cx).unwrap();
    });
```

dynamic boolean 在 render 时使用 `try_get`，在 callback 中使用 `try_set`。从另一个 state entity 的 active
update 发出的 callback 必须 defer write；这种场景应使用 stateful adapter。

## 刷新 option 而不改变 Form

delegate、catalog 与 option snapshot 属于应用。替换 native item 后，使用 native state 当前的 delegate
重新投影 Form 的权威 selection：

```rust,ignore
let selected_model = ProviderDraft::MODEL_ID.get(&form, cx);
model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags = JobDraft::TAG_IDS.get(&job_form, cx);
tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});
```

option refresh 不得选择 fallback、修改 Form data、开始 validation 或持久化配置。dynamic location 不再解析时，
应 teardown adapter，不能选择 replacement。

Form 会抑制 Combobox 提交 selection 后立刻发生的 self-echo，但不会改变 `gpui-component` 自己的
collection-selection 语义：`set_selected_values` 仍必须从完整 source 解析所有 committed value，即使 search
filter 正在生效。该行为由独立的
[gpui-component#2652](https://github.com/longbridge/gpui-component/issues/2652) 跟踪。

## 渲染 validation feedback

Form 持有 validation fact；页面持有可见性、本地化、布局与 focus 决策。total 与 dynamic path 保持不同的
failure mode：

```rust,ignore
let errors = ProviderDraft::NAME.errors(&form, cx);

let value = condition.clone().then(FilterCondition::VALUE);
match value.try_errors(&query_form, cx) {
    Ok(errors) => render_errors(errors),
    Err(error) => teardown_missing_control(error),
}
```

未解析展示草稿及其问题由 Form 按 editor key 持有，control 卸载后仍保留；合法 edit 只清除来源编辑者的草稿和问题。validation-only change 不会 reset native value，也不会擦除无关 editor state。

只有页面拥有的渲染需要时才 observe Form：

```rust,ignore
let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
```

内置 adapter 与 custom binding 都独立于该 observer 同步。不要让每个 adapter 订阅 `FormEvent`。

## 接入 custom component

### Stateless controlled element

如果 component 没有独立 state entity，使用与 `Checkbox`、`Switch` 相同的 render 时读取、callback 中写入模式。

### Stateful adapter

如果存在 native state entity，则 core binding 拥有 Form 到 control 的投影。adapter 持有 native entity、其
native event subscription 与一个 non-`Clone` `ControlBinding`。native callback 捕获可 clone 的 typed
`ControlWriter`。

```rust,ignore
use std::ops::Deref;
use gpui_form::{
    ControlBinding, ControlProjection, ControlWriter, Form, FormSchema,
    IntoTotalPath,
};

pub struct FormSlugInput {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
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
        let initial = path.get(form, cx);
        let state = cx.new(|state_cx| build(initial, window, state_cx));

        let (binding, writer): (ControlBinding, ControlWriter<M, String>) =
            path.bind_control_in(
                form,
                &state,
                |state, projection, window, cx| match projection {
                    ControlProjection::Value(value) => {
                        state.set_value_silently(value, window, cx);
                    }
                    ControlProjection::Retired => {
                        state.set_retired(window, cx);
                    }
                },
                window,
                cx,
            );

        let native_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SlugInputEvent, window, cx| match event {
                SlugInputEvent::Change(value) => {
                    writer.defer_set(value.clone(), window, cx);
                }
                SlugInputEvent::Blur => writer.defer_blur(window, cx),
            },
        );

        Self {
            subscriptions: vec![native_subscription],
            _binding: binding,
            state,
        }
    }
}
```

silent setter 不得发出 native `Change` event。`ControlProjection` 是穷尽协议：`Value` 更新 state，`Retired`
将 dynamic control 标记为 unavailable，直到 renderer 移除它。adapter 不持有 form entity、不订阅
`FormEvent`、不 clone binding、不识别 control，也不实现本地方向 flag。

对 dynamic path，使用 `try_get` 读取、调用 `try_bind_control_in`，并返回 `Result<Self, ResolveError>`。
只有该 dynamic location 仍处于 active 时，方法才返回 `ControlBinding` 与 `ControlWriter`。drop adapter 即
drop binding，之后 native callback 无法再修改 Form。

## 相关文档

- [gpui-form README](../../gpui-form/README.md)
- [gpui-form 使用指南](../../gpui-form/docs/guide.zh-CN.md)
- [gpui-form-macros 使用指南](../../gpui-form-macros/docs/guide.zh-CN.md)


## 草稿与映射绑定（#248）

FormIntegerInput::new/try_new 在 path 后新增稳定 EditorKey 参数，均返回 FormIntegerInputBuildError（含展示/映射类型不匹配）。同路径两个编辑器必须用不同 key。未解析数字文本由 Form 持有，卸载仍阻止 prepare；相同 key 重挂恢复草稿，程序写值不覆盖草稿。删除动态子树前明确弃稿。

`FormMappedInput::new(form, path, editor_key, mapping, context, build, window, cx)`
将原生 InputState 绑定到 `ControlMapping<S,String>`。`refresh(context,cx)` 只投影无草稿且
无待接纳输入的编辑器，不写模型、不重解析草稿。parse 在入口用当时的存储值、可选基线和上下文
执行一次。`Parsed` 用于完整字段替换，`Draft` 用于无效语法；共享 map 或复合字段的部分编辑
使用 `EditResult::update(move |latest| ...)`，入口捕获 key、已解析意图及需要的上下文/基线叶，
回到继承时也只在 latest 上恢复目标叶。

Update 沿用同一适配器与构造函数，按原生调用顺序在接纳时执行一次。失败把原文本和
`EditProblem` 保存在 Form 草稿中，重挂恢复、阻止 prepare，不自动重试。
结构拒绝同样保留原文（包括 Parsed），绝不自动放弃动态草稿。成功只清来源编辑器草稿；
其他编辑器的草稿和未接纳原文继续保留。穷举匹配 `EditResult` 需新增 Update 分支，
既有 mapping/adapter 签名保持不变。光标、焦点、选择和 IME 仍由原生控件持有。
取消、revision、错误 code 和动态退休规则见[接纳契约](../../gpui-form/docs/guide.zh-CN.md#映射编辑的接纳时更新)。

按 token 展示错误时，`FormMappedInput::editor_status(cx)` 返回来源编辑器已接纳的草稿标志和
当前问题，不包含同一 map 根上其他编辑器的错误。`discard_draft(cx)` 只明确清除该来源并取消
它的排队输入；移除草稿或取消票据后，控件仍挂载时再投影当前存储值。其他编辑器的草稿与票据保留，取消的更新不执行。
文本、整数、选择和组合框适配器也提供相同的两个方法。

控件卸载或继承 key 消失后，通过 `form.read(cx).editor_status(path, editor_key)` 查询保留状态，
无需重解析文本或维护应用侧无效状态副本。明确清除用
`form.discard_editor_draft(path, editor_key, cx)`。精确范围、无变化/版本和无效目标规则见
[按编辑器查询和弃稿](../../gpui-form/docs/guide.zh-CN.md#按编辑器查询和弃稿)。
