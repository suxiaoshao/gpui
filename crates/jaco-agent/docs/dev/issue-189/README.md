# jaco-agent：发布 Agent message request usage live change

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)
- Owner directory：`crates/jaco-agent`
- Owner status：`In progress`
- 消费 root IDs：`C-02`、`C-03`、`D-02`、`D-07`、`ST-01`、`R-02`、`R-07`、`R-08`
- Assigned WP：`WP-401`
- Owns：在DB run-finalization commit返回projection后，把同一个value加入既有Conversation event
- Does not own：usage association/重算、provider-step query、schema、core coverage、UI、composer/settings

## 证据与决定

- `E-401`：provider step完成时只知道step/usage，final entry可能尚未确定。
- `E-402`：`PersistenceContext::finish_run` 与 `AgentRuntime::finish_agent_run_with_observer` 都从 `finish_agent_run` commit构造 `ConversationCommitted.changes`。
- `E-403`：Jaco runtime已通过FIFO publication task消费 `ConversationCommitted`，无需新event/channel。
- `D-401`：两个finalization producer都使用 `FinishedAgentRun.request_usage`，不得从context中重建。
- `D-402`：ordered changes固定为RunStatusChanged、可选EntryAppended、可选AgentMessageRequestUsageChanged。
- `D-403`：ProviderStepChanged保持原样；不提前发布request usage。

## 文件与 ownership tree

```text
crates/jaco-agent/
├── src/
│   ├── persistence.rs                 # F-401 [Modify] PersistenceContext finalization publication
│   ├── runtime.rs                     # F-402 [Modify] AgentRuntime finalization publication
│   └── runtime/tests.rs               # F-403 [Modify] observer content/order/live-reload tests
└── docs/dev/
    ├── README.md                      # F-404 [Modify] owner index
    └── issue-189/README.md            # F-405 [Add] 本计划
```

不修改 `AgentRuntimeEvent` enum、`AgentPersistence` trait signature、provider completion mapping、Rig adapters、Tasks、channels或shutdown。

## Boundary implementation

### L-401：PersistenceContext finalization changes

`PersistenceContext::finish_run` 在DB commit成功后、`FinishCommitted` consume value前，clone可选 `request_usage` 并构造：

```rust
let mut changes = vec![ConversationChange::RunStatusChanged {
    run: Box::new(commit.value.run.clone()),
}];
if commit.value.appended_final_entry {
    changes.push(ConversationChange::EntryAppended {
        entry: Box::new(commit.value.final_entry.clone()),
    });
}
if let Some(request_usage) = commit.value.request_usage.clone() {
    changes.push(ConversationChange::AgentMessageRequestUsageChanged {
        request_usage: Box::new(request_usage),
    });
}
```

然后沿现有 `emit_conversation_commit_with_changes` 发布。Commit失败无event；observer缺失时持久化仍成功，reload恢复。

### L-402：AgentRuntime finalization changes

`AgentRuntime::finish_agent_run_with_observer` 使用与 `L-401` 完全相同的change builder/order。为避免两处漂移，新增crate-private纯helper：

```rust
fn finished_agent_run_changes(finished: &FinishedAgentRun) -> Vec<ConversationChange>;
```

`L-401`/`L-402` 都调用该helper；helper不query、不记录、不发布，只clone DB authoritative values。

### ST-401：Live publication

- **Authority：** DB `FinishedAgentRun.request_usage`
- **Owner/lifetime：** 当前run finalization call；不存入agent field
- **Publication：** 既有 `AgentRuntimeEvent::ConversationCommitted`
- **Ordering：** root `C-03`
- **Failure：** DB失败无change；observer/channel lifecycle沿用现有规则
- **Cancellation：** canceled/failed run只有DB返回eligible projection时才发布；agent不按run status过滤或补造
- **Recovery：** startup/no-observer path依赖下一次Conversation reload

## WP-401：完成 live publication

1. 增加 `L-402` helper并让两条finalization路径调用。
2. 删除两处重复的手写changes构造，避免后续variant遗漏。
3. 扩展observer fixture覆盖normal final entry、tool-loop final step、missing usage、final status/error、no observer。
4. 断言provider-step completion event不含request usage，run finalization commit才包含。
5. 断言change order与DB projection identity/content。

| T-ID | Proposed test |
| --- | --- |
| `T-401` | `finished_agent_run_changes_publish_request_usage_after_run_and_entry` |
| `T-402` | `tool_loop_publishes_only_final_entry_request_usage` |
| `T-403` | `provider_step_completion_does_not_publish_message_request_usage_early` |
| `T-404` | `final_error_or_status_without_step_publishes_no_request_usage` |
| `T-405` | `missing_usage_event_publishes_unavailable_projection` |
| `T-406` | `no_observer_finalization_relies_on_reload_without_changing_persistence` |

### Focused validation

```sh
cargo fmt
cargo test -p jaco-agent request_usage
cargo test -p jaco-agent runtime
git diff --check
```

完成条件：`L-401`–`L-402`、`ST-401` 与 `T-401`–`T-406` 通过，无新event、query、Task或provider adapter逻辑。

## 实施证据（2026-08-20）

- 两条 finalization 路径已统一调用 `finished_agent_run_changes`，按 run、可选 entry、可选 request usage 的顺序发布 DB authoritative projection；provider-step completion未提前绑定。
- `cargo test -p jaco-agent run_finalization_publishes_request_usage_after_run_status`：1 passed；`cargo test -p jaco-agent`：123 passed。
- workspace build/test/strict clippy、`cargo fmt` 与 `git diff --check` 通过；未新增 event、query、Task、channel 或 provider adapter logic。
