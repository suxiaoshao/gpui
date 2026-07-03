# Validation Routing Design

本文记录 `gpui-form` 把 parent `FormValidationReport` 路由到普通字段、nested group 和 dynamic array 的规则。

## 当前问题

group routing 曾经在 `report.strip_field_prefix(group_path)` 之后，又把 parent report 中路径等于 child
relative field path 的错误 merge 到 child report。

当 parent form 和 child group 有同名字段时，例如 parent 有 `name`，child `profile` 里也有 `name`，root
`name` 的错误会被复制到 `profile.name`。这会错误地标记 child group invalid，并在 nested field 上显示 sibling
error。

## 设计目标

- validation error routing 必须只按完整 field path 归属错误。
- group 只接收以 group path 为前缀的错误，例如 `profile.name`。
- group 内部 store 只处理 strip 后的相对路径，例如 `name`。
- parent root field error，例如 `name`，不能因为 child 有相同相对字段名而进入 child group。
- macro glue code 不做模糊兼容，不猜测 adapter 返回的相对路径属于哪个 group。

## 文件和模块结构

| 文件 | 计划 |
| --- | --- |
| `crates/gpui-form-macros/src/expand/validation.rs` | group routing 只使用 `strip_field_prefix(group_path)`，删除 child relative path merge。 |
| `crates/gpui-form/src/core/error.rs` | 保持 `FormValidationReport::strip_field_prefix` 作为唯一 prefix-stripping primitive。 |
| `crates/gpui-form/src/core/path.rs` | 保持 `FieldPath::starts_with` / `strip_prefix` 语义不变。 |
| `crates/gpui-form/tests/derive.rs` | 增加 parent/child 同名字段的 group routing 回归测试。 |

禁止新增 `mod.rs`。

## 自定义类型结构

不新增 public 类型。

继续使用现有类型：

```rust
pub struct FormValidationReport {
    field_errors: Vec<FieldError>,
    form_errors: Vec<FormError>,
}

pub struct FieldError {
    pub path: FieldPath,
    // existing fields...
}
```

核心 API：

```rust
impl FormValidationReport {
    pub fn strip_field_prefix(&self, prefix: &FieldPath) -> Self;
}
```

`strip_field_prefix` 是 group/array 把 parent absolute path 转成 child relative path 的唯一入口。

## Routing 规则

普通字段：

```text
field path = name
parent report contains error.path == name
  -> write to parent field name
```

group：

```text
group path = profile
parent report contains error.path == profile.name
  -> strip prefix profile
  -> child report contains error.path == name
  -> write to child field name
```

不能支持的模糊输入：

```text
group path = profile
parent report contains error.path == name
child form also has field path name
  -> do not copy into child
```

如果 adapter 在 parent scope 中产出 child-relative path，它无法和 parent root field path 区分；这类数据应在
adapter 层修正为完整 path，而不是由 generated group routing 猜测。

array：

```text
array path = headers
item index = 0
parent report contains error.path == headers[0].key
  -> strip prefix headers[0]
  -> child report contains error.path == key
  -> write to item child field key
```

array routing 已按 index prefix strip，不额外 merge child relative field path。

## 数据流

Submit validation：

```text
FormStore::submit
  -> ValidationAdapter validates full domain input
  -> adapter report uses parent absolute FieldPath
  -> GeneratedFormStore::apply_validation_report(scope = Form)
  -> ordinary fields match exact path
  -> groups strip group prefix only
  -> arrays strip array[index] prefix only
  -> child stores receive child-relative report
```

Live field validation：

```text
field component event
  -> apply_validation_for_scope(scope = Field(field_path))
  -> adapter returns report scoped to the field path
  -> group routing runs only if scope contains that group path
```

## 所用组件

无 UI 组件变更。

- Error display 继续由接入 app 使用 field errors 和 `gpui-component::form` 渲染。
- 本设计只影响 generated validation routing，不新增 view、element、button 或 form row component。

## 全局数据管理

无全局数据管理变更。

- Validation report 是 submit/live validation 调用内的瞬时数据。
- 不新增 registry、cache、`Global` 或 `gpui-store` 状态。

## 数据库变更

无数据库变更。

- Validation routing 不读写 app DB、config、keychain 或 runtime state。
- 错误路径不持久化。

## 数据获取方式

无网络或数据库读取。

- Parent report 来自当前 `ValidationAdapter`。
- Group path 来自 `FieldGroupStore::path()`。
- Child store paths 来自 generated field definitions，但 group routing 不再用 child paths 反向匹配 parent errors。

## Icon

无 icon 变更。

- 不新增 Lucide icon 或 app asset。
- 错误图标和 marker 仍由接入 app / `gpui-component` 决定。

## i18n

无新增 i18n key。

- 错误 message key 和 params 原样随 `FieldError` 进入正确字段。
- 本修复只改变错误归属，不新增文案。

## 新增依赖

不新增依赖。

## 测试计划

- group child error：`profile.nickname` 仍能进入 child `nickname`。
- sibling 同名字段：root `name` error 不能进入 child `profile.name`。
- existing array indexed validation：`headers[0].key` 仍能进入对应 row child `key`。

## 非目标

- 不支持 parent scope 中的 child-relative error path。
- 不改变 `garde` / `validify` adapter public API。
- 不改变 error visibility、focus first error 或 UI 渲染逻辑。
