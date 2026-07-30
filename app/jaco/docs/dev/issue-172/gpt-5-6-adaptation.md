# Rig 0.41 / GPT-5.6 实施计划

## 状态与边界

**已实施；本文保留实施级契约，真实 API smoke 仍需显式凭据。**

Rig gate 已由 `rig`/`rig-core`/`rig-agent 0.41.0` 正式发布满足。release、migration、
feature 和 API 证据见 [dependency-evidence.md](dependency-evidence.md)。实施顺序固定为：

1. 先完成 [dependency-refresh.md](dependency-refresh.md) 的全部依赖工作包，尤其是
   Rig 0.41/RMCP 2.2 的 HTTP/SSE 等价迁移。
2. 再修正 provider-step 生命周期并重设 fresh schema。
3. 最后接入 GPT-5.6 typed reasoning、Responses WebSocket 和 continuation recovery。

本计划不接入 Programmatic Tool Calling、多 agent beta、compaction 或新的产品设置。它们
即使是 GPT-5.6 能力，也不是 issue #172 的授权范围。

### 2026-07-29 实施记录

- 官方 OpenAI endpoint + GPT-5.6 family + stateful continuation capability 才选择
  `OpenAiWebSocketCompletionModel`；其他 provider、自定义 base URL 和其他 OpenAI 模型
  继续使用原 Rig HTTP/SSE 路径。
- `OpenAiReasoningPolicy` 使用 Rig typed `ReasoningEffort/Mode/Context`：
  fresh 不传 context、continuation 传 `all_turns`、fallback 传 `current_turn`，
  `store=true`；普通运行不传 `pro`。
- `OpenAiAttemptCoordinator` 在每次实际 WebSocket attempt 前建立独立 provider step；
  rejection 先失败当前 step 并失效 source continuation，再建立一条
  `FullHistoryFallback` step，最多回放一次。请求注入保留 hosted tools 等未知参数。
- WebSocket terminal success 先原子提交 response/usage/continuation，再推进内存
  response ID 和 documents-sent 状态；本地提交失败不重放 provider request，并使运行
  失败、关闭该会话连接。
- `ConversationRuntimeStore` 拥有连接池；stop 会保留原 task，等待其结束后关闭连接并做
  DB terminal repair。自然失败、删除成功和 app shutdown 也会关闭对应连接。
- fresh schema 已加入五态 CHECK、continuation CHECK/unique、usage one-to-one；
  `complete_provider_step_with_usage` 使用 immediate transaction。开发数据库按 fresh
  schema 重建，不提供旧库迁移。
- queued/running 对话删除由数据库返回 `ConversationHasActiveRun`；UI 显示 warning，
  不切 route、不取消 run，用户停止后再删除。
- 自动测试覆盖 typed policy、request shaping、结构化 rejection、fallback 两 attempt
  审计、TTL/invalidation、schema lifecycle、usage rollback、active-run delete 和现有
  runtime/tool regressions。真实 OpenAI WebSocket smoke 未在无凭据环境运行。

## 已确认产品决策

- `reasoning.context` 由运行时自动管理，不提供用户设置。
- `reasoning.mode = "pro"` 不进入产品/UI；adapter 底层使用 Rig typed enum，并保留
  `pub(crate)` runtime override，普通 app 运行路径不传 override。
- 正常路径优先使用 Rig 0.41 的 Responses WebSocket；每个后续请求只发送新 input 和
  `previous_response_id`。
- 只有 provider 明确拒绝 `previous_response_id` 时，才清空 continuation 并把同一逻辑
  请求以 full history 重放一次。未知 transport error 不自动重放，避免一个请求可能已被
  provider 接收后重复执行。
- `store=true` 显式发送。provider response 默认 30 天 TTL；本地 continuation 也按
  30 天判定已知过期，但 provider 仍是最终真相。
- Jaco 尚未对外发布：直接修改 initial migration/fresh schema，开发数据库从头重建；
  不写兼容 migration、旧 JSON backfill 或双读逻辑。

## 当前实现诊断

### 当前调用链

```text
ConversationRuntimeStore::start_run
  -> AgentRuntime::start/run_started_with_model_observed
  -> providers::run_saved_provider_model
  -> Rig Client::completion_model
  -> PersistingCompletionModel
  -> AgentBuilder + PersistingPromptHook
  -> stream_prompt/history/without_memory
  -> StreamingOutputAccumulator + AgentPersistence
```

现有所有权应保留：

- app `ConversationRuntimeStore`：每个 conversation 最多一个 active run、取消、审批 broker、
  runtime event publication；
- `AgentRuntime`：一次 run 的 Rig orchestration；
- `PersistenceContext`：run/step/entry/tool/usage 持久化和 observer publication；
- `jaco-db`：conversation timeline 的唯一持久化 owner；
- Rig：model/tool turn 状态机和 provider request/response 类型。

### 必须一并修复的既有缺口

1. `PersistingCompletionModel::stream` 只插入 provider step，不在单个 model turn 结束时
   完成；`runtime.rs` 又只在整个 multi-turn stream 结束后完成最后一个 step。tool loop
   的中间 step 因而可能永久保留 `running`。
2. blocking `finish_provider_step` 把通用 `message_id` 当成 continuation，写成
   `{"messageId": ...}`；streaming 则根本不保存 response ID。
3. `ProviderRunStateSnapshot.continuation` 没有读取路径；`state_snapshot_json` 不能支持
   typed TTL、失效和索引查询。
4. 使用 `previous_response_id` 的请求仍会由 Rig 构造完整历史。若 adapter 原样发送，
   provider 会同时看到 server-side chain 和重复的历史 input。
5. `runtime.rs` 对 streaming enum 是穷尽 match，Rig 0.40 新增的
   `StreamedAssistantContent::Unknown(Value)` 没有保存策略。
6. `reasoning_additional_params` 对 OpenAI 仍拼 raw JSON，无法验证 GPT-5.6 的
   `max`、`mode`、`context` 和 response effective context。

### Rig 0.41 API 核对结论

以下契约已直接对照 crates.io `rig-core 0.41.0`/`rig-agent 0.41.0` 发布源码，不留给
实施者猜测：

- `CompletionModel` 必须实现 `Clone`、关联类型 `Response`/`StreamingResponse`/`Client`
  以及 `make`、`completion`、`stream`；`AgentBuilder::new(model)` 持有传入的 model，
  build/run 不会重新调用 `make`。
- OpenAI `Client` 默认就是 Responses API client；native WebSocket 只对默认
  `reqwest::Client` transport 开放，入口是
  `responses_websocket_builder(model).connect()`，会话公开
  `send`/`send_with_options`、`next_event`、`completion`、
  `previous_response_id`、`clear_previous_response_id` 和 `close`。
- `ResponsesWebSocketSession` 自己在 successful terminal event 后缓存最新
  `response.id`，显式 request `AdditionalParameters.previous_response_id` 优先于缓存；
  failed/incomplete/error、close 和 failed connection 会清空缓存。
- 一个 session 同时只允许一个 in-flight response；Rig 默认 connect timeout 为 30 秒，
  event timeout 默认关闭，Jaco 必须显式配置后者。
- public event 是 `ResponsesWebSocketEvent::{Response,Item,Error,Done}`；公开
  `ResponseChunk`、`ItemChunk`、`Output` 和 `ResponsesUsage` 足以实现 adapter，但
  `RawChoiceAccumulator` 是 private，不能依赖或复制它。
- `CompletionError::provider_response_json()` 返回
  `Result<Option<serde_json::Value>, serde_json::Error>`；WebSocket provider error 会由
  Rig 保存为无 HTTP status 的 structured provider body，因此 continuation rejection
  必须匹配 `Ok(Some(body))`，而不是 display string。

## 文件与模块结构

禁止新增 `mod.rs`。目标结构：

```text
crates/jaco-agent/src/
  providers.rs                         # provider dispatch、OpenAI transport selection
  providers/
    capabilities.rs                    # GPT-5.6 family capability
    openai.rs                          # typed reasoning、transport policy、run model
    openai/
      websocket.rs                     # pool、CompletionModel、request delta、event decoder
      websocket/tests.rs               # fake connector/session、decoder/pool/request tests
  persistence.rs                       # PersistenceContext fields/methods
  persistence/
    model.rs                           # generic HTTP/SSE PersistingCompletionModel
    provider_step.rs                   # per-turn finish、continuation、unknown output
    port.rs                            # AgentPersistence 新查询/失效方法
  runtime.rs                           # model factory、stream step owner、Unknown handling
  runtime/
    reasoning.rs                       # 非 OpenAI provider mapping；OpenAI 委托 typed policy
  tools.rs                             # Rig 0.41 DynamicTool/ToolContext bundle
  mcp.rs
  mcp/connector.rs                     # RMCP 2.2 public Tool + ServerSink registration

crates/jaco-core/src/
  domain.rs                            # ProviderStep.continuation
  payloads/agent.rs                    # continuation/request context/transport snapshots

crates/jaco-db/src/
  error.rs                             # active-run delete typed error
  migrations.rs                        # 直接修改 initial provider_steps schema
  schema.rs
  models/agent.rs
  records/agent.rs
  repository/agent.rs
  repository/conversations.rs          # soft-delete transaction 与 active-run gate
  repository.rs
  validation.rs
  tests/{agent,projects,schema}.rs

app/jaco/src/features/conversation.rs  # soft delete 成功后关闭 conversation sessions
app/jaco/src/features/conversation/runtime.rs
                                       # pool owner、idle session close、shutdown
app/jaco/src/database/session.rs        # SessionAgentPersistence 新增查询/事务方法
app/jaco/src/features/home/workspace.rs # 删除成功后再切换 route
app/jaco/src/features/home/sidebar/menu.rs
                                       # active-run typed error -> warning notification
app/jaco/locales/{en-US,zh-CN}/main.ftl # 先停止再删除提示
```

`providers/openai.rs` 通过 `mod websocket;` 引用子模块；不创建
`providers/openai/mod.rs`。

## 核心类型与 API 契约

### `jaco-core` 持久化类型

在 `payloads/agent.rs` 新增：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderContinuationKind {
    OpenAiResponses,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderContinuationSnapshot {
    pub kind: ProviderContinuationKind,
    pub response_id: String,
    pub reasoning_context: String,
    pub expires_at: OffsetDateTime,
    pub invalidated_at: Option<OffsetDateTime>,
    pub invalidation_error: Option<RunErrorPayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderContinuationError {
    EmptyResponseId,
    EmptyReasoningContext,
    ExpirationOverflow,
    AlreadyInvalidated,
}

impl ProviderContinuationSnapshot {
    pub fn openai_responses(
        response_id: String,
        reasoning_context: String,
        completed_at: OffsetDateTime,
    ) -> Result<Self, ProviderContinuationError>;
    pub fn is_available(&self, now: OffsetDateTime) -> bool;
    pub fn invalidate(
        &mut self,
        at: OffsetDateTime,
        error: RunErrorPayload,
    ) -> Result<(), ProviderContinuationError>;
}
```

`reasoning_context` 使用 raw `String`，因为它是 provider 返回的 effective value，不是
产品设置；trim 后拒绝空值，但未知 future value 仍必须能持久化。`is_available` 只在
response ID 非空、
未 invalidated 且 `now < expires_at` 时返回 true。constructor trim 后拒绝空 response ID，
并用 checked arithmetic 将 `expires_at` 固定为 `completed_at + 30 days`；`invalidate`
拒绝重复失效。`ProviderContinuationError` 手写实现 `Display + Error`，避免只为四个
domain invariant 给 `jaco-core` 新增 `thiserror`；DB mapping/persistence 将它转为现有
错误类型，不在 UI 新增错误。

请求 audit 新增：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTransportSnapshot {
    ProviderDefault,
    Http,
    ServerSentEvents,
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRequestContextSnapshot {
    FullHistory,
    PreviousResponse,
    FullHistoryFallback,
}
```

`ProviderStepRequestSnapshot` 增加 `transport`、`context_mode`、
`previous_response_id: Option<String>`。`ProviderStepResponseSnapshot` 增加
`provider_outputs: Vec<ProviderRawPayload>`，用于保存 Rig `Unknown(Value)`/hosted output。
`ProviderRunStateSnapshot` 删除 `continuation`，只保留 provider run/output-item audit。
`ProviderStep`/`ProviderStepRecord` 增加
`continuation: Option<ProviderContinuationSnapshot>`。`NewProviderStep` 只能创建
queued/running row，continuation 固定为空；completed 的 continuation 只由下文
`CompleteProviderStep` 输入。`UpdateProviderStepStatus` 不增加 continuation，并收窄为
failed/canceled 更新。

### OpenAI reasoning policy

在 `providers/openai.rs` 新增：

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenAiReasoningPolicy {
    effort: Option<rig::providers::openai::responses_api::ReasoningEffort>,
    mode: Option<rig::providers::openai::responses_api::ReasoningMode>,
    context: Option<rig::providers::openai::responses_api::ReasoningContext>,
    store: bool,
}

impl OpenAiReasoningPolicy {
    pub(crate) fn from_run_settings(settings: &RunSettingsSnapshot) -> Result<Self>;
    pub(crate) fn for_request_context(
        &self,
        context: ProviderRequestContextSnapshot,
    ) -> Self;
    pub(crate) fn to_rig_reasoning(
        &self,
    ) -> rig::providers::openai::responses_api::Reasoning;
    pub(crate) fn apply_to_additional_parameters(
        &self,
        parameters: &mut rig::providers::openai::responses_api::AdditionalParameters,
    );
    pub(crate) fn merge_into_request_params(
        &self,
        parameters: Option<serde_json::Value>,
    ) -> Result<serde_json::Value>;
    pub(crate) fn with_mode(self, mode: ReasoningMode) -> Self;
}
```

方法语义：

- `from_run_settings` 只接受 capability 已声明的 OpenAI level，并映射到 Rig
  `ReasoningEffort::{None,Low,Medium,High,Xhigh,Max}`；不接受 GPT-5.6 不支持的
  `Minimal`。
- fresh `FullHistory`：`context=None`，让 GPT-5.6 使用模型默认并在 response 中记录
  effective context。
- `PreviousResponse`：显式 `context=AllTurns`，因为此时 continuation 能提供 prior
  reasoning。
- `FullHistoryFallback`：显式 `context=CurrentTurn`，因为 rejected ID 后不能假设 prior
  reasoning 仍可用。
- `from_run_settings` 产出的 `mode` 始终 `None`；`with_mode` 是无 UI/config/schema 来源的
  `pub(crate)` runtime override，当前普通 app call site 不调用，但 internal runtime/测试
  能构造 `Pro` 并走完整 request mapping。
- `store` 始终为 `true`。不要依赖 OpenAI 默认值，以免 continuation 生命周期被配置漂移
  改变。
- `to_rig_reasoning` 只用 Rig `Reasoning::new().with_effort/with_mode/with_context` 构建；
  `apply_to_additional_parameters` 写 typed `reasoning` 和 `store=Some(true)`，供 WebSocket
  model 使用。
- 现有 AgentBuilder 的 additional params 还可能包含 provider tools；HTTP/SSE 路径用
  `merge_into_request_params` 保留其他 key，只把 **typed `Reasoning` 序列化后的值**写入
  `reasoning` 并写 `store`。不得先把整个 object deserialize 成 `AdditionalParameters`，
  否则 Rig struct 未建模的 `tools` 等 key 会丢失。

OpenAI 分支不再由 `runtime/reasoning.rs` 手写
`json!({"reasoning":{"effort":...}})`；其他 provider 的现有 mapping 保持不变。

#### `safety_identifier` 明确不处理

**用户决定**：当前 Jaco 是本地 BYOK 桌面应用，每个用户直接配置自己的 OpenAI API key，
本轮不接入 `safety_identifier`。`OpenAiReasoningPolicy`、provider settings、
`AdditionalParameters` merge、WebSocket `response.create`、数据库和 request snapshot 都不
新增该字段；也不生成、持久化或发送 installation ID、用户名、邮箱 hash 或其他稳定用户标识。
这不是待实施时再决定的可选项。

只有未来引入 Jaco 托管代理、共享 OpenAI API key 或多用户云账户时，才重新设计其身份来源、
隐私派生、轮换和跨设备语义；该未来工作不属于 issue #172。当前也不回填已废弃的 OpenAI
`user` 字段，不把 `prompt_cache_key` 当作安全标识。

#### Prompt caching 与图片/PDF detail 的明确取舍

- 本轮不设置 `prompt_cache_key`、已被新 contract 取代的 `prompt_cache_retention`，也不
  通过 raw JSON 注入 `prompt_cache_options`。Responses 的 implicit caching 继续由 OpenAI
  管理；Jaco 只保存 provider 返回的 `cached_input_tokens` 和
  `cache_write_input_tokens`。conversation ID 不挪作 cache key，避免把新的 explicit
  breakpoint/TTL/routing policy 与 continuation 迁移绑在一起。
- `crates/jaco-agent/src/runtime/history.rs` 现有图片统一构造
  `ImageDetail::Auto`，本轮保持；不增加 detail picker、provider setting、DB 字段或 i18n。
- PDF 继续作为 Rig `UserContent::Document` 的 FileId/URL/Base64 document 发送；当前
  Rig 0.41 document type 没有独立 PDF detail 字段，因此不把 PDF 转成逐页图片，也不通过
  raw JSON 发明非 typed 参数。
- WebSocket request parity test 必须覆盖一张 `ImageDetail::Auto` 图片和一份 PDF，证明
  transport 切换没有丢 attachment media type/detail/data。将来若产品要让用户控制 detail
  或 retention，单独设计设置、成本提示与 snapshot，不在 #172 隐式加入。

#### `reasoning.context` 的确切语义与本产品策略

`reasoning.context` 不是“把推理过程文本发回去”的开关，而是告诉 Responses API：跨 response
延续时，先前保存的 reasoning items 有多少仍应对本次模型调用可用。它与用户消息历史、
`previous_response_id`、`store` 三者有关，但不能互相替代：

- omitted/`auto`：交给模型选择 effective mode；response 的 `reasoning.context` 才是本次
  实际值，因此 fresh request 必须记录返回值，不能把 request omission 当成 `current_turn`；
- `all_turns`：适合目标、假设、约束在多轮中保持稳定的 agent workflow；必须配合
  `previous_response_id` 才能让服务端保存的 earlier reasoning 可用；
- `current_turn`：明确不复用 earlier reasoning；适合 ID 失效后的 full-history replay，
  避免把“消息历史还在”误判成“provider reasoning chain 也还在”。

本 issue 固定由 runtime 根据 `ProviderRequestContextSnapshot` 选择，不增加用户配置：
fresh=`auto/omitted`，正常 continuation=`all_turns`，full-history fallback=`current_turn`。
同一条仍存活的 WebSocket 在 `store=false`/ZDR 下也能从 connection-local latest response
继续；但跨连接、app restart 和 DB 恢复没有 persisted fallback。如果未来要支持这两种
数据策略，必须另行拆分 live-socket 与 cross-connection 行为，并设计 output-item/
encrypted-reasoning replay；本 issue 不能只把 `store` 改为 false 后继续沿用 30-day
continuation。

`all_turns` 不会把隐藏推理正文暴露给 Jaco，但也不意味着历史免费：OpenAI 仍会对
continuation chain 中参与本次请求的历史 input 计费。Jaco 应以 response usage 中的
input/cached/cache-write/reasoning token 字段记账，不自行从本地发送字节数估算成本。

### Transport selection

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAiTransportPolicy {
    HttpSse,
    ResponsesWebSocket,
}

impl OpenAiTransportPolicy {
    pub(crate) fn select(
        provider: &ProviderRecord,
        model_id: &str,
        capabilities: &ModelCapabilitiesSnapshot,
    ) -> Self;
}
```

只有同时满足以下条件才返回 `ResponsesWebSocket`：

- `provider.kind == "openai"`，不是 `custom_openai_compatible`/Azure；
- base URL 未配置，或 normalize 后精确为 `https://api.openai.com/v1`；
- normalized model ID 以 `gpt-5.6` 开头（覆盖 alias/Sol/Terra/Luna/snapshot suffix）；
- model capability 支持 streaming、reasoning、`stateful_response_continuation`，且
  `extension` 精确匹配
  `ProviderCapabilityExtensionSnapshot::OpenAi { responses_api: true, .. }`。

自定义 base URL 即使声称 OpenAI compatible 也继续走 HTTP/SSE；计划没有证据证明其
WebSocket endpoint、event 或 continuation 兼容。

### Runtime model binding

`AgentRuntime::run_started_with_model_observed` 当前在创建 `PersistenceContext` 之前就接收
完成的 model。重构为：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamingStepOwner {
    Runtime,
    Model,
}

struct RuntimeModelBinding<M> {
    model: M,
    streaming_step_owner: StreamingStepOwner,
    cancellation_cleanup: RunCancellationCleanup,
}

#[derive(Clone)]
enum RunCancellationCleanup {
    None,
    CloseOpenAiConversation {
        pool: OpenAiResponsesSessionPool,
        conversation_id: ConversationId,
    },
}

impl RunCancellationCleanup {
    async fn before_terminal_status(&self) -> Result<()>;
}

async fn run_started_with_model_factory<M, F, Fut>(
    &self,
    agent_run: AgentRunRecord,
    request: AgentRunRequest,
    factory: F,
    observer: Option<AgentRuntimeObserver>,
) -> Result<AgentRunHandle>
where
    M: CompletionModel + 'static,
    F: FnOnce(PersistenceContext) -> Fut,
    Fut: Future<Output = Result<RuntimeModelBinding<M>>>;
```

现有 public/generic `run_started_with_model_observed` 委托 factory，返回
`PersistingCompletionModel` + `StreamingStepOwner::Runtime` +
`RunCancellationCleanup::None`。OpenAI WebSocket 分支在 async factory 中先通过
`PersistenceContext` 查询并验证 prior provider step，再创建 context-bound model，返回
`StreamingStepOwner::Model` 和 `CloseOpenAiConversation`。factory 必须是 async；同步
`FnOnce` 无法在 model 创建前读取数据库，禁止把 prior response ID 延迟成第一次
`stream()` 时的隐式查询。

runtime 的所有 cancellation 分支顺序固定为：

1. drop/结束 Rig stream，使 owned slot guard 释放；
2. `cancellation_cleanup.before_terminal_status().await`，从 pool 移除并关闭仍可能有
   in-flight response 的 socket；
3. 再把 provider step、tool invocation 和 agent run 写成 canceled；
4. drain publication 后才允许 app 从 `ActiveRun::Stopping` 移除该 conversation。

`app/jaco/src/features/conversation/runtime.rs::stop_run` 不再以 cleanup task 覆盖并 drop
原 `Running(Task<()>)`。保留现有 enum 外形，但新增 consuming transition：

```rust
impl ActiveRunTask {
    fn begin_stop(
        self,
        cleanup: impl FnOnce() -> Task<Result<(), String>>,
        cx: &mut Context<ConversationRuntimeStore>,
    ) -> Self;
}
```

`begin_stop` 只接受 `Running(run_task)`；创建的 `Stopping(stop_task)` closure 捕获并先
`run_task.await`，再执行 socket close/数据库 repair/publication drain。调用处通过
`std::mem::replace` 取出原 task 后立即装回 returned `Stopping`，不能 detach 或 drop 原
task。这样运行中 socket 关闭和 run 终态都发生在 delete gate 放行前，且不依赖异步
`Drop`。共同的 AgentBuilder、history、tool hook 和 output accumulator 仍只保留一份。

## 数据库 fresh schema

### `provider_steps` 列

直接编辑 `0001_create_fresh_schema` 的 `provider_steps`。除新增 continuation 列外，还要
一次性补齐整个 step 生命周期约束，不能只约束新增列：

```sql
continuation_kind TEXT
    CHECK (continuation_kind IN ('openai_responses')),
provider_response_id TEXT,
reasoning_context TEXT,
continuation_expires_at DateTime,
continuation_invalidated_at DateTime,
continuation_error_json JSON,
CHECK (
    (
        status = 'queued'
        AND started_at IS NULL
        AND completed_at IS NULL
        AND response_snapshot_json IS NULL
        AND state_snapshot_json IS NULL
        AND error_json IS NULL
    )
    OR
    (
        status = 'running'
        AND started_at IS NOT NULL
        AND completed_at IS NULL
        AND response_snapshot_json IS NULL
        AND state_snapshot_json IS NULL
        AND error_json IS NULL
    )
    OR
    (
        status = 'completed'
        AND started_at IS NOT NULL
        AND completed_at IS NOT NULL
        AND response_snapshot_json IS NOT NULL
        AND state_snapshot_json IS NOT NULL
        AND error_json IS NULL
    )
    OR
    (
        status IN ('failed', 'canceled')
        AND started_at IS NOT NULL
        AND completed_at IS NOT NULL
        AND error_json IS NOT NULL
    )
),
CHECK (
    (
        continuation_kind IS NULL
        AND provider_response_id IS NULL
        AND reasoning_context IS NULL
        AND continuation_expires_at IS NULL
        AND continuation_invalidated_at IS NULL
        AND continuation_error_json IS NULL
    )
    OR
    (
        continuation_kind = 'openai_responses'
        AND provider_response_id IS NOT NULL
        AND length(trim(provider_response_id)) > 0
        AND reasoning_context IS NOT NULL
        AND length(trim(reasoning_context)) > 0
        AND continuation_expires_at IS NOT NULL
        AND status = 'completed'
    )
),
CHECK (
    (continuation_invalidated_at IS NULL AND continuation_error_json IS NULL)
    OR
    (continuation_invalidated_at IS NOT NULL AND continuation_error_json IS NOT NULL)
)
```

新增：

```sql
CREATE UNIQUE INDEX idx_provider_steps_provider_response_id
ON provider_steps(provider_id, provider_response_id)
WHERE provider_response_id IS NOT NULL;

CREATE UNIQUE INDEX idx_usage_events_provider_step
ON usage_events(provider_step_id);
```

不新增独立 cursor table：provider step 本身就是每次 provider request 的 audit 与
continuation source。`state_snapshot_json` 保留非 continuation 的 provider state；不在
JSON 和 normalized columns 双写 response ID。唯一索引防止同一个 provider response 被
错误地当作两次 request 的新 continuation；prior-step 查询继续使用既有
`idx_agent_runs_conversation_id`、`idx_conversation_entries_conversation_seq` 和
`idx_provider_steps_agent_seq`，不新增无法服务该 join/order 的装饰性索引。

`usage_events` 与 provider request 明确定义为一对一：一次 completed provider step 只能
有一条 final usage。token 字段继续是该 final usage 的分解，不记录流式 delta；失败和取消
step 不插入 usage。`idx_usage_events_provider_step` 既是唯一性约束，也替代同列的普通索引
需求。failed/canceled 允许保留 `response_snapshot_json`/`state_snapshot_json`，用于记录
provider 已返回但本地 commit 失败、incomplete 或取消前已观察到的 raw output；但它们
必须有 `error_json`，且绝不能拥有 continuation。

同步修改 Diesel `schema.rs`、`SqlProviderStepRow`、`SqlNewProviderStepRow`、
`SqlProviderStepStatusChanges`、record/domain mapping、validation/fingerprint 和所有
fixture。初始 migration 名称/版本可保持 fresh schema 的 `0001`；现存开发 DB 必须删除
重建，不能因为 migration 已记录就继续使用旧表。

### Repository 与 persistence port

在 `jaco-db` 的 repository contract 增加事务输入/输出；其中 snapshot/usage/
continuation 字段继续复用 `jaco-core` 类型：

```rust
#[derive(Debug, Clone)]
pub struct CompleteProviderStep {
    pub response_snapshot: ProviderStepResponseSnapshot,
    pub state_snapshot: ProviderRunStateSnapshot,
    pub continuation: Option<ProviderContinuationSnapshot>,
    pub usage: ProviderUsage,
}

#[derive(Debug, Clone)]
pub struct CompletedProviderStep {
    pub step: ProviderStepRecord,
    pub usage: UsageEventRecord,
}
```

`FreshRepository`、`AgentPersistence`、`SessionAgentPersistence` 和 test
`DirectAgentPersistence` 同步增加：

```rust
fn complete_provider_step_with_usage(
    &self,
    provider_step_id: &str,
    completion: CompleteProviderStep,
) -> Result<CompletedProviderStep>;

fn latest_completed_provider_step_before_trigger(
    &self,
    conversation_id: &str,
    trigger_entry_id: &str,
) -> Result<Option<ProviderStepRecord>>;

fn invalidate_provider_continuation(
    &self,
    provider_step_id: &str,
    invalidated_at: OffsetDateTime,
    error: RunErrorPayload,
) -> Result<ProviderStepRecord>;
```

`complete_provider_step_with_usage` 使用一个 SQLite `immediate_transaction`：

1. 确认 step 仍为 `running`，并验证 response/state/provider/continuation 一致性；
2. 把 status、response/state、continuation、`completed_at/updated_at` 写为 completed；
3. 以 step 派生 conversation/provider/model/date，插入唯一 usage row；
4. 任一步失败整笔回滚，既不能出现 completed step 无 usage，也不能出现 usage 指向
   running/failed step。

普通 `update_provider_step_status` 只保留 running -> failed/canceled 的终态路径，不能再
用于 completed。失效操作只更新 source step 的 normalized continuation columns，不改其原
`completed` status 和 response audit。对应 async trait 使用 owned ID 和 owned
`CompleteProviderStep`；`app/jaco/src/database/session.rs` 必须显式转发三个新方法，不能只
更新 direct persistence。

删除 `AgentPersistence::insert_usage_event` 以及 `SessionAgentPersistence`/
`DirectAgentPersistence` 对它的公开转发；`insert_usage_event_with_conn` 降为
`jaco-db` repository 内部 helper，只能由
`complete_provider_step_with_usage` 的同一 transaction 调用。查询
`usage_events_for_provider_step` 可保留用于测试/统计，但返回长度的领域约束是 0 或 1。

对应 async trait 使用 owned ID。查询算法不能按 provider/model 先过滤：

1. 解析当前 `trigger_entry_id` 在 conversation 中的 seq。
2. 只考虑 trigger entry seq 严格小于当前 seq 的 prior agent run；retry 同一 trigger
   不从上一失败尝试继续。
3. 按 trigger seq DESC、provider step seq DESC 取最新的 completed step。
4. runtime 再检查其 provider/model 是否与当前精确相同、continuation 是否
   `is_available(now)`。
5. 若最新 step 的 provider/model 不同，不向更早记录“跳回”寻找匹配项；模型切换已切断
   conversation chain。

`PersistenceContext`/OpenAI adapter 的初始化 helper 固定为：

```rust
async fn load_openai_continuation_seed(
    &self,
    scope: &OpenAiRunScope,
    now: OffsetDateTime,
) -> Result<OpenAiRunContinuationSeed>;
```

它只组合上述 persistence port 与纯验证，不缓存 repository handle，也不修改 source step。
查询错误直接阻止 model 构造；无记录、provider/model 不同、TTL 到期或已 invalidated 返回
empty seed，而不是错误或向更早记录回跳。

### Conversation 删除的数据库权威 gate

保留现有 repository API：

```rust
pub fn soft_delete_conversation(&self, id: &str) -> Result<ConversationRecord>;
```

在 `crates/jaco-db/src/error.rs` 的 `DbError` 增加：

```rust
#[error(
    "conversation {conversation_id} cannot be deleted while agent run \
     {agent_run_id} is active"
)]
ConversationHasActiveRun {
    conversation_id: String,
    agent_run_id: String,
},
```

`soft_delete_conversation` 改为一个 SQLite `immediate_transaction`：

1. 在同一 transaction 内按 `conversation_id` 查询 `agent_runs.status IN
   ('queued', 'running')`，按 `created_at DESC` 取一条 ID；现有
   `idx_agent_runs_conversation_id` 服务该过滤，不新增索引。
2. 找到非终态 run 时返回 `DbError::ConversationHasActiveRun`，不得更新
   `conversations.status/deleted_at/updated_at`。
3. 没有非终态 run 时才把 conversation 更新为 `deleted` 并返回
   `ConversationRecord`。

数据库检查是最终一致性 gate；UI 不预先禁用或隐藏删除按钮，也不能仅依赖
`ConversationRuntimeStore` 的内存快照。`Queued` 与 `Running` 都阻止删除；等待 tool approval
时 agent run 仍为 `Running`，因此自然命中同一规则。completed/failed/canceled 是允许删除的
终态。

同一 agent run 内的 model/tool turns 不查数据库，由当前 WebSocket session 的最新
response ID 串联。

## Provider step 生命周期

### Generic HTTP/SSE

`PersistingCompletionModel::completion` 继续在 model 内 insert + terminal update。
`PersistingCompletionModel::stream` 只 insert 并返回 stream；runtime 每次收到
`StreamedAssistantContent::Final(raw)` 立即调用：

```rust
PersistenceContext::complete_current_streaming_provider_step_with_usage(
    Some(&raw),
    raw.token_usage(),
    None,
)
```

这发生在 Rig 进入 tool execution 前，因此每个 model turn 的 provider step 都会达到
terminal 状态。blocking `completion` 和 streaming `Final` 都必须调用同一个
`complete_provider_step_with_usage` persistence port，不能再先 update step、后
`insert_usage_event`。整次 run 结束时只调用
`finalize_active_provider_steps` 清理异常残留，不再把它当正常完成路径。

`PersistenceContext` 的失败 helper 改为可选保留 provider audit：

```rust
async fn fail_provider_step_with_audit(
    &self,
    provider_step_id: &str,
    error: RunErrorPayload,
    response_snapshot: Option<ProviderStepResponseSnapshot>,
    state_snapshot: Option<ProviderRunStateSnapshot>,
) -> Result<()>;
```

普通 provider/transport failure 可传 `None`；provider 已完成但本地 commit 失败必须传入
已解析的 raw response/state。该 helper 仍走 failed/canceled 的
`update_provider_step_status`，不插 usage、不创建 continuation。

### Unknown provider output

`PersistenceContext` 增加：

```rust
provider_outputs:
    Arc<Mutex<HashMap<ProviderStepId, Vec<ProviderRawPayload>>>>;

fn record_provider_output(&self, output: serde_json::Value) -> Result<()>;
fn take_provider_outputs(
    &self,
    provider_step_id: &str,
) -> Vec<ProviderRawPayload>;
```

runtime 匹配 `StreamedAssistantContent::Unknown(value)` 时记录到当前 step，并可发
`ProviderStepEvent::ProviderOutputObserved { output }` 给 observer；不创建
`ConversationEntryPayload`，因为 hosted/program/unknown item 在本 issue 中没有已定义的
用户可见语义。finish 时将值写入
`ProviderStepResponseSnapshot.provider_outputs`。`finish_provider_step`、
`fail_provider_step`、`cancel_provider_step` 统一通过一个 terminal helper
`take_provider_outputs(provider_step_id)`，保证所有终态都清理内存 map；失败/取消发生在
已观察到 unknown output 之后时，也把这些 raw items 留在 response audit。找不到 active
step 时 `record_provider_output` 返回 invariant error，不能把 output 错绑到上一个 step。

### WebSocket model

```rust
#[derive(Clone)]
struct OpenAiWebSocketModelClient {
    client: rig::providers::openai::Client,
    pool: OpenAiResponsesSessionPool,
    scope: OpenAiRunScope,
    persistence: PersistenceContext,
    reasoning_policy: OpenAiReasoningPolicy,
    continuation_seed: OpenAiRunContinuationSeed,
}

#[derive(Clone)]
pub(crate) struct OpenAiWebSocketCompletionModel {
    client: rig::providers::openai::Client,
    model: String,
    pool: OpenAiResponsesSessionPool,
    scope: OpenAiRunScope,
    persistence: PersistenceContext,
    reasoning_policy: OpenAiReasoningPolicy,
    run_state: Arc<tokio::sync::Mutex<OpenAiRunContinuationState>>,
}

#[derive(Debug, Clone)]
struct OpenAiRunScope {
    conversation_id: ConversationId,
    trigger_entry_id: ConversationEntryId,
    provider_id: ProviderId,
    model_id: ProviderModelId,
}

impl CompletionModel for OpenAiWebSocketCompletionModel {
    type Response =
        rig::providers::openai::responses_api::CompletionResponse;
    type StreamingResponse = OpenAiWebSocketStreamingResponse;
    type Client = OpenAiWebSocketModelClient;

    fn make(client: &Self::Client, model: impl Into<String>) -> Self;
    fn composes_native_output_with_tools(&self) -> bool {
        true
    }
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<Self::Response>, CompletionError>;
    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>;
}
```

`OpenAiWebSocketStreamingResponse` 必须满足 Rig 0.41 对关联类型的完整约束：
`Clone + Unpin + Send + Sync + Serialize + DeserializeOwned + GetTokenUsage`（WASM build
使用 Rig 的 `WasmCompatSend/WasmCompatSync` 别名）；不能只实现
`GetTokenUsage`。`composes_native_output_with_tools()` 必须显式返回 `true`，与 Rig 自带
OpenAI Responses model 一致，否则 structured output 与 tool calls 的组合路径会改变。

`OpenAiWebSocketModelClient` 是 Jaco 内部的 context-bound construction input，不实现
Rig provider client trait，也不进入 app/global state。OpenAI dispatch 在
`PersistenceContext` 创建后，async factory 调用
`load_openai_continuation_seed(scope, reasoning_policy, now).await`；完成 prior-step 查询、
provider/model/TTL/invalidation 验证后才组装 client，再调用
`OpenAiWebSocketCompletionModel::make(&binding, model_id)`；这样 `make` 返回的 model
始终具备 persistence、reasoning policy 和已初始化 continuation，不需要一个“只为满足
trait、运行必失败”的 unbound state，也不在首次 provider request 内临时查数据库。
`AgentBuilder::new(model)` 直接持有这个实例。`completion` 与 `stream` 共用下述
`execute_attempt`/continuation/terminal helper；blocking 路径直接调用 Rig session
`completion(prepared_request)`（0.41 已返回
`completion::CompletionResponse<responses_api::CompletionResponse>`），streaming 路径用
`send(prepared_request)` + `next_event()` 驱动 public-event decoder，并以
`StreamingCompletionResponse::stream(Box::pin(stream))` 返回 decoder 事件，不能只实现
当前 UI 常走的 streaming 分支。

WebSocket model 自己管理实际 attempt：

1. 每次 `response.create` 前 insert 一个 provider step。
2. terminal completed event 到达后，在向 Rig stream yield `FinalResponse` 前调用
   `complete_provider_step_with_usage`，在一个事务写 usage、response ID、effective
   reasoning context、30-day continuation；事务成功后才更新 run state。
3. `previous_response_id` 被拒绝时，先把第一条 step 标为 failed，并 invalidate source
   continuation；full replay 再插入第二条 step。
4. cancel、incomplete、provider error 和 transport close 都结束当前 step；不能留下
   `running`。
5. `PersistenceContext.last_provider_step_id` 始终指向最新 attempt，因此其后的 tool
   invocation 链接成功 attempt，而不是失败 attempt。

runtime 对 `StreamingStepOwner::Model` 不再二次完成 step。

### Provider 已完成但本地 commit 失败

provider success 与本地 success 明确不是同一状态。HTTP/SSE 和 WebSocket 的 terminal
helper 都采用以下顺序：

1. 保留 provider raw response、usage 和 response ID，但尚不发布 Completed event；
2. 调用 `complete_provider_step_with_usage`；
3. transaction 成功后才更新
   `OpenAiRunContinuationState::mark_completed`、设置 `documents_sent_in_run=true`、允许
   session cached ID 继续使用，并发布 UsageUpdated/Completed；
4. transaction 失败时绝不重放已经成功的 provider request。WebSocket 路径立即
   通过当前 guard `clear_previous_response_id`，随后显式 drop owned slot guard，再按
   identity 从 pool 移除并关闭该 socket；不能持有 slot lock 回调
   `invalidate_connection` 造成自锁。HTTP/SSE 直接返回本地持久化错误；
5. 以独立 best-effort `fail_provider_step` 写
   `code = "local_provider_commit_failed"`，在 failed step 的
   `response_snapshot_json` 保留 raw provider response，在 `error_json` 保留数据库错误
   stage/category（不记录凭据或完整 SQL）。若数据库仍不可写，启动时现有
   `recover_interrupted_runs` 将遗留 running step/run 收敛为 interrupted failed；
6. 向 runtime 返回错误并结束 run，不执行 tool call，也不发布 completed。

由此 session continuation、内存 `run_state` 和数据库只会在同一 commit 成功后一起前移。
故障注入必须覆盖“step update 成功但 usage insert 失败”的旧危险窗口，并证明新事务整体
回滚。

## WebSocket session pool 与全局所有权

### 类型

```rust
#[async_trait]
trait OpenAiResponsesSession: Send {
    fn previous_response_id(&self) -> Option<&str>;
    fn clear_previous_response_id(&mut self);
    async fn send(&mut self, request: CompletionRequest) -> Result<(), CompletionError>;
    async fn next_event(&mut self) -> Result<ResponsesWebSocketEvent, CompletionError>;
    async fn close(&mut self) -> Result<(), CompletionError>;
}

#[async_trait]
trait OpenAiResponsesSessionConnector: Send + Sync {
    async fn connect(
        &self,
        client: &openai::Client,
        model: &str,
        event_timeout: Duration,
    ) -> Result<Box<dyn OpenAiResponsesSession>, CompletionError>;
}

struct RigOpenAiResponsesSessionConnector;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OpenAiSessionKey {
    conversation_id: ConversationId,
    provider_id: ProviderId,
    model_id: ProviderModelId,
    connection_fingerprint: String,
}

struct OpenAiSessionSlot {
    session: Option<Box<dyn OpenAiResponsesSession>>,
    opened_at: Option<Instant>,
    last_used_at: Option<Instant>,
    connection_failed: bool,
}

#[derive(Clone)]
pub struct OpenAiResponsesSessionPool {
    slots: Arc<
        tokio::sync::Mutex<
            HashMap<OpenAiSessionKey, Arc<tokio::sync::Mutex<OpenAiSessionSlot>>>
        >
    >,
    connector: Arc<dyn OpenAiResponsesSessionConnector>,
}
```

production connector 只把上述方法委托给 Rig 0.41
`ResponsesWebSocketSession`/builder，不重写协议；test connector 返回可编排 event、记录
request/close 次数的 fake session。`async-trait` 已是 direct dependency，不新增测试框架或
socket crate。`new()` 安装 `RigOpenAiResponsesSessionConnector`，手写 `Default` 委托
`new()`；测试通过 `with_connector(Arc<...>)` 注入。

`connection_fingerprint` 是 normalized base URL + API key 的 SHA-256；只在内存比较，不
持久化、不日志输出原值或 hash。`sha2 0.11.0` 已是 `crates/jaco-agent` 的 direct
dependency，不新增 hashing crate。

方法：

```rust
impl OpenAiResponsesSessionPool {
    pub fn new() -> Self;
    #[cfg(test)]
    fn with_connector(connector: Arc<dyn OpenAiResponsesSessionConnector>) -> Self;
    async fn slot(&self, key: OpenAiSessionKey) -> Arc<Mutex<OpenAiSessionSlot>>;
    async fn acquire(
        &self,
        key: OpenAiSessionKey,
        client: &openai::Client,
        model: &str,
        event_timeout: Duration,
    ) -> Result<tokio::sync::OwnedMutexGuard<OpenAiSessionSlot>>;
    async fn ensure_open(
        slot: &mut OpenAiSessionSlot,
        client: &openai::Client,
        model: &str,
        event_timeout: Duration,
    ) -> Result<()>;
    async fn remove_slot_if_same(
        &self,
        key: &OpenAiSessionKey,
        expected: &Arc<Mutex<OpenAiSessionSlot>>,
    ) -> bool;
    async fn invalidate_connection(&self, key: &OpenAiSessionKey) -> Result<()>;
    async fn close_key(&self, key: &OpenAiSessionKey) -> Result<()>;
    pub async fn close_conversation(&self, conversation_id: &ConversationId) -> Result<()>;
    async fn close_other_conversation_keys(
        &self,
        conversation_id: &ConversationId,
        keep: &OpenAiSessionKey,
    ) -> Result<()>;
    async fn prune_aged(&self, now: Instant) -> Result<()>;
    pub async fn close_all(&self) -> Result<()>;
}
```

`acquire` 先在短 map lock 中取得/插入 `Arc<Mutex<OpenAiSessionSlot>>`，释放 map lock
后调用 `lock_owned()`，再把 `&mut slot` 交给 `ensure_open`。`ensure_open` 在 slot 无
session、已 known-failed，或 age >= 55 分钟时先 clean close 旧连接，再通过
`client.responses_websocket_builder(model).event_timeout(event_timeout).connect()` 打开
新连接；沿用 Rig 默认 30 秒 connect timeout，不使用 warmup。

map lock 只用于查找/插入 slot，不能跨 network await 持有。每次 response 从 send 到
terminal event 全程持有 `acquire` 返回的 owned slot guard，并通过
`guard.session.as_mut()` 调用 `send/next_event`，满足 OpenAI/Rig 的 one-in-flight
契约。Jaco 已有的 one-active-run-per-conversation 仍保留；slot lock 是 transport
invariant，不替代产品 guard。

每次取得 key 前先 `prune_aged(now)`，再
`close_other_conversation_keys(conversation_id, key)`：同一 conversation 的 provider、
model 或 credential fingerprint 一旦变化，旧 socket 立即 clean close，且 continuation
selection 也不会跳回旧 chain。

`close_key`/`invalidate_connection` 的删除算法固定为：

1. 在短 map lock 中 clone 当前 `(key, Arc<slot>)`；
2. 释放 map lock，取得 slot lock，确保没有 in-flight response；
3. 再短暂取得 map lock，以 `Arc::ptr_eq` 做 identity check，只在 map 仍指向该 slot 时
   remove；不能误删并发创建的新 slot；
4. 从 slot `take()` session、清空时间/failed 标记，释放所有 mutex 后才 await
   `session.close()`。

`prune_aged` 同样不能在 map lock 内读取 slot 或等待 network：

1. map lock 内只 snapshot `Vec<(OpenAiSessionKey, Arc<slot>)>`；
2. 逐项在 map lock 外取得 slot lock 并读取 `opened_at`；
3. 未满 55 分钟直接跳过；已过期则按上述 identity check 从 map 摘除并 take session；
4. 收集待关闭 session，释放全部 mutex 后逐个 await close。

identity check 失败表示该 key 已被替换，只跳过旧 snapshot，不得关闭或删除新 slot。任何
close error 只记录并继续关闭其余 slot，不能把陈旧 entry 放回 map。取消发生在
`response.create` 已发送、terminal event 尚未收到时，runtime 必须先 drop stream/guard，
再 await `invalidate_connection`/`close_conversation` 完成，之后才能写 canceled 终态；
不得依赖 async `Drop` 或只设置 `connection_failed=true` 后继续复用。

### app owner

`ConversationRuntimeStore` 增加：

```rust
openai_sessions: OpenAiResponsesSessionPool,
```

它不是 GPUI `Global`，而是 store-owned cloneable service。所有
`AgentRuntime::new(persistence)` 调用，包括 start/recovery/cancel 路径，都通过
`.with_openai_session_pool(self.openai_sessions.clone())` 获得同一个 pool。应用 shutdown
在 active run/event drain 后调用 `close_all()`，确保 Rig session clean close；普通 run
结束不关闭连接，以便下一用户轮次复用。

另有一条显式删除生命周期路由。删除入口保持可点击；确认框提交后调用
`features/conversation.rs::delete_conversation`，由上述 repository transaction 决定结果。
`ConversationRuntimeStore` 只需要提供成功后的 idle session 清理：

```rust
ConversationRuntimeStore::close_conversation_sessions(
    &mut self,
    conversation_id: ConversationId,
    cx: &mut Context<Self>,
) -> Task<()>;
```

结果数据流固定为：

1. `DbError::ConversationHasActiveRun`：不 cancel run、不切 route、不发布 removed、不关闭或
   遗忘 session；`features/home/sidebar/menu.rs` 捕获该 exact variant，以
   `gpui_component::notification::Notification` +
   `NotificationType::Warning` 通知用户先停止运行再删除。
2. 其他数据库错误：保持现有 error notification；同样不改变 route、catalog 或 session。
3. soft delete 成功：发布 removed；`features/home/workspace.rs::delete_conversation` 只在
   task 成功且当前 route 仍指向同一 conversation 时切到 `NewConversation`；随后通过
   `close_conversation_sessions` 调用 `pool.close_conversation`。
4. session close error 只记录，不回滚已成功的 soft delete；数据库 gate 已证明没有
   queued/running run，删除路径本身不执行 cancel/drain。

删除按钮不能因运行状态 disabled，也不能在点击后静默 `return`；用户必须收到上述 warning。
用户停止运行并等 run 达到 completed/failed/canceled 后，需要再次点击删除。

精确测试交付：

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| queued/running 阻止 soft delete | `crates/jaco-db/src/tests/agent.rs` | `soft_delete_conversation_rejects_non_terminal_agent_run` | 同一 conversation 分别插入 queued、running run | 返回 exact `ConversationHasActiveRun`；conversation 仍 active、`deleted_at` 仍 null |
| terminal 后允许删除 | `crates/jaco-db/src/tests/agent.rs` | `soft_delete_conversation_succeeds_after_agent_run_is_terminal` | completed、failed、canceled 三种终态 fixture | 每种均更新为 deleted；事务返回对应 conversation |
| active-run 错误投影 | `app/jaco/src/features/home/sidebar/menu.rs` | `active_run_delete_failure_requests_stop_without_changing_route` | `TestAppContext`、当前 conversation route、running run | warning 使用新增 i18n；route/catalog 不变；没有 removed event |
| 成功后的 removed/session | `app/jaco/src/features/conversation.rs` | `successful_delete_publishes_removed_and_closes_idle_sessions` | terminal run、记录 close 调用的 test pool | 先成功删除并发布 removed；只关闭目标 conversation key |
| 成功后才切换 route | `app/jaco/src/features/home/workspace.rs` | `delete_conversation_changes_route_only_after_storage_success` | 当前 conversation route、可控制成功/失败的删除任务 | 成功后切换到 conversations；active-run 错误保留原 route |

provider/model/credential 变化不需要设置页主动打断 active run：每个 run 继续使用开始时的
provider snapshot；同一 conversation 的下一次 run 计算新 `OpenAiSessionKey` 后，由
`close_other_conversation_keys` clean close 旧 key。其他 conversation 的 idle 旧连接由
55-minute `prune_aged` 和 app shutdown 收口。

## `previous_response_id` 请求算法

### Run continuation state

```rust
#[derive(Debug, Clone)]
struct OpenAiRunContinuationSeed {
    source_step_id: Option<ProviderStepId>,
    source_response_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SelectedOpenAiRequestContext {
    snapshot: ProviderRequestContextSnapshot,
    explicit_previous_response_id: Option<String>,
}

struct PreparedOpenAiRequest {
    request: CompletionRequest,
    context: ProviderRequestContextSnapshot,
    included_documents: bool,
}

struct OpenAiRunContinuationState {
    source_step_id: Option<ProviderStepId>,
    source_response_id: Option<String>,
    successful_model_calls: u32,
    documents_sent_in_run: bool,
    full_history_fallback_used: bool,
}

impl OpenAiRunContinuationState {
    fn from_seed(seed: OpenAiRunContinuationSeed) -> Self;
    fn select_request_context(
        &self,
        session_previous_response_id: Option<&str>,
    ) -> SelectedOpenAiRequestContext;
    fn prepare_request(
        &self,
        request: CompletionRequest,
        selected: SelectedOpenAiRequestContext,
    ) -> Result<PreparedOpenAiRequest>;
    fn mark_completed(
        &mut self,
        provider_step_id: ProviderStepId,
        response_id: String,
        included_documents: bool,
    );
    fn mark_fallback_used(&mut self);
}
```

async model factory 只查询一次
`latest_completed_provider_step_before_trigger`，将验证后的 ID 放进 seed；无可用
continuation 时两个字段都为 `None`。`select_request_context` 的优先级固定为：

1. `source_response_id=Some(id)` 时始终返回 `PreviousResponse`；
2. session cached ID 与 `id` 相同则不重复序列化；不同或新 socket 为 `None` 时，把
   `id` 放入 `explicit_previous_response_id`；
3. `source_response_id=None` 才返回 `FullHistory`。

`mark_completed` 同时把 `source_step_id`/`source_response_id` 更新为刚完成的 attempt，
递增 `successful_model_calls`，并仅在 `included_documents=true` 时设置
`documents_sent_in_run=true`。它只能在
`complete_provider_step_with_usage` transaction 成功后调用。这样同一 run 的下一次
tool-loop request 若 ID 被拒绝，失效的是最近 successful step，不是 run 启动时查询到的
旧 step；55 分钟轮换或 transport 重连产生的新 socket 也会显式发送这个最新 ID，不会误降
为 `FullHistory`。

### Fresh/full request

以下情况使用 `FullHistory`：

- conversation 没有 prior completed step；
- 最新 prior step provider/model 不同；
- continuation 本地 TTL 已过期或已 invalidated；
- session 连接存在但 run 没有可用 source ID。

发送前必须调用 Rig session `clear_previous_response_id()`，防止 pooled connection 自动
带入旧 chain。请求保留完整 Rig `chat_history`、documents、tools、output schema、
additional params 和当前 leading system instructions。

### Incremental request

使用 `PreviousResponse` 时：

- session 没有缓存同一个 source ID 时（包括首轮 DB hydration、55 分钟轮换、断线重连）
  显式把 run state 的 `source_response_id` 写入 Rig OpenAI
  `AdditionalParameters.previous_response_id`；同一健康 session 已缓存同一 ID 时省略；
- `CompletionRequest.chat_history` 只保留 leading `Message::System` run 和最后一条
  non-system prompt；Rig 0.41 会把 leading system 提升为顶层 `instructions`；
- 第一次 model call 可保留本 run 新增的 `documents`，随后 tool loop 清空
  `documents`，避免同一 static context 重复发送；
- tools、tool choice、output schema、temperature/max tokens、provider tools 和当前
  reasoning policy 每次都保留，因为它们属于当前 request，而不是 conversation input。

Rig `AgentRunStep::CallModel` 保证最后一条消息是当前 prompt：首轮是新用户消息，后续是
tool-result batch 或 corrective feedback。adapter 不通过字符串/role 猜测整个 delta。
`prepare_request` 只返回 `included_documents`，不得提前修改
`documents_sent_in_run`；previous-ID rejection、transport failure 或数据库 commit failure
都不算成功发送。只有 provider terminal success 且本地 transaction 成功后
`mark_completed` 才推进该标志。full-history fallback 因前一请求在
`previous_response_id` 校验阶段被拒绝，仍携带原始 documents。

绝不同时发送 `previous_response_id` 与完整旧 history。即使 provider 能接受，这也会让
旧 user/assistant/tool items 重复进入 chain；而且 OpenAI 明确说明 chain 中历史 input
token 仍计费，continuation 不是免费上下文。

### 结构化失效与一次性回退

只在本次请求实际使用了 previous ID（显式 DB ID 或 session cached ID），且
`CompletionError::provider_response_json()` 返回 `Ok(Some(body))`，其中 body 得到：

```json
{
  "error": {
    "code": "previous_response_not_found",
    "param": "previous_response_id"
  }
}
```

或 future code 仍明确 `param == "previous_response_id"` 时分类为
`previous_response_id_rejected`。不能对 `error.to_string()` 做 substring match。

恢复步骤：

1. 把当前 provider step 标为 failed，error code
   `previous_response_id_rejected`，保留 provider raw body/status。
2. `invalidate_provider_continuation(source_step_id, now, error)`。
3. 调用 session `clear_previous_response_id()`；不因 provider-level rejection 丢弃仍健康的
   socket。
4. 若 `full_history_fallback_used=false`，插入新 provider step，把原始
   `CompletionRequest` 完整 history 重发一次，request context 记为
   `FullHistoryFallback`、reasoning context 为 `CurrentTurn`。
5. fallback 成功后建立新 continuation；再次失败直接向上返回，不循环。

adapter 在 slimming 前必须 clone 完整 request，因此 fallback 不需要重建历史。tool loop
发生回退时，完整 request 已包含已提交的 assistant tool call 与 tool result；它只是再次
调用 model，不会再次执行本地/MCP tool。

连接关闭、timeout、decode error 等 transport failure 只：

- fail 当前 step；
- drop 当前 owned slot guard 后 await `invalidate_connection`，使下次请求新建 socket；
- 把错误返回 runtime。

不要自动 HTTP fallback 或重放当前 response.create，因为无法证明 provider 未接收。

## WebSocket event decoder

Rig 0.41 公开 `ResponsesWebSocketEvent`、`ResponseChunk`、`ItemChunk` 和 OpenAI output
types，但其 SSE `RawChoiceAccumulator` 是 crate-private。Jaco 只实现同构的薄 decoder，
不重写 socket/protocol/client。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OpenAiWebSocketStreamingResponse {
    pub response_id: String,
    pub usage: ResponsesUsage,
    pub reasoning_metadata: Option<Map<String, Value>>,
    pub reasoning_context: Option<String>,
}

impl rig::completion::GetTokenUsage for OpenAiWebSocketStreamingResponse {
    fn token_usage(&self) -> rig::completion::Usage;
}

type OpenAiRawStreamingChoice =
    rig::streaming::RawStreamingChoice<OpenAiWebSocketStreamingResponse>;

struct OpenAiWebSocketEventDecoder {
    response_id: Option<String>,
    usage: Option<ResponsesUsage>,
    reasoning_metadata: Option<Map<String, Value>>,
    reasoning_context: Option<String>,
    deferred_tool_calls: Vec<OpenAiRawStreamingChoice>,
    internal_tool_ids: HashMap<String, String>,
}

impl OpenAiWebSocketEventDecoder {
    fn new() -> Self;
    fn ingest(
        &mut self,
        event: ResponsesWebSocketEvent,
    ) -> Result<Vec<OpenAiRawStreamingChoice>, CompletionError>;
    fn finish(self) -> Result<Vec<OpenAiRawStreamingChoice>, CompletionError>;
}
```

映射规则与 Rig SSE decoder 保持一致：

- `OutputTextDelta`、`RefusalDelta` -> `RawStreamingChoice::Message`；
- reasoning summary/text delta -> `ReasoningDelta`；
- function-call added/args delta/done -> `ToolCallDelta`/deferred `ToolCall`，内部 ID 使用
  `jaco_core::new_id`，同一 provider item 稳定复用；
- completed `Output::Reasoning` 保留 summary/content/encrypted metadata；
- `Output::Message` 保存 message ID；
- `Output::Unknown(Value)` -> `RawStreamingChoice::Unknown`；
- `ResponseCompleted` 捕获 response ID、usage、reasoning metadata/effective context，并在
  `finish` 最后 yield `FinalResponse(OpenAiWebSocketStreamingResponse)`；
- `ResponseFailed`/`ResponseIncomplete`/`Error` 通过
  `CompletionError::from_provider_body` 保留结构化 JSON；
- `Done(done)` 若带可反序列化的完整 completed response，则走与
  `ResponseCompleted` 相同的 terminal capture；否则返回 response error。若
  `response.completed` 已先到，Rig session 会在下一 turn 自动过滤相同 response ID 的
  trailing `response.done`，Jaco 不自行维护第二套 filter；
- terminal event 缺少 response ID、完整 usage 或非空 effective reasoning context 时返回
  response error，不能生成不可恢复 continuation。

不能复制 Rig private source 文件；实现时以 public enum 编译穷尽检查为准，并用相同 fixture
对 HTTP/SSE 与 WebSocket 输出序列做 parity test。

## GPT-5.6 capability、UI、icon 与 i18n

### Model data

模型列表继续由现有 OpenAI `/models` 获取，不硬编码向数据库插入四个 model。只修改
`providers/capabilities.rs::apply_openai_profile`，在 GPT-5.5/5.4 与 generic GPT-5
分支之前加入 `id.starts_with("gpt-5.6")`：

```text
control values: none, low, medium, high, xhigh, max
default: medium
efforts: none, low, medium, high, xhigh, max
summaries: true
source: OpenAI GPT-5.6 official model guidance + checked_at
```

alias、`gpt-5.6-sol`、`gpt-5.6-terra`、`gpt-5.6-luna` 和 snapshot suffix 都由 prefix
规则处理。`openai_model_supports_user_attachments` 的现有 generic `gpt-5*` 分支已经覆盖
5.6，**该函数不改代码**；只增加 5.6 image/PDF regression test，防止实施者重复新增一条
等价分支。

### UI/no-change surfaces

- 继续使用现有 `FormReasoningPicker`/`PickerListDelegate`，不新增 custom component。
- `none/low/medium/high/xhigh/max` 的 reasoning i18n key 已存在，不新增对应 key。
- 不显示 `reasoning.context` 或 Pro toggle，不增加 settings/config/schema 字段。
- 删除按钮保持可点击；active-run 拒绝复用现有 `Notification`，以
  `NotificationType::Warning` 展示，不新增 custom component。
- `app/jaco/locales/{en-US,zh-CN}/main.ftl` 新增：
  - `sidebar-delete-conversation-running-title`：
    `Stop the conversation first` / `请先停止对话`；
  - `sidebar-delete-conversation-running-message`：
    `This conversation is still running. Stop it before deleting it.` /
    `该对话仍在运行，请先停止运行再删除。`
- 不新增 icon；model picker 继续使用现有 `capability-continuation` 文本标签。
- 不改 macOS `InfoPlist.strings` 或 app icon。
- 不增加 `safety_identifier` 的 UI、config、schema、i18n 或请求字段。

## 工作包与依赖顺序

### GA-10：Rig 0.41/RMCP 2.2 前置

执行 [dependency-refresh.md](dependency-refresh.md) DR-60。完成条件是现有 HTTP/SSE、
local/MCP tool、approval、usage 和 max-step tests 在新 API 上通过；不得提前接 WebSocket。

### GA-20：provider step lifecycle + fresh schema

先新增能复现 streaming tool-loop 中间 step 留在 `running` 的测试，再实施 per-Final
完成。随后修改 initial schema、core/DB/persistence types、continuation selection/
invalidation queries 和 reset-DB 文档。`complete_provider_step_with_usage`、usage unique
约束和 fault-injection rollback 必须在进入 WebSocket 工作包前完成。完成后每个实际 model
request 都对应一个终态 step。

### GA-30：GPT-5.6 capability + typed reasoning

增加 family profile、Rig typed policy、request/response snapshot 字段和
`Unknown(Value)` audit。HTTP/SSE 路径先验证 `max` effort 与 no-Pro/no-user-context 决策，
不依赖 WebSocket 才能测试。

### GA-40：WebSocket session/model/decoder

实现 pool、transport selection、context-bound CompletionModel、public-event decoder 和
app store ownership；同时实现数据库 active-run delete gate、warning notification 与成功后的
idle session close。先落 session/connector trait test seam 和完整 Rig trait bounds，再测试
fresh full-history、单个 response、cancel-after-send close，最后测试 tool loop。

### GA-50：continuation + recovery

实现 async prior-step seed、incremental shaping、跨 run pooled reuse、55-minute reconnect
显式 ID hydration、30-day TTL 和结构化一次性 fallback；`documents_sent_in_run` 只在
provider + DB commit 后前移。最后才运行真实 OpenAI smoke/eval。

## 验证矩阵

### Unit

| Requirement | Test file | Proposed test name | Fixture/mock | Exact assertions |
| --- | --- | --- | --- | --- |
| GPT-5.6 capability | `crates/jaco-agent/src/providers/capabilities.rs` | `openai_gpt_5_6_family_has_six_efforts_without_attachment_override` | alias/Sol/Terra/Luna/snapshot IDs | 六档且 default medium；generic `gpt-5*` attachment function 对 image/PDF 为 true；函数实现不新增 5.6 分支 |
| typed reasoning/no-change fields | `crates/jaco-agent/src/providers/openai.rs` | `gpt_5_6_reasoning_policy_serializes_only_runtime_owned_fields` | fresh/continuation/fallback policy table | effort/context/store exact；普通 mode omitted、test Pro typed；`safety_identifier`、`user`、`prompt_cache_key`、`prompt_cache_retention`、`prompt_cache_options` 都不存在 |
| transport selection | `crates/jaco-agent/src/providers/openai.rs` | `gpt_5_6_websocket_requires_official_responses_endpoint` | official/default、normalized official、custom、Azure、other-model records | 只有 official OpenAI + 5.6 + capability 返回 WebSocket |
| Rig trait contract | `crates/jaco-agent/src/providers/openai/websocket/tests.rs` | `websocket_model_preserves_openai_native_output_tool_composition` | compile-time generic assertion + constructed model | streaming response 满足全部 trait bounds；`composes_native_output_with_tools()` 为 true |
| incremental shaping/reconnect | 同上 | `request_context_uses_run_state_id_after_socket_rotation` | seed `resp-1`、fake session cached equal/different/none | equal 时省略 explicit ID；different/none 显式 `resp-1`；三者都只发 instructions + last input，不降为 full history |
| documents commit gate | 同上 | `documents_are_marked_sent_only_after_provider_and_database_commit` | image/PDF request；success/rejected/transport/DB-failure outcomes | 仅完整成功后 flag=true；其余保持 false；fallback 仍带原 documents，下一成功 tool turn 才清空 |
| image/PDF parity | 同上 | `websocket_request_preserves_auto_image_detail_and_pdf_document` | `ImageDetail::Auto` image + Base64 PDF | fake connector 捕获的 request 与 HTTP/SSE typed request media/detail/data 等价 |
| decoder | 同上 | `decoder_maps_all_public_response_event_variants` | text/refusal/reasoning/function/unknown/completed/failed/incomplete/error fixtures | choice 顺序、tool IDs、raw unknown、usage/response ID/effective context exact；缺 ID/usage/context 返回 error |
| one-in-flight/cancel | 同上 | `cancel_after_send_discards_socket_before_next_acquire` | fake session 在 send 后 pending、close recorder | cancel await close 且 slot 被移除；下一 acquire 使用不同 fake session；旧 terminal event 不进入新 turn |
| aged prune identity | 同上 | `prune_aged_uses_snapshot_identity_without_removing_replacement` | paused time、同 key replacement Arc、close recorder | map lock 不跨 await；只关闭 snapshot 中仍同 identity 的 aged slot，不关闭 replacement |
| key isolation/shutdown | 同上 | `session_pool_isolates_credentials_and_closes_all_slots` | 两 conversation、同 conversation 两 fingerprint | key 数量和 close 次数 exact；日志/record 不含 key/hash；close error 不阻止其他 close |

### Database/repository

| Requirement | Test file | Proposed test name | Fixture/fault injection | Exact assertions |
| --- | --- | --- | --- | --- |
| status/timestamp/payload CHECK | `crates/jaco-db/src/tests/schema.rs` | `provider_step_status_constraints_reject_invalid_lifecycle_shapes` | raw SQL table of queued/running/completed/failed/canceled invalid combinations | 每个非法组合触发 CHECK；合法五态各一条成功 |
| continuation CHECK/unique | 同上 | `provider_continuation_requires_completed_step_and_unique_provider_response` | null/partial/empty-context/invalidation-pair rows；同 provider duplicate ID | partial/非 completed/空 context/half invalidation 拒绝；未知非空 future context 可保留；不同 provider 可复用 provider-local ID |
| usage one-to-one | 同上 | `usage_event_is_unique_per_provider_step` | completed step 后插两条 usage | 第二条触发 unique；failed/canceled step 经 repository API 无法插 usage |
| atomic completion | `crates/jaco-db/src/tests/agent.rs` | `complete_provider_step_with_usage_rolls_back_when_usage_insert_fails` | transaction 内安装 test-only `BEFORE INSERT ON usage_events` `RAISE(ABORT, 'inject-usage-failure')` trigger | 返回 DB error；step 仍 running、response/continuation/completed_at 为空、usage 数为 0 |
| completion success | 同上 | `complete_provider_step_with_usage_commits_exactly_one_usage_and_continuation` | running step + OpenAI completion | step completed、唯一 usage、response ID/context/TTL round-trip，二次 complete 返回 terminal invariant |
| TTL/invalidation | 同上 | `provider_continuation_availability_honors_ttl_and_idempotent_invalidation` | now-1ms/now/now+1ms 与两次 invalidation | boundary exact；第一次保存 raw error，第二次返回 `AlreadyInvalidated` |
| prior selection | 同上 | `latest_provider_step_before_trigger_does_not_cross_retry_or_model_switch` | 两 trigger、同-trigger retry、provider/model switch、各 status | 只返回严格 prior trigger 最新 completed；不回跳更早 matching step |
| fallback audit | 同上 | `previous_id_fallback_persists_failed_and_completed_attempts` | rejected step + full fallback + tool invocation | seq 连续；第一 failed/no usage、第二 completed/one usage；tool 链第二条 |
| active-run delete gate | 同上 | `soft_delete_conversation_rejects_non_terminal_agent_run` / `soft_delete_conversation_succeeds_after_agent_run_is_terminal` | 采用上文删除测试 fixture | queued/running exact typed error 且零写；completed/failed/canceled 后成功 |

### Runtime/integration

| Requirement | Test file | Proposed test name | Fixture/mock/fault | Exact assertions |
| --- | --- | --- | --- | --- |
| HTTP/SSE per-turn terminal | `crates/jaco-agent/src/runtime/tests.rs` | `streaming_tool_loop_completes_each_provider_step_with_one_usage` | existing multi-turn mock + two tools | 每次 model call 一条 completed step/usage；无中间 running；max_steps 保持 exact-total |
| provider success/local failure | `crates/jaco-agent/src/providers/openai/websocket/tests.rs` | `local_commit_failure_closes_session_without_replaying_provider_request` | fake completed event + persistence wrapper 在 complete transaction 返回错误 | send=1、tool execution=0、session close=1、run state 未前移；best-effort failed audit 或 recovery marker |
| fresh/tool/next-turn continuation | 同上 | `websocket_tool_loop_and_next_run_send_only_incremental_inputs` | scripted fake events：fresh response、两次 tool、下一 user turn | response IDs 逐次链；后续 request 无旧 history；step/usage/tool link exact |
| restart/aged reconnect hydration | 同上 | `persisted_response_id_hydrates_new_and_aged_sessions` | repository seed + connector creates session #1/#2 + paused 55 min | 两个新 socket 首次 request 都显式同一最新 persisted ID；不发送 full history |
| structured one-shot fallback | 同上 | `previous_response_not_found_replays_full_history_once_without_reexecuting_tool` | structured provider error then success；tool call recorder | 两次 provider attempt、一条 local tool execution、source invalidated、第二 context CurrentTurn；第二失败不循环 |
| unknown transport | 同上 | `transport_failure_is_not_replayed_and_reconnects_next_request` | timeout/close/decode errors | 当前 send 一次、step failed、slot removed；同逻辑请求不 HTTP fallback；下一独立 run 新 socket |
| cancellation ordering | `crates/jaco-agent/src/runtime/tests.rs` | `cancel_after_websocket_send_closes_session_before_terminal_rows` | fake pending session、cancellation token、persistence event recorder | close ack 先于 provider/run canceled update；函数返回时无 running step；下一 run 新 session |
| app stop ownership | `app/jaco/src/features/conversation/runtime.rs` | `stop_run_retains_task_until_socket_close_and_publication_drain` | controlled run task/close ack/publication ack | `Stopping` 期间仍 gated；run task、close、DB repair、drain 后才移除 ActiveRun |
| provider switch | `app/jaco/src/features/conversation/runtime.rs` | `next_run_closes_old_openai_key_after_provider_snapshot_change` | active old snapshot + changed provider/model/fingerprint | active run 不被打断；下一 run close old key/open new key；其他 conversation 不受影响 |
| unknown output audit | `crates/jaco-agent/src/runtime/tests.rs` | `unknown_provider_output_is_audited_without_timeline_entry` | `Unknown(Value)` then complete/fail/cancel variants | raw output 在对应 step；无 chat entry；terminal helper 清内存 map |
| delete UI/runtime | 上文列出的 DB、`features/conversation.rs`、`home/workspace.rs`、`sidebar/menu.rs` 五项测试 | 使用上文 exact names | active error、普通 DB error、terminal success | warning/error、route/catalog/removed、close/no-close 均按已确认删除数据流 |

### 真实 API（显式凭据环境）

使用独立 smoke test 或 ignored test，不把 secret/request content 写日志：

1. `gpt-5.6` + effort `max`，确认 response effective context/usage/ID。
2. 两轮稳定目标的 `all_turns` continuation，第二轮 request snapshot 为 incremental。
3. 至少两次 local tool round-trip，比较 HTTP/SSE 与 WebSocket final text/tool sequence。
4. 人工注入不存在 ID，验证结构化 fallback 与两条 provider step。
5. 若可控，跨新 socket 使用 stored ID；记录是否从 persisted state hydration。
6. 一张 `ImageDetail::Auto` 图片和一份 PDF，确认请求成功并记录 input token/latency，
   不以旧模型尺寸成本作为 hard assertion。
7. 对稳定长 prefix 重复两次，记录 `cached_input_tokens`/`cache_write_input_tokens`；只验证
   usage 持久化，不要求 provider 必然 cache hit，也不因此启用 explicit caching。

### 命令

```sh
cargo fmt
cargo test -p jaco-core -p jaco-db
cargo test -p jaco-agent
cargo test -p jaco
cargo check --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo tree -i rig-core
cargo tree -i rig-agent
cargo tree -i rmcp
git diff --check
```

最终仍由 macOS/Linux/Windows CI 覆盖。WebSocket 真实 API 测试不作为无凭据 CI 的硬门禁，
但 public-event fixture、request shaping、DB 和 runtime tests 必须在 CI 中运行。

## 实施停止条件

遇到以下任一情况，不自行扩大或猜测：

- Rig 0.41 public WebSocket event 无法无损产出 Rig `RawStreamingChoice`，需要修改/patch
  上游 crate；
- OpenAI 对 `previous_response_id` 的错误 payload 与正式文档/API fixture 不同，无法做
  结构化分类；
- `AgentRunner` 会产生 adapter 无法判断的非增量 prompt 形态（例如 Repeat retry 丢弃了
  cached response，但仍自动链到它）；
- custom OpenAI endpoint 是否支持 WebSocket 必须成为产品选择；
- 实施证据显示需要把 Pro/context 暴露给用户。

这些问题不持久化成臆测方案；执行者应中断并直接向用户确认或先推动上游。
