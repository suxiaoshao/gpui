# gpui-form vNext 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

## 1. 加入页面需要的组件

应用通常使用 core crate 和一个 native-control adapter。Garde 等 validator adapter 是可选的；它绝不
是 path authority。

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
# garde.workspace = true # 可选 validator adapter
```

下文示例使用这个公共 prelude：

```rust,ignore
use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    form::field,
    input::{Input, InputState},
};
use gpui_form::{
    DynamicPath, Form, FormRevision, FormSchema, Position, Prepared, TotalPath,
    ValidationMessage, ValidationRequest, ValidationSink, ValidationTrigger,
    Validator,
};
use gpui_form_gpui_component::{
    FormInput, FormIntegerInput, IntegerInputState,
};
```

每次编辑 session 使用一个 `Entity<Form<M>>`。form 持有 draft 与 editing runtime；页面持有
persistence 与 presentation；native control 持有 focus、IME、popup state、selection 与未完成 editor text。

## 2. 声明 draft 并创建 session

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_change, on_blur, on_submit))]
    name: String,

    #[form(validate(on_change, on_submit))]
    retry_limit: u32,

    enabled: bool,
}
```

`required` 与 `validate(...)` 是 static leaf schema metadata，不属于 component configuration，也不
属于第二份 field value copy。

`FormSchema` 在 model type 上生成静态 definition：

```rust,ignore
ProviderDraft::NAME: FieldDef<ProviderDraft, String>
ProviderDraft::RETRY_LIMIT: FieldDef<ProviderDraft, u32>
```

以初始 draft 与 validator 创建 session；不需要注入 validator 时使用 `Form::try_new`。

```rust,ignore
let runtime = Form::try_new_with_validator(
    ProviderDraft { name: String::new(), retry_limit: 3, enabled: true },
    ProviderValidator::new(reserved_names),
)?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| runtime);
```

第 6 节会实现 `ProviderValidator`；`reserved_names` 是 page 传入的
`Arc<HashSet<String>>`。

同一 schema 可用于拥有不同 validator 或 validator context 的另一个 session。更新 options catalog 不会
改写 form；其 owner 显式刷新 validator data，并在产品规则要求时请求 dynamic validation。

## 3. 在页面中持有 form、control 与 observation

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

impl ProviderPage {
    fn new(
        reserved_names: Arc<HashSet<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let runtime = Form::try_new_with_validator(
            ProviderDraft { name: String::new(), retry_limit: 3, enabled: true },
            ProviderValidator::new(reserved_names),
        )?;
        let form = cx.new(|_| runtime);

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
            |window, cx| IntegerInputState::new(window, cx)
                .min(0u32).max(10u32).step(1u32),
            window,
            cx,
        )?;
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self { form, form_observer, name_input, retry_limit_input })
    }
}
```

root `FieldDef<M, T>` 是 total-path 简写。静态嵌套 path 同样是 total；item、enum case 或 optional
payload 会产生 dynamic path，并改用 adapter 的 `try_new`。

本指南不重复所有 native-control rule。[component adapter guide](../../gpui-form-gpui-component/docs/guide.zh-CN.md)
说明 deferred Input/Integer/Select/Combobox event、options refresh、teardown 与 custom stateful adapter。

## 4. 渲染 field 与 session status

static schema 回答 label 是否 required 等问题。显式 form 回答当前 editing session 的实时状态。

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let pending = self.form.read(cx).is_validating();
let dirty = self.form.read(cx).is_dirty();
let valid = self.form.read(cx).is_valid();
let feedback = errors
    .first()
    .map(|issue| validation_text(issue, cx))
    .unwrap_or_else(|| if pending { "Checking…".into() } else { String::new() });

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(feedback)
    .child(Input::new(&self.name_input));
```

form-level query family 包含 dirty、valid、pending、validation report、某个 path 的 error、首个
blocking error path 与 revision。form 永不持有 touched/blurred/error-visibility mirror；
submit 失败后由页面选择可见 native control 来 focus。

当 callback 不在 native state-entity update 中执行时，无状态 control 使用同一个 total shorthand：

```rust,ignore
let enabled = ProviderDraft::ENABLED;
let checked = enabled.value(&self.form, cx);
let form = self.form.clone();

Checkbox::new("provider-enabled")
    .checked(checked)
    .on_click(move |checked, _, cx| enabled.set(&form, *checked, cx));
```

## 5. Replace、reset、rebase 与 revision

安装应用数据，或保存后获得 canonical model 时，使用 whole-form lifecycle operation：

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next, cx));
self.form.update(cx, |form, cx| form.reset(cx));
self.form.update(cx, |form, cx| form.rebase(saved, cx));
```

- `replace` 安装新的 current draft，并保留 baseline。
- `reset` 将 baseline 恢复为 current draft。
- `rebase` 将同一个 model 同时安装为 current draft 与 baseline。

即使 Rust value 相等，每个 lifecycle operation 也会推进 revision 并重投影已 mount control。它使旧 async
与 topology lifetime work 失效、清理过期 validation state，且不会假装每个 leaf 都是 user change。
`rebase_if_revision` 是唯一的 async-save merge primitive：比较失败不会改变 draft、baseline、report、task
或 control。

## 6. 执行同步、dynamic 与异步 validation

### Validation rule 与 scoped result

runtime 支持 mount、change、blur、dynamic 与 submit trigger。required 语义是：trim 后为空的 string、
`None`、空的受支持 collection 与 `false` 都是 missing；numeric 与 enum 没有隐式 missing 语义。

change scope 包含变更 path、其 descendant 与 structural ancestor，但不包含无关 sibling。一轮 validation
只替换它拥有的 source/trigger/path bucket，因此修改一个 field 不会清掉另一个 field 的 error。使用过期
item path 或制造 move cycle 等 topology error 会在 validation 前拒绝 transaction；它们不是 validation
issue。

### 编写 session validator

这份完整 sketch 展示 validator flow：

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
        model: &ProviderDraft,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<ProviderDraft>,
    ) {
        let path = ProviderDraft::NAME;
        if request.includes(&path)
            && !model.name.is_empty()
            && self.reserved_names.contains(model.name.trim())
        {
            out.at(path).error(
                "provider-name-unavailable",
                ValidationMessage::key("provider-name-unavailable"),
            );
        }
    }
}
```

native validator 通过同一个 sink 发出 typed 或 canonical path。使用 Garde adapter 时，它通过同一个
form snapshot 的 topology 将 positional collection report 映射为 runtime-located item path 与 active
case。stale、unknown、inactive 或无法解析的 adapter path 都成为 blocking internal form issue，绝不会被
丢弃或错贴到别的 field。

### 有意请求 dynamic 与 async validation

外部 dependency 变化后，由 owner 显式请求 dynamic validation：

```rust,ignore
ProviderDraft::NAME.validate(&self.form, ValidationTrigger::Dynamic, cx);
```

页面还决定何时值得发起 remote check。它以一个 path 与 input snapshot 开始；之后 form 持有 cancellation、
generation、address/incarnation freshness 与 result publication：

```rust,ignore
self.form.update(cx, |form, cx| {
    form.start_async_validation(
        ProviderDraft::NAME,
        "provider-name",
        |name| async move { directory.check_name(name).await },
        cx,
    )
})?;
```

相交写入、lifecycle replacement、subtree removal 或过期 completion 都不能发布旧 result。pending
form-owned async validation 会阻止 `prepare`。native editor 暂时不能产生 typed `T` 时，把 raw text 留在
本地，并通过 binding 发布 lifecycle-scoped control issue。

## 7. Prepare、持久化与有条件 rebase

```rust,ignore
struct SaveProvider {
    name: String,
    retry_limit: u32,
    enabled: bool,
}

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self {
            name: draft.name,
            retry_limit: draft.retry_limit,
            enabled: draft.enabled,
        }
    }
}

let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;
let revision = prepared.revision();
let request = prepared.map(SaveProvider::from);

self.start_save(revision, request, cx);

// 在页面持有的 async completion callback 中：
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_revision(revision, canonical_saved_model, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`prepare` 在同一个 snapshot 上运行 submit validation，拒绝 blocking data/control issue 与 pending async
work，并原子捕获 snapshot 加 revision。`Prepared<M>::map` 消费它且只转换一次。persistence、loading、retry
与 notification 仍由应用持有。

## 8. 组合 static、optional 与 recursive path

### Static 与 optional child

```rust,ignore
#[derive(Clone, FormSchema)]
struct AuthDraft {
    username: String,
}

#[derive(Clone, FormSchema)]
struct RedirectDraft {
    callback_url: String,
}

#[derive(Clone, FormSchema)]
struct ServerDraft {
    #[form(child)]
    auth: AuthDraft,
    #[form(child)]
    redirect: Option<RedirectDraft>,
}

let username: TotalPath<ServerDraft, String> =
    ServerDraft::AUTH.then(AuthDraft::USERNAME);
// `server_form: Entity<Form<ServerDraft>>` 持有这次编辑 session。
let callback: DynamicPath<ServerDraft, String> = ServerDraft::REDIRECT
    .try_some(server_form.read(cx))?
    .then(RedirectDraft::CALLBACK_URL);
```

static composition 保持 total。`try_some(form)` 与 `try_case(form, case_def)` 显式定位当前
optional/case incarnation并返回dynamic path；Form 返回的 `ItemPath` 已经是 dynamic。所有
descendant 都保持dynamic并使用 `try_*` operation。返回的 path 不保存 form entity，调用方也永远
不提供 item ID。

### 由 runtime 定位的递归 tree

业务 model 只包含查询数据，不携带为了 form 导航而增加的 ID：

```rust,ignore
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
}

let runtime = Form::try_new(QueryDraft {
    root: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Group(FilterGroup {
                children: vec![FilterNode {
                    kind: FilterNodeKind::Condition(FilterCondition {
                        value: String::new(),
                    }),
                }],
            }),
        }],
    },
})?;
let query_form: Entity<Form<QueryDraft>> = cx.new(|_| runtime);

let root_children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let group_node = root_children
    .items(&query_form, cx)?
    .into_iter()
    .next()
    .expect("示例包含一个 root node");

let nested_children = group_node
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);
let condition_node = nested_children
    .try_items(&query_form, cx)?
    .into_iter()
    .next()
    .expect("示例包含一个 nested node");

let value = condition_node
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::CONDITION)?
    .then(FilterCondition::VALUE);

let current: String = value.try_value(&query_form, cx)?;
value.try_set(&query_form, "Rust".to_owned(), cx)?;
```

`Form` 在创建 session 时为每个 item occurrence 分配不透明的 session-local identity。`items` 与
`try_items` 按当前顺序返回带有该 identity 的 typed `ItemPath`。从返回的 `Vec` 中按位置选择 entry
只用于选择当前元素；得到的 path 不保存 index，后续也不靠 index 解析。调用方不能构造、读取或序列化
内部 token，也不会把它放进业务 model。

collection 不是可写的 `Vec<T>` leaf。topology method 接收并返回同一种 typed item path，不接收
raw ID：

```rust,ignore
let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let anchor = children.items(&query_form, cx)?.into_iter().next().unwrap();

let appended = children.append(
    &query_form,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
let inserted = children.insert_before(
    &query_form,
    &anchor,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
children.move_before(&query_form, &appended, &anchor, cx)?;
children.remove(&query_form, inserted, cx)?;

let fresh = children.replace_all(
    &query_form,
    vec![
        FilterNode {
            kind: FilterNodeKind::Group(FilterGroup { children: Vec::new() }),
        },
        FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
        },
    ],
    cx,
)?;

let parent = fresh[0].clone();
let source = fresh[1].clone();
let destination = parent
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);
let moved = source.move_to(&query_form, destination, Position::End, cx)?;
```

`append` 与 `insert_before` 返回新 occurrence；`replace_all` 为所有 replacement item 返回 fresh path。
same-parent reorder 保留 item path。remove、whole-collection replacement、case reconstruction、
whole-model replacement/rebase 或 cross-parent move 会 retire 受影响的旧 path。cross-parent move 是一次
经过 cycle check 的 root transaction，并返回 destination 下的新 path；旧 binding 不会隐式跟随。

## 9. Ownership 与 lifetime rule

静态 definition 持有 schema edge 与 typed accessor。located path 持有 typed access plan、canonical address
与 static target。`Form<M>` 持有可变 draft runtime。path 与 definition 都不持有 entity、value、validation
report、subscription 或 native control。

canonical address 标识位置，incarnation 标识该位置当前的 object。`try_case` 与 `try_some` 读取当前
`&Form<M>`，把该 incarnation 捕获到不持有 entity 的 path 中。只有 deferred binding 与 async work
持有 weak form ownership；它们还捕获 address、incarnation、generation 与 control-issue lease。
stale callback 变为 no-op。dynamic subtree 消失时，由 renderer 销毁 native entity 与 subscription。

## 10. 契约摘要

静态 `then` 不需要 Form。`try_case(&Form, CaseDef)` 与 `try_some(&Form)` 捕获当前 incarnation，Form
内部私有持有 topology snapshot。Form 持有 item identity，调用方只接收 typed located path，不接收
raw ID。lifecycle replacement 会退休旧 path、binding、issue 与 async completion，不会让它们在同一
结构位置复活。

## 相关文档

- [文档索引](README.md)
- [宏使用指南](../../gpui-form-macros/docs/guide.zh-CN.md)
- [Component adapter 使用指南](../../gpui-form-gpui-component/docs/guide.zh-CN.md)
