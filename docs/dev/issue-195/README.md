# Issue #195：Jaco 会话时间线持久化文件附件

## 状态与范围

- 状态：`Implemented locally`；本地实现、自动化与已授权人工场景完成，远端 CI、commit 和 PR 待执行
- 关联 issue：[#195](https://github.com/suxiaoshao/gpui/issues/195)
- Parent：[#159](https://github.com/suxiaoshao/gpui/issues/159)
- Plan ID：`issue-195`
- 根索引：[Workspace development plans](../README.md)
- 分支：`codex/195-jaco-render-persisted-attachments`
- 基线：`origin/main@184772e0f0a26de4a69e40151e65781d130361b7`
- 受影响 owner：`app/jaco`
- 实施状态：`WP-101`–`WP-104` 与 `WP-001` 本地门禁已完成；远端发布门尚未执行
- 最近证据刷新：2026-08-31

### 高影响变更摘要

| 审计门 | 结果 | Canonical IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | `[App-only]` 只修改 `app/jaco`；新增一个 app-local attachment access 模块，不新增 crate | `D-01`、`D-09`、`C-01`–`C-02`、`WP-101`–`WP-104` |
| Public or cross-owner contracts | `None`；`jaco-core`、`jaco-db`、`jaco-agent` 的类型、serde、SQL 和 runtime contract 全部保持 | `D-02`–`D-03`、`R-11`–`R-12` |
| Global/shared authority | `[Modify app view state]` `ConversationDetailPage` 继续独占 timeline、Markdown state 和附件动作状态；不增加 Global/Store | `D-04`、`D-07`、`ST-01`–`ST-02` |
| Persistence, data, configuration, or credentials | `[File output only]` 数据库与 payload 零变更；Save a copy 使用 staged atomic copy 写入用户选择的目标 | `D-03`、`D-07`、`C-02`、`ERR-04`–`ERR-05` |
| Runtime, concurrency, performance, or shutdown | `[Modify]` 文件 availability 在后台探测，点击动作重新校验；过期结果用 generation fence 丢弃，窗口关闭取消 retained tasks | `ST-02`、`R-07`–`R-09` |
| Security, privacy, or external access | `[Modify]` Provider/External locator 永不交给 OS；UI、通知和 Jaco-owned logs 不输出路径、URI、provider locator 或凭据 | `D-06`、`D-08`、`C-02`、`ERR-01`–`ERR-05` |
| Dependencies, toolchains, generated, or vendored artifacts | `None`；不增加播放库、媒体依赖、manifest 或 lockfile 变更，Save 图标复用已有 Lucide `download.svg` | `D-02`、`D-10`、`S-11`、`S-17` |
| Platform, packaging, CI, or release | `[Existing API reuse]` 使用锁定 GPUI 的 `open_with_system`、`reveal_path`、`prompt_for_new_path`；不改 bundle/workflow | `D-07`、`C-02`、`S-16` |
| User-visible defaults or removals | `[Modify]` Message content 严格按 payload 顺序显示 Text、Image、File、Attachment；旧 `Audio` 输入统一显示为 Attachment | `D-01`–`D-02`、`C-01`、`R-01`–`R-06` |
| Breaking change / migration | `None`；不删除/重命名 enum，不重写 payload，不迁移数据库，不改变 agent history | `D-03`、`R-11`–`R-14` |

## 目标

让 Jaco 时间线按每条持久化 Message 的 `content: Vec<ContentPart>` 原始顺序显示文字、现有图片预览和普通文件附件卡片。卡片通过 stable attachment ID 读取已加载的 attachment record，User 与 Assistant 共享同一投影和卡片契约；重载或重启后由同一 payload/record 恢复相同内容位置、元数据、availability 与动作。

最终用户可见附件类型只有：

```text
File
Attachment
```

底层 `ContentPart::Audio` 仅作为既有 serde/history 兼容输入，在 Jaco UI ingress 归一化为 `Attachment`。

## 非目标

- 音频播放器、Play/Pause、进度、时长、波形、自动播放、音频 MIME 检测或音频设备生命周期。
- Rodio、CPAL、Symphonia、GStreamer 或任何新增媒体依赖、feature、系统包、打包规则。
- 删除、重命名或改写 `ContentPart::Audio`、`AttachmentKind::Audio`、serialized tag 或 SQLite kind。
- 修改 `jaco-db::content_part_for_attachment`；`AttachmentKind::Audio | AttachmentKind::Attachment -> ContentPart::Attachment` 保持。
- 修改 `jaco-agent` 对 typed Audio 的 unsupported 行为，或增加 provider audio input。
- 新的上传流程、composer Audio 类型、provider-generated artifact ingestion、下载管理、转写或媒体库。
- 为 External URI 或 Provider file 增加打开、下载、鉴权、刷新 URL、凭据解析或 `open_url` 行为。
- 把附件名称、MIME、大小或 locator 加入 `search_text`、复制文本、Markdown 或模型历史。
- 修改 tool lifecycle、approval、reasoning、status/error block、AgentRun grouping 或 image preview dialog。
- 数据回填、payload 重写、schema migration、旧库升级路径或 `Cargo.lock` 变更。
- 在本轮计划阶段修改 Issue #195 正文；其音频播放条目由本计划记录的用户决定取代。

## 用户已确认决定

- 2026-08-31：Audio 不需要任何特殊处理，它是普通文件。
- 2026-08-31：最终产品类型只有 `File / Attachment`。
- 2026-08-31：保留 `ContentPart::Audio` 只为兼容旧数据；UI 将它归一化为 `Attachment`。
- 2026-08-31：现有 DB Audio→Attachment 映射保持，因此没有 schema、payload 或混合版本迁移。
- 2026-08-31：Issue 正文中的 minimal audio play/pause 和 playback validation 从实施范围移除。

这些决定已经封闭产品范围。当前没有待确认问题，实施已按这些决定完成。

## 计划映射

| Scope | 文档 | Owns | Assigned IDs/WPs |
| --- | --- | --- | --- |
| Root hub | 本文档 | 状态、范围、用户决定、S/C/ST/ERR/R、兼容策略、顺序和聚合验收 | `E-01`–`E-12`、`D-01`–`D-10`、`C-01`–`C-02`、`ST-01`–`ST-02`、`ERR-01`–`ERR-05`、`R-01`–`R-14`、`T-01`–`T-08`、`WP-001` |
| `app/jaco` | [owner plan](../../../app/jaco/docs/dev/issue-195/README.md) | ordered projection、per-block Markdown state、User/Assistant renderer、file access/actions、i18n/icons/tests | `F/L/ST/ERR/R/T/WP-1xx` |

## Applicability

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定或 negative reason | Owning section/WP |
| --- | --- | --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Applicable | 数据模型和 DB 已完整；缺口位于 Jaco timeline projection/render/action | 只建 app-local `detail/attachment_access.rs`，其余在既有 Jaco 模块内修改 | `D-09`、`WP-101`–`WP-104` |
| `S-02` | GPUI components, layout, interaction, and accessibility | Applicable | 当前 User 图片 strip 与 TextView 分离；File/Attachment 没有元素 | 复用 Button/Icon/Label/TextView/h_flex/v_flex，稳定 ElementId、tooltip、disabled/loading 语义 | `C-01`、`R-01`–`R-06`、`WP-102/103` |
| `S-03` | Entity, Store, Global, identity, and projections | Applicable | `ConversationDetailPage` 持有 timeline 与 entry-level TextViewState | 扩展为 block key 与 app-local access cache；不增加 Store/Global | `D-04`、`ST-01`–`ST-02`、`WP-101/103` |
| `S-04` | Actions, events, subscriptions, focus, and windows | Applicable | Timeline callbacks 已承载 toggle/copy/approval；TextView subscription 精确 remeasure row | 增加 attachment action callback；icon buttons stop propagation，subscription 仍按 entry remeasure | `D-05`、`D-07`、`WP-102/103` |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Applicable | GPUI file APIs 与 background executor 已可用；Jaco 有 window task retention | availability probe 带 generation fence；每次动作重新校验；Save busy 去重；window close 取消任务 | `ST-02`、`ERR-02`–`ERR-05`、`WP-103` |
| `S-06` | Data acquisition and Operation state | Applicable | conversation snapshot 已一次加载 entries + attachments；本地文件状态尚未探测 | 继续使用 snapshot，不建第二数据服务或 gpui-operation；view-owned probe 只表达 ephemeral availability | `D-06`–`D-07`、`ST-02`、`WP-103` |
| `S-07` | Forms and editable state | N/A | 本 issue 没有输入字段、draft 或 validation form | 不改 ChatForm、InputState、gpui-form 或 composer attachment kind | `S-07` |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | 数据跨 core/DB/agent 到 app；平台动作由 GPUI 提供 | 所有跨 crate contract 保持；只消费现有 app snapshot 和 GPUI 平台 API | `D-03`、`C-02`、`R-11`–`R-13`、`WP-103` |
| `S-09` | Error identity, propagation, recovery, and error UI | Applicable | 缺 record/path 目前静默消失；open/reveal API 不返回完成结果 | typed ERR 分类驱动 inline state/toast；cancel 静默；OS handoff 后不伪造成功 | `ERR-01`–`ERR-05`、`WP-103` |
| `S-10` | Database, persistence, and schema | Applicable, DB no change | payload/attachment records 已可重载；Save copy 需要 durable external file write | DB/schema/mapping 零变更；新增 staged streaming copy，失败不破坏既有目标 | `D-03`、`D-07`、`C-02`、`WP-103` |
| `S-11` | Generated, synchronized, copied, or vendored content | No change | Lucide `third_party/lucide/icons/download.svg` 已存在 | 只在 typed IconName 中引用已有 asset；不复制、生成或 vendor 文件 | `D-10`、`WP-103` |
| `S-12` | Icons and assets | Applicable | File/CircleAlert/ExternalLink/FolderOpen 已存在；Download enum 尚未声明 | 复用四个图标并声明 `Download => "download"`；无 Audio icon | `D-10`、`WP-103` |
| `S-13` | Fluent i18n and bundle localization | Applicable | 双 locale 仅有 composer attachment 文案 | 新增独立 `conversation-attachment-*` keys 与双 locale 存在性测试；不改 macOS bundle strings | `R-10`、`WP-103` |
| `S-14` | Security, privacy, and credentials | Applicable | record 同时含 path、URI、provider IDs；结构可 Debug | renderer/callback 只传 stable ID + File/Attachment kind + typed action；External/Provider deny-first；UI/Jaco logs 禁止 locator | `D-06`、`D-08`、`C-02`、`WP-103` |
| `S-15` | Observability and diagnostics | Applicable | 当前附件路径 resolver 无 typed diagnostics | Jaco日志只记 attachment ID、action、typed reason、可选 `io::ErrorKind`；禁止 `?attachment` | `D-08`、`ERR-01`–`ERR-05`、`WP-103` |
| `S-16` | Packaging, platform behavior, and CI/release | Applicable, packaging no change | locked GPUI 在三平台提供 open/reveal/save picker；Open/Reveal 返回 `()` | 复用平台 API；Quick Look 和 platform-ext 不进入范围；现有三平台 CI 是 release gate | `C-02`、`R-08`、`T-07`–`T-08`、`WP-001/103` |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | No change | 所需 GPUI/gpui-component/tempfile 已锁定 | 不改 manifest、feature、git SHA、toolchain 或 lockfile | `D-02`、`D-10`、`WP-001` |
| `S-18` | Owner documentation, indexes, and ADRs | Applicable | repo 使用 root hub + same-ID owner plan | 新增 root/Jaco 两份计划并更新两个索引；单 owner且不改变架构边界，无 ADR | `WP-001` |
| `S-19` | Validation and completion evidence | Applicable | 行为跨 projection、async file access、platform actions 与 reload | pure tests → GPUI interaction tests → Jaco checks → DB regression → manual restart → workspace/remote CI | `T-01`–`T-08`、`WP-001/104` |

## Evidence

### 当前流程

1. `Conversation` snapshot 已同时包含持久化 `entries` 与 `attachments`；Jaco 用 attachment ID 建 map。
2. `attachments::user_image_attachments` 只接受 User + Image，File/Audio/Attachment 和 Assistant attachment 全部不进入可见 timeline。
3. `UserMessageRow` 先渲染所有图片，再渲染 `item_markdown()` 合并后的所有文字，因此混合内容无法保持 payload 位置。
4. `AgentTurnRow` 的 final content 只读 `agent_final_markdown()`；expanded detail 也只渲染 entry markdown/tool blocks。
5. `ConversationDetailPage` 当前按 entry ID 持有一个 `TextViewState`，无法表示 `Text → attachment → Text` 的两个独立 Markdown block。
6. `AttachmentChanged` 已能找到引用 stable ID 的 entries，调用 `update_timeline_entry` 并重测量对应 row；目标方案沿用这条增量刷新链。
7. DB 已验证 timeline reload 同时返回 attachment records 与 `[Text, Image, File]` content；Audio/Attachment DB producer 当前统一写 `ContentPart::Attachment`。

### Evidence registry

| E-ID | Classification | Claim | Evidence | Plan consequence |
| --- | --- | --- | --- | --- |
| `E-01` | Current requirement | Issue #195 要求 stable ID、精确顺序、User/Assistant、safe metadata/actions、restart 和 image parity；正文仍含 minimal playback | GitHub Issue #195，2026-08-31 读取，`updatedAt=2026-07-31T12:20:35Z`，无评论 | 除 playback 外进入 `R-01`–`R-10`；playback 由 `E-02` 取代 |
| `E-02` | User decision | 最终只有 File/Attachment；Audio 作为普通附件且无特殊 UI | 本轮对话，2026-08-31 | `D-02`、`R-02`、非目标 |
| `E-03` | Current fact | core serde 仍有 File/Audio/Attachment；删除 tag 会破坏旧 payload 反序列化 | `crates/jaco-core/src/payloads/foundation.rs::{ContentPart,AttachmentKind}` | `D-03`、`R-11` |
| `E-04` | Current fact | DB producer 将 Audio/Attachment 统一映射为 ContentPart::Attachment | `crates/jaco-db/src/repository.rs::content_part_for_attachment` | 保持 producer，不“修正”为 typed Audio |
| `E-05` | Current fact | agent history 接受 File/Attachment，typed Audio 返回 unsupported | `crates/jaco-agent/src/runtime/history.rs` | `jaco-agent` 不变；UI compat 不承诺 Audio provider retry |
| `E-06` | Current fact | User 图片提取丢弃非 Image，且 strip 固定在合并文字之前 | `app/jaco/src/components/chat/detail/{attachments.rs,message.rs}` | 建立 C-01 ordered projection，连续图片才成组 |
| `E-07` | Current fact | entry-level TextViewState 与 `item_markdown` 合并文字 | `app/jaco/src/components/chat/detail.rs`、`foundation/conversation_format.rs` | 建立 ST-01 block identity；copy/search 保留旧 formatter |
| `E-08` | Current fact | AttachmentChanged 已按 stable ID 重建引用 rows | `ConversationDetailPage::sync_attachment_rows`、`entry_references_attachment` | 复用增量更新，不建第二 timeline |
| `E-09` | Current fact | managed files 位于 `DatabaseTarget.data_dir/attachments/{conversation_id}`；现有生成图片为 LocalFile storage + GeneratedFile source | `features/conversation/attachments.rs`、`database.rs` | C-02 使用实际 database target 和 shared managed-dir helper |
| `E-10` | Upstream fact | locked GPUI 提供 picker/open/reveal；open/reveal 返回 `()` | locked `gpui/src/app.rs::{prompt_for_new_path,reveal_path,open_with_system}` | 只报告 preflight 失败，handoff 后无 success toast |
| `E-11` | Current fact | `foundation::persistence` 已有 NamedTempFile + cross-platform `persist_staged`，`atomic_replace` 会把 bytes 全部置于内存 | `app/jaco/src/foundation/persistence.rs` | 新增 streaming `atomic_copy_file` 并复用 persist path |
| `E-12` | Current fact | Jaco 已有 notification、window task retention、File/alert/open/reveal icons 和双 locale tests | `detail.rs`、`app/tasks.rs`、`foundation/{assets,i18n}.rs` | 复用 app infrastructure，只补 Download 与附件 keys |

## Decisions

| D-ID | Decision | Evidence | Material rejected alternative | Consequence/owner |
| --- | --- | --- | --- | --- |
| `D-01` | 增加 app-local ordered projection，逐个遍历 Message content；只合并连续 Text 或连续可显示 Image，跨类型永不重排 | `E-01`、`E-06`–`E-08` | 继续用合并 Markdown + message-level image strip | `app/jaco` `WP-101/102` |
| `D-02` | 可见类型固定 File/Attachment；typed Audio 与 Audio-kind legacy reference 均投影为 Attachment；没有 audio-specific state/icon/action/playback validation，只保留归一化兼容断言 | `E-01`–`E-02` | 实现 issue 原文的最小播放器或 MIME-special case | `app/jaco` `WP-101/103` |
| `D-03` | core enum/serde、DB schema/mapping、agent history 全部保持；不 backfill、不重写 payload | `E-03`–`E-05` | Audio producer 改为 ContentPart::Audio、删除底层 Audio | 所有跨 crate owner保持无 diff |
| `D-04` | `ConversationDetailPage` 仍是 TextViewState 唯一 owner；Message block 用 `(entry_id, start_part_index)`，非 Message entry 用 whole-entry key | `E-07` | 一个 entry 共享一个 TextViewState，或 row 自建 state | `ST-01`、`WP-101` |
| `D-05` | User、run final Assistant、loose Assistant 和 expanded Assistant Message 共用 message content renderer；role shell、tool/status/error lifecycle 保持 | `E-01`、`E-06`–`E-08` | 只补 User FileCard 或只补 Agent final text | `C-01`、`WP-102` |
| `D-06` | Card projection只携带 safe display metadata、stable ID 与 File/Attachment kind；最新 record/path 仅在 page-owned access resolver 中使用 | `E-01`、`E-09` | 把 raw Path/URI/provider locator 捕获进 element callback | `C-02`、`WP-103` |
| `D-07` | static source gate + background filesystem probe决定 availability；点击时重新校验；Save a copy 用 picker + staged streaming atomic replace | `E-09`–`E-12` | render thread同步 I/O、直接 `fs::copy` 覆盖目标或缓存 path 后直接动作 | `ST-02`、`ERR-02`–`ERR-05`、`WP-103` |
| `D-08` | External/Provider deny-first；UI/toast/Jaco-owned logs不含 locator；Open/Reveal只报告 preflight，平台 handoff 后不显示成功 | `E-09`–`E-10` | provider URI fallback到 open_url，或把 OS error/raw record显示给用户 | `C-02`、`ERR-01`–`ERR-05` |
| `D-09` | 新模块只放 `app/jaco/src/components/chat/detail/attachment_access.rs`；projection/render 留在 attachments.rs，orchestration 留在 detail.rs | `E-06`–`E-12` | 把 view-only contract放进 jaco-core，或在多个 row重复 resolver | `WP-101/103` |
| `D-10` | 复用 gpui-component 和现有 icons；只新增 Download typed variant与双 locale keys；无 Card 假想组件、无新依赖 | `E-12` | 自建通用 component crate、复制 asset或加 media/file crate | `WP-102/103` |

## C-01：Ordered persisted message projection

Authority：`app/jaco::components::chat::detail::attachments`。

```text
ConversationEntryPayload::Message.content (source order)
        + attachments_by_id (stable-ID lookup only)
        + per-block TextViewState lookup
        + AttachmentAccessView keyed by attachment ID
    → Vec<MessageContentBlock>
    → shared User/Assistant block renderer
```

Canonical rules：

| Input part | Visible block | Record compatibility | Missing/invalid behavior |
| --- | --- | --- | --- |
| `Text` | Text block；只合并相邻 Text，使用 `\n` 连接 | 不访问 record | 空 run 不创建可见 block |
| `Image` | 现有 80×80 preview；只合并相邻且可解析 Image | record kind 必须 Image | 保持当前行为：不可解析 image 不创建 error card，并在该 slot 断开 image group |
| `File` | File card | record kind 必须 File | 保留 File card 位置，显示 safe unavailable |
| `Audio` | Attachment card | record kind 允许 Audio 或 Attachment | 保留 Attachment card 位置，绝不创建 Audio UI |
| `Attachment` | Attachment card | record kind 允许 Attachment 或 Audio | 保留 Attachment card 位置，显示 safe unavailable |

- attachment collection 的排序不参与 UI 顺序，只用于 ID lookup。
- 连续分组保留内部原始顺序；Text/Image/File/Attachment 之间不跨组。
- `item_markdown()`、`content_parts_text()` 和 copy/search contract 保持，只把 Text parts 连接；附件 metadata 不进入复制和搜索。
- `AttachmentChanged`、entry replace/append、reload 都重新运行相同 projection。

## C-02：Trusted local attachment access and actions

Authority：`app/jaco::components::chat::detail::attachment_access`；orchestrator 为 `ConversationDetailPage`。

```rust
enum AttachmentAction {
    Open,
    Reveal,
    SaveCopy,
}

struct AttachmentActionTarget {
    attachment_id: AttachmentId,
    kind: AttachmentCardKind,
}

enum AttachmentAccessProblem {
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
```

Resolver 顺序：

1. 通过 stable attachment ID 从最新 conversation snapshot 取 record；核对 `conversation_id` 与 C-01 kind compatibility。
2. `storage_kind=ExternalUri|ProviderFile` 立即拒绝，即使 record 同时带 path。
3. `storage_kind=LocalFile|GeneratedFile` 只接受 metadata source `LocalFile|GeneratedFile`；External/Provider metadata 拒绝。
4. 顶层 path 和 metadata path 都存在时分别 canonicalize 并要求相同；只有一个时使用该 locator；空值拒绝。
5. canonical target 必须存在且是 regular file。
6. managed root 从当前 `database::store(...).target.data_dir/attachments/{conversation_id}` 取得；Generated storage/source 必须 canonical containment，防止 `..`/symlink 逃逸；普通 Local 可位于 root 内外。
7. render probe 与每次点击都执行同一 resolver；action-time 结果是最终授权。

动作矩阵：

| Record/source state | Card state | Open | Reveal | Save copy |
| --- | --- | ---: | ---: | ---: |
| validated Local/Managed/Generated regular file | Available | 是 | 是 | 是 |
| probe pending | Checking | 否 | 否 | 否 |
| External URI / Provider file | Unavailable | 否 | 否 | 否 |
| missing record/kind/locator/file、directory、mismatch、unsafe generated | Unavailable | 否 | 否 | 否 |
| Save 同一 attachment 正在进行 | Available + saving | 是 | 是 | 否 |

Platform contract：

- Open：action-time resolver成功后调用 `cx.open_with_system(&path)`；返回 `()`，不显示 success toast。
- Reveal：action-time resolver成功后调用 `cx.reveal_path(&path)`；返回 `()`，不显示 success toast。
- Save copy：使用 sanitized basename 作为建议名；用户取消静默；picker error/toast；后台 `atomic_copy_file`；成功/失败 toast。
- Save selected target 是用户授权的外部写入；staged temp 位于目标同目录，copy + flush + sync + existing cross-platform persist；失败保留原目标。
- callback携带 `AttachmentActionTarget + AttachmentAction`；kind确保点击时仍按该message part的File/Attachment contract重新校验。
- locator只存在于 resolver/task内部，不进入 `MessageContentBlock`、element ID、tooltip、notification 或 copy text。

## ST-01：Per-block Markdown state lifecycle

```text
Message content scan
  ├─ consecutive Text run → MessageBlock(entry_id, start_part_index)
  └─ non-Message visible markdown → WholeEntry(entry_id)

same key + same source              → no update
same key + source prefix extension  → TextViewState::push_str(delta)
same key + other source change      → TextViewState::set_text(source)
text-block key set changed          → drop all Message block states for entry, rebuild exact set
entry removed/tool-lifecycle only   → drop its states/subscriptions
state notification                  → remeasure containing timeline row
```

- `(entry_id, start_part_index)` 只在 content structure 未移动该 run 时复用。
- 插入/删除/移动 part 导致 key set 变化时整条 Message 的 block states 重建，避免旧 Entity 错配。
- 结构不变的 streaming append 继续使用 `push_str`；AttachmentChanged 不重建不受影响的 TextViewState。
- 每个可交互/有状态 element ID 由 entry ID + block start index或 attachment ID 派生。

## ST-02：Availability and Save lifecycle

```text
snapshot/referenced attachment change
  → generation += 1
  → static invalid sources publish Unavailable
  → local candidates publish Checking
  → one background probe batch
      ├─ current generation → Available/Unavailable + rebuild referencing rows
      └─ stale generation   → discard

click Open/Reveal/Save
  → resolve latest record again
      ├─ failure → ERR toast, no platform/file call
      └─ success
          ├─ Open/Reveal → dispatch once, no success claim
          └─ Save → mark attachment busy → picker
                 ├─ cancel → clear busy, silent
                 ├─ picker error → clear busy + ERR-04
                 └─ target → background atomic copy
                            ├─ success → clear busy + success toast
                            └─ failure → clear busy + ERR-05
```

- `ConversationDetailPage` owns access map、generation、probe Task 与 `saving_attachments`。
- Probe Task 随 page drop 取消；动作 Task 用现有 window task retention，随 window close 取消并由 NamedTempFile 清理 staging。
- 同一 attachment 的重复 Save 在 busy 期间禁用；不同 attachment 可以独立开始，完成只更新引用该 ID 的 rows。

## Error catalog and privacy policy

| ERR-ID | Trigger | UI contract | Diagnostic contract | Recovery |
| --- | --- | --- | --- | --- |
| `ERR-01` | missing record、wrong conversation、kind mismatch、invalid metadata/locator | 原位置显示 localized Unavailable card；无 actions | attachment ID + typed reason；不 dump record | AttachmentChanged/reload 后重新投影 |
| `ERR-02` | External/Provider、Generated path逃逸或不受支持 source/storage | 原位置显示 source + safe unavailable；无 actions | attachment ID + source category；无 URI/provider ID | 数据变更后重新 probe |
| `ERR-03` | action-time missing/not regular/permission/IO | Error toast；不调用 open/reveal/copy | attachment ID + action + typed reason + optional ErrorKind | 用户恢复文件或 attachment update 后重试 |
| `ERR-04` | save picker receiver/platform error | Error toast；busy 清除；用户取消不属于 error | attachment ID + picker category | 再次 Save |
| `ERR-05` | staged create/copy/flush/sync/persist failure | Error toast；busy 清除；既有目标保持 | attachment ID + stage + ErrorKind；不输出 source/target path | 修复权限/磁盘后重试 |

Privacy invariants：

- 允许显示：sanitized filename、validated MIME、non-negative size、泛化 File/Attachment 与 Managed/Local/Generated/External/Provider 状态。
- 禁止显示：完整路径、external URI、provider ID/file ID、provider URL、query、header、credential、原始 locator。
- 禁止 Jaco-owned logs：`?attachment`、`?record`、`?path`、原始 picker target。
- GPUI 平台实现可能在 OS handoff 自身失败时记录已经验证的本地 path；Jaco 无法取得或改写该结果。C-02 保证 External/Provider locator 永不进入这条上游平台路径。
- `safe_display_name` 去除 `/`、`\\` 路径段和控制字符，trim 并设置长度上限；空值使用本地化 `Attachment`。
- MIME 只接受单行、无控制字符、合理长度的 `type/subtype`；异常值省略。
- `size_bytes < 0` 视为无可显示 size，不影响经过文件 resolver 验证后的 actions。

## 兼容性与回滚

| Contract | Target | Compatibility result |
| --- | --- | --- |
| `ContentPart::{File,Audio,Attachment}` serde | 不改 | 已有 payload 继续反序列化；Audio tag仍可读 |
| `AttachmentKind::{File,Audio,Attachment}` / SQLite | 不改 | 已有 rows、constraint 和 fresh schema 无变化 |
| DB Audio/Attachment producer | 保持映射到 `ContentPart::Attachment` | 新旧 app 数据形状一致；不引入 typed Audio 重试失败 |
| Agent history | 不改 | File/Attachment 继续普通文件路径；typed Audio 仍按既有 unsupported 处理 |
| Search/copy/Markdown | 不改 | 附件 metadata 不进入搜索、复制或模型文字 |
| Older app reading data after this change | 数据没有新字段/tag | 可正常读取；旧 UI 仍隐藏卡片属于功能差异 |
| Rollback | 回退 app UI commit | 数据库/payload无需回滚；用户主动 Save 的外部副本保留 |

因此本 issue 没有破坏性更新，也没有 schema、序列化、provider 或混合版本兼容问题。唯一有意的行为变化是 timeline 开始显示既有持久化附件，并修正混合 content 的视觉顺序。

## Root requirements

| R-ID | Requirement |
| --- | --- |
| `R-01` | Message content 按 payload source order投影；attachment list order不参与排序。 |
| `R-02` | 可见类型只有 File/Attachment；Audio统一为 Attachment且无任何专属 UI/state/dependency/playback test，只保留compatibility normalization test。 |
| `R-03` | Text/Image/File/Attachment 混排保持位置；连续图片复用现有缩略图/preview行为。 |
| `R-04` | User 与所有适用 Assistant Message 使用 C-01；tool/status/error/reasoning lifecycle保持。 |
| `R-05` | reload/restart、entry update 与 AttachmentChanged 运行同一 projection并保留 stable identity。 |
| `R-06` | copy/search/plain-text继续只包含 Text；附件 metadata不进入剪贴板或检索。 |
| `R-07` | availability在后台探测且带 stale fence；render/UI线程不做文件 I/O。 |
| `R-08` | Open/Reveal/Save只消费 action-time C-02 validated local path；External/Provider永不派发。 |
| `R-09` | Save 使用 staged streaming atomic copy、busy去重、cancel静默和安全结果通知。 |
| `R-10` | Card/actions/errors使用 semantic theme、stable IDs、tooltip与 en-US/zh-CN parity。 |
| `R-11` | core enum/serde、DB schema/mapping与 agent history保持无 diff。 |
| `R-12` | Cargo manifests、Cargo.lock、provider/Rig/MCP、platform-ext/window-ext保持无 diff。 |
| `R-13` | UI、toast、element ID、copy和 Jaco-owned logs不泄露 path/URI/provider locator/credential；上游平台只可能收到validated local path。 |
| `R-14` | 只修改计划列出的 app/Jaco 文件；自动化、manual restart、三平台 CI evidence在完成前同步。 |

## Work package sequence

```text
WP-101 ordered projection + block TextViewState
    ↓
WP-102 shared User/Assistant rendering + image parity
    ↓
WP-103 trusted access + card/actions + atomic save + i18n/icons
    ↓
WP-104 focused regression + restart/manual/platform acceptance
    ↓
WP-001 aggregate workspace gate and completion evidence
```

详细 symbol、文件和 tests 位于 [Jaco owner plan](../../../app/jaco/docs/dev/issue-195/README.md)。

## Root validation matrix

| T-ID | Scope | Command/scenario | Required evidence |
| --- | --- | --- | --- |
| `T-01` | Pure projection/state/access | `cargo test -p jaco --locked components::chat::detail` | mixed order、roles、Audio normalization、missing/mismatch、stable block state、privacy |
| `T-02` | File persistence | `cargo test -p jaco --locked foundation::persistence` | byte equality、same source/target、replace、failure leaves target、cleanup |
| `T-03` | i18n/icons/build | `cargo test -p jaco --locked foundation::i18n` + `cargo check -p jaco --locked` |双 locale keys、typed Download asset、compile |
| `T-04` | Existing DB contract regression | `cargo test -p jaco-db --locked attachments` | persisted entries + attachments reload unchanged；无 migration |
| `T-05` | Jaco owner gate | `cargo test -p jaco --locked --no-fail-fast` + Jaco clippy | focused与existing app regressions通过 |
| `T-06` | Manual UI/restart | isolated `JACO_CONFIG_DIR`，代表性 text/image/file + persisted Audio/Attachment fixtures | exact order、same IDs/restart、actions/errors、image preview parity |
| `T-07` | Workspace local gate | repo standard build/test/clippy with `--locked` | 没有跨 owner regression、dependency/lock/schema diff |
| `T-08` | Remote release gate | `.github/workflows/ci.yml` macOS/Linux/Windows | 三平台 compile/test/clippy 与 platform action代码通过 |

实施期间遵循 owner focused first。只有本轮产物确认不再修改后，执行一次 workspace aggregate gate；计划编写阶段不运行 code tests。

## WP-001：Aggregate completion and release gate

**Prerequisites**

- `WP-101`–`WP-104` 全部完成且 owner plan 记录实际结果。
- 没有新增 unresolved product/API/security decision。

**Sequence**

1. 审计实际 diff 只包含 owner plan 的 F-1xx 文件；确认 core/db/agent/manifests/lock/schema/workflows 无 diff。
2. 执行 `cargo fmt --all -- --check`、T-01–T-05 和 `git diff --check`。
3. 完成 T-06 manual restart/action/error/image parity。
4. 执行一次 workspace aggregate build/test/clippy；推送后等待 T-08 三平台 CI。
5. 将 root/Jaco status 与 Completion evidence 从 pending 更新为实际 commit、commands、manual/CI URLs。

**Aggregate commands**

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo test --workspace --locked --no-fail-fast
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
git diff --check
```

**Unchanged-scope audit**

```sh
git diff --exit-code -- \
  crates/jaco-core \
  crates/jaco-db \
  crates/jaco-agent \
  Cargo.toml \
  Cargo.lock \
  app/jaco/Cargo.toml \
  .github/workflows
```

**Done condition**

- R-01–R-14 有自动化或人工 evidence。
- Issue 原 playback 条目在 completion notes 中明确记为用户取消范围，不以缺测项出现。
- 没有 raw locator exposure、partial Save target、stale probe publication 或 image regression。
- local aggregate 与远端 macOS/Linux/Windows CI 通过。

## Manual acceptance

1. User message `Text A → Image 1 → Image 2 → Text B → File → Audio/Attachment → Text C` 按此顺序显示；两张连续图片仍为 80×80 strip，并可打开既有 preview。
2. Assistant final Message、loose Assistant Message、expanded run中的非-final Assistant Message使用同一 File/Attachment card；tool/status/error blocks不改变。
3. Audio record和 typed Audio payload都显示 `Attachment` 卡片；页面无 audio icon、Play/Pause、进度、波形或媒体状态。
4. 退出并使用相同隔离数据目录重启；卡片 attachment ID、位置、metadata和availability一致。
5. Local/Managed/Generated regular file的 Open/Reveal分别派发一次；派发后不显示虚假成功通知。
6. Save copy取消时无 toast；成功副本字节一致；选中已有目标可安全替换；失败不改变旧目标；同一 attachment busy期间不能重复 Save。
7. 删除/移动 source，或使用missing record、kind mismatch、directory、External、Provider、unsafe Generated fixtures；原内容位置显示 safe unavailable且timeline不崩溃。
8. UI、tooltip、toast、copy result与测试捕获的 Jaco logs均不出现测试 path、secret URI、provider ID/file ID/token。
9. 纯文字、纯图片、图片 preview、copy、AgentRun expand/collapse、approval/tool details与row scrolling不回归。

## Completion evidence

| Evidence | Actual result |
| --- | --- |
| Plan/root + owner topology | `Implemented locally`；root/Jaco 计划、实现和本地验收证据于 2026-08-31 同步 |
| Production implementation | Complete；ordered projection、per-block Markdown state、共享 User/Assistant renderer、trusted access、Open/Reveal/Save、atomic copy、i18n/icons 已实现；可见类型只有 File/Attachment，Audio 仅在 ingress 归一化 |
| Focused automated tests | Complete；相关 projection/access/action/persistence/i18n/GPUI 回归由最终 `cargo test -p jaco --locked --no-fail-fast` 覆盖，结果 575 passed、2 ignored |
| Jaco full tests/check/clippy | Passed；`cargo test -p jaco --locked --no-fail-fast`、`cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings`、`cargo test -p jaco-db --locked attachments` |
| Manual restart/actions/privacy/image parity | Persisted fixture 重新打开并在 Jaco 中验证 exact mixed order、User/Assistant surfaces、Image 显示、Audio→Attachment、External/Provider unavailable、Save cancel/success + byte equality、Open/Reveal dispatch；最终缓存修正后按用户要求未再次启动 GUI，由新增 GPUI regression test覆盖该修正 |
| Workspace aggregate gate | Passed；`cargo fmt --all -- --check`、`cargo build --workspace --locked`、`cargo test --workspace --locked --no-fail-fast`、workspace clippy、`git diff --check` 与 unchanged-scope audit；workspace tests 因 sandbox 禁止 loopback 首次失败，原命令提权后 exit 0 |
| Remote macOS/Linux/Windows CI | Pending |
| Commit/PR | Pending |

Bundle evidence：`cargo run -p xtask -- bundle jaco` 成功生成
`target/release/bundle/macos/Jaco.app`。受限构建环境中的 `actool` 无法连接
CoreSimulatorService，xtask 按既有 fallback 保留普通图标；bundle 命令 exit 0。

Known boundary：文件动作在 dispatch 前重新校验，仍保留通用文件系统固有的
check-to-use race；External/Provider locator 不会进入 OS dispatch。未改动 core/db/agent、
manifest、lockfile、schema、platform crates 或 workflows。

## Handoff checklist

- [x] 产品类型收敛为 File/Attachment，Audio compatibility ingress 已锁定。
- [x] DB/serde/agent compatibility 与 zero-migration 决策已锁定。
- [x] ordered projection、TextViewState identity、access resolver、error/privacy 与 Save atomicity 已锁定。
- [x] 文件/API/i18n/icon/test范围已路由到 `app/jaco` owner plan。
- [x] 实施 `WP-101`–`WP-104`。
- [x] 执行 T-01–T-05 与已授权 T-06 人工场景并记录实际结果。
- [x] 执行 WP-001 本地 aggregate gate。
- [x] 同步两份计划的本地完成状态与 evidence。
- [ ] 推送后执行远端三平台 CI，并补充 commit/PR/CI evidence。
