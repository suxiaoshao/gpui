# Meta and submit state design

本文记录 `gpui-form` 的 meta 派生属性和 submit 判定模型。目标是避免 `FieldMeta`、
`FormMeta`、field errors 和 `FormValidationReport` 各自成为合法性的事实源。

## 文件和模块结构

| 文件 | 职责 |
| --- | --- |
| `crates/gpui-form/src/core/meta.rs` | 保存用户交互和聚合 render snapshot；提供 `is_pristine()` 等只依赖 meta snapshot 的派生查询方法。 |
| `crates/gpui-form/src/core/submit.rs` | 保存 submit 生命周期事实：当前 submit task、attempt count、last outcome，并提供 sync/async submit runtime 类型。 |
| `crates/gpui-form/src/core/form.rs` | 暴露 store-level `is_submitting()`、`is_submitted()` 和 `can_attempt_submit()`，这些查询直接组合 submit runtime 与当前 meta snapshot。 |
| `crates/gpui-form/src/core/error.rs` | `FieldError`、`FormError`、`FormValidationReport` 是合法性事实源；`is_valid()` 只查询当前 report 是否存在 blocking error。 |
| `crates/gpui-form/src/core/field.rs` | 字段 store 保存 typed draft、default value、errors 和基础 `FieldMeta`；不再把 errors 同步成 `FieldMeta.is_valid`。 |
| `crates/gpui-form/src/core/group.rs` | group parent 缓存 child draft/meta，但合法性从 child 当前 report 聚合。 |
| `crates/gpui-form/src/core/array.rs` | array 保存 row identity、default values、array-level errors 和派生 dirty/default snapshot；合法性从 array errors 和 child reports 聚合。 |
| `crates/gpui-form/src/macro_support.rs` | hidden/internal `GeneratedFormStore` 提供 `prepare_submit` 和 `current_validation_report`，供 generated parent/group/array 递归聚合状态。 |
| `crates/gpui-form-macros/src/expand.rs` | submit 流程先执行 internal preflight，再运行 transform/validation；sync/async submit glue 在 validation 成功后调用用户传入的 handler。 |
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

`SubmitRuntime` 保存 submit 生命周期事实：

```rust
pub struct SubmitRuntime {
    task: Option<Task<()>>,
    submission_attempts: u32,
    last_outcome: Option<SubmitOutcome>,
}
```

`FormMeta` 只保存聚合 render snapshot；submit 状态从 `SubmitRuntime` 汇总或派生：

```rust
pub struct FormMeta {
    pub is_dirty: bool,
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_validating: bool,
    pub last_submit_outcome: Option<SubmitOutcome>,
    pub submission_attempts: u32,
}
```

- `is_dirty` / `is_touched` / `is_blurred` / `is_validating` 是 generated store 从字段 meta 聚合出来的 render snapshot。
- `FormStore::is_submitting()` 是 `SubmitRuntime::task.is_some()` 的派生查询，不作为 `FormMeta` 的独立保存事实。
- `submission_attempts`、`last_submit_outcome` 是 `SubmitRuntime` 保存的 submit 生命周期事实，再进入
  `FormMeta` snapshot 供 UI 渲染。
- `is_pristine()` 是 `!is_dirty`。
- `FormStore::can_attempt_submit()` 是 `!submit_runtime.is_submitting() && !form_meta.is_validating`，
  不表示数据合法。
- 不保存 `is_valid` / `can_submit` / `is_submitted` / `is_submit_successful`；这些都由 errors/report 或 submit outcome 派生。

## Submit 数据流

submit 拆成两个层次：

- `prepare_submit(...)` 的成功事实是 `Result<Output, FormValidationReport>`，只表示 internal
  preflight、normalize 和 submit validation 通过，并得到 normalized output。
- `submit_sync(...)` / `submit_async(...)` 的成功事实是 validation output 加用户 handler outcome；
  保存 DB/config/keychain 等业务副作用必须看 handler 结果，不能只看 `FormMeta`。

共享 `prepare_submit(...)` 管线：

1. `prepare_submit(cx)`：递归检查 internal UI state。内置 number input 会重新读取当前
   `InputState` 文本；parse 失败时写入 `ValidationSource::Internal` 的 field error，并立即返回
   invalid preflight report。
2. preflight invalid：返回 `Err(preflight_report)`，不执行 transform，
   不把 stale typed value 写回 component state。
3. preflight valid：执行 `SubmitTransform`，得到 normalized output。
4. `write_draft(normalized, NormalizeOnSubmit, ...)`：把 normalized output 写回 typed draft 和组件 state；
   期间不触发 change validation。
5. 执行 submit validation adapter，得到 adapter report。
6. `apply_validation_report` 写回字段和 form-level errors；internal errors 在字段侧保留。
7. `current_validation_report(cx)` 从当前 field/group/array/form errors 递归构造 final report。
8. final report valid：返回 `Ok(normalized)`。
9. final report invalid：返回 `Err(final_report)`。

同步提交：

1. `submit_sync(handler)` 先检查 `submit_runtime.task.is_some()`；busy 时直接返回
   `Err(SubmitError::Busy)`。
2. 递增 `submission_attempts` 并清空本轮 `last_submit_outcome`。
3. 调用 `prepare_submit(...)`；invalid 时记录 failure outcome，返回
   `Err(SubmitError::Invalid(report))`。
4. validation 成功后调用本次传入的 sync handler closure。
5. handler 成功时记录 success outcome；handler 失败时记录 failure outcome，并返回
   `Err(SubmitError::Handler(error))`。

异步提交：

1. `submit_async(handler)` 先检查 `submit_runtime.task.is_some()`；busy 时返回 `Err(SubmitError::Busy)`。
2. validation invalid 时返回 `Err(SubmitError::Invalid(report))`，不创建 `Task`。
3. validation 成功后调用本次传入的 async task builder；builder 同步返回
   `Err(SubmitError::Handler(error))` 时记录 failure outcome，不创建 `Task`。
4. builder 成功返回 task 后，把封装后的 `Task<()>` 存入
   `SubmitRuntime`。
5. `is_submitting()` 在 task 存在期间为 true。
6. task 完成时清空 `SubmitRuntime.task`，并根据 task outcome 更新 `last_submit_outcome`。
7. builder error 可以由 app 转成 field/form errors；task error 由 app completion callback 决定是否写回
   field/form errors 或 notification。

## 数据流和全局状态

- `gpui-form` 只持有 edit-time form store；不持有 app 全局配置、数据库连接、keychain、runtime
  provider/MCP 状态。
- `gpui-form` store 持有与表单提交等价的 submit task；task 内容由 app 在调用 `submit_async(...)` 时
  传入 handler 提供。
- `InputState` 等组件 state 是 raw UI source；`FieldCore<T>` 是 parse/commit 后的 typed draft source。
  number 字段的 dirty/default snapshot 必须以 raw input 基线计算，不能只看 typed value 是否变化。
- `FormValidationReport` 是当前合法性和错误渲染的 source；`FormMeta` 不参与合法性判定。
- group/array parent 可以缓存 child value/meta 作为 render snapshot，但 final report 必须从 child store 当前状态递归读取。

## UI、i18n、icon、依赖和存储

- 所用组件：普通 input 继续使用 `InputState` + `Input`；number binding 使用 `InputState` +
  `NumberInput`，具体 raw input dirty 设计见 `number-input-design.md`；select/combobox/bool binding 和 app
  自定义 `FormComponentBinding` 不变。
- 自定义类型：新增 `SubmitRuntime`、`SubmitError`、`SubmitStart` 和 `SubmitOutcome`；新增
  `FieldPath::join_path` 和 `FormValidationReport::with_field_prefix`
  辅助 group/array report 聚合。
- i18n：不新增用户可见文案；number parse 继续使用 `gpui-form-error-number-parse`。
- icon：无。
- 数据库变更：无。
- 数据获取方式：form core 无网络/DB 读取；只从当前 form store、GPUI component state 和调用方传入的
  submit handler 读取。
- 新增依赖：无。

## ai-chat2 使用约束

`app/ai-chat2` 不再把 `form.meta().is_valid` 或 `form.meta().can_submit` 当保存前判断。Settings
保存流程应调用 generated form `prepare_submit(...)` 做纯 validation/normalization，或调用
`submit_sync(...)` / `submit_async(...)` 把业务保存 handler 交给 form runtime。UI 禁用按钮只使用
`form.can_attempt_submit()`、从 submit task 派生的 `form.is_submitting()` 与 app 自己的 runtime blocker
组合。Provider/MCP/Prompt/Shortcut 这类与表单保存等价的 `save_task` 迁入 form submit runtime；MCP
test/refresh/OAuth sign-out、provider model fetch 等非表单任务仍留在 app state。
