# `FormSchema` derive 指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

`FormSchema` 在编译期声明可编辑 Rust model 的 schema。生成的 definition 是静态且带类型的；
`gpui-form` 提供它们所操作的唯一显式 `Entity<Form<M>>` session。本指南使用当前公开 API。

应用通常通过 `gpui-form` 使用该 derive：

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## 1. derive 字段并创建一个 form session

每个受支持的具名字段都会得到一份静态 descriptor。descriptor 不是 field instance，也不保留 form。
页面或 view 持有 form entity，并将它显式传给 descriptor 与组合 path。

```rust,ignore
use gpui::Entity;
use gpui_form::{Form, FormSchema};

#[derive(Clone, FormSchema)]
struct AccountDraft {
    email: String,
    max_projects: u32,
}

let form: Entity<Form<AccountDraft>> = cx.new(|_| {
    Form::new(AccountDraft {
        email: "owner@example.com".to_owned(),
        max_projects: 5,
    })
});

let email: String = AccountDraft::EMAIL.get(&form, cx);
let changed: bool = AccountDraft::EMAIL.set(
    &form,
    "team@example.com".to_owned(),
    cx,
);
```

`AccountDraft::EMAIL` 是 `FieldDef<AccountDraft, String>`。它是 total path：在此 form session
中始终恰好存在一个 `email` 值。total path 使用 `get` 与 `set`，从不使用 `Result`；等值 `set`
返回 `false`。

## 2. 标注嵌套 child 与 collection

嵌套 schema 使用 `#[form(child)]`，嵌套 schema 的 collection 使用 `#[form(items)]`。必需 child
在静态上一定存在，因此穿过它组合后的 path 仍是 total。

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

let profile_form = cx.new(|_| Form::new(ProfileDraft {
    address: AddressDraft { city: String::new() },
}));
let city = ProfileDraft::ADDRESS.then(AddressDraft::CITY);
let value: String = city.get(&profile_form, cx);
city.set(&profile_form, "Shanghai".to_owned(), cx);
```

derive 会为 `ADDRESS` 生成 `ChildDef<ProfileDraft, AddressDraft>`。组合后的 `city` path
仍是 `TotalPath<ProfileDraft, String>`，因为它没有跨过由 runtime 选择的边界。

对于 collection，只声明业务数据。不要为了 Form 导航增加 ID field 或 key trait：

```rust,ignore
#[derive(Clone, FormSchema)]
struct RuleDraft {
    label: String,
}

#[derive(Clone, FormSchema)]
struct PolicyDraft {
    #[form(items)]
    rules: Vec<RuleDraft>,
}
```

Form runtime 为每个 active item 创建不透明 occurrence identity。应用只能从 `items`、`try_items`
或 collection mutation 获得它的 typed `ItemPath`；不能从数组下标、业务 ID 或序列化 token 重建它。

## 3. 解析 optional child 与 enum case

optional payload 或 enum case 并不总是 active。先构造带类型的 resolver，再针对显式 form 解析。
两类 resolver 都返回 `Result<Option<_>, ResolveError>`：

- `Ok(Some(path))` 表示请求的 payload 当前 active；
- `Ok(None)` 表示 optional 为 `None` 或 enum 当前是另一个 case；以及
- `Err(_)` 表示 resolver 的 dynamic 起点属于另一个 session 或已经退休。

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

let connection_form = cx.new(|_| Form::new(ConnectionDraft {
    credentials: None,
}));
ConnectionDraft::CREDENTIALS.set(
    &connection_form,
    Some(CredentialsDraft::default()),
    cx,
);

let credentials = ConnectionDraft::CREDENTIALS
    .some()
    .resolve(&connection_form, cx)?;

if let Some(credentials) = credentials {
    let token = credentials.then(CredentialsDraft::TOKEN);
    let value: String = token.try_get(&connection_form, cx)?;
    token.try_set(&connection_form, "secret".to_owned(), cx)?;
}
```

path 一旦跨过 item、optional 或 case 边界，就成为 dynamic path。使用 `try_get` 与 `try_set`；
它们的错误表示这个具体 runtime 位置已经不可用，而不会暴露 session、token 或 topology 实现细节。

## 4. 构建递归 typed tree

递归 form 与业务 model 使用相同 Rust 类型。collection 与 resolver API 会在每一层保留精确的
payload type。

```rust,ignore
#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
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

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: String::new(),
            }),
        }],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);

for node in children.items(&query_form, cx) {
    let condition = node
        .then(FilterNode::KIND)
        .case(FilterNodeKind::CONDITION)
        .resolve(&query_form, cx)?;

    if let Some(condition) = condition {
        let value = condition.then(FilterCondition::VALUE);
        let current: String = value.try_get(&query_form, cx)?;
        value.try_set(&query_form, current.trim().to_owned(), cx)?;
    }
}
```

`FilterNodeKind::CONDITION` 是生成的 `CaseDef<FilterNodeKind, FilterCondition>`。resolver 会保留
该 payload type，因此写入错误 Rust type 的值会在编译期被拒绝。对 inactive case 调用
`case(...).resolve` 返回 `Ok(None)`，不是 stale-path error。

runtime 会为每个 item、active case 和 active optional payload 分配新的 occurrence。同 parent 内
重排会保留 item occurrence。删除后重新插入 item、离开后再回到一个 case，或重建 optional payload
都会生成新的 occurrence；旧 dynamic path 始终保持 retired。

## 5. 通过 path 而不是 ID 修改 collection

使用 collection method 创建、排序、删除和替换 item。它们接收和返回 typed item path，而不是 index
或 application ID。

```rust,ignore
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
let node = children.append(
    &query_form,
    FilterNode {
        kind: FilterNodeKind::Condition(FilterCondition {
            value: String::new(),
        }),
    },
    cx,
)?;

let first = children.items(&query_form, cx).into_iter().next();
if let Some(first) = first {
    children.move_before(&query_form, &node, &first, cx)?;
}

children.remove(&query_form, node, cx)?;
```

`append`、`insert_before`、`move_before`、`remove` 与 `replace_all` 构成 collection vocabulary。
`ItemPath::move_to` 执行显式跨 parent 移动，并返回新的 destination path。已删除或以其他方式退休的
path 不能再次使用。

## 6. 添加 validation metadata 与 validator

leaf field 可以使用 `#[form(required)]` 与 `#[form(validate(...))]`。`validate` 选择
`on_mount`、`on_change`、`on_blur`、`on_external` 和 `on_submit`。`on_external` 用于刷新 catalog
等 application-owned fact；它刻意独立于 `DynamicPath` 的含义。

如果没有显式选择非 submit trigger，业务 validation 会在 submit 时运行。validator 通过
`ValidationRequest` 获得一个一致 snapshot；使用 `request.model()` 读取 model，而不是再接收第二个
model parameter 或重新读取 live Form。

```rust,ignore
use gpui_form::{
    ValidationMessage, ValidationRequest, ValidationSink, Validator,
};

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_submit, on_external))]
    name: String,
}

struct ProviderValidator;

impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        if request.includes(&ProviderDraft::NAME)
            && request.model().name.trim().is_empty()
        {
            out.at(ProviderDraft::NAME).error(
                "provider-name-empty",
                ValidationMessage::key("provider-name-empty"),
            );
        }
    }
}

let form = cx.new(|_| {
    Form::new(ProviderDraft { name: String::new() })
        .with_validator(ProviderValidator)
});
```

当外部 dependency 变化时，应用可以显式请求 `ValidationTrigger::External`。Form 保存 validation
fact；页面仍拥有何时、何处显示它们的权力。

## 7. prepare、保存并条件 rebase

`prepare` 运行 submit validation 并返回已接受的 `Prepared<M>`。它包含绑定到当前编辑 session 的
不透明 `FormVersion`。应用把 draft 转换为 request 时，`map` 会保留该 version。

```rust,ignore
use gpui::{App, Entity};
use gpui_form::{Form, FormVersion, Prepared};

struct SaveProvider(ProviderDraft);

let prepared: Prepared<ProviderDraft> =
    form.update(cx, |form, cx| form.prepare(cx))?;
let request: Prepared<SaveProvider> = prepared.map(SaveProvider);
let version: FormVersion = request.version();

// 使用 `request.into_parts().1` 执行 application-owned I/O。

fn apply_saved_provider(
    form: &Entity<Form<ProviderDraft>>,
    version: FormVersion,
    canonical: ProviderDraft,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| {
        form.rebase_if_current(version, canonical, cx)
    })
}
```

如果用户在 prepare 后编辑过 form，或 version 来自另一个 session，`rebase_if_current` 会返回 `false`
且不改变 form。

## 8. 在声明处获得诊断

derive 会在声明处诊断不支持的 schema 形状：

| 无效声明 | 期望方向 |
| --- | --- |
| generic struct 或 enum | schema 是 monomorphic |
| tuple struct 或 union | 只有受支持的具名 struct 与 enum 形状能暴露 definition |
| 非 `Vec` field 使用 `#[form(items)]` | collection 使用受支持的 `Vec<Item>` item |
| item 没有 `FormSchema` | structured item 暴露 schema |
| `#[form(identity)]` | 已删除；Form 持有 occurrence identity |
| struct-like 或 multi-payload enum variant | variant 是 unit，或有一个具体 tuple payload |

macro 将不支持的形状排除在 application runtime code 之外。runtime resolution error 只限于曾指向
active item、case 或 optional payload 的 dynamic path。
