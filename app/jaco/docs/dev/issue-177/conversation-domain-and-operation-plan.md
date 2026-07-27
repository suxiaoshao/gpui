# Issue #177：Conversation 领域边界与剩余 Operation UI 接入计划

关联 issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)

状态：**待用户 Review，尚未实施**。

本文是 issue #177 的后续重构计划。它保留
[总计划](README.md) 和
[Resource UI 组合计划](resource-ui-composition-plan.md)
作为已经实施过的历史记录，但取代其中以下目标设计：

- app-global `ConversationIndexStore`；
- page-local `ConversationTimelineOperation`；
- `ConversationDetailPage` 自己理解数据库 timeline records、index delta 和 runtime change；
- 主窗口与临时窗口分别创建并持有完整 Conversation 数据 owner。

本计划同时纳入当前 Operation UI 审计中尚未完成的高、中优先级问题。本文只持久化已确认的
方向和实施工作包，不在本轮修改代码或执行最终完整性检查。

## 1. 目标

1. 应用层只理解产品概念：Conversation Catalog、完整 Conversation、Conversation
   可用性和用户操作；不理解数据库 index、timeline records、commit delta 或表组合方式。
2. `Conversation` 是详情页面消费的完整领域数据，消息、附件、Agent Run、工具调用和审批等
   都是它的一部分；Timeline 只是 Conversation 的一种 UI 投影。
3. 当前 `AppSession` 持有一个 `ConversationRegistry`。Registry 按
   `ConversationId` 复用共享的 `Entity<ConversationModel>`，数据库 Session 切换时整体销毁。
4. 主窗口和临时窗口可以同时展示同一个 Conversation。它们共享 Conversation 数据和
   `Operation<Conversation>`，但各自持有独立的窗口组件、Focus、Scroll、Composer、展开状态
   和 Subscription。
5. Conversation Catalog 与单个 Conversation 是两个独立失败域：
   - Catalog：`Operation<Vec<ConversationSummary>>`；
   - Detail：每个 `ConversationModel` 保存自己的 `Operation<Conversation>`。
6. 所有 Operation 消费点完整表达：
   - 无 Data 的运行态：Loading；
   - 无 Data 的 Problem：错误信息和 Refresh/Retry/Repair；
   - Ready：正确数据；
   - 有 Data 的运行态或 Problem：保留旧数据、显示状态和 Refresh/Retry，并禁止依赖该
     Resource 的写操作。
7. Conversation、Project、Skill 和搜索 UI 不再把 `None`、失败或 Loading 压缩成空列表，
   也不保存可以从 Operation/Data 计算出的第二份业务数据。

## 2. 非目标

- 不修改 `gpui-operation` 或 `gpui-store` 的公共 API。
- 不把 `ConversationDetailView`、窗口、Focus、ListState 或 Composer 保存到 Global。
- 不建立应用生命周期的永久 Conversation Global；Registry 只属于当前 `AppSession`。
- 不把所有 Conversation 放入一个
  `Operation<HashMap<ConversationId, Conversation>>`，避免把每个详情绑定到同一失败域。
- 不在 UI 中暴露 `ConversationIndexDelta`、`ConversationTimelineRecords`、
  `FreshRepository` 或数据库 schema。
- 不在本计划中设计数据库 migration、恢复数据库、文件 watcher、Plugin Skill 或
  `http_proxy`。
- 不要求两个窗口共享草稿、滚动位置、焦点或展开状态。以后若产品需要跨窗口共享草稿，应把
  Draft 建模为独立产品数据，而不是共享 View。
- 本轮计划阶段不执行 UI 自动化或最终验收。

## 3. 目标分层

### 3.1 `jaco-core`：领域类型

`jaco-core` 提供不依赖 GPUI、Diesel 或具体数据库 records 的产品类型：

```rust
pub struct ConversationSummary {
    pub id: ConversationId,
    pub project: Option<ProjectSummary>,
    pub title: String,
    pub pinned: bool,
    pub status: ConversationStatus,
    pub updated_at: OffsetDateTime,
}

pub struct Conversation {
    pub summary: ConversationSummary,
    pub entries: Vec<ConversationEntry>,
    pub attachments: Vec<ConversationAttachment>,
    pub runs: Vec<AgentRun>,
    pub tool_invocations: Vec<ToolInvocation>,
}
```

最终字段以现有功能实际消费为准，但应用层不得再直接组合 `jaco_db::*Record` 来获得完整
Conversation。

`ConversationChange` 也应是领域消息，只描述 Conversation 如何变化，不包含数据库 index
维护细节。

### 3.2 `jaco-db`：持久化与聚合查询

`jaco-db` 继续负责：

- transaction；
- schema/record；
- 从多张表读取 Conversation 所需的完整持久化数据；
- 将数据库 records 映射为稳定的领域输入；
- 原子提交创建、发送、pin、delete、Agent Run 和工具调用变化。

`ConversationIndexDelta` 如果仍对数据库内部提交有价值，可以保留为 crate-private 持久化实现
细节；不得再由 Jaco UI 或 Conversation View 消费。

### 3.3 `jaco-conversation`：Conversation 业务服务

新增 workspace crate `crates/jaco-conversation`，负责把 `jaco-db`、`jaco-agent` 和
`jaco-core` 组合成应用可直接使用的 Conversation 能力。它不依赖 GPUI，也不拥有 View。

候选公共边界：

```rust
pub trait ConversationService {
    fn load_catalog(&self) -> ConversationFuture<Vec<ConversationSummary>>;
    fn load(&self, id: ConversationId) -> ConversationFuture<Option<Conversation>>;
    fn create(&self, request: CreateConversation) -> ConversationFuture<CreatedConversation>;
    fn send(&self, request: SendConversationMessage)
        -> ConversationFuture<ConversationMutation>;
    fn pin(&self, id: ConversationId, pinned: bool)
        -> ConversationFuture<ConversationMutation>;
    fn delete(&self, id: ConversationId)
        -> ConversationFuture<ConversationMutation>;
}
```

准确的 Future alias、错误类型和 Agent Runtime 协作方式在实施时以现有 runtime trait 为准。
必须保证：

- 返回值是领域结果，不要求 app 再查询数据库拼装完整 Conversation；
- mutation 返回更新完整 Conversation 和 Catalog summary 所需的信息；
- Agent Runtime 的流式变化可以路由到对应 Conversation owner；
- app 不需要维护 database record 与 UI model 的双向同步。

### 3.4 Jaco `AppSession`：GPUI owner

`AppSessionData` 增加 Session-scoped Conversation owner：

```rust
pub struct AppSessionData {
    pub binding: DatabaseBinding,
    pub conversations: Entity<ConversationRegistry>,
}
```

`ConversationRegistry` 候选结构：

```rust
pub struct ConversationRegistry {
    service: Arc<dyn ConversationService>,
    catalog: Entity<ConversationCatalogModel>,
    conversations: HashMap<ConversationId, WeakEntity<ConversationModel>>,
    active_conversations: HashMap<ConversationId, Entity<ConversationModel>>,
    runtime_recovery: refresh::Operation<(), ConversationRuntimeProblem, Task<()>>,
}
```

职责：

- 为同一个 `ConversationId` 返回同一个存活的 `ConversationModel`；
- 默认只保存 `WeakEntity`，避免打开过的会话永久驻留；
- Agent Run 或其他 Session 工作正在使用 Conversation 时，用 `active_conversations` 保留强
  引用，结束后释放；
- 拥有 Catalog model 和 Conversation Runtime Recovery；
- 接收业务服务/Agent Runtime 的领域消息，发布到对应 Conversation model 和 Catalog；
- Session teardown 时取消所有 Session task，销毁 Catalog、Conversation model 和 runtime。

Registry 不实现 Global。现有全局 `AppSessionStore` 只负责定位当前 Session；消费者从
`AppSessionData` 取得 Registry handle。

### 3.5 Conversation 数据 Entity

```rust
pub struct ConversationModel {
    id: ConversationId,
    operation: refresh::Operation<Option<Conversation>, ConversationProblem, Task<()>>,
    registry: WeakEntity<ConversationRegistry>,
}
```

`Option<Conversation>` 只表达“读取成功但 Conversation 已不存在”；加载失败仍由 Problem
表达，不能转成 `None`。

Conversation model 负责：

- Load、Refresh、Retry 和 Task；
- 接收完成消息；
- 接收已提交的领域 `ConversationChange`；
- 对外提供当前 Operation；
- 在 command 入口验证精确 `Ready(Some(_))`；
- 把 create/send/pin/delete/approval 等用户命令交给 Conversation service；
- mutation 成功后更新完整 Conversation，并通知 Registry 更新 Catalog；
- 不保存 Window、Focus、ListState、Composer 或通知 UI。

### 3.6 Conversation Catalog

用 `ConversationCatalogModel` 取代 app-global `ConversationIndexStore`：

```rust
pub struct ConversationCatalogModel {
    operation:
        refresh::Operation<Vec<ConversationSummary>, ConversationCatalogProblem, Task<()>>,
}
```

它属于当前 Registry/Session，不是 app-global state。它提供：

- Session 初始化 Load；
- 用户显式 Refresh/Retry；
- mutation 成功后的领域消息更新；
- Home sidebar 和 Temporary empty-query 所需的领域 Summary；
- 精确 `Ready` 的 mutation capability。

Catalog 失败不影响已经打开并且自身仍有有效 Data 的 ConversationModel；某个 Conversation
详情失败也不把整个 Catalog 变成失败。

## 4. View 与组件边界

### 4.1 `ConversationDetailView`

主窗口和临时窗口分别创建自己的 View：

```rust
pub struct ConversationDetailView {
    conversation: Entity<ConversationModel>,
    composer: Entity<ChatInputController>,
    timeline: ListState,
    expanded_runs: HashSet<AgentRunId>,
    message_text_states: Vec<MessageTextState>,
    subscriptions: Vec<Subscription>,
}
```

View 只负责：

- 观察共享 ConversationModel；
- match `Operation<Option<Conversation>>`；
- 把 Conversation 投影为 timeline rows；
- 保存窗口交互状态；
- 根据精确状态设置按钮、Composer 和审批控件的 availability；
- 显示 Loading、Problem、Stale 和 Refresh/Retry UI。

View 不再拥有 Conversation Load Task，不维护 `pending_changes`，不接收数据库 record/delta，
不直接调用 repository。

### 4.2 两个窗口

`HomeView` 与 `TemporaryWindow` 可以继续各自缓存
`HashMap<ConversationId, Entity<ConversationDetailView>>`，该 Map 只复用本窗口内的 View。

创建 View 时先向当前 Session Registry 获取共享数据 Entity：

```rust
let conversation = registry.conversation(conversation_id, cx);
cx.new(|cx| ConversationDetailView::new(conversation, window, cx))
```

因此：

- 相同窗口内重复路由复用同一个 View；
- 不同窗口有不同 View；
- 两个 View 共享同一个 ConversationModel 和 Operation。

### 4.3 操作可用性

组件不通过多层 bool 参数镜像全部状态。直接拥有 ConversationModel 的 DetailView 在 render
时计算本层展示与 capability，并直接构造 Composer/按钮。

所有用户操作还必须在执行入口重新验证：

- Send、approval、pin、delete：Conversation 精确 `Ready(Some(_))`；
- New Conversation：Catalog、Provider、Project、Config、Database 和 runtime capability
  均满足该命令的依赖；
- Stop：只依赖当前存在 Active Run，不因 Conversation Refreshing 被禁用；
- Copy、展开、滚动等纯展示操作在有旧 Data 时可以继续。

## 5. 剩余高优先级问题

### H-01：删除 app-global Conversation Index

当前问题：

- `state/conversation_index.rs` 暴露数据库 index 概念；
- Home 把 `operation.data() == None` 当成空列表；
- UI 没有可靠的 Catalog Refresh/Retry；
- create/send/delete 依赖 Index Ready，但按钮没有统一表达该依赖。

实施结果：

- 删除 `ConversationIndexStore`、`ConversationIndexOperation` 和 app-level
  `ConversationIndexMessage`；
- Home 和 Temporary 消费 Session Catalog；
- Catalog 所有 phase 都有 UI；
- no-data Problem 与 Ready(empty) 明确区分；
- Catalog 有旧数据但失败时保留列表、显示错误和刷新按钮，mutation 控件禁用。

### H-02：Project 在 Home 中完整接入

保留现有 `ProjectStore`，但补齐 Home 消费：

- `HomeWorkspace`/Sidebar 订阅 Project Data 与 Status，而不是只拿 `Option<Vec<_>>`；
- Project Loading 不显示“没有项目”；
- Project Unavailable 显示错误和 Refresh；
- Project Degraded/Refreshing 保留旧列表并显示状态；
- add、select、pin、rename、remove、new-conversation-in-project 等控件仅在精确 Ready 时可用；
- command 入口继续重新验证精确 Ready。

设置页现有正确状态映射不重写。

### H-03：Runtime Recovery 进入 Session Conversation capability

当前 `ConversationRuntimeStore::recovery` 对用户不可见，`start_run` 失败只返回 `false`。

实施结果：

- Recovery 由 Session Conversation Registry/Service owner 持有；
- New Conversation 和 Detail Composer 能观察 recovery Operation；
- Loading/Unavailable/Degraded 显示原因和 Retry；
- recovery 非 Ready 时禁用会启动 Agent 的操作；
- 已经持久化消息但 Agent 未启动时返回明确的领域结果并显示错误，不能静默忽略 bool；
- Stop 和只读浏览继续遵守各自独立 capability。

### H-04：Skill Catalog 不再复制业务结果

ChatInput：

- 继续 page/controller-local 持有 `SkillCatalogOperation`；
- Composer 不保存第二份 `Vec<SkillEntry>` 作为事实源；
- completion rows 从当前 Operation Data 计算；仅保存选择索引、展开状态等交互状态；
- Scope 切换立即进入新 Operation，旧 Scope 数据不能冒充新 Scope 数据；
- Loading、Problem、Refresh/Retry 在输入组件附近可见；
- Skill 不可用不阻断普通不引用 Skill 的发送；引用不可用 Skill 的 submit 必须失败并给出明确
  错误。

Skills Settings：

- Loading/Refreshing 显示明确进度；
- Unavailable 显示错误和 Retry；
- Degraded 保留旧列表、显示错误和 Refresh；
- 正文仍由一次 Catalog 读取获得，不做二次详情查询。

## 6. 剩余中优先级问题

### M-01：Conversation Timeline 合并进 Conversation

- 删除 page-local `ConversationTimelineOperation`；
- 删除 View 中的 `pending_changes`；
- timeline rows 只从 `ConversationModel.operation.data()` 计算；
- Conversation Problem/Refresh UI 由整个 DetailView 表达；
- Degraded/Refreshing 时保留 timeline，但 Send、approval 等写操作禁用；
- Copy、展开和滚动继续可用。

### M-02：Home Conversation Search

- 空查询使用 Session Catalog Data，不复制另一份数据库 index；
- 非空查询继续使用 dialog-local refresh Operation，由 Conversation service 返回
  `Vec<ConversationSummary>`；
- Loading 显示 Loading，不显示“无结果”；
- Unavailable 显示 Problem 和 Retry；
- Degraded/Refreshing 保留旧结果并显示状态；
- 旧结果允许浏览，但会改变数据的入口禁用；
- Ready(empty) 才显示“无结果”。

### M-03：Temporary Search

- 空查询复用 Session Catalog 中的 scratch/no-project projection；
- 非空查询使用 window-local Operation；
- 保留现有 Loading、Problem、Retry 和 stale list；
- 明确区分纯导航与 mutation：打开旧 Conversation 可以继续，创建、删除、pin 等操作按对应
  Catalog/Conversation capability 禁用；
- 不再通过 ProjectOperation + ConversationIndexOperation 拼接 empty snapshot。

## 7. 实施工作包

### WP-01：建立领域类型与 crate 边界

涉及：

- `Cargo.toml`
- `crates/jaco-core/src/payloads/conversation.rs`
- `crates/jaco-db/src/records/conversations.rs`
- `crates/jaco-db/src/repository/conversations.rs`
- 新增 `crates/jaco-conversation/`

内容：

1. 定义 Conversation、ConversationSummary、领域 Change 和服务错误。
2. 将 DB 多表聚合映射收口到 jaco-db/jaco-conversation。
3. 建立不依赖 GPUI 的 Conversation service。
4. 为 load catalog、load detail 和 mutation 返回领域结果。

完成条件：

- app 不需要从多个 `jaco_db::*Record` 构造完整 Conversation；
- service API 不暴露 index/timeline/SQL 概念。

### WP-02：Session Registry 与共享 ConversationModel

涉及：

- `app/jaco/src/app/session.rs`
- `app/jaco/src/features/conversation.rs`
- 新增 `app/jaco/src/features/conversation/registry.rs`
- 新增 `app/jaco/src/features/conversation/model.rs`

内容：

1. Registry 随 AppSession 创建和销毁。
2. 建立 Catalog model。
3. 实现 `conversation(id)` 的 WeakEntity 复用。
4. 实现 Active Run 强引用保活。
5. 完成 Load/Refresh/Retry 和领域消息发布。

完成条件：

- 同一 Session 内同一 id 只有一个存活 ConversationModel；
- Session 切换后旧 Model 和 Task 不可继续发布；
- Operation Task 只有 Operation running variant 持有。

### WP-03：Runtime 与 mutation 路由

涉及：

- `app/jaco/src/features/conversation/runtime.rs`
- `crates/jaco-agent/src/runtime/*`
- `crates/jaco-agent/src/persistence/*`
- `crates/jaco-conversation/src/*`

内容：

1. Runtime 只发布领域 ConversationChange。
2. Registry 将变化路由到对应 ConversationModel 和 Catalog。
3. create/send/pin/delete/approval 返回明确结果，不返回被调用者忽略的 bool。
4. Recovery Operation 进入 Session capability。

完成条件：

- Runtime 不引用 app Conversation Index；
- View 不直接处理 DB record/delta；
- recovery 失败对用户可见且可 Retry。

### WP-04：迁移两个 Conversation Detail View

涉及：

- `app/jaco/src/components/chat/detail.rs`
- `app/jaco/src/features/home/shell.rs`
- `app/jaco/src/features/temporary.rs`

内容：

1. DetailView 改为消费共享 ConversationModel。
2. 移除本地 Load Task、Timeline Operation 和 pending changes。
3. 两个窗口分别缓存自己的 DetailView。
4. 完成所有 Operation phase UI 和按钮 capability。

完成条件：

- 同一 Conversation 同时打开时共享数据、独立窗口交互；
- 一个 View Refresh 后两个窗口同步；
- 任一窗口销毁不影响另一个窗口或正在运行的 Conversation。

### WP-05：迁移 Catalog、Home Workspace 与搜索

涉及：

- `app/jaco/src/state/conversation_index.rs`
- `app/jaco/src/features/home/workspace.rs`
- `app/jaco/src/features/home/sidebar.rs`
- `app/jaco/src/features/home/sidebar/search.rs`
- `app/jaco/src/features/temporary/search.rs`
- `app/jaco/src/features/temporary.rs`

内容：

1. Home/Temporary 改为 Session Catalog。
2. 完成 Loading/Unavailable/Degraded/Ready(empty) 映射。
3. 搜索改为服务返回领域 Summary。
4. 删除 Conversation Index app state。

完成条件：

- UI 不再导入 `ConversationIndexOperation`、`ConversationIndexDelta`；
- 加载失败不会显示为空列表；
- Catalog 可由用户 Refresh/Retry。

### WP-06：补齐 Project Home UI

涉及：

- `app/jaco/src/features/home/workspace.rs`
- `app/jaco/src/features/home/sidebar.rs`
- `app/jaco/src/features/home/sidebar/row.rs`
- `app/jaco/src/features/home/sidebar/menu.rs`
- `app/jaco/src/features/home/new_conversation.rs`

内容：

1. Home 观察 Project phase/problem。
2. 增加 Project status UI。
3. 所有 Project mutation 控件按 Ready 禁用。
4. 保留 command 入口验证。

### WP-07：修正 Skill Operation 消费

涉及：

- `app/jaco/src/components/chat/input.rs`
- `app/jaco/src/components/chat/composer_editor.rs`
- `app/jaco/src/features/settings/skills.rs`
- `app/jaco/src/features/settings/skills/rows.rs`

内容：

1. 移除 Composer 的 Skill 业务数据镜像。
2. completion 直接从当前 Operation Data 投影。
3. Settings 与 ChatInput 完成所有 phase UI。
4. Scope 切换和失败不再泄漏旧 Scope 数据。

### WP-08：删除旧抽象与更新文档

删除或收口：

- `ConversationIndexStore` / `ConversationIndexOperation`；
- app-level `ConversationIndexDelta` 消费；
- `ConversationTimelineOperation`；
- Detail View `pending_changes`；
- 两个窗口各自拥有的 Conversation 数据 owner；
- 从 `operation.data().unwrap_or_default()` 推导业务空状态的路径。

同步更新 issue #177 总计划中仍把旧实现描述为目标架构的段落，并明确历史实施与新架构的
替代关系。不要重写与本次 Conversation/剩余 UI 工作无关的 Config、Database、Provider、
Prompt 和 Shortcut 结论。

## 8. 测试与验证计划

实施阶段按工作包执行最小充分验证，不在每个工作包重复全量门禁。

必须覆盖的行为测试：

1. Registry 同 id 返回同一存活 ConversationModel。
2. Model 全部释放后 Registry 的 WeakEntity 可以重建。
3. Active Run 期间 Model 被保活，结束后释放。
4. Session binding 切换后旧 completion 不发布到新 Session。
5. Catalog 失败与单个 Conversation 失败互不污染。
6. 两个 DetailView 共享 Conversation 更新但保持独立 UI state。
7. Ready(empty)、Unavailable、Loading、Refreshing、Degraded 映射不同。
8. Degraded/Refreshing 下所有相关 mutation command 和控件禁用。
9. Skill Scope 加载失败不会继续使用上一个 Scope 的 entries。
10. Conversation/Project/Search 加载失败不显示为“空数据”。
11. Runtime recovery 失败会禁用启动 Agent 的操作，并显示 Retry。

建议验证命令：

```text
cargo fmt --all
cargo test -p jaco-core -p jaco-db -p jaco-conversation -p jaco-agent --locked
cargo test -p jaco --locked
cargo check -p jaco --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
git diff --check
```

可见 UI 仍由用户人工验证；除非用户另行要求，不在实施阶段增加 Computer Use 自动化。

## 9. 实施顺序与提交边界

建议顺序：

```text
WP-01 领域/服务边界
  -> WP-02 Session Registry/Model
     -> WP-03 Runtime 与 mutation
        -> WP-04 Detail View
        -> WP-05 Catalog/Home/Search
           -> WP-06 Project Home
           -> WP-07 Skill
              -> WP-08 清理与文档
```

阶段性提交建议：

1. Conversation domain/service crate；
2. Session Registry + Model；
3. Runtime/mutation 路由；
4. 两窗口 Detail View；
5. Catalog/Home/Search；
6. Project 与 Skill 剩余 UI；
7. 删除旧实现和文档同步。

迁移期间不保留长期兼容 API、双写 Store 或新旧 Conversation owner。允许在单个未提交工作包
内短暂存在编译中间态，但每个阶段性提交必须只有一条真实数据路径。
