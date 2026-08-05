# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` 提供 `#[derive(FormSchema)]`：它是 greenfield
`gpui-form` API 的编译期部分。把 derive 标在可编辑模型上，再创建一个统一的
`Form<M>` session，由它拥有草稿、拓扑、校验和提交边界。

应用通常依赖会 re-export `FormSchema` 的 `gpui-form`：

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## 从 flat model 开始

普通 struct 会生成 total 的根字段。创建一个 form session，把它保存为
`Entity<Form<M>>`，再通过生成的根定义读取或更新字段。

```rust,ignore
use gpui::{App, Entity};
use gpui_form::{Form, FormRevision, FormSchema, Prepared};

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    name: String,
    retry_limit: u32,
}

struct SaveProvider {
    name: String,
    retry_limit: u32,
}

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self { name: draft.name, retry_limit: draft.retry_limit }
    }
}

let runtime = Form::try_new(ProviderDraft {
    name: "primary".to_owned(),
    retry_limit: 3,
})?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| runtime);

let name = ProviderDraft::NAME;
let current: String = name.value(&form, cx);
name.set(&form, "backup".to_owned(), cx);

let prepared: Prepared<ProviderDraft> =
    form.update(cx, |form, cx| form.prepare(cx))?;
let revision: FormRevision = prepared.revision();
let request = prepared.map(SaveProvider::from);
// 把 `(revision, request)` 交给应用持有的 persistence task。

fn apply_saved_provider(
    form: &Entity<Form<ProviderDraft>>,
    revision: FormRevision,
    saved_provider: ProviderDraft,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| {
        form.rebase_if_revision(revision, saved_provider, cx)
    })
}
```

`ProviderDraft::NAME` 是生成的 `FieldDef<ProviderDraft, String>`。根与 `name`
之间没有拓扑边界，因此它的路径是 total，`value` 和 `set` 可以直接作用于该
form session。

`prepare` 是从可编辑 session 交给可提交快照的显式边界：它运行 session 的校验
策略并捕获 revision；随后 `map` 在不重新开放修改的前提下转换已接受的快照。应在
`map` 前保存 revision；持久化完成后，只通过 `rebase_if_revision` 应用 canonical
`ProviderDraft`，避免旧 response 覆盖较新的编辑。

## 用属性加入结构

`FormSchema` 从 Rust 字段和 variant 推导静态定义：

| 属性 | 目标模型形状 | 生成的定义 |
| --- | --- | --- |
| `#[form(child)]` | 嵌套 schema，允许 `Option<Child>` | `ChildDef` |
| `#[form(items)]` | item 拥有 form schema 的 `Vec<Item>` | `ItemsDef` |

定义从根到叶组合。例如，集合中 item 的属性使用
Form runtime 返回的 `ItemPath` 开始，再通过 `item_path.then(...)` 继续组合。经过 item、可选值或
enum case 后，路径变为 dynamic；读写时必须使用 `try_value` / `try_set`。model 永不声明或保存
form-only item ID。

完整教程包括 total child path、`try_some(&Form)`、递归数组、
`try_case(&Form, CaseDef)`、拓扑修改、validator 和提交/rebase：

- [English guide](docs/guide.md)
- [中文指南](docs/guide.zh-CN.md)

## 生成的名称就是契约

derive 展开的是 schema metadata，不是编辑 runtime。runtime crate 提供 `Form<M>`、
path、validator、拓扑操作和 prepared snapshot；macro 提供带类型的静态入口，例如：

```rust,ignore
ProfileDraft::DISPLAY_NAME; // FieldDef<ProfileDraft, String>
ProfileDraft::ADDRESS;      // ChildDef<ProfileDraft, AddressDraft>
ProfileDraft::RULES;        // ItemsDef<ProfileDraft, RuleDraft>
ModeDraft::REMOTE;          // CaseDef<ModeDraft, RemoteDraft>
```

definition type 由 `gpui-form` re-export；上面的名称就是 derive output contract。

## 校验属性

`child` 与 `items` 描述结构。leaf field 还接受 validation metadata：

| Attribute | 含义 |
| --- | --- |
| `#[form(required)]` | 标记字段必填；没有显式 trigger list 时启用 mount/change/blur/submit |
| `#[form(validate(...))]` | 启用 `on_mount`、`on_change`、`on_blur`、`on_dynamic`、`on_submit` 中的任意组合 |

required value 通过 `RequiredValue` 判断：string 会 trim，option 与受支持 collection 不能为空，bool 必须为
true。

## 编译期诊断

derive 应在模型声明附近拒绝不支持的 schema 形状：泛型 schema type、tuple struct、
union、struct-like enum variant，以及含有多个 payload field 的 enum variant。`items`
字段必须是受支持的 collection，其 item 需要提供 form schema。已经删除的
`#[form(identity)]` 会报错：item identity 由 Form runtime 生成并持有，不属于 model field。

支持的递归形状与 diagnostics 请见指南。
