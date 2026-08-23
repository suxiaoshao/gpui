# Jaco：Issue #193 侧边栏 HoverCard、相对活动时间与会话状态

## Root hub and ownership

- Plan ID：`issue-193`
- Root status：`In progress`
- Root hub：[Issue #193 root hub](../../../../../docs/dev/issue-193/README.md)
- Owner directory：`app/jaco`
- Owner plan：`app/jaco/docs/dev/issue-193/README.md`
- Owner index：[Jaco 开发计划](../README.md)
- Root-owned IDs consumed：`S-01`–`S-06`、`S-08`–`S-09`、`S-12`–`S-19`、`D-01`–`D-08`、`C-01`–`C-02`、`ST-01`、`R-01`–`R-14`
- Owner-authored local IDs/ranges：`E/F/L/ST/R/T/WP-3xx`
- Assigned WPs：`WP-301`、`WP-302`
- Owns：catalog/workspace recency projection、project-only HoverCard、relative formatter/clock/i18n、runtime sidebar status authority、row suffix/action switch、Jaco tests/manual matrix
- Does not own：SQLite schema/write timing、domain C-01、GitHub integration、message preview、provider/Rig/MCP、existing Pin/Archive mutation semantics

## Owner-local evidence

| E-ID | Claim | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-301` | catalog 与 workspace 都按 summary.updated_at 排序 | `src/features/conversation/registry.rs::sort_catalog`、`src/features/home/workspace.rs` | 全链路切到 C-01 recency |
| `E-302` | one node clone到 project/no-project/pinned | `workspace.rs::build_sidebar_snapshot` | project name/recency只投影一次，三处不会分叉 |
| `E-303` | normal project与scratch project已在 workspace build时分类 | `workspace.rs::build_sidebar_snapshot` | normal node带 project display name；scratch node为 None且不建 card |
| `E-304` | conversation row已有 stable row/action IDs、group hover、focus-visible Pin/Archive、stop propagation | `home/sidebar/row.rs::ConversationSidebarRow` | status使用同 suffix；actions/handlers保持 |
| `E-305` | `HomeView`已持有 runtime，`HomeSidebar`当前只持 workspace | `home/shell.rs::HomeView::new`、`home/sidebar.rs::HomeSidebar` | 显式把同一 runtime Entity传给 sidebar并observe |
| `E-306` | runtime拥有 active attempts、ActiveRunKey、approval broker、last_errors、recovery | `features/conversation/runtime.rs` | 扩展现有 authority，不建第二状态源 |
| `E-307` | pending approval每次增减都会 publication + notify；broker目前只能查单 invocation | `runtime/approval.rs` | 增加 current conversation/run聚合查询 |
| `E-308` | `finish_run`能读取完整 `AgentRunHandle.agent_run.status`；recovery返回 recovered records | `runtime.rs::finish_run`、`jaco_agent::AgentRuntime::recover_interrupted_runs` | terminal/recovery status不依赖忽略中的 event |
| `E-309` | `take_last_error`当前 remove map entry | `runtime.rs::take_last_error` | 通知消费与 Failed marker生命周期必须分开 |
| `E-310` | locked HoverCard使用 stable keyed state并自持 600/300ms tasks | `gpui-component@57a9903` `hover_card.rs` | wrapper提供 trigger/content/id，并只测量 trigger bounds 来定位整个 popover root |
| `E-311` | Folder、ShieldAlert、CircleAlert、Spinner与双 locale已存在 | `foundation/assets.rs`、`locales/*/main.ftl` | 不改 assets，只增 keys |

## Owner-local decisions

| D-ID | Decision | Evidence | Consequence |
| --- | --- | --- | --- |
| `D-301` | `WorkspaceConversationInput`/`SidebarConversationNode`用 typed `OffsetDateTime recency_at`，删除 conversation-node updated_at | C-01、`E-301` | formatter无需 nanos reparse/fallback |
| `D-302` | node携带 `project_display_name: Option<SharedString>`：normal project Some，scratch None；pinned clone保留同值 | root `D-01`、`E-302`–`E-303` | row可纯判定是否建 HoverCard |
| `D-303` | HomeSidebar显式持有/observe runtime并持一个 minute redraw Task；row只读 runtime query | root `D-04`–`D-06`、`E-305` | 无 HomeWorkspace/status snapshot cache，无 per-row Task |
| `D-304` | runtime failure record分离 `failed marker` 与 `pending notification`；take只消费 notification | `E-306`、`E-309` | notification后 sidebar仍为 Failed |
| `D-305` | terminal status以 `finish_run` handle为准；approval以 broker pending aggregate为准；ActiveRunKey仍是 stale fence | `E-307`–`E-308` | 不依赖历史 entry或无 conversation-id 的 status event |
| `D-306` | HoverCard使用 default appearance/delays/padding，固定 320px宽、full-title normal whitespace、relative label nowrap、project row normal whitespace；whole popover root 对齐 sidebar 右边缘和 trigger 顶部 | root `D-01`–`D-02`、`E-310` | exact两行内容，长标题不截断且定位不裁切 |
| `D-307` | special status slot与 action overlay共用 56px trailing rail；hover淡出status并显示两个action；Idle不创建status element | root `D-07`、`E-304` | three sections一致，现有 actions不变 |
| `D-308` | relative formatter严格实现 root D-04且由调用方注入 now；六单位和三个status tooltip使用 Fluent | root `D-04`、`E-311` | deterministic双 locale tests |

## Owner-local target design

### File and ownership tree

```text
app/jaco/
├── src/
│   ├── foundation/conversation_format.rs           # F-301 [Modify] relative formatter/tests
│   ├── features/conversation/registry.rs            # F-302 [Modify] catalog recency sorting/tests
│   ├── features/conversation/runtime.rs             # F-303 [Modify] status/failure/recovery authority
│   ├── features/conversation/runtime/approval.rs    # F-304 [Modify] pending aggregate query/tests
│   ├── features/home/shell.rs                       # F-305 [Modify] pass runtime to sidebar
│   ├── features/home/workspace.rs                   # F-306 [Modify] recency/project-name projection/sort
│   ├── features/home/sidebar.rs                     # F-307 [Modify] runtime observer/minute task/pass-through
│   ├── features/home/sidebar/row.rs                 # F-308 [Modify] HoverCard + status/action suffix
│   └── features/home/sidebar/search.rs              # F-309 [Modify] node fixture/construction parity
├── locales/
│   ├── en-US/main.ftl                               # F-310 [Modify] recency/status keys
│   └── zh-CN/main.ftl                               # F-311 [Modify] recency/status keys
└── docs/dev/
    ├── README.md                                    # F-312 [Modify] owner index
    └── issue-193/README.md                          # F-313 [Add] this plan
```

Explicit unchanged：`foundation/assets.rs`、sidebar actions/menu mutation code、conversation detail UI、Cargo manifests、`Cargo.lock`、bundle assets/workflows。No new Rust module/file is required。

### L-301：Recency and project presentation node

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceConversationInput {
    id: ConversationId,
    project_id: ProjectId,
    title: String,
    pinned: bool,
    status: ConversationStatus,
    recency_at: OffsetDateTime,
    deleted_at: Option<i128>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarConversationNode {
    pub(crate) id: ConversationId,
    pub(crate) project_id: ProjectId,
    pub(crate) title: SharedString,
    pub(crate) recency_at: OffsetDateTime,
    pub(crate) project_display_name: Option<SharedString>,
    pub(crate) pinned: bool,
}
```

Projection：

1. `workspace_conversation_input` copies C-01 recency exactly。
2. Build a normal-project ID→display-name map from the existing project selection。
3. Normal conversation node receives Some(name)；scratch/no-project receives None。
4. Project/no-project/pinned all sort `recency_at DESC, id ASC`；pinned clone preserves every field。
5. Search constructor populates the same node fields from its summary/project result，but search UI does not add preview/status content。

### L-302：Compact relative recency formatter

```rust
pub(crate) fn sidebar_relative_recency_label(
    recency_at: OffsetDateTime,
    now: OffsetDateTime,
    i18n: &I18n,
) -> String;
```

```text
elapsed = max(now - recency_at, 0)
minutes = floor(seconds / 60)
if minutes < 60       → minutes
else if hours < 24    → hours
else if days < 7      → days
else if days < 30     → floor(days / 7) weeks
else if days < 365    → floor(days / 30) months
else                  → floor(days / 365) years
```

Fluent keys（both locales, `$value` integer）：

```text
sidebar-conversation-recency-minutes
sidebar-conversation-recency-hours
sidebar-conversation-recency-days
sidebar-conversation-recency-weeks
sidebar-conversation-recency-months
sidebar-conversation-recency-years
sidebar-conversation-status-running
sidebar-conversation-status-awaiting-approval
sidebar-conversation-status-failed
```

English remains compact (`0m`, `23h`, `6d`, `4w`, `11mo`, `2y`)；Chinese uses compact localized units (`0 分钟`, `23 小时`, `6 天`, `4 周`, `11 个月`, `2 年`)。Idle has no key/node。

### L-303 / ST-301：Runtime sidebar status authority

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConversationSidebarStatus {
    Idle,
    Running,
    AwaitingApproval,
    Failed,
}

struct ConversationFailure {
    pending_notification: Option<String>,
}

pub(crate) struct ConversationRuntimeStore {
    // existing fields
    failures: HashMap<ConversationId, ConversationFailure>,
}

impl ConversationRuntimeStore {
    pub(crate) fn sidebar_status(
        &self,
        conversation_id: &ConversationId,
    ) -> ConversationSidebarStatus;

    pub(crate) fn take_last_error(
        &mut self,
        conversation_id: &ConversationId,
    ) -> Option<String>;
}

impl ConversationApprovalBroker {
    pub(super) fn has_pending_for_run(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
    ) -> bool;
}
```

`sidebar_status` exact priority：

```text
Running/Stopping active with matching agent_run_id and broker pending > AwaitingApproval
any Submitting/Running/Stopping attempt                               > Running
no active attempt + failures contains conversation                    > Failed
otherwise                                                             > Idle
```

Failure lifecycle：

- Accepted submission inserts Submitting after removing prior failure。
- Submission failure、launch failure、outer run error、stop finalization error set marker and existing user-notification string where applicable。
- `Ok(AgentRunHandle)` with `AgentRunStatus::Failed` sets marker；Completed/Canceled removes marker。
- `take_last_error` takes only `pending_notification`; map entry remains as marker。
- Last approval resolved while run remains active returns Running；deny itself does not set failure。
- Archive cleanup removes marker and active authority for archived IDs。
- Successful recovery maps every returned interrupted record to marker without restoring approval；recovery operation failure sets no per-conversation marker。
- Entity retirement/restart naturally drops ordinary markers；only interrupted records returned by the new recovery become Failed in the new app session。
- Every mutation calls `cx.notify()` only after accepted ActiveRunKey/current-attempt checks。

### L-304 / ST-302：HomeSidebar observation and minute clock

```rust
pub(crate) struct HomeSidebar {
    workspace: Entity<HomeWorkspace>,
    runtime: Entity<ConversationRuntimeStore>,
    _subscriptions: Vec<Subscription>,
    _relative_clock_task: Task<()>,
}

impl HomeSidebar {
    pub(crate) fn new(
        workspace: Entity<HomeWorkspace>,
        runtime: Entity<ConversationRuntimeStore>,
        cx: &mut Context<Self>,
    ) -> Self;
}
```

- Observe runtime and call `cx.notify()`; do not rebuild/copy status into workspace。
- One loop waits 60 seconds on background executor and notifies HomeSidebar；dropping sidebar cancels the owned task。
- Pass runtime Entity explicitly through pinned/project/no-project row builders。
- Row render queries `runtime.read(cx).sidebar_status(id)`；no global lookup or row-local cache。

### L-305：Project-only HoverCard

```rust
fn conversation_hover_card(
    conversation: &SidebarConversationNode,
    trigger: AnyElement,
    sidebar_width: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement;
```

Behavior：

- `project_display_name == None` returns trigger unchanged。
- Some(name) wraps trigger in a conversation-ID-stable `HoverCard` with default 600ms/300ms and default appearance。
- A keyed `Bounds<Pixels>` measurement positions the entire popover root at sidebar right + 4px and the trigger top；it does not replace HoverCard's hover state/timers or move inner card content。
- Content is exactly `v_flex().w(px(320.)).gap_2()` and relies on HoverCard's single default padding layer：
  1. top `h_flex().items_start()` with full title (`whitespace_normal`, flex-1) and L-302 label (`whitespace_nowrap`, muted)；
  2. second `h_flex().items_center()` with existing `IconName::Folder` and project display name (`whitespace_normal`)。
- Content closure calculates current `OffsetDateTime::now_utc()` at render；minute task keeps an open card within one minute of threshold change。
- No status、message、attachment、GitHub、computer icon or extra row。

### L-306：Trailing status/action rail

```rust
fn conversation_status_slot(
    status: ConversationSidebarStatus,
    group: SharedString,
    cx: &mut App,
) -> Option<AnyElement>;
```

| Status | Normal suffix | Tooltip | Hover |
| --- | --- | --- | --- |
| Idle | no element | none | Pin/Unpin + Archive |
| Running | `Spinner::new().small()` | running key | Pin/Unpin + Archive |
| AwaitingApproval | `IconName::ShieldAlert`, warning semantic color | approval key | Pin/Unpin + Archive |
| Failed | `IconName::CircleAlert`, danger semantic color | failed key | Pin/Unpin + Archive |

- Status slot and existing `action_overlay(ACTION_SUFFIX_WIDTH)` occupy the same trailing rail。
- Special status gives title permanent suffix padding；Idle gains padding on hover as today。
- Status slot uses group-hover opacity 0；existing buttons use group-hover/focus-visible opacity 1 and keep stable IDs/handlers/stop propagation。
- Row click、context menu、active/hover background and project nesting remain unchanged。

## Owner-local requirements

| R-ID | Requirement |
| --- | --- |
| `R-301` | C-01 recency and normal-project name project exactly once into the shared node and all three sidebar sections. |
| `R-302` | Catalog/workspace ordering and relative formatter use recency only，with deterministic tie-break/thresholds. |
| `R-303` | HoverCard inclusion/content/long-text/default-delay rules match L-305. |
| `R-304` | Runtime status/failure/approval/recovery follows ST-301 with one authority. |
| `R-305` | HomeSidebar owns one observation/clock lifecycle；rows own no status/time/task authority，only measured trigger bounds required for popover placement. |
| `R-306` | Status/action rail follows L-306 across pinned/project/no-project and preserves action accessibility/propagation. |
| `R-307` | Nine Fluent keys have en-US/zh-CN parity；no icon/assets/dependency change. |

## WP-301：Project recency and exact HoverCard

**Owner**

`app/jaco`。

**Prerequisites and contracts**

- `WP-101`、`WP-201` complete。
- Root `D-01`–`D-04`、`D-08`、`C-01`–`C-02`、`R-01`–`R-07`、`R-12`–`R-13`。

**File IDs**

- `F-301`–`F-302`、`F-305`–`F-311`

**Implementation sequence**

1. Switch catalog/workspace/search node projection and sorting to L-301 recency/project name。
2. Implement L-302 pure formatter and paired Fluent keys/tests。
3. Pass runtime to HomeSidebar while adding only the minute redraw half of L-304；status observation completes in WP-302。
4. Wrap only Some(project) rows with L-305 native HoverCard；keep same row as trigger。
5. Add T-301–T-306，run focused tests and unchanged asset/dependency audit。

**Tests**

| Root R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-04` | `T-301` `registry.rs` | updated_at conflicts with recency/equal IDs | recency DESC、ID ASC |
| `R-01`–`R-04` | `T-302` `workspace.rs` | normal + scratch + pinned clones | project name Some/None；same node fields；recency order |
| `R-07` | `T-303` `conversation_format.rs` | 0/59m/60m/23h/24h/6d/7d/29d/30d/364d/365d/future | exact six-unit floors/clamp |
| `R-07`、`R-12` | `T-304` formatter/i18n | en-US and zh-CN | exact labels and key parity |
| `R-01`–`R-03` | `T-305` `row.rs` pure projection | Some/None project and long title | HoverCard decision；exact two content rows/no extras |
| `R-11` | `T-306` `sidebar.rs` GPUI task lifecycle | one sidebar with many rows, timer tick/drop | one redraw task；drop cancels；no row tasks |

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco --lib foundation::conversation_format
cargo test -p jaco --lib features::conversation::registry
cargo test -p jaco --lib features::home::workspace
cargo test -p jaco --lib features::home::sidebar
cargo clippy -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

**Done condition**

T-301–T-306 pass；normal project/pinned use exact card，scratch/no-project no card；all app ordering/time uses C-01 recency；no message/GitHub/asset/dependency diff。

## WP-302：Runtime status projection and row suffix switch

**Owner**

`app/jaco`。

**Prerequisites and contracts**

- Root `D-05`–`D-07`、`ST-01`、`R-08`–`R-13`。
- Existing ActiveRunKey、archive fence、Pin/Archive action contracts remain authoritative。

**File IDs**

- `F-303`–`F-305`、`F-307`–`F-308`、`F-310`–`F-311`

**Implementation sequence**

1. Add L-303 status/failure record and approval aggregate；replace remove-on-read last_errors behavior with one-shot notification field。
2. Apply ST-301 transitions to accepted submission、submission/launch/outer/stop failures、terminal handle、archive and recovery returned records。
3. Complete L-304 runtime observation and explicit pass-through；row queries authority by ID。
4. Add L-306 status slot beneath the unchanged action overlay and status tooltip keys。
5. Add T-307–T-314；retain existing stale publication、recovery、direct action/order tests。

**Failure and lifecycle behavior**

- Runtime errors keep their existing notification/tracing path；sidebar stores only marker identity, never error details。
- Stale run key、archive fence or dropped runtime cannot change a newer row。
- Approval lock poisoning continues existing broker recovery behavior；the aggregate reads through the same guard。
- Sidebar/window drop cancels observation/clock；runtime shutdown owns active-run cancellation as today。

**Tests**

| Root R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-08`、`R-10` | `T-307` `runtime.rs` | accepted Submitting→Running→Completed/Canceled | Running then Idle |
| `R-08`、`R-10` | `T-308` `runtime/approval.rs` + runtime | one/multiple approvals resolve sequentially | Awaiting until last，then Running |
| `R-10` | `T-309` `runtime.rs` | deny then run continues/terminal fails | deny→Running；only terminal fail→Failed |
| `R-10` | `T-310` `runtime.rs` | submission/launch/outer/stop failure | marker Failed；notification delivered at most once |
| `R-10` | `T-311` `runtime.rs` | Failed then next accepted submission | marker cleared before Running |
| `R-10` | `T-312` `runtime.rs` | startup recovers Running/waiting approval | Failed marker；no actionable/pending approval |
| `R-10`–`R-11` | `T-313` existing runtime tests | stale run key/archive/shutdown | no stale status publication/regression |
| `R-08`–`R-09` | `T-314` `row.rs` | four statuses + group hover/focus actions | Idle empty；icons/spinner exact；hover two actions only；stable IDs |

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco --lib features::conversation::runtime
cargo test -p jaco --lib features::home::sidebar
cargo test -p jaco --lib features::home::workspace
cargo test -p jaco
cargo clippy -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

Unchanged scope audit：

```sh
git diff --exit-code -- \
  app/jaco/src/foundation/assets.rs \
  app/jaco/Cargo.toml \
  Cargo.toml \
  Cargo.lock \
  .github/workflows
```

**Done condition**

T-307–T-314 and existing runtime/action regressions pass；three sidebar locations同步显示 status/hover actions；ordinary restart status/recovery semantics match ST-301；no second status authority。

## Manual UI acceptance

1. Project conversation with a long title：row remains truncated；hover card opens after default delay and shows full title + relative time, then Folder + project name。
2. The same pinned project conversation shows identical card/time/status；renaming/pinning does not change ordering/time。
3. Scratch/no-project conversation and pinned clone show no card。
4. Idle suffix empty；submit shows Running；tool approval shows AwaitingApproval；resolve returns Running；complete/cancel returns empty。
5. Failure shows Failed after any one-shot notification；next submission clears it；restart drops ordinary marker。
6. Force restart during run/approval；recovery produces Failed and no approval remains actionable。
7. Hover each Idle/special row in all sections；suffix always becomes only Pin/Unpin + Archive and each click does not open the row。
8. Keyboard focus still exposes and activates both direct buttons；context menu and row open continue working。
9. Keep card open across a minute threshold；label updates within one minute。
10. Confirm no GitHub/message/computer/status content inside the card and no new asset/dependency behavior。

## Completion evidence

| Evidence | Actual result |
| --- | --- |
| Production diff | recency projection/排序、project-only HoverCard、相对时间、runtime status 与 trailing rail 已实现 |
| Focused/owner commands | sidebar row focused tests：3 passed；workspace build/test、all-targets/all-features clippy、fmt 与 diff check 通过 |
| Manual UI matrix | 按用户要求停止，未计为通过 |
| Delivered local IDs | `WP-301/WP-302` production implementation complete |
| Unchanged asset/dependency audit | 无新 asset/dependency；manifest 与 lockfile 未改 |
| Deviations | `None` |
