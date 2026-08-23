# Jaco：实现 Issue #188 侧边栏动作、菜单与 publication

## Root hub and ownership

- Plan ID：`issue-188`
- Root hub：[Issue #188 root hub](../../../../../docs/dev/issue-188/README.md)
- Owner directory：`app/jaco`
- Owner plan：`app/jaco/docs/dev/issue-188/README.md`
- Owner index：[Jaco 开发计划](../README.md)
- Root-owned IDs consumed：`S-01`–`S-19`、`E-01`–`E-12`、`D-01`–`D-10`、`C-01`–`C-03`、`ERR-01`–`ERR-05`、`R-01`–`R-12`、`T-01`–`T-04`
- Owner-authored local IDs/ranges：`E/F/L/ST/R/T/WP-1xx`
- Assigned WPs：`WP-101`、`WP-102`、`WP-103`
- Owns：sidebar action sets、row/menu/dialog composition、mutation task adapter、registry/model/catalog/workspace publication、route/runtime cleanup、clipboard、typed icons、Fluent 与 Jaco tests/manual handoff
- Does not own：SQLite query/transaction、public service API、schema/migration、deep link/new window/worktree/archive management

## Owner-local evidence

| E-ID | Claim | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-101` | Project row 已有 Ellipsis dropdown 和 new-conversation shortcut，两个 child click 均 stop propagation | `src/features/home/sidebar/row.rs::ProjectSidebarRow::render` | 保留 layout/handler，新增 context menu 与 shared action set |
| `E-102` | Conversation row 使用 row open + hover pin/Trash，suffix width 为两个按钮设计 | `src/features/home/sidebar/row.rs::ConversationSidebarRow::render` | 保留两个按钮位置，把 Trash 改为 Archive；完整 popup 仅挂在右键 context menu，并保留 row open |
| `E-103` | pinned、project subtree、no-project 都调用同一个 `conversation_row`；pinned project 也调用 `project_row` | `src/features/home/sidebar.rs` render helpers | 在 row/action constructor 固定 location parity，不为 section 分叉 |
| `E-104` | registry `publish_summary` 同时更新 catalog 与 retained model；`publish_removed` 当前逐条通知 | `src/features/conversation/registry.rs` | rename 直接复用；batch 增加 `RemoveMany` |
| `E-105` | mutation driver 在 DB executor 上运行并由 resources retain；window completion 由 `retain_window` 持有 | `src/features/conversation.rs::spawn_conversation_mutation`、sidebar handlers | 继续现有 task ownership |
| `E-106` | 当前 open conversation 删除成功后 route 回 NewConversation，feature 关闭 runtime sessions | `HomeWorkspace::delete_conversation`、`features::conversation::delete_conversation` | rename 为 archive，并扩展 batch result path |
| `E-107` | project rename dialog 已使用 `InputState`、defer focus、trim/nonempty guard、async close/error | `src/features/home/sidebar/menu.rs::open_rename_project_dialog` | conversation rename 复用同一交互契约 |
| `E-108` | `copy_to_clipboard` 已验证 GPUI clipboard readback | `src/components/chat/detail.rs::copy_to_clipboard` | sidebar local helper采用同一行为 |
| `E-109` | `DestructiveAction` 只有 Delete，确认按钮固定 danger | `src/components/delete_confirm.rs` | 增加 Archive label；project remove 的 Delete 不变 |
| `E-110` | `IconName` 已有 Pin/PinOff/Pencil/Copy/Ellipsis/FolderOpen/FolderMinus/SquarePen，缺 Archive | `src/foundation/assets.rs` | 只新增 app-local Archive variant |

## Owner-local decisions

| D-ID | Decision | Evidence | Consequence |
| --- | --- | --- | --- |
| `D-101` | 新建 `sidebar/actions.rs` 持有 typed action kinds、availability、target guard 与唯一 invoke handlers；`menu.rs` 只负责 PopupMenu/dialog/notification composition | `E-101`–`E-103` | 避免右键、project overflow、conversation direct buttons 和 shortcut callback drift |
| `D-102` | 不注册行级 global actions/keybindings；project overflow 保持组件键盘路径，conversation pin/archive 直接按钮保持 focus-visible；conversation 完整菜单仅右键显示 | root `D-03`、用户最新决定、锁定 component API | 保留 `features/home/actions.rs` 现有 global shortcuts |
| `D-103` | copy path 从 current workspace project projection 解析并在执行前再次 guard；不存在时 ERR-03 | `E-103`、root `D-06` | normal/pinned/scratch 统一，不使用 process cwd |
| `D-104` | registry 新增 `RemoveMany`，single archive 也走批量 helper；catalog 一次 retain，每个 live model 仍收 existing `ConversationChange::Deleted` | `E-104` | 不修改 jaco-core event enum，避免 N 次 catalog rebuild |
| `D-105` | mutation closure 统一调用 C-01/C-02，并在 existing `SessionDatabaseExecutor` 边界把透明 `ConversationError::Database` 解包回 `DbError`；existing pin 同步走 `ConversationService::set_pinned` | `E-105`、root `D-09` | app 不直接选择 repository mutation，也不改变 executor error contract |
| `D-106` | rename 每次打开创建独立 `InputState`/dialog closure，关闭即释放 | `E-107` | 无 mirrored business state、无 retained form task |

## Owner-local target design

### File and ownership tree

```text
app/jaco/
├── src/
│   ├── components/
│   │   ├── chat/detail.rs                        # F-115 [Modify, handwritten] authoritative persisted project ID
│   │   └── delete_confirm.rs                     # F-101 [Modify, handwritten] Archive confirm label
│   ├── foundation/assets.rs                      # F-102 [Modify, handwritten] app-local Lucide Archive
│   └── features/
│       ├── conversation.rs                       # F-103 [Modify, handwritten] service-backed mutations
│       ├── conversation/registry.rs              # F-104 [Modify, handwritten] summary + RemoveMany publication
│       ├── conversation/runtime.rs               # F-114 [Modify, handwritten] generation-keyed archive fences
│       └── home/
│           ├── sidebar.rs                        # F-105 [Modify, handwritten] actions module + section tests
│           ├── sidebar/actions.rs                # F-106 [Add, handwritten] shared action/availability/invoke/clipboard
│           ├── sidebar/menu.rs                   # F-107 [Modify, handwritten] popup builders/dialogs/notifications
│           ├── sidebar/row.rs                    # F-108 [Modify, handwritten] context + project overflow + conversation direct actions
│           └── workspace.rs                      # F-109 [Modify, handwritten] queries, commands, route adapter
├── locales/en-US/main.ftl                        # F-110 [Modify, handwritten] runtime Fluent source
├── locales/zh-CN/main.ftl                        # F-111 [Modify, handwritten] locale parity
└── docs/dev/
    ├── README.md                                 # F-112 [Modify, handwritten] owner index
    └── issue-188/README.md                       # F-113 [Add, handwritten] this plan
```

No production path is moved/deleted. No runtime SVG、bundle asset、manifest、schema、migration、generated file or lockfile is added.

### Owner-local contracts

#### L-101：Typed action kinds

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectSidebarAction {
    NewConversation,
    TogglePinned,
    Rename,
    RevealInFileManager,
    ArchiveConversations,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConversationSidebarAction {
    TogglePinned,
    Rename,
    Archive,
    CopyWorkingDirectory,
}
```

- Menu order is the enum order above, with separators before project Archive/Remove and before conversation CopyWorkingDirectory.
- Toggle label/icon derives from target snapshot `pinned`; action identity remains stable.
- No section/location enum exists. Every row location constructs the same target action set.

#### L-102：Availability snapshots and action sets

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProjectActionAvailability {
    pub(super) project_mutations: bool,
    pub(super) new_conversation: bool,
    pub(super) archive_conversations: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ConversationActionAvailability {
    pub(super) conversation_mutations: bool,
    pub(super) copy_working_directory: bool,
}

#[derive(Clone)]
pub(super) struct ProjectSidebarActions {
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    availability: ProjectActionAvailability,
}

#[derive(Clone)]
pub(super) struct ConversationSidebarActions {
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    working_directory: Option<PathBuf>,
    availability: ConversationActionAvailability,
}
```

```rust
impl ProjectSidebarActions {
    pub(super) fn new(
        project: SidebarProjectHeader,
        workspace: Entity<HomeWorkspace>,
        cx: &App,
    ) -> Self;

    pub(super) fn availability(&self, action: ProjectSidebarAction) -> bool;
    pub(super) fn invoke(
        &self,
        action: ProjectSidebarAction,
        window: &mut Window,
        cx: &mut App,
    );
}

impl ConversationSidebarActions {
    pub(super) fn new(
        conversation: SidebarConversationNode,
        workspace: Entity<HomeWorkspace>,
        cx: &App,
    ) -> Self;

    pub(super) fn availability(&self, action: ConversationSidebarAction) -> bool;
    pub(super) fn invoke(
        &self,
        action: ConversationSidebarAction,
        window: &mut Window,
        cx: &mut App,
    );
}
```

Availability：

- Project pin/rename/remove：project resource Ready and target exists。
- Project new conversation：project + conversation resources Ready and target exists。
- Project archive chats：both resources Ready、target exists、active count > 0；项目无 active conversation 时菜单项仍显示但 disabled。若 action set 创建后数量降为 0，执行层允许 C-02 返回 empty success no-op。
- Project reveal：保留 existing enabled behavior；系统处理 filesystem target 缺失。
- Conversation pin/rename/archive：conversation resource Ready and current catalog contains target。
- Copy working directory：project resource Ready、conversation exists、`project_path(project_id).is_some()`。
- `invoke` immediately rechecks resource/target；stale-open menu maps to ERR-02/ERR-03。

#### L-103：Popup builders

```rust
pub(super) fn project_popup_menu(
    menu: PopupMenu,
    actions: ProjectSidebarActions,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu;

pub(super) fn conversation_popup_menu(
    menu: PopupMenu,
    actions: ConversationSidebarActions,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu;
```

- Project `ContextMenuExt::context_menu` and `Button::dropdown_menu` share this callback shape. Conversation mounts the same popup builder only on `ContextMenuExt`；its pin/archive buttons call the same `ConversationSidebarActions::invoke` directly.
- Every item reads label/icon/disabled from `L-101/L-102` and calls only `actions.invoke(...)`.
- Project SquarePen shortcut calls `invoke(NewConversation, ...)`。
- PopupMenu owns arrow/Enter/Space/Escape navigation and dismissal；no custom menu focus state。

#### L-104：Workspace queries and adapters

```rust
impl HomeWorkspace {
    pub(crate) fn contains_project(&self, project_id: &ProjectId) -> bool;
    pub(crate) fn contains_conversation(&self, conversation_id: &ConversationId, cx: &App) -> bool;
    pub(crate) fn project_path(&self, project_id: &ProjectId) -> Option<PathBuf>;
    pub(crate) fn active_conversation_count(&self, project_id: &ProjectId, cx: &App) -> usize;

    pub(crate) fn rename_conversation(
        &mut self,
        conversation_id: ConversationId,
        title: String,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ConversationSummary>>;

    pub(crate) fn archive_conversation(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<ConversationSummary>>;

    pub(crate) fn archive_project_conversations(
        &mut self,
        project_id: ProjectId,
        cx: &mut Context<Self>,
    ) -> Task<jaco_db::Result<Vec<ConversationSummary>>>;
}
```

- Queries read existing project selection/catalog; no second map/cache。
- Rename does not mutate route。
- Single archive resets to `NewConversation` only after successful returned ID match。
- Batch uses returned ID set；empty/failure leaves route unchanged。

#### L-105：Execution-time guard

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum SidebarActionGuardError {
    #[error("sidebar resource is not ready: {resource:?}")]
    ResourceNotReady { resource: SidebarResource },
    #[error("sidebar target disappeared: {target:?}")]
    TargetDisappeared { target: SidebarTarget },
    #[error("clipboard verification failed")]
    ClipboardVerificationFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SidebarResource { Projects, Conversations }

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SidebarTarget {
    Project(ProjectId),
    Conversation(ConversationId),
}
```

The guard is ephemeral app interaction state. UI maps it to root ERR-02/ERR-03/ERR-05；tracing may include IDs but never path/title。

#### L-106：Feature mutation boundary

```rust
pub(crate) fn rename_conversation(
    conversation_id: ConversationId,
    title: String,
    cx: &mut App,
) -> Task<jaco_db::Result<ConversationSummary>>;

pub(crate) fn archive_conversation(
    conversation_id: ConversationId,
    cx: &mut App,
) -> Task<jaco_db::Result<ConversationSummary>>;

pub(crate) fn archive_project_conversations(
    project_id: ProjectId,
    cx: &mut App,
) -> Task<jaco_db::Result<Vec<ConversationSummary>>>;

fn spawn_conversation_mutation<R>(
    cx: &mut App,
    command: impl FnOnce(&FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    publish: impl FnOnce(&R, &mut App) + Send + 'static,
) -> Task<jaco_db::Result<R>>
where
    R: Send + 'static;
```

- All conversation mutations, including existing pin, instantiate `ConversationService` inside the DB executor closure，then map its sole `ConversationError::Database(error)` variant back to `error` because `SessionDatabaseExecutor::execute` remains `jaco_db::Result`-typed。
- Rename publishes full summary once。
- Single/batch archive call `publish_removed_many` once and close sessions only for returned IDs；commit precedes publication。
- Driver remains retained by conversation resources；window close may cancel notification receiver but not committed mutation/publication。

#### L-107：Registry batch publication

```rust
pub(crate) enum ConversationCatalogMessage {
    Upsert(Box<ConversationSummary>),
    RemoveMany(Vec<ConversationId>),
}

impl ConversationRegistry {
    pub(crate) fn publish_removed_many(
        &mut self,
        ids: Vec<ConversationId>,
        cx: &mut Context<Self>,
    );
}

pub(crate) fn publish_removed_many(ids: Vec<ConversationId>, cx: &mut impl AppContext);
```

- Build one ID `HashSet`、retain/sort/notify catalog once、apply existing Deleted change to each live model、release active retention。
- Single archive passes one ID；empty is a no-notify no-op；running refresh is canceled before committed transition。

#### L-108：Dialogs and Archive confirmation

```rust
pub(super) fn open_rename_conversation_dialog(
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
);

pub(super) fn open_archive_conversation_confirm(
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
);

pub(super) fn open_archive_project_conversations_confirm(
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
);

pub(crate) enum DestructiveAction { Delete, Archive }
```

- Rename mirrors project dialog：current title、localized placeholder、defer focus、trim、empty ignored、retained save、success close、failure stays open。
- Archive confirmations use `button-archive`；project remove still uses Delete。
- ERR-01 has distinct single/batch warnings；ERR-04 uses operation title + generic safe detail。

#### L-109：Verified clipboard helper

```rust
fn copy_working_directory(
    path: &Path,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), SidebarActionGuardError>;
```

- Copy `path.display().to_string()` without canonicalization/Git-root substitution。
- Write plain text，read back and compare exact value；mismatch returns ERR-05 and safe notification。
- No Task、cache、retry、file read、path logging or persistence。

### Boundary implementations

| Root contract/error | App implementation |
| --- | --- |
| `C-01` | `L-104/L-106` call service and publish full summary through existing Upsert path |
| `C-02` | `L-104/L-106/L-107` call archive service、publish committed IDs、close sessions、reset successful active route |
| `C-03` | `L-102/L-109` resolve current project path and perform verified explicit clipboard write |
| `ERR-01` | Match the executor result `DbError::ConversationHasActiveRun { .. }` after the transparent service adapter；operation-specific warning；no route/publication |
| `ERR-02/ERR-03` | `L-102/L-105` disabled-state + execution-time guard；safe notification；no command |
| `ERR-04` | trace typed error and show localized generic detail；never render `error.to_string()` |
| `ERR-05` | `L-109` failure notification；no path in log/message |

### GPUI application contracts

#### ST-101：Persisted summary and projections

- **Authority:** SQLite `conversations` row through C-01/C-02.
- **Initialization and lifetime:** catalog Operation loads active summaries；retained `ConversationModel` is created on open and registry keeps weak/active handles.
- **Readers:** `ConversationCatalogModel`、`ConversationModel`、`HomeWorkspace::snapshot`、sidebar rows/search.
- **Mutation:** service-backed `L-106` commands only in this scope.
- **Publication and projections:** rename -> Upsert -> catalog + model SummaryChanged；archive -> RemoveMany -> catalog removal + model Deleted -> workspace observer rebuilds every location.
- **Persistence boundary:** C-01/C-02 and DB owner DB-301/DB-302.
- **Reset and cancellation:** committed delta cancels a running catalog refresh；failure publishes nothing.

#### ST-102：Project batch archive command

- **Authority:** persisted DB transaction；the returned Vec is the committed delta.
- **Initialization and lifetime:** created after confirmation；DB driver retained by conversation resources、UI completion retained by window.
- **Readers:** registry、runtime、workspace route completion.
- **Mutation:** `L-106::archive_project_conversations` only.
- **Publication and projections:** committed Vec -> one RemoveMany + per-ID session close + route ID membership check.
- **Persistence boundary:** C-02/DB-302.
- **Reset and cancellation:** window close only cancels UI completion；ERR-01/ERR-04 produces no partial app change.

#### ST-103：Ephemeral row action set

- **Authority:** immutable `L-102` value created per row render.
- **Initialization and lifetime:** clones live until row rerender/menu dismissal.
- **Readers:** context builder、project overflow/new shortcut、conversation pin/archive direct buttons.
- **Mutation:** none；`invoke` rechecks current workspace.
- **Publication and projections:** none；it invokes owner commands.
- **Persistence boundary:** none.
- **Reset and cancellation:** dropped on dismissal；stale availability cannot bypass execution guard.

#### ST-104：Conversation rename editor

- **Authority:** per-dialog `Entity<InputState>` for draft/focus/IME/selection.
- **Initialization and lifetime:** current title at open；drop on close.
- **Readers:** submit closure reads and trims value.
- **Mutation:** native Input only.
- **Publication and projections:** successful C-01 result updates business summary.
- **Persistence boundary:** C-01；draft is not persisted.
- **Reset and cancellation:** Cancel/Escape drops draft；failed save keeps dialog open；success closes.

#### ST-105：Window/resource tasks

- **Authority:** driver Task in conversation resources；UI completion Task in window retention.
- **Initialization and lifetime:** one pair per async mutation.
- **Readers:** DB executor/result receiver and dialog/notification completion.
- **Mutation:** no mirrored loading flag；menu activation is one-shot.
- **Publication and projections:** driver publishes after commit even if invoking window closes.
- **Persistence boundary:** C-01/C-02.
- **Reset and cancellation:** receiver drop does not cancel retained driver；shutdown follows existing resource cancellation.

#### ST-106：Home route and runtime sessions

- **Authority:** existing `HomeWorkspace.route` and conversation runtime session registry.
- **Initialization and lifetime:** existing Home entity/application runtime.
- **Readers:** Home shell/detail and runtime.
- **Mutation:** only successful archive result changes route/closes sessions in this scope.
- **Publication and projections:** `cx.notify()` only when active route was removed.
- **Persistence boundary:** none；archive already committed.
- **Reset and cancellation:** rename/failure/empty batch leave route unchanged；active archive selects `NewConversation` with no pending project.

#### ST-107：Clipboard transfer

- **Authority:** OS clipboard after explicit C-03 action.
- **Initialization and lifetime:** one synchronous write/readback；app retains no copy.
- **Readers:** immediate verification and external paste target.
- **Mutation:** `L-109` only.
- **Publication and projections:** success/failure notification only.
- **Persistence boundary:** none.
- **Reset and cancellation:** later OS clipboard write supersedes it；no retry.

#### Data-source and Operation decision

- Existing catalog `refresh::Operation` remains unchanged.
- Rename/archive remain save/delete-style one-shot commands in workspace/feature/service Task ownership；no new operation phase.
- `InputState` owns physical editor behavior；DB summary owns the value after save；no generated Form.

#### Interaction flows

Menu flow (`R-01`–`R-03`)：

1. Row creates one project/conversation action set with stable target ID.
2. Project row `.context_menu(...)` and Ellipsis `.dropdown_menu(...)` clone its action set into `L-103`；conversation row mounts `L-103` only through `.context_menu(...)`.
3. Project overflow/new shortcut and conversation pin/archive direct buttons call `cx.stop_propagation()` and the same `invoke` path；right-click is consumed by `ContextMenuExt` and never enters the left-click row handler.
4. PopupMenu owns focus/navigation；direct buttons use Button focus-visible behavior；every activation calls exactly one `invoke`.
5. Left-click outside child controls continues project toggle/conversation open.

Rename flow (`R-04`)：

```mermaid
sequenceDiagram
    participant M as PopupMenu
    participant D as Dialog/InputState
    participant W as HomeWorkspace
    participant F as feature Task
    participant S as ConversationService C-01
    participant R as Registry/catalog/model
    M->>D: open current title; focus input
    D->>D: trim; reject empty
    D->>W: rename_conversation
    W->>F: retained command
    F->>S: rename
    alt success
        S-->>F: persisted summary
        F->>R: publish_summary once
        R-->>D: projections notify
        D->>D: close
    else ERR-04
        S-->>D: error
        D->>D: stay open; safe notification + trace
    end
```

Archive flow follows the root sequence. Individual/project confirmation share Archive button semantics；only project command may return empty Vec；no app publication occurs before `Ok`.

### Component, layout, identity and accessibility

- Stable IDs remain `sidebar-project-row-{project_id}` / `sidebar-conversation-row-{conversation_id}`；conversation direct controls use `sidebar-conversation-pin-{id}` and `sidebar-conversation-archive-{id}`；project trigger IDs remain unchanged.
- `ContextMenuExt` keys state from stable row ElementId；project dropdown keys from its stable trigger ElementId；no list index identity.
- Project and conversation suffixes each reserve two buttons (`56px` width、`64px` hover padding) while titles keep flex/truncation.
- Conversation retains Pin/PinOff and replaces Trash with Archive. Both controls use localized labels as tooltips，shared availability、focus-visible reveal and `cx.stop_propagation()`；conversation has no Ellipsis/dropdown.
- Project Ellipsis retains Tab/Enter/Space/arrow/Escape behavior；conversation direct buttons are keyboard focusable，while the full conversation menu follows the user-required right-click-only entry.
- Rename input gets initial focus；existing Dialog controls own Tab/Enter/Escape/Cancel behavior；empty value starts no Task.
- Use existing theme/component variants；no custom popover、focus trap、low-level Element or window.

### Icons and assets

| UI role | Exact typed icon/Lucide slug | Owner/F-ID | Placement | Fallback | R/T IDs |
| --- | --- | --- | --- | --- | --- |
| Archive conversation/project chats | `IconName::Archive => "archive"` | `F-102` | app-local typed Lucide；vendored slug exists | None；build fails if unavailable | `R-09`、`T-112` |
| Copy working directory | existing `IconName::Copy` | `F-106/F-107` | app-local typed icon | None | `R-07`、`T-109` |
| Project overflow | existing `IconName::Ellipsis` | `F-108` | app-local typed icon | None | `R-01`、`T-103` |
| Conversation direct pin/archive | existing `Pin/PinOff` + new `Archive` | `F-102/F-108` | app-local typed icons | None | `R-01`–`R-03`、`T-103` |
| Rename/new/reveal/remove | existing `Pencil/SquarePen/FolderOpen/FolderMinus` | `F-106/F-107` | app-local typed icons | None | `R-09`–`R-10` |

No runtime/bundle asset or generated output changes.

### Fluent i18n

Every row updates both `F-110/F-111`；missing-key fallback is not accepted.

| Key | Meaning/variables | Caller/UI state | R/T IDs |
| --- | --- | --- | --- |
| `button-archive` | Archive / 归档 | `DestructiveAction::Archive` | `R-05`–`R-06`、`T-112` |
| `sidebar-project-new-conversation` | project menu item | `L-103` | `R-01`–`R-02` |
| `sidebar-project-archive-conversations` | Archive Chats / 归档聊天 | project menu item | `R-01`、`R-06` |
| `sidebar-project-archive-conversations-title` | batch confirmation title | `L-108` | `R-06` |
| `sidebar-project-archive-conversations-message` | confirmation with `{ $name }` | `L-108` | `R-06` |
| `sidebar-project-archive-conversations-failed` | batch persistence failure title | ERR-04 | `R-08` |
| `sidebar-project-archive-conversations-running-title` | blocked batch title | ERR-01 | `R-06` |
| `sidebar-project-archive-conversations-running-message` | all-or-nothing blocked message | ERR-01 | `R-06` |
| `sidebar-conversation-pin` / `sidebar-conversation-unpin` | toggle menu labels and direct-button tooltips | target pin state | `R-01`–`R-02` |
| `sidebar-conversation-pin-failed` | pin failure title | ERR-04 | `R-10` |
| `sidebar-conversation-rename` | rename menu item | `L-103` | `R-01`、`R-04` |
| `sidebar-rename-conversation-title` | rename dialog title | `L-108` | `R-04` |
| `sidebar-rename-conversation-placeholder` | input placeholder | InputState | `R-04` |
| `sidebar-rename-conversation-failed` | rename failure title | ERR-04 | `R-04`、`R-08` |
| `sidebar-conversation-archive` | archive menu item and direct-button tooltip | `L-103` / row button | `R-01`、`R-05` |
| `sidebar-archive-conversation-title` | single confirmation title | `L-108` | `R-05` |
| `sidebar-archive-conversation-message` | confirmation with `{ $title }` | `L-108` | `R-05` |
| `sidebar-archive-conversation-failed` | single archive failure title | ERR-04 | `R-05`、`R-08` |
| `sidebar-archive-conversation-running-title` | blocked single title | ERR-01 | `R-05` |
| `sidebar-archive-conversation-running-message` | stop-running guidance | ERR-01 | `R-05` |
| `sidebar-conversation-copy-working-directory` | copy menu item | `L-103` | `R-07` |
| `sidebar-action-resource-unavailable-title` / `-message` | stale resource rejection | ERR-02 | `R-08` |
| `sidebar-action-target-unavailable-title` / `-message` | disappeared target rejection | ERR-03 | `R-08` |
| `sidebar-action-failed-message` | generic safe persistence detail | ERR-04 | `R-08`、`R-12` |
| existing `conversation-copy-success` / `conversation-copy-failed` / `conversation-copy-failed-message` | clipboard result | C-03/ERR-05 | `R-07` |
| existing `conversation-missing-subtitle` | wording changes to archived/removed | missing conversation UI | `R-05`、`R-09` |

Delete `sidebar-conversation-delete-tooltip` and all `sidebar-delete-conversation-*` keys after callers are replaced；existing project keys stay unchanged.

### Security, diagnostics and platform

- Reveal keeps `show_project_label_key()` and `cx.open_with_system(&project.path)` platform behavior.
- Clipboard is explicit local exposure；do not canonicalize/read/send/persist path or put path/title in diagnostics.
- Mutation failure logs `action`、`target_kind`、stable ID and typed error once；ERR-01 may use warning. Raw error text never enters UI.
- No bundle、entitlement、deep-link、window、workflow、dependency or lockfile change.

## Owner-local work packages

### WP-101：Service-backed mutations and publication convergence

**Owner**

`app/jaco` conversation feature/registry/workspace。

**Prerequisites and contracts**

- Root `D-02`、`D-04`–`D-05`、`C-01`–`C-02`、`ERR-01`–`ERR-04`；`WP-201` complete.

**File IDs**

- `F-103`、`F-104`、`F-109`、`F-114`–`F-115`

**Implementation sequence**

1. Convert mutation driver and pin to `ConversationService`；add `L-106` rename/single/batch commands.
2. Implement `L-107` RemoveMany and transition tests.
3. Add `L-104` workspace queries/adapters；route reset is successful-result-ID based.
4. Close sessions only for committed summaries；remove delete-named app functions/callers.

**Failure and lifecycle behavior**

- ERR-01/ERR-04 returns without registry、route or runtime change；driver remains resource-retained.

**Tests**

| R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-04` | `T-104` `conversation/registry.rs` | `publish_summary_updates_catalog_and_retained_model_title` | catalog/model share renamed title |
| `R-04` | `T-105` `workspace.rs` | `renamed_summary_rebuilds_pinned_project_and_no_project_projections` | same title in every projection |
| `R-05` | `T-106` `conversation.rs` | `archive_conversation_publishes_remove_closes_session_and_resets_active_route` | success-only convergence |
| `R-06` | `T-107` `conversation/registry.rs` | `remove_many_updates_catalog_once_and_deletes_live_models` | all removed、active handles released |
| `R-06`、`R-08` | `T-108` `workspace.rs` | `failed_or_empty_project_archive_does_not_change_route` | no partial app state |

**Focused validation**

```sh
cargo fmt
cargo test -p jaco conversation_registry
cargo test -p jaco sidebar_archive
git diff --check
```

**Done condition**

C-01/C-02 converge through one publication path；no `delete_conversation` app symbol or direct conversation repository mutation remains.

### WP-102：Shared actions, menus, dialogs, clipboard, icon and Fluent

**Owner**

`app/jaco` sidebar/components/foundation/locales。

**Prerequisites and contracts**

- Root `D-01`–`D-03`、`D-06`–`D-10`、`C-03`、`ERR-01`–`ERR-05`；`WP-101` complete.

**File IDs**

- `F-101`–`F-111`

**Implementation sequence**

1. Add `L-101/L-102/L-105` and pure availability/guard tests.
2. Convert menu to `L-103/L-108`，reuse existing project actions and add project new/archive + all conversation actions.
3. Wire project ContextMenuExt + overflow and conversation ContextMenuExt + pin/archive direct buttons；project shortcut and conversation controls call the same invoke；remove the old Trash semantics and conversation dropdown.
4. Add C-03 `L-109`、Archive icon/action、both locale changes and delete-key cleanup.
5. Replace raw error UI with root ERR mapping and safe tracing.

**Failure and lifecycle behavior**

- Disabled items cannot invoke；stale menus are guarded；clipboard failure changes no business state.

**Tests**

| R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-01`–`R-02`、`R-08` | `T-101` `sidebar/actions.rs` | `project_action_availability_is_identical_for_pinned_and_tree_rows` | same order/disabled state |
| `R-01`–`R-02`、`R-07` | `T-102` `sidebar/actions.rs` | `conversation_action_availability_is_location_independent` | normal/pinned/scratch parity |
| `R-01`–`R-03` | `T-103` `sidebar/row.rs` | `conversation_row_exposes_pin_and_archive_as_direct_actions` + row wiring audit | direct action order、stable IDs、shared invoke、propagation guard；conversation popup only on ContextMenuExt |
| `R-07` | `T-109` `sidebar/actions.rs` (`#[gpui::test]`) | `copy_working_directory_writes_and_verifies_exact_project_path` | exact text；mismatch ERR-05 |
| `R-08` | `T-110` `sidebar/actions.rs` | `stale_menu_rejects_unready_resource_or_missing_target` | no command、exact guard |
| `R-08`、`R-12` | `T-111` `sidebar/menu.rs` | `sidebar_error_mapping_uses_safe_localized_details` | no raw DB/path/title detail |
| `R-09`–`R-10` | `T-112` confirm/i18n/icon tests | `archive_confirm_and_locales_use_archive_semantics` | locale parity；existing Delete unchanged |

**Focused validation**

```sh
cargo fmt
cargo test -p jaco sidebar_context_menu
cargo test -p jaco destructive_archive
cargo test -p jaco i18n
cargo check -p jaco
git diff --check
```

**Done condition**

One action source powers project dual menu entrypoints and conversation context/direct controls；delete-facing UI/key/icon remnants and conversation dropdown are gone；no excluded action appears.

### WP-103：Focused UI regression and manual handoff

**Owner**

`app/jaco`。

**Prerequisites and contracts**

- `WP-101`、`WP-102` complete；root `R-01`–`R-12`.

**File IDs**

- Tests colocated in `F-101`、`F-104`–`F-109`；no snapshot file required.

**Implementation sequence**

1. Run owner focused tests once after final code state.
2. Launch with disposable `JACO_CONFIG_DIR`；create normal project、scratch/no-project conversation、pinned duplicate and two project conversations.
3. Execute root `T-01`–`T-03`，including running-run atomic rejection and restart persistence.
4. Record exact results；handoff workspace/CI to root `WP-001`.

**Failure and lifecycle behavior**

- A failed manual scenario keeps root status `In progress` and records the exact step/environment.

**Tests**

| R-ID | T-ID | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-01`–`R-03` | root `T-01` | project right-click/overflow、conversation right-click/direct buttons/keyboard focus、row click | conversation 无 Ellipsis；no accidental toggle/open/double action |
| `R-04`–`R-07` | root `T-02/T-03` | rename/archive/copy/restart | projection/persistence/clipboard exact |
| `R-08`–`R-12` | root `T-01/T-04` | disabled/disappeared/error/excluded/log audit | safe behavior、bounded scope |

**Focused validation**

```sh
cargo fmt --check
cargo test -p jaco
cargo check -p jaco
git diff --check
```

Manual launch：`JACO_CONFIG_DIR=<temporary-directory> cargo run -p jaco`。

**Done condition**

Owner automation and root manual matrix have current-state evidence；aggregate gates are left to root `WP-001`.

## Focused validation and handoff

| Local R-ID | T-ID/evidence | Expected result |
| --- | --- | --- |
| `R-101` action definition parity | `T-101`–`T-103` | pinned/tree/no-project share kinds/order/handlers |
| `R-102` rename projection | `T-104`–`T-105` | catalog/model/all row projections converge |
| `R-103` archive lifecycle | `T-106`–`T-108` | committed IDs only、sessions/route correct、errors no partial |
| `R-104` clipboard privacy | `T-109` + root `T-03/T-04` | exact text、no retained/logged path |
| `R-105` safe stale/error behavior | `T-110`–`T-111` | localized rejection、full detail only in tracing |
| `R-106` icon/i18n/confirm parity | `T-112` | en-US/zh-CN and Delete/Archive contracts pass |

## Implementation evidence（2026-08-23）

- `sidebar/actions.rs` now owns the typed project/conversation action order、availability、execution-time guards and exact clipboard write/readback. `menu.rs` builds popup surfaces and dialogs from those actions；`row.rs` keeps project right-click/Ellipsis，while conversation exposes focus-visible pin/archive direct buttons and mounts the full popup only on right-click.
- `conversation.rs`、`registry.rs` and `workspace.rs` implement service-backed rename/single archive/project archive、single `RemoveMany` publication、retained model updates、session cleanup and committed-ID route reset. `runtime.rs` adds generation-keyed conversation/project archive fences；submission admission and archive admission are mutually exclusive for the same scope，and persisted running rows remain the DB transaction's second gate.
- `detail.rs` now takes the persisted conversation summary's authoritative project ID when building follow-up requests. `delete_confirm.rs`、`assets.rs` and both Fluent locales expose archive semantics and the typed Archive icon；the project remove action keeps destructive Delete semantics.
- Automated evidence after the direct-button revision：`cargo test -p jaco` 528 pass/2 ignored；`cargo check -p jaco` pass；`cargo clippy -p jaco --all-targets --all-features -- -D warnings` pass. Existing full-workspace build/test/clippy evidence remains current for unchanged lower layers. Focused tests cover conversation direct-action order、stable action order、copy availability independence、exact clipboard readback、localized pin/archive labels、archive icon parity、rename projections、batch removal and archive-fence admission/stale-ticket behavior.
- Packaged-app evidence：`cargo run -p xtask -- bundle jaco` succeeded. An isolated signed copy with disposable config reached no AX window after five minutes；sample showed existing startup skill-file-watch setup synchronously scanning `/Users/sushao` before first-window creation. Root `T-01`–`T-03` therefore remain unverified，and aggregate status stays `In progress`.
- Product scope has no deviation. The runtime fence is an implementation refinement required to close the discovered `Submitting`→persisted-running-row race；independent review found no P0/P1 correctness issue. Direct driver-error/session-close lifecycle tests remain a lower-priority test gap covered indirectly by full app tests and the runtime/DB focused suites.

Aggregate completion、manual matrix and remote CI remain in the root hub.
