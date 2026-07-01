# Meta and submit state design

本文记录 `gpui-form` 的 meta 派生属性和 submit 判定模型。目标是避免 `FieldMeta`、
`FormMeta`、field errors 和 `FormValidationReport` 各自成为合法性的事实源。

## 文件和模块结构

| 文件 | 职责 |
| --- | --- |
| `crates/gpui-form/src/core/meta.rs` | 保存用户交互和 submit 生命周期事实；提供 `is_pristine()`、`can_attempt_submit()` 等派生查询方法。 |
| `crates/gpui-form/src/core/error.rs` | `FieldError`、`FormError`、`FormValidationReport` 是合法性事实源；`is_valid()` 只查询当前 report 是否存在 blocking error。 |
| `crates/gpui-form/src/core/field.rs` | 字段 store 保存 typed draft、default value、errors 和基础 `FieldMeta`；不再把 errors 同步成 `FieldMeta.is_valid`。 |
| `crates/gpui-form/src/core/group.rs` | group parent 缓存 child draft/meta，但合法性从 child 当前 report 聚合。 |
| `crates/gpui-form/src/core/array.rs` | array 保存 row identity、default values、array-level errors 和派生 dirty/default snapshot；合法性从 array errors 和 child reports 聚合。 |
| `crates/gpui-form/src/macro_support.rs` | `GeneratedFormStore` 提供 `prepare_submit` 和 `current_validation_report`，供 generated parent/group/array 递归聚合状态。 |
| `crates/gpui-form-macros/src/expand.rs` | submit 流程先执行 internal preflight，再运行 transform/validation，最后用 current final report 决定 `Ok`/`Err`。 |
| `crates/gpui-form-macros/src/expand/validation.rs` | 生成 preflight 和 current report 聚合代码；number parse error、group report prefix、array index prefix 都在这里处理。 |
| `crates/gpui-form-macros/src/expand/accessors.rs` | `focus_first_error` 直接尝试聚焦有错误的字段/子表单，不依赖 cached `meta.is_valid`。 |

## 状态分类

`FieldMeta` 只保存字段层真实事件状态：

```rust
pub struct FieldMeta {
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_dirty: bool,
    pub is_default_value: bool,
    pub is_validating: bool,
}
```

- `is_touched` / `is_blurred` 是用户交互历史，需要保存。
- `is_validating` 是 runtime 阶段；如果后续支持 async validation，应升级为 pending count 或
  `ValidationRuntimeState`。
- `is_dirty` / `is_default_value` 是 owner 根据 value/default 或 array structure 刷新的派生 snapshot；
  保留在 `FieldMeta` 是为了 GPUI render 订阅方便，但不能作为独立事实源。
- `is_pristine()` 是 `!is_dirty`。
- 不保存 `is_valid`；字段合法性由 `errors().iter().any(FieldError::is_error)` 派生。

`FormMeta` 只保存 form 生命周期事实和聚合 snapshot：

```rust
pub struct FormMeta {
    pub is_dirty: bool,
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_validating: bool,
    pub is_submitting: bool,
    pub last_submit_outcome: Option<SubmitOutcome>,
    pub submission_attempts: u32,
}
```

- `is_dirty` / `is_touched` / `is_blurred` / `is_validating` 是 generated store 从字段 meta 聚合出来的 render snapshot。
- `is_submitting`、`submission_attempts`、`last_submit_outcome` 是 submit 生命周期事实。
- `is_pristine()` 是 `!is_dirty`。
- `can_attempt_submit()` 是 `!is_submitting && !is_validating`，不表示数据合法。
- 不保存 `is_valid` / `can_submit` / `is_submitted` / `is_submit_successful`；这些都由 errors/report 或 submit outcome 派生。

## Submit 数据流

submit 的唯一成功事实是 `Result<Output, FormValidationReport>`：

1. `begin_submit()`：设置 `is_submitting = true`，递增 `submission_attempts`，清空本轮
   `last_submit_outcome`。
2. `prepare_submit(cx)`：递归检查 internal UI state。内置 number input 会重新读取当前
   `InputState` 文本；parse 失败时写入 `ValidationSource::Internal` 的 field error，并立即返回
   invalid preflight report。
3. preflight invalid：`finish_submit_failure()`，返回 `Err(preflight_report)`，不执行 transform，
   不把 stale typed value 写回 component state。
4. preflight valid：执行 `SubmitTransform`，得到 normalized output。
5. `write_draft(normalized, NormalizeOnSubmit, ...)`：把 normalized output 写回 typed draft 和组件 state；
   期间不触发 change validation。
6. 执行 submit validation adapter，得到 adapter report。
7. `apply_validation_report` 写回字段和 form-level errors；internal errors 在字段侧保留。
8. `current_validation_report(cx)` 从当前 field/group/array/form errors 递归构造 final report。
9. final report valid：`finish_submit_success()` 并返回 `Ok(normalized)`。
10. final report invalid：`finish_submit_failure()` 并返回 `Err(final_report)`。

## 数据流和全局状态

- `gpui-form` 只持有 edit-time form store；不持有 app 全局配置、数据库连接、keychain、runtime
  provider/MCP 状态。
- `InputState` 等组件 state 是 raw UI source；`FieldCore<T>` 是 parse/commit 后的 typed draft source。
  number 字段的 dirty/default snapshot 必须以 raw input 基线计算，不能只看 typed value 是否变化。
- `FormValidationReport` 是当前合法性和错误渲染的 source；`FormMeta` 不参与合法性判定。
- group/array parent 可以缓存 child value/meta 作为 render snapshot，但 final report 必须从 child store 当前状态递归读取。

## UI、i18n、icon、依赖和存储

- 所用组件：普通 input 继续使用 `InputState` + `Input`；number binding 使用 `InputState` +
  `NumberInput`，具体 raw input dirty 设计见 `number-input-design.md`；select/combobox/bool binding 和 app
  自定义 `FormComponentBinding` 不变。
- 自定义类型：新增 `SubmitOutcome`；新增 `FieldPath::join_path` 和
  `FormValidationReport::with_field_prefix` 辅助 group/array report 聚合。
- i18n：不新增用户可见文案；number parse 继续使用 `gpui-form-error-number-parse`。
- icon：无。
- 数据库变更：无。
- 数据获取方式：无网络/DB 读取；只从当前 form store 和 GPUI component state 读取。
- 新增依赖：无。

## ai-chat2 使用约束

`app/ai-chat2` 不再把 `form.meta().is_valid` 或 `form.meta().can_submit` 当保存前判断。Settings
保存流程应直接调用 generated form `draft()` 做 app validator，或调用 `submit()` 获取
`Result<Output, FormValidationReport>`。UI 禁用按钮只使用 `form.meta().can_attempt_submit()` 与 app
自己的 busy/runtime blocker 组合。
