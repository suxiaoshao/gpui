# Issue #188：Jaco 侧边栏项目与对话上下文菜单

## 状态与范围

- 状态：`In progress`（本地实现与自动化门已完成；人工 UI 与远端 CI 待验证）
- 关联 issue：[#188](https://github.com/suxiaoshao/gpui/issues/188)
- 父 issue：[#159](https://github.com/suxiaoshao/gpui/issues/159)
- Plan ID：`issue-188`
- 根计划：`docs/dev/issue-188/README.md`
- 根索引：[Workspace development plans](../README.md)
- 分支：`codex/188-jaco-sidebar-context-menus`
- 受影响 owner：`app/jaco`、`crates/jaco-conversation`、`crates/jaco-db`
- 发布门：无外部 release gate；实现完成后执行本计划的 focused、workspace、人工 UI 与远端 CI 门
- 最近证据刷新：2026-08-23
- 实施引用：commit `6cd0cb2`；PR [#208](https://github.com/suxiaoshao/gpui/pull/208)

### 高影响变更摘要

| 审计门 | 结果 | Canonical IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | `[Cross-owner]` 在既有三个 owner 内补齐 DB、service、Jaco publication/UI 链路，不新增 crate | `D-04`、`C-01`–`C-02`、`WP-101`、`WP-201`、`WP-301` |
| Public or cross-owner contracts | `[Modify] [Breaking]` `jaco-conversation` 增加 rename/batch archive，并以 archive 名称取代未被使用的 delete service API | `D-02`、`D-09`、`C-01`–`C-02`、`WP-201` |
| Global/shared authority | `[Modify]` SQLite 继续持有 conversation authority，现有 registry/catalog/model/workspace 投影接收 rename 和批量 remove publication | `D-04`、`R-04`、`WP-101` |
| Persistence, data, configuration, or credentials | `[Modify]` 更新既有 `conversations.title`，并原子软删除项目中的 active conversations；schema、migration 与配置不变 | `D-02`、`D-05`、`C-01`–`C-02`、`WP-301` |
| Runtime, concurrency, performance, or shutdown | `[Modify]` runtime archive fence 与提交准入互斥，DB transaction 再检查 persisted running run；成功后批量 publication、关闭 runtime sessions 并修正当前 route | `D-05`、`ERR-01`、`R-05`–`R-06`、`WP-101`、`WP-301` |
| Security, privacy, or external access | `[Modify]` 仅在用户显式选择时把本地项目路径写入系统剪贴板，禁止日志记录路径内容 | `D-06`、`C-03`、`ERR-05`、`R-07`、`R-12` |
| Dependencies, toolchains, generated, or vendored artifacts | `None` | `S-11`、`S-17` |
| Platform, packaging, CI, or release | `None`；复用现有 `open_with_system`、GPUI clipboard 与三平台 CI，不改 bundle/workflow | `S-16`、`R-10` |
| User-visible compatibility, defaults, or removals | `[Modify]` UI 中“删除会话”统一改为“归档会话”；conversation 行右侧保留置顶/归档按钮，完整上下文菜单仅由右键打开；不提供永久删除或恢复入口 | `D-01`–`D-03`、`D-10`、`R-01`–`R-03`、`R-05` |

### 目标

让侧边栏中的项目行和对话行都具备一套可复用动作定义。项目继续从右键菜单和 Ellipsis 调用；conversation 右侧置顶/归档按钮调用同一 action set，完整上下文菜单仅由右键打开。补齐对话重命名、项目批量归档聊天、复制对话工作目录，以及 rename/archive 成功后的持久化、publication、当前会话模型、所有侧边栏投影和 runtime 清理。

### 非目标

- 标记为未读、分享、复制深度链接、在新窗口中打开。
- 创建永久 worktree 或任何 worktree 生命周期管理。
- 归档管理页、归档列表、取消归档、恢复或永久删除。
- 启用 `ConversationStatus::Archived`、写入 `archived_at`、新增 migration、schema 字段或历史数据回填。
- 编辑项目路径、Git 配置、provider/model 或其他项目配置；项目“编辑”仍只修改显示名称。
- 在项目菜单增加“复制工作目录”；本计划只为 conversation 动作提供该项。
- 为行级动作增加全局快捷键、deep-link scheme、第二个 Home window 或新的 `Store`/`Operation`/`gpui-form`。

### 用户已确认决定

- Jaco 当前“删除会话”的 soft-delete 行为在产品语义中就是“归档”；菜单只保留一个归档动作。
- conversation 菜单包含置顶/取消置顶、重命名、归档、复制工作目录。
- conversation 行右侧保留置顶/取消置顶和归档两个直接按钮；conversation 上下文菜单只在右键时显示，不提供 Ellipsis 菜单入口。
- project 菜单包含新建对话、置顶/取消置顶、重命名项目名称、在系统文件管理器显示、归档聊天、移除项目。
- project 批量归档遇到任一 running conversation 时整批拒绝，不能出现部分归档。
- ChatGPT/Codex 截图仅作为动作组织和交互参考；其中未列入本计划的能力继续排除。

### 兼容与迁移策略

- 既有数据库继续使用 `status = 'deleted'` 与 `deleted_at` 表示 UI“归档”；现有记录无需迁移、回填或重建。
- `ConversationStatus::Archived`、`archived_at`、`ConversationChange::Deleted` 与 `FreshRepository::soft_delete_conversation` 保持现状；产品文案和 service/app 命名采用 archive。
- `ConversationService::delete` 当前没有 workspace 调用方。实现时直接改名为 `archive`，不保留第二个兼容别名，避免继续暴露两套产品语义。
- 项目已有 pin/rename/reveal/remove 行为、确认框和持久化不改；新动作必须复用同一 handler/availability 结构，防止既有入口回归。
- 不新增依赖、Cargo feature、`Cargo.lock`、bundle、entitlement 或 workflow 变更。

### 计划映射

| Scope | 文档 | Owns | Assigned IDs/WPs |
| --- | --- | --- | --- |
| Root hub | 本文档 | 状态、范围、用户决定、S/C/ERR、跨 owner 顺序、aggregate validation/completion | `E-01`–`E-12`、`D-01`–`D-10`、`F-001`–`F-002`、`C-01`–`C-03`、`ERR-01`–`ERR-05`、`R-01`–`R-12`、`T-01`–`T-04`、`WP-001` |
| `app/jaco` | [owner plan](../../../app/jaco/docs/dev/issue-188/README.md) | action sets、菜单/行、dialogs、tasks、publication、route/runtime、clipboard、icons、Fluent 与 UI tests | `F/L/ST/R/T/WP-1xx` |
| `crates/jaco-conversation` | [owner plan](../../../crates/jaco-conversation/docs/dev/issue-188/README.md) | rename/archive service API 与 DB error 透明传播 | `F/L/R/T/WP-2xx` |
| `crates/jaco-db` | [owner plan](../../../crates/jaco-db/docs/dev/issue-188/README.md) | title update、单 transaction project batch soft-delete 与 DB tests | `F/L/DB/R/T/WP-3xx` |

## Applicability

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定或 negative reason | Owning section/WP |
| --- | --- | --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Applicable | app 直接持有 UI/publication，service 是 DB 薄边界，repository 持有 SQLite writes | 保持三个既有 owner，按同一 Plan ID 建 owner plans | `D-04`、`C-01`–`C-02`、`WP-101/201/301` |
| `S-02` | GPUI components, layout, interaction, and accessibility | Applicable | 项目已有 `DropdownMenu`；conversation 只有 hover buttons | 项目复用 `ContextMenuExt` + `DropdownMenu`；conversation 使用 `ContextMenuExt` + 两个直接 `Button`；其余复用 `PopupMenu`、`Dialog`、`Input`、`Notification` | `D-03`、`D-10`、`WP-102` |
| `S-03` | Entity, Store, Global, identity, and projections | Applicable | SQLite summary 经 registry catalog、retained `ConversationModel`、`HomeWorkspace` snapshot 投影 | 不新增 authority；rename 用 `publish_summary`，archive 用单次 `RemoveMany` publication | `D-04`、`R-04`、`WP-101` |
| `S-04` | Actions, events, subscriptions, focus, and windows | Applicable | child buttons 已采用 stop propagation；rename project dialog 已 defer focus | project 双菜单入口、conversation 右键菜单与直接按钮消费同一 action set；按钮保持 focus-visible，dialog 保持现有 focus/task 生命周期 | `D-03`、`R-02`–`R-03`、`WP-102` |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Applicable | conversation command driver 由 resources retain，window completion 由 `retain_window` 持有 | 继续相同 ownership；batch DB commit 后一次 publication，window 关闭不取消已提交 mutation | `D-05`、`ERR-01`、`WP-101/301` |
| `S-06` | Data acquisition and Operation state | No change | catalog 已使用 `refresh::Operation`；pin/delete 是 one-shot command Task | rename/archive 仍是 controller/service command，不引入新 Operation 或 phase | `D-08`、`WP-101` |
| `S-07` | Forms and editable state | Applicable | project rename 使用 native `InputState` + `Input` | conversation rename 复用该 ownership；trim 后非空才提交，不接入 `gpui-form` | `D-08`、`WP-102` |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | `jaco-conversation` 包装 `FreshRepository`；GPUI 提供 clipboard/system-open | 固定 C-01–C-03，不增加 provider/Rig/MCP contract | `C-01`–`C-03`、`WP-101/201/301` |
| `S-09` | Error identity, propagation, recovery, and error UI | Applicable | active run 有 typed DB error；其他 sidebar failure 当前显示原始错误字符串 | 固定 ERR-01–ERR-05；UI 只显示本地化安全文案，完整错误写 tracing | `ERR-01`–`ERR-05`、`WP-101/102/201/301` |
| `S-10` | Database, persistence, and migrations | Applicable | `conversations` 已有 title/status/deleted_at；单删已有 immediate transaction | 新增 rename query 与原子 project batch command；无 schema/migration | `D-02`、`D-05`、`WP-301` |
| `S-11` | Generated, synchronized, copied, or vendored content | No change | 新 archive Lucide slug 已存在于 `third_party/lucide/icons/archive.svg`；无需生成或复制文件 | 只扩展 app-local typed enum | `S-12`、`WP-102` |
| `S-12` | Icons and assets | Applicable | Jaco 通过 `define_lucide_icons!` 持有 app-local `IconName` | 增加 `Archive => "archive"`，其余复用现有 typed icons | `D-01`、`WP-102` |
| `S-13` | Fluent i18n and bundle localization | Applicable | runtime 文案位于 en-US/zh-CN `main.ftl`，当前 delete keys 使用产品错误语义 | 增加/替换 sidebar archive/rename/copy/error keys，保持两 locale parity；bundle strings 不变 | `D-01`、`ERR-01`–`ERR-05`、`WP-102` |
| `S-14` | Security, privacy, and credentials | Applicable | project path 是本地 filesystem 路径 | 只在显式 copy 时写系统 clipboard；不发送网络、不写日志、不持久化额外副本 | `C-03`、`R-07`、`R-12`、`WP-102` |
| `S-15` | Observability and diagnostics | Applicable | 部分现有 sidebar error 直接进 UI，conversation pin 只写 tracing | 每个 failed mutation 记录 action/target kind/ID 与 error；路径内容和用户标题不进入日志 | `D-07`、`ERR-04`–`ERR-05`、`WP-101/102` |
| `S-16` | Packaging, platform behavior, and CI/release | No change | reveal label 已按 macOS/Windows/other 分支，clipboard 为 GPUI platform API | 不改 bundle/manifest/workflow；三平台 CI 作为 aggregate gate | `R-10`、`WP-001` |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | No change | 锁定 gpui-component 已提供 context/dropdown menu；Lucide archive 已 vendored | 不改 manifests 或 lockfile | `D-03`、`WP-102` |
| `S-18` | Owner documentation, indexes, and ADRs | Applicable | repo 使用 root hub + same-ID owner plans + 各级 index | 新增四份 plan 并更新四个 index；无需 ADR | `WP-001` |
| `S-19` | Validation and completion evidence | Applicable | Issue 要求 DB/feature/UI/人工验证，AGENTS 定义 workspace gates | 执行 owner focused checks、manual matrix、workspace build/test/clippy 与远端三平台 CI | `R-01`–`R-12`、`T-01`–`T-04`、`WP-001` |

## Evidence

### 当前流程

1. `app/jaco/src/features/home/sidebar/row.rs::ProjectSidebarRow::render` 为项目提供 Ellipsis dropdown 和独立 new-conversation shortcut；整行 click 只展开/折叠。
2. `app/jaco/src/features/home/sidebar/menu.rs::project_popup_menu` 内联构造 pin、reveal、rename、remove handlers；project row 尚未挂 `ContextMenuExt`。
3. `ConversationSidebarRow::render` 的整行 click 打开 conversation，hover suffix 直接提供 pin 和 Trash；没有 overflow/context menu，也没有 rename/copy。
4. `HomeWorkspace::{pin_conversation,delete_conversation}` 调用 `features::conversation` command；成功后 catalog publication 重建 pinned/project/no-project projections。
5. `features::conversation::delete_conversation` 直接调用 `FreshRepository::soft_delete_conversation`，成功后 `registry::publish_removed`、关闭 runtime sessions；workspace 在当前 conversation 被删除时切回 `NewConversation`。
6. `FreshRepository::soft_delete_conversation` 在 `immediate_transaction` 中拒绝 running run，然后写 `status = deleted`、`deleted_at`、`updated_at`。
7. `ConversationRegistry::publish_summary` 同时更新 catalog 和已加载 `ConversationModel`；现有链路可承载 rename，无需修改 `jaco-core` change types。
8. `SidebarProjectHeader.path` 来源于 `projects.path`；`features::conversation::build_run_request` 使用相同 project path 作为 agent working directory。

### Evidence registry

| E-ID | Classification | Claim | Evidence | Plan consequence |
| --- | --- | --- | --- | --- |
| `E-01` | Current fact | Issue #188 要求统一 project/conversation context menu、shared action definitions、explicit overflow、rename publication 与 location parity | `gh issue view 188 --repo suxiaoshao/gpui`，2026-08-23 | `R-01`–`R-04`、`R-08` |
| `E-02` | Current fact | project 已有 dropdown/new shortcut，conversation 只有 hover pin/delete | `app/jaco/src/features/home/sidebar/row.rs::{ProjectSidebarRow,ConversationSidebarRow}` | 扩展项目，重构 conversation suffix |
| `E-03` | Current fact | current project menu 已有 pin/reveal/rename/remove | `app/jaco/src/features/home/sidebar/menu.rs::project_popup_menu` | 必须复用并回归，不重写 project persistence |
| `E-04` | Current fact | soft delete 使用 existing status/deleted_at，并 typed-reject running run | `crates/jaco-db/src/repository/conversations.rs::soft_delete_conversation`、`DbError::ConversationHasActiveRun` | `D-02`、`ERR-01` |
| `E-05` | Current fact | schema 同时存在 archived/deleted，但当前产品链路仅实现 soft delete | `crates/jaco-db/src/migrations.rs`、`schema.rs`、repository search | 不启用 archived，不做 migration |
| `E-06` | Current fact | `publish_summary` 更新 catalog 和 retained model；非-active summary 会从 catalog 移除 | `app/jaco/src/features/conversation/registry.rs` | rename 与 archive 可复用 existing projection model |
| `E-07` | Current fact | conversation service 已有 unused `delete`/`set_pinned` thin wrappers | `crates/jaco-conversation/src/lib.rs`；workspace `rg` call-site audit | service 可收敛成产品命名的唯一 mutation boundary |
| `E-08` | Upstream fact | 锁定 gpui-component 的 context/dropdown builders 都接受 `Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>)` | Cargo.lock `gpui-component#57a9903`；locked `menu/{context_menu,dropdown_menu}.rs` | 同一 popup builder 可挂两个入口 |
| `E-09` | Current fact | Jaco 已有可验证 clipboard helper，先写入再 readback 比较 | `app/jaco/src/components/chat/detail.rs::copy_to_clipboard` | C-03 采用同一验证语义 |
| `E-10` | Current fact | Jaco bundle 未注册 conversation deep-link scheme，Home route 只在当前主窗口内表达 | `app/jaco/Cargo.toml`、`HomeRoute`、window setup audit | deep link/new window 为非目标 |
| `E-11` | Current fact | app-local icon enum 缺 Archive，但 Lucide `archive.svg` 已存在 | `app/jaco/src/foundation/assets.rs`、`third_party/lucide/icons/archive.svg` | 仅新增 typed variant |
| `E-12` | User decision | 当前 delete 即产品 archive；新增 batch archive/copy working directory；conversation 右侧保留 pin/archive，完整上下文菜单仅右键显示；排除未读/share/deep link/new window/worktree | 本轮对话，最近更新 2026-08-23 | `D-01`–`D-03`、`D-05`–`D-06`、`D-10` |

## Decisions

| D-ID | Decision | Evidence | Material rejected alternative | Consequence/owner |
| --- | --- | --- | --- | --- |
| `D-01` | conversation 只显示一个“归档会话”动作；删除 delete 文案和 Trash affordance | `E-12` | 同时提供 Delete 与 Archive | `app/jaco` |
| `D-02` | UI archive 继续调用 DB soft-delete，写 `ConversationStatus::Deleted/deleted_at` | `E-04`–`E-05`、`E-12` | 新增真实 Archived lifecycle、迁移或恢复 UI | `jaco-db`、`jaco-conversation`、`app/jaco` |
| `D-03` | project/conversation 各有一个 typed action set；project context/overflow/new shortcut 与 conversation context/pin/archive direct buttons 调用同一 `invoke` handler 与 availability | `E-01`、`E-08`、`E-12` | 为各入口复制 callbacks | `app/jaco` |
| `D-04` | rename 成功返回完整 `ConversationSummary`，通过 registry 一次 publication 更新 catalog、retained model 和 workspace projections | `E-06` | 局部修改 row label或强制 reload 全 catalog | `app/jaco`、`jaco-conversation`、`jaco-db` |
| `D-05` | individual/project archive 先持有 generation-keyed runtime fence，和同 scope 的 submission admission 双向互斥；project DB command 再在一个 immediate transaction 中预检所有 active conversations 的 persisted running runs，存在冲突则零写入；成功后单次 `RemoveMany` publication | `E-04`、用户决定、实施期 Submitting→DB race review | 逐条循环、允许部分成功或只做非原子的运行状态快照 | `jaco-db`、`app/jaco` |
| `D-06` | copy working directory 使用 conversation 所属 project 的 stored `path`，按 plain text 写剪贴板并 readback 验证 | `E-09`、`E-12` | 复制 cwd、Git root、deep link 或项目菜单路径 | `app/jaco` |
| `D-07` | 用户只看到本地化安全错误；完整 DB/platform error 只进入 tracing，且不记录路径或标题内容 | 当前 error UI audit | 把 `error.to_string()` 直接显示给用户 | `app/jaco` |
| `D-08` | rename/archive 是 one-shot command Task；rename editor 继续使用 native `InputState`；不增加 Store、Operation 或 gpui-form | 当前 ownership、S-06/S-07 | 为短命令建立新的 resource/form framework | `app/jaco` |
| `D-09` | service 的 `delete` 改名 `archive`，新增 project batch archive；不保留 alias | `E-07`、`E-12` | 同时暴露 delete/archive 两套同义 API | `jaco-conversation` |
| `D-10` | conversation hover/focus suffix 保留两个直接按钮，Trash 改为 Archive，pin/unpin 继续按状态切换；conversation 不提供 Ellipsis，完整菜单仅右键显示；project 保留 new shortcut + overflow | `E-01`–`E-03`、`E-12` | conversation 使用 Ellipsis 或保留 Trash | `app/jaco` |

## Root-owned files and workspace topology

```text
docs/dev/
├── README.md                  # F-001 [Modify, handwritten] workspace feature-plan index
└── issue-188/README.md        # F-002 [Add, handwritten] status、shared contracts、sequencing and aggregate completion
```

Owner-local production/plan/index files are inventoried once in their respective owner plans. No crate、workspace member、manifest、workflow、ADR or generated artifact is added/moved/deleted.

## Observable requirements

| R-ID | Requirement |
| --- | --- |
| `R-01` | 每个 project 和 conversation row 都提供右键菜单；project 保留显式 overflow，conversation 右侧提供可聚焦的 pin/unpin 与 archive 直接按钮，且无 Ellipsis。菜单动作和顺序符合已确认范围。 |
| `R-02` | project context/overflow/new shortcut 与 conversation context/direct buttons 使用各自唯一 action set 的相同 label、icon、availability、confirmation 和 handler。 |
| `R-03` | 打开右键菜单或点击直接按钮不会展开/折叠 project、打开 conversation 或重复执行动作；普通 row click 保持原行为。 |
| `R-04` | conversation rename trim 后持久化，立即更新 pinned/project/no-project/sidebar search 的后续结果和当前 open model，并在 restart 后保留。 |
| `R-05` | individual archive 保留当前 soft-delete、active-run gate、runtime session close 和 active route reset；产品无第二个 delete。 |
| `R-06` | project archive 只处理该 project 的 active conversations；repository/service 对 empty 返回成功 no-op，UI 在已知 active count 为 0 时显示 disabled；任一 running run 使整批零写入；成功后所有相关投影与 sessions 一次收敛。 |
| `R-07` | copy working directory 在 normal、pinned 和 scratch/no-project rows 中复制对应 project path；readback 失败有本地化反馈。 |
| `R-08` | resource 未 ready 时动作 disabled；menu 打开后资源失效或 target 消失时安全拒绝，无 panic、无 partial state。 |
| `R-09` | en-US/zh-CN labels、confirmations、warnings、failures、tooltips 完整同义；archive/copy 使用 typed icons。 |
| `R-10` | 既有 project pin/rename/reveal/remove、新建 conversation、conversation pin、search 和 row routing 无回归。 |
| `R-11` | 未读/share/deep-link/new-window/worktree/unarchive/permanent-delete/path-edit 不出现在代码、菜单或 persistence diff。 |
| `R-12` | clipboard path 只因显式用户动作离开 app；diagnostics 不含 path/title，DB error detail 不直接呈现。 |

## Integration contracts

### C-01：Conversation rename command

Authoritative public boundary：`crates/jaco-conversation::ConversationService`。

```rust
impl<'a> ConversationService<'a> {
    pub fn rename(
        &self,
        id: &ConversationId,
        title: String,
    ) -> Result<ConversationSummary>;
}
```

- Producer：`jaco-conversation`，以 owner-local `FreshRepository::rename_conversation` 实现。
- Consumer：`app/jaco::features::conversation::rename_conversation`。
- App adapter：`SessionDatabaseExecutor::execute` 保持 `jaco_db::Result`；closure 将 service 的唯一 `ConversationError::Database(error)` 透明解包回 `error`。
- Input：UI 已 `trim()` 且非空的 title；DB 保存该精确字符串并更新时间。
- Output：完整 persisted summary；app 必须通过 `publish_summary` 投影，不能局部拼装。
- Compatibility：新增 API；现有 load/search contract 不变。
- Tests：`R-04`、owner `T-201`、`T-301`、app `T-104`–`T-105`。

### C-02：Single and project conversation archive commands

Authoritative public boundary：`crates/jaco-conversation::ConversationService`。

```rust
impl<'a> ConversationService<'a> {
    pub fn archive(&self, id: &ConversationId) -> Result<ConversationSummary>;

    pub fn archive_project_conversations(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ConversationSummary>>;
}
```

- Producer：`jaco-conversation`；single 复用既有 DB soft delete，batch 调用 owner-local atomic command。
- Consumer：`app/jaco::features::conversation::{archive_conversation,archive_project_conversations}`。
- App adapter：与 C-01 相同，在固定 `SessionDatabaseExecutor` 边界透明恢复 `DbError`，不修改 executor 泛型。
- Batch output：仅本次从 Active 转为 Deleted 的 summaries，按 `id ASC` 稳定排序；empty project 返回 `Ok(vec![])`。
- Atomicity：batch 的 active-run 检查和 UPDATE 在同一 immediate transaction；ERR-01 导致零写入。
- Compatibility：`delete` 改名为 `archive`；数据库 status/event 名称不迁移。
- Tests：`R-05`–`R-06`、owner `T-202`–`T-203`、`T-302`–`T-306`、app `T-106`–`T-108`。

### C-03：Explicit local working-directory clipboard

```rust
fn copy_working_directory(
    path: &Path,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), SidebarActionGuardError>;
```

- Definition/producer：`app/jaco` action layer；source 是 `SidebarProjectHeader.path`/project selection 的 stored `projects.path`。
- External consumer：OS clipboard via `ClipboardItem::new_string(path.display().to_string())`。
- Verification：立即 `read_from_clipboard().and_then(ClipboardItem::text)`，必须与写入字符串相等。
- Exposure：仅 plain text、本地显式动作；无 network、persistence、telemetry 或 path logging。
- Failure：ERR-03/ERR-05；测试与人工验证见 `R-07`、app `T-109`、root `T-02`。

## Error contracts

| ERR-ID | Identity/producer | Boundary and partial effects | Recovery/UI | Diagnostics/tests |
| --- | --- | --- | --- | --- |
| `ERR-01` | `DbError::ConversationHasActiveRun { conversation_id }`；single/batch DB command | service 透明包装；transaction 零写入，app 不 publication、不改 route | individual/project 使用各自 warning；用户先停止 run 后重试 | warning 记录 action + conversation ID；`T-303`、`T-307`、`T-02` |
| `ERR-02` | `SidebarActionGuardError::ResourceNotReady { resource }`；app action execution-time guard | 不启动 Task、不写 DB | menu 初始 disabled；stale-open menu 点击显示 localized unavailable | debug/warn 仅 resource/action；`T-101`、`T-110` |
| `ERR-03` | `SidebarActionGuardError::TargetDisappeared { target }`；workspace target recheck | 不启动 command；若 race 发生在 DB 后则按 ERR-04 且 transaction rollback | localized target unavailable，dismiss menu/dialog | warn 记录 target kind/ID；`T-102`、`T-110` |
| `ERR-04` | service 内为 `ConversationError::Database(DbError)`，app executor adapter 后恢复为 `DbError`；existing project action 同样使用 `DbError`，排除 ERR-01 | command transaction rollback；无 success publication/route change | operation-specific failure title + generic safe message；允许用户重试 | `tracing::error!` 保留 typed error、action、ID；`T-201`、`T-305`、`T-111` |
| `ERR-05` | `SidebarActionGuardError::ClipboardVerificationFailed`；C-03 readback mismatch/missing | DB/UI business state不变；clipboard 内容视为不可信 | `conversation-copy-failed` + existing safe message | 不记录 path/text；`T-109`、`T-02` |

目标 app-local guard declaration 由 [Jaco owner plan](../../../app/jaco/docs/dev/issue-188/README.md) 的 `L-105` 持有；owner plans 只实现上述映射，不重新定义 ERR 意义。

## Shared state and sequencing

```mermaid
sequenceDiagram
    participant UI as Sidebar action set
    participant APP as app/jaco feature command
    participant SVC as ConversationService
    participant DB as FreshRepository/SQLite
    participant REG as ConversationRegistry
    participant WS as HomeWorkspace + runtime

    UI->>APP: rename/archive/project archive
    APP->>SVC: C-01/C-02
    SVC->>DB: DB-301/DB-302
    alt ERR-01 or ERR-04
        DB-->>UI: error, zero publication/route change
    else committed
        DB-->>APP: persisted summary/summaries
        APP->>REG: publish_summary or RemoveMany
        REG->>WS: catalog/model notifications
        APP->>WS: close sessions; reset active route when removed
        WS-->>UI: all sidebar projections converge
    end
```

## Work-package sequence

| Order | WP | Owner | Observable outcome | Dependencies | Owner plan |
| --- | --- | --- | --- | --- | --- |
| 1 | `WP-301` | `jaco-db` | rename query and atomic active-project soft-delete with focused DB tests | `D-02`、`D-05`、`ERR-01` | [DB owner](../../../crates/jaco-db/docs/dev/issue-188/README.md) |
| 2 | `WP-201` | `jaco-conversation` | C-01/C-02 product-named service boundary and error preservation | `WP-301` | [service owner](../../../crates/jaco-conversation/docs/dev/issue-188/README.md) |
| 3 | `WP-101` | `app/jaco` | feature commands, `publish_summary`/`RemoveMany`, session close and route convergence | `WP-201` | [Jaco owner](../../../app/jaco/docs/dev/issue-188/README.md) |
| 4 | `WP-102` | `app/jaco` | shared action sets、project 双菜单入口、conversation 右键菜单与直接按钮、dialogs、clipboard、icons 和 Fluent | `WP-101` | [Jaco owner](../../../app/jaco/docs/dev/issue-188/README.md) |
| 5 | `WP-103` | `app/jaco` | focused UI/state regression tests and repeatable manual matrix | `WP-102` | [Jaco owner](../../../app/jaco/docs/dev/issue-188/README.md) |
| 6 | `WP-001` | workspace root | aggregate validation, CI evidence and plan status synchronization | `WP-301/201/101/102/103` | 本文档 |

### WP-001：Aggregate validation and completion synchronization

**Owner**

Workspace root。

**Prerequisites and contracts**

- 所有 owner WPs 完成；`R-01`–`R-12`、`C-01`–`C-03`、`ERR-01`–`ERR-05` 无 deviation。

**File IDs**

- Root/owner plan 与四个 `docs/dev/README.md` index。

**Implementation sequence**

1. 按下表先执行 focused checks，再执行 workspace gates；不得用单个 package compile 替代行为测试。
2. 使用 `JACO_CONFIG_DIR=<temporary-directory> cargo run -p jaco` 完成 `T-01`–`T-03`，保留同一 temporary directory 做 restart persistence 验证。
3. 推送实现后运行 `.github/workflows/ci.yml` 的 macOS/Linux/Windows jobs，记录 PR/commit/CI URL。
4. 同步四份 plan 的实际文件、命令、结果和 deviation；所有必要证据满足后才把 root/index 改为 `Done`。

**Failure and lifecycle behavior**

- 任一 owner test、人工场景或 CI 未通过时保持 `In progress`，记录 exact blocker；不能以未验证推断完成。

**Tests**

| R-ID | T-ID/file | Proposed scenario | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| `R-01`–`R-03`、`R-08`–`R-10` | `T-01` manual | project right-click/overflow、conversation right-click + pin/archive hover/focus/click、Escape、普通 row click、disabled state | isolated Jaco data + normal/pinned/scratch rows | conversation 无 Ellipsis；直接按钮和右键菜单复用同一行为；无 row toggle/open/duplicate action |
| `R-05`–`R-06` | `T-02` manual | individual/project archive，包含一条 running conversation | 同项目至少两条 conversations | running 时零消失；停止后整批消失；当前 route 回 NewConversation |
| `R-04`、`R-07` | `T-03` manual | rename duplicate projections、copy path、restart | pinned conversation + project/no-project rows | 标题即时一致且重启保留；clipboard 等于 stored project path |
| `R-11`–`R-12` | `T-04` diff/log audit | 搜索排除项与敏感 log fields | final branch diff | 无额外能力/schema/dependency/path-title logging |

**Focused validation**

| Command/manual scenario | Purpose | Required environment | Expected evidence |
| --- | --- | --- | --- |
| `cargo fmt --check` | formatting | repository toolchain | clean |
| `cargo test -p jaco-db` | persistence/atomicity | local SQLite | all pass |
| `cargo test -p jaco-conversation` | service contract | local SQLite | all pass |
| `cargo test -p jaco sidebar_context_menu` | action/menu/state tests | GPUI test runtime | all new focused tests pass |
| `cargo test -p jaco conversation_registry` | publication/model tests | GPUI test runtime | rename/remove-many convergence passes |
| `cargo test -p jaco i18n` | locale parsing/parity | local | both locale bundles load |
| `cargo check -p jaco` | app integration | locked deps | success |
| `cargo clippy -p jaco -p jaco-conversation -p jaco-db --all-targets --all-features -- -D warnings` | selected-owner lint | locked deps | no warnings |
| `cargo build` | workspace build | repository baseline | success |
| `cargo test` | workspace regression | repository baseline | all pass |
| `cargo clippy --all-targets --all-features -- -D warnings` | workspace lint | repository baseline | no warnings |
| `git diff --check` | whitespace/docs | final diff | clean |

**Done condition**

`R-01`–`R-12` 均有通过证据；implementation commit/PR 与三平台 CI 已记录；schema/migration/Cargo/lock diff 为空；四份 plan 与 indexes 状态一致。

## Validation mapping

| R-ID/requirement | Owner/WP | Automated/manual evidence | Expected result | External prerequisite |
| --- | --- | --- | --- | --- |
| `R-01`–`R-03` | `WP-102/103` | app `T-101`–`T-103` + root `T-01` | project 双菜单入口与 conversation context/direct buttons 共用 action set；direct buttons keyboard 可达，无 propagation/duplicate | local desktop for manual |
| `R-04` | `WP-301/201/101` | `T-301`、`T-201`、`T-104`–`T-105`、`T-03` | DB/model/sidebar/restart title一致 | local desktop restart |
| `R-05` | `WP-301/201/101/102` | existing single archive active-run test + `T-106`、`T-02` | soft-delete、session/route 与 warning 正确 | running agent for manual |
| `R-06` | `WP-301/201/101` | `T-302`–`T-307`、`T-202`–`T-203`、`T-107`–`T-108`、`T-02` | empty/no-op、stable output、atomic rejection、batch publication | local SQLite/desktop |
| `R-07` | `WP-102/103` | `T-109`、`T-03` | exact project path copied/readback | OS clipboard |
| `R-08` | `WP-102/103` | `T-101`–`T-102`、`T-110`、`T-01` | disabled/reject，无 panic/partial | local desktop |
| `R-09` | `WP-102` | icon/i18n focused tests + `T-01` | typed icon、locale parity、正确文案 | none |
| `R-10` | all owners | owner regressions + workspace gates | existing actions/routing/search pass | CI |
| `R-11`–`R-12` | `WP-001` | root `T-04` | excluded diff absent，safe diagnostics | final diff/log review |

## Completion evidence

| Evidence | Actual result |
| --- | --- |
| Implementation PR and commits | implementation commit `6cd0cb2`；ready PR [#208](https://github.com/suxiaoshao/gpui/pull/208) targets `main`；本次 plan evidence synchronization 由后续 docs commit 记录 |
| Actual added, modified, moved, deleted, generated, synchronized, submodule, and vendored files | 新增 `sidebar/actions.rs` 与 root/三个 owner 的 `issue-188/README.md`；修改 sidebar/menu/row、conversation feature/registry/runtime、workspace、chat detail、archive confirm、icons、双 locale、service、DB repository/tests 与四个 plan indexes；无移动、删除、生成、同步、submodule 或 vendored 文件 |
| Delivered D/F/L/C/ERR/DB/G/ST/R/T/WP IDs | `D-01`–`D-10`、`C-01`–`C-03`、`ERR-01`–`ERR-05` 与 owner `L/DB/ST` production contracts 已实现；`WP-301/201/101/102` 完成，`WP-103/001` 的现有自动化与 diff audit 完成。Direct tests cover action order/availability/clipboard、rename projection、RemoveMany、DB/service atomicity/error identity and runtime fence admission；driver release/session-close、stale guard/error mapping 的独立 P2 tests、人工 `T-01`–`T-03` 与远端 CI 尚未闭合 |
| Automated commands and results | 最新交互修订后 `cargo fmt --all` pass、`cargo test -p jaco` 528 pass/2 ignored、`cargo check -p jaco` pass、`cargo clippy -p jaco --all-targets --all-features -- -D warnings` pass；此前同一实现分支的 `cargo test -p jaco-db` 77 pass、`cargo test -p jaco-conversation` 8 pass、`cargo build` pass、提权允许 loopback 后 `cargo test` 全 workspace pass、full-workspace clippy pass；`git diff --check` pass |
| Manual, packaged-app, or real-API scenarios and environment | `cargo run -p xtask -- bundle jaco` 成功生成 `target/release/bundle/macos/Jaco.app`，CoreSimulator 不可用时按 xtask 既有逻辑回退普通 icon；使用隔离 `JACO_CONFIG_DIR` 和唯一 bundle identifier 启动测试副本，5 分钟后仍无 AX window，sample 显示首窗初始化同步阻塞于既有 skill file-watch 对 `/Users/sushao` 的扫描，因此未执行菜单点击、restart、clipboard 与 running-run UI 矩阵 |
| Schema/migration/dependency/generated/vendored diff | `git diff --exit-code -- Cargo.toml Cargo.lock crates/jaco-db/Cargo.toml crates/jaco-db/src/migrations.rs crates/jaco-db/src/schema.rs` pass；无 dependency、schema、migration、generated 或 vendored diff |
| Owner README, index, and ADR updates | root/三个 owner plans 记录实际实现与验证；四个 indexes 同步为 `In progress`；本次无需 ADR |
| Accepted deviations and approving decision | 无产品范围 deviation。用户最新确认 conversation 保留 pin/archive 直接按钮且完整菜单仅右键显示，已替换先前 Ellipsis 设计；实施期发现 submission `Submitting` 到 DB running row 之间存在竞态，按 `R-05/R-06` 增加 generation-keyed runtime archive fence；独立 runtime review 未发现 P0/P1 correctness 问题 |
| Unverified boundaries and reason | `T-01`–`T-03` 人工交互/重启/clipboard/running-run 场景受上述启动期文件监听阻塞；PR #208 的 macOS/Linux/Windows 远端 CI 等待结果；状态保持 `In progress` |

## Execution handoff audit

- [x] Root hub 和三个 same-ID owner plans 存在并双向链接。
- [x] 19 个 S-row 均有唯一 applicability decision 与证据。
- [x] 用户决定、非目标、兼容与 soft-delete 数据策略已固定。
- [x] C/ERR 由 root 持有，owner-local L/DB/ST 不重复其共享语义。
- [x] DB atomicity、empty behavior、stable output、rollback 和 no-migration policy 已固定。
- [x] GPUI action/menu/focus/task/publication/route/clipboard ownership 已分配，无待选 primitive。
- [x] 所有 observable R-ID 映射到 owner tests 或 root manual/aggregate evidence。
- [x] icons、Fluent、privacy、diagnostics、platform、dependency 与 excluded scope 均有显式决策。
- [x] 执行者无需再发明 architecture、migration、action entrypoint 或 acceptance criterion。
