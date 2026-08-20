# jaco-db：构造 Issue #189 usage 与 context projections

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)、[Composer context occupancy](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md)
- Owner directory：`crates/jaco-db`
- Owner status：`In progress`（`WP-201`、`WP-202` 均已 `Implemented`；root-level workspace/known-provider/CI gates待做）
- 消费 root IDs：`C-02`、`C-03`、`C-12`、`D-01`、`D-02`、`D-06`、`D-14`–`D-18`、`DB-01`、`DB-02`、`DB-11`、`DB-12`、`R-01`–`R-08`、`R-24`–`R-27`
- Assigned WP：`WP-201`、`WP-202`
- Owns：conversation usage query、message projection assembler、composer latest-step selector/assembler、reload records、finalization transaction result与repository tests
- Does not own：usage写入基数/schema、core语义、agent event、provider discovery、UI或Settings aggregate

## Owner-local 证据与决定

- `E-201`：`usage_events` 已含 conversation/provider/model identity、六个token columns与 `usage_json`，并对provider step唯一。
- `E-202`：`complete_provider_step_with_usage` 已原子完成step与usage；本计划不改变它。
- `E-203`：`conversation_timeline_records` 当前一次加载conversation数据，但usage只支持按step查询。
- `E-204`：`finish_agent_run_with_conn` 在一个transaction内确定final run/final entry。
- `D-201`：reload一次查询conversation usage events，再以内存ID map与已加载runs/entries/steps调用同一helper。
- `D-202`：finalization也调用同一helper；missing event是 `usage: None`，identity mismatch仍为invariant。
- `D-203`：无migration/index/schema变化；查询沿provider-step/run的现有关联按权威conversation归属取event，并由assembler校验event冗余identity。

## 文件与 ownership tree

```text
crates/jaco-db/
├── src/
│   ├── records/
│   │   ├── agent.rs                  # F-201 [Modify] FinishedAgentRun.request_usage
│   │   └── conversations.rs          # F-202 [Modify] ConversationTimelineRecords collection
│   ├── repository/
│   │   ├── agent.rs                  # F-203 [Modify] conversation usage-events query
│   │   └── conversations.rs          # F-204 [Modify] timeline assembly
│   ├── repository.rs                 # F-205 [Modify] shared assembly/finalization integration
│   └── tests/
│       └── agent.rs                  # F-206 [Modify] transaction、reload与association tests
└── docs/dev/
    ├── README.md                     # F-207 [Add] owner index
    └── issue-189/README.md           # F-208 [Add] 本计划
```

## Database contracts

### DB-201：Usage events for conversation

`F-203` target：

```rust
impl FreshRepository {
    pub fn usage_events_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<UsageEventRecord>>;
}
```

Diesel query：

```sql
SELECT <SqlUsageEventRow columns>
FROM usage_events
JOIN provider_steps ON provider_steps.id = usage_events.provider_step_id
JOIN agent_runs ON agent_runs.id = provider_steps.agent_run_id
WHERE agent_runs.conversation_id = ?1
ORDER BY created_at ASC, id ASC;
```

- 空conversation返回空vec。
- row conversion沿用 `TryFrom<SqlUsageEventRow> for UsageEventRecord`。
- event自身的冗余 `conversation_id` 不参与筛选，留给 `DB-202` 做identity校验；因此损坏数据不会静默退化成missing usage。
- 不分页；query只加载当前打开conversation的数据，不是Settings全历史聚合。

### DB-202：Single assembly helper

`F-205` target：

```rust
fn agent_message_request_usage_from_parts(
    run: &AgentRunRecord,
    final_entry: &ConversationEntryRecord,
    provider_step: &ProviderStepRecord,
    usage: Option<&UsageEventRecord>,
) -> Result<Option<AgentMessageRequestUsage>>;
```

严格执行root `DB-02`：

- assistant eligibility通过parsed `ConversationEntryPayload::Message { role: TranscriptRole::Assistant, .. }` 判断。
- run/final entry/step/conversation/provider/model IDs全部校验。
- step非Completed、无completed_at、final entry无step ID返回None。
- usage missing返回Some projection with None；usage存在则校验冗余identity并clone `usage_json` parsed snapshot。
- provider kind取 `provider_step.settings_snapshot.provider_settings.provider_kind`。

### DB-203：Reload assembly

`F-204` 中 `conversation_timeline_records`：

1. 按既有方式加载conversation/project/items/attachments/runs/provider steps/tool invocations。
2. 调用 `DB-201` 一次。
3. 为entries、steps、events建立borrowed ID maps。
4. 仅遍历有 `output.final_entry_id` 的runs；找到parts后调用 `DB-202`。
5. 收集Some projection，按final entry seq/ID稳定排序。
6. 写入 `ConversationTimelineRecords.agent_message_request_usages`。

任何identity invariant失败使整个timeline load失败并沿现有 `ConversationProblem` 呈现，不回退到猜测关联。

### DB-204：Finalization transaction

`F-201` 增加：

```rust
pub struct FinishedAgentRun {
    pub run: AgentRunRecord,
    pub final_entry: ConversationEntryRecord,
    pub appended_final_entry: bool,
    pub request_usage: Option<AgentMessageRequestUsage>,
}
```

`finish_agent_run_with_conn` 在run/final entry更新后、transaction返回前：

1. final entry无step ID或不是eligible assistant Message：`request_usage = None`。
2. 有step ID：用同一connection加载exact step与可选usage event；调用 `DB-202`。
3. helper invariant/query失败：rollback run-finalization transaction；不发布change。
4. missing usage event：transaction成功并返回projection `usage: None`。

已在更早transaction完成的provider step与usage不属于本transaction rollback范围。

## WP-201：实现 repository projection

1. 扩展 `F-201`/`F-202` records。
2. 实现 `DB-201` 与deterministic query tests。
3. 实现 `DB-202` 及normal/tool-loop/missing/mismatch table tests。
4. 接入 `DB-203` reload。
5. 接入 `DB-204` finalization并覆盖rollback。
6. 更新所有records fixtures与schema validation fixtures；确认migration/schema文件无diff。

### Tests

| T-ID | Proposed test |
| --- | --- |
| `T-201` | `usage_events_for_conversation_filters_and_orders_deterministically` |
| `T-202` | `timeline_load_associates_final_assistant_entry_with_exact_step_usage` |
| `T-203` | `timeline_load_does_not_sum_tool_loop_steps` |
| `T-204` | `timeline_load_preserves_missing_usage_as_unavailable` |
| `T-205` | `timeline_load_rejects_cross_run_or_cross_conversation_usage_identity`，包括event conversation字段损坏时reload报invariant |
| `T-206` | `finish_agent_run_returns_same_request_usage_as_reload` |
| `T-207` | `request_usage_assembly_failure_rolls_back_run_finalization` |
| `T-208` | `failed_or_canceled_provider_step_has_no_message_request_usage` |

### Focused validation

```sh
cargo fmt
cargo test -p jaco-db agent_message_request_usage
cargo test -p jaco-db usage_event
git diff --check
```

完成条件：`DB-201`–`DB-204` 与 `T-201`–`T-208` 通过，`migrations.rs`、`schema.rs`、Cargo manifests与 `Cargo.lock` 无变化。

## 实施证据（2026-08-20）

- 已实现 conversation usage 单次查询、唯一 assembler、reload hydration 与 run-finalization transaction projection；normal/no-sum、missing event、非 completed step、identity mismatch rollback及reload一致性均有测试。
- `cargo test -p jaco-db agent_message_request_usage`：5 passed；`cargo test -p jaco-db`：55 passed（包含 corrupt-candidate 回归）。
- `cargo fmt` 与 selected-package combined strict clippy 通过；workspace-wide `cargo build`、`cargo test`、`cargo clippy`、known/provider 场景与三平台 CI 未执行。
- `migrations.rs`、`schema.rs`、Cargo manifests、`Cargo.lock` 与 serialized usage format均未修改。

## Composer extension — `WP-202`（Implemented）

本节只登记 [composer 执行文档](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md) 的 DB owner contract；`WP-201` 的 message projection 与实施证据保持不变。

### Owner-local 文件与边界

```text
crates/jaco-db/src/
├── records/agent.rs                  # F-211 [Modify] FinishedAgentRun.context_request_usage
├── records/conversations.rs          # F-212 [Modify] reload singular fact
├── repository.rs                     # F-213 [Modify] selector/assembler/finalization
├── repository/conversations.rs       # F-214 [Modify] timeline reload integration
├── tests.rs                          # F-216 [Modify] capability fixture fallout
└── tests/agent.rs                    # F-215 [Modify] ordering, identity, parity, rollback
```

- 复用现有 `agent_runs`、`provider_steps`、`usage_events` 与 settings snapshots；无 migration/schema/index。
- composer selector独立于 `AgentMessageRequestUsage` 的 final-entry association；不得从消息projection或entry presence筛选candidate。
- selector不接受current provider/model，不执行conversation sum或per-model backscan。

### `DB-211`：Latest eligible request selector

按 root `DB-11` 选择一个父run为Completed、step为Completed的candidate，固定降序为：

```text
step.completed_at,
run.completed_at,
step.seq,
step.id
```

同一 helper 同时支持：

1. reload 已加载 collections + usage map。
2. run-finalization transaction 中的 connection-scoped exact records。

Assembler 校验 conversation/run/step/settings/provider/model/usage-event identities；missing event 返回 `Some(fact { usage: None })`，损坏或cross-identity返回invariant error。

### `DB-212`：Reload/finalization parity

- `ConversationTimelineRecords.latest_context_request_usage` 保存 selector 结果。
- `FinishedAgentRun.context_request_usage` 是publication delta：Completed run仅当selector的全局latest fact属于本次run时返回该fact。
- 新鲜终态写入与现有terminal early-return分支都调用同一selector；重复finalize当前latest Completed run重建同一fact，重复finalize旧Completed run返回None。
- fresh/early-return failed/canceled、无eligible step或已有更新run胜出时返回 `None`，不让已完成的中间step替换旧composer状态。
- finalization在commit前组装；query/assembler error rollback本次run终态写入。
- 同一次成功完成的live return与立刻reload值完全相等。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-211` | `composer_context_selects_latest_completed_step_deterministically_without_sum` |
| `T-212` | `composer_context_preserves_missing_usage_for_latest_candidate` |
| `T-213` | `composer_context_excludes_running_failed_and_canceled_runs` |
| `T-214` | `composer_context_identity_mismatch_rolls_back_finalization` |
| `T-215` | `composer_context_finalization_projection_matches_reload` |
| `T-216` | `composer_context_terminal_idempotent_finalize_rebuilds_only_current_latest_completed_delta` |

```sh
cargo fmt
cargo test -p jaco-db composer_context
git diff --check
```

完成条件：root `DB-11`/`DB-12`、`T-211`–`T-216` 通过，并明确确认 migration、schema、index、Cargo 与 serialized usage format 无 diff。

## Composer 实施证据（2026-08-20）

- `WP-202` 已 `Implemented`；`cargo test -p jaco-db`：55 passed，`cargo test -p jaco-db composer_context`：7 passed，包含 corrupt-candidate 回归与 finalization/reload parity。
- `cargo fmt` 与 `cargo clippy -p jaco -p jaco-agent -p jaco-db --all-targets --all-features -- -D warnings` 通过；workspace-wide build/test/clippy、known/provider 场景与三平台 CI 未执行。
- migration、schema、index、Cargo manifests、`Cargo.lock` 与 serialized usage format 无 diff；implementation commit/PR：`Pending`。
