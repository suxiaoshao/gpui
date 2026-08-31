# Issue #195：Jaco owner implementation plan

## Authority and assignment

- Root authority：[Issue #195 root plan](../../../../../docs/dev/issue-195/README.md)
- Owner：`app/jaco`
- Assigned work packages：`WP-101`–`WP-104`
- Assigned ID ranges：`E/D/F/L/ST/ERR/R/T-1xx`
- Owner readiness：root status为 `Implemented locally`；本文件没有新增待确认产品或架构问题
- Implementation evidence：2026-08-31 已完成 `WP-101`–`WP-104`、owner tests 和本地 aggregate gate

本 owner 计划只解释 Jaco 内部如何执行 root `D-01`–`D-10`、`C-01`–`C-02`、`ST-01`–`ST-02`、`ERR-01`–`ERR-05` 和 `R-01`–`R-14`。产品范围、Audio compatibility、跨 crate 兼容策略和 root completion gate 以根计划为唯一 authority。

## Owner evidence

| E-ID | Current fact | Evidence | Implementation consequence |
| --- | --- | --- | --- |
| `E-101` | `ConversationDetailPage` 持有 `timeline_rows` 与 `Vec<MessageTextState>`；key 目前只有 entry ID | `src/components/chat/detail.rs::{ConversationDetailPage,MessageTextState,sync_message_text_states}` | 在同一 owner 扩展 block identity与access state，不迁移到 row/Global |
| `E-102` | `attachments.rs` 只解析 User Image，Image path helper可复用当前 preview | `detail/attachments.rs::{user_image_attachments,render_user_image_attachments,attachment_path}` | 替换为通用 message projection，同时保留 image record/path/preview视觉逻辑 |
| `E-103` | User row先显示全量 image strip，再显示合并 markdown | `detail/message.rs::UserMessageRow::render` | 改为遍历 ordered blocks，role shell和action row保持 |
| `E-104` | Agent final只用 `agent_final_markdown`；expanded entry统一走 `DetailBlock` | `detail/message.rs::{AgentTurnRow::render,render_details}` | Message entry走共享 renderer；non-Message/tool lifecycle继续原路径 |
| `E-105` | `build_rows` 用 attachment ID map只建 User images；`update_entry` 有精确 row替换 | `detail/timeline.rs::{build_rows,ConversationTimelineRows::update_entry}` | build/update使用同一 pure projection并传新action callback |
| `E-106` | AttachmentChanged已经定位所有 Image/File/Audio/Attachment引用 | `detail.rs::{sync_attachment_rows,entry_references_attachment}` | probe/action state完成后复用此路径更新、remeasure引用 rows |
| `E-107` | 当前 managed write目录硬编码为 `data_dir/attachments/conversation_id` | `features/conversation/attachments.rs::prepare_message_attachments_in` | 抽取单一 shared helper，resolver使用当前 DatabaseTarget根 |
| `E-108` | `DatabaseTarget.data_dir` 是 production/test当前打开数据库的真实根 | `src/database.rs::DatabaseTarget`、`features/conversation.rs::conversation_data_dir` | 禁止在resolver直接重新调用 ambient `paths::data_dir()` |
| `E-109` | `foundation::persistence` 有 cross-platform staged persist；`atomic_replace`接收完整 bytes | `foundation/persistence.rs` | 增加 streaming file-to-file helper，复用 persist/dir sync |
| `E-110` | GPUI test platform可模拟 new-path picker，但 `reveal_path` test implementation不可直接调用 | locked GPUI `platform/test/platform.rs` | actions通过 injected dispatch sink测 guard；picker单独做 GPUI state test |
| `E-111` | Jaco Button已使用 `.ghost().xsmall().icon().tooltip().disabled()`；window task retention已存在 | `detail/copy_button.rs`、`app/tasks.rs` | attachment action buttons与task owner复用相同模式 |
| `E-112` | File/Paperclip/CircleAlert/ExternalLink/FolderOpen存在，Lucide download asset存在 | `foundation/assets.rs`、`third_party/lucide/icons/download.svg` | 仅声明 Download variant；File和Attachment分别用File/Paperclip |

## Owner-local decisions

| D-ID | Decision | Root authority / evidence | Consequence |
| --- | --- | --- | --- |
| `D-101` | pure `project_message_content` 不持有 Entity、Path或callbacks；它只产生 safe blocks | root `C-01`、`D-06`；`E-101`–`E-105` | projection可用普通 Rust table tests覆盖 |
| `D-102` | Text和Image只在 payload中连续时成run；run key用第一个 part index | root `D-01`、`ST-01` | 相邻内容保持旧视觉密度，跨附件保持精确顺序 |
| `D-103` | Agent Message projection按 entry ID存入 row；primary final/loose Message与expanded非-final Message共用 renderer | root `D-05`；`E-104`–`E-105` | tool/status/error/Reasoning仍走现有 DetailBlock/formatter |
| `D-104` | filesystem resolver独立于renderer；renderer只接收 `AttachmentAccessView` | root `C-02`、`D-06`–`D-08` | raw locator无法通过 element closure或Debug意外显示 |
| `D-105` | availability采用单个 page-owned batch probe + generation；动作采用 window-retained task + `(attachment_id, action)` 去重 | root `ST-02`；`E-106`、`E-111` | row没有Task；多文件timeline没有per-card长期owner |
| `D-106` | managed path helper归 `features::conversation::attachments`；atomic copy归 `foundation::persistence` | `E-107`–`E-109` | directory规则和durable write各只有一个authority |
| `D-107` | file card使用普通GPUI layout与gpui-component primitives，不创建共享Card abstraction | root `D-10`；`E-111`–`E-112` | 所有视觉改动留在detail/attachments.rs |

## File and ownership tree

```text
app/jaco/
├── src/
│   ├── components/chat/
│   │   ├── detail.rs                               # F-101 [Modify] state owner, probes, actions, text reconciliation
│   │   └── detail/
│   │       ├── attachment_access.rs                # F-102 [Add] trusted resolver, view state, sanitization/tests
│   │       ├── attachments.rs                      # F-103 [Modify] ordered projection, image/file card renderer/tests
│   │       ├── message.rs                          # F-104 [Modify] shared block rendering in User/Assistant shells/tests
│   │       └── timeline.rs                         # F-105 [Modify] projection wiring, callbacks, incremental update/tests
│   ├── features/conversation/attachments.rs        # F-106 [Modify] shared managed attachment directory helper/tests
│   └── foundation/
│       ├── persistence.rs                          # F-107 [Modify] streaming atomic_copy_file/tests
│       ├── assets.rs                               # F-108 [Modify] Download typed Lucide variant
│       └── i18n.rs                                 # F-109 [Modify] attachment key parity assertions
├── locales/
│   ├── en-US/main.ftl                              # F-110 [Modify] conversation attachment strings
│   └── zh-CN/main.ftl                              # F-111 [Modify] conversation attachment strings
└── docs/dev/
    ├── README.md                                   # F-112 [Modify] owner index
    └── issue-195/README.md                         # F-113 [Add] this plan
```

Explicit unchanged：

```text
crates/jaco-core/**
crates/jaco-db/**
crates/jaco-agent/**
app/jaco/src/foundation/conversation_format.rs
app/jaco/src/components/chat/input/**
app/jaco/src/components/chat/form.rs
app/jaco/src/components/chat/image_preview.rs
app/jaco/Cargo.toml
Cargo.toml
Cargo.lock
crates/{platform-ext,window-ext}/**
.github/workflows/**
```

## L-101：Presentation identity and blocks

`F-103` owns the app-local types below. Names may only change during implementation if all semantics and tests remain identical.

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum TimelineTextKey {
    WholeEntry(ConversationEntryId),
    MessageBlock {
        entry_id: ConversationEntryId,
        start_part_index: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttachmentCardKind {
    File,
    Attachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PersistedAttachmentCard {
    pub(super) attachment_id: AttachmentId,
    pub(super) kind: AttachmentCardKind,
    pub(super) display_name: String,
    pub(super) mime_type: Option<String>,
    pub(super) size_label: Option<String>,
    pub(super) source_hint: AttachmentSourceHint,
    pub(super) static_problem: Option<AttachmentAccessProblem>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum MessageContentBlock {
    Text {
        start_part_index: usize,
        markdown: String,
    },
    Images {
        start_part_index: usize,
        attachments: Vec<PersistedImageAttachment>,
    },
    File(PersistedAttachmentCard),
    Attachment(PersistedAttachmentCard),
}
```

Invariants：

- enum中没有 Audio variant。
- `PersistedAttachmentCard` 不含 Path、URI、provider IDs、record clone、callback或 Entity。
- `start_part_index` 来源为 payload index，元素 ID 使用 `entry_id + start_part_index`；card动作 ID使用 stable attachment ID。
- `display_name`/MIME/size在 projection时完成安全化；renderer不接触原始 record。
- `WholeEntry` 只服务现有 non-Message markdown entry；Message永远使用 `MessageBlock`。

## L-102：Ordered projection algorithm

```rust
pub(super) fn project_message_content(
    entry: &ConversationEntry,
    attachments: &HashMap<AttachmentId, ConversationAttachment>,
) -> Vec<MessageContentBlock>;
```

Algorithm：

1. 只接受 `ConversationEntryPayload::Message`；其他 payload返回空。
2. `enumerate()` 遍历 source `content`，禁止先筛选attachments再拼接。
3. 相邻 Text进入一个 run，原文以 `\n` 连接；遇到任意非Text先flush。
4. 相邻、可解析且record kind为Image的Image进入一个run；遇到invalid Image时flush并沿用当前“该图片不显示”行为。
5. File总是产生一个File card；missing record/kind mismatch也保留同一slot的unavailable card。
6. Audio先转换为 `AttachmentCardKind::Attachment`，接受record kind Audio或Attachment。
7. Attachment产生Attachment card，接受record kind Attachment或Audio。
8. 每次进入不同part类型先flush当前run；output order等于source order。

Exact compatibility table：

| Part | Accepted record kind | Output |
| --- | --- | --- |
| Image | Image | Images run；invalid继续省略 |
| File | File | File card |
| Audio | Audio / Attachment | Attachment card |
| Attachment | Attachment / Audio | Attachment card |
| any attachment part + missing record | none | same File/Attachment slot + `MissingRecord` |
| File + Audio/Attachment/Image record | mismatch | File slot + `KindMismatch` |
| Audio/Attachment + File/Image record | mismatch | Attachment slot + `KindMismatch` |

Image parity boundary：

- 保留当前 `80×80`、8px gap/radius、border/muted background、`ObjectFit::Cover`、pointer cursor、hover border和existing preview dialog。
- 图片仍只在能解析local/generated path且record kind为Image时显示。
- 新变化只涉及它在message中的位置和Assistant coverage；preview dialog本身无 diff。

## L-103 / ST-101：TextViewState reconciliation

`F-101` 保留 page ownership，将当前 entry-only state改成：

```rust
struct MessageTextState {
    key: TimelineTextKey,
    state: Entity<TextViewState>,
    source: String,
    _subscription: Subscription,
}
```

`sync_message_text_states` 和 `sync_message_text_state` 的 exact rules：

1. 对每个非tool-lifecycle entry构建 sources：
   - Message：从 L-102 Text blocks取得 `(MessageBlock key, markdown)`；
   - 其他 payload：保持 `format::item_markdown`，非空时使用 WholeEntry。
2. 对单个Message更新前比较existing/new `MessageBlock` key set：
   - key set改变：先删除该 entry 所有 MessageBlock state/subscription，再按new set建立；
   - key set相同：逐key使用现有 `message_text_update` 的 unchanged/append/replace。
3. 空Text run不创建state；它变为非空时key set改变并安全建立。
4. subscription从key取entry ID，只remeasure包含该entry的row。
5. `message_text_state_map()` 返回 `HashMap<TimelineTextKey, Entity<TextViewState>>`。
6. AttachmentChanged只重投影card；只要Text key set不变，不替换Markdown Entity。

Tests必须证明：

- `Text A → File → Text B` 有两个不同state/ElementId。
- 最后一个Text streaming append只push对应state。
- insert/delete/move part导致key变化时旧states被drop，文字不会串位。
- replace attachment metadata不重建Text states。
- whole-entry status/reasoning/error现有更新路径保持。

## L-104：Shared message renderer and role integration

`F-103/F-104` 提供一个共享入口：

```rust
enum MessageContentAppearance {
    User,
    Assistant,
}

pub(super) fn render_message_content(
    entry_id: &ConversationEntryId,
    blocks: Vec<MessageContentBlock>,
    text_states: &HashMap<TimelineTextKey, Entity<TextViewState>>,
    access: &HashMap<AttachmentId, AttachmentAccessView>,
    appearance: MessageContentAppearance,
    on_attachment_action: OnAttachmentAction,
    cx: &mut App,
) -> AnyElement;
```

Role shells：

| Surface | Message selection | Content appearance | Unchanged behavior |
| --- | --- | --- | --- |
| User row | User Message entry | items_end/max 680；Text block使用当前primary bubble | row right alignment、timestamp、copy button |
| Agent run primary | `run.output.final_entry_id` matching Message | max 760；Assistant Text保持当前plain markdown | status/separator/action row、run expand/collapse |
| Loose Assistant | no-run row中的Message entry | Assistant appearance | row grouping/key、copy/time contract |
| Expanded non-final Agent Message | `AgentDetailItem::Entry(Message)` | Assistant appearance，位于原detail item slot | tool invocation/unresolved/status/error/Reasoning DetailBlock |

Agent implementation rules：

- `primary_item()`返回run final entry；no-run且唯一可见Message时返回loose Message。
- primary Message用L-104并从details过滤；primary Error/Status继续`agent_final_markdown`。
- expanded details遇到 Message走L-104；其余entry继续`DetailBlock::new`。
- `agent_copy_text`、User copy仍调用`item_markdown`，附件名称/metadata不进入copy。
- AgentRun items、tool lifecycle、request usage、approval和row key不改变。

## L-105：Trusted access types and resolver

`F-102` is the only module that may carry a raw local `PathBuf` for these actions.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum AttachmentAction {
    Open,
    Reveal,
    SaveCopy,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct AttachmentActionTarget {
    pub(super) attachment_id: AttachmentId,
    pub(super) kind: AttachmentCardKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttachmentSourceHint {
    Local,
    Generated,
    External,
    Provider,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolvedAttachmentSource {
    Managed,
    Local,
    Generated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttachmentSourceLabel {
    Managed,
    Local,
    Generated,
    External,
    Provider,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AttachmentAccessProblem {
    MissingRecord,
    WrongConversation,
    KindMismatch,
    UnsupportedSource,
    MissingLocator,
    LocatorMismatch,
    MissingFile,
    NotRegularFile,
    UnsafeGeneratedPath,
    Io(std::io::ErrorKind),
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedLocalAttachment {
    attachment_id: AttachmentId,
    path: PathBuf,
    source: ResolvedAttachmentSource,
}

pub(super) enum AttachmentAccessState {
    Checking,
    Available(ResolvedLocalAttachment),
    Unavailable(AttachmentAccessProblem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AttachmentAvailability {
    Checking,
    Available,
    Unavailable(AttachmentAccessProblem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttachmentAccessView {
    pub(super) availability: AttachmentAvailability,
    pub(super) source: AttachmentSourceLabel,
    pub(super) busy_actions: HashSet<AttachmentAction>,
}
```

`AttachmentAccessView` 是 renderer clone；它不包含 path。`AttachmentAccessState` 只由 page持有并传给action orchestrator。

Resolver signature：

```rust
pub(super) fn resolve_local_attachment(
    record: &ConversationAttachment,
    expected_conversation_id: &ConversationId,
    expected_card_kind: AttachmentCardKind,
    database_data_dir: &Path,
) -> Result<ResolvedLocalAttachment, AttachmentAccessProblem>;
```

Source/storage rules：

| storage_kind | metadata.source | Result |
| --- | --- | --- |
| LocalFile | LocalFile | canonical regular file；inside managed root→Managed，outside→Local |
| LocalFile | GeneratedFile | 只在canonical current-conversation managed root内→Generated |
| GeneratedFile | GeneratedFile | 只在canonical current-conversation managed root内→Generated |
| GeneratedFile | LocalFile | `UnsupportedSource` |
| ExternalUri | any | deny-first External unavailable；忽略并绝不打开stale path |
| ProviderFile | any | deny-first Provider unavailable；忽略并绝不打开stale path |
| Local/Generated storage | ExternalUri/ProviderFile metadata | `UnsupportedSource` |

Locator rules：

- `record.path` 与 metadata path trim后都为空→`MissingLocator`。
- 两者都有值→分别canonicalize并要求相等；不相等→`LocatorMismatch`。
- canonicalize NotFound→`MissingFile`；PermissionDenied等→`Io(kind)`。
- metadata必须是regular file；directory/device/socket→`NotRegularFile`。
- managed root使用F-106 + current `DatabaseTarget.data_dir`；canonical candidate containment按path components，symlink escape→`UnsafeGeneratedPath`。
- `record.conversation_id`错误在任何path操作之前返回。
- resolver不写 Jaco log；caller只用typed result记录safe diagnostics。

Safe metadata helpers：

```rust
fn safe_display_name(name: Option<&str>) -> Option<String>;
fn safe_mime_type(mime: Option<&str>) -> Option<String>;
fn format_persisted_size(size: Option<i64>) -> Option<String>;
```

- filename按`/`和`\\`取末段，移除control chars，trim，最多160 Unicode scalar；空值由renderer用localized fallback。
- MIME要求单行、无control、长度≤127且含一个非边界`/`；否则省略。
- size只接受non-negative，按B/KiB/MiB/GiB/TiB格式化；unknown/negative省略。

## ST-102：Probe and action orchestration

`F-101` adds：

```rust
attachment_access: HashMap<AttachmentId, AttachmentAccessState>,
attachment_access_generation: u64,
attachment_probe_task: Option<Task<()>>,
attachment_actions_in_flight: HashSet<(AttachmentActionTarget, AttachmentAction)>,
```

Probe：

1. `sync_attachment_access` 从最新snapshot收集File/Audio/Attachment references并去重。
2. missing record/static External/Provider/mismatch立即写Unavailable；local/generated候选写Checking。
3. generation递增；旧probe task由替换而取消。
4. 将owned record summaries、expected kind、conversation ID、DatabaseTarget.data_dir送入一个background batch。
5. 回写前核对generation；当前结果替换map，调用既有`sync_attachment_rows(id)`和`cx.notify()`。
6. snapshot移除的IDs从access与busy view删除；page drop取消probe。

Actions：

```rust
type OnAttachmentAction = Rc<
    dyn Fn(AttachmentActionTarget, AttachmentAction, &mut Window, &mut App) + 'static,
>;
```

- callback只携带stable ID、File/Attachment kind和action，经page weak entity进入`handle_attachment_action`；kind用于action-time compatibility revalidation。
- 相同`(id, action)`在in-flight时忽略；状态变化只刷新引用该ID的rows。
- action task从最新snapshot复制record，并在background再次执行L-105。
- Open/Reveal成功回到window context后调用GPUI API一次；platform API返回`()`，清busy且无success toast。
- Save成功preflight后调用picker；initial directory使用validated source parent，suggested name使用safe basename；cancel静默。
- picker选择target后在background调用L-108；task由`app::tasks::retain_window`持有，window关闭后取消。
- weak page仍存在时清busy和toast；page已drop时文件task可在window lifetime内完成，不再访问旧row。

TOCTOU boundary：action-time canonicalize/metadata是应用可执行的最后检查；从检查到OS handoff之间的filesystem替换无法完全消除。应用不缓存跨record-update授权，也不把External/Provider locator交给平台。

## L-106：File / Attachment card visual contract

Card root：

```text
h_flex
  stable id: conversation-attachment-card-{entry}-{part}-{attachment}
  width 360px, max-width 100%, min-height 64px
  radius 8px, 1px semantic border, muted background, 8px gap/padding
```

Content：

1. leading icon：File→`IconName::File`；Attachment→`IconName::Paperclip`；无Audio icon。
2. center `v_flex().min_w_0().flex_1()`：
   - safe name，single-line truncate；tooltip只显示同一个safe name；
   - metadata line只拼接存在的 type/MIME/size/source label。
3. trailing：
   - Checking：现有small Spinner + localized status，actions disabled/hidden；
   - Unavailable：CircleAlert + localized safe reason，actions hidden；
   - Available：三个 `Button::new(...).ghost().xsmall()` icon-only actions，localized tooltips；
   - Save busy：Save button disabled/loading；Open/Reveal各自in-flight时禁用对应button。

Action icons：

| Action/state | Icon |
| --- | --- |
| Open | `ExternalLink` |
| Reveal | `FolderOpen` |
| Save copy | new `Download` typed variant |
| Unavailable | `CircleAlert` |

All buttons call `cx.stop_propagation()`；card本身没有click-to-open，避免意外文件动作。颜色只使用theme semantic tokens；User/Assistant共享card视觉，外层alignment由L-104决定。

## L-107：Fluent contract

F-110/F-111 add exactly these keys；F-109 asserts every key exists in both locales.

```text
conversation-attachment-fallback-name
conversation-attachment-type-file
conversation-attachment-type-attachment
conversation-attachment-source-managed
conversation-attachment-source-local
conversation-attachment-source-generated
conversation-attachment-source-external
conversation-attachment-source-provider
conversation-attachment-status-checking
conversation-attachment-status-unavailable
conversation-attachment-unavailable-missing-record
conversation-attachment-unavailable-invalid-record
conversation-attachment-unavailable-source
conversation-attachment-unavailable-missing-file
conversation-attachment-unavailable-access
conversation-attachment-open
conversation-attachment-reveal-macos
conversation-attachment-reveal-windows
conversation-attachment-reveal-linux
conversation-attachment-save-copy
conversation-attachment-action-failed-title
conversation-attachment-action-failed-message
conversation-attachment-save-failed-title
conversation-attachment-save-failed-message
conversation-attachment-save-success-title
conversation-attachment-save-success-message
```

Exact copy：

```ftl
# en-US
conversation-attachment-fallback-name = Attachment
conversation-attachment-type-file = File
conversation-attachment-type-attachment = Attachment
conversation-attachment-source-managed = Managed
conversation-attachment-source-local = Local
conversation-attachment-source-generated = Generated
conversation-attachment-source-external = External
conversation-attachment-source-provider = Provider
conversation-attachment-status-checking = Checking file…
conversation-attachment-status-unavailable = Unavailable
conversation-attachment-unavailable-missing-record = The attachment record is missing.
conversation-attachment-unavailable-invalid-record = The attachment data does not match this message.
conversation-attachment-unavailable-source = This attachment is not available as a local file.
conversation-attachment-unavailable-missing-file = The local file is missing or is not a regular file.
conversation-attachment-unavailable-access = Jaco cannot access this local file.
conversation-attachment-open = Open
conversation-attachment-reveal-macos = Show in Finder
conversation-attachment-reveal-windows = Show in Explorer
conversation-attachment-reveal-linux = Show in File Manager
conversation-attachment-save-copy = Save a copy…
conversation-attachment-action-failed-title = Attachment action failed
conversation-attachment-action-failed-message = Jaco could not access this attachment.
conversation-attachment-save-failed-title = Save failed
conversation-attachment-save-failed-message = Jaco could not save a copy of this attachment.
conversation-attachment-save-success-title = Copy saved
conversation-attachment-save-success-message = Saved a copy of { $name }.

# zh-CN
conversation-attachment-fallback-name = 附件
conversation-attachment-type-file = 文件
conversation-attachment-type-attachment = 附件
conversation-attachment-source-managed = 已管理
conversation-attachment-source-local = 本地
conversation-attachment-source-generated = 已生成
conversation-attachment-source-external = 外部
conversation-attachment-source-provider = 提供商
conversation-attachment-status-checking = 正在检查文件…
conversation-attachment-status-unavailable = 不可用
conversation-attachment-unavailable-missing-record = 找不到这条附件记录。
conversation-attachment-unavailable-invalid-record = 附件数据与这条消息不匹配。
conversation-attachment-unavailable-source = 这个附件没有可用的本地文件。
conversation-attachment-unavailable-missing-file = 本地文件已丢失，或目标类型无法作为普通文件使用。
conversation-attachment-unavailable-access = Jaco 无法访问这个本地文件。
conversation-attachment-open = 打开
conversation-attachment-reveal-macos = 在“访达”中显示
conversation-attachment-reveal-windows = 在文件资源管理器中显示
conversation-attachment-reveal-linux = 在文件管理器中显示
conversation-attachment-save-copy = 存储副本…
conversation-attachment-action-failed-title = 附件操作失败
conversation-attachment-action-failed-message = Jaco 无法访问这个附件。
conversation-attachment-save-failed-title = 存储失败
conversation-attachment-save-failed-message = Jaco 无法存储这个附件的副本。
conversation-attachment-save-success-title = 副本已存储
conversation-attachment-save-success-message = 已存储 { $name } 的副本。
```

Reason mapping：

| Problem | Key |
| --- | --- |
| MissingRecord | `unavailable-missing-record` |
| WrongConversation / KindMismatch / LocatorMismatch / UnsafeGeneratedPath | `unavailable-invalid-record` |
| UnsupportedSource / MissingLocator | `unavailable-source` |
| MissingFile / NotRegularFile | `unavailable-missing-file` |
| Io | `unavailable-access` |

## L-108：Streaming atomic Save copy

`F-107` adds：

```rust
pub(crate) fn atomic_copy_file(source: &Path, target: &Path) -> std::io::Result<u64>;
```

Sequence：

1. target必须有parent；不自动创建picker未选择的目录。
2. 先`File::open(source)`，再在target parent创建`NamedTempFile`；source==target仍读取原file到独立stage。
3. `std::io::copy` streaming，记录bytes；flush + staged file `sync_all()`。
4. 调用现有platform-specific `persist_staged(staged, target, parent)`；Unix随后sync directory，Windows沿用write-through replace。
5. 任何失败由NamedTempFile drop清理stage；replace前目标不变。

Production代码不做full-file `fs::read`，不调用direct `fs::copy(target)`，不新增dependency。

## Upstream/component reuse audit

| Need | Existing authority checked | Decision |
| --- | --- | --- |
| Markdown rendering | `gpui-component::text::{TextView,TextViewState}` | 复用；page继续拥有state/subscription |
| File card structure | composer file card + GPUI layout primitives | 只借用尺寸/层级语义；timeline card在F-103实现，因为包含persisted status/actions |
| Buttons/tooltips | `gpui-component::button::Button` | 复用ghost/xsmall/icon/tooltip/disabled；不自建button |
| Icons | app-local `IconName` + existing Lucide assets | 复用5个既有variant，只补Download declaration |
| Loading/error | existing Spinner、CircleAlert、Notification | 复用；无audio/media state |
| Open/reveal/save picker | locked GPUI App APIs | 复用；无platform-ext/window-ext wrapper |
| Durable file replacement | Jaco `foundation::persistence::persist_staged` | 复用并增加streaming input helper |
| Async task lifetime | `app::tasks::retain_window` + page-owned Task | 复用；row/card不拥有Task |
| Generic Card component | locked gpui-component component index | 没有适合的Card API；使用plain semantic layout |
| HTTP Client response save | app-private staged save flow | 仅作为evidence；不建立cross-app dependency |

## Interaction and accessibility table

| Interaction | Pointer | Keyboard/focus | Busy/error | Propagation |
| --- | --- | --- | --- | --- |
| Image preview | current thumbnail click | current behavior unchanged | invalid image remains absent | existing stop propagation |
| Open | explicit xsmall button | Button focus/activation | duplicate same action disabled；preflight error toast | stop propagation |
| Reveal | explicit xsmall button | platform-local tooltip | duplicate same action disabled；preflight error toast | stop propagation |
| Save copy | explicit xsmall button | localized tooltip | disabled/loading through picker+copy；cancel silent | stop propagation |
| Unavailable | no action buttons | readable icon + text status | safe localized category | no hidden handler |
| Checking | no action dispatch | readable status；no focus trap | Spinner until current generation | no hidden handler |

## Owner requirements

| R-ID | Requirement |
| --- | --- |
| `R-101` | F-103 implements L-101/L-102 with no Path/Entity/callback in pure blocks. |
| `R-102` | F-101 implements ST-101 and preserves append optimization only for stable block keys. |
| `R-103` | F-104/F-105 apply L-104 to User and all applicable Assistant Message positions without changing lifecycle rows. |
| `R-104` | Existing image style/path/preview stays; source position becomes exact and Assistant coverage uses same image contract. |
| `R-105` | F-102 implements full L-105 deny-first/canonical/containment matrix using current DatabaseTarget. |
| `R-106` | F-101 implements ST-102 generation fence, action revalidation, dedupe and targeted row refresh. |
| `R-107` | F-103 implements L-106 with semantic tokens、stable IDs、safe labels and no card-wide action. |
| `R-108` | F-107 implements L-108 atomicity and cross-platform tests without full-file buffering. |
| `R-109` | Open/Reveal dispatch once after preflight and never claim platform completion；Save cancel/error/success follow root ERR. |
| `R-110` | F-108–F-111 provide exact icons/Fluent contract and locale parity；no Audio key/icon. |
| `R-111` | copy/search/agent history/DB producer/schema/serde remain unchanged. |
| `R-112` | Jaco-owned logs/notifications/rendered tree exclude path/URI/provider IDs/credentials and raw records；External/Provider locator never reaches GPUI. |

## Test plan

| T-ID | File/scope | Scenario | Assertions |
| --- | --- | --- | --- |
| `T-101` | F-103 pure projection | `Text A, Image1, Image2, Text B, File, Audio, Attachment, Text C` | exact block order；only consecutive groups；Audio output variant=Attachment |
| `T-102` | F-103 compatibility table | every part/record kind pair + missing record | accepted matrix exact；File/Attachment slot preserved on error；no Audio block |
| `T-103` | F-103 image regression | current image fixtures、invalid path、mixed boundaries | same path/name/size/preview payload；invalid image absent；groups split correctly |
| `T-104` | F-101/ST-101 pure helpers | stable append、replace、part insert/delete/move | correct push/replace/reset decision；unique key perrun |
| `T-105` | F-104/F-105 | User、run final Assistant、loose Assistant、expanded non-final Message | same blocks/card contract；role shell/lifecycle ordering exact |
| `T-106` | F-105 incremental update | entry replace、AttachmentChanged、other row present | onlyreferencing row rebuilt/remeasured；other row/key/Text Entity preserved |
| `T-107` | F-102 resolver table | storage/source matrix、wrong conversation/kind、empty/dual locators | exact typed result；External/Provider deny-first |
| `T-108` | F-102 filesystem | inside/outside managed、missing、directory、canonical mismatch、`..` | Managed/Local/Generated exact；no lexical-prefix bug |
| `T-109` | F-102 platform-specific | symlink escape (`#[cfg(unix)]`) | Generated rejected；portable tests仍coverreal outside path |
| `T-110` | F-102 metadata/privacy | names with `/\\/control`、bad MIME、negative size、secret URI/IDs | safe display values；render/problem/debug capture没有secret locator |
| `T-111` | F-101 GPUI action state | injected open/reveal sink、picker cancel/error、duplicate clicks | guard before dispatch；busy clear；cancel silent；same action once |
| `T-112` | F-107 persistence | create/replace/same source-target/missing source/directory target/large fixture | bytes/length exact；old target survives failure；stage cleaned |
| `T-113` | F-109–F-111 | all26 keys in en-US/zh-CN + `$name` | no fallback-to-key；exact platform reveal labels/successformat |
| `T-114` | existing detail/timeline | pure text、pure image、AgentRun/tool/error/copy | no regression；attachment metadata absent from copy |
| `T-115` | existing jaco-db tests | persisted attachment reload | records/stable IDs/content unchanged；no new migration |

Testing mechanics：

- T-101–T-110、T-112使用 ordinary `#[test]`。
- T-111只用`#[gpui::test]`测试Entity/task/picker状态；Open/Reveal用injected dispatch sink，避免直接触发test platform未实现的reveal API。
- filesystem fixtures使用`tempfile::tempdir`；symlink escape只在可靠平台cfg下运行，不要求Windows developer mode。
- privacy test使用可识别 sentinel URI/token/provider file ID，并断言 rendered safe model、notifications 和 captured Jaco logs 均不包含 sentinel。

## WP-101：Ordered projection and per-block text state

**Prerequisites**

- Root `D-01`–`D-04`、`C-01`、`ST-01`、`R-01`–`R-06`。

**Files**

- `F-101`、`F-103`、`F-105`

**Implementation sequence**

1. Add L-101 types and L-102 pure projection；migrate current image helper/type without changingpreview contract。
2. Replaceentry-level text identity with ST-101；retainwhole-entry path for non-Message payloads。
3. Change timeline build/update signatures to consume `HashMap<TimelineTextKey, Entity<TextViewState>>` and projected blocks。
4. Keep `format::item_markdown` only for copy/search/non-Message rendering。
5. Add T-101–T-104 and projection half of T-106/T-114。

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco --locked components::chat::detail::attachments
cargo test -p jaco --locked components::chat::detail::tests
git diff --check
```

**Done condition**

Mixed content has exact blocks/keys；Audio only appears as Attachment；streaming/structural update cannot cross-wire TextViewState；no cross-owner diff。

## WP-102：User/Assistant shared rendering and image parity

**Prerequisites**

- `WP-101` complete。
- Root `D-05`、`D-10`、`R-03`–`R-06`。

**Files**

- `F-103`–`F-105`

**Implementation sequence**

1. Implement L-104 shared renderer with User/Assistant appearance。
2. Replace User image-first/merged-markdown body with block traversal；preserveouter shell/action row。
3. Route Agent primary/loose/expanded Message entries through L-104；preserve non-Message DetailBlock/tool lifecycle。
4. Generalize image strip outeralignment while retainingexact thumbnail/preview behavior。
5. Add T-103、T-105、T-106、T-114。

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco --locked components::chat::detail::message
cargo test -p jaco --locked components::chat::detail::timeline
git diff --check
```

**Done condition**

All applicableUser/Assistant Message surfaces shareblocks；images look/behave the same；run/tool/copy behavior passesexisting and new tests。

## WP-103：Trusted access, cards, actions, persistence and localization

**Prerequisites**

- `WP-101`–`WP-102` complete。
- Root `C-02`、`ST-02`、`ERR-01`–`ERR-05`、`R-07`–`R-13`。

**Files**

- `F-101`–`F-103`、`F-105`–`F-111`

**Implementation sequence**

1. Add F-106 shared managed-dir helper and migrate existing writer call site without behavior change。
2. Add L-105 pure access resolver/safe metadata helpers and T-107–T-110。
3. Add ST-102 page probe state、generation fence、targeted row refresh and callback wiring。
4. Implement L-106 card and action buttons；declare Download icon；add exact L-107 locale keys/parity test。
5. Add L-108 streaming atomic copy/tests；wire Open/Reveal/Save with action-time revalidation、window retention、busy/toasts。
6. Add T-111–T-113 and privacy/unchanged-scope assertions。

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco --locked components::chat::detail::attachment_access
cargo test -p jaco --locked components::chat::detail
cargo test -p jaco --locked foundation::persistence
cargo test -p jaco --locked foundation::i18n
cargo check -p jaco --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
git diff --check
```

**Unchanged-scope audit**

```sh
git diff --exit-code -- \
  crates/jaco-core \
  crates/jaco-db \
  crates/jaco-agent \
  app/jaco/src/foundation/conversation_format.rs \
  app/jaco/Cargo.toml \
  Cargo.toml \
  Cargo.lock \
  crates/platform-ext \
  crates/window-ext \
  .github/workflows
```

**Done condition**

Every card has safe Available/Checking/Unavailable behavior；only validated local paths reach platform/file APIs；Save atomicity/privacy/i18n/icons pass；no Audio-special artifact or dependency。

## WP-104：Jaco regression and acceptance closure

**Prerequisites**

- `WP-101`–`WP-103` complete and implementation diff frozen for final validation。

**Files**

- Tests colocated in `F-101`–`F-111`；plan evidence in `F-113`。

**Sequence**

1. Run all T-101–T-115 focused commands once against final owner state。
2. Run full Jaco test/check/clippy once；do not stackduplicate gates on unchanged state。
3. Run root manual acceptance with isolated `JACO_CONFIG_DIR` and persisted fixtures。
4. Verify the same database after restart；inspect rendered/copy/Jaco-log outputs for sentinel privacy。
5. Record actual commands/results/known external platform limits in both plans，then hand off to root WP-001。

**Owner gate**

```sh
cargo fmt --all -- --check
cargo test -p jaco --locked --no-fail-fast
cargo check -p jaco --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
cargo test -p jaco-db --locked attachments
git diff --check
```

**Manual scenarios**

1. Mixed User and Assistant fixtures verify exactblock positions、role shells and image preview。
2. Restart with the same isolated data dir verifies stable IDs/cards/availability。
3. Open/Reveal/Save valid Local/Managed/Generated files；cancel/overwrite/failure/busy behavior exact。
4. Missing/invalid/external/provider/unsafe-generated fixtures show safe inline states and no enabled actions。
5. Search/copy/Jaco-log sentinel audit；no path/URI/provider locator appears。
6. Noaudio UI/icon/state/dependency/test naming beyondcompatibility input assertions。

**Done condition**

Owner tests and manual evidence satisfyR-101–R-112；owner file list/unchanged audit clean；root WP-001 can run aggregate gates without further implementation decision。

## Completion evidence

| Evidence | Actual result |
| --- | --- |
| Owner implementation plan | `Implemented locally`；F/L/ST/R/T/WP 与实际实现/验证一致 |
| WP-101 | Complete；source-ordered blocks、stable per-block TextViewState、structural reconciliation 与 Audio→Attachment compatibility tests 已实现 |
| WP-102 | Complete；User、Assistant primary/loose/expanded Message 共用 renderer，non-Message/tool/status/error 与 text-only copy 保持 |
| WP-103 | Complete；deny-first resolver、safe card metadata、background probes、action-time revalidation、Open/Reveal/Save、atomic streaming copy、icons/i18n 已实现 |
| WP-104 focused automated validation | Passed；`cargo test -p jaco --locked --no-fail-fast` 为 575 passed、2 ignored；Jaco clippy、jaco-db attachment regression、fmt 和 diff checks 通过 |
| Manual restart/actions/privacy/image parity | Persisted fixture 重新打开并验证 exact order、User/Assistant、Image、Audio→Attachment、External/Provider unavailable、Save cancel/success byte equality、Open/Reveal dispatch；缓存指纹最终修正后未再启动 GUI，新增 GPUI test验证同 ID kind conflict 消失时会重新探测 |
| Unchanged core/db/agent/dependency audit | Passed；core/db/agent、conversation formatter、manifests、lockfile、platform crates 和 workflows 无 diff |
| Root WP-001 / remote CI | Local aggregate passed；remote macOS/Linux/Windows CI、commit 和 PR pending |

Final bundle：`cargo run -p xtask -- bundle jaco` 成功生成
`target/release/bundle/macos/Jaco.app`；受限环境的 Liquid Glass `actool` 步骤失败后，
xtask 按现有 fallback 保留普通图标并成功完成 bundle。

## Implementation handoff checklist

- [x] Exact code/document file set与unchanged paths已定义。
- [x] Pure projection、block identity、Assistant coverage与image parity已定义。
- [x] Source/storage/path matrix、generation/busy/task owner与atomic Save已定义。
- [x] Component/icon/i18n/accessibility/privacy/error/test contracts已定义。
- [x] Implement WP-101。
- [x] Implement WP-102。
- [x] Implement WP-103。
- [x] Freeze final owner state and execute WP-104。
- [x] Update completion evidence and hand off to root WP-001。
