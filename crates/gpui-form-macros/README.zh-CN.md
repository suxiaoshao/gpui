# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` 为 `gpui-form` 提供 `#[derive(FormModel)]`。它从普通 Rust model
生成一个 GPUI form state 类型、可复用的类型化字段 descriptor、schema 元数据、validation
遍历与 submit-preparation glue。

## 示例

```rust,ignore
use gpui::AppContext as _;
use gpui_form::FormState as _;

#[derive(Clone, Debug, PartialEq, gpui_form::FormModel)]
#[form(state = ProviderForm)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,
    retry_limit: u32,
}

let form = cx.new(|cx| {
    ProviderForm::from_value(
        ProviderInput {
            name: String::new(),
            retry_limit: 3,
        },
        cx,
    )
});

let name = ProviderForm::NAME;
name.set(&form, "OpenAI".to_owned(), cx);

let prepared = form.update(cx, |form, cx| form.prepare_submit(cx))?;
assert_eq!(prepared.output.name, "OpenAI");
```

默认情况下，`Model` 生成 `ModelForm`；`#[form(state = ProviderForm)]` 可覆盖该 state
类型名。生成类型实现 `FormState`，并持有恰好一个 internal runtime：current model、baseline、
revision、validation context 与 validation state。

derive 会为每个静态声明的字段生成一个 schema-level、`SCREAMING_SNAKE_CASE` 的 associated const，
例如 `ProviderForm::NAME`。每个 const 都是一份 allocation-free schema definition，并暴露轻量的
`FormField<ProviderForm, T>` typed lens，仅由静态 schema 与 access function 支撑；它不持有也不弱引用
`Entity<ProviderForm>`，访问它不会创建 per-form field state、allocation 或 subscription。每个数据操作都由
调用者显式传入 `&Entity<ProviderForm>`。

宏不会公开 `ProviderInputField` enum 或 `FormFieldId` API。schema 属于 descriptor：

```rust,ignore
let name = ProviderForm::NAME;
assert!(name.schema().is_required());
let errors = name.errors(&form, cx);
```

## Total 与 partial descriptor

根字段和普通 group projection 是 `FormField<Form, T>`（total descriptor）。它们的 `value`、
`set`、validation 与 error API 都不返回 `Result`：只要调用者提供的 form entity 存在，该路径在
类型上就确定存在。

identified array item 和计算型 `project_value` 是 `PartialFormField<Form, T>`。因为 stable-ID
item 可能已移除、计算 projection 也可能不可用，它们只提供显式的 `try_*` API：

```rust,ignore
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
username.set(&form, "alice".into(), cx);

let header = ServerForm::HEADERS.item(row_id);
let header_name = HeaderRowForm::NAME.within(header);
let row = header_name.try_value(&form, cx)?;
```

可用性沿组合传播：`within` 保持 parent 的可用性；`HEADERS.item(id)` 与
`project_value(...)` 变为 partial；partial descriptor 的所有后代仍为 partial。宏会保持 descriptor
constructor 私有，因此每个公开 descriptor 都携带 generated path 与 schema contract。

## Validation 与 submit

derive 支持 Garde、自定义 validation adapter、generic model、nested group 和 stable-ID array。
Validation adapter 是 type-level associated policy，不保存为实例，也不要求 `Default`。运行时依赖
属于 typed validation context 或 application-owned state。

`SubmitTransform` 是从已验证 model 到 application output 的 static、pure、不可失败的转换。
`prepare_submit` 返回 `PreparedSubmit<Output>`，其中同时包含 output 与产生它的 form revision。
持久化、request task、retry 和 conditional rebase 仍由应用负责。

nested model 同样 derive `FormModel`，但不会创建 child form entity。`within` 在同一个 root form 上把
child lens 组合到 parent lens；`item(id)` 创建 `PartialFormField`，每个后代仍为 partial；
`project_value` 同样是 partial。`FormField` constructor 保持 core-private。宏不生成
control、component configuration、raw draft、codec、focus/touched/blurred state、persistence 或 operation
lifecycle。

## 文档

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [文档索引](docs/README.md)
