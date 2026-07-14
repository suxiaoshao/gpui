# Issue #173：目标数据模型与 API Contract

## 1. 领域层级

```text
Project
└── Conversation
    ├── ConversationEntry（持久化、有序事实）
    │   ├── User/Assistant Message
    │   ├── SkillActivation / Reasoning
    │   ├── ToolCall / ToolResult
    │   ├── ApprovalRequest / ApprovalDecision
    │   ├── Status
    │   └── Error
    └── AgentRun（后台执行聚合）
        ├── trigger_entry_id -> User Entry
        ├── final_entry_id -> 最终 Entry
        ├── ProviderSteps
        └── ToolInvocations + current approval state
```

Entry 决定 Conversation 历史顺序。Run 不保存正文，不提供第二套历史顺序。

## 2. Core 类型

在 `crates/jaco-core/src/payloads.rs` 使用以下最终形状；visibility 和现有 serde policy 保持
`pub`、`Clone + Debug + PartialEq`、camelCase tagged payload、`deny_unknown_fields`。

```rust
pub type ConversationEntryId = String;

pub enum ConversationEntryKind {
    Message,
    SkillActivation,
    Reasoning,
    ToolCall,
    ToolResult,
    ApprovalRequest,
    ApprovalDecision,
    Status,
    Error,
}

pub enum ConversationEntryStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Canceled,
    WaitingForApproval,
}

pub enum ConversationStatusCode {
    Canceled,
    MaxStepsReached,
    CompletedWithoutOutput,
}

pub struct ConversationStatusEntry {
    pub code: ConversationStatusCode,
    pub message: Option<String>,
}

pub enum ConversationEntryPayload {
    Message {
        role: TranscriptRole,
        content: Vec<ContentPart>,
    },
    SkillActivation(SkillActivationEntry),
    Reasoning {
        text: String,
        summary: Option<String>,
    },
    ToolCall(ToolCallEntry),
    ToolResult(ToolResultEntry),
    ApprovalRequest(ApprovalRequestEntry),
    ApprovalDecision(ApprovalDecisionEntry),
    Status(ConversationStatusEntry),
    Error(RunErrorPayload),
}

pub struct AgentRunInput {
    pub prompt_snapshot: Option<PromptContent>,
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub settings_snapshot: RunSettingsSnapshot,
    pub runtime_snapshot: AgentRuntimeSnapshot,
    pub max_steps: u32,
}

pub struct AgentRunOutput {
    pub final_entry_id: ConversationEntryId,
    pub stopped_reason: AgentStoppedReason,
}
```

`SkillActivationItem/ToolCallItem/ToolResultItem/ApprovalRequestItem/ApprovalDecisionItem` 同步改名
为 `*Entry`，避免 payload 已是 Entry 却继续暴露 Item 命名。

### ContentPart 与 Attachment

`ContentPart` 保持现状：

```rust
pub enum ContentPart {
    Text { text: String },
    Image { attachment_id: AttachmentId },
    File { attachment_id: AttachmentId },
    Audio { attachment_id: AttachmentId },
    Attachment { attachment_id: AttachmentId },
}
```

一条 User Entry 可包含任意多个 part。part 顺序等于用户发送顺序；当前 composer 先提供文字
part，repository 按附件选择顺序追加 attachment part。不得为每个 part 创建独立 Entry。

## 3. SQLite fresh schema（`SCHEMA_VERSION = 1`）

### conversations

仅将 `last_item_seq` 重命名为 `last_entry_seq INTEGER NOT NULL DEFAULT 0`。其他列不变。

### conversation_entries

```sql
CREATE TABLE conversation_entries (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'message', 'skill_activation', 'reasoning', 'tool_call', 'tool_result',
        'approval_request', 'approval_decision', 'status', 'error'
    )),
    status TEXT NOT NULL CHECK (status IN (
        'pending', 'running', 'completed', 'failed', 'canceled', 'waiting_for_approval'
    )),
    agent_run_id TEXT REFERENCES agent_runs(id)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED,
    provider_step_id TEXT REFERENCES provider_steps(id) ON DELETE SET NULL,
    tool_invocation_id TEXT REFERENCES tool_invocations(id) ON DELETE SET NULL,
    provider_item_id TEXT,
    payload_json JSON NOT NULL,
    search_text TEXT NOT NULL DEFAULT '',
    created_at DateTime NOT NULL,
    updated_at DateTime NOT NULL,
    UNIQUE(conversation_id, seq)
);

CREATE INDEX idx_conversation_entries_conversation_seq
    ON conversation_entries(conversation_id, seq);
CREATE INDEX idx_conversation_entries_agent_run_seq
    ON conversation_entries(agent_run_id, seq);
```

### agent_runs

```sql
CREATE TABLE agent_runs (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    trigger_entry_id TEXT NOT NULL REFERENCES conversation_entries(id)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED,
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('user', 'shortcut', 'retry')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'canceled')),
    input_json JSON NOT NULL,
    final_entry_id TEXT REFERENCES conversation_entries(id)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED,
    stopped_reason TEXT CHECK (stopped_reason IN ('completed', 'max_steps', 'canceled', 'failed')),
    error_json JSON,
    created_at DateTime NOT NULL,
    started_at DateTime,
    completed_at DateTime,
    updated_at DateTime NOT NULL,
    CHECK (
        (status IN ('queued', 'running') AND final_entry_id IS NULL AND stopped_reason IS NULL)
        OR
        (status IN ('completed', 'failed', 'canceled') AND final_entry_id IS NOT NULL AND stopped_reason IS NOT NULL)
    ),
    CHECK (
        (status = 'failed' AND error_json IS NOT NULL)
        OR
        (status <> 'failed' AND error_json IS NULL)
    )
);

CREATE INDEX idx_agent_runs_conversation_created
    ON agent_runs(conversation_id, created_at);
CREATE INDEX idx_agent_runs_trigger_entry_created
    ON agent_runs(trigger_entry_id, created_at);
```

`output_json` 删除。`AgentRunRecord.output` 由 `final_entry_id + stopped_reason` 构造：两列同时
为空得到 `None`，同时非空得到 `Some(AgentRunOutput)`；一空一非空是数据库 invariant error。

### 外键与删除

Run 与 Entry 存在双向引用，因此相关外键使用 deferred `NO ACTION`。删除 Conversation 时，
同一事务级联删除两侧记录；单独删除仍被另一侧引用的 Run/Entry 必须失败。repository 不新增
单条 Run/Entry destructive delete API。

## 4. DB records

`crates/jaco-db/src/records.rs`：

```rust
pub struct ConversationRecord {
    // existing fields
    pub last_entry_seq: i32,
}

pub struct ConversationEntryRecord {
    pub id: ConversationEntryId,
    pub conversation_id: ConversationId,
    pub seq: i32,
    pub kind: ConversationEntryKind,
    pub status: ConversationEntryStatus,
    pub agent_run_id: Option<AgentRunId>,
    pub provider_step_id: Option<ProviderStepId>,
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub provider_item_id: Option<String>,
    pub payload: ConversationEntryPayload,
    pub search_text: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct NewConversationEntry {
    pub conversation_id: ConversationId,
    pub status: ConversationEntryStatus,
    pub agent_run_id: Option<AgentRunId>,
    pub provider_step_id: Option<ProviderStepId>,
    pub tool_invocation_id: Option<ToolInvocationId>,
    pub provider_item_id: Option<String>,
    pub payload: ConversationEntryPayload,
}

pub struct AgentRunRecord {
    pub id: AgentRunId,
    pub conversation_id: ConversationId,
    pub trigger_entry_id: ConversationEntryId,
    pub trigger_kind: AgentRunTriggerKind,
    pub status: AgentRunStatus,
    pub input: AgentRunInput,
    pub output: Option<AgentRunOutput>,
    pub error: Option<RunErrorPayload>,
    // timestamps unchanged
}

pub struct NewAgentRun {
    pub conversation_id: ConversationId,
    pub trigger_entry_id: ConversationEntryId,
    pub trigger_kind: AgentRunTriggerKind,
    pub status: AgentRunStatus,
    pub input: AgentRunInput,
}

pub struct FinishAgentRun {
    pub status: AgentRunStatus,
    pub stopped_reason: AgentStoppedReason,
    pub error: Option<RunErrorPayload>,
    pub final_entry: AgentRunFinalEntry,
}

pub enum AgentRunFinalEntry {
    Existing(ConversationEntryId),
    Append(Box<NewConversationEntry>),
}

pub struct FinishedAgentRun {
    pub run: AgentRunRecord,
    pub final_entry: ConversationEntryRecord,
    pub appended_final_entry: bool,
}
```

`FinishAgentRun` 只接受 terminal status；queued/running 仍使用单独的
`UpdateActiveAgentRunStatus`，该类型不含 output/error。

## 5. Repository API

重命名：

```rust
pub fn append_conversation_entry(
    &self,
    input: NewConversationEntry,
) -> Result<ConversationEntryRecord>;

pub fn append_conversation_entry_with_attachments(
    &self,
    input: NewConversationEntry,
    attachments: Vec<NewAttachment>,
) -> Result<ConversationEntryRecord>;

pub fn conversation_entries(
    &self,
    conversation_id: &str,
) -> Result<Vec<ConversationEntryRecord>>;
```

新增/收口：

```rust
pub fn finish_agent_run(
    &self,
    agent_run_id: &str,
    finish: FinishAgentRun,
) -> Result<FinishedAgentRun>;

pub fn request_tool_invocation_approval_with_entry(
    &self,
    entry: NewConversationEntry,
    tool_invocation_id: &str,
    approval: NewToolInvocationApproval,
) -> Result<(ConversationEntryRecord, ToolInvocationRecord)>;

pub fn decide_tool_invocation_approval_with_entry(
    &self,
    entry: NewConversationEntry,
    tool_invocation_id: &str,
    outcome: ToolInvocationApprovalOutcome,
    status: ToolInvocationStatus,
) -> Result<(ConversationEntryRecord, ToolInvocationRecord)>;

pub fn record_auto_tool_invocation_approval_with_entries(
    &self,
    request_entry: NewConversationEntry,
    decision_entry: NewConversationEntry,
    tool_invocation_id: &str,
    approval: ToolInvocationApproval,
    status: ToolInvocationStatus,
) -> Result<([ConversationEntryRecord; 2], ToolInvocationRecord)>;
```

### finish_agent_run 事务顺序

1. `BEGIN IMMEDIATE`。
2. 加载 Run；不存在返回 typed missing/invariant error。
3. 如果已终态，直接返回现有 Run，不追加 Entry，保证幂等。
4. 验证 `finish.status/stopped_reason/error` 组合。
5. `Existing(id)`：Entry 必须存在、属于同一 Conversation 和 Run；failed 不允许引用非 Error。
6. `Append(entry)`：强制补齐同一 conversation/run link 后追加，取得新 ID。
7. 更新 Run 的 status/final_entry_id/stopped_reason/error/timestamps。
8. 提交并返回完整 `FinishedAgentRun`；runtime 使用 `appended_final_entry` 决定是否发
   `ConversationEntryAppended`，不靠二次查询猜测。

数据库错误导致整个事务回滚；runtime 不能在提交前发 `AgentRunStatusChanged`。

### Entry link 验证

- User Message：`agent_run_id/provider_step_id/tool_invocation_id` 全为空。
- Run 产物：`agent_run_id` 必须存在且属于同一 conversation。
- provider step/tool invocation link 必须与 `agent_run_id` 一致。
- ApprovalRequest/Decision 必须有 `tool_invocation_id`。
- Error final Entry 必须保存与 Run `error_json` 相同的 `RunErrorPayload`。
- trigger Entry 必须是同一 conversation 的 `Message { role: User }`。

## 6. Approval 当前状态与历史

`tool_invocations.approval_json` 是 mutable current state；approval Entries 是 immutable history。

| 转换 | 同事务 Entry | ToolInvocation 状态 |
| --- | --- | --- |
| 请求人工审批 | ApprovalRequest | AwaitingApproval + Pending approval |
| 用户允许 | ApprovalDecision(approved) | Running + Approved approval |
| 用户拒绝 | ApprovalDecision(denied) | Denied + Denied approval；随后独立 ToolResult error Entry |
| runtime 取消 | 无 decision payload 时不伪造 ApprovalDecision；ToolResult canceled Entry | Canceled + Canceled approval |
| 自动允许且有 access request | ApprovalRequest + ApprovalDecision 在一个事务按 seq 连续写入 | 保持当前 invocation status + Approved approval |

需要增加一个可在单事务追加两个 Entries 并更新 approval 的 repository 方法用于 auto approval；
不得在 runtime 连续调用三个独立 repository 方法。

## 7. Schema/repository 测试

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| fresh schema | `crates/jaco-db/src/tests.rs` | `fresh_schema_uses_conversation_entries_and_run_links` | 新表/列/check/index 存在，旧表/列不存在 |
| trigger 必须是 User Entry | 同上 | `agent_run_rejects_non_user_trigger_entry` | assistant/missing/cross-conversation 均失败 |
| terminal final 必填 | 同上 | `terminal_agent_run_requires_valid_final_entry` | missing/cross-run/cross-conversation 均回滚 |
| failed final 是 Error | 同上 | `failed_agent_run_requires_matching_error_entry` | payload 与 error_json 一致 |
| 终态幂等 | 同上 | `finishing_terminal_run_does_not_append_duplicate_entry` | Entry count 不变 |
| 事务回滚 | 同上 | `finish_agent_run_rolls_back_entry_when_update_fails` | 无孤立 final Entry |
| approval request 原子 | 同上 | `approval_request_entry_and_state_commit_together` | seq、payload、approval/status 一致 |
| duplicate approval 决定 | 同上 | `duplicate_approval_decision_does_not_append_entry` | 第二次失败且 count 不变 |
| 多模态单 Entry | 同上 | `user_entry_with_text_image_and_file_is_atomic` | 一条 Entry、三个 parts、两条 attachment records |
| 外键 | 同上 | `conversation_entry_foreign_keys_reject_cross_run_links` | `PRAGMA foreign_key_check` 为空 |

## 8. 明确无变化

- `attachments` 表字段、文件存储目录和清理规则不变。
- provider/provider_models/prompts/projects/shortcuts/usage_events 的产品模型不变；仅因 FK/table
  rename 更新 SQL 引用。
- 不新增 cache、pagination、authentication、secret 或 network 行为。
- 不新增 crate、feature、build script、icon 或 asset。
