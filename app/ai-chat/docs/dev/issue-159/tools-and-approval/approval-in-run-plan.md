# Issue #159 审批不拆 run 计划

日期：2026-06-25

状态：已按本计划实现。问题 1/2/3/4/5 已确认；聚焦验证已完成，严格 `ai-chat2` clippy 仍受既有无关 lint 阻塞。

## 背景和目标

当前 `ai-chat2` 的工具审批会把一次用户消息拆成两个 agent run：

1. 原 run 在 `PersistingPromptHook::on_tool_call` 中创建 `tool_invocations` 和 approval request，然后用 `ToolCallHookAction::terminate("tool approval required")` 结束 Rig 工具流程。
2. app 收到 `AgentRunHandleStatus::WaitingForApproval` 后保留 active run。
3. 用户点击批准时，`ConversationRuntimeStore::approve_tool_invocation` 启动新的 task，调用 `AgentRuntime::approve_and_resume_tool` 执行工具。
4. `approve_tool_with_runtime` 再构造 `AgentRunTriggerKind::Resume` / `parent_agent_run_id` 的新 `AgentRunRequest`，继续模型。

这个模型会让一次用户动作在 `agent_runs` 中出现 parent/resume 两段。目标是改成 Codex / Zed 风格：审批是当前工具调用内部的异步 gate，批准或拒绝后继续同一个 `agent_run_id`，不再因为审批创建 `Resume` run。

非目标：

- 不重做内置文件工具能力。
- 不重做 timeline UI 视觉结构。
- 不把 app 状态迁移到 `gpui-store`。
- 不新增 shell sandbox、run_command、LSP、web tool、sub-agent。

## 目标行为

- 一个用户消息只创建一个 `agent_runs` row。
- 需要审批的工具调用在同一个 run 下进入 `tool_invocations.status = awaiting_approval`。
- approval request row 仍由 timeline 从 `tool_invocations.approval_json` 派生。
- 用户批准后：
  - 同一个 tool invocation 更新为 `running`。
  - 同一个 run 执行原工具 executor。
  - 同一个 run 写入 `ToolResult`。
  - Rig multi-turn loop 收到 tool result 后继续后续模型调用。
- 用户拒绝后，同一个 run 写入 denied/error tool result 回传模型继续，不 terminalize 当前 run。
- 不新增 approval expiry；普通审批 `expires_at = None`，不会由 broker timeout 自动过期。
- 用户 stop/cancel 当前 run 时，pending approval resolve 为 canceled，当前 run 按取消路径收口。
- 同一 run 支持多个 pending approval，但交互上每个 approval row 只批准或拒绝自己的 `tool_invocation_id`，不做“批准整个 run”或批量批准。
- 审批流程不再设置 `AgentRunTriggerKind::Resume`，不再设置 `parent_agent_run_id`。

## 文件和模块结构

### `crates/ai-chat-agent`

新增：

- `crates/ai-chat-agent/src/approval.rs`
  - 存放 source-neutral approval broker trait 和 request/decision 类型。
  - 不依赖 GPUI，不依赖 app state。

调整：

- `crates/ai-chat-agent/src/lib.rs`
  - re-export 稳定 approval 类型：
    - `ToolApprovalBroker`
    - `ToolApprovalRequest`
    - `ToolApprovalDecision`

- `crates/ai-chat-agent/src/runtime.rs`
  - `AgentRuntime` 增加 `approval_broker: Option<Arc<dyn ToolApprovalBroker>>`。
  - 增加 builder：
    - `with_approval_broker(mut self, broker: Arc<dyn ToolApprovalBroker>) -> Self`
  - 创建 `PersistenceContext` 时传入 `approval_broker.clone()`。
  - 删除 approval path 对 `context.waiting_tool_invocation_id()` 的特殊 run-splitting 处理，最终应不再从正常运行路径返回 `AgentRunHandleStatus::WaitingForApproval`。
  - 保留 interrupted-run recovery 逻辑，但它只处理进程中断或 app 重启后的数据库状态。

- `crates/ai-chat-agent/src/types.rs`
  - 目标上移除正常运行对 `AgentRunHandleStatus::WaitingForApproval` 的依赖。
  - `ApprovalResumeOutcome` 和 `approve_and_resume_tool` 迁出正常路径后应删除或只保留为迁移期私有辅助；最终不作为 app 调用 API。
  - 删除 `AgentRunRequest.parent_agent_run_id`；本轮不做旧 resume run 兼容。
  - 删除 `AgentRunTriggerKind::Resume`；retry/resend 如有需要应重新建模，不复用 approval resume 语义。
  - `AgentRuntimeEvent` 增加：
    - `ToolApprovalRequested { agent_run_id, tool_invocation_id }`
    - 只作为 app active-run 信号；timeline 数据仍由 DB snapshot reload 获取。

- `crates/ai-chat-agent/src/persistence.rs`
  - `PersistenceContext` 增加：
    - `approval_broker: Option<Arc<dyn ToolApprovalBroker>>`
  - 移除 `waiting_tool_invocation_id` 作为正常审批暂停信号的职责；正常审批不再通过 handle status 把 run task 结束。

- `crates/ai-chat-agent/src/persistence/tool_hook.rs`
  - 将 `request_tool_approval(...) -> Result<()>` 拆成两个概念：
    - `record_tool_approval_request(...) -> Result<ToolInvocationRecord>`
    - `await_tool_approval(...) -> Result<ToolApprovalDecision>`
  - `on_tool_call` 中遇到 `Ask` 或 `approval_policy != Never` 时：
    - 先持久化 approval request。
    - emit runtime event，刷新 UI。
    - await broker decision。
    - Approved：更新 approval 为 approved、invocation 为 running，然后执行 `execute_tool_invocation(...)`。
    - Denied：更新 approval 为 denied、invocation 为 denied/failed，写入 error `ToolResult`，把错误结果回传模型继续同一 run。
    - Canceled：更新 approval 为 canceled、invocation 为 canceled，写入 error `ToolResult`，然后让 cancellation 路径结束当前 run。
  - 如果没有 broker 但工具需要审批，应写入可恢复错误或设置为 failed，不能 silently auto-approve。
  - 不实现 broker timeout；新路径不产生 `ApprovalStatus::Expired`。

- `crates/ai-chat-agent/src/runtime/approval_resume.rs`
  - 废弃本文件作为 approved resume 主路径。
  - 可先拆出可复用的 helper：
    - `approval_error_payload(...)`
    - `approval_after_outcome(...)`
  - 最终删除 `approve_and_resume_tool` 对 app 的依赖。

- `crates/ai-chat-agent/src/history.rs`
  - 删除 `parent_agent_run_id` 的 resume history 构建；本轮不保留旧 resume run 读取兼容。
  - approval 新路径不再调用 `build_resume_prompt_history`。

新增类型草案：

```rust
#[async_trait::async_trait]
pub trait ToolApprovalBroker: Send + Sync {
    async fn request_tool_approval(
        &self,
        request: ToolApprovalRequest,
    ) -> ToolApprovalDecision;
}

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub conversation_id: ConversationId,
    pub agent_run_id: AgentRunId,
    pub tool_invocation_id: ToolInvocationId,
    pub request: ApprovalRequestPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolApprovalDecision {
    Approved {
        decided_by: String,
        reason: Option<String>,
    },
    Denied {
        decided_by: String,
        reason: Option<String>,
    },
    Canceled,
}
```

说明：

- `ToolApprovalDecision` 不包含 `Expired`，因为问题 4 已确认不做 expiry。
- `ai_chat_core::ApprovalStatus::Expired` 作为现有存储枚举暂不删除，避免把 approval status 清理和本轮 run-splitting 重构混在一起；新 broker 不写这个状态。

### `app/ai-chat2`

调整：

- `app/ai-chat2/src/state/conversation_runtime.rs`
  - `ConversationRuntimeStore` 继续作为唯一 GPUI global：`ConversationRuntimeGlobal(Entity<ConversationRuntimeStore>)`。
  - `ActiveRun` 增加：
    - `approval_broker: Arc<ConversationApprovalBroker>`
  - 删除 `ActiveRunPhase` 作为审批等待状态的 source of truth；是否等待审批从 `approval_broker.pending_count_for_run(...) > 0` 派生。
  - `start_run` 创建 broker，并用 `AgentRuntime::new(repository).with_approval_broker(broker)` 启动同一个 run task。
  - `approve_tool_invocation` 不再 spawn `approve_tool_with_runtime`，只 resolve broker decision。
  - `deny_tool_invocation` 不再直接调用 `AgentRuntime::decide_approval`，只 resolve broker decision。
  - 删除 `approve_tool_with_runtime(...)` 和 `resume_request_after_approval(...)`。
  - `finish_run` 不再把 `WaitingForApproval` 当作 task 完成结果；等待审批期间 run task 仍然 alive。
  - `active_waiting_approval_matches(...)` 替换为 broker 查询：
    - 当前 conversation 必须仍有 active run。
    - `tool_invocation_id` 必须存在于该 active run 的 broker pending map。
    - stale row action 返回 `false`，只触发 reload/notify，不写 DB。

新增：

- `app/ai-chat2/src/state/conversation_runtime/approval.rs`
  - 作为 `conversation_runtime.rs` 的子模块，不新增 `mod.rs`。
  - 存放 app-local broker 实现。

新增 app-local 类型草案：

```rust
pub(super) struct ConversationApprovalBroker {
    pending: Mutex<HashMap<ToolInvocationId, PendingApproval>>,
}

struct PendingApproval {
    conversation_id: ConversationId,
    agent_run_id: AgentRunId,
    sender: oneshot::Sender<ToolApprovalDecision>,
}

pub(super) struct ApprovalResolveOutcome {
    conversation_id: ConversationId,
    agent_run_id: AgentRunId,
    remaining_for_run: usize,
}

impl ConversationApprovalBroker {
    pub(super) fn resolve(
        &self,
        tool_invocation_id: &ToolInvocationId,
        decision: ToolApprovalDecision,
    ) -> Option<ApprovalResolveOutcome>;

    pub(super) fn is_pending_for_run(
        &self,
        agent_run_id: &AgentRunId,
        tool_invocation_id: &ToolInvocationId,
    ) -> bool;

    pub(super) fn pending_count_for_run(&self, agent_run_id: &AgentRunId) -> usize;

    pub(super) fn cancel_all_for_run(&self, agent_run_id: &AgentRunId) -> usize;

    pub(super) fn cancel_all(&self) -> usize;
}
```

说明：

- `ConversationApprovalBroker` 使用 `tokio::sync::oneshot`，因为 `ai-chat-agent` 和 `app/ai-chat2` 已经依赖 tokio sync。
- `pending` 使用 `std::sync::Mutex`，不为 broker 新增同步原语依赖。
- broker 不访问 DB，不持有 GPUI `Entity`，只做 async decision bridge。
- `resolve(...)` 每次只移除并 resolve 一个 `tool_invocation_id`，不提供 approve-all 或 run-wide approve。
- `request_tool_approval(...)` 如果发现重复 `tool_invocation_id`，记录 error 并返回 `Canceled`，不能覆盖已有 sender。

### `crates/ai-chat-db`

默认不新增表、不新增列。

继续使用：

- `agent_runs`
- `provider_steps`
- `tool_invocations.approval_json`
- `conversation_items`

需要调整 repository/API 的地方：

- `request_tool_invocation_approval(...)` 继续负责把 invocation 置为 `AwaitingApproval` 并写 `approval_json`。
- `update_tool_invocation_approval(...)` 继续负责 Approved/Denied/Canceled 的 JSON 决策写入；现有 Expired 分支暂不作为本轮新路径使用。
- 如 Denied/Canceled 需要和 tool result 原子写入，复用或补强：
  - `append_conversation_item_and_update_tool_invocation_full(...)`

不做：

- 不使用旧文档里提到的独立 `approval_decisions` 主路径；当前代码的真实主路径是 `tool_invocations.approval_json`。
- 不为了移除 `resume` enum 做破坏性 migration。

## UI 组件和自定义组件

不新增新的 UI 组件文件。

继续使用：

- `app/ai-chat2/src/components/conversation_detail.rs`
  - `ListState`
  - `list(...)` timeline
  - `ConversationRuntimeStore` event subscription

- `app/ai-chat2/src/components/conversation_detail/timeline.rs`
  - `ConversationTimelineRows`
  - `items_with_derived_approvals(...)`
  - approval request/decision 继续从 `ToolInvocationRecord.approval` 派生。

- `app/ai-chat2/src/components/conversation_detail/tool_blocks.rs`
  - `gpui_component::button::Button`
  - `gpui_component::Icon`
  - `gpui_component::label::Label`
  - `gpui_component::text::TextView`
  - `h_flex` / `v_flex`
  - `approval_action_buttons(...)`

UI 行为调整：

- approval row 的 approve/deny button 仍调用 `ConversationDetailPage::decide_tool_approval(...)`。
- button 回调的下游从“启动 resume run”改为“resolve 当前 run 的 pending approval”。
- 每次点击只处理当前 row 的 `tool_invocation_id`；不新增批量批准、不新增“本 run 全部同意”。
- 如果同一 run 未来因为并发工具出现多个 pending approval row，timeline 可以同时展示多行，但每行按钮互不影响。
- 当前 `RuntimeGuards::default().tool_concurrency = 1`，正常产品路径仍是一个 approval 处理完后才进入下一个工具审批。
- 点击批准后，同一 row 通过 DB reload 看到 approval status 变为 approved / invocation status 变为 running 或 succeeded。
- `ChatForm` 的 running 状态仍由 `ConversationRuntimeStore::is_running(...)` 控制；等待审批期间仍算 running，所以 stop button 保持可用。

## Icons

不新增 icon。

继续使用 `app/ai-chat2/src/foundation/assets.rs` 已有 Lucide icon：

- `ShieldAlert`：approval request / denied。
- `ShieldCheck`：approved。
- `CircleCheck`：tool success。
- `CircleAlert`：tool error。
- `FileText`、`FolderOpen`、`FileSearch`、`Search`、`FilePen`、`Terminal`、`Wrench`：现有 tool name mapping。

如果后续增加“等待用户审批中”的独立状态 icon，优先复用 `ShieldAlert`，不新增 SVG。

## i18n

默认不新增用户可见文案。

继续复用已有 Fluent keys：

- `conversation-approval-request`
- `conversation-approval-approved`
- `conversation-approval-denied`
- `conversation-approval-approve`
- `conversation-approval-deny`
- `conversation-tool-call`
- `conversation-tool-result`
- `conversation-run-failed`
- `chat-form-approval-header`
- `chat-form-approval-auto`
- `chat-form-approval-request`
- `chat-form-approval-full`

本轮不新增用户可见状态文案。若后续要把 Canceled/Expired 做成独立可见状态，而不是只体现在 tool result 文本里，再在以下文件同步新增 keys：

- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`

## 数据流

### 发送消息

```text
ConversationDetailPage::submit_message
  -> state::conversations::send_conversation_message
  -> build_run_request(...)
  -> ConversationRuntimeStore::start_run
  -> AgentRuntime::with_approval_broker(...).run_started_with_model_observed(...)
```

### 工具调用需要审批

```text
Rig model emits tool call
  -> PersistingPromptHook::on_tool_call
  -> insert_tool_invocation_and_append_call(status = AwaitingApproval)
  -> record_tool_approval_request(...)
  -> emit AgentRuntimeEvent::ToolInvocationChanged
  -> emit AgentRuntimeEvent::ToolApprovalRequested
  -> ConversationRuntimeStore derives waiting state from broker pending count
  -> ToolApprovalBroker::request_tool_approval(...).await
```

### 用户批准

```text
approval row approve button
  -> ConversationDetailPage::decide_tool_approval(approved = true)
  -> ConversationRuntimeStore::approve_tool_invocation
  -> ActiveRun.approval_broker.resolve(tool_invocation_id, Approved)
  -> if remaining_for_run == 0, active run is no longer waiting for approval
  -> PersistingPromptHook resumes inside same on_tool_call
  -> update_tool_invocation_approval(status = Running)
  -> execute_tool_invocation(...)
  -> append ToolResult
  -> Rig loop continues
  -> AgentRuntime finalizes same agent_run_id
```

### 用户拒绝

已确认采用 Codex/Zed 风格：拒绝审批不是整个 run 的 terminal 状态，而是当前工具调用的失败结果。

```text
approval row deny button
  -> ConversationDetailPage::decide_tool_approval(approved = false)
  -> ConversationRuntimeStore::deny_tool_invocation
  -> ActiveRun.approval_broker.resolve(tool_invocation_id, Denied)
  -> if remaining_for_run == 0, active run is no longer waiting for approval
  -> PersistingPromptHook resumes inside same on_tool_call
  -> update_tool_invocation_approval(status = Denied or Failed)
  -> append error ToolResult
  -> Rig loop receives tool error text
  -> model continues or explains alternative in same agent_run_id
```

### 用户停止等待中的 run

```text
ChatForm stop button
  -> ConversationRuntimeStore::stop_run
  -> active.cancellation_token.cancel()
  -> active.approval_broker.cancel_all()
  -> ToolApprovalBroker::request_tool_approval(...).await returns Canceled
  -> PersistingPromptHook records canceled approval/tool result if it still owns the invocation
  -> AgentRuntime cancellation path finalizes same agent_run_id
```

## 全局数据管理

保留现有 global：

- `ConversationRuntimeGlobal`
- `McpRuntimeGlobal`

不新增新的 GPUI global。

`ConversationRuntimeStore` 是运行中状态唯一入口：

- `active_runs: HashMap<ConversationId, ActiveRun>`
- `last_errors: HashMap<ConversationId, String>`
- `next_run_key: u64`

计划调整后的 `ActiveRun`：

```rust
struct ActiveRun {
    key: ActiveRunKey,
    agent_run_id: Option<AgentRunId>,
    cancellation_token: AgentCancellationToken,
    approval_broker: Arc<ConversationApprovalBroker>,
    run_task: Option<Task<()>>,
    _event_task: Task<()>,
}
```

等待审批期间：

- `run_task` 仍存在，保持当前 run alive。
- 不存独立 `phase`；等待状态由 `approval_broker.pending_count_for_run(agent_run_id) > 0` 派生。
- `ChatForm` 仍处于 running，用户可以 stop。
- `stop_run` 需要 cancel token，并调用 `approval_broker.cancel_all()`，让等待中的 hook 立即返回 canceled。
- 多个 pending approval 时，broker 可以保存多条 `PendingApproval`；UI 每次 resolve 一个，`remaining_for_run` 决定是否还处于等待审批。

## 数据获取方式

不新增新的查询路径。

Timeline 仍走 snapshot reload：

```text
ConversationDetailPage::reload
  -> load_snapshot(...)
  -> state::conversations::load_conversation(...)
  -> FreshRepository::conversation_timeline_records(...)
  -> ConversationTimelineRecords {
       conversation,
       project,
       items,
       attachments,
       runs,
       tool_invocations,
     }
```

approval rows 仍由 `timeline::items_with_derived_approvals(...)` 从 `tool_invocations` 派生，不直接 join 或读取额外表。

实时刷新仍靠 runtime events：

- `ConversationItemAppended`
- `ConversationItemUpdated`
- `ProviderStepChanged`
- `ToolInvocationChanged`
- `ToolApprovalRequested`
- `AgentRunStatusChanged`

`AgentRuntimeEvent::ToolApprovalRequested` 只作为 app active-run 信号，不作为 timeline 数据源。

## 数据库变更

不编写 migration 代码。

原因：

- `tool_invocations.status = awaiting_approval` 已存在。
- `tool_invocations.approval_json` 已存在。
- `agent_runs.status = running` 可以覆盖等待审批状态；等待审批是运行中 run 的 app/runtime phase，不需要持久化到 `agent_runs.status`。
- `tool_invocations.approval_json.expires_at` 继续写 `None`；新 broker 不写 `Expired`。
- `ApprovalStatus::Expired` 暂不作为 schema 清理目标，本轮只保证新运行路径不会产生它。
- 开发期旧数据库由开发人员自行清理；代码不保留 `resume` / `parent_agent_run_id` 兼容路径。

需要代码层行为变更：

- approval path 不再插入第二个 `agent_runs` row。
- approval path 不再写 `AgentRunInput.parent_agent_run_id`。
- approved tool result 的 `agent_run_id` 仍是原 run id。
- derived approval request/decision row 的 `agent_run_id` 仍来自原 invocation。
- `crates/ai-chat-core/src/payloads.rs` 删除 `AgentRunTriggerKind::Resume` 和 `AgentRunInput.parent_agent_run_id`。
- `crates/ai-chat-db/src/migrations.rs` 的 fresh schema 删除 `trigger_kind IN (..., 'resume', ...)` 中的 `resume`；不新增升级 migration。

## MCP 和 provider-hosted 工具

MCP 工具已经通过 `McpConnector::register_rmcp_tool(...)` 注册进同一个 `ToolRegistry`，并带 `ToolRunPolicy.approval_policy`。

因此本计划里的 broker 必须 source-neutral，至少覆盖：

- `ToolSource::Local`
- `ToolSource::Mcp { server_id }`

`ToolSource::ProviderHosted` 已确认不纳入本轮。

这里的区别是：

- Local tools：仓库内置工具，`ai-chat-agent` 本地执行。
- MCP tools：外部 MCP server 提供工具，但已经注册进本地 `ToolRegistry`，仍由本地 runtime 发起调用并接收结果。
- Provider-hosted tools：模型 provider 原生托管的工具，例如 provider 自己的 web search、file search、code interpreter 等。当前这类工具通过 `request.provider_tools` / provider request params 传给模型，不一定经过本地 `ToolExecutor`。

所以“问题 3”问的是：本轮 approval refactor 是否只修我们本地能拦截和执行的工具调用，还是也要同时设计 provider 原生工具的审批语义。

已确认选择 A：本轮不纳入 provider-hosted，只覆盖 `Local` + `Mcp`。provider-hosted 后续等具体 provider adapter 暴露工具事件后单独设计。

## Zed/Codex 对照：expiry 与并发

### Zed

- 普通工具审批通过 `ToolCallEventStream::run_authorization_loop(...)` 发出 `ToolCallAuthorization`，然后 await `oneshot` 用户响应。
- Zed 没有给普通用户审批设置固定 expiry/timer。
- 等待期间如果 settings 变化并变成 allow/deny，当前 prompt 会自动 resolve。
- 如果 authorization channel closed，则返回 `"authorization channel closed"` 错误。
- ACP thread 里 tool call 会进入 `ToolCallStatus::WaitingForConfirmation`；显式 cancel 会把状态改为 `Canceled`。
- 同一会话可以同时存在多个 pending permission request，但 conversation UI 默认取第一个 pending tool call，按 FIFO 顺序处理。Zed 有测试覆盖 `tc-1` 处理后才轮到 `tc-2`。
- Agent thread 使用 `FuturesUnordered<Task<LanguageModelToolResult>>` 收集 tool result，所以工具执行层支持多个 tool task 并发；审批展示层对主会话采用 FIFO pending。

### Codex

- 普通用户审批通过 `request_command_approval(...)` 把 `oneshot::Sender<ReviewDecision>` 放入当前 turn 的 `pending_approvals: HashMap<String, ...>`，发出 approval request 后 await `rx_approve`。
- Codex 没有给普通用户审批设置固定 expiry/timer；用户审批的等待由 response channel、turn interrupt 和 active turn 生命周期控制。
- `ReviewDecision::TimedOut` 主要来自 Guardian/auto-review 超时，不是普通用户 approval 的默认过期语义。
- turn 被 interrupt/abort 时，Codex 先取消 task，再 clear pending waiters，避免 pending approval 继续变成模型可见拒绝。
- 并发上，Codex 为每个 tool call 创建 future 并加入 `FuturesOrdered`；`ToolCallRuntime` 用读写锁控制并发：
  - 支持 parallel 的工具拿读锁，可并发。
  - 不支持 parallel 的工具拿写锁，会串行。
  - 默认 `ToolExecutor::supports_parallel_tool_calls()` 是 `false`。
  - `shell_command` / `unified_exec` 返回 `true`。
  - MCP 根据 server/tool 的 parallel 支持或 read-only annotation 决定。

### 对本计划的落地决策

- 问题 4 已确认选择 A：不新增 approval expiry，不设置 broker timeout；普通等待保持 `expires_at = None`，只由用户 approve/deny、stop/cancel、app recovery 结束。
- 问题 5 已确认选择 A：broker 内部继续用 `HashMap<ToolInvocationId, PendingApproval>` 支持多 pending；当前默认 `tool_concurrency = 1`，正常产品路径仍是单 pending。若未来启用并发工具，同一 run 下多个 approval row 可以按 `tool_invocation_id` 独立点击，不新增批量审批；交互语义是“一个 approval row 一次 approve/deny”。

## 依赖库

默认不新增依赖。

复用现有依赖：

- `tokio::sync::oneshot`
- `tokio::sync::Mutex` 或 `std::sync::Mutex`
- `async-trait`
- `smol::channel` 仍用于 runtime event listener，不用于 approval decision。

不新增：

- `parking_lot`
- 新的 channel crate
- 新的 UI/component crate
- 新的 DB/migration crate

## 测试计划

### `crates/ai-chat-agent`

更新或新增：

- approval broker 单元测试：
  - 需要审批时 `on_tool_call` 等待 broker。
  - Approved 后同一个 `agent_run_id` 执行工具并继续模型。
  - 不创建 `trigger_kind = Resume` 的第二个 run。
  - `tool_invocations.approval_json.status = Approved`。

- Denied/Canceled 测试：
  - Denied 写 error tool result，并回传模型继续同一 run。
  - Canceled 来自 stop/cancel/broker drop，写 canceled approval/tool result 并结束当前 run。
  - 不新增 Expired runtime 测试；新 broker 不产生 expiry。

- streaming 测试：
  - `streaming_approval_required_preserves_partial_text` 应验证 partial assistant text 保留，审批后同一 run 继续。

- history 测试：
  - approval 不再触发 `build_resume_prompt_history`。
  - 删除 `parent_agent_run_id` / resume history 兼容测试。

需要删除或改写：

- `approval_policy_pauses_run_with_pending_decision`
- `approved_builtin_tool_executes_and_completes_run`
- `denied_approval_terminalizes_tool_and_run`
- `build_resume_prompt_history` / `parent_agent_run_id` 相关兼容测试
- `canceled_approval_terminalizes_tool_and_run`
- `expired_approval_terminalizes_tool_and_run`

### `app/ai-chat2`

更新或新增：

- `approve_tool_invocation` resolve broker，不启动新 task/run。
- `deny_tool_invocation` resolve broker，不直接调用 `AgentRuntime::decide_approval`。
- waiting approval active run 保持 `run_task` alive。
- stop waiting approval cancels broker pending decision。
- stale approve/deny action 不影响已经 running 或 finished 的 run。
- 同一 run 多个 pending 时，resolve 一个 approval 不影响其它 pending；`remaining_for_run > 0` 时仍保持 waiting 派生态。

### `crates/ai-chat-db`

通常不需要新增 migration 测试。

保留/补充 repository 级测试：

- approved/denied approval JSON 更新保持现有序列化行为。
- timeline records 能加载同一 run 下的 approval request/decision 和 tool result。

## 验证命令

计划落地后至少运行：

```sh
cargo fmt
cargo test -p ai-chat-agent approval
cargo test -p ai-chat-agent runtime::tests
cargo test -p ai-chat2 conversation_runtime
cargo check -p ai-chat-agent -p ai-chat2
cargo clippy -p ai-chat-agent -p ai-chat2 --all-targets -- -D warnings
git diff --check
```

如果实际改动触及 DB serialization 或 core payload：

```sh
cargo test -p ai-chat-core
cargo test -p ai-chat-db
```

本次实现已执行：

```sh
cargo fmt
cargo check -p ai-chat-agent -p ai-chat2
cargo test -p ai-chat-agent approval
cargo test -p ai-chat-agent runtime::tests
cargo test -p ai-chat2 conversation_runtime
cargo test -p ai-chat-core
cargo test -p ai-chat-db
cargo clippy -p ai-chat-agent --all-targets -- -D warnings
```

`cargo clippy -p ai-chat2 --all-targets -- -D warnings` 已执行，但当前失败项位于既有文件：

- `app/ai-chat2/src/components/chat_form/composer_editor/element.rs`
  - `large_enum_variant`
  - `too_many_arguments`
- `app/ai-chat2/src/features/settings/mcp/dialog.rs`
  - `too_many_arguments`
  - `collapsible_if`
  - `needless_borrow`

这些失败项不在本轮新增的 approval broker / runtime 改造文件中。

## 分阶段实施

### Phase 1：引入 broker，但保持 UI 不变

- 新增 `ai-chat-agent::approval` 类型。
- `AgentRuntime` 支持 `with_approval_broker(...)`。
- `PersistenceContext` 在 approval request 后 await broker。
- `ConversationRuntimeStore` 创建 app-local broker。
- approve/deny button resolve broker。
- 目标验证：批准审批后只有一个 `agent_runs` row。

### Phase 2：移除 approved resume 主路径

- 删除 app 中 `approve_tool_with_runtime(...)` 和 `resume_request_after_approval(...)`。
- 废弃或删除 `AgentRuntime::approve_and_resume_tool`。
- 更新测试，确保 approval 不再设置 `AgentRunTriggerKind::Resume`。

### Phase 3：清理旧 resume 模型和文档

- 删除 `AgentRunTriggerKind::Resume`。
- 删除 `AgentRunInput.parent_agent_run_id`。
- 删除 `build_resume_prompt_history` 和相关测试。
- 更新 fresh schema，不写迁移代码；开发期旧数据库由开发人员自行清理。
- 更新 `tools-and-approval/README.md` 和 issue-159 status。

## 已确认问题

- 问题 1：选择 B。拒绝审批写 error tool result 回传模型继续同一 run。
- 问题 2：不做兼容，不写迁移代码；删除 `Resume` / `parent_agent_run_id` 代码和 fresh schema，开发人员自行清理旧数据库。
- 问题 3：选择 A。本轮不纳入 provider-hosted，只覆盖 local 和 MCP registry tools。
- 问题 4：选择 A。不新增 expiry，不设置 broker timeout；继续只由用户操作、stop/cancel、app recovery 处理。
- 问题 5：选择 A。支持多个 pending approval，但每次用户操作只处理一个 approval row；当前 `tool_concurrency = 1` 下实际仍是一个审批处理完再进入下一个。
