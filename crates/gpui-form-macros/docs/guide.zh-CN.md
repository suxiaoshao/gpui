# gpui-form-macros 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form-macros` 提供 `#[derive(FormModel)]`。它把类型化 Rust model 转换为一个 root form
state 类型和一组可复用 field descriptor。生成 state 实现 `gpui_form::FormState`；descriptor 是
纯 lens，绝不保存强或弱 GPUI entity handle。

## 派生 form model

```rust,ignore
#[derive(Clone, Debug, PartialEq, gpui_form::FormModel)]
#[form(state = ServerForm)]
struct ServerInput {
    #[form(required, validate(on_change, on_blur))]
    name: String,

    #[form(group)]
    auth: AuthInput,

    #[form(array(id = "row_id"))]
    headers: Vec<HeaderRowInput>,
}
```

对于 `ServerInput`，宏生成：

- `ServerForm`：实现 `FormState` 的 root GPUI entity state；
- 每个静态声明字段各有一个 schema-level、`SCREAMING_SNAKE_CASE` 的 associated const，例如
  `ServerForm::NAME`、`ServerForm::AUTH` 与 `ServerForm::HEADERS`；
- validation、schema traversal、revision 和 submit-preparation glue。

每个 associated const 是一个 allocation-free、可复用的 schema definition，以及由静态 schema 与
access function 支撑的 `FormField<ServerForm, T>` typed lens。它不保存 form instance、`Entity`、
`WeakEntity`、value 或 subscription，也不按 form allocation；访问它不会创建 per-form field state。
composition 可以创建轻量的 located descriptor，但不会创建第二个 field 或 schema definition。

宏不会公开 `ServerInputField` enum 或 `FormFieldId`。静态 schema 从实际标识字段的 descriptor
取得：

```rust,ignore
let name = ServerForm::NAME;
assert!(name.schema().is_required());
assert_eq!(name.path(), FieldPath::field("name"));
```

宏不会创建 child form entity。nested model 有自己的 descriptor namespace，但所有 descriptor 都
读写同一个 root entity。

## State 名称与泛型

默认 `Model` 生成 `ModelForm`。当更具体的 state 名在调用点更清晰时，使用 `state = ...`：

```rust,ignore
#[derive(Clone, PartialEq, gpui_form::FormModel)]
#[form(state = GenericValueForm)]
struct ValueEditor<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
}
```

这会生成 `GenericValueForm<T>`。生成声明和实现保留 model 的 lifetime、type parameter、const
generic、合法 default 与 `where` clause；实现会在 Rust 要求的位置移除 type default。

## 类型属性与 canonical grammar

一个 model 最多有一个 `#[form(...)]` helper attribute；每个 option 最多出现一次，option 以逗号
分隔，顺序不限：

```text
state = StateIdent
validation(adapter = "garde"[, messages = ProviderType])
validation(adapter = CustomValidatorType[, context = ContextType])
transform(adapter = "validify")
transform(adapter = CustomTransformType)
```

`StateIdent` 是不加引号的 identifier。custom adapter、context 和 Garde message provider 是不加引号的
Rust type path；只有内建 adapter 名 `"garde"` 和 `"validify"` 使用 string literal。带引号的
custom type、未知内建名、重复 option、空 clause 或第二个 helper attribute 都是 compile error。

内建 validation 与 transform policy 的写法：

```rust,ignore
#[form(
    validation(adapter = "garde", messages = AppGardeMessageProvider),
    transform(adapter = "validify")
)]
```

Garde 的 validation context 始终是 `<Model as garde::Validate>::Context`，并在 Garde model
本身声明：

```rust,ignore
#[derive(gpui_form::FormModel, garde::Validate)]
#[garde(context(ServerValidationContext))]
#[form(validation(adapter = "garde", messages = AppGardeMessageProvider))]
struct ServerInput {
    // ...
}
```

应用自定义 policy 直接使用 type name：

```rust,ignore
#[form(
    validation(adapter = ServerValidator, context = ServerValidationContext),
    transform(adapter = ServerTransform)
)]
```

Validation 是生成 `FormState` 实现选择的 type-level policy；它既不作为 state field 保存，也不通过
`Default` 构造。运行时依赖属于 typed validation context 或 application state。

组合规则仍然严格：

| Adapter | `context` | `messages` |
| --- | --- | --- |
| 没有 validation adapter | 禁止 | 禁止 |
| `"garde"` | 禁止；Garde 持有 `Validate::Context` | 可选 |
| custom validation type | 可选；省略时使用其 associated context | 禁止 |

## 字段属性

| 属性 | 用途 |
| --- | --- |
| `required` | 内建 submit-time required rule 与静态 schema metadata |
| `validate(on_mount, ...)` | 此精确 schema path 的 validation trigger |
| `group` | nested typed form model |
| `array(id = "row_id")` | 带 caller-owned stable item ID 的 typed `Vec<T>` |

每个 field 最多有一个 `#[form(...)]` helper attribute。`required` 和 `group` 是 bare flag；
`validate(...)` 至少包含一个不重复 trigger；`array` 只接受一个 string-literal ID field name，并且
要求 `Vec<T>`。`group` 和 `array` 互斥。

支持的 trigger 是 `on_mount`、`on_change`、`on_blur`、`on_dynamic` 和 `on_submit`。`on_mount`
在 generated state 安装初始值和 validation context 后恰好执行一次。

属性只描述 model data 与 validation rule。component type、options、layout、focus、persistence 和
operation lifecycle 都在 derive 外部。

## 创建并使用 state

每个编辑会话创建一个 `Entity<State>`：

```rust,ignore
use gpui::AppContext as _;
use gpui_form::FormState as _;

let form = cx.new(|cx| {
    ServerForm::from_value(
        ServerInput {
            name: String::new(),
            auth: AuthInput::default(),
            headers: Vec::new(),
        },
        cx,
    )
});

let name = ServerForm::NAME;

let current: String = name.value(&form, cx);
name.set(&form, "api.example.com".into(), cx);

let errors = name.errors(&form, cx);
let validating = name.is_validating(&form, cx);
```

当 validation context 实现 `Default` 时可使用 `from_value`。如果 validation 需要
application-owned dependency，使用
`from_value_with_validation_context(value, context, cx)`。

对于 total descriptor，这些 API 不返回 `Result`。entity 是强引用，generated root path 也在类型上
确定存在。same-value `set` 是 no-op；发生业务值变化时恰好推进一次 revision、执行适用 validation、
发出一个 event 并通知一次。

`FormField` constructor 对 core 私有。应用使用 generated schema-level const 和 composition，而不是
自行构造 path 或 model projection。

## Group 与 array 中的可用性

`FormField<Form, T>` 是 total descriptor，表示声明的 root path 或 parent 为 total 的 projection。
`PartialFormField<Form, T>` 表示相对于当前 live model 可能无法解析的 path。

Group 通过 `within` 保持 parent 的可用性：

```rust,ignore
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
username.set(&form, "alice".into(), cx);
```

identified item 会变为 partial，因为在创建 descriptor 与使用之间该 item 可能已被移除：

```rust,ignore
let header = ServerForm::HEADERS.item(gpui_form::FormItemId::new(row_id));
let header_name = HeaderRowForm::NAME.within(header);

let value = header_name.try_value(&form, cx)?;
header_name.try_set(&form, "Authorization".into(), cx)?;
```

`project_value(...)` 也为 partial；它可能代表 optional 或 computed projection，因此它和每个 child
都公开 `try_*` 方法。相应 error 只描述真正无法解析的 projection 或缺失/歧义 stable item；不存在
同步 `FormReleased` error。

完整 generated traversal vocabulary：

```rust,ignore
RootForm::FIELD;
ChildForm::FIELD.within(parent);
RootForm::ITEMS;
RootForm::ITEMS.item(id);
```

whole-array write 始终是 total，是显式 add、remove、reorder 和 replace item 的 API。stable ID 在当前
array 中唯一，且不能通过 identified-item descriptor 改变。

## Validation、schema 与 submit

generated state 持有 current value、baseline、monotonic revision、validation context、report 和
validation task bookkeeping。descriptor 只标识 typed path；core 完成成功写入 transaction，并恰好
notify 一次。

nested schema ownership 是精确的：group 持有 direct path，array 持有 direct path 和 item root，
child model 持有其 descendant。Garde model 可在 `group` 或 `array` 上使用 `#[garde(dive)]` 选择
递归；derive 会将外部 vector index 映射成 stable item path。

`SubmitTransform` 是 static、pure、不可失败的转换：从 validated model 得到 application output，
不需要 adapter instance、不做 I/O，也不修改 form state：

```rust,ignore
struct SaveServer {
    name: String,
}

struct ServerTransform;

impl gpui_form::SubmitTransform<ServerInput> for ServerTransform {
    type Output = SaveServer;

    fn transform(model: &ServerInput) -> Self::Output {
        SaveServer {
            name: model.name.trim().to_owned(),
        }
    }
}
```

`prepare_submit` 先执行同步 submit validation，拒绝 blocking issue 或 pending validation，恰好应用
一次 transform，然后返回：

```rust,ignore
let prepared = form.update(cx, |form, cx| form.prepare_submit(cx))?;
let gpui_form::PreparedSubmit { revision, output } = prepared;
self.start_save(revision, output, cx);
```

页面或 controller 持有 persistence，并在保存成功后用返回的 revision 调用 `rebase_if_revision`。
form 没有 busy flag、save task、retry state、persistence callback 或 operation runtime。

## 编译期诊断

derive 会在编译期报告不支持的 attribute、无效 validation trigger、不正确 group type、在非 `Vec`
field 上使用 `array`、缺失 stable ID、无法解析的 adapter type、Garde 上的 custom context，以及
用于其他 adapter 的 Garde message provider。

它还会拒绝重复 helper attribute/option、带引号的 custom type、空 clause 和所有已移除的
draft/component/focus option。`store = ...` 是已移除的 type option，diagnostic 应引导调用者使用
`state = ...`；`FormStore` 是已移除的 derive name，diagnostic 应引导调用者使用 `FormModel`。
非法配置永不覆盖、忽略或通过 compatibility alias 接受。

## 相关文档

- [gpui-form 使用指南](../../gpui-form/docs/guide.zh-CN.md)
- [gpui-form-gpui-component 使用指南](../../gpui-form-gpui-component/docs/guide.zh-CN.md)
