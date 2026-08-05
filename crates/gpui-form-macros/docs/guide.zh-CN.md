# `FormSchema` derive 指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

本指南按“从模型声明到 prepared submission”的顺序讲解已确认的 greenfield derive 设计。
它以任务为中心：先 derive 静态定义，再用这些定义操作一个统一的 `Form<M>` session。

应用通常通过 `gpui-form` 使用这个 derive：

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## 1. derive 一个 flat `FormSchema`

从只包含叶字段的模型开始。`FormSchema` 会为每个字段生成带类型的根定义。

```rust,ignore
use gpui::Entity;
use gpui_form::{Form, FormSchema};

#[derive(Clone, FormSchema)]
struct AccountDraft {
    email: String,
    max_projects: u32,
}

let runtime = Form::try_new(AccountDraft {
    email: "owner@example.com".to_owned(),
    max_projects: 5,
})?;
let form: Entity<Form<AccountDraft>> = cx.new(|_| runtime);

let email = AccountDraft::EMAIL;
let current = email.value(&form, cx);
email.set(&form, "team@example.com".to_owned(), cx);
```

`AccountDraft::EMAIL` 是 `FieldDef<AccountDraft, String>`。根字段是 total path：
它不经过 item、可选值或 case 边界，因此无需运行时选择。

## 2. helper attribute grammar

| 属性 | 可接受的目标形状 | derive 输出 |
| --- | --- | --- |
| `#[form(child)]` | 实现 `FormSchema` 的字段类型，包括 `Option<Child>` | `ChildDef<Parent, Child>` |
| `#[form(items)]` | `Vec<Item>`，其中 `Item` 实现 `FormSchema` | `ItemsDef<Parent, Item>` |
| `#[form(required)]` | leaf field | required metadata |
| `#[form(validate(...))]` | leaf field | mount/change/blur/dynamic/submit trigger metadata |

`child` 与 `items` 关乎形状。item identity 不是 macro input：Form 为每个 item occurrence 生成
不透明的 session-local identity，并且只通过 typed `ItemPath` 返回它。model 不为 form 导航实现 key
trait，也不声明 ID field。

## 3. 组合一个 total static child path

必需的嵌套 child 仍然是 total。`then` 将 parent-child edge 与 child schema 的定义
连接，因此结果不需要运行时选择即可读写。

```rust,ignore
#[derive(Clone, FormSchema)]
struct AddressDraft {
    city: String,
}

#[derive(Clone, FormSchema)]
struct ProfileDraft {
    #[form(child)]
    address: AddressDraft,
}

let runtime = Form::try_new(ProfileDraft {
    address: AddressDraft { city: String::new() },
})?;
let form: Entity<Form<ProfileDraft>> = cx.new(|_| runtime);

let city = ProfileDraft::ADDRESS.then(AddressDraft::CITY);
let current = city.value(&form, cx);
city.set(&form, "Shanghai".to_owned(), cx);
```

组合后的类型是 `TotalPath<ProfileDraft, String>`：`ADDRESS` 始终存在，因此下面的
`CITY` 始终有唯一目标。

## 4. 使用 `try_some` 进入 optional child

optional child 的值为 `None` 时没有 target。应用需要创建 child 时，先写入 total option field；
`try_some(&Form)` 在当前 session 中定位已经存在的 payload，因此结果是 dynamic path。

```rust,ignore
#[derive(Clone, Default, FormSchema)]
struct CredentialsDraft {
    token: String,
}

#[derive(Clone, FormSchema)]
struct ConnectionDraft {
    #[form(child)]
    credentials: Option<CredentialsDraft>,
}

let runtime = Form::try_new(ConnectionDraft { credentials: None })?;
let form: Entity<Form<ConnectionDraft>> = cx.new(|_| runtime);

ConnectionDraft::CREDENTIALS.set(
    &form,
    Some(CredentialsDraft::default()),
    cx,
);

let token: DynamicPath<ConnectionDraft, String> = ConnectionDraft::CREDENTIALS
    .try_some(form.read(cx))?
    .then(CredentialsDraft::TOKEN);

let current = token.try_value(&form, cx)?;
token.try_set(&form, "secret".to_owned(), cx)?;
```

option replacement 的详细 error field 仍在 API 设计中；resolver contract 已经固定：`try_some`
显式接收当前 `&Form<ConnectionDraft>`，捕获 active `Some` incarnation，但不保存 form entity，并返回
dynamic path。路径一旦经过这个边界，它和所有后代都保持 dynamic。

## 5. 建模 recursive items 与 enum case

这个自包含示例包含递归 group，以及用 `case` 选择具体 payload 的 enum。item model 不包含
form-only ID。

```rust,ignore
use gpui::Entity;
use gpui_form::{DynamicPath, Form, FormSchema};

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
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
let form: Entity<Form<QueryDraft>> = cx.new(|_| runtime);

let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let group_node = children
    .items(&form, cx)?
    .into_iter()
    .next()
    .expect("示例包含一个 root node");
let nested_children = group_node
    .then(FilterNode::KIND)
    .try_case(form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);
let condition_node = nested_children
    .try_items(&form, cx)?
    .into_iter()
    .next()
    .expect("示例包含一个 nested node");

let value: DynamicPath<QueryDraft, String> = condition_node
    .then(FilterNode::KIND)
    .try_case(form.read(cx), FilterNodeKind::CONDITION)?
    .then(FilterCondition::VALUE);

let current = value.try_value(&form, cx)?;
value.try_set(&form, "Rust".to_owned(), cx)?;
```

Form 为每个 item occurrence 生成并持有不透明 identity。`items` 与 `try_items` 从当前 runtime
topology 返回 typed `ItemPath`；调用方不能从 model value 构造或恢复它。item path 已经是 dynamic；
`try_case(&Form, CaseDef)` 定位 active case 并捕获其当前 incarnation。已经 retire 的 item 或 inactive
case 是可恢复的 path-resolution error，不能静默 fallback。

支持的 enum variant 是 unit variant，或带一个实现 `FormSchema` 的具体 tuple payload
的 variant。泛型 schema model、struct-like variant 和带多个 payload field 的 variant
都是编译期错误。

## 6. 把生成的 definitions 当作积木

derive 暴露静态 definition；应用组合它们，而不是用字符串构造 field name 或 path。

```rust,ignore
ProfileDraft::DISPLAY_NAME; // FieldDef<ProfileDraft, String>
ProfileDraft::ADDRESS;      // ChildDef<ProfileDraft, AddressDraft>
FilterGroup::CHILDREN;      // ItemsDef<FilterGroup, FilterNode>
FilterNodeKind::GROUP;      // CaseDef<FilterNodeKind, FilterGroup>
```

重要的生成 family 是 `FieldDef`、`ChildDef`、`ItemsDef` 与 `CaseDef`。typed `ItemPath` 由 runtime
创建，不属于 derive output。具体 module path 与非 resolver helper 的细节仍由 runtime API 决定；
resolver 形状已经固定为 path 调用 `try_case(&Form, CaseDef)` 或 `try_some(&Form)`，generated definition
与 located path 都不持有 form entity。

## 7. 通过 topology methods 修改 items

collection 是 topology boundary，不是从模型借出的可变 `Vec`。form session 负责 item
创建、删除、排序、不透明 identity 与移动，从而保持 revision tracking 和 validation scope。

```rust,ignore
use gpui_form::Position;

let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
let anchor = children.items(&form, cx)?.into_iter().next().unwrap();
let appended = children.append(
    &form,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
let inserted = children.insert_before(
    &form,
    &anchor,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition { value: String::new() }),
    },
    cx,
)?;
children.move_before(&form, &appended, &anchor, cx)?;
children.remove(&form, inserted, cx)?;

let fresh = children.replace_all(
    &form,
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
```

跨 parent 的移动是显式的，因为它会改变 ownership 并 retire source path：

```rust,ignore
let parent = fresh[0].clone();
let source = fresh[1].clone();
let destination_children = parent
    .then(FilterNode::KIND)
    .try_case(form.read(cx), FilterNodeKind::GROUP)?
    .then(FilterGroup::CHILDREN);

let moved = source.move_to(
    &form,
    destination_children,
    Position::End,
    cx,
)?;
```

`items`、`append`、`insert_before`、`move_before`、`remove` 与 `replace_all` 是 topology
vocabulary。它们只交换 typed item path，不接收 raw ID。runtime 拒绝 stale/wrong-session 的 source、
destination 或 anchor path，以及造成 cycle 的 move。`TopologyIndex` 是私有 runtime state；调用方不会
向这些 API 传入它或 topology snapshot。

## 8. 用 validator、prepare 和 rebase 闭合 session

校验属于 `Form<M>` session，而不是独立的模型 state。validator 接收请求的 scope，并向
sink 报告；它可以按 runtime 的定义校验 leaf、subtree 或整个 model。

```rust,ignore
use gpui::{App, Entity};
use gpui_form::{
    Form, FormRevision, ValidationMessage, ValidationRequest, ValidationSink,
    Validator,
};

#[derive(Clone)]
struct QueryContext {
    allow_empty: bool,
}

struct QueryValidator {
    context: QueryContext,
}

impl Validator<QueryDraft> for QueryValidator {
    fn validate(
        &self,
        model: &QueryDraft,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<QueryDraft>,
    ) {
        let children = QueryDraft::ROOT.then(FilterGroup::CHILDREN);
        if request.includes(&children)
            && !self.context.allow_empty
            && model.root.children.is_empty()
        {
            out.at(children).error(
                "query-empty",
                ValidationMessage::key("query-empty"),
            );
        }
    }
}

struct SaveQuery(QueryDraft);

impl From<QueryDraft> for SaveQuery {
    fn from(query: QueryDraft) -> Self {
        Self(query)
    }
}

let context = QueryContext { allow_empty: false };
let initial_query = QueryDraft {
    root: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: "Rust".to_owned(),
            }),
        }],
    },
};
let runtime = Form::try_new_with_validator(initial_query, QueryValidator { context })?;
let form: Entity<Form<QueryDraft>> = cx.new(|_| runtime);

let prepared = form.update(cx, |form, cx| form.prepare(cx))?;
let revision: FormRevision = prepared.revision();
let request = prepared.map(SaveQuery::from);
// 把 `(revision, request)` 交给应用持有的 persistence task。

fn apply_saved_query(
    form: &Entity<Form<QueryDraft>>,
    revision: FormRevision,
    canonical_saved_model: QueryDraft,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| {
        form.rebase_if_revision(revision, canonical_saved_model, cx)
    })
}
```

`prepare` 校验并冻结已接受的 snapshot。`map` 将该 snapshot 消费为 request。持久化后，
条件 rebase 会阻止旧 response 覆盖请求飞行期间产生的新编辑。`Validator`、
`ValidationRequest`、`ValidationSink` 与 error type 由 runtime crate 提供。

## 9. 在声明处获得诊断

macro 应让无效 schema 在声明处就足够明确：

| 无效声明 | 期望的诊断方向 |
| --- | --- |
| 泛型 struct 或 enum | schema 必须是 monomorphic |
| tuple struct 或 union | 只有支持的 struct/enum shape 能暴露具名 definition |
| 把 `#[form(items)]` 标在非 `Vec` field 上 | item collection 使用受支持的 `Vec<Item>` 形状 |
| `#[form(items)]` 的 item 没有 `FormSchema` | structured item 必须暴露 schema |
| 使用已删除的 `#[form(identity)]` | Form 持有 item identity；删除该属性，只在字段属于业务数据时保留字段 |
| struct-like 或 multi-payload enum variant | case 必须是 unit 或一个具体 tuple payload |

diagnostic 应指向出问题的 attribute、field 或 variant，并说明期望的支持 model shape，
把递归相关的 runtime failure 排除在应用代码之外。
