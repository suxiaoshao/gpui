# gpui-form validation pipeline strengthening plan

本文档记录 `gpui-form` 自身需要补齐的通用能力。它不写入 `jaco` 的 Provider、MCP、Prompt 或
Shortcut 业务规则；app-specific 字段、i18n key、DB/config/keychain 写回和数据源放在接入 app 文档中。

最后同步时间：2026-07-03。本文中的通用能力已经落地；验收标准保留在文末，供后续回归检查。

## 目标

强化前 `gpui-form` 已有 field store、component binding、`ValidationAdapter`、`SubmitTransform`、`FieldPath`
和 submit runtime，但使用方仍会被迫在 app 层处理以下表单内部职责：

- 手动在 submit handler 中执行字段校验。
- 手动把校验错误写回普通字段、group 或 array item。
- 手动实现 required 空值错误。
- 手动 trim/canonicalize 字段并决定是否写回 draft。
- 手动维护 array row id 到字段错误的映射。
- 手动拼装 field visible error view state。

强化后的完成口径：

- 用户输入时，binding 根据 `validate(on_change/on_blur/...)` 触发 `gpui-form` validation pipeline。
- submit 时，`gpui-form` 先执行 internal parse、transform/normalize、required rule 和 custom validator；
  invalid 时返回 `SubmitError::Invalid(FormValidationReport)`，不进入用户 handler。
- 用户 handler 只处理业务保存副作用，不再负责可归属字段的校验和 field error 写入。
- 普通字段、group 字段和 array item 字段的错误路由由 macro/runtime 统一完成。
- 正常使用路径不暴露 `apply_field_error(...)` / `clear_all_errors(...)` 这类手动错误写入 API。

## 文件和模块结构

### `crates/gpui-form/src/pipeline/validation.rs`

继续作为 public re-export 入口，新增 re-export：

- `RequiredValue`
- `RequiredRule`
- `ValidationContext`
- `ValidationContextValue`
- `FieldViewState`

### `crates/gpui-form/src/pipeline/validation/adapter.rs`

调整 `ValidationAdapter` trait，让 validator 可以读取 GPUI app context 和 form-local external context：

```rust
pub trait ValidationAdapter<Draft>: 'static {
    type Context: Clone + 'static;

    fn validate(
        &self,
        draft: &Draft,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &gpui::App,
    ) -> ValidationAdapterReport;
}
```

新增类型：

```rust
pub struct ValidationContext<'a, C> {
    pub submitted: bool,
    pub external: &'a C,
}

#[derive(Clone, Debug, Default)]
pub struct NoValidationContext;
```

`NoopValidationAdapter` 和 `GardeAdapter<T>` 使用 `Context = NoValidationContext`。

### `crates/gpui-form/src/pipeline/validation/required.rs`

新增 required 规则：

```rust
pub trait RequiredValue {
    fn is_empty_value(&self) -> bool;
}

pub struct RequiredRule {
    pub path: FieldPath,
    pub label_key: Option<Cow<'static, str>>,
    pub message_key: Cow<'static, str>,
}
```

默认实现覆盖：

- `String`
- `Option<T>`
- `Vec<T>`
- `bool` 固定为非空，避免把 checkbox/switch 当作 required consent。

自定义 value 类型由接入方实现 `RequiredValue`，例如 secret value、model selection、hotkey selection。

默认 required error：

- `code = "required"`
- `message_key = "gpui-form-error-required"`
- `params.field = label key 或 field path`

### `crates/gpui-form/src/pipeline/validation/report.rs`

保留 `ValidationIssue`，补充构造辅助：

```rust
impl ValidationIssue {
    pub fn required(path: FieldPath, label_key: Option<&'static str>, trigger: ValidationTrigger) -> Self;
}
```

不新增 app-specific error enum。所有 app validator 仍通过 `ValidationIssue` / `FieldError` 表达字段错误。

### `crates/gpui-form/src/pipeline/transform/adapter.rs`

保留 `SubmitTransform<Draft, Output>`，但让宏支持用户自定义 transform 类型，而不是只支持 identity /
validify。

新增约定：

```rust
pub trait SubmitTransform<Draft, Output>: 'static {
    fn preview(&self, draft: &Draft, context: &TransformContext) -> Result<Output, TransformReport>;
    fn transform_on_submit(&self, draft: &Draft, context: &TransformContext) -> Result<Output, TransformReport>;
}
```

现有 trait shape 可以保留；本计划重点是宏属性和 generated store 能接入任意 `SubmitTransform` 类型。

### `crates/gpui-form/src/view/state.rs`

新增 view-state helper，避免 app 自己传错 `FormMeta`：

```rust
pub struct FieldViewState {
    pub required: bool,
    pub errors: Vec<FieldErrorViewState>,
    pub has_error: bool,
}
```

`FieldViewState` 只封装可见错误和 required marker，不依赖具体 UI 库。`gpui-component` 的
`field().required(...)`、`field().error(...)` 仍由 app render 层调用。

### `crates/gpui-form/src/core/form.rs`

将 async submit 成功启动结果收敛为 `Result<(), SubmitError<E>>`，删除只有一个 variant 的
`SubmitStart`。

`FormStore` 增加：

```rust
fn validate_with_context(
    &mut self,
    trigger: ValidationTrigger,
    context: Self::ValidationContext,
    window: &mut Window,
    cx: &mut Context<'_, Self>,
) -> FormValidationReport;
```

具体 signature 以 trait object 和 macro 约束实现为准，但语义必须是：validation context 进入 form
runtime，而不是 app 在 submit handler 里绕过 pipeline。

### `crates/gpui-form-macros/src/attributes.rs`

扩展 form-level 属性：

```rust
#[form(
    store = MyFormStore,
    validation(adapter = MyValidator, context = MyValidationContext),
    transform(adapter = MyTransform)
)]
```

保留兼容写法：

```rust
#[form(validation(adapter = "garde"))]
#[form(transform(adapter = "validify"))]
```

`context = ...` 缺省为 `NoValidationContext`。

### `crates/gpui-form-macros/src/expand/pipeline.rs`

生成：

- `validation: MyValidator`
- `validation_context: MyValidationContext`
- `transform: MyTransform`
- `from_value_with_validation_context(value, context, window, cx)`
- `set_validation_context(context, cx)`
- `validation_context()`

`validate_method_body(...)` 和 `submit_validation(...)` 都必须调用用户 validator，且传入 `&App`。

### `crates/gpui-form-macros/src/expand/validation.rs`

职责变化：

- 在 validation report 写回前合并 required report。
- 对 `ValidationScope::Field` 只写回当前 field。
- 对 group / array 继续把 prefixed report route 到 child store。
- array 内部路由以当前 field path index 为主；`FormItemId` 继续作为 runtime identity，不要求 app
  自己 zip row id。

### `crates/gpui-form-macros/src/expand/errors.rs`

将 generated `apply_field_error(...)` / `clear_field_errors(...)` / `clear_all_errors(...)` 从 public
用户 API 收窄：

- 第一阶段保留为 `#[doc(hidden)] pub` 或 crate-internal macro support，供测试和迁移期使用。
- 文档不再鼓励 app 直接调用。
- 在 `jaco` 迁移完成后删除 app 正常业务路径中的调用。

## 自定义类型结构

### `RequiredValue`

接入方对复杂类型实现：

```rust
impl RequiredValue for ProviderSecretValue {
    fn is_empty_value(&self) -> bool {
        self.value.trim().is_empty()
    }
}
```

是否存在 saved secret ref 不属于 `RequiredValue` 本身，而属于 app validator context；否则会把 app
keychain 语义塞进 core crate。

### `ValidationContext<C>`

`C` 是 form-local context，不是全局单例。例如：

- 当前编辑对象 id。
- saved secret refs。
- existing ids snapshot。
- runtime capability snapshot。

`gpui-form` 只保存 `C` 并传给 validator，不解释其业务语义。

### `FieldViewState`

用于把 `field.visible_errors(form_meta)`、`field.is_required()` 和 severity/icon 文本整理成稳定快照。
它不创建 GPUI 元素、不引入 `gpui-component`，只给 app render 使用。

## 数据流

### 输入和 blur

```text
component event
  -> binding emits FormComponentEvent
  -> ComponentFieldStore syncs Draft -> Value parse
  -> generated form receives FieldChanged/FieldBlurred
  -> if field validate trigger enabled: run validation adapter with ValidationScope::Field(path)
  -> merge required report + custom report
  -> route report to field/group/array child store
  -> refresh meta and notify
```

### submit

```text
form.submit_sync / form.submit_async
  -> internal prepare_submit for component parse errors
  -> transform_on_submit(draft)
  -> write normalized output back with NormalizeOnSubmit
  -> run required + custom validation with ValidationTrigger::Submit
  -> route errors to field stores
  -> invalid: Err(SubmitError::Invalid(report))
  -> valid: call user handler(output, window, cx)
```

### array item validation

Custom validator should emit `FieldPath` values using generated path helpers, for example:

```rust
McpServerFormPaths::env_index(index).join_field("key")
```

The macro routes that path to the child row store. App code does not call row-specific `apply_*_error` helpers.

## 全局数据管理

`gpui-form` 不新增 `Global`，不读取 app-global state。需要外部状态时，form owner 把 snapshot 放入
`validation_context`，或者 validator 通过传入的 `&App` 读取 app 已有 global store。

`validation_context` 是 form entity 的局部状态，随 form drop 一起释放。

## 数据库变更

无。`gpui-form` 不访问 SQLite、TOML config、keychain、credentials 或网络。

## 数据获取方式

无内置数据获取。validator 只能使用：

- `draft`
- `ValidationScope`
- `ValidationContext<C>`
- `&App`

app-specific 查询仍在 app validator 内完成。

## 所用组件

无新增 UI 组件。`gpui-form` core 保持 UI-library agnostic。

现有 `gpui-form-gpui-component` adapter crate 继续提供 `TextInputBinding`、`NumberInputBinding`、
`BoolBinding`、select/combobox binding。required marker 和错误展示仍由接入 app 在
`gpui_component::form::field()` 上渲染。

## icon

无新增 icon。required 不使用 Lucide icon；错误 severity 的 icon 继续由 `FieldErrorViewState` /
接入 app 的现有 form render 规则决定。

## i18n

`gpui-form` core 只产出稳定 message key，不内置具体 locale 文件：

- `gpui-form-error-required`
- 保留已有 internal parse/array error key。

接入 app 负责在自己的 Fluent 文件中提供翻译，并通过现有 `FormTextResolver` / app i18n resolver 解析。

## 新增依赖库

无新增依赖。继续使用当前 optional `garde` / `validify` features。

## 验收标准

- `SubmitStart` 删除，所有 async submit 调用点改为 `Result<(), SubmitError<E>>`。
- `#[form(required)]` 字段在 submit validation 中自动生成 required error。
- `validate(on_change)` 和 `validate(on_blur)` 能触发 custom validator 并写回 field errors。
- custom validator 可以读取 form-local validation context 和 `&App`。
- custom transform 可以 trim/canonicalize output，并通过 `NormalizeOnSubmit` 写回 draft/component state。
- array item validator issues 能自动写入对应 row child store，不需要 app 手写 row error router。
- docs 和 README 不再把 handler error mapping 作为推荐字段校验路径。

## 测试计划

- `cargo test -p gpui-form required`
- `cargo test -p gpui-form validation`
- `cargo test -p gpui-form submit`
- `cargo test -p gpui-form --features form-pipeline`
- `cargo test -p gpui-form-gpui-component`
- `cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component --all-targets --all-features -- -D warnings`
