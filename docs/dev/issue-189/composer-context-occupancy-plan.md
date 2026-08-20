# Issue #189 执行计划：输入框上下文占用

## 文档元数据

- 状态：`In progress`（`WP-102`、`WP-202`、`WP-302`、`WP-402`、`WP-502` 已 `Implemented`；workspace-wide gates、现场provider refresh/新请求、完整人工矩阵与三平台 CI 待做）
- 关联 issue：[#189](https://github.com/suxiaoshao/gpui/issues/189)
- Plan ID：`issue-189`
- 根计划：[Issue #189 总计划](README.md)
- 前置执行文档：[Agent 消息单次请求用量](agent-message-request-usage-plan.md)
- 后续执行文档：[设置页时间范围使用统计](settings-usage-analytics-plan.md)
- 分支：`codex/189-jaco-show-context-usage`
- 最近更新：2026-08-20
- 文档职责：composer 当前选择的 context window 与当前 conversation 最新成功请求占用
- 明确不负责：Agent 消息单次 request usage、设置页范围聚合
- 实施引用：implementation commit/PR `Pending`

## 目标

在 Jaco 的 composer footer 中，在模型选择器左侧持续展示一组紧凑的 `Gauge + 百分比`：

- 已知时显示当前选择模型的最新上下文占用，例如 `Gauge 1%`、`Gauge 37.5%`。
- 未知时显示 `Gauge —`，不得把任何未知状态显示为 `0%`。
- 整组内容使用 `HoverCard` 的默认延迟；详情保留完整 token 数、当前 provider/model、请求完成时间与明确的未知原因。
- reload、live completion、当前 provider/model 切换与 catalog 刷新都通过同一 typed domain fact 和同一纯派生函数得到一致结果。

## 范围

1. 为有效模型能力增加可缺省、带 provenance 的正整数 context-window snapshot，并沿既有 conversation/run settings snapshot 序列化路径保存。
2. 从当前已经返回权威限制的 Rig model listing、Gemini、OpenRouter 与 Ollama discovery response 映射 context window；当官方 listing 不提供该字段时，允许对官方文档明确公布的 exact model ID 建立带 provenance 的 capability profile；无可靠值时保持 unknown。
3. 为 conversation 构造一个与消息 projection 独立的“最新成功请求”fact；其 numerator 是该请求唯一 `usage_event.total_tokens`，不累加 conversation 历史。
4. reload 与成功 run finalization 复用同一个 selector/assembler；running、failed、canceled 不覆盖已显示的上一个成功请求。
5. 在 `ConversationDetailPage` / `ChatInputController` / `ChatForm` 既有 ownership 下派生并渲染 `Gauge + 百分比` 与 pointer-only HoverCard。
6. 增加 typed icon、双语 Fluent 文案、domain/repository/lifecycle/GPUI 回归测试和人工场景。

## 非目标

- 不估算未发送 draft、attachment 或下一次请求的 token 数。
- 不将 conversation 所有请求的 token 求和后当作 context occupancy。
- 不复用 Agent message action-row projection，也不在 composer 展示 input/output/cache/reasoning 明细。
- 不新增自动 compact、截断、发送阻断、context-limit enforcement 或阈值告警。
- 不显示成本、quota、TTFT、TPS、延迟或吞吐。
- 不新增通用、family-wide 或启发式 model context-window 默认表，也不使用 Alma 的 128K fallback；官方文档对 exact model ID 公布的正整数上限不属于默认值。
- 不实现 #194 的 manual model editor、CRUD、override layering 或 refresh preservation；本计划只让已有/构造出的 `Manual` provenance 值走同一 typed contract。
- 不修改 SQLite schema、migration、index、`Cargo.toml` 或 `Cargo.lock`。
- 不新增 `Operation`、后台 `Task`、`Store`、`Global`、hover state 或独立缓存。
- 不实现 settings 时间范围 analytics。

## 用户决定

- `U-11`：composer 常驻摘要采用参考图的 `Gauge icon + 百分比`，不在行内重复 `used / window`。
- `U-12`：未知摘要采用 `Gauge + —`；完整数值和原因进入详情。
- `U-13`：整组 `Gauge + 百分比` 是 HoverCard trigger，使用组件默认打开/关闭延迟。
- `U-14`：交互是 pointer-only；不提供点击固定、Escape、键盘打开或 focus return。
- `U-15`：最新成功请求如果是 partial、unreported 或 missing usage，立即把旧占用清为 unknown，不回退查找更早的可用请求。
- `U-16`：running 请求暂不改变摘要；failed/canceled run 不替换上一个成功请求。
- `U-17`：详情保留完整精确 token 数；百分比最多一位小数并移除末尾 `.0`。

## 适用性检查

| ID | 检查 | 结论 |
| --- | --- | --- |
| `S-11` | 跨 owner / crate | 是：`jaco-core`、`jaco-db`、`jaco-conversation`、`jaco-agent`、`app/jaco` |
| `S-12` | 数据库 schema / migration | 否：复用 `provider_steps`、`agent_runs`、`usage_events` 与现有 JSON snapshots |
| `S-13` | 序列化兼容 | 是：旧 `ModelCapabilitiesSnapshot` JSON 必须以 serde default 读成 unknown |
| `S-14` | provider discovery | 是：四条已有 discovery mapping 增加权威正整数能力映射 |
| `S-15` | GPUI state / lifecycle | 是：沿 Conversation effect 和现有 detail/controller/form owner 同步，不新增状态系统 |
| `S-16` | gpui-component | 是：消费现有 `HoverCard` 与 `DescriptionList` 默认行为 |
| `S-17` | icon / runtime asset | 是：声明 app-local typed Lucide `Gauge`；无需新增 SVG 文件 |
| `S-18` | i18n / accessibility | 是：`en-US`、`zh-CN` Fluent parity 与非可聚焦 image role label |
| `S-19` | 新依赖 | 否：token/percentage 格式化使用本地纯函数 |

## 当前实现证据

### 当前流程

```text
provider discovery
    -> NewProviderModel.capabilities
    -> ProviderModelChoice.capabilities
    -> ConversationSettingsSnapshot / RunSettingsSnapshot.model_capabilities

completed provider request
    -> provider_steps (Completed)
    -> usage_events (unique provider_step_id)
    -> finish_agent_run transaction
    -> FinishedAgentRun
    -> ConversationCommitted.changes
    -> ConversationDetailPage

ConversationDetailPage
    -> owns ConversationModel + ChatInputController
    -> ChatInputController owns typed ChatForm form and watches provider catalog
    -> ChatForm renders footer spacer + model selector + send/stop control
```

### 证据登记

- `E-11`：`ModelCapabilitiesSnapshot` 当前没有 context-window 字段；`ConversationSettingsSnapshot` 与 `RunSettingsSnapshot` 已包含该 capability snapshot，因此无需新增平行 run 字段。
- `E-12`：`CapabilitySourceSnapshot` 已有 `ApiDiscovered`、`OpenRouterNormalized` 与 `Manual` 等 provenance variants。
- `E-13`：Rig `Model.context_length`、Gemini `input_token_limit` 与 OpenRouter `context_length` 已被 provider response 解析，但当前 mapping 丢弃这些值。
- `E-14`：Ollama `/api/show` 当前只消费部分 `details`；上游 response 还能提供 `details.context_length` 或 `model_info` 中的 architecture-prefixed `*.context_length`。
- `E-15`：`usage_events` 对 `provider_step_id` 唯一，保存 normalized `total_tokens` 与 provider/model/conversation identity；它已是 numerator authority。
- `E-16`：conversation timeline reload 已加载 runs、provider steps 和 conversation usage events；可在内存中选择一个 candidate 并调用唯一 assembler，无需额外表或范围聚合 API。
- `E-17`：`finish_agent_run_with_conn` 在事务内确定 run 终态；`FinishedAgentRun` 已承载 message request usage，可并列返回 composer 最新请求 fact。
- `E-18`：`finished_agent_run_changes` 是两条 run-finalization producer 的统一 ordered publication helper。
- `E-19`：`ConversationDetailPage` 同时持有 conversation 与 input controller，是 reload/effect 后同步 singular fact 的现有 owner。
- `E-20`：`ChatForm` footer 已有 `flex_1().min_w_0()` spacer，模型选择器与 primary action 保持 intrinsic width；occupancy trigger 的自然插槽是 spacer 之后、模型选择器之前。
- `E-21`：当前 `request_usage.rs` 已有 exact/compact token formatter；将通用 formatter 移入 `foundation/conversation_format.rs` 可复用且无需引入数字格式化依赖。
- `E-22`：当前 gpui-component `HoverCard` 原生拥有 keyed hover/timer state，默认 open delay 为 600ms、close delay 为 300ms，并支持 trigger/content 间移动。
- `E-23`：Lucide `gauge.svg` 已存在于 workspace runtime assets，可经 app-local `IconName` typed path 使用。
- `E-24`：OpenAI `/v1/models` 的官方 model object 不提供 context-window 字段。OpenAI 官方 Models 文档对 `gpt-5.6`、`gpt-5.6-sol`、`gpt-5.6-terra`、`gpt-5.6-luna` 公布的精确 context window 为 `1_050_000`；provider refresh mapping只在默认或显式 `api.openai.com[/v1]` 官方端点补入该值并持久化。

## 设计决定

- `D-11`：context window 使用 `Option<ContextWindowCapabilitySnapshot>`；已知值只能是 `NonZeroU64`，unknown 用 `None` 表示。
- `D-12`：provenance 与 token 上限组成一个不可拆分 snapshot；禁止另建 source 字符串、布尔位或通用静态 fallback 表。Exact model 的官方文档 profile 只在 discovery 缺失且 provider 使用OpenAI官方端点时填充，不覆盖 API-discovered 或 Manual 值；第三方compatible `base_url` 保持unknown。
- `D-13`：numerator 固定为最新 eligible provider step 对应 usage event 的 provider-reported `total_tokens`；不采用 `input_tokens`、draft estimate 或 conversation sum。
- `D-14`：candidate 是当前 conversation 中父 run 已 `Completed` 的最新 `Completed` provider step；排序固定为 step completion、run completion、step seq、step ID 的降序。
- `D-15`：selector 不按当前 composer model 预过滤，也不向前回找；当前选择只在 app 纯派生阶段与 singular latest fact 做 exact provider/model identity match。
- `D-16`：partial、unreported、missing usage 都保留 candidate identity 并派生 unknown；这样新成功请求能清除旧百分比。
- `D-17`：当前 composer denominator 使用当前 `ProviderModelChoice` 的 effective capability；同一 capability 也自然进入新建 run 的既有 settings snapshot，供历史消费者保持当时值。
- `D-18`：reload 与 live completion 调用同一 DB selector/assembler；Conversation transition 只接受比当前 fact 更新的 deterministic key，并允许同 step idempotent replace。
- `D-19`：app controller 只保存 singular raw request fact；当前选择、能力、unknown reason 和百分比均在 render 前由纯函数派生。
- `D-20`：UI 为 muted、无按钮 chrome 的 `Gauge + percentage`，整个 cluster 是 pointer trigger；无 progress fill、环形进度、阈值色、click 或 tab stop。
- `D-21`：百分比使用 `u128` 整数运算四舍五入到十分位，保留大于 100% 的精确结果；不 clamp、不使用浮点值作为 domain state。
- `D-22`：manual provenance 只通过 domain/persistence fixture 验证正整数能力可被保存和消费；可见 editor 继续由 #194 负责。
- `D-23`：provider catalog不做读取时capability补全；升级前已持久化且缺少新capability的model record与effective choice均保持unknown，直到用户执行provider refresh并保存新值。

## 目标设计

### 文件、模块与 owner 边界

| WP | Owner | Owner plan | 主要职责 |
| --- | --- | --- | --- |
| `WP-102` | `crates/jaco-core` | [owner plan](../../../crates/jaco-core/docs/dev/issue-189/README.md) | context capability、latest request fact、Conversation change/effect/transition |
| `WP-202` | `crates/jaco-db` | [owner plan](../../../crates/jaco-db/docs/dev/issue-189/README.md) | deterministic selector、唯一 assembler、reload/finalization transaction |
| `WP-302` | `crates/jaco-conversation` | [owner plan](../../../crates/jaco-conversation/docs/dev/issue-189/README.md) | singular fact hydration |
| `WP-402` | `crates/jaco-agent` | [owner plan](../../../crates/jaco-agent/docs/dev/issue-189/README.md) | provider discovery mapping、成功 finalization live publication |
| `WP-502` | `app/jaco` | [owner plan](../../../app/jaco/docs/dev/issue-189/README.md) | current-selection projection、footer HoverCard、formatter/icon/Fluent/GPUI tests |

预期 source ownership tree：

```text
crates/jaco-core/src/
├── payloads/capabilities.rs          # [Modify] context capability + provenance contract
├── payloads/resources.rs             # [Modify] ModelCapabilitiesSnapshot optional field
├── capabilities.rs                   # [Modify] conservative constructor defaults unknown
└── domain.rs                         # [Modify] singular request fact/change/effect/transition

crates/jaco-db/src/
├── records/agent.rs                  # [Modify] FinishedAgentRun.context_request_usage
├── records/conversations.rs          # [Modify] reload singular fact
├── repository.rs                     # [Modify] selector/assembler/finalization integration
├── repository/conversations.rs       # [Modify] timeline reload projection
├── tests.rs                          # [Modify] capability fixture fallout
└── tests/agent.rs                    # [Modify] selector, identity, rollback, parity tests

crates/jaco-conversation/src/lib.rs   # [Modify] singular fact hydration

crates/jaco-agent/src/
├── providers.rs                      # [Modify] discovery response字段解析/传递与fixtures
├── providers/capabilities.rs         # [Modify] typed context-window snapshot mappings/tests
├── providers/openai.rs               # [Modify] capability fixture fallout only
├── persistence.rs                    # [Modify] ordered live change helper
└── runtime/{lifecycle.rs,reasoning.rs,tests.rs}
                                        # [Modify] capability fixtures + publication/reload tests

app/jaco/
├── src/components/chat.rs            # [Modify] context_occupancy module
├── src/components/chat/context_occupancy.rs
│                                       # [Add] pure projection, trigger, HoverCard, tests
├── src/components/chat/detail.rs     # [Modify] reload/effect sync into input controller
├── src/components/chat/input.rs      # [Modify] singular raw fact owner/render projection
├── src/components/chat/form.rs       # [Modify] footer builder slot
├── src/components/chat/detail/request_usage.rs
│                                       # [Modify] consume shared formatter
├── src/components/chat/model_picker.rs # [Modify] capability fixture fallout
├── src/features/conversation.rs        # [Modify] capability/conversation fixture fallout
├── src/features/conversation/attachments.rs
│                                       # [Modify] capability fixture fallout
├── src/features/conversation/model.rs  # [Modify] Conversation fixture fallout
├── src/features/home/sidebar.rs        # [Modify] Conversation fixture fallout
├── src/foundation/assets.rs          # [Modify] typed Gauge icon/path test
├── src/foundation/conversation_format.rs
│                                       # [Modify] shared exact/compact token formatter
└── locales/{en-US,zh-CN}/main.ftl    # [Modify] parity keys
```

### `C-11`：能力与运行快照契约

在 `jaco-core` 增加：

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextWindowCapabilitySnapshot {
    pub tokens: std::num::NonZeroU64,
    pub source: CapabilitySourceSnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCapabilitiesSnapshot {
    // existing fields unchanged
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<ContextWindowCapabilitySnapshot>,
}
```

约束：

- missing、zero、negative、non-integer、overflow 或相互冲突的 discovery 值一律映射为 `None`。
- 不保存 derived percentage；tokens 与 provenance 随 capability snapshot 一起 clone/serialize。
- 旧 JSON 缺字段时反序列化成功且为 `None`；重新序列化 unknown 时省略字段。
- `ConversationSettingsSnapshot.model_capabilities` 和 `RunSettingsSnapshot.model_capabilities` 的现有路径自动携带该字段，不新增平行 denominator。
- 一个构造出的 `source: CapabilitySourceSnapshot::Manual` 正整数值必须能完成 capability -> run snapshot serde round trip；本计划不创建该值的 UI。

#### Provider discovery mapping

| 来源 | 权威字段 | 映射 | unknown 条件 |
| --- | --- | --- | --- |
| Rig model listing | `Model.context_length: Option<u32>` | positive -> `ApiDiscovered { provider, endpoint: "rig model listing" }` | `None` / `0` |
| [Gemini Models API](https://ai.google.dev/api/models) | `input_token_limit: Option<u64>` | positive -> `ApiDiscovered { provider, endpoint: "/v1beta/models" }` | `None` / `0` |
| [OpenRouter Models API](https://openrouter.ai/docs/api/api-reference/models/get-models) | `context_length: Option<u32>` | positive -> `OpenRouterNormalized` | `None` / `0` |
| [Ollama `/api/show`](https://docs.ollama.com/api-reference/show-model-details) | `details.context_length` 与 `model_info` 中 exact/suffix `context_length` | 收集所有认可来源的可解析正整数；仅一个 distinct value -> `ApiDiscovered { provider, endpoint: "/api/show" }` | 缺失、0、非整数或多个冲突值 |
| [OpenAI Models 文档](https://developers.openai.com/api/docs/models) | exact `gpt-5.6` / `gpt-5.6-sol` / `gpt-5.6-terra` / `gpt-5.6-luna` | discovery 缺失且默认/显式 `api.openai.com[/v1]` 端点时 `1_050_000` -> `OfficialDocs` | 非 exact ID、不属于 `openai` provider，或第三方compatible `base_url` |

Ollama 不从模型名称、parameter size、architecture 默认值或其他启发式字段推算上限。

### `C-12`：最新 conversation request fact

在 `jaco-core::domain` 增加独立于 message projection 的 raw fact：

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationContextRequestUsage {
    pub agent_run_id: AgentRunId,
    pub provider_step_id: ProviderStepId,
    pub provider_step_seq: i32,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub provider_step_completed_at: time::OffsetDateTime,
    pub agent_run_completed_at: time::OffsetDateTime,
    pub usage: Option<ProviderUsageSnapshot>,
}

pub struct Conversation {
    // existing fields unchanged
    pub latest_context_request_usage: Option<ConversationContextRequestUsage>,
}
```

该 fact 只回答“这个 conversation 最新 eligible request 是谁以及它上报了什么”。它不包含当前 composer choice、context window、percentage、labels 或 unknown reason。

`ConversationChange` / `ConversationEffect` 增加：

```rust
ConversationChange::ConversationContextRequestUsageChanged {
    request_usage: Box<ConversationContextRequestUsage>,
}

ConversationEffect::ConversationContextRequestUsageChanged {
    provider_step_id: ProviderStepId,
}
```

Transition contract：

1. 当前为 `None` 时接受新 fact。
2. 相同 `provider_step_id` 可幂等 replace。
3. 不同 step 按 DB 同一 ordering key `(provider_step_completed_at, agent_run_completed_at, provider_step_seq, provider_step_id)` 比较，只接受更大的 key，忽略迟到的旧 change。
4. 现有 `Transition<ConversationChange>` 固定返回一个 `ConversationEffect`，所以 accepted、same-step duplicate 与 ignored late change 都返回该 context effect；effect 不表示 state 一定变化。
5. app 收到 effect 后只重新同步当前 authoritative singular fact；controller setter 以 `PartialEq` 等值去重，state 未变时不 `notify`、不重载 timeline rows。
6. conversation reopen 直接 hydrate DB singular fact；不从 message projection 重建。

### `DB-11`：selector 与唯一 assembler

`jaco-db` 增加一个 connection-scoped selector/assembler，供 reload 与 finalization 共用。Eligible 条件：

- `agent_runs.conversation_id` 精确匹配目标 conversation。
- 父 run 状态为 `Completed` 且 `run.completed_at` 非空。
- provider step 状态为 `Completed` 且 `step.completed_at` 非空。
- step/run/settings snapshot identity 能确定 provider ID 与 model ID。

排序固定为：

```text
provider_step.completed_at DESC,
agent_run.completed_at DESC,
provider_step.seq DESC,
provider_step.id DESC
```

选择过程不接受 current provider/model 参数。取得一个 step 后，按唯一 `provider_step_id` 查找可选 usage event，并交给单一 assembler 校验：

- run、step、conversation 归属一致。
- step settings snapshot 的 provider/model 与 usage event 冗余 identity 一致。
- usage event 的 provider step ID 与 candidate 一致。
- completed timestamps / statuses 满足 eligible 条件。

Identity mismatch、损坏 JSON 或不可能状态返回 repository invariant error；缺少 usage event 是合法状态，返回 `usage: None`。

### `DB-12`：reload 与 finalization

Reload：

- `ConversationTimelineRecords` 增加 `latest_context_request_usage: Option<_>`。
- 使用 timeline query 已加载的 completed runs/steps 和 conversation usage-event map 调用 `DB-11`；不新增按日期或按模型聚合 query。
- `jaco-conversation::conversation_from_records` 原样 move singular fact。

Run finalization：

- `FinishedAgentRun` 增加 `context_request_usage: Option<_>`；该字段表示本次 finalization 应发布的 singular-fact delta。
- 新鲜 finalization 在将 run 更新为最终状态后、同一 transaction commit 前调用 `DB-11`；现有 terminal early-return 分支也必须调用同一 selector。
- 对 `Completed` run，只有 selector 的全局 latest fact 仍属于本次 `run.id` 时才返回 `Some(fact)`；missing usage 仍返回 `Some(fact { usage: None })`。无 eligible step、已有更新run胜出或重复 finalize 一个旧run时返回 `None`。
- 对 fresh 或 early-return 的 failed/canceled run 都返回 `None`，不发布 composer change；其已完成的中间 step 也不得成为新的 candidate。
- 重复 finalize 当前 latest `Completed` run 必须重建并返回与 reload 相同的 fact；这固定 terminal-idempotent path，不依赖首次调用留下的临时值。
- assembler/query invariant 失败使本次 run-finalization transaction rollback；早先已经提交的 provider-step transaction不回滚。
- `Some` live delta 必须与立即 reopen 得到的 singular fact完全相等；返回 `None` 时不改变当前 Conversation，最终state仍须与reload一致。

### `C-13`：实时 publication

`jaco-agent::finished_agent_run_changes` 在既有 ordered changes 末尾追加：

```text
RunStatusChanged
-> optional EntryAppended
-> optional AgentMessageRequestUsageChanged
-> optional ConversationContextRequestUsageChanged
```

规则：

- value 只能来自 `FinishedAgentRun.context_request_usage`；agent 不 query、不 join、不重算 usage。
- provider-step completion event 不提前发布 composer change。
- failed/canceled/no-step finalization 不发布该 change。
- 沿既有 `ConversationCommitted` FIFO publication；不新增 event enum、channel、Task 或 subscription。
- no-observer/startup recovery 继续依赖 Conversation reload。

### `ST-11`：状态与生命周期

| 输入事件 | Raw fact | 当前选择 / denominator | UI 结果 |
| --- | --- | --- | --- |
| 打开或 reload conversation | DB singular fact | 当前 form + catalog | 纯派生 known/unknown |
| compatible 成功请求完成 | replace 为新 fact | 不变 | reported -> 新百分比；partial/unreported/missing -> `—` |
| 请求 running | 不变 | 不变 | 保留上一个成功结果 |
| run failed/canceled | 不变 | 不变 | 保留上一个成功结果 |
| 切换 provider/model | 不变 | 立即读取新 choice/capability | exact match 可立即 known；mismatch -> `—`，不回找历史 |
| catalog refresh | 不变 | 更新 current effective capability/labels | 立即重新派生 denominator 或 unknown |
| 迟到的旧 live change | transition 忽略；仍返回effect | 不变 | controller等值去重，不回退/notify UI |
| conversation deleted/cleared | controller 清为 `None` | 当前 form 状态按既有流程 | `—` |

### App 纯派生契约

新增 app-local types：

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ComposerContextProjection {
    pub current_choice: Option<ComposerContextChoice>,
    pub latest_request: Option<ConversationContextRequestUsage>,
    pub occupancy: ComposerContextOccupancy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposerContextChoice {
    pub provider_id: ProviderId,
    pub provider_label: SharedString,
    pub model_id: ProviderModelId,
    pub model_label: SharedString,
    pub context_window_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComposerContextOccupancy {
    Known {
        used_tokens: u64,
        percentage_tenths: u128,
    },
    Unknown(ComposerContextUnknownReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerContextUnknownReason {
    NoModelSelected,
    ContextWindowUnknown,
    NoCompletedRequest,
    LatestRequestModelMismatch,
    UsageUnavailable,
    UsageUnreported,
    UsagePartial,
}
```

派生顺序固定，确保一个状态只落入一个可测试原因：

1. 从 typed form 取得当前 provider/model；不存在 -> `NoModelSelected`。
2. 用 form 中的 exact provider/model IDs 构造 current choice；catalog 有记录时取display labels与effective capability，暂时无记录时以IDs作label fallback并令capability unknown；无正整数 -> `ContextWindowUnknown`。
3. raw fact 不存在 -> `NoCompletedRequest`。
4. fact provider/model identity 与当前 choice 不一致 -> `LatestRequestModelMismatch`。
5. `usage == None` -> `UsageUnavailable`。
6. `coverage() == Unreported` -> `UsageUnreported`。
7. `coverage() == Partial` -> `UsagePartial`。
8. `coverage() == Reported` -> 使用 `total_tokens` 与 `NonZeroU64` denominator 计算 known。

切换回一个模型时仍只检查 singular latest fact；如果它恰好匹配，可立即恢复 known。没有“自切换以来”的 ephemeral history，也没有按模型保存第二套 latest cache。

### 百分比与数字格式化

```rust
fn percentage_tenths(used_tokens: u64, capacity: NonZeroU64) -> u128 {
    let numerator = u128::from(used_tokens) * 1_000;
    (numerator + u128::from(capacity.get()) / 2) / u128::from(capacity.get())
}
```

- `percentage_tenths` 表示百分比的十分位：`375` -> `37.5%`，`10` -> `1%`。
- 渲染时整数十分位省略 `.0`；百分号由 Fluent value key 提供。
- 48K / 128K 必须显示 `37.5%`；160K / 128K 必须显示 `125%`。
- used 超过 window 不 clamp；详情继续显示精确 numerator/denominator，便于暴露 provider 或 metadata 问题。
- 将 message request usage 的 exact/compact token helpers 移到 `foundation/conversation_format.rs`；WP-501 现有 action-row compact 文本必须保持不变。
- composer 行内只显示百分比；HoverCard 中的 used/window 使用 shared exact formatter，不添加新库。

### GPUI composer 状态与界面

Ownership：

- `ConversationDetailPage` 在 initial load、reload、`ConversationContextRequestUsageChanged` effect 与 conversation clear/delete 后，把 `Conversation.latest_context_request_usage.clone()` 同步给 `ChatInputController`。
- `ChatInputController` 保存一个 plain `Option<ConversationContextRequestUsage>`；singular-fact setter先做`PartialEq`等值判断，值变化才notify，form/catalog仍沿既有observe触发render；controller不订阅DB，也不拥有overlay/timer state。
- controller render 时调用纯派生函数，向 `ChatForm` 传入始终存在的 `ComposerContextProjection` builder/view；无选择也显示 `Gauge —`。
- `ChatForm` 只负责 footer composition；不得读取 Conversation、repository 或 provider store。

Footer layout：

```text
[attachment / MCP controls] [flex spacer] [Gauge 37.5%] [model selector] [send/stop]
```

Trigger：

- `h_flex`、`items_center`、`gap_1`、`flex_shrink_0`、`whitespace_nowrap`。
- `IconName::Gauge` 使用 xsmall 尺寸；percentage label 使用 `text_xs`。
- icon/text 都使用 `muted_foreground`，与 footer 次级信息层级一致。
- 无 background、border、rounded button、hover fill、selected state、pointer click handler 或 tab stop。
- 整组 cluster 包在一个稳定 ID 的 `HoverCard` trigger 内；spacer、模型选择器和 send/stop 都不属于热区。
- accessibility 为单个非可聚焦 image role / localized label；unknown 与 known 都能被读屏读出状态。

HoverCard：

- 使用 gpui-component 默认 600ms open / 300ms close delay，不在 app 层覆写 timer。
- `Anchor::BottomRight`，使 footer 右侧详情优先向上/向左展开。
- 内容约 280px，采用 `v_flex` + title/summary + `DescriptionList::horizontal().columns(1).bordered(false).small()`。
- known rows：Used context、Context window、Occupancy、Provider、Model、Request completed。
- unknown rows：typed reason；能确定时仍显示 Context window、Provider、Model 与 Request completed，未知 token 值使用 localized em dash。
- 不重复 WP-501 的 input/output/cache read/cache write/reasoning 字段。

### 本地化与可访问性

`en-US/main.ftl` 与 `zh-CN/main.ftl` 同步增加：

```text
conversation-context-occupancy-tooltip
conversation-context-occupancy-title
conversation-context-occupancy-summary-known
conversation-context-occupancy-summary-unknown
conversation-context-occupancy-accessible-known
conversation-context-occupancy-accessible-unknown
conversation-context-occupancy-used-tokens
conversation-context-occupancy-context-window
conversation-context-occupancy-percentage
conversation-context-occupancy-provider
conversation-context-occupancy-model
conversation-context-occupancy-request-completed
conversation-context-occupancy-token-value
conversation-context-occupancy-percentage-value
conversation-context-occupancy-unknown-value
conversation-context-occupancy-reason-no-model
conversation-context-occupancy-reason-window-unknown
conversation-context-occupancy-reason-no-request
conversation-context-occupancy-reason-model-mismatch
conversation-context-occupancy-reason-usage-unavailable
conversation-context-occupancy-reason-usage-unreported
conversation-context-occupancy-reason-usage-partial
```

约束：

- labels、summary、unknown reason、aria text 和 percentage suffix 全部走 Fluent。
- provider/model 的用户配置名称和 ID 作为变量传入，不拼进 key。
- request completion 使用 `provider_step_completed_at` 并复用现有本地时间格式化路径。
- trigger 不进入 Tab order；HoverCard 保持 pointer-only，测试不得加入 keyboard/click 行为。

### 未知值与诊断

- 所有 unknown 原因在行内统一显示 `—`，详情用 typed reason 区分。
- missing usage、all-zero unreported、partial usage 是三个不同原因；不得用 `unwrap_or_default()` 合并。
- context-window unknown 与 request usage unknown 独立；派生顺序使当前 capability 缺失优先显示该原因。
- mismatch 详情显示当前 provider/model；若 latest request identity 可用，再以说明文案指出最新请求来自不同选择，禁止自动回找匹配历史。
- repository identity mismatch 是 error/logging path，不转换成 UI unknown；finalization 应 rollback，reload 应失败并沿现有 error UI 处理。
- provider response 未给权威上限属于正常 unknown，不写 warning flood。

## Requirements

| ID | 验收要求 |
| --- | --- |
| `R-21` | 已知 capability 只接受带 provenance 的正整数，旧 JSON 缺字段可读为 unknown |
| `R-22` | Rig、Gemini、OpenRouter、Ollama 的权威 discovery 值按 `C-11` 映射；OpenAI exact GPT-5.6 IDs 在 listing 缺字段且使用官方端点时使用官方文档值，不覆盖 discovered/manual，compatible `base_url` 保持unknown；其他缺失/冲突不猜测 |
| `R-23` | 新 run settings snapshot 自然保存当时 capability；Manual provenance fixture 与 discovery fixture 走相同 serde path |
| `R-24` | singular request fact 只选择 Completed run 的最新 Completed step，且 deterministic、无 conversation sum |
| `R-25` | latest partial/unreported/missing 保留 identity 并把旧百分比清为 unknown；不 backscan |
| `R-26` | running、failed、canceled 不覆盖上一个成功请求 |
| `R-27` | reload 与 live completion 得到完全相等的 fact、projection 和 UI state |
| `R-28` | model/provider exact match 才可 known；切换和切回在同一纯函数中立即重算 |
| `R-29` | 48K/128K 显示 37.5%，整百分比去掉 `.0`，超过 100% 保留精确值 |
| `R-30` | footer 常驻 `Gauge + percentage/—`，位于模型选择器左侧，整个 cluster 才是 HoverCard 热区 |
| `R-31` | HoverCard 使用默认延迟、pointer-only，不新增 click/keyboard/focus state |
| `R-32` | known 详情显示 exact used/window/percentage/provider/model/request time；unknown 详情显示 typed reason |
| `R-33` | `en-US`/`zh-CN` keys parity，typed Gauge path 与 accessibility label 有测试 |
| `R-34` | WP-501 message request usage 的 compact/exact 格式化和 action row 行为无回归 |
| `R-35` | schema/migration/index/Cargo/Settings/automatic compact 无 diff |

## 工作包

### `WP-102` — jaco-core capability 与 domain fact（Implemented）

1. 实现 `C-11`，更新`conservative_model_capabilities`令新增字段默认为unknown，并补 serde default/round-trip/positive-value tests。
2. 实现 `C-12`，更新全部 `Conversation` fixtures。
3. 实现 deterministic transition/idempotence/late-change tests。
4. 验证 discovery/manual provenance 只是 data，不引入 editor contract。

依赖：无。解锁 `WP-202`、`WP-302`、`WP-402`、`WP-502`。

### `WP-202` — jaco-db selector、assembler 与 transaction（Implemented）

1. 扩展 `ConversationTimelineRecords` 与 `FinishedAgentRun`。
2. 实现 `DB-11` selector/assembler 与 deterministic ordering tests，并更新DB capability fixtures。
3. 接入 timeline reload 的 singular projection。
4. 按 `DB-12` 接入 finalization transaction，覆盖 missing usage、failed/canceled 与 invariant rollback。
5. 断言 migrations/schema/index/Cargo files 无 diff。

依赖：`WP-102`。解锁 `WP-302` 与 `WP-402` 的完整验证。

### `WP-302` — jaco-conversation hydration（Implemented）

1. 在 `conversation_from_records` 原样 move `latest_context_request_usage`。
2. 更新 fixtures。
3. 覆盖 known、missing usage 与 reopen equality；不重选、不 join、不缓存。

依赖：`WP-102`、`WP-202`。

### `WP-402` — provider capability discovery 与 live publication（Implemented）

1. 在 `providers.rs` 扩展Rig/Gemini/OpenRouter/Ollama response字段传递与fixtures，在 `providers/capabilities.rs` 完成 `C-11` 的typed snapshot mapping，并对 listing 不提供该字段、使用OpenAI官方端点的 exact GPT-5.6 IDs 补官方文档 profile；其余agent capability struct literals只补unknown字段。
2. Ollama mapping收集全部认可来源，只接受唯一distinct positive context length。
3. 更新 `finished_agent_run_changes` 依 `C-13` 发布 DB authoritative fact。
4. 覆盖 success、partial/unreported/missing、failed/canceled、no observer 和 reload parity。

依赖：`WP-102`；publication tests 依赖 `WP-202`。

### `WP-502` — Jaco composer projection 与 UI（Implemented）

1. 提取 shared exact/compact token formatter，先保持 WP-501 tests 全绿。
2. 实现 app-local projection/unknown/percentage pure functions 与 table tests。
3. 接入 detail -> input controller singular fact synchronization，并更新受新增capability/domain字段影响的app fixtures。
4. 在 ChatForm footer 插入 `Gauge + percentage/—` 与 native HoverCard。
5. 增加 typed Gauge、Fluent parity、accessibility 和真实 GPUI window tests。
6. 用隔离数据执行 manual scenarios，记录已验证与未验证边界。
7. provider catalog只消费持久化capability；不增加旧缓存兼容层，缺失值保持unknown直至用户执行provider refresh。

依赖：`WP-102`、`WP-302`、`WP-402`。

### 执行顺序

```text
WP-102
  -> WP-202
      -> WP-302 ─┐
  -> WP-402 ─────┼-> WP-502 -> focused validation -> workspace gates/manual
                 ┘
```

`WP-302` 与 `WP-402` 在 `WP-202` contract 稳定后可并行；`WP-502` 只消费稳定 typed contract。

## 测试

| T-ID | Owner | Proposed test / 覆盖 |
| --- | --- | --- |
| `T-21` | core | old capability JSON defaults unknown；known discovered/manual snapshot round trip |
| `T-22` | agent/core/app | Rig/Gemini/OpenRouter positive/zero/missing discovery mapping 与 provenance；exact OpenAI GPT-5.6 official-doc profile、official/compatible endpoint、discovered/manual precedence；旧缓存保持unknown直至refresh，已持久化capability原样进入choice |
| `T-23` | agent | Ollama details/model_info unique、duplicate same、conflicting、non-integer、zero cases |
| `T-24` | db | latest completed step deterministic selection；tool loop 不求和；running/failed/canceled 排除 |
| `T-25` | db | missing event 保留 candidate；identity mismatch / corrupt snapshot rollback 或 reload error；terminal-idempotent Completed/Failed/Canceled branches |
| `T-26` | conversation/agent/app | reload 与 live singular fact equality；late change state ignored但effect可安全重放；same step idempotent replace；controller setter等值不notify |
| `T-27` | app | no model/window/request、mismatch、unavailable、unreported、partial、reported 派生表 |
| `T-28` | app | 1%、37.5%、rounding boundary、125%、large u64 均由整数算法稳定格式化 |
| `T-29` | app GPUI | footer order/intrinsic widths；整个 cluster 是 default-delay HoverCard trigger；无 click/tab/focus behavior |
| `T-30` | app | Gauge typed path、Fluent parity/accessibility、shared formatter 与 WP-501 request usage 回归 |

## 聚焦验证

以下验证已执行并通过（2026-08-20）：

```text
cargo fmt                                                         passed
cargo test -p jaco-core                                           31 passed
cargo test -p jaco-db                                             55 passed
cargo test -p jaco-db composer_context                             7 passed
cargo test -p jaco-conversation                                    4 passed
cargo test -p jaco-agent                                          131 passed
cargo test -p jaco-agent capabilities --lib                        11 passed
cargo test -p jaco-agent documented_openai_context_window --lib     3 passed
cargo test -p jaco-agent composer_context                           2 passed
cargo test -p jaco state::providers::tests:: --no-default-features  4 passed
cargo test -p jaco context_occupancy                                7 passed
cargo test -p jaco request_usage                                     8 passed
cargo test -p jaco i18n                                             11 passed
cargo test -p jaco context_request_usage_setter                       1 passed
cargo check -p jaco                                               passed
cargo clippy -p jaco -p jaco-agent -p jaco-db --all-targets --all-features -- -D warnings
                                                                     passed
```

`jaco-db` 的 55 个全量测试包含 corrupt-candidate 回归；`jaco-agent` 的 131 个全量测试包含 malformed-payload、exact OpenAI official-doc profile、official/compatible endpoint 与 discovered/manual precedence 回归；app provider state 的 4 个聚焦测试锁定旧缓存保持unknown，并验证已持久化capability/provenance原样进入choice。`git diff --check` 由根 agent 在本轮最终对全量 diff 执行。

以下 workspace gates 当前尚未执行：

```sh
cargo build                                                        not run
cargo test                                                         not run
cargo clippy --all-targets --all-features -- -D warnings           not run
```

### 人工场景

已验证：此前UI构建使用隔离配置验证fresh no-model状态显示`Gauge —`、AX label、默认延迟HoverCard、详情内容与footer布局。移除读取时兼容补全后的自动化回归验证旧model cache没有`contextWindow`时保持unknown；最终bundle尚未重建。

尚未验证：37.5% 和超过 100% 的人工视觉场景、现场 provider refresh/新请求、running/failed/canceled 与 partial/unreported/missing 全量人工路径、catalog refresh/reload 完整人工矩阵、英文/中文与深浅主题的完整人工矩阵，以及三平台 CI。

计划中的人工场景：

1. 新 conversation、已知 model window、尚无成功请求：footer 显示 `Gauge —`，详情为 no completed request。
2. 128K model 完成 reported 1,280-token 请求：显示 `1%`；详情显示精确 used/window/provider/model/time。
3. 128K model 完成 48K 请求：显示 `37.5%`；超过 128K 的 fixture 显示大于 100% 且不 clamp。
4. 新请求 running 时保留旧值；成功且 partial/unreported/missing 后变为 `—`；failed/canceled 后仍保留旧值。
5. 切换到不同 model 立即显示 mismatch unknown；切回 latest fact 对应 model 立即恢复；不出现更早匹配请求的值。
6. catalog refresh 从 unknown 变成权威 window 或更新 window 时立即重算；draft/attachment 编辑不改变占用。
7. 快速掠过不打开；持续 hover 按默认延迟打开；trigger 与 content 间移动保持；离开按默认延迟关闭。
8. 重开 app/conversation 后与 live 完成时的 summary/details 完全一致。
9. 英文、中文与深浅主题下检查 cluster 对齐、muted 颜色、长 model/provider 文本、窗口窄宽和 overlay 边界。

## 完成条件

- `R-21`–`R-35` 与 `T-21`–`T-30` 全部有实现和证据。
- capability、request fact、reload/live 与 app projection 各只有一个 authority/derivation path。
- 用户截图对应的 footer 视觉为 `Gauge + 百分比/—`，详情按默认 HoverCard 延迟显示精确信息。
- WP-501 action row 行为与 formatter 输出无回归。
- 聚焦验证、workspace gates 与人工场景结果写回本计划和 owner plans。
- 未执行的 provider/平台/人工边界明确记录；不以 `git diff --check` 代替功能验证。
- Settings analytics 仍保留为独立第三执行文档。

## 完成证据

| 证据 | 当前结果 |
| --- | --- |
| Implementation commits / PR | `Pending` |
| `WP-102` | `Implemented`；core full suite 31 passed |
| `WP-202` | `Implemented`；DB full suite 55 passed，`composer_context` 7 passed |
| `WP-302` | `Implemented`；conversation full suite 4 passed |
| `WP-402` | `Implemented`；agent full suite 131 passed，exact OpenAI official-doc / precedence / official-endpoint 回归已覆盖，`composer_context` 2 passed |
| `WP-502` | `Implemented`；provider persisted-capability state 4、`context_occupancy` 7、`request_usage` 8、`i18n` 11、`context_request_usage_setter` 1、`cargo check -p jaco` passed |
| Focused automated validation | 已通过；命令与计数见上文；selected-package strict clippy 与 `cargo fmt` 通过 |
| Workspace build/test/clippy | `Not run`；仅执行 selected-package combined strict clippy |
| Manual scenarios / provider fixtures | `Partial`；此前UI构建的fresh no-model `Gauge —`、AX label、默认HoverCard/details/layout已验证；最终bundle、现场refresh/新请求与完整人工矩阵未验证 |
| macOS/Linux/Windows CI | `Not run` |

当前仍保留 `In progress`，仅表示根级 release gates 和实现提交/PR 记录尚未完成；五个 composer 工作包本身已 `Implemented`。

## 执行交接审计

- [x] 目标、范围、非目标与用户决定完整。
- [x] 当前 source owners、数据 authority 与 GPUI composition 有代码证据。
- [x] capability provenance、serde compatibility、DB identity 与 lifecycle contract 已定。
- [x] latest request、model switch、unknown clearing、running/failed/canceled 语义无待确认分叉。
- [x] footer 视觉、HoverCard 默认交互、数字格式与 accessibility contract 已定。
- [x] 工作包、依赖、测试映射、聚焦验证、workspace gates 与人工场景可直接执行。
- [x] schema/dependency/#194/Settings 边界明确。
- [x] 当前无阻塞实施的用户问题。
