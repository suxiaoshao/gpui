# Submit handler and runtime design

本文记录 `gpui-form` submit API 的目标模型。结论是：同步和异步业务提交需要两套入口，但不新增
`SubmitHandler` / `AsyncSubmitHandler` trait。handler 在调用 submit 时以 `FnOnce` closure/function 传入，
不长期存入 generated form store；form store 只保存 submit runtime state，其中异步提交中的 loading 状态由
持有的 `Task` 派生。

更新口径：本文保留 submit task ownership 和 sync/async handler 生命周期设计；其中“handler error 可由
app 映射回 field/form errors”的旧口径仅适用于迁移期或真正非字段级业务错误。字段级 required、format、
duplicate、array row 等校验应进入 `validation-pipeline-strengthening-plan.md` 中定义的 validation
pipeline，并通过 `SubmitError::Invalid(FormValidationReport)` 返回。

## 文件和模块结构

| 文件 | 计划职责 |
| --- | --- |
| `crates/gpui-form/src/core/submit.rs` | 新增 `SubmitRuntime`、`SubmitError` 和 `SubmitOutcome` 等 submit runtime 类型；`SubmitStart` 已删除，async submit 启动成功直接返回 `Ok(())`。 |
| `crates/gpui-form/src/core/form.rs` | 收敛 public `FormStore` API：保留 form-level 操作，新增 `prepare_submit`、`submit_sync`、`submit_async`；删除未接入的 `FormState`。 |
| `crates/gpui-form/src/core/meta.rs` | `FormMeta` 不再保存独立 `is_submitting` 字段；submit loading 只能通过 store/runtime 查询。 |
| `crates/gpui-form/src/core/form.rs` | `FormStore` 暴露 `is_submitting()`、`is_submitted()` 和 `can_attempt_submit()`，由 submit runtime 与当前 meta snapshot 计算。 |
| `crates/gpui-form/src/core/field.rs` | 删除或隐藏 `AnyFormField`，除非后续真的需要 path-based dynamic field lookup。`FormField` 继续作为 typed field store 边界。 |
| `crates/gpui-form/src/core/group.rs` | 删除未使用的 `FormFragment`；group 继续通过 `FieldGroupStore<Value, Store>` 和 hidden generated-store internals 运行。 |
| `crates/gpui-form/src/macro_support.rs` | `GeneratedFormStore` 保留为 macro/internal trait，必要时改名为 `GeneratedFormStoreInternals`，不作为用户-facing API 讲解。 |
| `crates/gpui-form-macros/src/expand.rs` | generated store 新增 `submit_runtime` 字段，生成 shared `prepare_submit` 管线和 sync/async submit glue。 |
| `crates/gpui-form/tests/submit.rs` | 新增 submit handler focused tests：sync success、async task lifecycle、busy 时拒绝 reentrant submit。 |
| `app/ai-chat2/src/features/settings/*` | 后续迁移 Provider/MCP/Prompt/Shortcut 的保存流程：从 dialog-local `save_task` 迁到 form store 的 async submit runtime。 |

## Trait 清理结论

当前核心 crate 中的 trait 按目标模型分组：

| Trait | 处理 |
| --- | --- |
| `FormStore` | 保留并扩展，是用户操作 form 的主入口。 |
| `FormComponentBinding` | 保留，是 UI component state 与 typed field store 的边界。 |
| `ValidationAdapter` | 保留，负责 validation strategy；未来 async validation 单独设计，不混入 submit handler。 |
| `SubmitTransform` | 保留，负责 draft -> normalized output；不负责业务保存。 |
| `FormField` | 保留，统一 typed field store 的 value/meta/errors/focus/component state 访问。 |
| `GeneratedFormStore` | 保留但隐藏为 macro/internal helper；不要在 app 计划中要求用户直接依赖。 |
| `FormState` | 删除。当前没有 impl/调用路径，职责与 `FormStore` / internals 重叠。 |
| `FormFragment` | 删除。当前 group 实现没有接入这个 trait。 |
| `AnyFormField` | 删除或隐藏。它主要服务未落地的 `FormState::field(...)`，不是当前运行路径必需抽象。 |

不新增 submit handler trait：

- `FormStore::submit_sync(...)` 直接接收
  `FnOnce(Output, &mut Window, &mut App) -> Result<Success, Error>`。
- `FormStore::submit_async(...)` 直接接收
  `FnOnce(Output, &mut Window, &mut App) -> Result<Task<Result<Success, TaskError>>, StartError>`。
- 这样用户可以直接传捕获环境的 closure，也可以传普通 function item。复杂业务逻辑放到 app helper、
  command struct 的 inherent method 或 state/repository 函数内，由 closure 调用即可。
- async handler 是同步 task builder：output 后的 contextual validation / request construction 可以返回
  `StartError`，此时不创建 task，也不会进入 `is_submitting`。
- 不把 handler 存入 form store，避免捕获过期 dialog/window/app state，也支持同一表单上的 `Save`、
  `Save & Test`、`Create & Authorize` 等多个提交动作。

放弃 `SubmitHandler` / `AsyncSubmitHandler` trait 的原因是 Rust 类型层面的，不是实现偷懒：如果 trait
方法暴露 `&mut Window` / `&mut App`，再给 closure 做 blanket impl，会要求 closure 对任意借用生命周期都成立；
实际 settings 保存 handler 需要捕获 request、dialog entity、provider id 等一次性值，无法稳定满足这个 HRTB。
把上下文包装成带生命周期的 `SubmitContext<'a>` 仍有同样问题；把上下文做成无生命周期 raw pointer 会把安全性问题
转嫁给用户，因此不采用。

## 自定义类型

`core/submit.rs` 计划新增：

```rust
pub struct SubmitRuntime {
    task: Option<Task<()>>,
    submission_attempts: u32,
    last_outcome: Option<SubmitOutcome>,
}

pub enum SubmitError<E> {
    Invalid(FormValidationReport),
    Busy,
    Handler(E),
}
```

`FormMeta` 继续作为 render snapshot，但事实源调整为：

- `FormStore::is_submitting()` 由 `SubmitRuntime::task.is_some()` 派生。
- `submission_attempts` 与 `last_submit_outcome` 保存在 `SubmitRuntime`，再汇总到 `FormMeta` snapshot。
- `FormStore::can_attempt_submit()` 由 `!submit_runtime.is_submitting() && !form_meta.is_validating`
  派生，不表示数据合法。

## Submit API 数据流

共享管线：

```text
UI component state
  -> generated form store prepare_submit(...)
  -> internal parse/preflight report
  -> SubmitTransform normalize output
  -> write normalized output back to draft/component state
  -> submit validation adapter
  -> final FormValidationReport
```

同步提交：

```text
form.submit_sync(handler)
  -> if busy: Err(SubmitError::Busy)
  -> prepare_submit
  -> invalid: Err(SubmitError::Invalid(report))
  -> valid: handler(output, window, cx)
  -> handler Ok: last_outcome = Success
  -> handler Err: last_outcome = Failure, Err(SubmitError::Handler(err))
```

同步提交在同一个调用栈内完成，不产生可观察的 loading 状态；`is_submitting()` 仍只表示已有
form-owned async task。

异步提交：

```text
form.submit_async(handler)
  -> if task.is_some(): Err(SubmitError::Busy)
  -> prepare_submit
  -> invalid: Err(SubmitError::Invalid(report)), 不创建 task
  -> valid: handler 同步构造 Task
  -> handler Err: last_outcome = Failure, Err(SubmitError::Handler(err)), 不创建 task
  -> handler Ok(task)
  -> store 保存 task
  -> returns Ok(())
  -> is_submitting() == true
  -> task 完成后清空 task
  -> Ok: last_outcome = Success
  -> Err: last_outcome = Failure，并允许把业务错误映射为 form-level 或 field-level errors
```

字段级错误映射不下沉成 submit handler。required、format、duplicate、array row 等可归属字段的错误必须由
validation pipeline 生成并通过 `SubmitError::Invalid(FormValidationReport)` 返回；app handler 只处理保存副作用。
`SubmitError::Handler(error)` 仅表达 task builder 无法启动或非字段级业务错误，例如 keychain/config runtime
错误需要弹 notification。真正异步 task 内的业务错误由 app 在 handler task / completion callback 内更新
dialog state 或弹 notification。

## UI、i18n、icon、依赖和存储

- 所用组件：无新增组件。Settings dialog 的 Save button 继续用 `gpui-component::button::Button`；
  loading/disabled 读取 `form.is_submitting()` / `form.can_attempt_submit()` 与 app runtime blocker。
- 自定义组件：无。
- icon：无新增 icon；MCP Save button 继续使用 `IconName::Plug`，其他 dialog 保持现有 icon。
- i18n：核心 crate 不新增用户可见文案；app 如果把 handler error 映射为 `FieldError`，继续使用现有
  Fluent key。
- 新增依赖库：无。使用现有 `gpui::Task`、`Window::spawn` / `Context::spawn`。
- 数据库变更：无。submit handler 只改变提交 API 和 task ownership，不改变 ai-chat2 DB schema。
- 全局数据管理：不新增 `Global` 保存活跃表单。form store 仍是 dialog/page 局部 `Entity<FormStore>`。
- 数据获取方式：form core 不读取 DB/config/keychain/network；handler 由 app 传入，app 自己读取需要的
  repository/config/keychain/runtime state。

## ai-chat2 迁移计划

| 表单 | 当前状态 | 目标 submit handler |
| --- | --- | --- |
| Provider settings | generated form store 已承载字段、validation context、transform 和 submit runtime。 | `submit_async` 托管 credentials/keychain 写入；field validation 由 Provider validator 进入 `SubmitError::Invalid(report)`，Validate 按钮也复用 form pipeline。 |
| MCP settings | generated form store 已承载字段、array row、validation context、transform 和 submit runtime。 | `McpServerFormStore::submit_async(...)` 持有保存 task；validator 使用 generated field/index path，不再维护 `McpSubmitRowIds` 或 row-specific apply helper。 |
| Prompt edit | 已使用 `PromptEditFormStore`。 | `submit_sync` handler 只执行 prompts DB create/update 和 notification；required/duplicate/trim 在 validator/transform 内完成。 |
| Shortcut edit | 已使用 `ShortcutEditFormStore`。 | `submit_sync` handler 只构造并写入 shortcut draft；hotkey/model validation 和 hotkey canonicalize 在 validator/transform 内完成。 |
| ChatForm controls | 暂不迁移。 | 运行态 composer/attachment submit guard 不纳入本轮 Settings submit handler 迁移。 |

迁移原则：

- app 不再单独维护与 form submit 等价的 `save_task`；非 form 任务，如 MCP test/refresh/OAuth sign-out，可以继续留在 app state。
- handler 里的保存 payload 字段值必须来自 submit output；app 只提供 original config、provider id、secret refs、
  OAuth draft keys、duplicate-check snapshots 等非字段上下文。
- 成功提交后是否关闭 dialog、弹 notification、刷新列表，由 handler 或 handler completion callback 完成。
- prepare validation 或 task builder validation 失败都不启动 async task。
- handler 不存入 store；每次点击 Save 时由 app 传入对应 handler。

## 测试计划

- `submit_sync_runs_handler_after_valid_prepare`：valid form 调用 sync handler，并返回 handler result。
- `submit_sync_skips_handler_when_invalid`：invalid preflight 不调用 handler，返回 `SubmitError::Invalid`。
- `submit_async_sets_is_submitting_from_task`：task 存在期间 `form.is_submitting()` 为 true，完成后为 false。
- `submit_async_rejects_reentrant_submit`：已有 task 时第二次 submit 返回 Busy。
- `submit_async_returns_handler_start_error`：task builder 同步失败返回 `SubmitError::Handler(error)`，不创建 task。
- `submit_async_does_not_store_handler`：同一个 form 可用不同 handler 连续提交，store 不保存旧 handler。
- ai-chat2 MCP focused tests：create invalid 不启动保存 task；create valid upsert config；OAuth draft promotion 仍覆盖 add/edit/rename/URL-change。
