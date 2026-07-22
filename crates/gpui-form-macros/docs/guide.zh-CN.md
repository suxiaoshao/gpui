# gpui-form-macros 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form-macros` 提供 `gpui-form` 应用使用的 `#[derive(FormStore)]` 入口。

> **实现状态：**本指南描述已经实现的公开契约。

## 派生 store

```rust,ignore
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
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

- `ServerInputFormStore`；
- `ServerInputField` 与静态字段 schema；
- `ServerInputFormStore::name_field(&form)` 这类类型化字段访问；
- 验证与 submit 遍历；
- 嵌套 group 与稳定 ID 数组 path。

## Store 名称与泛型

默认情况下，`Model` 会生成 `ModelFormStore` 与 `ModelField`。需要更清晰的领域名称时可以覆盖 store 名称：

```rust,ignore
#[derive(Clone, PartialEq, gpui_form::FormStore)]
#[form(store = GenericValueStore)]
struct ValueEditor<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
}
```

这会生成 `GenericValueStore<T>` 与 `ValueEditorField`。`store = ...` 只覆盖 store 名，field enum
始终根据 model 名生成。Generated store 声明与实现保留 model 的 lifetime、type parameter、const
generic、合法位置上的 default 和 `where` 约束；Rust 不允许的 impl type default 会被移除。

## 类型属性与 canonical grammar

本次重构是有意的 breaking change。每个 model 最多只能有一个 `#[form(...)]` helper
attribute，每个 option 最多出现一次。类型 option 使用逗号分隔，顺序不限：

```text
store = StoreIdent
validation(adapter = "garde"[, i18n = ProviderType])
validation(adapter = CustomValidatorType[, context = ContextType])
transform(adapter = "validify")
transform(adapter = CustomTransformType)
```

`StoreIdent` 是不加引号的 identifier。Custom adapter、context 与 I18n provider 都是不加引号
的 Rust type path。只有内建 adapter 名 `"garde"` 和 `"validify"` 使用 string literal。带引号
的 custom type、未知内建名、重复 option、空 clause 与第二个 helper attribute 都是 compile
error，不提供 compatibility alias。

内建 validation 与 transform adapter 的写法如下：

```rust,ignore
#[form(
    validation(adapter = "garde", i18n = AppGardeI18nProvider),
    transform(adapter = "validify")
)]
```

对于 Garde，validation context 始终是
`<Model as garde::Validate>::Context`，通过 Garde 自身声明：

```rust,ignore
#[derive(gpui_form::FormStore, garde::Validate)]
#[garde(context(ServerValidationContext))]
#[form(validation(
    adapter = "garde",
    i18n = AppGardeI18nProvider
))]
struct ServerInput {
    // ...
}
```

宏会选择 `GardeAdapter<ServerInput, AppGardeI18nProvider>`。省略 `i18n` 时选择
`DefaultGardeI18nProvider`。宏不生成 translation、Fluent key、Garde rule 或 locale observer。

应用自定义 adapter type 不使用引号：

```rust,ignore
#[form(
    validation(
        adapter = ServerValidator,
        context = ServerValidationContext
    ),
    transform(adapter = ServerTransform)
)]
```

组合规则是严格的：

| Adapter | `context` | `i18n` |
| --- | --- | --- |
| 无 validation adapter | 禁止 | 禁止 |
| `"garde"` | 禁止；context 来自 Garde 的 `Validate::Context` | 可选 |
| custom validation type | 可选；省略时使用 adapter 的关联 context | 禁止 |

生成的 store 实现 `FormStore` constructor contract：
`from_value_with_validation_context`、`validation_context` 与
`set_validation_context` 始终可用。`FormStore::from_value` 是带有
`Self::ValidationContext: Default` 约束的 trait method；derive 不会按 context 是否实现
`Default` 而生成另一套 inherent API。

两个 constructor 都会先安装初始 model 与 validation context，再恰好执行一次 `on_mount`
validation。`set_validation_context` 只替换 context 并通知 observer；新 context 需要验证时，
调用者显式选择 trigger 与 scope。

Custom validation 与 transform adapter type 实现 `Default + 'static`。它们通过 `FormStore`
关联类型选择，只在执行 validation 或 transform 时调用 `default()` 临时构造；generated store
不保存这两类实例。Runtime 依赖放在 validation context 中，不由 adapter 或 transform value
保存应用状态。Custom `SubmitTransform<Model>` 声明关联 `Output` 与唯一的 `transform` method；
generated `FormStore::Output` 是
`<Transform as SubmitTransform<Model>>::Output`。Identity 与 Validify transform 使用 model
本身作为 output。

## 字段属性

| 属性 | 用途 |
| --- | --- |
| `required` | 内建 submit-time 必填规则与静态 required schema；`validate(...)` 增加更早 trigger |
| `validate(on_mount, ...)` | 字段 validation trigger |
| `group` | 嵌套类型化 form model |
| `array(id = "row_id")` | 带稳定 item path 的类型化数组 |

每个 field 最多只能有一个 `#[form(...)]` helper attribute。`required` 与 `group` 是 bare
flag；`required = true`、`group()` 和 `group(store = ...)` 这类嵌套配置都无效。
`validate(...)` 至少包含一个 trigger，且不能重复。`array` 只接受
`id = "..."` 这一个 string-literal field name；bare identifier、缺少 ID、额外 option 或用于非
`Vec<T>` 字段都会报错。`group` 与 `array` 互斥。

Validation trigger 包括 `on_mount`、`on_change`、`on_blur`、`on_dynamic` 和
`on_submit`。`on_mount` 会在任一 generated constructor 内执行一次，且此时初始值与
validation context 已安装。

属性只描述 form data 与规则。Component type、options、layout、focus 与 persistence 由应用或 adapter crate 配置。
`component`、`codec`、`binding`、`state`、`focus`、`touched`、`blurred`、
`show_error` 与 nested `store` 等旧 field option 会得到明确的迁移诊断。Derive 不会静默忽略
未知或已删除的 option。

## Generated ownership 与生命周期边界

Generated store 恰好只有一个 private、doc-hidden
`FormRuntime<Model, ValidationContext>` field；它只是 macro/core plumbing，不是调用者 API。该
runtime 持有当前 model、baseline、单调 revision、validation context 与 validation state。
Validation adapter 与 submit transform 是关联类型，不是保存的实例。Generated 实现把它们的
生命周期委托给 core `FormStore` contract，并发出 typed field 或 runtime notification。

它不持有或生成以下内容：

- raw/String draft、codec 或每个 field 的业务值副本；
- component entity、options、config、subscription 或 focus handle；
- touched、blurred、focused 或 error-visibility flag；
- `SubmitRuntime`、request task、busy flag、submission-attempt counter、retry policy 或
  persistence call。

`prepare_submit` 只是同步 validation 与 transform 边界。页面或应用 store 持有异步请求生命周期，
并通过 core revision API 有条件地 rebase 保存后的值。

## 类型化字段访问

生成字段访问保留声明的 Rust 类型：

```rust,ignore
use gpui_form::{FormFieldId as _, FormStore as _};

let name = ServerInputFormStore::name_field(&form);
name.set("api.example.com".to_owned(), cx)?;

let path = ServerInputField::Name.path();
let required = ServerInputField::Name.schema().is_required();

let report: gpui_form::ValidationReport =
    form.read(cx).validation_report();
let errors: Vec<gpui_form::ValidationIssue> =
    form.read(cx).errors_at(&path);
```

生成 API 不会让整数或 enum 经过 String draft。

每次非相等的 typed write 都执行一个由 core 持有的 transaction：先投影并保存 typed value，同时推进
revision；只清除相交的 required、structural 与 generated synchronous field bucket；取消并清除
相交的 async validation；保留 adapter-wide form bucket 与所有 active control issue；对字段的
validation path 运行 `on_change`；最后发出一次 typed form event，并且只通知 observer 一次。
投影后的值与当前 field value 相等时，整条 transaction 都是 no-op。宏只提供 field projection 与
schema；projection 只作用于 cloned `Model` candidate，不能访问 runtime、validation、`Context`、
event 或 notify，因此 root/nested accessor 都不可能复制或提前执行该生命周期。Validation query
返回 owned snapshot：
`validation_report() -> ValidationReport` 与
`errors_at(path) -> Vec<ValidationIssue>`。

## Validation 与 submit 生成

即使 field 没有声明 `on_submit`，`required` 也始终参与 submit validation。Field 的
`validate(...)` 只增加更早或显式的验证时机。Nested leaf rule 从 leaf schema 选择，不会复制到
每一层 ancestor group/array。

同步 adapter validation 始终读取 store 持有的 model snapshot。Garde-backed model 使用自己的关联
context 与选中的 I18n provider。Derive 实现 `GardePathMapper`，把外部 vector index 转换为
generated stable path。Custom adapter 通过 core trait 收到同一 model、trigger、scope 和 typed
validation context。

Derive 还会为每个完整 stable path 生成递归 model schema resolution。Core 按固定的
`schema resolver -> scope -> exact owner trigger` 顺序规范化每个 adapter issue；generated code
不使用 root-prefix filter，也不把 leaf trigger 复制到 ancestor。Resolver failure 会在 scope
过滤前变成 blocking internal issue，因此窄 validation run 不能隐藏非法 adapter path。

`prepare_submit` 对一个 model snapshot 按固定顺序执行：

1. 执行 submit validation，包括 required 与结构不变量；
2. 遇到 validation issue 或仍 pending 的 blocking async validation 时拒绝；
3. 恰好调用一次所选 `SubmitTransform<Model>::transform`；
4. 返回关联 output，不修改 model，也不启动 I/O。

Custom output type 的写法如下：

```rust,ignore
#[derive(Default)]
struct ServerTransform;

struct SaveServer {
    name: String,
}

impl gpui_form::SubmitTransform<ServerInput> for ServerTransform {
    type Output = SaveServer;

    fn transform(
        &self,
        model: &ServerInput,
    ) -> Result<Self::Output, gpui_form::TransformReport> {
        Ok(SaveServer {
            name: model.name.trim().to_owned(),
        })
    }
}
```

没有 transform preview method 或 transform context。Transform failure 直接返回给调用者，不会变成
validation state。

## Group 与数组

`#[form(group)]` 复用 child model schema，同时保留唯一顶层 form store。
Child model 也 derive `FormStore`。它生成的 store type 只是
`*_in(parent_field)` accessor 的命名空间；调用 accessor 不会创建 child store entity：

```rust,ignore
let username = AuthInputFormStore::username_in(
    ServerInputFormStore::auth_field(&form),
);
```

`#[form(array(id = "row_id"))]` 要求每个 item 暴露声明字段对应的 stable ID。Error 与 bound
field 使用 stable ID，而不是当前 vector index。ID field type 实现 `ToFormItemId`；generated
item accessor 接收 `FormItemId`。

父 store 会为 identified array 提供 `*_item(form, id)`。先取得 item handle，再与 child model 的
`*_in` accessor 组合，就能进入 leaf field：

```rust,ignore
let header_name = HeaderRowInputFormStore::name_in(
    ServerInputFormStore::headers_item(
        &form,
        gpui_form::FormItemId::new(row_id),
    ),
);
```

结果类型是 `FormField<ServerInputFormStore, String>`：它保留顶层 store type，并携带包含 stable
ID 的 nested path。重复调用任一 accessor 都只会创建另一个轻量 typed handle。

如果 identified array 属于 nested model，使用它生成的
`*_item_in(parent_field, id)` accessor。完整 traversal API 是：

```rust,ignore
RootFormStore::field_field(&form);
ChildFormStore::field_in(parent_field);
RootFormStore::items_item(&form, item_id);
ChildFormStore::items_item_in(parent_field, item_id);
```

这些 API 都是指向 root form 的 typed lens，不会分配另一个 form store、持有 subscription 或复制
business state。

Stable ID 在当前 array 内必须唯一，并且不能通过 identified-item handle 改变。Item 缺失或存在
歧义时，generated handle 的读写返回 `FormFieldError::ValueUnavailable`；accessor 绝不会选择
第一个 duplicate。通过 identified-item 整项写入或 generated ID leaf 写入改成不同或无法转换的
ID 时返回 `FormFieldError::ItemIdentityChanged`。两类错误都在 cloned candidate 上被拒绝并且完全
no-op。Duplicate 或无法转换的 ID 会产生阻止提交的 internal structural issue；submit 检查这个
不变量不需要 array 声明 `validate(...)`。Whole-array write 仍可显式完成 add、remove、reorder 与
replacement。

Stable identity 是名义语义，不是历史追踪：在一次 form session 中，相同的
`(array path, stable ID)` 表示同一个 logical item。Runtime 不保存 retired ID；whole-array write
保留 ID 表示更新该名义 item，改变 ID 表示 remove + insert。应用必须为新的 logical item 分配
新的 ID。Reorder 会保留寻址，但 whole-array write 会使 descendant synchronous issue 与 async
check 失效，下一次 validation run 再把新 issue 映射到当前 ID。

Nested leaf validation 由 leaf 自己的 generated schema 控制，ancestor group/array 不重复声明这些
trigger；nested `required` 始终自动传播到 submit。精确 ownership 会递归应用：

| Stable path 形态 | Generated schema owner |
| --- | --- |
| `auth` | 声明的 `auth` group field |
| `auth.username` | child model 的 `username` field |
| `headers` | 声明的 `headers` array field |
| `headers[#id]` | 直接拥有该 item 的 `headers` array field |
| `headers[#id].name` | item model 的 `name` field |

Group 内 array、identified item 内 array 与更深组合都使用同一规则。Item root 没有 synthetic
schema，也不会让 array 拥有 item descendant。Group 或 array 自身的 `validate(...)` 因此只作用于
挂在该 parent 精确 path 上的 issue；对于 array，还包括它的 direct item root，而不包括 nested leaf。

Garde recursion 是另一项显式选择。Garde 持有 nested rule 时，在 group 和 array 上添加
`#[garde(dive)]`；nested type 也实现具有兼容 context 的 `garde::Validate`：

```rust,ignore
#[derive(gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
struct ServerInput {
    #[form(group)]
    #[garde(dive)]
    auth: AuthInput,

    #[form(array(id = "row_id"))]
    #[garde(dive)]
    headers: Vec<HeaderRowInput>,
}
```

Derive 会为所有 array 形态实现 `GardePathMapper`：container（`headers`）、direct item root
（`headers[2]`）与 item leaf（`headers[2].name`），并递归支持 nested group/array。Indexed path
会针对本次 validated model 做 bounds check，再转换为 stable item ID。Mapping 不检查 schema
trigger；内建 Garde adapter 返回完整 mapped report，core 再独立执行精确 schema resolution、
scope filter 与 trigger filter。Unknown field、非法或越界 index、duplicate ID 与无效 item ID
返回 typed `GardePathError`；runtime 会把失败转换为阻止提交的 internal issue。

## 编译期诊断

Derive 会在编译期报告不支持的属性、无效 validation trigger、不正确 group 类型、在非数组字段上使用 `array`、缺失稳定 ID、无法解析的 adapter type、Garde 上的 custom context，以及用于其他 adapter 的 Garde I18n。

它还会拒绝重复的 `#[form(...)]` helper attribute、同一 option 的重复声明、用字符串表示的 custom type、空的 `validation(...)` / `transform(...)` / `validate(...)`、非 canonical 拼写，以及已经删除的 legacy option。错误必须定位到具体 token，并在存在直接替代项时给出替代写法；宏不能通过“后一个覆盖前一个”或静默忽略继续生成代码。

## 相关文档

- [gpui-form 使用指南](../../gpui-form/docs/guide.zh-CN.md)
- [gpui-form-gpui-component 使用指南](../../gpui-form-gpui-component/docs/guide.zh-CN.md)
