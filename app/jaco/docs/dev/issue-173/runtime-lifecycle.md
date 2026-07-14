# Issue #173：Runtime 生命周期、UI 投影与测试

## 1. 当前根因

当前持久化顺序来自 `conversation_items.seq`，但 UI `build_rows` 同时读取按
`created_at` 排序的 `agent_runs`。当 provider 在首个流式内容前失败时：

1. `StreamingItemAccumulator` 仍为空，`finish(Failed, None)` 不创建 item。
2. runtime 只更新 `agent_runs.status/error_json`，`output_json` 为 `NULL`。
3. timeline 遍历 items 时看不到该 Run。
4. `build_rows` 在遍历结束后把 unseen Run 追加到底部。
5. `message.rs` 再从 `run.error` 合成错误 Markdown。

这不是滚动问题，而是持久化 timeline 与执行状态的 source of truth 分裂。

## 2. State ownership

| 状态 | 唯一 owner | 生命周期/通知 |
| --- | --- | --- |
| ConversationEntries | SQLite / `FreshRepository` | append/update 后发 EntryAppended/Updated |
| AgentRun + final output link | SQLite / `FreshRepository` | 终态事务提交后发 RunStatusChanged |
| ProviderStep/ToolInvocation/approval current state | SQLite | combined repository transaction 后发对应 runtime event |
| active task/cancellation token | `ConversationRuntimeStore` + `AgentRuntime` | 一 conversation 同时一个 active Run；task 完成移除 |
| streaming accumulator | `PersistenceContext` | Run 内局部，持有当前 running Entry IDs |
| GPUI timeline rows/TextViewState | `ConversationDetailPage` | 从 snapshot 派生；不拥有业务状态 |

禁止新增镜像 `is_failed/has_error_entry` 等布尔状态。是否终态、最终 Entry 和 payload 都从现有
typed state 派生。

## 3. 请求与 Run 创建

### 用户发送

1. Composer 产生 `content_parts + attachments`。
2. `send_conversation_message` 原子追加一条 User Entry 与附件记录。
3. `AgentRunRequest` 保存 `conversation_id + trigger_entry_id`；trigger Entry 是上一步结果。
4. `begin_run` 注册工具、验证 request，然后 `insert_agent_run`。
5. repository 验证 trigger Entry 是同 conversation 的 User Message。
6. Run 从 queued 更新为 running，runtime 提交后发 Started/StatusChanged。

Run 尚未产生 Entry 时，页面可在最后一条 User Entry 后显示一个仅内存态 processing row；它由
当前 active Run 投影，不写 DB、不进入历史、复制或搜索。首条 Run Entry 到达后，该 ephemeral
row 被真实 Agent row 替代。

### Retry

Retry 创建新的 Run，但复用同一 `trigger_entry_id`。每次 Run 的 Entries 使用 Conversation 全局
`seq`，因此尝试顺序由真实写入顺序决定；不得按 Run created_at 二次重排 Entries。

## 4. 统一终态 API

在 `crates/jaco-agent/src/persistence.rs` / `conversation_entries.rs` 提供：

```rust
impl PersistenceContext {
    pub(crate) fn finish_run(
        &self,
        outcome: AgentRunOutcome,
    ) -> Result<FinishedAgentRun>;
}

pub(crate) enum AgentRunOutcome {
    Completed {
        final_entry_id: Option<ConversationEntryId>,
    },
    MaxSteps {
        final_entry_id: Option<ConversationEntryId>,
    },
    Failed {
        error: RunErrorPayload,
    },
    Canceled {
        final_entry_id: Option<ConversationEntryId>,
    },
}
```

`finish_run` 把 runtime outcome 转为 DB `FinishAgentRun`：

- Completed + existing final：引用 existing。
- Completed + None：追加 `Status(CompletedWithoutOutput)`。
- MaxSteps + existing final：引用 existing。
- MaxSteps + None：追加 `Status(MaxStepsReached)`。
- Failed：无论是否有 partial output，追加新的 Error Entry 并将其设为 final。
- Canceled + existing final：引用 existing。
- Canceled + None：追加 `Status(Canceled)`。

只有 repository transaction 成功后才：

1. 更新 `PersistenceContext.final_entry_id`；字段同步重命名。
2. push `AgentRunEvent::Completed/Failed/Canceled`。
3. emit EntryAppended（如果追加）和 AgentRunStatusChanged。

如果 DB 提交失败，Run 保持 active/non-terminal，调用方返回 storage error；不能对 UI 报告一个未
持久化的终态。

## 5. 成功与流式输出

### Streaming

- 首个 text/reasoning delta 延续现有 lazy Entry 创建。
- text Entry 完成时记录最后 assistant Entry ID，但不直接终态化 Run。
- reasoning、tool call/result、approval Entries 继续加入 input history IDs；Status/Error/Approval
  是否进入模型历史由 `history.rs` 的 payload match 明确决定。
- stream 正常结束后调用 `finish_run(Completed { final_entry_id })`。

### Blocking

`PersistingCompletionModel`/hook 持久化 assistant Entry 后设置 final ID；prompt 返回成功再统一
finish。blocking 和 streaming 不允许各自实现 Run status update。

## 6. 失败状态机

### Provider/transport/prompt failure before output

1. provider step 标记 failed。
2. active tool invocations 终态化并写必要 ToolResult error Entries。
3. 构造 `RunErrorPayload`。
4. `finish_run(Failed)` 原子追加 Error Entry + 更新 Run。
5. 提交后依次通知 Entry append 和 Run finished。

最终 Error Entry 的 `seq` 紧随当时最后 Entry，不依赖 Run created_at；对于没有并发 Run 的当前
conversation，它自然位于触发用户消息之后。

### Partial output failure

- streaming accumulator 将 partial assistant/reasoning Entry 标成 Failed。
- Run finalizer仍追加独立 Error Entry并将其设为 final。
- 折叠 Agent row 显示 Error；展开时 partial output 作为 detail 保留。
- Error Entry 会进入下一次 retry 的历史，转换为 system error context；partial failed assistant
  是否进入 history 延续当前显式规则并增加测试，禁止重复文本。

### Setup failure

`record_setup_failed_started_run` 和 `mark_setup_failed` 合并到同一 failed finalizer。不得保留一个
追加 Error、另一个只写 run.error 的分叉。

### Interrupted recovery

启动时先终态化 active provider steps/tools/approval，再对每个 queued/running Run 调用 failed
finalizer，生成 `Error { code: "interrupted" }`。恢复过程幂等：第二次启动不会新增 Error。

## 7. 取消与 max steps

### Cancel

- cancellation token 在 connect/stream/tool/approval 任一阶段触发后，先停止外部工作。
- provider steps -> Canceled；active tool invocations/approval -> Canceled。
- 有 partial assistant 时，将最后 assistant Entry 设为 final；无正文时追加 typed Canceled Status。
- canceled Run 的 `error_json` 必须为空；子 execution 可保留 canceled error payload 作为诊断。
- terminal Run 再 cancel 是幂等 no-op，不追加 Status/ToolResult/Approval Entry。

### Max steps

- Run status 仍为 Completed，stopped reason 为 MaxSteps。
- 有 assistant final 时引用它；否则追加 MaxStepsReached Status。
- max-steps 不是 provider failure，不写 `run.error_json`。

## 8. Tool 与审批

### Tool call/result

ToolCall 和 ToolResult 继续是独立 Entries，并链接同一 Run/ProviderStep/ToolInvocation。工具自身失败
但 Agent 可继续时，只写 `ToolResult { is_error: true }`；只有 Agent 整体终止时才另写 final Error。

### Approval

1. ToolInvocation 已存在。
2. `request_tool_invocation_approval_with_entry` 原子写 ApprovalRequest Entry 和 Pending state。
3. broker 等待期间 UI 直接读取该 Entry，不再派生。
4. approve/deny 调用 `decide_tool_invocation_approval_with_entry` 原子写 ApprovalDecision 与状态。
5. deny 后再写 ToolResult error Entry；approve 后执行工具。
6. app reload 后审批按钮是否可操作由 approval current state + terminal ToolResult/Decision Entries 判断。

`timeline.rs::items_with_derived_approvals`、`derived_approval_request/decision/item` 全部删除。

## 9. UI projection

### build_rows

目标算法：

1. repository 已按 `conversation_entries.seq ASC` 返回 Entries。
2. User Entry 直接生成 `UserMessageRow`。
3. 带 `agent_run_id` 的连续 Entries 按 Run 聚合为 `AgentTurnRow`；第一次遇到 Run 时确定行位置。
4. Approval Entries 已真实存在，直接进入 detail block。
5. `final_entry_id` 必须在该 Run Entries 中找到；终态找不到是 invariant error/加载失败，不 fallback。
6. 不遍历 `snapshot.runs` 追加 unseen terminal Run。
7. 仅当前 active non-terminal Run 且尚无 Entry 时追加 ephemeral tail row。

### final content

- `agent_final_markdown` 只接受 final Entry，不读取 `run.error`。
- `agent_run_terminal_fallback_markdown` 及其测试删除。
- copy text 由 Entries 生成；折叠复制 final Entry，展开复制 Run Entries。
- Error 使用 `conversation-error`；typed Status 使用新增 Fluent key。

### i18n

新增：

```text
conversation-status-canceled
conversation-status-max-steps
conversation-status-completed-without-output
```

删除不再使用的：

```text
conversation-agent-failed-fallback
conversation-agent-canceled-fallback
```

保留耗时 header：`conversation-agent-processed/failed/canceled/processing`。

## 10. Runtime/UI tests

### 贯穿根因的集成回归测试

不能用真实 OpenAI、DNS 故障或超时作为自动测试输入；这些路径速度慢且结果不稳定。测试代码在
`crates/jaco-agent/src/runtime/tests.rs` 内实现两个仅测试可见的 Rig `CompletionModel`：

```rust
#[derive(Clone)]
struct FailBeforeFirstTokenModel;

#[derive(Clone)]
struct FailAfterTextModel;
```

- `FailBeforeFirstTokenModel::stream` 在建立 stream 时立即返回
  `CompletionError::ProviderError("forced provider-open failure")`，精确覆盖截图中的首 token 前
  request/provider error；`completion` 返回同一错误以支持 blocking 用例。
- `FailAfterTextModel::stream` 返回一个 text delta，随后返回
  `CompletionError::ProviderError("forced mid-stream failure")`，覆盖 partial output 分支。
- 两个模型不得进入 production module、provider 列表或 feature flag，也不发起真实网络请求。

主回归测试
`streaming_provider_open_error_stays_before_later_user_entry_after_reload` 必须执行完整顺序：

1. fresh 临时 SQLite 中追加 User Entry A。
2. 使用 `FailBeforeFirstTokenModel` 执行 streaming Run A，等待其失败终态提交。
3. 在同一 conversation 追加 User Entry B；这一步是复现“错误沉底”不可省略的后续消息。
4. drop 当前 store/repository，使用同一路径重新 `FreshStore::open`，排除仅内存事件顺序造成的
   假通过。
5. 从 `conversation_timeline_records` 重新加载 snapshot。
6. 断言持久化 Entries 严格为 `User(A) -> Error(run A) -> User(B)`，`seq` 严格递增；failed Run
   的 `final_entry_id` 指向中间 Error，且不存在没有 final Entry 的 terminal Run。

UI 回归不重复 mock runtime，而是在
`app/jaco/src/components/conversation_detail/timeline.rs` 抽出 production `build_rows` 共用的纯投影函数
`collect_pending_rows`。测试 `persisted_error_entry_keeps_position_when_later_entries_exist` 直接使用
持久化 Entry 顺序，断言 row keys 为 `User(A) -> Agent(run A) -> User(B)`，Run 的 Entry 集合只包含中间
Error；这证明 UI 不再按 Run 顺序追加终态行，也没有 `run.error` 合成内容。

这两项组合覆盖 `runtime -> SQLite transaction -> database reopen -> repository snapshot -> UI
projection`。不以截图像素或滚动位置作为自动断言，因为本问题的正确性由持久化顺序和 row keys
决定。

### 隔离手工 UI smoke

自动化测试不依赖真实网络；需要核对真实窗口时，使用临时配置目录和临时数据目录启动当前
`Jaco.app`，创建一个仅用于测试的 OpenAI-compatible provider，把 base URL 指向本机未监听端口
（例如 `http://127.0.0.1:9`），不要填写真实 API key。按以下顺序操作：

1. 在一个 conversation 发送 User A，等待 provider connection error 出现。
2. 在同一 conversation 发送 User B。
3. 关闭并重新打开当前 bundle，重新进入该 conversation。
4. 预期顺序始终为 `User A -> Error -> User B`；Error 不应被追加到最底部，也不应重复出现。

本次隔离验证已实际执行上述流程：通过 Computer Use 展开项目并进入预置 conversation，首次加载与
关闭窗口后重新加载均显示 `fish 配置文件 A -> Error: forced provider-open failure -> fish 配置文件 B`，
错误保持在中间位置且没有重复。该手工步骤只验证真实窗口、滚动和重载投影；结束后删除临时配置、
数据库、日志和附件目录。

| Requirement | Test file | Proposed test name | Key assertions |
| --- | --- | --- | --- |
| zero-output streaming error + reload ordering | `crates/jaco-agent/src/runtime/tests.rs` | `streaming_provider_open_error_stays_before_later_user_entry_after_reload` | reopen 后 `User(A) -> Error -> User(B)`；Error 是 Run final |
| zero-output blocking error | 同上 | `blocking_provider_error_persists_final_error_entry` | 与 streaming 同 contract |
| partial failure | 同上 | `partial_stream_failure_keeps_partial_entry_and_finishes_with_error` | `FailAfterTextModel` 产生 partial + Error，两者同 Run，Error final |
| setup failure paths unified | 同上 | `setup_failure_marks_agent_run_failed` / `saved_provider_setup_failure_records_failed_run_and_error_item` | skill/provider secret/model create 路径一致 |
| interrupted recovery | 同上 | `recovery_fails_active_child_execution_rows` | 两次 recovery 只有一条 Error |
| cancel without output | 同上 | `non_streaming_cancellation_before_response_persistence_marks_run_canceled` | canceled、无 run error、Status final |
| cancel with partial output | 同上 | `cancel_with_partial_output_uses_partial_assistant_as_final` | 不额外追加 Status |
| max steps no assistant | 同上 | `max_steps_without_assistant_uses_status_final_entry` | completed + MaxSteps + Status |
| empty completed | 同上 | `completed_without_output_uses_status_final_entry` | non-null final，typed code |
| retry ordering | 同上 | `retry_runs_share_trigger_entry_and_keep_entry_sequence` | 两 Run、同 trigger、各自 final |
| approval persistence | 同上 | `approval_request_and_decision_are_persisted_entries` | reload 后 request/decision 存在且顺序正确 |
| history filtering | `crates/jaco-agent/src/history.rs` tests | `prompt_history_includes_errors_but_skips_status_and_approval_entries` | 不污染 provider protocol |
| persisted snapshot -> timeline projection | `app/jaco/src/components/conversation_detail/timeline.rs` tests | `persisted_error_entry_keeps_position_when_later_entries_exist` | 逆序 Runs 不影响 `User(A) -> Agent(error) -> User(B)` |
| final Entry required | `crates/jaco-db/src/tests.rs` | `agent_run_finalization_persists_terminal_entry_and_is_idempotent` | 不显示 run.error fallback |
| active ephemeral row | `app/jaco/src/components/conversation_detail/timeline.rs` tests | `active_zero_entry_run_gets_ephemeral_tail_row_only_while_non_terminal` | 仅 active 且尚无 Entry 时显示；Entry 到达后复用同一 Agent row，终态空 Run 不显示 |
| multimodal user Entry | `crates/jaco-db/src/tests.rs` | `multimodal_user_message_persists_text_and_attachments_in_one_entry` | 一个 User Entry，多 parts/attachments |
| i18n | `app/jaco/src/foundation/i18n.rs` tests | `conversation_terminal_status_entries_localize` | en-US/zh-CN 都不是 key fallback |

## 11. Shutdown 与错误边界

- `Task` 继续由 `ConversationRuntimeStore` 持有；不在 `Drop` 执行异步持久化。
- 正常窗口关闭触发 cancellation；若进程直接退出，下一次启动由 recovery finalizer 处理。
- provider 已成功但 final Entry/Run transaction 失败时返回 storage error，不伪造 provider failure；
  persisted partial Entries 保留，Run 留在 non-terminal，由 recovery 生成 interrupted Error。
- runtime event channel 发送失败只记录日志，不回滚已提交数据库；UI reload 可从 DB 恢复。
- 不使用字符串匹配区分错误类型；继续以 `RunErrorPayload.code/retryable/raw` 持久化结构化信息。
