# Issue #189 执行计划：Agent 消息单次请求用量

## 状态与范围

- 状态：`In progress`
- 关联 issue：[#189](https://github.com/suxiaoshao/gpui/issues/189)
- Plan ID：`issue-189`
- 根计划：[Issue #189 总计划](README.md)
- 分支：`codex/189-jaco-show-context-usage`
- 受影响 owner：`crates/jaco-core`、`crates/jaco-db`、`crates/jaco-conversation`、`crates/jaco-agent`、`app/jaco`
- Release gate：无外部 release gate
- 最近证据刷新：2026-08-20
- 实施引用：2026-08-20 本提交实现；未创建 PR

### 目标

对每个能够关联到最终 assistant message 的 completed provider request，在该 agent turn 的 Copy/完成时间 action row 中提供始终可hover的用量图标。Reported total token摘要与完成时间采用相同字体、字号、颜色和message group-hover显示时机，摘要以`k`/`M`等紧凑形式节省横向空间，但只有图标本身是详情pointer热区；详情使用原生`HoverCard`及其默认打开/关闭延迟，不提供click固定或键盘控制，并保留完整精确整数。详情展示该请求自己的input、output、cache read、可证明的cache hit rate、cache write、reasoning与provider-reported total tokens。实时完成与数据库重载必须产生同一个typed projection。

### 非目标

- Composer context window、context occupancy、百分比或进度条。
- Settings 时间范围聚合、趋势和 provider/model breakdown。
- 将一个run的多个provider steps求和或逐步列在消息HoverCard中。
- TTFT、TPS、streaming throughput、response latency、cost 或 pricing。
- 修改 SQLite schema、migration、`usage_events` 写入基数或现有 usage JSON。
- 根据当前 provider/model 配置重算历史消息。
- 对 missing/unreported usage 做本地估算。

### 用户决定

- 每次请求的用量放在对应 agent 消息后的复制/时间工具栏。
- 输入框另行展示总 context window 与当前占用；消息工具栏不展示 context occupancy。
- 用量HoverCard参考Alma的字段组织。
- 本 issue 排除 TTFT 与 Token/秒。
- 一个 agent run 有多个 provider requests 时，消息只取最终可见 assistant entry 所关联的 provider step，不累加中间 tool-loop steps。

## 高影响变更摘要

| 审计门 | 结果 | 权威 IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | [Add] Jaco 新增 request usage UI 模块；五个既有 owner 增加同一 typed projection 的生产、hydration 与消费 | `D-01`、`C-01`–`C-03`、`WP-101`–`WP-501` |
| Public or cross-owner contracts | [Cross-owner] `jaco-core` 增加消息 request usage、coverage 与 conversation change/effect contract | `D-01`–`D-04`、`C-01`–`C-03` |
| Global/shared authority | [Modify] persisted provider-step usage 保持 authority；`Conversation` 只持有 reload/live 同形 projection | `D-02`、`ST-01`、`C-02` |
| Persistence, data, configuration, or credentials | [Modify] timeline load 与 run-finalization transaction 构造 projection；无 schema/migration/backfill | `D-02`、`DB-01`、`DB-02`、`WP-201` |
| Runtime, concurrency, performance, or shutdown | [Modify] run finalization commit在final entry已知后发布usage change；app不新增业务/runtime/UI Task或channel，HoverCard timing由gpui-component内部拥有 | `D-02`、`C-03`、`ST-01`、`WP-401` |
| Security, privacy, or external access | None；UI 只读 normalized numeric fields，不展示 provider raw metadata、credential 或 payload | `D-05` |
| Dependencies, toolchains, generated, or vendored artifacts | None；复用当前依赖、Lucide tree 与 Fluent 文件 | `D-07` |
| Platform, packaging, CI, or release | None；无 bundle、platform branch 或 workflow 改动 | `S-16` |
| User-visible compatibility, defaults, or removals | [Modify] 每个 eligible agent message 新增始终可 hover 的 icon trigger；仅 reported usage 在 group-hover 时显示 total 摘要；旧记录 missing/unreported 明确显示 unavailable/unreported | `D-05`、`D-06`、`R-01`–`R-12` |

## 适用性

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定或负面理由 | Owner / WP |
| --- | --- | --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Applicable | usage domain、DB、conversation service、agent publication、Jaco timeline 分属五个 owner | 各 owner 只实现自己的 adapter/projection/UI；不新增 crate | `WP-101`–`WP-501` |
| `S-02` | GPUI components, layout, interaction, and accessibility | Applicable | `AgentTurnRow::agent_action_row`已有Copy/时间；详情为多行key-value | 用纯icon trigger + 原生HoverCard + DescriptionList；仅total摘要与时间使用group-hover；详情是pointer-only | `D-05`、`WP-501` |
| `S-03` | Entity, Store, Global, identity, and projections | Applicable | `ConversationModel` authority 是 Ready `Conversation`；timeline rows 是可重建 projection | `Conversation` 持有 keyed usage；页面不建第二个业务 cache | `D-01`、`ST-01`、`WP-301`、`WP-501` |
| `S-04` | Actions, events, subscriptions, focus, and windows | Applicable | detail页面订阅`ConversationModelEvent`；HoverCard为window overlay | 新effect精确重测所属run；HoverCard只处理pointer hover，不新增app-local action/focus状态 | `C-03`、`D-05`、`WP-501` |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Applicable | runtime publication已通过现有FIFO task/channel；usage先于final entry持久化；HoverCard组件内部拥有默认timing Task | 不在ProviderStepChanged时猜关联；仅在run-finalization commit后发同形change；app不新增业务/runtime/UI task | `D-02`、`C-03`、`WP-401` |
| `S-06` | Data acquisition and Operation state | Applicable | `ConversationModel::refresh` 通过既有 refresh Operation 加载 timeline | 扩展同一 load data；不新增 Operation、retry 或 loading flag | `D-07`、`WP-301`、`WP-501` |
| `S-07` | Forms and editable state | N/A | 用量详情只读，没有 editor、validation 或 save | 不引入 Form/native input state | — |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | Rig `Usage` 经 `ProviderUsageSnapshot` 持久化；Conversation changes 跨 agent/app | 固定 `C-01`–`C-03`，UI 不解析 provider raw response | `WP-101`–`WP-501` |
| `S-09` | Error identity, propagation, recovery, and error UI | No change | DB load failure已进入`ConversationProblem`；missing usage是数据覆盖状态 | 沿用既有conversation error/retry；HoverCard unavailable不新增typed error | `D-06` |
| `S-10` | Database, persistence, and migrations | Applicable | `usage_events` 唯一索引 `provider_step_id`；timeline 当前未加载 usage | 增加 conversation query/assembly 与 finalization result，无 schema/migration | `DB-01`、`DB-02`、`WP-201` |
| `S-11` | Generated, synchronized, copied, or vendored content | N/A | 目标均为 handwritten Rust/Fluent/Markdown；Lucide SVG 已在 tree 中 | 不生成或复制新 SVG | — |
| `S-12` | Icons and assets | Applicable | Jaco app-local `IconName` 缺少 usage 图标，Lucide 已有 `chart-no-axes-column.svg` | 增加 typed `ChartNoAxesColumn` variant，不新增 runtime/bundle asset | `D-05`、`WP-501` |
| `S-13` | Fluent i18n and bundle localization | Applicable | conversation runtime text 位于两份 `main.ftl` | 两 locale 增加同 key 的 trigger/title/field/state 文案 | `D-05`、`D-06`、`WP-501` |
| `S-14` | Security, privacy, and credentials | No change | projection 只含 IDs、provider kind、timestamp 与 normalized counts | 不渲染 `metadata`、raw response/request、secret 或 prompt content | `D-05` |
| `S-15` | Observability and diagnostics | No change | 当前 DB/runtime 已记录失败；UI 不记录 token payload | 不增加 tracing、telemetry 或日志字段 | `D-07` |
| `S-16` | Packaging, platform behavior, and CI/release | No change | Rust runtime UI/Fluent/app-local typed icon 不改变 bundle 输入 | 三平台现有 CI 是最终门；无 packaging 工作包 | `R-13` |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | No change | 当前已依赖 jaco crates、gpui-component、time、Fluent | 不改 Cargo manifests、features 或 `Cargo.lock` | `D-07` |
| `S-18` | Owner documentation, indexes, and ADRs | Applicable | #189 原骨架只有两个专题且无 owner plans | 同步三执行文档、root/owner plans 与索引；无需 ADR | `WP-101`–`WP-501` |
| `S-19` | Validation and completion evidence | Applicable | 行为跨 domain/DB/live/GPUI | `R-01`–`R-13` 映射自动化、人工和 CI | 全部 WPs |

## 当前实现证据

### 当前流程

1. `crates/jaco-agent/src/persistence.rs::provider_usage` 把 Rig `Usage` 归一化为 `ProviderUsageSnapshot`。
2. `FreshRepository::complete_provider_step_with_usage` 在一个 immediate transaction 内把 provider step 标为 completed 并插入唯一 usage event。
3. Provider output entry 在持久化时保存当前 `provider_step_id`；run finalization 将最终 entry ID 保存到 `AgentRunOutput.final_entry_id`。
4. Provider step completion目前只发布 `ConversationChange::ProviderStepChanged`。此时 run 的 final entry 可能尚未确定，不能安全创建 message association。
5. `FreshRepository::conversation_timeline_records` 加载 entries、runs、provider steps 与 tool invocations，但不加载 usage。
6. `ConversationService::load` 把 timeline records 组装为 `Conversation`；`ConversationModel` 的 reload/live change 都更新这一份 authority。
7. `timeline::agent_turn_row` 为每个 AgentRun 构造一个 `AgentTurnRow`。其唯一 action row 当前包含 Copy 与 hover 时间。

### 证据登记

| E-ID | 分类 | 结论 | 证据 | 计划后果 |
| --- | --- | --- | --- | --- |
| `E-01` | Current fact | usage event 是 provider-step authority 且每 step 唯一 | `crates/jaco-db/src/migrations.rs:264-306` | 不新增 message usage 表或累计字段 |
| `E-02` | Current fact | step completion 与 usage insert 原子提交 | `crates/jaco-db/src/repository/agent.rs::complete_provider_step_with_usage` | live/reload 都消费 transaction 结果 |
| `E-03` | Current fact | final entry 与 provider step 有持久化 identity chain | `AgentRunOutput.final_entry_id`；`ConversationEntry.provider_step_id` | 只按两个 ID 关联，不按时间/顺序猜测 |
| `E-04` | Current fact | timeline records 与 `Conversation` 尚无 usage projection | `ConversationTimelineRecords`；`jaco_core::Conversation` | 增加同形 typed collection |
| `E-05` | Current fact | run finalization commit 同时返回 run 与 final entry，并发布 ordered changes | `FinishedAgentRun`；`PersistenceContext::finish_run`；`AgentRuntime::finish_agent_run_with_observer` | finalization transaction 是 live association 边界 |
| `E-06` | Current fact | 一个 agent turn 只有一个 action row | `app/jaco/src/components/chat/detail/message.rs::AgentTurnRow` | 只显示 final assistant entry 的 request |
| `E-07` | Upstream fact | Rig 全零 `Usage` 是未报告哨兵；Anthropic input/cache 字段为并列部分，其他已核验 provider 的 input 包含 cache detail | pinned `rig-core 0.41.0` `completion/request.rs` 及 provider adapters | coverage 忽略 metadata；cache hit 使用 provider-specific denominator |
| `E-08` | Current fact | 当前gpui-component `HoverCard`原生提供trigger/content hover、默认600ms open delay、300ms close delay和可取消epoch Task；`DescriptionList`覆盖key-value详情；HoverCard明确是pointer-only | repo-local component docs与当前dependency source | 直接复用HoverCard/DescriptionList，不在app重复timing、overlay或focus system |
| `E-09` | User decision | 消息、composer、Settings 三个语义分开；消息 UI 参考 Alma 且排除 TTFT/TPS | 当前对话与 #189 | 固定 `D-05`、`D-08` |

## 设计决定

| D-ID | 决定 | 依据 | 放弃的方案 | 后果/owner |
| --- | --- | --- | --- | --- |
| `D-01` | 新增 `AgentMessageRequestUsage`，以 final entry ID + provider step ID 为 identity | `E-03`、`E-06` | latest conversation step、run sum、时间邻近关联 | core 定义；DB 唯一生产 |
| `D-02` | DB reload 与 run-finalization transaction 调用同一 assembly helper；ProviderStepChanged 不创建 message usage | `E-02`、`E-05` | UI query、agent 内存拼装、step 完成时提前发布 | DB/live 一致且 final entry 已知 |
| `D-03` | coverage 只有 `Unreported`、`Partial`、`Reported`；metadata 不影响分类 | `E-07` | all-zero 显示 0、重建 total | core 纯函数与跨 UI 一致 |
| `D-04` | cache hit rate 为可选派生值；positive cache 才计算，Anthropic 与 inclusive-input providers 使用不同分母 | `E-07` | 对所有 provider 使用 cached/input、missing 当 0 | 无法证明时显示 unknown |
| `D-05` | 使用始终可hover的app-local纯icon trigger + 原生HoverCard + 单列无边框DescriptionList；图标为`ChartNoAxesColumn`。Reported total token摘要与完成时间跟随message group-hover并复用相同文本样式，摘要使用app-local纯函数生成`k`/`M`等紧凑表示且位于trigger之外；HoverCard详情保留精确整数；HoverCard使用组件默认延迟，内容hover保持展开，不提供click固定或键盘控制 | `E-06`、`E-08`、用户截图与最终确认 | 把摘要并入trigger热区、为单一展示字段新增数字格式化依赖、Tooltip承载详情、受控Popover、app-local timing/hover state、自定义overlay | 保持Alma式纯悬停查看，避免经过摘要或消息空白时误开详情，用短摘要节省工具行空间，并把timing/lifecycle交回组件 |
| `D-06` | projection 存在但 usage event 缺失显示 unavailable；all-zero 显示 unreported；partial 显示可用字段与 unknown total | `E-01`、`D-03` | 隐藏异常、借用其他 step、全部显示 0 | 历史覆盖边界可见 |
| `D-07` | 不改schema、serialized usage、依赖、Operation、业务/runtime/UI Task或telemetry；HoverCard内部状态/Task完全由现有gpui-component拥有 | 当前类型/索引/组件足够 | 新表、JSON presence flags、页面polling、app-local hover Entity/Task | 兼容旧数据并避免重复组件已有的timing lifecycle |
| `D-08` | 消息 projection 不携带或渲染 context-window occupancy；后续 composer 使用独立 plan/type | 用户决定、`E-09` | 复用 request usage 作为 context UI shape | 防止三种统计语义合并 |

## 目标契约

### C-01：Normalized usage coverage 与 cache rate

| Contract | Authority | Producer | Consumers | Compatibility |
| --- | --- | --- | --- | --- |
| `C-01` | `crates/jaco-core/src/payloads/capabilities.rs` | persisted `ProviderUsageSnapshot` + provider kind | DB projection、Jaco HoverCard、后续Settings coverage | Additive Rust API；persisted JSON不变 |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUsageCoverage {
    Unreported,
    Partial,
    Reported,
}

impl ProviderUsageSnapshot {
    pub fn coverage(&self) -> ProviderUsageCoverage;
    pub fn cache_hit_rate(&self, provider_kind: &str) -> Option<f64>;
}
```

Coverage 规则：

1. input/output/cached input/cache write/reasoning/total 全为 0 时返回 `Unreported`；`metadata` 不参与。
2. `total_tokens == 0` 且任一 detail 非 0 时返回 `Partial`。
3. `total_tokens > 0` 时返回 `Reported`。

Cache rate 规则：

```text
if cached_input_tokens == 0:
    unknown
else if provider_kind == "anthropic":
    denominator = checked_sum(input_tokens, cached_input_tokens, cache_write_input_tokens)
else if provider_kind in ["openai", "gemini", "openrouter", "deepseek", "mistral"]:
    denominator = input_tokens
else:
    unknown

if denominator == 0 or sum overflowed:
    unknown
else:
    cached_input_tokens / denominator
```

结果不 clamp；若 adapter 数据产生大于 100% 的值，文字保留实际比例以暴露不一致。Ollama、自定义 provider、zero cache 与无法证明的语义均返回 `None`。

### C-02：Agent message request usage projection

| Contract | Authority | Producer | Consumers | Compatibility |
| --- | --- | --- | --- | --- |
| `C-02` | `crates/jaco-core/src/domain.rs::AgentMessageRequestUsage` | jaco-db reload/finalization assembly | jaco-conversation、Conversation transition、Jaco timeline | Additive in-memory contract；无持久化格式 |

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AgentMessageRequestUsage {
    pub conversation_entry_id: ConversationEntryId,
    pub agent_run_id: AgentRunId,
    pub provider_step_id: ProviderStepId,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub provider_kind: String,
    pub completed_at: time::OffsetDateTime,
    pub usage: Option<ProviderUsageSnapshot>,
}

pub struct Conversation {
    // existing fields unchanged
    pub agent_message_request_usages: Vec<AgentMessageRequestUsage>,
}
```

Eligibility：

- run 的 `output.final_entry_id` 必须解析到同 conversation、同 run 的 completed assistant `Message` entry。
- entry 必须带 `provider_step_id`，step 必须属于同 run、状态为 `Completed` 且有 `completed_at`。
- provider/model identity 与 provider kind 来自该 step 及其 `settings_snapshot`，不读当前 catalog。
- usage event按 step ID读取；缺失时仍创建 projection，但 `usage = None`。
- final error/status、loose historical agent entry、running/failed/canceled step均不创建 projection。
- run 自身后来 canceled/failed 不会抹除一个已经 completed、且仍是 final assistant entry 所属的 step；eligibility 由 final entry + step status 决定。

### C-03：Reload/live publication

```rust
pub enum ConversationChange {
    // existing variants
    AgentMessageRequestUsageChanged {
        request_usage: Box<AgentMessageRequestUsage>,
    },
}

pub enum ConversationEffect {
    // existing variants
    AgentMessageRequestUsageChanged {
        agent_run_id: AgentRunId,
    },
}
```

`Transition<ConversationChange> for &mut Conversation` 按 `conversation_entry_id + provider_step_id` upsert，并保持 entry sequence 对应的稳定顺序；effect只返回所属 run ID。

Live 顺序固定为：

1. run finalization transaction确定 final run与final entry；
2. 同一 transaction调用 `DB-02` 读取 final step/usage并构造 `C-02`；
3. `ConversationCommitted.changes` 依次包含 `RunStatusChanged`、可选 `EntryAppended`、可选 `AgentMessageRequestUsageChanged`；
4. registry/ConversationModel按顺序 transition；
5. detail页面只更新并 remeasure该 run row。

中间 `ProviderStepChanged` 继续更新 step snapshot，但不触发消息用量 UI。

## 数据库契约

### DB-01：Conversation usage events query

```rust
impl FreshRepository {
    pub fn usage_events_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<UsageEventRecord>>;
}
```

- scope：沿 `usage_events.provider_step_id -> provider_steps.agent_run_id -> agent_runs.conversation_id` 的权威归属筛选；保留event自身 `conversation_id` 给 `DB-02` 校验，避免损坏的冗余identity被误判成missing event。
- order：`created_at ASC, id ASC`，仅用于 deterministic assembly；message identity不依赖顺序。
- 使用现有provider-step/run关联与索引；本工作包不新增索引。
- `conversation_timeline_records` 在现有 runs/entries/steps load 后执行一次 query，不逐 message N+1 查询。

### DB-02：Single assembly rule

```rust
fn agent_message_request_usage_from_parts(
    run: &AgentRunRecord,
    final_entry: &ConversationEntryRecord,
    provider_step: &ProviderStepRecord,
    usage: Option<&UsageEventRecord>,
) -> Result<Option<AgentMessageRequestUsage>>;
```

- 返回 `Ok(None)`：final entry不是 completed assistant Message、无 step ID，或step不是 Completed。
- identity/conversation/provider/model不一致：`DbError::Invariant`。
- missing usage event：`Ok(Some(... usage: None))`。
- normal event：校验 event 的 conversation/provider/model/step identity 后 clone `usage`。
- reload 和 finalization 必须调用这一个 helper。

目标 record：

```rust
pub struct ConversationTimelineRecords {
    // existing fields
    pub agent_message_request_usages: Vec<AgentMessageRequestUsage>,
}

pub struct FinishedAgentRun {
    pub run: AgentRunRecord,
    pub final_entry: ConversationEntryRecord,
    pub appended_final_entry: bool,
    pub request_usage: Option<AgentMessageRequestUsage>,
}
```

Finalization 中 projection 构造失败会回滚该次 run-finalization transaction并不发布 change；先前已经提交的 provider step/usage 保持原样。Missing usage event是可表示覆盖状态，不是 transaction error。

## 状态与数据流

### ST-01：Agent message request usage

- **Authority：** `usage_events`、final `ConversationEntry.provider_step_id`、`AgentRunOutput.final_entry_id` 与 completed `ProviderStep`
- **初始化与生命周期：** Conversation reload构造全部 projections；run finalization增量 upsert；随 Conversation Entity 生命周期存在
- **Readers：** `timeline::build_rows`、`AgentTurnRow`、`request_usage.rs`
- **Writer：** 只有 jaco-db `DB-02`；agent只转发 DB commit返回值
- **Publication：** root `C-03`；Conversation transition后由现有 model subscription通知页面
- **Persistence：** projection本身不持久化；可完全由现有 records重建
- **Reset：** reload整批替换；Conversation deletion随 authority清空；页面不保留跨reload业务 cache
- **Cancellation/partial：** step completion前无 projection；missing usage与partial/unreported由 typed state表示

## GPUI 与界面契约

### 组件与 identity

- 新增`app/jaco/src/components/chat/detail/request_usage.rs`，只负责消息request usage formatter、无状态布局、原生HoverCard/DescriptionList与字段映射；composer/settings不得复用该UI type。
- `AgentTurnRow` 增加 `request_usage: Option<AgentMessageRequestUsage>`，通过 `timeline::agent_turn_row` 唯一传入。
- 只有 `request_usage.is_some()` 时渲染 trigger；`usage: None` 仍渲染并显示 unavailable。
- trigger ID：`conversation-request-usage-{provider_step_id}`；HoverCard ID在同一agent row scope使用provider step ID，禁止使用列表index。
- trigger：始终存在且可hover的app-local `ChartNoAxesColumn`纯图标；仅Reported total token摘要由message group-hover控制显隐，且摘要位于HoverCard trigger之外。
- interaction：app不持有open/hover/pinned/focus状态，也不注册click/key handler；只有图标是HoverCard trigger，total摘要与消息空白不参与。
- overlay：原生`HoverCard`，anchor选action row上方/下方不会遮住当前message的方向；使用组件默认open/close delay与trigger/content hover桥接。
- content：标题 + `DescriptionList::horizontal().columns(1).bordered(false).small()`；不新增业务/UI Entity、Store、Global、Form、subscription或Task。

### 字段与格式

- token count使用完整十进制并加千分位，例如 `24,716`；不在详情中缩写成 `24.7K`。
- cache rate保留一位小数百分比，例如 `98.9%`；`None` 不显示命中率行。
- `Reported`：action row 在 group-hover 时显示紧凑的 provider-reported total token 摘要（如`25.1k Token`），其文本样式与完成时间一致；详情仍以千分位完整显示 input、output、total；cache read/cache write/reasoning 仅在值非 0 时增加；cache hit仅在 `C-01` 返回值时增加。
- `Partial`：action row 只保留统计图标，不把未知 total 显示为 0；详情显示可用 details，total value为本地化 em dash/unknown。
- `Unreported`：action row 只保留统计图标；详情显示本地化“提供商未报告用量”，不渲染七个零。
- `usage: None`：action row 只保留统计图标；详情显示本地化“请求用量不可用”。
- UI不读取或显示 `ProviderUsageSnapshot.metadata`。

### Fluent 与图标

新增 typed icon：

| UI role | Variant / Lucide slug | Owner | Fallback |
| --- | --- | --- | --- |
| request usage trigger | `IconName::ChartNoAxesColumn => "chart-no-axes-column"` | `app/jaco/src/foundation/assets.rs` | 无；macro/build test验证已有 Lucide path |

两份 locale增加完全相同的 keys：

| Key | en-US meaning | zh-CN meaning |
| --- | --- | --- |
| `conversation-request-usage-tooltip` | Request usage | 请求用量 |
| `conversation-request-usage-title` | Usage | 用量 |
| `conversation-request-usage-compact-total` | `{ $tokens } Token` | `{ $tokens } Token` |
| `conversation-request-usage-input-tokens` | Input tokens | 输入 Token |
| `conversation-request-usage-output-tokens` | Output tokens | 输出 Token |
| `conversation-request-usage-cache-read` | Cache read | 缓存读取 |
| `conversation-request-usage-cache-hit-rate` | Cache hit rate | 缓存命中率 |
| `conversation-request-usage-cache-write` | Cache write | 缓存写入 |
| `conversation-request-usage-reasoning-tokens` | Reasoning tokens | 推理 Token |
| `conversation-request-usage-total-tokens` | Total tokens | 总 Token |
| `conversation-request-usage-unreported` | Provider did not report token usage | 提供商未报告 Token 用量 |
| `conversation-request-usage-unavailable` | Request usage is unavailable | 请求用量不可用 |
| `conversation-request-usage-unknown-value` | Unknown value glyph/text | 未知值 |

Rust 不拼接可见句子；详情数字由exact formatter生成并作为 DescriptionList value，action-row摘要由app-local compact formatter生成后再通过 Fluent 参数形成 `{ $tokens } Token`。该局部规则不新增数字格式化依赖。

## 需求与验收

| R-ID | 可观察要求 |
| --- | --- |
| `R-01` | 每个 eligible final assistant message最多关联一个 exact provider-step projection。 |
| `R-02` | 同一 run 的中间 tool-loop steps不会进入最终 message usage，也不会被求和。 |
| `R-03` | 两个不同消息/steps的usage与HoverCard identity互不串联。 |
| `R-04` | all-zero、partial、reported、missing event四种覆盖状态按 `C-01`/`D-06` 呈现。 |
| `R-05` | total直接使用 provider-reported值，cache/reasoning不被二次加入。 |
| `R-06` | cache hit仅在provider语义和positive denominator可证明时显示；Anthropic分母规则正确。 |
| `R-07` | final error/status、loose entry、running/failed/canceled step不借用其他 usage。 |
| `R-08` | live run finalization与数据库reload得到相同 `C-02`。 |
| `R-09` | action row的统计图标始终可hover；Reported total摘要和完成时间采用相同字体、字号、颜色与message group-hover reveal，摘要用`k`/`M`等紧凑表示而详情保留完整整数；只有图标本身是HoverCard pointer热区；使用HoverCard组件默认延迟，不提供click固定、Escape、键盘打开或focus return。 |
| `R-10` | details显示完整千分位值、可选字段与一位小数cache rate，不显示context/cumulative/TTFT/TPS。 |
| `R-11` | en-US/zh-CN key parity与typed Lucide path通过测试。 |
| `R-12` | Provider raw metadata、request/response payload和credential不进入UI或clipboard。 |
| `R-13` | focused tests、workspace gates、Jaco人工场景和三平台CI结果被如实记录。 |

## 工作包

### WP-101：Core typed usage contract

- Owner：[jaco-core plan](../../../crates/jaco-core/docs/dev/issue-189/README.md)
- 前置：`C-01`、`C-02`、`C-03`、`D-01`、`D-03`、`D-04`
- 结果：coverage/cache纯函数、projection、Conversation collection/change/effect与transition tests完成。

### WP-201：DB reload/finalization projection

- Owner：[jaco-db plan](../../../crates/jaco-db/docs/dev/issue-189/README.md)
- 前置：`WP-101`、`DB-01`、`DB-02`
- 结果：timeline一次加载usage events；reload与run finalization使用同一helper；missing event可表示，identity mismatch仍是invariant。

### WP-301：Conversation hydration

- Owner：[jaco-conversation plan](../../../crates/jaco-conversation/docs/dev/issue-189/README.md)
- 前置：`WP-101`、`WP-201`
- 结果：service load把DB projection原样放入 `Conversation`，无第二次association。

### WP-401：Agent live publication

- Owner：[jaco-agent plan](../../../crates/jaco-agent/docs/dev/issue-189/README.md)
- 前置：`WP-101`、`WP-201`、`C-03`
- 结果：所有run-finalization路径在ordered commit changes中发布可选request usage；step completion不提前绑定。

### WP-501：Jaco action row与HoverCard

- Owner：[Jaco plan](../../../app/jaco/docs/dev/issue-189/README.md)
- 前置：`WP-101`–`WP-401`、`D-05`、`D-06`
- 结果：timeline/effect精确更新、始终可hover的纯icon trigger、trigger外group-hover total摘要、原生HoverCard/DescriptionList、format/i18n/tests与人工pointer场景完成。

## 测试与验证映射

| R-ID | T-ID | 场景 | 预期 |
| --- | --- | --- | --- |
| `R-04`–`R-06` | `T-01` | core coverage/cache table tests | all-zero、partial、reported、Anthropic、inclusive provider、unknown/overflow正确 |
| `R-01`–`R-08` | `T-02` | DB normal/tool-loop/missing/mismatch/reload tests | exact association、no sum、same projection、transaction rollback |
| `R-08` | `T-03` | jaco-conversation service reopen | hydrated projection等于DB result |
| `R-02`、`R-07`、`R-08` | `T-04` | agent observer normal/tool-loop/fail/cancel tests | change只在final association后发布且顺序固定 |
| `R-03`、`R-09`、`R-10` | `T-05` | Jaco timeline/formatter/ElementId tests | 两行不串状态、字段/格式正确、无excluded metrics |
| `R-11` | `T-06` | icon path与locale parity | typed path存在、两locale keys一致 |
| `R-09`、`R-10` | `T-07` | packaged/local app人工pointer | 图标始终可hover；message group-hover以和时间一致的文本样式显示紧凑reported total但摘要不触发详情；仅图标按HoverCard默认延迟展开，详情显示完整整数，内容可移入且离开延迟关闭 |
| `R-13` | `T-08` | workspace build/test/clippy + CI | repository gates通过或准确记录未通过项 |

### 聚焦验证命令

```sh
cargo fmt
cargo test -p jaco-core usage
cargo test -p jaco-db agent_message_request_usage
cargo test -p jaco-conversation
cargo test -p jaco-agent request_usage
cargo test -p jaco request_usage
cargo check -p jaco
git diff --check
```

交付前聚合门：

```sh
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

人工场景使用隔离测试数据：

1. 发送一个有reported usage的普通请求，确认统计图标始终可hover，message group-hover时图标右侧以和时间一致的文本样式显示紧凑total token（例如`25.1k Token`）且摘要不触发详情；只有图标按HoverCard默认延迟展开完整精确字段。
2. 运行含tool loop的请求，确认最终消息只显示final step用量。
3. 用fixture打开 partial、all-zero、missing usage历史对话，确认三种状态。
4. 用OpenAI-style与Anthropic-style cache fixture确认命中率分母；Ollama/unknown不显示rate。
5. 用鼠标验证快速掠过不打开、持续hover图标后打开、可移入内容、离开后按组件默认延迟关闭；click和键盘不属于本交互。
6. 重启/重新打开同一对话，确认值与live完成时一致。

## 完成条件

- `WP-101`–`WP-501` 全部完成，`R-01`–`R-13` 有对应证据。
- 实际变更文件、commit、PR、命令结果、人工场景和CI记录回写 root/owner plans。
- Agent message UI没有context-window、occupancy、累计usage、TTFT或TPS。
- 本执行文档未实现或替代 Composer 与 Settings；两份独立计划均已实施并保留各自验证证据。

## 完成证据

| 证据 | 当前结果 |
| --- | --- |
| Implementation commits / PR | 本提交包含本计划的完整实现；未创建 PR |
| 实际文件与差异 | 已完成`jaco-core` usage/domain contract、`jaco-db` query/assembly/finalization、`jaco-conversation` hydration、`jaco-agent` ordered live publication、Jaco timeline/action-row/HoverCard/icon/Fluent；未修改composer、Settings、schema、migration、Cargo manifest/lock |
| 自动化命令 | `cargo fmt`、`cargo build`、`cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`、`git diff --check` 通过；focused：`jaco-core` 26 tests、`jaco-db` 48 tests、`jaco-conversation` 2 tests、`jaco-agent` 123 tests，以及 Jaco request usage/timeline/effect/icon tests 通过。首次沙盒内 workspace test 的 14 个 `http-client` bind cases 因 `Operation not permitted` 失败，提权后原命令完整通过；最终审阅后的cross-conversation reload修复另以jaco-db focused/full tests与crate strict clippy验证 |
| 人工场景 | 最终HoverCard交互已由用户检查确认符合预期；未发送真实provider请求 |
| 三平台 CI | 未执行；尚未创建 PR |
| 接受的偏差 | `None` |
| 未验证边界 | 真实provider数据呈现与macOS/Linux/Windows CI |

## 执行交接审计

- [x] 产品语义、association、coverage、cache denominator与unknown规则已确定。
- [x] reload/live authority、transaction与change顺序已确定。
- [x] 每个 owner、精确类型/方法、UI组件、图标、Fluent与work package已确定。
- [x] 无 unresolved user/product/architecture choice。
- [x] 无schema、dependency、Operation、app-local业务/runtime/UI Task、pricing、performance或context scope creep；hover timing由gpui-component HoverCard内部拥有。
- [x] 每个需求已有自动化或人工验证映射。
- [x] `WP-101`–`WP-501` 代码实现与本地自动化验证已完成并回写证据。
- [x] `T-07` 最终HoverCard交互已由用户检查确认；真实provider请求留作未验证边界。
- [ ] `T-08` 三平台 CI 待创建PR后执行。
