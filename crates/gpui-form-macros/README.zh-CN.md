# gpui-form-macros

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-macros` 为 `gpui-form` 提供 `#[derive(FormStore)]`，根据普通 Rust model
生成 typed form store、field identity 与 schema、typed field accessor、validation traversal
和 submit-preparation glue。

> **实现状态：**本 README 描述已经实现的公开 API。验证证据参见
> [实施计划](dev/form-store-derive.md)。

## 使用方式

```rust,ignore
use gpui::AppContext as _;
use gpui_form::FormStore as _;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
struct ProviderInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,
    retry_limit: u32,
}

let form = cx.new(|cx| {
    ProviderInputFormStore::from_value(
        ProviderInput {
            name: String::new(),
            retry_limit: 3,
        },
        cx,
    )
});

let name = ProviderInputFormStore::name_field(&form);
name.set("OpenAI".to_owned(), cx)?;

let output = form.update(cx, |form, cx| form.prepare_submit(cx))?;
assert_eq!(output.name, "OpenAI");
```

这会生成 `ProviderInputFormStore`、`ProviderInputField`、静态 field schema/path，以及保留
原始 Rust 类型的 accessor。`required` 始终参与 submit validation；`validate(...)` 增加更早的
trigger。

Generated store 只包含一个 internal、doc-hidden
`FormRuntime<Model, ValidationContext>`，调用者不直接访问该 runtime。Validation 与 submit
行为分别由 `FormStore::ValidationAdapter` 和 `FormStore::SubmitTransform` 关联类型选择；两者都要求
`Default + 'static`，只在执行 validation 或 transform 时临时构造，不作为实例保存在 store
中。`validation_report()`、`errors_at(path)` 等 validation query 返回 owned snapshot。

Derive 还支持 Garde 或 custom validation adapter、submit transform、generic model、nested
group 与 stable-ID array。Nested model 独立 derive `FormStore`，但不会创建 child form entity；
生成的 `*_in`、`*_item` 和 `*_item_in` 始终是指向唯一 root form value 的 typed lens。UI
control 属于 adapter crate，不由 derive macro 生成。

需要显式命名 generated store 时使用 `#[form(store = ValueEditorStore)]`，对应的 generated
store 名是 `ValueEditorStore`。这个属性只覆盖 store 名；field enum 始终根据 model 命名为
`ModelField`。

宏不生成 UI control、component config、raw draft、codec、focus/touched/blurred 状态、submit
task、busy flag、retry policy 或 persistence。Control 属于 adapter crate；请求生命周期与持久化
属于应用。

## 文档

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [文档索引](docs/README.md)
