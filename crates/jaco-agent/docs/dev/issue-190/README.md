# jaco-agent：补齐 ToolInvocation lifecycle snapshot 发布

## 根计划与 owner 边界

- Plan ID：`issue-190`
- 根计划：[Issue #190 root hub](../../../../../docs/dev/issue-190/README.md)
- Owner directory：`crates/jaco-agent`
- Owner plan：`crates/jaco-agent/docs/dev/issue-190/README.md`
- Owner index：[jaco-agent 开发计划](../README.md)
- Owner status：`Implemented`；snapshot publication 生产实现、focused tests 与本地 workspace 门禁已通过，canonical 完成状态跟随 root
- 消费的 root IDs：`S-01`、`S-05`、`S-08`、`S-18`、`S-19`、`C-01`、`D-01`、`D-04`、`D-08`、`R-07`、`R-13`
- Owner-local IDs：`E-201`–`E-209`、`D-201`–`D-204`、`F-201`–`F-208`、`L-201`–`L-204`、`ST-201`、`R-201`–`R-205`、`T-201`–`T-205`
- Assigned WP：`WP-201`
- Owns：在 persistence 成功后通过现有 observer/event/change contract 发布完整 invocation snapshot，覆盖初始插入和强制终态
- Does not own：Jaco UI/projection、bounded preview、approval broker、domain/schema/repository、Rig/MCP wire、真实 tool progress

## Owner-local 证据

| E-ID | 结论 | 证据 | 设计后果 |
| --- | --- | --- | --- |
| `E-201` | `AgentRuntimeEvent` 已有 `ConversationTimelineChanged` 与 `ConversationCommitted` | `src/runtime/types.rs::AgentRuntimeEvent` | 不新增 event variant或公开协议 |
| `E-202` | `ConversationChange` 已有完整 `ToolInvocationChanged`，Jaco consumer已处理其他 change | `crates/jaco-core/src/domain.rs::ConversationChange`; root `C-01` | publication payload直接使用 persisted record clone |
| `E-203` | `PersistenceContext` 已持有 conversation ID和 optional observer，并有 `emit_runtime` helper | `src/persistence.rs::PersistenceContext`; `persistence/conversation_entries.rs::emit_runtime` | 初始 snapshot可用 timeline-only event发布 |
| `E-204` | `insert_tool_invocation_and_append_call` 先插入 record，再追加 ToolCall Entry；当前只后者发布 | `persistence/tool_hook.rs::insert_tool_invocation_and_append_call`; `conversation_entries.rs::append_tool_item` | 在两个 DB动作之间补一次 snapshot event |
| `E-205` | approval request/decision/result 原子 commit 已发布 entries 后跟 invocation snapshot | `conversation_entries.rs::{append_entries_and_update_tool_invocation_full,emit_tool_entry_commit,emit_tool_entries_commit}` | 不改这些成功路径，避免重复 event |
| `E-206` | `finalize_active_tool_invocations` 对每个 active record原子追加 ToolResult并更新终态，但丢弃 commit metadata且不发布 | `runtime/finalization.rs::{finalize_active_tool_invocations,append_error_tool_result_and_update_tool_invocation}` | 方法接收 observer并发布 commit的 entries + invocation |
| `E-207` | main execution已有 `PersistenceContext` observer；explicit `cancel_run`已有 `observer`参数，finalizing lifecycle也暴露 observer；startup recovery明确没有 observer | `persistence.rs::PersistenceContext`; `runtime.rs::{cancel_run,recover_interrupted_runs}`；`runtime/lifecycle.rs::FinalizingAgentRun::observer` | main/cancel/finalizing路径实时发；`PersistedActiveAgentRun`无需新增 accessor；recovery依赖下一次 DB reload |
| `E-208` | app runtime已消费两种 runtime event并通过 FIFO publication task/drain 路由 | `app/jaco/src/features/conversation/runtime.rs::handle_runtime_event` | 不新增 consumer adapter或 channel |
| `E-209` | runtime tests已有 observer capture、append failure injection与成功/失败/取消/审批 lifecycle fixtures | `src/runtime/tests.rs`; `persistence::direct_agent_persistence_failing_append_conversation_entry` | 在现有 fixture附近补事件顺序/内容断言 |

## Owner-local 决定

| D-ID | 决定 | 依据 | 放弃的方案 | 实施落点 |
| --- | --- | --- | --- | --- |
| `D-201` | `PersistenceContext` 增加 conversation timeline change helper，初始 record通过 `ConversationTimelineChanged` 发布 | `E-201`–`E-204`、root `C-01` | 改 persistence port返回 ConversationCommit；新增 event variant；UI polling | `F-202`、`L-201`、`L-202` |
| `D-202` | 初始事件顺序固定为 persisted record snapshot在前、ToolCall Entry commit在后 | root `C-01`、`E-204` | 等 ToolCall append成功后合并发布；失败时隐藏 orphan | `F-201`、`L-202`、`ST-201` |
| `D-203` | 强制 finalization接收现有 observer，并用原子 commit返回的 conversation/entries/invocation发布 `ConversationCommitted` | `E-205`–`E-207` | 只发布 invocation；重新 query；构造平行 result event | `F-203`–`F-205`、`L-203`、`L-204` |
| `D-204` | startup recovery保持 `observer=None`；它没有 live consumer，持久化完成后由正常 reload恢复 | `E-207`、root `D-08` | 为 recovery创建 detached/global observer | `F-204`、`ST-201` |

## Owner-local 目标设计

### 文件与 ownership tree

```text
crates/jaco-agent/
├── src/
│   ├── persistence.rs                         # F-201 [Modify] expose PersistenceContext observer只读引用
│   ├── persistence/
│   │   ├── conversation_entries.rs            # F-202 [Modify] timeline-only invocation change helper
│   │   └── tool_hook.rs                       # F-203 [Modify] 初始插入后调用helper
│   ├── runtime.rs                             # F-204 [Modify] finalization各call site传递正确observer
│   ├── runtime/finalization.rs                # F-205 [Modify] 强制终态commit发布
│   └── runtime/tests.rs                       # F-206 [Modify] publication内容/顺序/缺失observer测试
└── docs/dev/
    ├── README.md                              # F-207 [Add] owner计划索引
    └── issue-190/README.md                    # F-208 [Add] 本 owner plan
```

无 `lib.rs`、Cargo manifest/lock、public trait、jaco-core、jaco-db、schema/migration、tool executor或provider adapter变更。

### L-201：timeline-only change helper

`F-202` target declaration：

```rust
impl PersistenceContext {
    pub(super) fn emit_conversation_timeline_changes(
        &self,
        changes: Vec<ConversationChange>,
    );
}
```

行为：

- empty changes直接 return。
- 有 observer时发布：

  ```rust
  AgentRuntimeEvent::ConversationTimelineChanged {
      conversation_id: self.conversation_id.clone(),
      changes,
  }
  ```

- 无 observer时无副作用；不 query数据库、不改内部 events/steps、不记录 payload。
- helper只用于已经成功持久化、但 persistence API没有返回 `ConversationCommit` summary 的 record。

### L-202：初始 invocation publication

`F-203` 中 `insert_tool_invocation_and_append_call` 的目标顺序：

1. `AgentPersistence::insert_tool_invocation` 成功，取得 authoritative `ToolInvocationRecord`。
2. 更新 `tool_calls` map，写既有 `AgentRunEvent::ToolInvocationRequested` 与 `AgentStep::ToolInvocation`。
3. 调用 `L-201`，changes恰好为：

   ```rust
   vec![ConversationChange::ToolInvocationChanged {
       invocation: Box::new(invocation.clone()),
   }]
   ```

4. 使用相同 record构造 ToolCall Entry并调用现有 `append_tool_item`；该 commit只发布 `EntryAppended`。
5. append成功返回 invocation；append失败向上返回原错误，步骤 3 已发布的 orphan与数据库事实保持一致。

不得把 runtime function入参的 status/name/input重新构造成 change；必须使用 DB 返回 record，包括生成 ID和 timestamps。

### L-203：PersistenceContext observer query

`F-201` target declaration：

```rust
impl PersistenceContext {
    pub(crate) fn observer(&self) -> Option<&AgentRuntimeObserver>;
}
```

只返回现有 borrowed observer，不 clone、不公开到 crate外、不改变 lifetime/task ownership。

### L-204：强制终态 publication

`F-205` target signatures：

```rust
impl AgentRuntime {
    pub(super) async fn finalize_active_tool_invocations(
        &self,
        agent_run_id: &str,
        conversation_id: &str,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<()>;

    pub(super) async fn append_error_tool_result_and_update_tool_invocation(
        &self,
        conversation_id: &str,
        invocation: &ToolInvocationRecord,
        status: ToolInvocationStatus,
        error: RunErrorPayload,
        observer: Option<&AgentRuntimeObserver>,
    ) -> Result<ConversationEntryId>;
}
```

在 `append_entries_and_update_tool_invocation` 成功后、拆出 `commit.value` 前，使用现有 `runtime::emit_runtime` 发布：

```rust
AgentRuntimeEvent::ConversationCommitted {
    conversation: Box::new(commit.conversation.clone()),
    changes: commit
        .value
        .0
        .iter()
        .cloned()
        .map(|entry| ConversationChange::EntryAppended {
            entry: Box::new(entry),
        })
        .chain(std::iter::once(ConversationChange::ToolInvocationChanged {
            invocation: Box::new(commit.value.1.clone()),
        }))
        .collect(),
}
```

- changes顺序严格等于 DB返回 entries顺序，invocation snapshot始终最后。
- commit失败不发布；已经成功的前一个 invocation commit不回滚，循环从错误处返回，与现有 partial finalization语义一致。
- main execution call sites传 `context.observer()`；explicit `cancel_run` 直接传它现有的 `observer: Option<&AgentRuntimeObserver>` 参数（`PersistedActiveAgentRun` 不新增 accessor）；`recover_interrupted_runs` 传 `None`。
- ordinary hook success/error/approval paths继续使用现有 `emit_tool_*_commit`，不调用 `L-204`。

### `C-01` producer implementation

- `L-202` 生产 initial `ConversationTimelineChanged`。
- 现有 approval/result helper继续生产 atomic `ConversationCommitted`。
- `L-204` 生产 cancel/fail/stop forced terminal `ConversationCommitted`。
- 所有 payload均为 persistence返回的完整 `ToolInvocationRecord`；不发 delta、不发progress、不按名称/顺序关联。
- observer/channel failure不影响 DB transaction结果；现有 runtime drain负责有 observer的event顺序。reload始终是恢复边界。

## 状态与 lifecycle

### ST-201：ToolInvocation publication

- **Authority：** `AgentPersistence` 成功返回的 `ToolInvocationRecord` / `ConversationCommit`
- **Initialization/lifetime：** 每个 `PersistenceContext`/run observer lifetime；startup recovery可无 observer
- **Readers：** root `C-01` 的 Jaco runtime consumer
- **Mutation：** 本 owner不修改 snapshot；DB methods完成 insert/update transaction
- **Publication/projection：** `L-201` timeline-only event或现有/`L-204` committed event
- **Persistence：** jaco-db现有 transaction；无新 query/schema
- **Ordering：** initial snapshot -> ToolCall Entry；atomic terminal entries -> terminal snapshot；FIFO observer channel保持发送顺序
- **Partial failure：** insert成功/ToolCall append失败保留orphan；terminal commit失败不发event；多 invocation finalization在首个错误返回
- **Cancellation/shutdown：** 使用调用方既有 observer/cancel lifecycle；不新增task/detach/shutdown逻辑

## Owner-local 工作包

### WP-201：补齐 ToolInvocation snapshot 发布

**前置与 contracts**

- root `C-01`、`D-04`、`D-08`、`R-07`、`R-13`
- `L-201`–`L-204`、`ST-201`

**File IDs**

- `F-201`–`F-206`

**实施顺序**

1. 增加 `L-201` 与 `L-203`，不改变 public `AgentPersistence` / `AgentRuntimeEvent`。
2. 在 `L-202` 的固定位置发布 persistence返回的初始 snapshot。
3. 给 `L-204` 传 observer并在 forced terminal commit后发布 entries + invocation。
4. 更新 main execution、explicit cancel、startup recovery全部 call site；让 compiler exhaustive检查遗漏。
5. 扩展现有 observer fixtures，覆盖内容、顺序、append failure、terminal状态和 no-observer reload边界。

**Failure 与 lifecycle**

- DB operation是 publication前置；失败无event。
- observer缺失只是没有 live consumer，不影响持久化成功；后续 reload恢复。
- publication不 retry、不回写数据库、不改变 agent run outcome。
- 不记录 invocation input/output/error/approval payload。

**Tests**

| R-ID | T-ID / file | 场景 | Fixture | Assertions |
| --- | --- | --- | --- | --- |
| `R-201` | `T-201` / `runtime/tests.rs` | normal tool初始插入 | observer capture + mock tool | 首个相关event为完整 `ToolInvocationChanged`；ID/status/timestamps等于DB record |
| `R-202` | `T-202` / same | injected ToolCall append failure | failing append persistence | initial snapshot仍已发布；DB有orphan；无虚构 Entry |
| `R-203` | `T-203` / same | cancel/fail forced finalization | existing active invocation fixtures | event含transaction顺序的 result/decision entries，最后是同 ID terminal snapshot |
| `R-204` | `T-204` / same | ordinary result/approval path | existing success/deny fixtures | 每个commit恰好一个 invocation change，无新增重复publication |
| `R-205` | `T-205` / same | startup recovery observer=None | interrupted run fixture | persistence terminal正确；无 observer event要求；reload record完整 |

固定测试名：

- `tool_invocation_initial_publication_contains_persisted_snapshot`
- `tool_invocation_initial_publication_survives_call_entry_failure`
- `finalized_tool_invocation_publication_contains_terminal_commit`
- `ordinary_tool_result_publication_is_not_duplicated`
- `recovery_without_observer_persists_terminal_invocation`

**Focused validation**

- `cargo fmt`
- `cargo test -p jaco-agent tool_invocation`
- `cargo test -p jaco-agent finaliz`
- `cargo test -p jaco-agent`
- `cargo clippy -p jaco-agent --all-targets --all-features -- -D warnings`
- `git diff --check`

**Done condition**

- 初始/普通/审批/强制终态的每次成功 persistence都具备 root `C-01` 要求的完整 snapshot publication；所有 call site传递正确 observer；public API、schema、manifest/lock diff为零；focused tests通过。

## Focused validation 与 handoff

| Local R-ID | Root requirement | Evidence |
| --- | --- | --- |
| `R-201`–`R-205` | root `R-07`、`R-09`、`R-13` | `T-201`–`T-205` observer/persistence tests |

- owner实现只修改 `F-201`–`F-206` production/tests；任何 public event/port/schema需求都必须先回到 root plan重审。
- 实施完成后把实际 event sequence、commands、diff 与 deviations回填 root completion evidence，不在 owner index复制进度明细。
