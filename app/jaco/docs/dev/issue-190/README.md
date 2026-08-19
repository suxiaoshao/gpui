# Jaco：按 ToolInvocationId 检查工具调用详情

## 根计划与 owner 边界

- Plan ID：`issue-190`
- 根计划：[Issue #190 root hub](../../../../../docs/dev/issue-190/README.md)
- Owner directory：`app/jaco`
- Owner plan：`app/jaco/docs/dev/issue-190/README.md`
- Owner index：[Jaco 开发计划](../README.md)
- 消费的 root IDs：`S-01`–`S-06`、`S-08`、`S-09`、`S-13`、`S-14`、`S-18`、`S-19`、`C-01`、`ERR-01`、`D-01`–`D-08`、`R-01`–`R-13`
- Owner status：`Implemented`；生产实现与本地自动化已验证，完整 Local/MCP 人工场景和远端三平台 CI 尚未验证
- Owner-local IDs：`E-101`–`E-114`、`D-101`–`D-106`、`F-101`–`F-113`、`L-101`–`L-111`、`ST-101`–`ST-104`、`R-101`–`R-113`、`T-101`–`T-113`
- Assigned WPs：`WP-101`–`WP-104`
- Owns：有界 preview、run 内 invocation projection、GPUI block、页面 interaction/cache、approval availability、Fluent、app-level reload/UI 测试
- Does not own：持久化 schema/domain/load、agent hook 的 snapshot publication、Rig/MCP/provider wire contract、真实 progress

## Owner-local 证据

| E-ID | 结论 | 证据 | 设计后果 |
| --- | --- | --- | --- |
| `E-101` | `ConversationDetailPage` 已拥有 `ListState`、rows、TextView states 与 `expanded_agent_runs` | `src/components/chat/detail.rs::ConversationDetailPage` | invocation expansion/cache 继续由页面拥有，不建 Store/Global |
| `E-102` | 页面收到 reload 后先同步 TextView、再重建 timeline；收到 invocation effect 时当前无动作 | `detail.rs::{handle_conversation_model_event,apply_conversation_effect}` | reload 同步必须先排除 tool entries并刷新 preview cache；增量 effect 精确更新 |
| `E-103` | timeline 顶层只有 User/Agent；run row 持有 `Vec<ConversationEntry>` 并在每次 list render clone row | `detail/timeline.rs::{ConversationTimelineRows,build_rows}`；`detail/message.rs::TimelineRow` | 顶层 key 不变；row 内只保存轻量 invocation metadata/Arc preview |
| `E-104` | `AgentTurnRow::render_details` 扫描 Entry 决定 approval，并为每条 Entry 建 `DetailBlock` | `detail/message.rs::render_details` | 投影在 render 前完成；render 不推断 identity/approval |
| `E-105` | generic `DetailBlock` 用 Entry ID 的 keyed child state，toggle 后没有显式 ListState remeasure | `detail/tool_blocks.rs::DetailBlock` | invocation expansion 由 page 控制，toggle 后重测 Agent row |
| `E-106` | `sync_message_text_states` 和 agent copy 都调用无界 `item_markdown` | `detail.rs::sync_message_text_states`；`detail/message.rs::agent_copy_text` | grouped/unresolved lifecycle entry 永久离开旧 formatter/copy 路径 |
| `E-107` | `item_markdown` 会输出 arguments、structured/raw output 与 approval arguments preview | `src/foundation/conversation_format.rs::item_markdown` | 旧 tool arms 收紧为无 payload 输出，payload 只走 `L-102` |
| `E-108` | model transition 已按 invocation ID replace/insert并发出精确 effect | `crates/jaco-core/src/domain.rs::Transition<ConversationChange>` | 页面无需额外 domain state 或 polling |
| `E-109` | pending已保存 conversation/run/invocation；query只校验 run+invocation、resolve只按 invocation且一次性；approve/deny在成功前清理 `last_errors` | `src/features/conversation/runtime/approval.rs::{PendingApproval,is_pending_for_run,resolve}`；`runtime.rs::{approve_tool_invocation,deny_tool_invocation}` | `L-108` 收紧三元组原子query/resolve，availability驱动页面，no-op不改error |
| `E-110` | runtime 已用 retained FIFO event task把 background publication路由回 Entity | `features/conversation/runtime.rs::{prepare_active_run,spawn_event_listener}` | approval availability 复用同一 Task/channel，不新增 task owner |
| `E-111` | `ListState::remeasure_items` 与 stable row splice 已有封装 | `detail.rs::{remeasure_timeline_row,sync_timeline_list}` | toggle/update 只重测 `TimelineRowKey::Agent(run_id)` |
| `E-112` | 当前 `Collapsible::open/content`、`Button` API 在锁定 SHA 可用；`TextView` 只解析 Markdown/HTML并支持 active Link/Image，图片路径进入 `img(url)` | Cargo.lock `gpui-component#57a9903f`；本地 checkout `collapsible.rs`、`text_view.rs`、`inline_flow.rs` | `L-106` 复用 disclosure/button；preview 必须用普通 text node literal renderer |
| `E-113` | 两个 runtime locale 已有 conversation/tool/approval/copy 文案 | `locales/{en-US,zh-CN}/main.ftl` | 新 key 与现有前缀并列，保持 locale parity |
| `E-114` | Jaco normal dependencies 已含 `jaco-db`、`jaco-conversation`、`serde_json`、`url`、`tempfile` | `app/jaco/Cargo.toml` | reopen 与 bounded preview 测试无需加 dependency/lockfile |

## Owner-local 决定

| D-ID | 决定 | 依据 | 放弃的方案 | 实施落点 |
| --- | --- | --- | --- | --- |
| `D-101` | 新建 feature-private `detail/tool_invocation.rs`，同时拥有轻量 projection、bounded preview 与 invocation block | root `D-01`–`D-03`、`E-103`–`E-107` | 放到 jaco-core/shared crate；继续扩张 generic Entry formatter | `F-101`、`L-101`–`L-106` |
| `D-102` | run details 使用 `Entry | ToolInvocation | UnresolvedToolLifecycle`；outer `ConversationEntry.tool_invocation_id` 是唯一 association；record缺失时同一 outer ID仍只生成一个 unresolved 单元 | root `D-01`、`D-02` | payload inner ID/name/call ID/邻接 fallback；每 entry重复 unresolved card | `F-104`、`L-104`、`ST-104` |
| `D-103` | preview cache 只保存 bounded `Arc<ToolInvocationPreview>`，key 为 invocation ID；增量 revision用 persisted `updated_at`，任何 model reload清空 cache并为仍展开 ID从当前 record重建 | root `D-03`、`D-06`、`E-103` | row clone完整 JSON；只凭 `updated_at` 跨 reload复用；child Entity 保存第二份业务状态 | `F-103`、`L-103`、`ST-102` |
| `D-104` | generic `DetailBlock` 仅渲染非工具 Entry；unresolved lifecycle只显示 outer ID（若有）、Entry kinds/count与 unavailable，不读取任何 payload value | `E-105`–`E-107`、root `R-10` | missing record时回到 raw generic card；展示 payload内 name/call ID | `F-105`、`F-106`、`L-105`–`L-106` |
| `D-105` | broker registration/removal/cancel 通过现有 runtime publication channel发 availability change；页面只读 query为最终 authority | root `D-05`、`E-109`、`E-110` | 依赖 observer事件与broker注册的竞态时序；persisted pending直接显示可操作按钮 | `F-101`、`F-108`、`F-109`、`L-108`–`L-111`、`ST-103` |
| `D-106` | reload保留仍存在 ID的 expansion，但无条件清空全部 preview cache并从 reload后的 record重建 expanded ID；删除 ID同时清理 expansion | root `R-04`、`R-06`、`E-102` | 跨 reload按 `updated_at` 复用；reload重置 expansion；持久化 expansion | `F-103`、`ST-102`、`WP-104` |

## Owner-local 目标设计

### 文件与 ownership tree

```text
app/jaco/
├── src/components/chat/
│   ├── detail.rs                                  # F-101 [Modify] 声明新 sibling模块；页面 expansion/cache、runtime/model event、remeasure、eager TextView过滤
│   └── detail/
│       ├── copy_button.rs                         # F-102 [Add] message/invocation 共用的 keyed CopyButton UI state
│       ├── tool_invocation.rs                     # F-103 [Add] bounded preview、literal renderer、projection、Collapsible block、纯函数/GPUI测试
│       ├── timeline.rs                            # F-104 [Modify] run detail grouping/order、orphan row、精确 invocation update
│       ├── message.rs                             # F-105 [Modify] 渲染 AgentDetailItem、copy 排除 lifecycle、移出 CopyButton
│       └── tool_blocks.rs                         # F-106 [Modify] 只处理非工具 Entry，删除 Entry-based approval action
├── src/foundation/conversation_format.rs          # F-107 [Modify] legacy tool arms不再输出 payload/raw，补安全回归
├── src/features/conversation/
│   ├── runtime.rs                                 # F-108 [Modify] availability publication/query/event、runtime tests
│   └── runtime/approval.rs                        # F-109 [Modify] broker change publication与竞态测试
├── locales/en-US/main.ftl                         # F-110 [Modify] invocation/status/detail/preview/accessibility keys
├── locales/zh-CN/main.ftl                         # F-111 [Modify] 与 en-US 同 key
└── docs/dev/
    ├── README.md                                  # F-112 [Modify] #190 owner index
    └── issue-190/README.md                        # F-113 [Add] 本 owner plan
```

无 `Cargo.toml`、asset、bundle locale、`mod.rs`、Store、Operation、Form 或 database production file 变更。

### L-101：有界 preview 类型与固定预算

`F-103` target declarations：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ToolPreviewLimits {
    max_depth: usize,
    max_nodes: usize,
    max_input_bytes: usize,
    max_string_bytes: usize,
    max_output_bytes: usize,
    max_lines: usize,
}

const TOOL_PREVIEW_LIMITS: ToolPreviewLimits = ToolPreviewLimits {
    max_depth: 12,
    max_nodes: 2_048,
    max_input_bytes: 256 * 1_024,
    max_string_bytes: 8 * 1_024,
    max_output_bytes: 64 * 1_024,
    max_lines: 1_000,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BoundedPreview {
    text: String,
    truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ToolAccessPreview {
    kind: ToolAccessKind,
    target: BoundedPreview,
    normalized_path: Option<BoundedPreview>,
    within_project: bool,
    reason_key: Option<BoundedPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ToolErrorPreview {
    code: BoundedPreview,
    message: BoundedPreview,
    retryable: bool,
    provider: Option<BoundedPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ToolApprovalDecisionPreview {
    approved: bool,
    decided_by: BoundedPreview,
    reason: Option<BoundedPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ToolApprovalPreview {
    status: ApprovalStatus,
    request_reason: BoundedPreview,
    decision: Option<ToolApprovalDecisionPreview>,
    requested_at: OffsetDateTime,
    decided_at: Option<OffsetDateTime>,
    expires_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolInvocationPreview {
    arguments: BoundedPreview,
    access_requests: Vec<ToolAccessPreview>,
    access_requests_truncated: bool,
    approval: Option<ToolApprovalPreview>,
    text_output: Option<BoundedPreview>,
    structured_output: Option<BoundedPreview>,
    error: Option<ToolErrorPreview>,
    provider_raw_hidden: bool,
}
```

- 所有类型均为 feature-private、handwritten、不可序列化；它们只包含允许进入 UI/clipboard 的 bounded text。
- 所有 canonical string保持 persisted原值；只允许在达到预算时截断，不按 key、header、token、URL或路径形态改写。
- `BoundedPreview.text` 始终在 UTF-8 边界内且同时满足 byte/line budget。
- approval 展开固定展示 status、request reason、requested/decided/expires timestamps、decision approved、decided_by、decision reason；重复的 `arguments_preview` 省略，access requests只通过 `ToolAccessPreview` 展示。
- `provider_raw_hidden` 只记录 `output.raw_output` 或 `error.raw` 是否存在，不读取其 value；UI/copy据此显示 localized raw-hidden marker。

### L-102：preview builder 与有界 sink

```rust
pub(super) fn build_tool_invocation_preview(
    invocation: &ToolInvocation,
) -> ToolInvocationPreview;

fn bounded_json_preview(value: &serde_json::Value, limits: ToolPreviewLimits) -> BoundedPreview;
fn bounded_text_preview(value: &str, limits: ToolPreviewLimits) -> BoundedPreview;
fn bounded_metadata_preview(value: &str, limits: ToolPreviewLimits) -> BoundedPreview;
fn bounded_access_requests(
    requests: &[ToolAccessRequestPayload],
    limits: ToolPreviewLimits,
) -> (Vec<ToolAccessPreview>, bool);
```

- JSON visitor 在访问 child 前检查 depth/node/input/output/line budget；object key和string value保持 persisted原值，分别受单 string/input/output预算约束。
- public-to-sibling builder始终使用 `TOOL_PREVIEW_LIMITS`；limits只暴露给同模块的低层 pure helper/tests，生产调用方不能放宽预算。
- visitor 直接向为 marker预留空间的 bounded pretty JSON sink写入；禁止构造 bounded tree后 `to_string_pretty`。最终 sink而非中间节点必须满足 byte/line budget。
- text/metadata helper只负责 UTF-8安全截断、行数和输出预算；不增加 regex，不识别或改写 header、Bearer、URL userinfo、token前缀、secret key或路径。
- arguments 只取 `invocation.input.arguments.value`；structured只取 `output.structured_output.value`；text output逐个处理 `ContentPart`，每个 part计入 node budget，只单遍读取 `ContentPart::Text`并写同一 sink，禁止先 join。
- `bounded_access_requests` 为整个 collection创建一个共享 depth/node/input/output/line budget；每个 request本身计一个 node，`target`、`normalized_path`与`reason_key`共享剩余预算，任一预算耗尽立刻停止并返回 `access_requests_truncated=true`。UI只为返回的 bounded Vec创建 children并显示一个截断 marker。
- name、namespace、call/server/provider ID、approval actor/reason、error code/provider/message和全部access string通过 `bounded_metadata_preview` 或 `bounded_text_preview`，没有无界 `String` 进入 row/render/copy。
- `raw_output.value`、`error.raw.value` 与 `arguments_preview` 在此 API 中没有输入到输出的字段路径；builder只读取前两个 raw字段的 `is_some()`，设置 `provider_raw_hidden`。
- copy formatter从 `ToolInvocationPreview` 和 bounded metadata构造纯文本，绝不重新读取 invocation payload，也不增加 Markdown/HTML wrapper。

### L-103：轻量 projection 与 cache

```rust
#[derive(Clone)]
pub(super) struct ToolInvocationPreviewCacheEntry {
    pub(super) revision: OffsetDateTime,
    pub(super) preview: Arc<ToolInvocationPreview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolSourceKind {
    Local,
    Mcp,
    ProviderHosted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ToolOutcomeSummary {
    Succeeded,
    Failed { code: Option<BoundedPreview> },
    Denied { code: Option<BoundedPreview> },
    Canceled { code: Option<BoundedPreview> },
}

#[derive(Clone)]
pub(super) struct ToolInvocationDetail {
    pub(super) id: ToolInvocationId,
    pub(super) id_label: BoundedPreview,
    pub(super) agent_run_id: AgentRunId,
    pub(super) call_id: BoundedPreview,
    pub(super) source_kind: ToolSourceKind,
    pub(super) namespace: Option<BoundedPreview>,
    pub(super) server_or_provider_id: Option<BoundedPreview>,
    pub(super) tool_name: BoundedPreview,
    pub(super) runtime_tool_name: BoundedPreview,
    pub(super) status: ToolInvocationStatus,
    pub(super) approval_status: Option<ApprovalStatus>,
    pub(super) outcome: Option<ToolOutcomeSummary>,
    pub(super) created_at: OffsetDateTime,
    pub(super) started_at: Option<OffsetDateTime>,
    pub(super) completed_at: Option<OffsetDateTime>,
    pub(super) updated_at: OffsetDateTime,
    pub(super) expanded: bool,
    pub(super) approval_decidable: bool,
    pub(super) preview: Option<Arc<ToolInvocationPreview>>,
}

#[derive(Clone)]
pub(super) struct UnresolvedToolLifecycle {
    pub(super) anchor_entry_id: ConversationEntryId,
    pub(super) outer_invocation_id: Option<ToolInvocationId>,
    pub(super) outer_id_label: Option<BoundedPreview>,
    pub(super) entry_kinds: ToolLifecycleEntryKinds,
    pub(super) entry_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ToolLifecycleEntryKinds {
    pub(super) tool_call: bool,
    pub(super) tool_result: bool,
    pub(super) approval_request: bool,
    pub(super) approval_decision: bool,
}

#[derive(Clone)]
pub(super) enum AgentDetailItem {
    Entry(ConversationEntry),
    ToolInvocation(ToolInvocationDetail),
    UnresolvedToolLifecycle(UnresolvedToolLifecycle),
}
```

- `F-103` 固定转换入口：

  ```rust
  pub(super) fn project_tool_invocation_detail(
      invocation: &ToolInvocation,
      expanded: bool,
      preview: Option<Arc<ToolInvocationPreview>>,
      broker_decidable: bool,
  ) -> ToolInvocationDetail;
  ```

- `id`/`agent_run_id`/status/approval status/timestamps直接来自 record；`id_label`、call/name/namespace/source ID保持原值并通过固定 metadata budget构建。
- source映射固定为 `Local -> (Local,None)`、`Mcp { server_id } -> (Mcp, invocation.server_id.as_deref().unwrap_or(server_id))`、`ProviderHosted { provider_id } -> (ProviderHosted, provider_id)`；这里只是同一 record内的展示兼容，不参与 association。
- `approval_decidable = broker_decidable && status == AwaitingApproval && approval.status == Some(Pending)`；不在 render中再次推导。
- terminal `outcome` 只按 status投影；Failed/Denied/Canceled最多附带 bounded `error.code`，Succeeded只显示 localized success。active status返回 `None`，折叠态不读取 output/error message/raw。
- `project_agent_details` 只有在 `expanded=true` 且 cache revision等于 `invocation.updated_at` 时把 `Some(preview)` 传给转换入口，否则传 `None`；转换入口不自行读取 cache。
- `ToolInvocationDetail` 不包含 input/output/error/raw JSON或无界 metadata string；`TimelineRow::clone` 只复制 scalar、`BoundedPreview` 与 `Arc`。
- model-visible label 使用 `runtime_tool_name`，original label 使用 `tool_name`。
- `ProviderHosted` 必须穷尽 match并显示 provider ID；Local/MCP 是 #190 的人工验收主路径。
- active duration 使用 `updated_at-started_at` 的静态值；terminal duration 使用 `completed_at-started_at`；缺失 `started_at` 或 terminal `completed_at` 返回 `None`。

### L-104：ID-only grouping 与排序

```rust
pub(super) fn project_agent_details<'a>(
    entries: impl IntoIterator<Item = &'a ConversationEntry>,
    invocations: impl IntoIterator<Item = &'a ToolInvocation>,
    expanded: &HashMap<ToolInvocationId, bool>,
    previews: &HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
    approval_decidable: &HashSet<ToolInvocationId>,
) -> Vec<AgentDetailItem>;

pub(super) fn is_tool_lifecycle_entry(entry: &ConversationEntry) -> bool;
```

算法：

1. 仅 ToolCall、ToolResult、ApprovalRequest、ApprovalDecision 视为 lifecycle entry。
2. outer `entry.tool_invocation_id` 有值且同 run persisted record 存在时，在该 ID 的第一个关联 Entry `seq` 位置发出一个 `ToolInvocation`；后续同 ID lifecycle entries 被吸收。
3. outer ID有值但record缺失时，按 outer ID吸收该 run 的全部关联 lifecycle entries，在第一条关联 Entry位置发一个 `UnresolvedToolLifecycle`；outer ID也缺失时才按 Entry各发一个 unresolved单元。两种情况都不读取 payload inner ID/name/call ID。
4. 非 lifecycle Entry 原位发出 `Entry`；final assistant/status/error 继续由现有 final-item 规则处理。
5. 没有关联 Entry 的 persisted invocation 在所属 run details 尾部按 `(created_at, id)` 排序。
6. run 没有任何 agent Entry 但存在 invocation 时，`timeline::build_rows` 仍在 `AgentRun.trigger_entry_id` 对应 user row 后创建 `TimelineRowKey::Agent(run_id)`；active-run fallback继续保留。
7. 同一 persisted或unresolved outer `ToolInvocationId` 在结果中至多一次；不同 ID即使 name/args/call ID相同也保持独立。

unresolved投影规则：raw `outer_invocation_id` 只用于分组/identity；用户可见 ID使用 `bounded_metadata_preview` 生成 `outer_id_label`。`ToolLifecycleEntryKinds` 对四种 lifecycle kind各保留一个固定 boolean并按 ToolCall、ApprovalRequest、ApprovalDecision、ToolResult 顺序显示；重复 entry只增加 `entry_count`，不会扩张可见 kind collection。

`F-104` 增加：

```rust
impl ConversationTimelineRows {
    pub(super) fn update_tool_invocation(
        &mut self,
        detail: ToolInvocationDetail,
    ) -> Option<TimelineRowKey>;

    pub(super) fn row_key_for_tool_invocation(
        &self,
        id: &ToolInvocationId,
    ) -> Option<TimelineRowKey>;
}
```

只有现有 block 的 scalar/preview 变化可走精确 replace；新增/删除/anchor/run 改变返回 `None`，调用方执行结构性 `sync_timeline`。

`F-104` / `F-105` 的目标数据与签名：

```rust
pub(super) struct AgentTurnRow {
    pub(super) run_id: Option<AgentRunId>,
    pub(super) run: Option<AgentRun>,
    pub(super) items: Vec<AgentDetailItem>,
    pub(super) text_states: HashMap<ConversationEntryId, Entity<TextViewState>>,
    pub(super) expanded: bool,
    pub(super) on_toggle: OnToggleAgent,
    pub(super) on_toggle_tool_invocation: OnToggleToolInvocation,
    pub(super) on_copy: OnCopy,
    pub(super) on_approval_decision: OnApprovalDecision,
}

#[derive(Clone)]
pub(super) struct TimelineCallbacks {
    on_toggle: OnToggleAgent,
    on_toggle_tool_invocation: OnToggleToolInvocation,
    on_copy: OnCopy,
    on_approval_decision: OnApprovalDecision,
}

pub(super) fn build_rows(
    snapshot: &Conversation,
    active_agent_run_id: Option<&AgentRunId>,
    expanded_agent_runs: &HashMap<AgentRunId, bool>,
    expanded_tool_invocations: &HashMap<ToolInvocationId, bool>,
    previews: &HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
    approval_decidable: &HashSet<ToolInvocationId>,
    text_states: &HashMap<ConversationEntryId, Entity<TextViewState>>,
    callbacks: TimelineCallbacks,
) -> Vec<TimelineRow>;

fn collect_pending_rows<'a>(
    entries: &'a [ConversationEntry],
    runs: &[AgentRun],
    invocations: &[ToolInvocation],
    active_agent_run_id: Option<&AgentRunId>,
) -> (
    Vec<PendingTimelineRow<'a>>,
    HashMap<AgentRunId, Vec<&'a ConversationEntry>>,
);

pub(super) fn callbacks(
    on_toggle: impl Fn(AgentRunId, &mut Window, &mut App) + 'static,
    on_toggle_tool_invocation: impl Fn(ToolInvocationId, &mut Window, &mut App) + 'static,
    on_copy: impl Fn(String, &mut Window, &mut App) -> bool + 'static,
    on_approval_decision: impl Fn(ToolInvocationId, bool, &mut Window, &mut App) + 'static,
) -> TimelineCallbacks;
```

- `build_rows` 按 `agent_run_id` 建 invocation map，并在 `agent_turn_row` 内调用 `project_agent_details`；分组阶段只借用 entry/invocation，最终 `AgentTurnRow` 不保存 raw `Vec<ConversationEntry>` 或完整 `ToolInvocation`。
- `collect_pending_rows` 对只有 persisted invocation、没有 Agent Entry 的 run也创建一次 `PendingTimelineRow::Agent`，位置按 root `D-02` 固定在 trigger user row之后；active fallback与 loose non-run entry保持现有顺序。
- `ConversationTimelineRows::update_entry` 遇到 lifecycle entry直接返回 `None`要求结构性 regroup；非 lifecycle entry只替换 `AgentDetailItem::Entry`，User row逻辑不变。`row_index_for_item`/`TimelineRow::contains_item` 通过下述 helper识别普通 Entry或 unresolved anchor：

  ```rust
  impl AgentDetailItem {
      pub(super) fn entry(&self) -> Option<&ConversationEntry>;
      pub(super) fn contains_entry_id(&self, id: &ConversationEntryId) -> bool;
      pub(super) fn stable_id_suffix(&self) -> String;
  }
  ```

- `AgentTurnRow::final_item` 只在 `AgentDetailItem::Entry` 中按 run `final_entry_id` 查找；`render_details` 排除 final Entry后穷尽 match：Entry -> `DetailBlock`、ToolInvocation -> `ToolInvocationBlock`、Unresolved -> literal unavailable block。
- `agent_copy_text` 只遍历 `AgentDetailItem::Entry` 且再次排除 lifecycle kind；invocation只能通过自己的 CopyButton复制，unresolved永不复制 payload。
- `TimelineCallbacks` / `callbacks(...)` 新增 `on_toggle_tool_invocation: OnToggleToolInvocation`；`agent_turn_row` 原样下传 toggle/copy/approval callbacks，不在 row内创建新 Entity或状态。

### L-105：共享 CopyButton 与 generic block 收紧

`F-101` 顶层显式声明：

```rust
mod copy_button;
mod tool_invocation;
```

`F-102` 的 `OnCopy`、`CopyButton` 保持 `pub(super)`；`message.rs` 与 `tool_invocation.rs` 分别通过 sibling import使用，不从 `detail.rs` re-export。`OnApprovalDecision` 继续由 `message.rs` 拥有，`tool_invocation.rs` 通过 sibling import复用。

`F-102` 将当前 `message.rs` 的 `CopyButtonState` / `CopyButton` 原样迁移为 sibling-private reusable component：

```rust
pub(super) type OnCopy = Rc<dyn Fn(String, &mut Window, &mut App) -> bool + 'static>;

#[derive(IntoElement)]
pub(super) struct CopyButton { /* existing keyed copied-state fields */ }

impl CopyButton {
    pub(super) fn new(
        id: String,
        copy_text: String,
        on_copy: OnCopy,
        copy_tooltip: String,
        copied_tooltip: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Self;
}
```

- message 与 invocation copy ID 分别包含 row/run ID 或 invocation ID。
- copied timer 继续由 `app::tasks::retain_window` 持有；不新增 task字段。
- `DetailBlock::new` 删除 `approval_decidable` 与 `OnApprovalDecision` 参数；只接收非工具 Entry。
- `conversation_format::item_markdown` 的四种 tool lifecycle arms返回空字符串；只被这些 arms使用的 `pretty_json`/`format_raw_payload` 删除。这样即使未来误调用，也没有 tool lifecycle raw payload输出；普通 conversation Error/assistant formatter不在本 issue范围内且保持原行为。

### L-106：ToolInvocationBlock

```rust
pub(super) type OnToggleToolInvocation =
    Rc<dyn Fn(ToolInvocationId, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub(super) struct ToolInvocationBlock {
    detail: ToolInvocationDetail,
    on_toggle: OnToggleToolInvocation,
    on_copy: OnCopy,
    on_approval_decision: OnApprovalDecision,
}

impl RenderOnce for ToolInvocationBlock { /* controlled composition */ }

#[derive(IntoElement)]
pub(super) struct LiteralPreview {
    preview: BoundedPreview,
}

impl LiteralPreview {
    pub(super) fn new(preview: BoundedPreview) -> Self;
}

impl RenderOnce for LiteralPreview { /* ordinary text nodes only */ }
```

组件树固定为：

```text
Collapsible(open = detail.expanded)
├── summary row
│   ├── existing typed tool/status icon
│   ├── model-visible name + source/server + status + persisted duration
│   ├── approve/deny Buttons when approval_decidable
│   └── ghost xsmall Button: expand/collapse
└── content
    ├── metadata rows: alias/original/source/server/invocation/call/time
    ├── bounded arguments/access/approval/text/structured/error sections
    ├── localized unavailable/truncated/raw-hidden notices
    └── CopyButton for the same bounded preview
```

- `Collapsible` 只负责 controlled visibility；交互全部使用 `Button`。
- root/toggle/copy/action `ElementId` 都使用 `tool-invocation-{id}-<role>`，不得使用 index/name/call ID；literal line nodes不声明 interactive ID。
- 每个 access request按 persisted顺序渲染 `kind`、`target`、可选 `normalized_path`、`within_project` 与可选 `reason_key`；kind/boolean使用 Fluent label，三个字符串使用各自 `BoundedPreview`，集合耗尽后只追加一个 localized truncated marker。
- `LiteralPreview` 对已经 bounded 的 `BoundedPreview.text` 按行创建普通 `div().child(SharedString)` text nodes，并使用等宽/换行样式；它不调用 `TextView::{markdown,html}`、Markdown/HTML parser、`Link`/`Image`、`img`、URI opener或任何点击回调。最多 `max_lines` 个 child，空行用普通空 text node保留。
- Markdown image、HTML image、link、本地 URI、嵌套 fence等输入保留为普通字符；同一个 bounded纯文本字符串直接传给 CopyButton。
- 默认 collapsed；展开/收起回调给 page，page 更新 state/cache、`FollowMode::Normal` 并 remeasure Agent row。

### L-107：ConversationDetailPage 方法

`F-101` 增加字段：

```rust
expanded_tool_invocations: HashMap<ToolInvocationId, bool>,
tool_invocation_previews: HashMap<ToolInvocationId, ToolInvocationPreviewCacheEntry>,
```

目标方法：

```rust
fn toggle_tool_invocation(
    &mut self,
    id: ToolInvocationId,
    _window: &mut Window,
    cx: &mut Context<Self>,
);

fn ensure_tool_invocation_preview(
    &mut self,
    id: &ToolInvocationId,
    cx: &mut Context<Self>,
) -> bool;

fn update_timeline_tool_invocation(
    &mut self,
    id: &ToolInvocationId,
    cx: &mut Context<Self>,
);

fn sync_tool_invocation_ui_state(&mut self, cx: &mut Context<Self>);

fn refresh_tool_approval_availability(&mut self, cx: &mut Context<Self>);

fn update_tool_approval_availability(
    &mut self,
    agent_run_id: &AgentRunId,
    id: &ToolInvocationId,
    cx: &mut Context<Self>,
);
```

- opening 时从当前 Ready Conversation 按 ID 读取 record并同步构建 bounded preview；closing 不删除 cache。
- invocation effect后，cache revision与 `updated_at` 不同则失效；expanded ID立即重建 bounded preview，collapsed ID等待首次展开。
- 任意 `ConversationModelEvent::Reloaded` 先清空全部 preview cache，retain当前 snapshot仍存在的 expanded ID，再从 reload后的 authoritative record为 expanded ID重建；不存在的 expansion删除。
- `sync_message_text_states` / `sync_message_text_state` 对 `is_tool_lifecycle_entry` 直接删除/跳过；初始 render 不接触 tool JSON。
- `apply_conversation_effect(ToolInvocationChanged)` 调用精确 update；结构变化 fallback `sync_timeline`。
- timeline full build直接对当前 snapshot 的每个 invocation 调用 `L-108` 并形成瞬时 ID set；单个 availability 通过 `update_tool_approval_availability` 精确查询/更新，RunStarted/RunFinished 通过 `refresh_tool_approval_availability` 逐 ID调用同一精确更新入口。row不长期缓存 broker派生集合。

### L-108：审批可操作性 query

`F-108`：

```rust
impl ConversationRuntimeStore {
    pub(crate) fn can_decide_tool_invocation(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
        tool_invocation_id: &ToolInvocationId,
    ) -> bool;
}
```

返回 true 的充分必要条件：runtime Ready、未 shutdown、conversation 处于 Running attempt、active run ID完全相等、该 broker 的完整三元组 `is_pending_for` 为 true。它不读取数据库或 Conversation Entry。

broker 内部 query同时收紧为完整 key：

```rust
fn is_pending_for(
    &self,
    conversation_id: &ConversationId,
    agent_run_id: &AgentRunId,
    tool_invocation_id: &ToolInvocationId,
) -> bool;

fn resolve_for(
    &self,
    conversation_id: &ConversationId,
    agent_run_id: &AgentRunId,
    tool_invocation_id: &ToolInvocationId,
    decision: ToolApprovalDecision,
) -> Option<ApprovalResolveOutcome>;
```

两者在同一 `pending` mutex临界区验证 `PendingApproval::{conversation_id,agent_run_id}`；不依赖 ID全局唯一假设，也不保留先查后按 ID resolve的 TOCTOU窗口。`L-108` 调用 `is_pending_for`，approve/deny调用 `resolve_for`。

### L-109：approval availability publication

```rust
enum RuntimePublication {
    Event(jaco_agent::AgentRuntimeEvent),
    ToolApprovalAvailabilityChanged {
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
    Drain(Sender<()>),
}

pub(crate) enum ConversationRuntimeEvent {
    // existing variants
    ToolApprovalAvailabilityChanged {
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
}
```

- `ConversationApprovalBroker::new(publications: Sender<RuntimePublication>)` 保存 cloneable unbounded sender；`prepare_active_run` 把与 observer共用的 `tx.clone()` 传入。
- `cancel_all_for_run(&ConversationId, &AgentRunId) -> usize` 与 `cancel_all() -> usize` 保持返回 count，但在锁内收集完整 `(conversation_id, agent_run_id, tool_invocation_id, sender)`，锁外先为整个 batch逐 ID `try_send` availability，全部尝试完成后才逐 sender发送 `Canceled`；stop、finish、shutdown既有 call site全部传 conversation ID并继续消费 count。
- pending成功 insert后先发布 availability再返回等待future；`resolve_for`成功 remove后先 `try_send`对应完整 key，再用移出的 sender发送 decision；cancel遵守上一条 batch顺序。duplicate request、wrong key、已移除 ID不发送。任何 publication或oneshot send都不持有 pending mutex。
- unbounded sender断开时 mutation/decision仍生效，不 retry、不 panic、不记录 payload；后续 model reload或 `RunFinished` 通过 `L-108` 重算。sender可唤醒的agent路径只能在 availability入队尝试完成后继续，因此同一 publication FIFO中的 `Drain` 不会越过该 availability。
- event listener 只在 closure的 `ActiveRunKey` 仍匹配 Running/Stopping attempt时 `cx.emit` + `cx.notify`；shutdown移除 active attempt后的逐 ID publication可以被丢弃，因为直接 `RunFinished` 必须触发页面全 run重算且 `shutting_down` 令 `L-108=false`。
- 页面 runtime subscription只对当前 conversation且 run ID匹配的 availability事件调用 `L-111`，精确重投影 invocation并 remeasure Agent row。

### L-110：approval action

- `ToolInvocationBlock` 只有在 persisted `status == AwaitingApproval`、`approval.status == Pending` 且 `L-108` 为 true 时传入 `approval_decidable=true`。
- 点击 approve/deny继续调用现有 `ConversationRuntimeStore::{approve_tool_invocation,deny_tool_invocation}`。
- approve/deny先取得 exact active conversation/run，再直接调用 `resolve_for`；该方法按 `authority remove -> availability enqueue attempt -> decision wake` 顺序完成。只有 resolve成功后调用方才执行现有 `last_errors.remove(conversation_id)` 与 `cx.notify()`。
- shutting down、recovery非 Ready、missing active/run、stale、duplicate、restart、cancel、wrong conversation/run任一路径返回 false，且不修改 broker、DB、`last_errors` 或其他 UI状态。
- 不新增 `is_deciding`、loading spinner、notification 或 error variant。

### L-111：runtime event 到页面的精确更新

`F-101` 的 `ConversationDetailPage::handle_runtime_event` 增加 exhaustively matched分支：

1. `ToolApprovalAvailabilityChanged`：校验当前 conversation，调用 `update_tool_approval_availability(agent_run_id, id)`；方法重算 `L-108`、replace同 ID detail并 remeasure返回的 Agent row，找不到 ID/run时结构性 `sync_timeline`。
2. `RunStarted`：保持现有 owned-run处理，再为当前 conversation重算所有可见 invocation的 decidable set。
3. `RunFinished`：先让现有错误通知/owned-run逻辑完成，再重算所有 invocation；restart/shutdown/cancel后所有 broker action消失。
4. model `ToolInvocationChanged` 与 runtime availability无顺序假设：两条路径都读取当前 persisted status + current broker authority，只有二者同时满足才显示 action。

### Boundary implementations

#### `C-01` consumer

1. `ConversationRuntimeStore::handle_runtime_event` 继续把 `ConversationTimelineChanged` 和 `ConversationCommitted` 交给 registry。
2. ConversationModel 的现有 transition按 ID upsert并发出 effect。
3. `ConversationDetailPage::apply_conversation_effect` 消费 `ToolInvocationChanged`，失效/重建 preview并更新所属 Agent row。
4. Entry insert仍触发结构性 regroup，以把初始 orphan block移动到第一个 outer-ID anchor。

#### `ERR-01` UI

- `ToolErrorPreview` 只包含 root allowlist的 bounded canonical字段；render 使用 field rows和 `LiteralPreview`。
- error 缺失显示 `conversation-tool-unavailable`；不从 ToolResult/Entry补造。
- inspector 不增加 retry/repair action，现有 conversation/run恢复路径不变。

## GPUI 状态与数据流

### ST-101：持久化 invocation authority

- **Authority：** `ConversationModel.operation().data().as_ref().tool_invocations`
- **Initialization/lifetime：** registry 创建 model并 refresh；页面只持 Entity handle；model/页面销毁时结束
- **Readers：** `L-104`、`L-107`、timeline build/update
- **Mutation：** DB reload 或 `C-01` 的 ConversationChange transition；UI无写入口
- **Publication/projection：** model event -> page -> `ToolInvocationDetail`
- **Persistence：** 现有 jaco-db contract，owner不修改
- **Reset：** Deleted/Reloaded 根据新 snapshot重建；无 stale business copy

### ST-102：展开状态与 preview cache

- **Authority：** `ConversationDetailPage::{expanded_tool_invocations,tool_invocation_previews}`
- **Initialization/lifetime：** 页面创建为空；只活在该页面 Entity lifetime
- **Readers：** timeline projection、`ToolInvocationBlock`
- **Mutation：** `toggle_tool_invocation`、invocation effect、reload/deleted
- **Publication/projection：** page `sync_timeline` / exact row update；cache传入 row为 `Arc`
- **Persistence：** None；interaction/cache state不落盘
- **Reset：** ID消失即删除；incremental revision变化失效；任何 model reload清空 cache并从当前 record重建 expanded，collapsed继续 lazy

### ST-103：审批可操作性

- **Authority：** active run 的 `ConversationApprovalBroker.pending`
- **Initialization/lifetime：** `prepare_active_run` 创建；run finish/shutdown/cancel drop/cancel all
- **Readers：** `L-108`；projection只接收 bool snapshot
- **Mutation：** `ToolApprovalBroker::request_tool_approval`、exact `resolve_for`、cancel；mutation先于availability publication
- **Publication/projection：** `L-109` runtime publication -> page subscription -> exact row update
- **Persistence：** approval audit在 ToolInvocation；可操作性有意不持久化
- **Reset：** restart无 active broker，所有 action unavailable；run结束/取消逐 ID通知，shutdown通过 `RunFinished` 强制全量重算

### ST-104：AgentRun detail projection

- **Authority：** 可丢弃的 `ConversationTimelineRows.rows`
- **Initialization/lifetime：** 页面 build/reload；ListState render clone row
- **Readers：** list render、row lookup/remeasure
- **Mutation：** Entry/Run/Invocation effect精确更新；结构变化完整 rebuild
- **Publication/projection：** stable `TimelineRowKey::Agent(run_id)` + nested invocation ID
- **Persistence：** None
- **Reset：** reload/deleted rebuild；不保留 payload-derived shadow state

### Interaction flow

1. 页面 build时 `L-104` 只读 metadata，collapsed invocation 不访问 payload JSON。
2. 用户用 `Button` 展开 ID；`L-107` 从 `ST-101` 生成 bounded preview，写 `ST-102`，重投影并 remeasure Agent row。
3. `ToolInvocationBlock` 从 bounded preview渲染/复制；没有读取 raw record的回调。
4. `C-01` 或 approval availability到达时，页面按 ID失效/更新同一 block；expanded preview同步重建且有硬预算。
5. 用户审批时 `L-108` 再验证 broker authority；resolve后 availability事件移除 action，后续 persisted invocation change更新 audit/status。

不新增 Entity、Store、Global、Operation、Form、focus owner、overlay、window 或 async preview Task。

## Fluent i18n

以下 key 同时加入 `F-110`、`F-111`；没有 bundle localization：

| Keys | Meaning / variables | Caller/UI state |
| --- | --- | --- |
| `conversation-tool-invocation-title` | 工具标题，`$name` | summary |
| `conversation-tool-source-local`、`conversation-tool-source-mcp`、`conversation-tool-source-provider-hosted` | 三种 `ToolSource` label | summary/detail |
| `conversation-tool-status-requested`、`conversation-tool-status-awaiting-approval`、`conversation-tool-status-running`、`conversation-tool-status-succeeded`、`conversation-tool-status-failed`、`conversation-tool-status-denied`、`conversation-tool-status-canceled` | 七种 persisted status | summary/detail |
| `conversation-tool-duration` | terminal duration，`$duration` | summary |
| `conversation-tool-duration-updated` | active persisted duration，`$duration` | summary |
| `conversation-tool-unavailable` | 缺失值/record | detail/unresolved |
| `conversation-tool-field-model-name`、`conversation-tool-field-original-name`、`conversation-tool-field-source`、`conversation-tool-field-server` | identity/source field labels | detail |
| `conversation-tool-field-invocation-id`、`conversation-tool-field-call-id` | stable/call ID labels | detail |
| `conversation-tool-field-arguments`、`conversation-tool-field-access`、`conversation-tool-field-approval` | input/policy labels | detail |
| `conversation-tool-access-kind-read`、`conversation-tool-access-kind-write`、`conversation-tool-access-kind-execute`、`conversation-tool-access-kind-network` | `ToolAccessKind` labels | access detail |
| `conversation-tool-access-target`、`conversation-tool-access-normalized-path`、`conversation-tool-access-within-project`、`conversation-tool-access-reason-key` | access request field labels | access detail |
| `conversation-tool-value-yes`、`conversation-tool-value-no` | `within_project` typed boolean labels | access detail |
| `conversation-tool-field-created-at`、`conversation-tool-field-started-at`、`conversation-tool-field-completed-at`、`conversation-tool-field-updated-at` | persisted invocation time labels | detail |
| `conversation-tool-field-text-output`、`conversation-tool-field-structured-output`、`conversation-tool-field-error` | output/error section labels | detail |
| `conversation-tool-approval-pending`、`conversation-tool-approval-approved`、`conversation-tool-approval-denied`、`conversation-tool-approval-expired`、`conversation-tool-approval-canceled` | `ApprovalStatus` labels | detail |
| `conversation-tool-approval-request-reason`、`conversation-tool-approval-requested-at`、`conversation-tool-approval-decision`、`conversation-tool-approval-decided-by`、`conversation-tool-approval-decision-reason`、`conversation-tool-approval-decided-at`、`conversation-tool-approval-expires-at` | 完整 approval audit字段；optional值使用 unavailable | detail |
| `conversation-tool-preview-truncated`、`conversation-tool-raw-hidden` | bounded-preview notices | expanded/copy |
| `conversation-tool-expand`、`conversation-tool-collapse`、`conversation-tool-copy-preview` | action label/tooltip，`$name` | Button accessibility |
| `conversation-tool-unresolved` | outer association/record unavailable，`$id` | unresolved block |

现有 `conversation-approval-approve`、`conversation-approval-deny`、`conversation-copy-success`、`conversation-copy-failed*` 继续复用。缺 key fallback只用于开发诊断，测试必须验证两 locale parity。

## 安全、性能与 diagnostics

- root `D-03` 与 `L-101`–`L-102` 是本 owner 的 payload disclosure contract：canonical字符串保持原值，只有预算截断，provider raw envelope默认省略。
- initial timeline/reload不会调用 tool payload formatter；collapsed projection只读取 scalar metadata。
- preview生成是同步、单遍、有界、无 Task；input/output/depth/node/string/line/part上限保证不需要 cancellation/stale async result设计。
- cache只保存 bounded strings；row clone只 clone strings/Arc，不 clone原始 JSON。
- 任何 `tracing` 新增字段只允许 invocation/run/conversation ID、status和 truncation/raw-hidden bool；不得记录 preview、arguments、output、error message、access target或路径。
- `ToolInvocationOutput.raw_output`、`RunErrorPayload.raw` 与重复的 approval `arguments_preview` 没有 debug/reveal/copy路径。
- preview只进入 literal text nodes；任何工具内容都不能创建 link/image/HTML node或发起 URI访问。

## Owner-local 工作包

### WP-101：bounded preview 与 ID-only projection

**前置与 contracts**

- root `D-01`–`D-03`、`D-07`、`D-08`、`ERR-01`、`R-01`–`R-11`

**File IDs**

- `F-103`、`F-104`、`F-107`

**实施顺序**

1. 在 `F-103` 实现 `L-101`–`L-104`，先完成 canonical string fidelity、metadata/access/input-output budget pure tests。
2. 在 `F-104` 将 run details从 raw entries改为 `AgentDetailItem`，实现 first-anchor、absorb、unresolved、orphan与trigger-row算法。
3. 在 `F-107` 删除 legacy raw formatter helper，让 tool lifecycle `item_markdown` 不返回 payload。
4. 补七状态、三 source、完整 approval字段、duration/missing data、same-name concurrency、outer/inner mismatch、wide/deep/Unicode JSON、逐 part流式文本测试。

**Failure/lifecycle**

- preview builder infallible；bounded sink直接产出最终字符串，没有 tree serialization或无界 join fallback。
- missing record/data发 unresolved/unavailable，不触发 reload、error notification或身份推断。

**Tests**

| R-ID | T-ID / file | 场景 | Assertions |
| --- | --- | --- | --- |
| `R-101` | `T-101` / `detail/tool_invocation.rs` | 同 ID lifecycle + 同名不同 ID | 一 ID一单元；不同 ID 独立 |
| `R-102` | `T-102` / same | 七状态/三 source/duration/missing | label facts与 persisted timestamps一致 |
| `R-103` | `T-103` / same | synthetic credential-shaped值出现在普通 JSON string、metadata、header/Bearer/URL/token prefix key/value | 所有 canonical字符串在预算内保持原值，无 heuristic replacement；fixture不含真实凭据 |
| `R-104` | `T-104` / same | wide/deep/long Unicode JSON、超长 key、2048×8KiB strings、海量 ContentPart、海量 access request | JSON/text/access collection的input/output/depth/node/string/line/part预算均满足；UI child数有界；UTF-8有效；单遍停止；显式 truncated |
| `R-105` | `T-105` / same | raw/error/完整approval、Markdown image/link/HTML/local URI/nested fence、copy parity | raw不可达；literal renderer无active node；copy与visible preview相同 |
| `R-106` | `T-106` / `timeline.rs` | missing outer/record、超长outer ID、同outer大量重复entry、orphan、inner mismatch | 同outer一个unresolved；ID label受metadata预算；kind最多固定四种且去重；无 fallback；稳定排序 |

**Focused validation**

- `cargo test -p jaco tool_invocation`
- `cargo test -p jaco conversation_timeline`

**Done condition**

- pure projection与preview不依赖 GPUI Entity/window；collapsed build不访问完整 payload formatter；所有 root内容忠实度与预算测试通过。

### WP-102：GPUI 详情单元与页面状态

**前置与 contracts**

- `WP-101`、`L-103`–`L-107`、`ST-101`、`ST-102`、`ST-104`

**File IDs**

- `F-101`–`F-107`、`F-110`、`F-111`

**实施顺序**

1. 抽取 `F-102` CopyButton并保持现有 keyed timer语义。
2. 实现 controlled `ToolInvocationBlock`；generic `DetailBlock` 删除 tool/approval职责。
3. `AgentTurnRow` 渲染 `AgentDetailItem`，agent copy过滤 lifecycle/unresolved payload。
4. 页面接入 expansion/cache、toggle、reload clear/rebuild/prune、exact row remeasure和 legacy tool TextView过滤；expanded preview使用 `LiteralPreview`。
5. 两 locale一次性加入完整 key并补 parity/key测试。

**Tests**

| R-ID | T-ID / file | 场景 | Assertions |
| --- | --- | --- | --- |
| `R-107` | `T-107` / `detail/tool_invocation.rs` | ID keyed block rebuild与 sibling | Entity/Element/copy state不碰撞 |
| `R-108` | `T-108` / `detail.rs` | toggle、preview revision update | owning Agent row remeasure，expanded保持 |
| `R-109` | `T-109` / `detail.rs` | initial/reload TextView sync + literal preview | grouped lifecycle不建 TextViewState；untrusted content只建普通 text nodes |
| `R-110` | `T-110` / i18n tests | locale parity + variables | 两 locale key/args一致 |

**Focused validation**

- `cargo test -p jaco detail`
- `cargo test -p jaco i18n`
- `cargo check -p jaco`

**Done condition**

- 一个 invocation block完成折叠/展开/copy；无 child-local expansion；所有高度变化精确重测；无 tool payload进入 legacy formatter。

### WP-103：实时更新与审批 authority

**前置与 contracts**

- root `C-01`、`D-05`；`WP-102`、`WP-201`；`L-107`–`L-111`、`ST-103`

**File IDs**

- `F-101`、`F-104`、`F-108`、`F-109`

**实施顺序**

1. 增加 `L-108` triple query/resolve，移除先查后按 ID resolve窗口。
2. broker在 pending insert/remove/cancel 后按完整 key发送 `L-109` availability publication；resolve/cancel必须在唤醒oneshot waiter前完成入队尝试，batch cancel先发布全部 ID再唤醒任何 waiter；所有 stop/finish/shutdown call site保持count语义并路由到 existing runtime Entity/event。
3. timeline build把 `L-108` 的 exact pending set投影为 `approval_decidable`；`L-111` 明确处理 availability、RunStarted、RunFinished和 `C-01` effect。
4. approve/deny只在 exact resolve成功后清理 `last_errors`；成功/duplicate/stale/wrong-conversation/cancel/shutdown竞态补测试。

**Tests**

| R-ID | T-ID / file | 场景 | Assertions |
| --- | --- | --- | --- |
| `R-111` | `T-111` / `features/conversation/runtime.rs` | register/resolve/duplicate/wrong conversation/run/cancel/shutdown/restart + existing last_error；waiter被唤醒后立即入队Drain的竞态 | triple query/event精确；失败路径false且error不变；resolve/cancel的availability始终排在由waiter触发的Drain前；batch每个remove ID先通知再唤醒 |
| `R-112` | `T-112` / `detail.rs` | snapshot-first orphan→Entry anchor、terminal Entry batch→snapshot、availability/RunFinished | 始终同 ID单block；expanded/cache保持；owning row重测；action按双authority变化 |

**Focused validation**

- `cargo test -p jaco conversation_runtime`
- `cargo test -p jaco tool_invocation`

**Done condition**

- persisted audit与 ephemeral action authority清晰分离；页面没有 broker注册竞态或重启 stale button；`C-01` 更新同一 block。

### WP-104：reload 与最终验收

**前置与 contracts**

- `WP-101`–`WP-103`、`WP-201`、root `R-01`–`R-13`

**File IDs**

- 上述 production/test文件；不新增 database owner file

**实施顺序**

1. 在 `F-103`/相邻 app test使用 `FreshStore::open_or_create_initial` 写入 run、invocation、ToolCall/Approval/Result，关闭后重新打开并通过 `ConversationService::load` 取得新 snapshot。
2. 把 reopen snapshot送入真实 `ConversationModelEvent::Reloaded` 页面路径：保留仍存在 ID的 expansion、prune删除 ID、清空所有 cache、从 reload后的 record重建 expanded preview，并以新 runtime验证restart无 broker action。
3. 对 reopen前后调用 `L-104`/`L-102`，断言 association、summary/detail、truncation/raw-hidden marker与copy一致；修改同 ID record的 access字段后再次 reload，断言 cache没有复用旧值。
4. 完成 focused commands，再执行 root aggregate commands一次。
5. 构建 Jaco，以隔离数据执行 Local/MCP approve、deny、fail、cancel、success、同名并发、large JSON、restart/scroll手测。
6. 回填 root完成证据、owner index状态、实际 diff/commands/manual/CI；全部满足后标记 Done。

**Tests**

| R-ID | T-ID / file | 场景 | Assertions |
| --- | --- | --- | --- |
| `R-113` | `T-113` / `detail.rs` + temp DB fixture | DB close/reopen、real Reloaded、ID删除、同 ID access字段变化、新 runtime | persisted projection一致；expansion retain/prune；cache全清并重建；access preview刷新；restart action unavailable |

**Focused validation**

- `cargo fmt`
- `cargo test -p jaco-agent`
- `cargo test -p jaco`
- `cargo clippy -p jaco --all-targets --all-features -- -D warnings`
- `cargo check -p jaco`
- `git diff --check`

**Done condition**

- app-level DB reopen测试与全部 focused checks通过；人工场景结果可观察且已记录；root aggregate/CI gate按实际状态回填。

## Focused validation 与 handoff

| Local R-ID | Root requirement | Evidence |
| --- | --- | --- |
| `R-101`–`R-106` | `R-01`–`R-07`、`R-09`–`R-11` | `T-101`–`T-106` pure tests |
| `R-107`–`R-110` | `R-02`–`R-06`、`R-09`、`R-12` | `T-107`–`T-110` GPUI/i18n tests |
| `R-111`–`R-112` | `R-07`、`R-08` | `T-111`–`T-112` runtime/page tests |
| `R-113` | root `R-04`、`R-06`、`R-08`、`R-10` | `T-113` reopen/real page Reloaded |
| root `R-13` | aggregate/manual | `WP-104` commands、manual、CI |

- 所有 target type/method/component/state owner已固定。
- 所有 payload disclosure只有 `L-102` 一条路径；legacy formatter、agent copy和unresolved fallback均不可输出 provider raw envelope或无界payload。
- 每个 expansion/cache/action/ElementId 使用 `ToolInvocationId`；顶层 list identity仍使用 `AgentRunId`。
- owner实现发现任何需要修改 jaco-core/jaco-db/jaco-conversation schema/API 的事实时，停止对应 WP并更新 root scope；不得在实现中自行扩 owner。
