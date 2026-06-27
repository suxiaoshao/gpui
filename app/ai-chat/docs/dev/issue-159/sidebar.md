# Issue #159 ai-chat2 Sidebar 专项计划

本文档是 `app/ai-chat2` project-first sidebar 的实施计划。父级清单仍是
`app/ai-chat/docs/dev/issue-159/README.md`；本文档只固定 sidebar、project/conversation
navigation、搜索入口和右侧 conversation 路由的具体实现方案。

创建时间：2026-06-03。

当前状态：已实现。实现包含 fresh DB sidebar 状态列、repository API、`ai-chat2` project catalog
event、Home workspace store、project-first sidebar、conversation search dialog、项目菜单、hover action、
右侧 conversation route、统一 shortcut action row、i18n 和 app-local Lucide 图标。

## 目标行为

- 顶部入口只包含“新对话”和“搜索”：
  - “新对话”进入 New Conversation 页面，并聚焦 composer。
  - “搜索”弹出 conversation search dialog。
  - 快捷键使用 GPUI `secondary-n` / `secondary-f`；GPUI 的 `secondary` 在 macOS 是 Command，在 Windows/Linux 是 Ctrl。
- 底部“设置”入口使用同一套 sidebar shortcut action row：
  - 点击后 dispatch `ToggleSettings`。
  - 快捷键使用 GPUI `secondary-,`。
  - hover 时右侧显示 `Kbd` shortcut badge；非 hover 时不预留普通布局宽度。
- 主体顺序：
  - 置顶：置顶对话和置顶项目。
  - 项目：所有未移除的 normal projects。
  - 无项目对话：scratch/no-project conversations。
- 项目行为：
  - 点击项目行展开/关闭它下面的对话。
  - hover 项目行显示两个操作按钮：更多、新对话。
  - 项目“新对话”按钮必须有 tooltip。
  - 更多菜单包含：置顶/取消置顶、在 Finder/资源管理器/文件管理器中显示、重命名、移除。
- 对话行为：
  - 点击对话后，右侧打开该 conversation。
  - hover 对话行显示置顶/取消置顶、删除两个按钮。
  - 置顶/取消置顶和删除按钮都必须有 tooltip。
- 删除语义：
  - 项目移除只从 app 侧边栏隐藏，不删除磁盘目录，不删除项目下 conversations。
  - 对话删除使用 soft delete，不物理删除数据库行。

## 模块结构

在 `app/ai-chat2/src/features/home` 下保持当前 `sidebar.rs` 为入口文件，并增加 sidebar 子模块目录：

```text
app/ai-chat2/src/features/home/
├── actions.rs
├── conversation.rs
├── sidebar.rs
└── sidebar/
    ├── menu.rs
    ├── row.rs
    └── search.rs
```

- `actions.rs`
  - 定义 `OpenNewConversation` 和 `OpenConversationSearch`。
  - 在 `home::init(cx)` 中注册 `secondary-n`、`secondary-f`，key context 使用现有 `AiChat2Home`。
- `conversation.rs`
  - 提供右侧打开已有 conversation 的第一版页面壳。
  - 第一版只需要读取 conversation title/status 和已有 items 概览；完整 timeline 渲染后续继续在 #159 中推进。
- `sidebar.rs`
  - 保留 `HomeSidebar` 入口和 `gpui_component::sidebar::Sidebar` 外层结构。
  - 负责读取 `AiChat2WorkspaceStore` snapshot，组织顶部入口、置顶区、项目区、无项目对话区和底部 Settings action row。
- `sidebar/row.rs`
  - 定义 `ShortcutSidebarActionRow`、`ProjectSidebarRow`、`ConversationSidebarRow`。
  - 行组件只处理渲染、hover action、shortcut badge 和点击事件转发，不直接访问数据库。
- `sidebar/menu.rs`
  - 定义项目更多菜单、重命名 dialog、移除确认入口。
  - 复用现有 `components/delete_confirm.rs` 做 destructive confirm。
- `sidebar/search.rs`
  - 定义 `ConversationSearchView`。
  - 使用 `Dialog` + `Input` + `ListState/List` 实现搜索弹窗。

新增全局状态模块：

```text
app/ai-chat2/src/state/workspace.rs
```

同时扩展现有项目状态模块：

```text
app/ai-chat2/src/state/projects.rs
```

`state/mod.rs` 或等价入口中注册这些模块。`HomeView`、`HomeSidebar` 和 search dialog 通过
`AiChat2WorkspaceStore` 共享当前路由和 sidebar 数据；Settings Projects 页和 Sidebar 项目操作通过
`ProjectCatalogStore` 共享项目变更事件。

## 自定义类型

`state/workspace.rs` 中新增：

```rust
pub enum HomeRoute {
    NewConversation,
    Conversation(ConversationId),
}

pub struct AiChat2WorkspaceStore {
    route: HomeRoute,
    snapshot: SidebarSnapshot,
    expanded_project_ids: HashSet<ProjectId>,
    project_catalog: Entity<ProjectCatalogStore>,
}

pub struct SidebarSnapshot {
    pub pinned: Vec<SidebarPinnedEntry>,
    pub projects: Vec<SidebarProjectNode>,
    pub no_project_conversations: Vec<SidebarConversationNode>,
}

pub enum SidebarPinnedEntry {
    Conversation(SidebarConversationNode),
    Project(SidebarProjectHeader),
}

pub struct SidebarProjectNode {
    pub project: SidebarProjectHeader,
    pub is_expanded: bool,
    pub conversations: Vec<SidebarConversationNode>,
}

pub struct SidebarProjectHeader {
    pub id: ProjectId,
    pub path: PathBuf,
    pub display_name: SharedString,
    pub updated_at: DateTime<Utc>,
    pub pinned: bool,
}

pub struct SidebarConversationNode {
    pub id: ConversationId,
    pub project_id: ProjectId,
    pub title: SharedString,
    pub updated_at: DateTime<Utc>,
    pub pinned: bool,
}
```

`state/projects.rs` 中把现有 helper 扩展为 project catalog entity：

```rust
pub struct ProjectCatalogGlobal {
    pub store: Entity<ProjectCatalogStore>,
}

pub struct ProjectCatalogStore {
    revision: u64,
}

pub enum ProjectCatalogEvent {
    Changed(ProjectCatalogChange),
}

pub enum ProjectCatalogChange {
    Added { project_id: ProjectId },
    Renamed { project_id: ProjectId },
    Removed { project_id: ProjectId },
    PinChanged { project_id: ProjectId, pinned: bool },
}
```

- `ProjectCatalogGlobal` 实现 `Global`，只保存 `Entity<ProjectCatalogStore>` handle。
- `ProjectCatalogStore` 实现 `EventEmitter<ProjectCatalogEvent>`。
- `ProjectCatalogStore` 是 project mutation 的 app 层入口；Settings 和 Sidebar 都不能绕过它直接写 project DB。
- 每次 project mutation 成功后递增 `revision`，`cx.emit(ProjectCatalogEvent::Changed(...))`，并 `cx.notify()`。

`AiChat2WorkspaceStore` 提供这些方法：

- `reload_sidebar(cx)`
- `open_new_conversation(cx)`
- `open_conversation(conversation_id, cx)`
- `toggle_project(project_id, cx)`
- `new_conversation_in_project(project_id, cx)`
- `pin_project(project_id, pinned, cx)`
- `pin_conversation(conversation_id, pinned, cx)`
- `rename_project(project_id, display_name, cx)`
- `remove_project(project_id, cx)`
- `delete_conversation(conversation_id, cx)`
- `search_conversations(query, limit, cx)`

Project 相关方法委托给 `ProjectCatalogStore`；conversation 相关方法由 `AiChat2WorkspaceStore`
调用 repository。UI 层只调用这些 state 方法；数据库访问集中在 `state/projects.rs`、
`state/workspace.rs` 和 `ai-chat-db` repository。

## 数据库和 Repository

`crates/ai-chat-core/src/payloads.rs`：

- `ProjectMetadata` 增加：
  - `pinned: bool`
  - `removed: bool`
- 两个字段必须使用 serde default 兼容旧 metadata JSON。
- `empty_project_metadata()` 默认 `pinned = false`、`removed = false`。
- `ConversationMetadata.pinned` 已存在，继续使用。

`crates/ai-chat-db` repository 新增或扩展：

- `list_sidebar_projects()`
  - 返回 `ProjectKind::Normal` 且 `metadata.removed == false` 的 projects。
- `list_sidebar_conversations()`
  - 返回 `status == Active` 的 conversations。
  - 包含 normal project conversations 和 scratch/no-project conversations。
  - 不返回 removed project 下的 conversations。
- `update_project_metadata(project_id, metadata)`
- `rename_project(project_id, display_name)`
- `set_project_removed(project_id, removed)`
- `update_conversation_metadata(conversation_id, metadata)`
- `soft_delete_conversation(conversation_id)`
  - 设置 `status = Deleted` 和 `deleted_at`。
- `search_sidebar_conversations(query, limit)`
  - 只搜索 active conversations。
  - 不返回 removed project 下的结果。
  - 搜索范围包含 title、project display/path 和 `conversation_items.search_text`。

本轮不做 SQL schema migration；新增 project 状态放在 existing `projects.metadata_json` 中。

## 数据流

启动和刷新：

1. app 初始化时创建 `ProjectCatalogStore` entity，并写入 `ProjectCatalogGlobal`。
2. `HomeView` 创建 `AiChat2WorkspaceStore` entity，并传入 `ProjectCatalogStore`。
3. `AiChat2WorkspaceStore` 订阅 `ProjectCatalogEvent::Changed`。
4. `HomeView` observe `AiChat2WorkspaceStore`；store notify 后 Home 和 Sidebar 重新渲染。
5. `HomeSidebar` render 时读取 `SidebarSnapshot`。
6. `reload_sidebar()` 从 repository 拉取 projects/conversations，并按展开状态组装 snapshot。

跨窗口 project 同步：

- Settings Projects 页添加项目时，调用 `ProjectCatalogStore::insert_existing_folder_project()`。
- Sidebar 项目菜单中的置顶、重命名、移除，也调用同一个 `ProjectCatalogStore`。
- `ProjectCatalogStore` 写 DB 成功后发 `ProjectCatalogEvent::Changed`。
- `AiChat2WorkspaceStore` 收到事件后调用 `reload_sidebar()`，Home 窗口立即刷新项目区、置顶区和无项目区。
- Settings Projects 页也订阅 `ProjectCatalogEvent::Changed`；当 Sidebar 重命名/移除项目后，Settings 列表同步刷新。
- `state::projects::normal_projects()` 必须过滤 `ProjectMetadata.removed == false`，避免 Settings 继续显示已从 Sidebar 移除的项目。
- 不允许新增“Settings 写 DB 后只刷新 Settings 自己”或“Sidebar 写 DB 后只刷新 Sidebar 自己”的路径。

交互流：

- 点击“新对话”或触发 `secondary-n`：
  - `AiChat2WorkspaceStore::open_new_conversation()`
  - `route = HomeRoute::NewConversation`
  - `HomeView` 右侧渲染现有 `NewConversationPage`
  - 调用现有 focus path 聚焦 composer。
- 点击项目行：
  - `toggle_project(project_id)`
  - 更新 `expanded_project_ids`
  - 重新组装 snapshot。
- 点击项目 hover 的“新对话”：
  - `new_conversation_in_project(project_id)`
  - 跳转 New Conversation 页面，并把目标 project 写入该页当前 project selection。
- 点击对话：
  - `open_conversation(conversation_id)`
  - `route = HomeRoute::Conversation(conversation_id)`
  - `HomeView` 右侧渲染 `ConversationPage`。
- 置顶/取消置顶：
  - project 置顶走 `ProjectCatalogStore`，conversation 置顶走 `AiChat2WorkspaceStore`。
  - project 事件会触发 Settings 和 Home 同步刷新；conversation 事件只需要刷新 Home sidebar。
- 移除项目：
  - 弹 destructive confirm。
  - 确认后通过 `ProjectCatalogStore` 设置 `ProjectMetadata.removed = true`。
  - 如果当前 route 属于该项目，切回 `HomeRoute::NewConversation`。
  - `ProjectCatalogEvent::Changed` 触发 Home 和 Settings 同步刷新。
- 删除对话：
  - 弹 destructive confirm。
  - 确认后 `soft_delete_conversation()`。
  - 如果当前打开该对话，切回 `HomeRoute::NewConversation`。
  - `reload_sidebar()`。

排序：

- 置顶区：置顶 conversations 按 `updated_at desc`，置顶 projects 按 `updated_at desc`；conversations 显示在 projects 前。
- 项目区：normal projects 按 `display_name` case-insensitive 升序；同名时按 path。
- 项目内 conversations：按 `updated_at desc`。
- 无项目 conversations：按 `updated_at desc`。

## UI 组件和样式

使用的 `gpui-component` 组件：

- `Sidebar`：侧边栏外层。
- `Button`：hover action、dialog action。
- `Kbd`：新对话、搜索、设置 shortcut badge。
- `Tooltip` / `tooltip_with_action`：hover action。
- `DropdownMenu` 或现有 popup menu API：项目更多菜单。
- `Dialog`：搜索、重命名、确认。
- `Input`：搜索输入、重命名输入。
- `ListState/List`：搜索结果。
- `Scrollable`：sidebar 主体滚动。
- `Label` / theme token：分组标题和 muted metadata。

侧边栏行规则：

- 行高稳定，hover 不改变布局高度。
- 项目和对话 hover action 使用 overlay，hover 前不占普通文字布局宽度。
- 顶部和底部 shortcut action row 使用 overlay `Kbd` badge，hover 前不占普通文字布局宽度。
- 项目和对话 title 使用单行截断。
- 对话右侧可显示相对时间，但不能挤压 action 按钮。
- 选中 conversation/project row 使用 theme muted/selected surface，不使用高饱和强调色。

## 图标

`app/ai-chat2/src/foundation/assets.rs` 需要新增缺失的 app-local Lucide `IconName`：

- `SquarePen` -> `square-pen.svg`
- `Pin` -> `pin.svg`
- `PinOff` -> `pin-off.svg`
- `Ellipsis` -> `ellipsis.svg`
- `Pencil` -> `pencil.svg`
- `MessageSquare` -> `message-square.svg`
- `ChevronRight` -> `chevron-right.svg`
- `ExternalLink` -> `external-link.svg`
- `FolderMinus` -> `folder-minus.svg`

已存在图标继续复用：

- `Search`
- `Settings`
- `Folder`
- `FolderOpen`
- `FolderX`
- `ChevronDown`
- `Trash`

不新增图标依赖，不在 feature 代码中直接引用 SVG path。

## i18n

更新：

- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`

新增 key 分组：

- 顶部入口：
  - `sidebar-new-conversation`
  - `sidebar-search`
- section：
  - `sidebar-section-pinned`
  - `sidebar-section-projects`
  - `sidebar-section-no-project-conversations`
  - `sidebar-empty-projects`
  - `sidebar-empty-conversations`
- tooltip：
  - `sidebar-project-new-conversation-tooltip`
  - `sidebar-project-more-tooltip`
  - `sidebar-conversation-pin-tooltip`
  - `sidebar-conversation-unpin-tooltip`
  - `sidebar-conversation-delete-tooltip`
- 项目菜单：
  - `sidebar-project-pin`
  - `sidebar-project-unpin`
  - `sidebar-project-show-in-finder`
  - `sidebar-project-show-in-explorer`
  - `sidebar-project-show-in-file-manager`
  - `sidebar-project-rename`
  - `sidebar-project-remove`
- 搜索：
  - `sidebar-search-title`
  - `sidebar-search-placeholder`
  - `sidebar-search-no-results`
- dialog：
  - `sidebar-rename-project-title`
  - `sidebar-rename-project-placeholder`
  - `sidebar-remove-project-title`
  - `sidebar-remove-project-message`
  - `sidebar-delete-conversation-title`
  - `sidebar-delete-conversation-message`
- notification/error：
  - `sidebar-open-project-failed`
  - `sidebar-rename-project-failed`
  - `sidebar-remove-project-failed`
  - `sidebar-delete-conversation-failed`

Finder/Explorer/File Manager 文案由 platform-specific label 选择；不要在 Windows/Linux 显示 Finder。

## 依赖

本轮不新增第三方依赖。

- 打开目录复用 GPUI `cx.open_with_system(path)`。
- 搜索不引入 FTS 或外部搜索库。
- 图标继续使用 repo-vendored Lucide assets。

## 测试计划

文档落地后：

- `git diff --check`

代码实现后：

- `cargo fmt`
- `cargo test -p ai-chat-core`
- `cargo test -p ai-chat-db`
- `cargo test -p ai-chat2`
- `cargo check -p ai-chat2`
- `git diff --check`

新增测试覆盖：

- `ProjectMetadata` 旧 JSON 缺少 `pinned` / `removed` 时默认解码为 `false`。
- normal project pin/remove 后 sidebar 查询结果正确。
- conversation pin/delete 后 sidebar 查询结果正确。
- search 不返回 deleted conversation。
- search 不返回 removed project 下的 conversation。
- scratch/no-project conversation 出现在“无项目对话”。
- `ProjectCatalogStore` mutation 成功后递增 revision 并发出 `ProjectCatalogEvent::Changed`。
- Settings 添加项目后，Home sidebar store 收到 project changed event 并重新加载 snapshot。
- Sidebar 重命名/移除项目后，Settings Projects 页收到 project changed event 并重新加载列表。

手动验证：

- `secondary-n` 打开 New Conversation 页面并聚焦 composer；macOS 显示 `⌘N`，Windows/Linux 显示 `Ctrl+N`。
- `secondary-f` 打开搜索 dialog；macOS 显示 `⌘F`，Windows/Linux 显示 `Ctrl+F`。
- `secondary-,` 打开或切换 Settings；macOS 显示 `⌘,`，Windows/Linux 显示 `Ctrl+,`。
- 打开 Settings 和 Home 两个窗口，在 Settings 添加项目后，Home sidebar 无需重启即可显示新项目。
- 在 Home sidebar 重命名或移除项目后，Settings Projects 列表同步更新。
- hover 项目显示更多和新对话按钮，且新对话按钮 tooltip 正常。
- 项目更多菜单包含置顶、显示目录、重命名、移除。
- hover 对话显示置顶/删除按钮，且两个 tooltip 正常。
- 点击项目展开/关闭。
- 点击对话后右侧打开该 conversation。
- macOS 显示“在 Finder 中显示”；Windows/Linux 使用资源管理器或文件管理器文案。

## 非目标

- 不实现完整 timeline 渲染。
- 不接真实 agent run/send/cancel/retry。
- 不做 prompt selector、attachments、tool approval、usage summary。
- 不做项目目录物理删除。
- 不做 legacy `app/ai-chat` 数据迁移。
- 不新增全文搜索索引或 FTS migration。

## 实现记录

2026-06-03 已完成第一版实现：

- `ProjectMetadata` 增加 `pinned` / `removed`，使用 serde default 兼容旧 JSON。
- `ai-chat-db` repository 增加 sidebar project/conversation 查询、project metadata 更新、重命名、移除、conversation metadata 更新、soft delete 和 search。
- `app/ai-chat2` 增加 `ProjectCatalogStore`，Settings Projects、New Conversation 和 Home Sidebar 共享 project mutation event。
- Home root 使用 `AiChat2WorkspaceStore` 驱动 route，右侧可在 New Conversation 和 Conversation 页面壳之间切换。
- Sidebar 已渲染顶部新对话/搜索、置顶、项目展开、无项目对话、hover action、项目更多菜单和 conversation search dialog。
- 项目“在 Finder/资源管理器/文件管理器中显示”复用 GPUI `open_with_system`，不新增 opener 依赖。

2026-06-04 补充完成 Sidebar 组件对齐和 shortcut action row polish：

- 置顶、项目、无项目对话 section 使用 `SidebarGroup::new(name)`；移除手写 section/header 结构。
- 展开项目使用与 `gpui-component::sidebar::SidebarMenuItem::children(...)` 对齐的 submenu 容器样式：左边线、左外边距、左内边距和 `sidebar_border` token。
- 项目行只保留 chevron 表示展开状态，不再同时使用 folder-open icon 表示同一状态。
- 顶部“新对话/搜索”和底部“设置”统一使用 `ShortcutSidebarActionRow`，样式对齐 `SidebarMenuItem`，hover 时显示 `Kbd` shortcut badge。
- 快捷键改为 GPUI `secondary` 语义：`secondary-n`、`secondary-f`、`secondary-,`，避免 Windows/Linux 上显示或触发平台 `Win` 键。

2026-06-08 补充完成 sidebar 状态列化：

- `projects.pinned`、`projects.removed`、`conversations.pinned` 成为 fresh schema columns；`ProjectMetadata`
  和 `ConversationMetadata` 不再保存这些 sidebar UI 状态。
- `ai-chat-db` repository 的 project pin/remove、conversation pin 和 sidebar 可见性查询改为直接读写 SQL
  columns，避免列表热路径依赖 metadata JSON。
- `app/ai-chat2` 的 ProjectCatalogStore、WorkspaceStore 和 sidebar/header node 构建改为读取
  `ProjectRecord.pinned` / `ProjectRecord.removed` / `ConversationRecord.pinned`。
- fresh DB 仍处于 pre-main 开发阶段，本轮按 baseline schema 清理处理，不为旧开发期 fresh DB 追加兼容
  migration。

已执行验证：

- `cargo fmt`
- `cargo check -p ai-chat2`
- `cargo test -p ai-chat-core`
- `cargo test -p ai-chat-db`
- `cargo test -p ai-chat2`
- `cargo test -p ai-chat-agent`
- `git diff --check`
