# Issue #193：Jaco 侧边栏会话悬浮预览、活动时间与运行状态

## 状态与范围

- 状态：`In progress`
- 关联 issue：[#193](https://github.com/suxiaoshao/gpui/issues/193)
- Plan ID：`issue-193`
- 根计划：`docs/dev/issue-193/README.md`
- 根索引：[Workspace development plans](../README.md)
- 分支：`codex/193-jaco-sidebar-activity-previews`
- 受影响 owner：`crates/jaco-core`、`crates/jaco-db`、`app/jaco`
- 发布门：本地自动化已完成；Jaco 人工 UI 与远端三平台 CI 尚未执行
- 最近证据刷新：2026-08-24
- 实施引用：本地分支 `codex/193-jaco-sidebar-activity-previews` 正在实施

### 高影响变更摘要

| 审计门 | 结果 | Canonical IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | `[Cross-owner]` 在既有 core、DB、Jaco 三个 owner 内增加 recency 契约、持久化和 UI/runtime 投影，不新增 crate | `D-03`、`D-05`、`C-01`–`C-02`、`WP-101/201/301/302` |
| Public or cross-owner contracts | `[Modify] [Breaking]` workspace-internal `ConversationSummary` 增加非空 `recency_at`，所有 producer/consumer 同步更新 | `C-01`、`WP-101/201/301` |
| Global/shared authority | `[Modify]` `ConversationRuntimeStore` 成为 sidebar transient status 唯一 authority；`HomeSidebar` 只观察和读取，不复制状态 | `D-05`–`D-06`、`R-11`、`WP-302` |
| Persistence, data, configuration, or credentials | `[Modify]` 唯一 `0001` fresh schema 直接增加 `conversations.recency_at DateTime NOT NULL` 与 active-recency index；不增加额外数据库打开路径 | `D-03`、`D-09`、`DB-01`、`ERR-01`–`ERR-03`、`WP-201` |
| Runtime, concurrency, performance, or shutdown | `[Modify]` runtime failure/approval 投影实时通知 sidebar；一个 sidebar-owned minute task 刷新相对时间，HoverCard 继续自持 delay task | `D-04`–`D-08`、`ST-01`、`WP-301/302` |
| Security, privacy, or external access | `None`；卡片只消费已有 title、project display name、recency，不读取消息正文、不访问外部服务、不增加过滤或隐藏链路 | `D-02`、`R-13` |
| Dependencies, toolchains, generated, or vendored artifacts | `None`；复用锁定的 `gpui-component::hover_card::HoverCard`、现有 Spinner 与 Lucide icons | `D-08`、`S-11`、`S-12`、`S-17` |
| Platform, packaging, CI, or release | `None`；不改 bundle、entitlement、workflow 或平台分支，远端三平台 CI 仍是聚合门 | `S-16`、`R-14`、`WP-001` |
| User-visible defaults or removals | `[Modify]` 项目会话新增两行悬浮卡片；无项目会话无卡片；Idle 留空，特殊状态在 hover 时让位给现有置顶/归档按钮 | `D-01`–`D-08`、`R-01`–`R-10` |

### 目标

让用户在不打开会话的情况下查看项目会话的完整标题、所属项目和最近活动时间，并直接从侧边栏识别 Running、AwaitingApproval、Failed 状态。鼠标 hover 时，行尾统一显示既有 Pin/Unpin 与 Archive 两个操作按钮。

### 非目标

- GitHub PR、branch、worktree 或 repository 状态集成。
- 最近消息、消息摘要、用户输入或 agent 输出预览。
- 无项目会话的悬浮卡片。
- 将 transient sidebar status 写入 `ConversationSummary`、SQLite、`HomeWorkspace` 或 `SidebarSnapshot`。
- 持久化普通历史 Failed 状态；应用重启后仅本次 recovery 终结的遗留 run 显示 Failed。
- 新增 provider、Rig、MCP、network、credential、telemetry、filter 或 redaction 能力。
- 更改现有 conversation context menu、Pin/Archive mutation、确认框或 focus-visible 行为。
- 为每一行创建 timer、Task 或独立 hover/status authority。

### 用户已确认决定

- 悬浮卡片只包含两行：完整标题与相对时间；Folder 图标与项目显示名称。
- 卡片不显示 computer/status 图标。
- 第三行原本对应 GitHub PR/branch；Jaco 没有该功能，本 issue 省略。
- 无项目会话不显示悬浮卡片，包含它在 pinned 区域的副本。
- 特殊会话状态显示在 sidebar 行右侧；Idle 常态为空。
- 鼠标 hover 时，行尾状态或空白区域切换为既有 Pin/Unpin 与 Archive 两个按钮。
- pinned、project、no-project 三个位置采用相同状态和操作规则。
- 独立 recency 只由创建会话和成功 append 新 entry 推进；rename、pin、metadata、已有 entry payload 更新、archive/delete 不推进。

### Schema 与数据策略

- `ConversationSummary.recency_at` 是 workspace-internal breaking field；同一分支一次性更新所有 DB rows、fixtures、catalog 和 sidebar consumers，不保留旧构造器。
- 产品尚未发布，本 issue 只定义 fresh schema：`SCHEMA_VERSION` 保持 `1`，唯一 `0001_create_fresh_schema` 直接声明 `conversations.recency_at DateTime NOT NULL` 与 active-recency index。
- 产品尚未发布，本 issue 不设计或验证旧数据库兼容、升级、回填或迁移路径。
- `updated_at` 继续表示记录修改时间；recency 专用于 conversation activity、sidebar 排序与 hover 时间。
- 不改公开配置、provider、tool、MCP、bundle、manifest、Cargo dependency 或 `Cargo.lock`。

### 计划映射

| Scope | 文档 | Owns | Assigned IDs/WPs |
| --- | --- | --- | --- |
| Root hub | 本文档 | 状态、范围、用户决定、S/C/ERR、跨 owner 顺序与聚合验收 | `E-01`–`E-12`、`D-01`–`D-10`、`C-01`–`C-02`、`ERR-01`–`ERR-03`、`DB-01`、`ST-01`、`R-01`–`R-14`、`T-01`–`T-06`、`WP-001` |
| `crates/jaco-core` | [owner plan](../../../crates/jaco-core/docs/dev/issue-193/README.md) | 非空 conversation recency domain contract 与 fixtures | `F/L/R/T/WP-1xx` |
| `crates/jaco-db` | [owner plan](../../../crates/jaco-db/docs/dev/issue-193/README.md) | fresh schema、recency writes、records、排序与 DB tests | `F/L/DB/R/T/WP-2xx` |
| `app/jaco` | [owner plan](../../../app/jaco/docs/dev/issue-193/README.md) | catalog/workspace recency projection、HoverCard、relative clock/i18n、runtime status、row suffix 与 UI tests | `F/L/ST/R/T/WP-3xx` |

## Applicability

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定或 negative reason | Owning section/WP |
| --- | --- | --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Applicable | core 定义 summary，DB 持久化，Jaco registry/workspace/row 消费 | 保持三 owner；不新增 crate/module root | `C-01`–`C-02`、`WP-101/201/301/302` |
| `S-02` | GPUI components, layout, interaction, and accessibility | Applicable | conversation row 已有两个 focus-visible direct buttons；锁定组件提供 HoverCard | 复用 native HoverCard、Spinner、Icon、Label；状态 hover 让位且保留键盘按钮 | `D-01`、`D-07`–`D-08`、`WP-301/302` |
| `S-03` | Entity, Store, Global, identity, and projections | Applicable | catalog/workspace 持久化摘要，runtime 持有 active attempts/approval | runtime 是唯一 transient status authority；HomeSidebar 观察，不新增 Store/Global | `D-05`–`D-06`、`ST-01`、`WP-302` |
| `S-04` | Actions, events, subscriptions, focus, and windows | Applicable | row 已用 group hover、stop propagation 和 focus-visible buttons | runtime subscription 驱动重绘；现有 click/context/direct action semantics 保持 | `D-07`、`WP-302` |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Applicable | runtime 用 ActiveRunKey fence；HoverCard 自持 delay tasks | 保留 stale-publication fence；HomeSidebar 只持一个 minute task，随 Entity drop 取消 | `D-04`–`D-06`、`WP-301/302` |
| `S-06` | Data acquisition and Operation state | No change | catalog/recovery 已使用 `refresh::Operation` | recency 进入既有 catalog load；sidebar status 不新增 Operation phase | `WP-301/302` |
| `S-07` | Forms and editable state | N/A | 本 issue 没有输入、表单或可编辑 draft | 不改 `gpui-form`、InputState 或 mutation dialogs | `S-07` |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | `ConversationSummary` 从 DB 经 conversation service 进入 app | 固定 C-01/C-02；provider/Rig/MCP/platform contracts 不变 | `C-01`–`C-02`、`WP-101/201/301` |
| `S-09` | Error identity, propagation, recovery, and error UI | Applicable | runtime last error 可被通知消费 | status failure marker 与一次通知分离；DB 沿用现有错误边界 | `WP-201/302` |
| `S-10` | Database, persistence, and schema | Applicable | fresh schema 由唯一 `0001` 定义 | 直接增加非空 recency、active index 与 write semantics；不设计旧库路径 | `D-03`、`D-09`、`DB-01`、`WP-201` |
| `S-11` | Generated, synchronized, copied, or vendored content | No change | Diesel schema 为手写文件；所需 Lucide SVG 已存在 | 不生成、复制、vendor 或同步 artifact | `S-11` |
| `S-12` | Icons and assets | Applicable | Jaco 已有 `Folder`、`ShieldAlert`、`CircleAlert`，Running 可用 Spinner | 只复用现有 typed icons/assets，不改 enum 或 asset bundle | `D-01`、`D-07`、`WP-301/302` |
| `S-13` | Fluent i18n and bundle localization | Applicable | en-US/zh-CN `main.ftl` 持有 sidebar 文案 | 增加相对时间六单位和状态 tooltip keys，保持 locale parity | `D-04`、`R-12`、`WP-301/302` |
| `S-14` | Security, privacy, and credentials | No change | title/project/时间已在 sidebar snapshot；需求不含消息正文或外部数据 | 不建立额外安全过滤、隐藏、网络、credential 或日志内容链路 | `D-02`、`R-13` |
| `S-15` | Observability and diagnostics | Applicable | runtime/DB 已用 tracing 记录失败 | DB/runtime 原错误继续 tracing；sidebar 只显示 typed indicator/本地化 tooltip，不显示错误正文 | `ERR-01`–`ERR-03`、`WP-201/302` |
| `S-16` | Packaging, platform behavior, and CI/release | No change | UI、SQLite 与 assets 已进入现有 app bundle/CI | 不改 packaging/workflow；保留三平台 CI gate | `WP-001` |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | No change | `Cargo.lock` 锁定 gpui-component `0.5.2@57a9903` 且已提供 HoverCard | 不改 manifest、feature、git SHA 或 lockfile | `D-08`、`WP-301` |
| `S-18` | Owner documentation, indexes, and ADRs | Applicable | repo 采用 root hub + same-ID owner plans | 新增四份计划并更新四级 index；无需 ADR | `WP-001` |
| `S-19` | Validation and completion evidence | Applicable | AGENTS 定义 fmt/build/test/clippy，视觉行为需人工验证 | owner focused → manual Jaco → workspace gates → remote CI | `R-14`、`T-01`–`T-06`、`WP-001` |

## Evidence

### 当前流程

1. `FreshRepository::list_sidebar_conversations` 按 `conversations.updated_at DESC` 返回 `ConversationSummary`；`ConversationCatalogModel` 和 `HomeWorkspace` 再次按 `updated_at` 排序。
2. `append_conversation_entry_with_conn` 更新 `last_entry_seq + updated_at`；rename、pin、metadata、payload update、archive/delete 也更新 `updated_at`，所以它不能表达纯 conversation activity。
3. `FreshStore::open_or_create_initial` 为新库执行唯一 `0001_create_fresh_schema`；本 issue 只修改这份 fresh schema。
4. `HomeWorkspace` 把同一 `SidebarConversationNode` clone 到 project、no-project 与 pinned；row 当前渲染 truncated title 和同一 trailing Pin/Archive overlay。
5. `ConversationRuntimeStore` 已持有 Submitting/Running/Stopping attempts、ActiveRunKey、approval broker 和 `last_errors`；`HomeSidebar` 当前没有 runtime subscription。
6. `ConversationApprovalBroker` 独占 pending approvals 并在数量变化时发布 `ToolApprovalAvailabilityChanged`；startup recovery 会把遗留 Running/WaitingForApproval run 持久化为 Failed 并返回 recovered records。

### Evidence registry

| E-ID | Classification | Claim | Evidence | Plan consequence |
| --- | --- | --- | --- | --- |
| `E-01` | Current fact | Issue #193 已同步最终两行卡片、project-only、recency 和 row status/action 规则 | [GitHub Issue #193](https://github.com/suxiaoshao/gpui/issues/193)，2026-08-23 | `R-01`–`R-10` |
| `E-02` | User decision | card 两行、无 computer/status icon、无 GitHub 第三行、无项目无卡片 | 本轮对话，2026-08-23 | `D-01`–`D-02` |
| `E-03` | User decision | 特殊状态在行右；Idle 为空；hover 换为两个操作按钮；三个位置一致 | 本轮对话，2026-08-23 | `D-05`–`D-07` |
| `E-04` | User decision | recency 只由创建和 append entry 推进 | 本轮对话，2026-08-23 | `D-03`、`DB-01` |
| `E-05` | Current fact | current row 只有 title 与 Pin/Archive overlay，按钮 stop propagation 且 focus-visible | `app/jaco/src/features/home/sidebar/row.rs::ConversationSidebarRow` | 复用既有 actions，不重建交互 |
| `E-06` | Current fact | 同一 node clone 到 project/no-project/pinned，且全部按 updated_at | `app/jaco/src/features/home/workspace.rs::build_sidebar_snapshot` | 单一 node recency/project-name projection覆盖三处 |
| `E-07` | Current fact | active attempt、approval broker、recovery 与 last_errors 都归 runtime | `app/jaco/src/features/conversation/runtime.rs`、`runtime/approval.rs` | runtime 成为 status authority |
| `E-08` | Current fact | fresh DB 由唯一 `0001_create_fresh_schema` 建立 | `crates/jaco-db/src/migrations.rs::{SCHEMA_VERSION,MIGRATIONS,CREATE_FRESH_SCHEMA_SQL}` | 本 issue 只修改 fresh schema 与非空 recency contract |
| `E-09` | Current fact | append 与多种 presentation mutation 都推进 updated_at | `crates/jaco-db/src/repository.rs::append_conversation_entry_with_conn`、`repository/conversations.rs` | 增加独立 recency |
| `E-10` | Upstream fact | 锁定 HoverCard 默认 open 600ms/close 300ms，使用 stable ElementId 的 keyed state 自持 hover timer | locked `gpui-component@57a9903` `crates/ui/src/hover_card.rs` | app 只测量 trigger bounds 以定位；不创建 hover state/task |
| `E-11` | Reference implementation | Codex sidebar 使用 `recencyAt ?? updatedAt`，按 m/h/d/w/mo/y 分段并每分钟刷新；Codex mode 不给 projectless thread 建 card | `/Applications/ChatGPT.app/Contents/Resources/app.asar`、`/Applications/ChatGPT.app/Contents/Resources/codex`，2026-08-23 本地解包核对 | `D-01`、`D-04`、`D-08` |
| `E-12` | Current fact | Folder、ShieldAlert、CircleAlert、Spinner 已存在且本地化文件为双 locale | `app/jaco/src/foundation/assets.rs`、`app/jaco/locales/{en-US,zh-CN}/main.ftl` | 不新增 icon/assets，只增 Fluent keys |

## Decisions

| D-ID | Decision | Evidence | Material rejected alternative | Consequence/owner |
| --- | --- | --- | --- | --- |
| `D-01` | 只有 normal-project conversation 建 HoverCard；project 与 pinned clone 共享同一 title/project/recency；scratch/no-project 及其 pinned clone 不建 card | `E-01`–`E-03`、`E-11` | 给所有会话显示“无项目”card | `app/jaco` |
| `D-02` | card 只含 full title + relative recency、Folder + project display name；不加入 GitHub、消息、computer/status icon | `E-01`–`E-02` | 最近消息、安全过滤层或未来 GitHub placeholder | `app/jaco` |
| `D-03` | 非空 domain recency 与 updated_at 分离；创建及成功 append 推进，所有 presentation/bookkeeping mutation 保持 recency | `E-04`、`E-09` | 继续复用 updated_at | `jaco-core`、`jaco-db`、`app/jaco` |
| `D-04` | relative formatter 使用 floor 后的 m(<60m)、h(<24h)、d(<7d)、w(<30d)、mo(<365d)、y；future clamp 到 0m；HomeSidebar 用一个 60s task 重绘 | `E-11` | per-row timer或只在数据 mutation 时刷新 | `app/jaco` |
| `D-05` | `ConversationRuntimeStore` 是 `ConversationSidebarStatus` 唯一 authority；HomeSidebar observe/query，workspace/snapshot/DB 不保存 transient status | `E-07` | catalog、workspace、row 各持一份 status map | `app/jaco` |
| `D-06` | priority 为 pending approval > active attempt > current-session failure marker > Idle；Submitting/Running/Stopping→Running，Completed/Canceled→Idle | `E-03`、`E-07` | 把 Stopping/Completed 单独显示 | `app/jaco` |
| `D-07` | Idle suffix 无节点；特殊状态常驻 suffix；pointer group-hover 隐藏状态并显示既有 Pin/Archive，按钮的 focus-visible 能力保持 | `E-03`、`E-05` | 新增第三个 More 或把状态放 card | `app/jaco` |
| `D-08` | 使用 locked native HoverCard 默认 delay/appearance、conversation-ID stable ElementId；app 只保存 measured trigger bounds 并移动整个 popover root，不自建 hover state/task | `E-10` | on_hover app state machine/Task 或移动 card 内部内容 | `app/jaco` |
| `D-09` | 唯一 `0001` fresh schema 直接声明 non-null physical recency column 和 active-recency index；SQL/records/domain 全链路保持 non-null | `E-08`–`E-09` | dynamic timestamp default、fallback recency 或额外 schema path | `jaco-db` |
| `D-10` | 不增加依赖、assets、provider/external contract、bundle 或 workflow | `E-10`、`E-12` | 引入新 UI/time crate | 所有 owner |

## 跨 owner 目标契约

### C-01：Conversation recency domain contract

Authority：`crates/jaco-core::ConversationSummary`。

```rust
pub struct ConversationSummary {
    // existing fields
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub recency_at: OffsetDateTime,
    // existing archived/deleted fields
}
```

- Producer：`jaco-db` typed row conversion and all returned `ConversationRecord` values。
- Consumers：`jaco-conversation` transparent transport；Jaco catalog/workspace/sidebar。
- Source contract：workspace source break, one-shot coordinated update；no serialized public API。
- Tests：`T-01`、owner `T-1xx/T-2xx/T-3xx`。

### C-02：Persisted recency to sidebar projection

```text
SQLite conversations.recency_at
→ SqlConversationRow (non-null physical representation)
→ strict ConversationSummary.recency_at (C-01)
→ ConversationService::load_catalog
→ ConversationCatalogModel sort
→ HomeWorkspace SidebarConversationNode.recency_at
→ HoverCard relative formatter
```

- A committed append returns one complete summary and `ConversationIndexDelta::EntryAdvanced.recency_at`; catalog publication/upsert uses that summary.
- Rename/pin/metadata/archive publications carry unchanged recency and may change updated_at.
- DB query、catalog 和 workspace all use recency descending with conversation ID ascending tie-break。

### DB-01：Recency mutation matrix

| Operation | `updated_at` | `recency_at` | Atomicity |
| --- | --- | --- | --- |
| create conversation without/with first entry | initialize now | initialize/append now | existing create transaction |
| append a new conversation entry | advance | advance | same entry append transaction |
| update existing entry payload/status | advance | preserve | existing statement/transaction |
| rename, pin, metadata/settings | advance | preserve | existing statement |
| archive/delete bookkeeping | advance | preserve | existing immediate transaction |
| failed append | preserve | preserve | SQLite transaction |

### ST-01：Sidebar status lifecycle

```text
accepted submission ──> Running ── pending approval ──> AwaitingApproval
       │                    ▲               │ resolved/denied, run continues
       │                    └───────────────┘
       │
       ├─ submission/launch/outer failure ──> Failed
       └─ run Completed/Canceled ───────────> Idle

Failed ── next accepted submission ──> Running
startup recovered interrupted run ──> Failed (current app session)
app restart without recovered run ──> Idle
```

- ActiveRunKey continues rejecting stale completion/approval publications.
- Multiple pending approvals remain AwaitingApproval until the last is resolved.
- Deny does not directly mean Failed；terminal run result decides。
- Recovery operation failure remains a resource problem and does not mark every conversation Failed。

## Error contract

Fresh-schema 写入和运行时错误继续沿用现有 `DbError`、`DatabaseValidationError` 与 notification/tracing 路径。Runtime `Failed` 是可观察会话状态，不承载错误正文；sidebar 只显示类型化状态，通知字符串最多消费一次，failure marker 保留到下一次成功接受提交。

## Observable requirements

| R-ID | Requirement |
| --- | --- |
| `R-01` | Normal-project conversation row shows the same two-row HoverCard from project and pinned sections. |
| `R-02` | No-project/scratch row and its pinned clone have no HoverCard. |
| `R-03` | Card contains full title, current relative recency, Folder icon, project display name only；no GitHub/message/computer/status row. |
| `R-04` | DB query、catalog、project conversations、no-project conversations and pinned conversations sort by recency DESC, ID ASC tie-break；project sorting remains unchanged. |
| `R-05` | Only create and committed entry append advance recency；all matrix-preserve operations and failed transactions keep it unchanged. |
| `R-06` | Fresh schema uses `SCHEMA_VERSION == 1` and the single `0001` containing non-null recency plus active-recency index；SQL/domain mapping has no fallback. |
| `R-07` | Relative labels follow D-04 in English/Chinese and refresh at most one minute after a threshold crossing. |
| `R-08` | Idle renders no suffix；Running/AwaitingApproval/Failed use typed spinner/icon + localized tooltip. |
| `R-09` | Pointer hover replaces status/empty suffix with existing Pin/Unpin + Archive in pinned/project/no-project；row open/context/direct-action propagation and focus-visible buttons remain correct. |
| `R-10` | Status priority/reset/recovery follows ST-01, including multiple approvals, terminal outcomes, transient failures and stale publications. |
| `R-11` | Runtime owns all transient status/failure markers；HomeSidebar only holds observation/minute redraw Task；no per-row Entity/Task/status map. |
| `R-12` | en-US/zh-CN keys stay in parity and only existing assets/icons are used. |
| `R-13` | No message-content projection, external access, credential, telemetry, redaction/filter layer, dependency or lockfile change is introduced. |
| `R-14` | Focused owner tests, manual UI matrix, workspace fmt/build/test/clippy and remote macOS/Linux/Windows CI complete before Done. |

## Work packages

```text
WP-101 core recency contract
        ↓
WP-201 DB fresh schema + recency writes/order
        ↓
WP-301 app catalog/workspace recency + HoverCard/relative clock/i18n
        ├──────────────┐
        ↓              ↓
WP-302 runtime status + row suffix/action switch
        ↓
WP-001 aggregate validation, manual UI, documentation and CI evidence
```

| WP | Owner | Observable outcome | Dependencies | Owner plan |
| --- | --- | --- | --- | --- |
| `WP-101` | `crates/jaco-core` | every conversation summary exposes strict recency | `C-01`、`D-03` | [core plan](../../../crates/jaco-core/docs/dev/issue-193/README.md) |
| `WP-201` | `crates/jaco-db` | fresh schema and exact recency write/order semantics | `WP-101`、`C-01/C-02`、`DB-01` | [DB plan](../../../crates/jaco-db/docs/dev/issue-193/README.md) |
| `WP-301` | `app/jaco` | catalog/sidebar use recency and project conversations show exact HoverCard | `WP-101/201`、`D-01`–`D-04`、`D-08` | [Jaco plan](../../../app/jaco/docs/dev/issue-193/README.md) |
| `WP-302` | `app/jaco` | runtime projects typed statuses and row hover reveals existing actions | `D-05`–`D-07`、`ST-01` | [Jaco plan](../../../app/jaco/docs/dev/issue-193/README.md) |

### WP-001：聚合验收与完成证据

**Owner**

Workspace root。

**Prerequisites and contracts**

- `WP-101`、`WP-201`、`WP-301`、`WP-302` complete。
- `R-01`–`R-14` and owner tests pass。

**File IDs**

- `docs/dev/README.md` `[Modify, handwritten]`
- `docs/dev/issue-193/README.md` `[Add, handwritten]`

**Implementation sequence**

1. 执行 owner focused validation，记录 exact commands/results。
2. 用隔离 Jaco data dir 手工验证 project/pinned/no-project、长标题、时间阈值、Idle/Running/approval/Failed、hover actions 和 keyboard focus。
3. 执行 workspace fmt/build/test/clippy；提交后核对远端三平台 CI。
4. 补充 root/owner completion evidence；只有所有必需门通过才把状态改为 Done。

**Tests**

| R-ID | T-ID | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-01`–`R-03`、`R-07` | `T-03` manual UI | project + pinned + no-project，长标题与跨 threshold clock | 两行 card exact；no-project no card；时间更新 |
| `R-08`–`R-10` | `T-04` manual UI | Idle→Running→approval→Running→Failed/Idle and hover | status/action replacement exact，three locations synchronized |
| `R-09` | `T-05` manual accessibility | Tab/focus/click/context menu | two direct actions remain focus-visible；no accidental row open |
| `R-14` | `T-06` aggregate | workspace commands + remote CI | all required gates pass |

**Focused validation**

```sh
cargo fmt --all -- --check
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

**Done condition**

所有 owner contracts、自动化、人工 UI 和远端 CI 证据已记录；没有 production diff 越过三 owner、manifest、lockfile、bundle 或 workflow 边界。

## Validation

| R-ID/requirement | Owner/WP | Automated/manual evidence | Expected result | External prerequisite |
| --- | --- | --- | --- | --- |
| `R-05` | core/DB `WP-101/201` | `T-01` recency contract/mutation tests | exact advance/preserve matrix | None |
| `R-06` | DB `WP-201` | `T-02` fresh schema tests | `SCHEMA_VERSION == 1`；0001 non-null recency/index；typed rows decode without fallback | None |
| `R-01`–`R-04`、`R-07` | app `WP-301` | owner pure/GPUI tests + `T-03` | exact card and recency order/time | macOS Jaco window for manual |
| `R-08`–`R-11` | app `WP-302` | runtime/row tests + `T-04/T-05` | status lifecycle and action replacement | local provider/tool flow for manual approval/failure |
| `R-12`–`R-13` | app/root | locale parity + diff audit | keys match；no assets/deps/content pipeline | None |
| `R-14` | root `WP-001` | `T-06` aggregate commands and GitHub checks | local gates + macOS/Linux/Windows CI pass | GitHub CI |

## Completion Evidence

| Evidence | Actual result |
| --- | --- |
| Implementation PR and commits | `88b8b3b`；PR [#209](https://github.com/suxiaoshao/gpui/pull/209) targets `main` |
| Actual added, modified, moved, deleted, generated, synchronized, submodule, and vendored files | production、tests、locales 与四份 owner/root 计划已修改；无 moved/deleted/generated/vendored/submodule |
| Delivered D/F/L/C/DB/ST/R/T/WP IDs | `WP-101/201/301/302` production implementation complete |
| Automated commands and results | `cargo fmt --all -- --check`、`cargo build --workspace --locked`、`cargo test --workspace --locked`、workspace all-targets/all-features clippy 与 diff check 通过；core 43 passed；DB 82 passed |
| Manual, packaged-app, or real-API scenarios and environment | 人工 UI 按用户要求停止；未计为通过 |
| Schema/dependency/generated/vendored diff | fresh `0001` 增加非空 recency/index；无额外 schema version；manifest/lockfile/generated/vendored 无改动 |
| Owner README, index, and ADR updates | Plan/index complete；ADR `None` |
| Accepted deviations and approving decision | `None` |
| Unverified boundaries and reason | 人工 UI 已按用户要求停止；remote CI 未运行 |

## Execution Handoff Audit

- [x] Root hub、三个 same-ID owner plans 与四级 index 路径已定义并双向链接。
- [x] Root owns S/C/ERR/shared D/R/T/WP；owner plans only own local implementation IDs。
- [x] 所有 S-row 已逐项判定且带 current evidence/target decision。
- [x] 产品范围、project/no-project、card content、status/action、recency 已由用户确认。
- [x] Cross-owner recency、fresh schema、runtime authority、status lifecycle、i18n/icons、task ownership 已固定。
- [x] 每个 mutable value 有单一 authority；没有 per-row hover/status/time task。
- [x] Fresh schema、non-null recency 与 write matrix 已固定；不包含旧库兼容工作。
- [x] Automated/manual/CI validation 与 completion evidence 位置已固定。
- [x] 没有待确认产品或架构问题；`WP-101/201/301/302` 已实现，本地自动化通过。
