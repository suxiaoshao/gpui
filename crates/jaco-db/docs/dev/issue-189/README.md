# jaco-db：构造 Issue #189 usage、context、analytics 与 cost projections

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)、[Composer context occupancy](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md)、[Settings usage analytics](../../../../../docs/dev/issue-189/settings-usage-analytics-plan.md)、[Settings activity heatmap](../../../../../docs/dev/issue-189/settings-usage-activity-heatmap-plan.md)、[Settings 请求费用统计](../../../../../docs/dev/issue-189/settings-usage-cost-analytics-plan.md)
- Owner directory：`crates/jaco-db`
- Owner status：`Implemented`（`WP-201`–`WP-205`均已实施）
- 消费 root IDs：既有IDs，以及`C-83`、`C-84`、`R-87`–`R-90`、`R-93`
- Assigned WP：`WP-201`、`WP-202`、`WP-203`、`WP-204`、`WP-205`
- Owns：既有usage/context/analytics projections；provider model price、step price snapshot、usage-event amount原子持久化、cost聚合与schema tests
- Does not own：models.dev HTTP、Token price calculation、local calendar preset、Settings Operation/UI或本地手工迁移执行

## Owner-local 证据与决定

- `E-201`：`usage_events` 已含 conversation/provider/model identity、六个token columns与 `usage_json`，并对provider step唯一。
- `E-202`：`complete_provider_step_with_usage` 已原子完成step与usage；本计划不改变它。
- `E-203`：`conversation_timeline_records` 当前一次加载conversation数据，但usage只支持按step查询。
- `E-204`：`finish_agent_run_with_conn` 在一个transaction内确定final run/final entry。
- `E-205`：Settings全局范围查询必须按`created_at`过滤；现有`(conversation_id, date_key)`索引不能支持该access path。
- `E-206`：app已直接依赖jaco-db，analytics snapshot只有DB producer与Settings consumer，不需要提升到core。
- `D-201`：reload一次查询conversation usage events，再以内存ID map与已加载runs/entries/steps调用同一helper。
- `D-202`：finalization也调用同一helper；missing event是 `usage: None`，identity mismatch仍为invariant。
- `D-203`：`WP-201`/`WP-202` 无migration/index/schema变化；查询沿provider-step/run的现有关联按权威conversation归属取event，并由assembler校验event冗余identity。
- `D-204`：`WP-203` 在schema version 1的0001 fresh schema直接增加created-at index与DB-owned typed aggregate；不新增migration/旧库兼容层，不改usage rows/JSON，也不让app加载全量events。

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

## Settings extension — `WP-203`（Implemented）

本节只登记 [Settings analytics 执行文档](../../../../../docs/dev/issue-189/settings-usage-analytics-plan.md) 的 DB owner contract。`WP-201`/`WP-202` 的message/composer实现和证据保持不变。

### Owner-local 文件与边界

```text
crates/jaco-db/src/
├── migrations.rs                         # F-221 [Modify] version 1 fresh created_at index
├── records.rs                            # F-222 [Modify] module/export
├── records/analytics.rs                  # F-223 [Add] C-21 public query projection
├── repository.rs                         # F-224 [Modify] module declaration
├── repository/analytics.rs               # F-225 [Add] aggregate transaction/queries
├── tests.rs                              # F-226 [Modify] test module
└── tests/analytics.rs                    # F-227 [Add] focused DB tests
```

- public analytics types只放 `records/analytics.rs` 并经现有records exports导出；SQL row structs留在repository module私有。
- 不修改`records/agent.rs::UsageEventRecord`、usage write transaction、`schema.rs`、serialized `usage_json`、Cargo manifests或`Cargo.lock`。
- repository不接受period enum或系统时区；app传入已经确定的 `UsageAnalyticsRange`。

### `DB-221`：Public query boundary

严格实现root `C-21`：

```rust
impl FreshRepository {
    pub fn usage_analytics(
        &self,
        range: UsageAnalyticsRange,
    ) -> Result<UsageAnalyticsSnapshot>;
}
```

方法从pool取一个connection，在同一个read transaction中依次构造summary、finite daily与provider/model buckets。empty是成功的zero summary；DB/parse/overflow/invariant失败都返回`Err`，不返回partial snapshot。

### `DB-222`：Filter、coverage 与 exact aggregation

- finite filter只有 `created_at >= start_utc AND created_at < end_utc`；start/end使用`TimestamptzSqlite` bind，AllTime无range predicate。
- daily key使用 `strftime('%Y-%m-%d', created_at, printf('%+d seconds', ?))`，绑定`UtcOffset::whole_seconds()` integer；不读取`date_key`。
- all-zero predicate必须检查input/output/cache-read/cache-write/reasoning/total全部六列。
- reported predicate是六列任一非零，包含只有`total_tokens`非零的row；total-covered只检查`total_tokens > 0`。
- 所有SUM只用`COALESCE(..., 0)`处理empty-row SQL NULL；六列分别direct SUM，禁止将cache或reasoning再次加到input/total。
- 同一SQL row额外返回negative-field count；非零时`DbError::Invariant`。所有SQLite signed结果checked-convert到u64；SUM overflow保持Diesel error。
- finite结果由repository补齐 `[local_start, local_end)` 每个local date的zero bucket；AllTime `daily` 固定为空。

### `DB-223`：Stable grouping、labels 与 invariants

- aggregate CTE只按`usage_events.provider_id, usage_events.model_id`分组。
- CTE完成后left join `providers.id` 与 `provider_models(provider_id, model_id)`；label是当前显示projection，不能成为identity/group key。
- sort：`total_tokens DESC, provider_id ASC, model_id ASC`。
- 每个aggregate及snapshot验证`reported + unreported == requests`、`total-covered <= reported`。
- finite daily与provider/model buckets的requests、coverage counts和六个token sums分别重新checked-sum，并与summary完全相等。
- conversation/run/entry status不进入query；存在的每条usage event都是一个eligible request。

### `DB-224`：Schema v1 fresh index 与 query plan

`migrations.rs` 的 `CREATE_FRESH_SCHEMA_SQL` usage index区追加：

```sql
CREATE INDEX idx_usage_events_created_at ON usage_events(created_at);
```

`SCHEMA_VERSION`固定保持1，`MIGRATIONS`固定保持唯一`0001_create_fresh_schema`。不新增0002、旧库检测、自动repair/backfill或兼容错误。fresh DB直接得到index；已有本地DB由使用者自行重建或手工执行同一SQL，缺index只影响query plan、不改变结果语义。`schema.rs`无index表示，因此不得产生diff。production同形finite predicate的`EXPLAIN QUERY PLAN`必须包含使用该index的`SEARCH`；不预先增加复合宽index。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-221` | finite range rejects empty/reversed/non-midnight-local bounds |
| `T-222` | UTC half-open start/end与故意错误`date_key` |
| `T-223` | positive/negative/sub-hour offset daily bucket |
| `T-224` | one turn multiple steps remain multiple requests |
| `T-225` | all-zero/partial/total-covered predicates include all six columns |
| `T-226` | six independent sums、cache no-double-count与large exact integer |
| `T-227` | negative stored value与SUM overflow fail explicitly |
| `T-228` | dense finite days、AllTime no daily、deterministic date order |
| `T-229` | stable provider/model IDs、label rename/missing与deterministic sort |
| `T-230` | summary/daily/group cross-total invariants与empty snapshot |
| `T-231` | fresh schema remains version1/one migration and contains created-at index; no upgrade path |
| `T-232` | production-shaped range query uses `idx_usage_events_created_at` |

```sh
cargo fmt
cargo test -p jaco-db usage_analytics
cargo test -p jaco-db bootstrap
cargo clippy -p jaco-db --all-targets --all-features -- -D warnings
git diff --check
```

完成条件：root `C-21`、`DB-21`–`DB-23`、`R-42`–`R-51`与`T-221`–`T-232`通过；schema version仍为1且只有0001，fresh SQL只增加index，usage rows/JSON、Diesel schema、Cargo与lockfile无其他变化。

## Settings 完成证据

- `WP-203`：`Implemented`。`records/analytics.rs` 与 `repository/analytics.rs` 已落地；0001 fresh schema在version 1直接增加`idx_usage_events_created_at`，无upgrade/compatibility path。
- `cargo test -p jaco-db usage_analytics`：11 passed；覆盖真实同run多completed steps后run失败仍逐event计数、六字段coverage/sum、边界/offset/dense dates、错误/invariant、stable IDs/labels，以及直接复用三条生产SQL的query-plan断言。
- `cargo test -p jaco-db bootstrap`：5 passed；strict jaco-db clippy、`cargo fmt`与`git diff --check`通过。
- Manual Settings matrix、workspace-wide gates、three-platform CI与implementation commit/PR：`Pending`。

## Activity heatmap extension — `WP-204`（Implemented）

本节只登记[Settings activity heatmap执行文档](../../../../../docs/dev/issue-189/settings-usage-activity-heatmap-plan.md)的DB owner contract。它以rolling activity daily替换`WP-203`中供LineChart消费的selected daily projection；`WP-203`已实施的aggregate、provider/model projection、created-at index、transaction和历史证据继续有效。

### Owner-local 文件与边界

```text
crates/jaco-db/src/
├── records/analytics.rs                  # F-231 [Modify] query/activity/snapshot
├── repository/analytics.rs               # F-232 [Modify] selected + activity transaction
├── repository.rs                         # F-233 [Modify] production SQL test exports
└── tests/analytics.rs                    # F-234 [Modify] WP-204 focused tests
```

- 不修改`UsageEventRecord`、usage write transaction、serialized `usage_json`、fresh schema、`SCHEMA_VERSION`、migration或Cargo manifests。
- repository不接受period、today或系统时区；app传入两个已经确定的range。
- `UsageAnalyticsAggregate`、`UsageAnalyticsDailyBucket`、`UsageAnalyticsRange`和`UsageAnalyticsFiniteRange`保持现有语义。

### `DB-231`：Breaking public query boundary

严格实现root `C-22`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsQuery {
    pub selected_range: UsageAnalyticsRange,
    pub activity_range: UsageAnalyticsFiniteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsActivity {
    pub range: UsageAnalyticsFiniteRange,
    pub summary: UsageAnalyticsAggregate,
    pub daily: Vec<UsageAnalyticsDailyBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsSnapshot {
    pub selected_range: UsageAnalyticsRange,
    pub selected_summary: UsageAnalyticsAggregate,
    pub provider_models: Vec<UsageAnalyticsProviderModelBucket>,
    pub activity: UsageAnalyticsActivity,
}

impl FreshRepository {
    pub fn usage_analytics(
        &self,
        query: UsageAnalyticsQuery,
    ) -> Result<UsageAnalyticsSnapshot>;
}
```

保留`UsageAnalyticsProviderModelBucket`、`ProviderModelAggregateSqlRow`、provider-model SQL、loader、snapshot field与exports。删除`selected_daily` snapshot field且不保留deprecated alias、旧signature wrapper或compatibility constructor；daily loader继续服务activity projection。

### `DB-232`：Single transaction与invariants

一个pooled connection、一个deferred read transaction内固定执行：selected summary → selected provider/models → activity summary → activity daily。两个range相同也独立执行，不增加cache/fallback。

- selected provider/model buckets逐字段checked sum等于selected summary，稳定ID、当前label与确定性排序语义保持不变。
- activity daily逐字段checked sum等于activity summary，日期strict ascending、dense且恰好覆盖activity range。
- `load_daily`继续支持任意合法finite range；DB不硬编码365，Jaco consumer负责该产品约束。
- 任一SQL/decode/negative/overflow/date/invariant失败返回单一`Err`，不返回partial snapshot。
- eligible universe仍是每条persisted usage event，不按run最终status过滤，也不把cache/reasoning二次加入total。

### `DB-233`：SQL与query plan

- summary/daily继续复用production finite SQL和`idx_usage_events_created_at`。
- provider/model继续按稳定IDs聚合并left join当前`providers`/`provider_models`显示label。
- EXPLAIN测试直接复用production `SUMMARY_FINITE_SQL`、`DAILY_FINITE_SQL`与provider-model SQL。
- AllTime summary table scan保持预期；无新宽索引、schema或migration。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-241` | finite selected provider-model与365-day activity各自cross-total一致 |
| `T-242` | AllTime仍返回provider-model聚合，activity有365个bucket，snapshot无selected daily |
| `T-243` | positive/negative/half-hour fixed offset跨年与leap day |
| `T-244` | selected/activity empty、all-zero与independent totals |
| `T-245` | multiple provider steps仍逐event累计且不依赖parent run final status |
| `T-246` | negative/SUM overflow/date/invariant使整个snapshot失败 |
| `T-247` | production summary/daily SQL使用created-at index |
| `T-248` | public API无selected daily；provider-model SQL/type/label join继续存在且被consumer覆盖 |

```sh
cargo fmt
cargo test -p jaco-db usage_analytics
cargo clippy -p jaco-db --all-targets --all-features -- -D warnings
git diff --check -- crates/jaco-db
```

完成条件：root `C-22`、`DB-31`–`DB-33`、`R-63`–`R-66`、`R-77`–`R-78`与`T-241`–`T-248`通过；schema/version/migration/usage authority无diff；`WP-203`的非provider projection回归保持通过。

### Activity实施证据

- 最终snapshot保留`selected_range`、`selected_summary`、`provider_models`与`activity`；selected daily snapshot/query已删除。selected provider/model与activity daily分别在同一read transaction中完成checked cross-total。
- `cargo test -p jaco-db usage_analytics --no-fail-fast`通过12项；`cargo test -p jaco-db --no-fail-fast`通过67项；strict clippy、`cargo fmt --package jaco-db`与scoped diff check通过。
- 无schema、migration、version或兼容API变化；`WP-601`组件实现不受影响。

## Cost extension — `WP-205`（Implemented）

本节登记[Settings 请求费用统计计划](../../../../../docs/dev/issue-189/settings-usage-cost-analytics-plan.md)的DB owner contract。既有usage/context/analytics authority、created-at index与activity token projection保持不变。

### Owner-local文件与边界

```text
crates/jaco-db/src/
├── migrations.rs
├── schema.rs
├── records/
│   ├── analytics.rs
│   ├── provider_step.rs
│   ├── resources.rs
│   └── usage.rs
└── repository/
    ├── analytics.rs
    ├── provider_step.rs
    └── resources.rs
```

### `DB-241`：Fresh schema与catalog price

Fresh schema version 1只追加三个nullable列：

```sql
provider_models.pricing_json JSON NULL
provider_steps.pricing_snapshot_json JSON NULL
usage_events.cost_amount_nano_usd INTEGER NULL
    CHECK (cost_amount_nano_usd IS NULL OR cost_amount_nano_usd >= 0)
```

- `ProviderModelRecord`/`NewProviderModel`原样roundtrip core pricing snapshot；missing/unmatched price保持NULL。
- 不新增`0002`、schema version、runtime detection、repair、compatibility或backfill；仓库不保存本地手工迁移SQL。
- 不增加cost index；selected analytics继续使用现有created-at路径。

### `DB-242`：Provider-step request-time snapshot

- `NewProviderStep`不接受caller-supplied price。
- `insert_provider_step`在插入Running step的同一transaction中，按本地provider ID与provider-facing model ID查当前`provider_models.pricing_json`。
- 只有row provider/model、pricing models.dev model ID以及pricing route key与从step settings生成的typed route key完全一致时才写`pricing_snapshot_json`；missing或mismatch写NULL，禁止family/cross-provider借价。
- 返回的`ProviderStepRecord`携带typed frozen snapshot；目录refresh只影响以后插入的step，不修改running/completed step。

### `DB-243`：Usage amount原子持久化

- `CompleteProviderStep`增加`cost_amount: Option<UsdNanoAmount>`。
- 既有complete-step transaction同时写terminal state、usage event和nullable amount；DB不解析models.dev、不读取current catalog重新定价，也不计算Token费用。
- amount必须可表示为non-negative SQLite INTEGER。unknown为NULL，explicit free为0。
- failed/canceled step、缺usage event的既有路径和unique provider-step usage约束保持不变；DB失败整体rollback。

### `DB-244`：Selected analytics

- `UsageAnalyticsAggregate`与provider/model bucket增加`priced_request_count`和`estimated_cost_nano_usd`。
- SQL以`cost_amount_nano_usd IS NOT NULL`统计coverage并只SUM non-NULL金额；所有count/sum都checked转成u64。
- selected summary与provider/model buckets对request、priced count和amount做cross-total。
- snapshot增加`selected_cost_daily`稀疏投影：finite使用selected range offset，AllTime使用同query捕获的activity offset；只聚合cost非NULL事件，explicit zero保留，不补缺失日期，并与selected summary的priced count/amount做checked cross-total。
- activity summary/daily、heatmap与provider/model token排序保持不变。

### Tests与验证

| T-ID | Owner test |
| --- | --- |
| `T-281` | fresh schema version1、既有单一migration与三个nullable列/CHECK；无upgrade path |
| `T-282` | provider model pricing create/list/replace/None roundtrip |
| `T-283` | step insert exact provider/model/kind/endpoint lookup、mismatch冻结None与refresh不改旧step |
| `T-284` | completion原子写usage+amount，unknown/free区分、duplicate/rollback |
| `T-285` | amount负值/overflow与step/usage identity corruption拒绝 |
| `T-286` | selected summary exact subtotal、priced coverage与finite/AllTime sparse local-day cost |
| `T-287` | provider/model partial coverage、known zero、daily与summary cross-total |
| `T-288` | activity token-only regression与created-at EXPLAIN |

```sh
cargo fmt
cargo test -p jaco-db pricing
cargo test -p jaco-db usage_analytics
cargo clippy -p jaco-db --all-targets --all-features -- -D warnings
git diff --check -- crates/jaco-db
```

完成条件：root `C-83`/`C-84`、DB-owned `R-87`–`R-90`/`R-93`与`T-281`–`T-288`通过；fresh v1只增加catalog price、step snapshot和usage amount三列；step insert冻结exact price，completion原子写usage+amount，analytics准确聚合coverage；无额外source/audit列或兼容代码；本地迁移只由root `WP-006`执行。

### Cost 实施证据（2026-08-21）

- fresh v1 schema、catalog pricing roundtrip、request-start snapshot、completion amount、selected analytics coverage与sparse daily cost已落地；activity保持token-only。
- `cargo test -p jaco-db usage_analytics --no-fail-fast`：15 passed；四package strict clippy、目标package `cargo fmt`与`git diff --check`通过。
- root `WP-006` 已对真实本地数据库完成online backup、三列transactional migration、完整性/外键/row-count核对；仓库未新增migration或手工SQL。
