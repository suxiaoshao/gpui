# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` 提供 `#[derive(FormSchema)]`，它是 `gpui-form` 的 schema
声明层。为可编辑的 Rust model 使用该 derive；随后 runtime crate 会为该 model 创建一个
`Entity<Form<M>>` 编辑 session。

应用通常依赖会 re-export 该 derive 的 `gpui-form`：

```toml
[dependencies]
gpui.workspace = true
gpui-form.workspace = true
```

## 静态 descriptor 与显式 Form 所有权

derive 为每个字段生成一份静态、带类型的 descriptor。descriptor 只保存 schema 数据：它永远
不持有 `Form`、weak form reference、值或控件。每次读取或修改 path 时，都显式传入当前强
`Entity<Form<M>>`。

```rust,ignore
use gpui::Entity;
use gpui_form::{Form, FormSchema};

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    name: String,
    retry_limit: u32,
}

let form: Entity<Form<ProviderDraft>> = cx.new(|_| {
    Form::new(ProviderDraft {
        name: "primary".to_owned(),
        retry_limit: 3,
    })
});

let name: String = ProviderDraft::NAME.get(&form, cx);
let changed: bool = ProviderDraft::NAME.set(&form, "backup".to_owned(), cx);
```

`ProviderDraft::NAME` 是 `FieldDef<ProviderDraft, String>`，同时也是 total path。total
path 不经过依赖 runtime 的边界，因此 `get` 和 `set` 不会失败。`set` 返回 model 是否真的发生变化。

## 声明嵌套形状

只在字段引入结构时使用属性：

| 属性 | model 形状 | 生成的 descriptor |
| --- | --- | --- |
| `#[form(child)]` | 嵌套 `FormSchema`，包括 `Option<Child>` | `ChildDef` |
| `#[form(items)]` | `Vec<Item>`，其中 `Item: FormSchema` | `ItemsDef` |
| `#[form(required)]` | leaf field | required validation metadata |
| `#[form(validate(...))]` | leaf field | validation trigger metadata |

必需 child 通过 `then` 组合后仍是 total path。集合 item、活跃 enum case 与活跃 optional child
会形成 dynamic path。dynamic path 使用 `try_get` 和 `try_set`，因为 runtime 位置可能退休。

```rust,ignore
let city = ProfileDraft::ADDRESS.then(AddressDraft::CITY);
let city: String = city.get(&form, cx);
```

完整指南说明 optional 与 enum resolver、递归 collection、validation 和提交：

- [English guide](docs/guide.md)
- [中文指南](docs/guide.zh-CN.md)

## runtime 持有的 item identity

model 不保存 form 导航 ID，也不需要为该 derive 实现 identity trait。每当 item、enum case 或
optional payload 变为 active，Form session 都会创建一个不透明 occurrence。应用只能通过枚举
collection 或调用 collection mutation method 获得 typed item path。

这使递归 typed tree 不需要字符串 path 或应用计数器。item 在同一 parent 内重排会保留 identity；
删除后重新插入、重新激活 case 或 optional payload，以及跨 parent 移动都会创建新的 occurrence。
已经退休的 dynamic path 会返回 resolution error，而不会静默指向之后的值。

## validation 与已接受的 snapshot

`#[form(validate(...))]` 可以让字段选择 `on_mount`、`on_change`、`on_blur`、`on_external`
或 `on_submit`。`on_external` 用于 catalog 或其他 application-owned dependency 变化；它与 dynamic
path 无关。若未声明 trigger，普通业务 validation 会在 submit 时运行。

validator 接收一个 snapshot-bound request，并通过 `request.model()` 读取其 model。prepared
submission 携带不透明、session-bound 的 `FormVersion`。application-owned I/O 完成后，用该 version
调用 `rebase_if_current`；此时旧 response 就无法覆盖较新的编辑或另一个 form session。

## 编译期诊断

derive 会在 model 定义附近拒绝不支持的声明：

- generic struct 或 enum、tuple struct 和 union；
- 把 `#[form(items)]` 标在受支持 `Vec<Item>` schema 之外的类型上；
- struct-like enum variant 或具有多个 payload field 的 variant；以及
- 已删除的 `#[form(identity)]` 属性。

支持的 enum variant 是 unit variant，或携带一个实现 `FormSchema` 的具体 tuple payload 的
variant。完整示例请见指南。
