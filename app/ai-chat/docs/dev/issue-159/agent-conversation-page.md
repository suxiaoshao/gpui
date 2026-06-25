# Issue #159 ai-chat2 Agent Conversation Page 专项计划

本文档是 `app/ai-chat2` agent 对话落地到页面的实施计划。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159/README.md`；本文档只固定 New Conversation 发送、
真实 conversation create/send、匿名 scratch project、agent runtime 事件接线和 conversation
timeline 渲染的具体实现方案。

创建时间：2026-06-05。
最后状态同步：2026-06-11。

当前状态：Agent Conversation Page 首版已实现，首版提交为 `dba4f7c`
（`Implement ai-chat2 agent conversation page`）。后续 #159 分支已继续整理 fresh DB sidebar 状态列化，
并已把 conversation timeline 从 `gpui-component::List` 调整为 GPUI 原生 `ListState` / `list`
和 `vertical_scrollbar`，对齐旧 `ai-chat` conversation detail 的滚动模型；仍需要开 PR 合入
`codex/issue-137-llm-abstractions`。实现范围围绕本文档列出的文件、组件、数据流和验证项推进；
未回到旧 `app/ai-chat` 的 `messages` table、legacy tab/draft store，也未在 GPUI 层重新实现
agent loop。2026-06-11 本地增量已补齐当前 conversation 的 stop generation：运行中发送按钮切换为
同尺寸停止按钮，点击后先 cancel `AgentRunRequest` token，并给 runtime 100ms graceful window；若
run 未自然结束，则将当前 active run 终态化为 `Canceled`，移除 active run，并发 `RunFinished` 让
ChatForm 恢复发送态。

实现时间：2026-06-05 至 2026-06-06。

已验证：

- `cargo fmt`
- `cargo check -p ai-chat2`
- `cargo test -p ai-chat2 timestamp_label`
- `cargo test -p ai-chat-agent -p ai-chat-core -p ai-chat-db`
- `git diff --check`
- 2026-06-10 timeline scroll/list 修正：`cargo fmt`、`cargo check -p ai-chat2`
- 2026-06-11 stop generation 修正：`cargo fmt --package ai-chat2 --package ai-chat-agent`、
  `cargo fmt --package ai-chat2`、`cargo test -p ai-chat-agent cancel`、
  `cargo test -p ai-chat2 conversation_runtime`、`cargo test -p ai-chat2 chat_form`、
  `cargo test -p ai-chat2`、`cargo check -p ai-chat2`、
  `cargo clippy -p ai-chat2 --all-targets -- -D warnings`、`git diff --check`

首版已落地：

- New Conversation 发送后创建 fresh conversation、首条 user item，并打开右侧 conversation page。
- 无项目发送时创建每会话独立 scratch project，并继续按无项目对话展示。
- Conversation page 使用顶部 timeline + 底部无项目选择器 ChatForm；运行中禁用发送按钮。
- Runtime store 接真实 `AgentRuntime`，通过 observer 事件刷新 timeline。
- Timeline 使用 GPUI 原生 `ListState` / `list` + `vertical_scrollbar`，按 user bubble、
  agent final markdown/details collapse 渲染。
- 消息 hover action 已补 copy/time；复制成功按钮切 `Check` 两秒，失败弹通知。
- 时间显示按 Codex app 规则实现：当天只显示时间，最近 1-6 天显示星期+时间，更早显示月日+时间。

仍未完成：

- stop/cancel 已完成；retry/resend UI 仍未完成。
- prompt selector、attachments/multimodal input、Temporary Conversation Window；Temporary Window 的具体状态见
  `app/ai-chat/docs/dev/issue-159/temporary-window.md`。该能力由独立专项跟踪，不属于本 Agent
  Conversation Page 专项的实现范围。
- approval approve/deny action 和 rich tool UI。
- streaming delta 的更细粒度展示、last item preview、完整 project metadata/status UI。

## 目标行为

- 在 New Conversation 页面输入文本后，按 Enter 或点击发送按钮：
  - 立即创建一条 fresh conversation。
  - 立即写入首条 user `conversation_items`。
  - 立即刷新左侧 sidebar，并把新 conversation 展示出来。
  - 立即在右侧打开该 conversation 页面。
  - 随后启动真实 `ai-chat-agent::AgentRuntime` run。
- 如果当前选择了 normal project：
  - conversation 归属该 project。
  - composer skill catalog 使用该 project path。
- 如果当前未选择项目：
  - 每次新建无项目 conversation 都创建一个独立匿名 scratch folder。
  - scratch folder 写入 fresh `projects` 表，`kind = ProjectKind::Scratch`。
  - scratch project 不显示在 Settings Projects 页；其 conversations 继续按现有 sidebar 逻辑归入“无项目对话”。
- Conversation 页面分为两块：
  - 上方是对话详情 timeline。
  - 下方是复用 `ChatForm` 的发送区，不显示项目选择器。
- 运行中：
  - 当前 conversation 的发送按钮切换为停止按钮；点击停止只影响当前 conversation 的 active run。
  - 停止后最多等待 100ms grace；若 runtime 未自然结束，主动把 run 终态化为 `Canceled` 并恢复发送态。
  - 用户取消不写入 `last_errors`，不弹 “runtime canceled” 错误通知。
  - 输入框仍可编辑，避免用户打字被阻塞；只有 send action disabled。
- 展示规则：
  - user message 是靠右的 chat bubble。
  - agent run 如果已有最终 assistant 结论，默认只展示最终结论，并用 markdown 渲染。
  - agent 最终结论上方显示分隔行和 `已处理 {duration}`，右侧用展开图标；点击后展开 reasoning/tool/status details。
  - agent run 如果没有最终结论或仍在运行，默认展开 details，并显示 `处理中 {elapsed}`。
  - user message hover 显示发送时间和复制 icon button；agent message hover 显示复制 icon button 和时间。

## 模块结构

禁止新增 `mod.rs`。本阶段新增普通 `.rs` 文件或目录入口同名文件。

```text
app/ai-chat/docs/dev/
└── issue-159/
    └── agent-conversation-page.md

app/ai-chat2/src/features/home/
├── conversation.rs
├── conversation/
│   ├── format.rs
│   ├── message.rs
│   └── timeline.rs
├── new_conversation.rs
└── shell.rs

app/ai-chat2/src/state/
├── conversation_runtime.rs
├── conversations.rs
├── provider_secrets.rs
├── projects.rs
└── workspace.rs

crates/ai-chat-agent/src/
├── provider_models.rs
├── runtime.rs
├── persistence.rs
└── types.rs

crates/ai-chat-db/src/
├── records.rs
└── repository.rs
```

职责划分：

- `features/home/new_conversation.rs`
  - 订阅 `ChatFormEvent::SendRequested`。
  - 只负责读取当前项目选择、调用 `state::conversations` service、成功后清空 composer、刷新 sidebar 和跳转 route。
  - 创建失败时必须保留 composer 输入，并通过 notification 展示错误。
- `features/home/shell.rs`
  - 将 `HomeRoute::Conversation(id)` 的右侧页面改为缓存 `Entity<ConversationDetailPage>`。
  - 缓存键是 `ConversationId`；route 反复 render 时不能重建 page 内部展开状态和 list state。
- `features/home/conversation.rs`
  - 从 placeholder 改为 `Entity` 页面。
  - 拥有 `conversation_id`、`ConversationLoadSnapshot`、`Entity<ChatForm>`、timeline state、runtime subscription。
  - 负责 bottom ChatForm 的 send event，向已有 conversation 追加 user item 并启动新 run。
- `features/home/conversation/timeline.rs`
  - 定义 `ConversationTimelineRows`、`TimelineRow` 和稳定 row key。
  - Conversation page 持有 GPUI 原生 `ListState`，用 `gpui::list` 渲染 row，并在外层挂
    `vertical_scrollbar`。
- `features/home/conversation/message.rs`
  - 定义 `UserMessageBubble`、`AgentTurnView`、`AgentDetailsView`。
  - 只处理渲染和 hover action，不直接读 DB。
- `features/home/conversation/format.rs`
  - 放 markdown source、copy text、duration、local time、tool/result pretty text helper。
- `state/conversations.rs`
  - app 层 conversation service，负责事务创建、追加 user item、加载 snapshot、构建 run snapshot。
- `state/conversation_runtime.rs`
  - app 层 active run store，负责启动/跟踪 agent run，并把 runtime observer 事件转成 GPUI entity event。
- `state/provider_secrets.rs`
  - 从 Settings Provider 私有模块抽出 provider secret read helper，运行时按 `ProviderSecretRefs` 读取 keychain。
- `state/projects.rs`
  - 新增匿名 scratch project 创建入口。
- `ai-chat-agent`
  - 增加 runtime observer 和 saved provider model dispatch。
- `ai-chat-db`
  - 增加事务 helper 和 conversation load helper；不新增 migration。

## 自定义类型和接口

### `state::conversations`

新增 app 层 service 类型：

```rust
pub(crate) struct CreateConversationRequest {
    pub(crate) project_id: Option<ProjectId>,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) title_seed: String,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
}

pub(crate) struct SendConversationMessageRequest {
    pub(crate) conversation_id: ConversationId,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
}

pub(crate) struct CreatedConversation {
    pub(crate) record: ConversationWithUserItemRecord,
    pub(crate) run_request: AgentRunRequest,
}

pub(crate) type ConversationLoadSnapshot = ConversationTimelineRecords;
```

公开函数：

- `create_conversation(cx, request) -> AiChat2Result<CreatedConversation>`
- `send_conversation_message(cx, request) -> AiChat2Result<SentConversationMessage>`
- `load_conversation(cx, conversation_id) -> AiChat2Result<ConversationLoadSnapshot>`

规则：

- conversation title 从首条 user 文本生成：取 trim 后第一行，限制为 48 个 Unicode scalar；空标题 fallback 到 i18n
  `conversation-default-title`。
- `ConversationSettingsSnapshot` 写入 provider/model/capabilities/tool policy；prompt 先为 `None`。
- `RunSettingsSnapshot` 使用同一份 provider/model/capabilities，加上 `ChatFormSubmit.reasoning_selection`。
- user item 使用 `ConversationItemPayload::Message { role: TranscriptRole::User, content }`。
- `ChatFormSubmit.composer.content_parts` 是 user item 的唯一内容来源；不要从 UI rendered text 反推。
- 成功创建后更新 project metadata `last_active_conversation_id`。

### `state::projects`

新增：

```rust
pub(crate) fn create_anonymous_scratch_project(cx: &mut App) -> AiChat2Result<ProjectRecord>;
```

规则：

- 目录为 `AiChat2Config::data_dir()?.join("scratch-projects").join(new_id())`。
- 使用 `std::fs::create_dir_all` 创建真实目录。
- `NewProject.kind = ProjectKind::Scratch`。
- `ProjectMetadata.scratch_reason = Some("no-project".to_string())`。
- display name 使用 i18n `anonymous-project-name`，例如中文“匿名项目”。
- 每个无项目 conversation 创建一个新的 scratch project，不复用全局 scratch。

### `state::conversation_runtime`

新增：

```rust
pub(crate) struct ConversationRuntimeGlobal(Entity<ConversationRuntimeStore>);

pub(crate) struct ConversationRuntimeStore {
    active_runs: HashMap<ConversationId, ActiveRun>,
    last_errors: HashMap<ConversationId, String>,
    next_run_key: u64,
}

struct ActiveRun {
    key: ActiveRunKey,
    agent_run_id: Option<AgentRunId>,
    cancel_requested: bool,
    cancel: Box<dyn Fn() + Send + Sync>,
    _run_task: Task<()>,
    _event_task: Task<()>,
}

pub(crate) enum ConversationRuntimeEvent {
    RunStarted { conversation_id: ConversationId },
    ConversationChanged { conversation_id: ConversationId },
    RunFinished { conversation_id: ConversationId },
}
```

公开方法：

- `start_run(run_request, window, cx) -> bool`
- `stop_run(conversation_id, cx) -> bool`
- `is_running(conversation_id) -> bool`
- `take_last_error(conversation_id) -> Option<String>`

实现规则：

- 同一 conversation 同时只允许一个 active run；运行中 ChatForm 主按钮切换为 stop。
- `stop_run` 首次调用时 cancel 当前 run token，并启动 100ms grace task。
- grace 到期后如果同一个 `ActiveRunKey` 仍处于 active/cancel-requested 状态，则调用
  `AgentRuntime::cancel_run` 把 run、active provider steps 和 active tool invocations 标为 `Canceled`，
  移除 active run，并发 `ConversationChanged + RunFinished`。
- `finish_run` 和强制 stop 都必须校验 `ActiveRunKey`，避免用户快速启动新 run 后旧 task 迟到 finish 误删新 run。
- Runtime store 通过 `cx.spawn`/`window.spawn` 在前台协调 UI state，通过 `gpui_tokio` 或已有 async bridge 在后台执行 provider I/O。
- observer 收到 DB 写入或 run terminal 后，在 foreground 更新 store 并 `cx.emit(ConversationRuntimeEvent::ConversationChanged { .. })`。
- ConversationDetailPage 订阅 runtime store 事件，只在目标 conversation id 匹配时 reload snapshot。

### `ai-chat-agent` observer

新增 public observer 类型：

```rust
#[derive(Clone)]
pub struct AgentRuntimeObserver {
    sender: Arc<dyn Fn(AgentRuntimeEvent) + Send + Sync>,
}

pub enum AgentRuntimeEvent {
    AgentRunStarted { agent_run_id: AgentRunId, conversation_id: ConversationId },
    AgentRunStatusChanged { agent_run_id: AgentRunId, status: AgentRunStatus },
    ConversationItemAppended { conversation_id: ConversationId, item_id: ConversationItemId },
    ConversationItemUpdated { conversation_id: ConversationId, item_id: ConversationItemId },
    ProviderStepChanged { agent_run_id: AgentRunId, provider_step_id: ProviderStepId },
    ToolInvocationChanged { agent_run_id: AgentRunId, tool_invocation_id: ToolInvocationId },
}
```

`AgentRuntime` 新增：

```rust
pub async fn run_with_saved_provider_observed(
    &self,
    request: AgentRunRequest,
    provider: ProviderRecord,
    secrets: ProviderSecretValues,
    observer: Option<AgentRuntimeObserver>,
) -> Result<AgentRunHandle>;
```

规则：

- `PersistenceContext` 在 append/update conversation item、provider step、tool invocation、agent run status 后触发 observer。
- observer 是 UI 刷新的通知源，不改变 DB truth。
- 原有 `run_with_model` 保留；测试和低层调用仍可直接传 fake/completion model。

### provider model dispatch

`crates/ai-chat-agent/src/provider_models.rs` 把现有测试 helper 提升为生产能力：

- `build_openai_client`
- `build_anthropic_client`
- `build_gemini_client`
- `build_ollama_client`
- `build_openrouter_client`
- `build_deepseek_client`
- `build_mistral_client`

`run_with_saved_provider_observed` 按 `ProviderRecord.kind` 分发：

- `openai`
- `anthropic`
- `gemini`
- `ollama`
- `openrouter`
- `deepseek`
- `mistral`

不支持的 provider kind 返回 typed error，不 fallback 到其他 provider，避免隐藏配置错误。manual/no-listing provider
只有在 kind 本身被上述 runtime client 支持时才能运行。

### `ai-chat-db` repository helper

不新增 migration，不改 schema。

新增事务 helper：

```rust
pub struct NewConversationWithUserItem {
    pub conversation: NewConversation,
    pub user_item: NewConversationItem,
}

pub struct ConversationWithUserItemRecord {
    pub conversation: ConversationRecord,
    pub user_item: ConversationItemRecord,
}

pub fn insert_conversation_with_user_item(
    &self,
    input: NewConversationWithUserItem,
) -> Result<ConversationWithUserItemRecord>;
```

新增加载 helper：

```rust
pub struct ConversationTimelineRecords {
    pub conversation: ConversationRecord,
    pub project: ProjectRecord,
    pub items: Vec<ConversationItemRecord>,
    pub runs: Vec<AgentRunRecord>,
}

pub fn conversation_timeline_records(
    &self,
    conversation_id: &str,
) -> Result<Option<ConversationTimelineRecords>>;
```

规则：

- UI 加载 timeline 只用 `conversation_items` 和 `agent_runs` 组织展示。
- 不为了 timeline 渲染 join `provider_steps`、`tool_invocations`、`approval_decisions` 或 `usage_events`。
- 工具、reasoning、approval、status、error 的用户可见文本必须来自 `conversation_items.payload_json`。

## 数据流

### New Conversation 首次发送

```text
ChatForm Enter / Send button
  -> ChatFormEvent::SendRequested(ChatFormSubmit)
  -> NewConversationPage::handle_send
  -> state::conversations::create_conversation
     -> selected normal project OR create_anonymous_scratch_project
     -> repository.insert_conversation_with_user_item
     -> build AgentRunRequest
  -> ChatForm::clear_after_submit
  -> AiChat2WorkspaceStore::reload_sidebar
  -> AiChat2WorkspaceStore::open_conversation(conversation.id)
  -> ConversationRuntimeStore::start_run
  -> AgentRuntime observer emits ConversationChanged
  -> ConversationDetailPage reloads snapshot
```

失败语义：

- 创建 scratch folder、insert project、insert conversation、append user item 任一步失败：
  - 不清空 composer。
  - 不切 route。
  - 显示 localized error notification。
- conversation 已创建但 agent run 启动失败：
  - 保留 conversation 和 user item。
  - 打开 conversation。
  - 追加/展示 user-visible error item，或至少显示 notification 并让发送按钮恢复。

### 既有 Conversation 继续发送

```text
ConversationDetailPage bottom ChatForm
  -> ChatFormEvent::SendRequested(ChatFormSubmit)
  -> state::conversations::send_conversation_message
     -> repository.append_conversation_item(user message)
     -> build AgentRunRequest
  -> clear composer
  -> reload current snapshot
  -> reload sidebar
  -> ConversationRuntimeStore::start_run
```

规则：

- 发送期间当前 conversation 的主按钮切换为 stop；输入仍可编辑。
- 如果当前 conversation 已有 active run，`submit_snapshot` 不发送事件；Enter 不触发 stop，只有点击 stop 按钮才发出 `StopRequested`。
- 继续发送沿用当前 conversation 的 project；不重新打开项目选择器。

### Runtime update

```text
AgentRuntime / PersistenceContext writes DB
  -> AgentRuntimeObserver::emit(...)
  -> ConversationRuntimeStore receives event on foreground
  -> emits ConversationRuntimeEvent::ConversationChanged
  -> ConversationDetailPage reloads ConversationLoadSnapshot
  -> ConversationTimelineRows rebuilds rows from items + runs
```

规则：

- UI state 只把 observer 当 invalidation signal；DB 仍是唯一 truth。
- 不做页面轮询。
- 运行中 elapsed label 可以用 foreground timer 每秒刷新，但数据 reload 只由 observer 触发。

## Timeline 结构

`TimelineRow`：

```rust
pub(crate) enum TimelineRow {
    UserMessage {
        item: ConversationItemRecord,
    },
    AgentTurn {
        run: Option<AgentRunRecord>,
        items: Vec<ConversationItemRecord>,
        final_item: Option<ConversationItemRecord>,
        expanded: bool,
        default_expanded: bool,
    },
}
```

分组规则：

- `TranscriptRole::User` message 单独渲染为 `UserMessage`。
- 非 user item 优先按 `agent_run_id` 聚合为 `AgentTurn`。
- 没有 `agent_run_id` 的非 user item 归入前一个 synthetic agent turn。
- `final_item` 是该 run 中最后一条 `ConversationItemPayload::Message { role: Assistant, .. }`。
- terminal run 且存在 `final_item` 时默认 collapsed。
- running / waiting / failed without final / no final 时默认 expanded。
- 用户手动展开/收起后，使用 `agent_run_id` 或 synthetic row key 保持本地展开状态，不因 reload 丢失。

列表实现：

- 使用 GPUI 原生 `ListState` / `list`，不使用 `gpui-component::List`；provider/model picker
  这类可搜索/可选择数据列表仍可继续使用组件库 List。
- `ConversationTimelineRows` 持有 `Vec<TimelineRow>` 和稳定 row key，page 持有原生 `ListState`。
- 外层容器通过 `vertical_scrollbar(&list_state)` 显式挂滚动条。
- row render 不直接访问 repository。
- 新消息到达时依赖 `FollowMode::Tail` 在底部跟随；用户正在上方查看历史时不强行滚动，后续再补
  “跳到底部” affordance。

## UI 组件细节

### User message

- Layout：
  - 外层 `h_flex().justify_end().w_full()`。
  - bubble 最大宽度为内容区 70% 或固定 max width，避免长文本横向撑破。
  - bubble 使用 theme primary/secondary 语义，不引入强装饰色。
- 内容：
  - `ContentPart::Text` 合并为 markdown/plain text；第一版只保证 text parts。
  - 后续 file/image/audio part 再扩展为 attachment chip。
- Hover action：
  - 显示发送时间。
  - 显示 `Copy` icon button。
  - copy button 必须有 tooltip。
  - 复制成功/失败使用 notification。

### Agent turn

- Header：
  - 只在存在 agent details 或 run 状态需要说明时显示。
  - collapsed：`Separator` + `已处理 {duration}` + `ChevronRight`。
  - expanded：`Separator` + `已处理 {duration}` 或 `处理中 {elapsed}` + `ChevronDown`。
  - 点击 header 切换 expanded。
- Final answer：
  - 使用 `gpui_component::text::TextView::markdown` 或 `TextViewState::markdown`。
  - markdown source 来自 final assistant item 的 text content。
  - code fence copy/wrap 能力沿用 `gpui-component` markdown text view 的默认能力；不引入新 markdown crate。
- Details：
  - `Reasoning` 渲染 markdown text block；有 `summary` 时优先显示 summary，没有 summary 时显示 `text`。
  - `ToolCall` 显示 tool name、runtime tool name 和 pretty JSON arguments。
  - `ToolResult` 显示 tool output text，structured output 用 pretty JSON。
  - `ApprovalRequest` / `ApprovalDecision` v1 只显示状态文本，不实现 approve/deny action。
  - `Status` 显示 label/message。
  - `Error` 显示 error message 和 code。
- Hover action：
  - 显示复制按钮和时间。
  - copy final answer 时只复制最终结论；copy details 时复制该 details block。

### 时间语义

Zed 参考：`/Users/sushao/Documents/code/zed/crates/agent_ui/src/conversation_view/thread_view.rs`
使用 turn start `Instant` 和 `last_turn_duration` 展示本轮耗时；运行中显示 elapsed duration。

本项目语义：

- `已处理 {duration}` 使用 `agent_runs.completed_at - agent_runs.started_at`。
- `处理中 {elapsed}` 使用 now - `agent_runs.started_at`，每秒刷新。
- user hover 时间使用 `conversation_items.created_at`。
- agent final hover 时间优先使用 `agent_runs.completed_at`；没有 completed time 时使用 final item `updated_at`。
- running hover 时间使用 `agent_runs.started_at`。

### ChatForm 变更

`ChatForm` 新增：

```rust
pub(crate) fn set_submit_blocked(
    &mut self,
    blocked: bool,
    tooltip: Option<SharedString>,
    cx: &mut Context<Self>,
);

pub(crate) fn clear_after_submit(&mut self, cx: &mut Context<Self>);
```

`ComposerEditor` 新增清空能力：

```rust
pub(crate) fn clear(&mut self, cx: &mut Context<Self>);
```

规则：

- `can_send` 必须同时满足 composer 非空、selected model 可用、`submit_blocked == false`。
- 父级 service 成功写 DB 后才调用 `clear_after_submit`。
- 发送失败时不能清空 composer。
- ConversationDetailPage 的 ChatForm 不渲染项目选择器；项目选择器本来就在 NewConversationPage 层，保持不进入通用 ChatForm。

## Icon 和 i18n

### Icon

`app/ai-chat2/src/foundation/assets.rs` 新增：

```rust
Copy => "copy"
```

复用现有：

- `Send`
- `ChevronRight`
- `ChevronDown`
- `FolderX`
- `Settings`

不新增 raw SVG，不把 brand icon 塞进 `IconName`。

### i18n keys

`app/ai-chat2/locales/en-US/main.ftl` 和 `zh-CN/main.ftl` 新增：

```text
conversation-untitled-title
conversation-anonymous-project-name
conversation-create-failed-title
conversation-send-failed-title
conversation-run-failed-title
conversation-copy-tooltip
conversation-copy-success-title
conversation-copy-success-message
conversation-copy-failed-title
conversation-copy-failed-message
conversation-user-sent-at
conversation-agent-started-at
conversation-agent-completed-at
conversation-processed-duration
conversation-processing-duration
conversation-expand-details-tooltip
conversation-collapse-details-tooltip
chat-form-stop-tooltip
conversation-tool-call-title
conversation-tool-result-title
conversation-reasoning-title
conversation-status-title
conversation-error-title
```

带参数的文案用现有 `I18n::t_with_args` + `FluentArgs`，不要手拼 key。

## 依赖和 Cargo feature

`app/ai-chat2/Cargo.toml`：

- 增加：

```toml
[features]
default = ["tree-sitter-languages-basic"]
tree-sitter-languages-basic = [
  "gpui-component/tree-sitter-bash",
  "gpui-component/tree-sitter-diff",
  "gpui-component/tree-sitter-json",
  "gpui-component/tree-sitter-markdown",
  "gpui-component/tree-sitter-rust",
  "gpui-component/tree-sitter-toml",
  "gpui-component/tree-sitter-yaml",
]
```

- 新增直接依赖：

```toml
serde_json = "1.0.149"
time = { version = "0.3.47", features = ["formatting", "local-offset", "serde"] }
```

说明：

- 不新增 markdown 解析库；markdown 渲染复用 `gpui-component` text view。
- `serde_json` 只用于 tool arguments/result pretty display。
- `time` 只用于 UI 层格式化 `OffsetDateTime`。

## 数据库变更

不新增 migration，不改 schema。

现有 fresh schema 已覆盖本阶段需要的 truth：

- `projects`：normal/scratch project。
- `conversations`：conversation metadata、default provider/model、settings snapshot。
- `conversation_items`：canonical timeline。
- `agent_runs`：run status、input/output/error、started/completed 时间。
- `provider_steps` / `tool_invocations` / `approval_decisions` / `usage_events`：执行/debug/统计索引，不作为 UI timeline 主路径。

本阶段只新增 repository helper，避免 UI 直接拼事务或多次散乱查询。

## 验证计划

文档落地阶段：

- `git diff --check`

实现阶段：

- `cargo fmt`
- `cargo test -p ai-chat-db conversation`
- `cargo test -p ai-chat-agent observer`
- `cargo test -p ai-chat-agent provider_models`
- `cargo test -p ai-chat2 conversation`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 sidebar`
- `cargo check -p ai-chat2`
- `cargo clippy -p ai-chat2 -p ai-chat-agent -p ai-chat-core -p ai-chat-db --all-targets --all-features -- -D warnings`
- `git diff --check`

必须覆盖的场景：

- New Conversation 发送成功后立即创建 conversation、user item、刷新 sidebar、打开 conversation。
- 无项目发送创建独立 scratch project，Settings Projects 不显示 scratch project，Sidebar 无项目区显示 conversation。
- 创建失败时 composer 不清空。
- 已有 conversation 继续发送时追加 user item，并启动新 run。
- active run 期间 send submit 被忽略。
- active run 期间主按钮展示 stop；Enter 不触发 stop，点击 stop 发出 `StopRequested`。
- stop 后 cancel token；100ms grace 后仍未结束则强制终态化为 `Canceled` 并发 `RunFinished`。
- stop 后旧 run 的迟到 `finish_run` 不会误删新 run。
- observer 收到 item append / run terminal 后 ConversationDetailPage reload。
- 有 final assistant item 的 agent turn 默认 collapsed。
- running/no-final agent turn 默认 expanded。
- `已处理 {duration}` 使用 run started/completed。
- user/agent hover copy button 有 tooltip，复制文本符合 markdown/source 预期。

## 非目标

- 不实现 retry/resend UI。
- 不实现 approval approve/deny action。
- 不实现 prompt selector。
- 不实现 attachments/multimodal input。
- 不实现 full tool rich UI。
- 不在本页实现 Temporary Conversation Window；该能力的状态见
  `app/ai-chat/docs/dev/issue-159/temporary-window.md`。
- 不新增 DB migration。
