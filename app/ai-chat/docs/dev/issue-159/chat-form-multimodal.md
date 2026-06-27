# Issue #159 ai-chat2 ChatForm 多模态输入实现记录

本文档固定 `app/ai-chat2` 的 `ChatForm` 文件/图片附件支持计划和第一版实现记录。当前状态：第一版产品代码已落地，附件操作流、附件视图已按 ChatForm 子模块拆分；图片预览已提升为共享 `components/image_preview.rs`，供 ChatForm 附件和 conversation timeline 用户图片复用；仍保留完整调研结论和后续增强边界。

最后同步时间：2026-06-15。

## 目标

- 在 `ChatForm` 文本输入框上方新增附件 strip；没有附件时不渲染这一行。
- 支持从粘贴板添加图片和文件，普通文本粘贴继续由 `ComposerEditor` 处理。
- 支持把 Finder/系统文件管理器中的文件拖拽到 `ChatForm` 范围内；drop 成功等价于用户添加文件/图片附件。
- 支持通过 `+` 按钮打开附件动作菜单，第一版至少提供系统文件选择器添加文件/图片。
- 图片附件在 app 内预览；普通文件在 macOS 上优先使用 Finder 同级 Quick Look preview panel，不把默认程序打开当作 macOS 预览替代。
- 发送时把文本和附件合成 `ContentPart::{Text, Image, File}`，复用 fresh `attachments` 表和 `AttachmentMetadata`，不新增 schema。
- 发送前做 provider/model capability gating；模型不支持附件类型时保留附件、禁用发送并展示原因，不静默降级为文本。

## 非目标

- 第一版不实现 audio input、generated output timeline、下载管理或完整 multimodal timeline 渲染；conversation timeline 当前只补了 user image attachment 缩略图和点击预览。
- 第一版不把附件二进制塞进 `conversation_items.payload_json` 或文本内容。
- `cx.open_with_system(&path)` 不是 macOS Finder 级预览；它只作为非 macOS 行为，或 macOS Quick Look 明确失败后的可见 fallback。

## 当前实现

- `app/ai-chat2/src/components/chat_form.rs` 已持有 pending attachments 和图片 preview 状态，并接入文件选择器、从剪贴板添加、`ExternalPaths` drop、capability gating 和发送按钮 disabled reason；附件操作和附件视图已拆到 `chat_form/attachment_flow.rs` 与 `chat_form/attachment_views.rs`，root `ChatForm` 保留整体状态、提交和布局编排。`+` 附件菜单不再由 `ChatForm` 保存 open state，改由 `gpui-component` `DropdownMenu` 自管理打开、关闭、focus 和键盘行为。
- `app/ai-chat2/src/components/image_preview.rs` 已实现当前窗口全窗口图片 preview overlay，支持 fit zoom、按钮缩放、触控板 pinch/scroll、滚动查看、右上角关闭和点击遮罩关闭；预览控件使用 `popover` / `popover_foreground` / `muted_foreground` 等 theme token，不硬编码白色控件。
- `app/ai-chat2/src/components/chat_form/attachments.rs` 已集中固定 Codex app reference 得到的 strip 样式常量：`80px` 图片缩略图、`8px` gap、`220px x 56px` 文件 card、`20px` remove button、`8px` radius。
- `app/ai-chat2/src/components/chat_form/composer_editor.rs` 在 `secondary-c` 时优先识别 GPUI `ClipboardItem::{ExternalPaths, Image}`，有文件/图片时发 `PasteAttachmentRequested`，普通文本仍走原 `ComposerEditor` 粘贴。
- 第一版未引入 `clipboard-rs`：GPUI 已提供 `ClipboardEntry::ExternalPaths` 和 `ClipboardEntry::Image`，且 macOS pasteboard 顺序已能满足 Files -> Image -> Text 判定。后续只有在 GPUI clipboard 覆盖不足时再引入 `clipboard-rs`。
- `app/ai-chat2/src/state/attachments.rs` 已实现文件/图片分类、clipboard image pending 落盘、文件 MIME/扩展名判定、附件入库和 `ContentPart::{Image, File}` 合成；复用 fresh `attachments` 表，不新增 schema。
- `app/ai-chat2/src/state/conversations.rs` 已把 `CreateConversationRequest` / `SendConversationMessageRequest` 扩展为接收 composer attachments，创建/追加 user item 后插入 attachment records 并回写最终 content parts。
- `crates/ai-chat-db` 已让 `ConversationTimelineRecords` 返回 `attachments`，并新增 `conversation_attachments` 读取接口。
- `crates/ai-chat-agent/src/history.rs` 已把 user message 的 `ContentPart::Image` 转成 Rig `UserContent::Image` base64 content，把 PDF 转成 base64 document，把文本类文件转成 UTF-8 document。当前 runtime 不支持的二进制文件会明确报 unsupported，ChatForm 也会提前禁用发送。
- `crates/ai-chat-agent/src/model_capabilities.rs` 已同步 ChatForm 多模态所需的 provider capability profile：OpenAI GPT-5.5/GPT-5.4/GPT-5/GPT-4o/GPT-4.1/o3/o4 标记 image/file input；Claude 当前模型标记 image/file input；Gemini 2.5/3 标记 image/file input；OpenRouter 从 `/api/v1/models` 的 `architecture.input_modalities` 读取 image/file；DeepSeek v4/chat/reasoner 补齐 tool/structured output；Mistral 补齐 tool/structured output。Mistral 官方 vision models 暂不标记 image input，因为当前 Rig Mistral adapter 明确不支持 `UserContent::Image`。
- `crates/window-ext/src/lib.rs` 已提供 `preview_file_with_quick_look(&Path)`：macOS 使用 `QLPreviewPanel` + datasource bridge 显示 Finder 同级 Quick Look；非 macOS 或 Quick Look 失败时由 ChatForm fallback 到 `cx.open_with_system(&path)`。
- `+` 按钮现在使用 `Button::dropdown_menu_with_anchor` + `PopupMenuItem` 打开附件菜单，第一版提供“添加文件”和“从剪贴板添加”；拖拽文件到整个 `ChatForm` surface 等价于添加附件。2026-06-14 已修正 attachment strip / warning / editor / footer 的纵向 flow，移除提示与输入框重叠；图片缩略图使用专用容器而非通用 card，0 inset 内层负责 rounded + `overflow_hidden` 裁剪，0 inset overlay 负责圆角边框，不引入 margin、padding 或 1px 白边；删除按钮点击会 `stop_propagation`，不会打开预览。
- ChatForm 添加文件的 `PathPromptOptions.prompt` 已改为 `None`，避免覆盖 macOS `NSOpenPanel` 默认按钮文案；系统文件对话框整体语言依赖 app bundle localization。`xtask` 打包时把源码中的 `en-US.lproj` / `zh-Hans.lproj` 映射为 macOS 常见的 `en.lproj` / `zh_CN.lproj`，并写入 `CFBundleAllowMixedLocalizations = true` 与 `CFBundleLocalizations = ["en", "zh_CN"]`。
- `app/ai-chat2/src/components/conversation_detail/attachments.rs` 已补齐 user image attachment 的 timeline 显示：按 `ContentPart::Image` 顺序从 `ConversationTimelineRecords.attachments` 取本地图片，渲染在用户文本气泡上方，点击复用共享图片 preview overlay。

## 剩余边界

- 完整 timeline multimodal rendering 尚未完成；当前只显示用户图片附件缩略图。文件附件 chip、tool/MCP 结果、思考内容、生成图片/文件、下载管理仍是后续 rich timeline 工作。
- 文件发送第一版支持图片、PDF 和 UTF-8 文本类文件。Office、zip 等普通文件仍可作为附件添加和 Quick Look/open，但不会直接发送给模型，后续应做 provider file upload / file-id 流程。
- 目录拖拽第一版按“不支持的非普通文件”拒绝，不做递归。
- Audio input、generated output、下载管理不在本轮范围。

## macOS Quick Look 调研结论

调研来源：

- Apple Developer Documentation：`QuickLookUI/QLPreviewPanel`、`QuickLookUI/QLPreviewItem`。
- 本机 SDK 头文件：`/Applications/Xcode.app/.../QuickLookUI.framework/Headers/QLPreviewPanel.h` 和 `QLPreviewItem.h`。
- 本地 `objc2` 生成清单：`objc2-quick-look-ui` 已作为 `QuickLookUI` framework 的 Rust binding 发布。

可实现结论：

- Finder 级预览对应公开 API `QLPreviewPanel`，不是 `open_with_system`。`QLPreviewPanel` 是 app 级 shared preview panel，显示由 `QLPreviewPanelDataSource` 提供的一组 preview items。
- `QLPreviewPanel` 不能 subclass；需要设置 datasource/delegate 或提供 responder-chain controller。第一版应先实现稳定的 datasource bridge，再决定是否需要 responder-chain controller。
- datasource 必须实现 `numberOfPreviewItemsInPreviewPanel:` 和 `previewPanel:previewItemAtIndex:`。
- `QLPreviewItem` 的核心字段是 file URL；Apple SDK 头文件说明 `NSURL` 已通过 category conform `QLPreviewItem`，因此第一版可以把本地文件 path 转成 `NSURL` 作为 preview item，不需要自己实现 PDF/Office/text/image 渲染。
- 预览必须在 macOS 主线程触发；如果当前 GPUI 回调不是主线程，平台层要把调用转回主线程，不能在后台任务里直接操作 `QLPreviewPanel`。
- 需要持有 datasource/controller 生命周期。`QLPreviewPanel.dataSource` 是 assign 语义，不能创建临时对象后立刻 drop；计划在 `window-ext` 内用 main-thread `thread_local!` 保存 controller/items，panel 关闭或下一次 preview 时替换。
- `qlmanage -p` 可以作为调试参考，但不是产品实现方案；它会启动外部进程，生命周期和窗口归属都不符合 app 内平台能力。
- macOS 第一版要求：文件 chip 点击先打开 Quick Look panel；Quick Look 初始化、URL 构造或 panel 操作失败时显示可见错误和“用系统程序打开”动作。不能静默直接 `open_with_system`。

## GPUI 文件拖拽调研结论

调研来源：

- 当前 workspace 锁定的 GPUI 源码：`~/.cargo/git/checkouts/zed-*/crates/gpui/src/interactive.rs`、`elements/div.rs`、`window.rs`。
- GPUI macOS platform 源码：`crates/gpui_macos/src/window.rs`。
- Zed 现有用法：`crates/agent_ui/src/agent_panel.rs`。

可实现结论：

- GPUI 已有外部文件拖放模型：`ExternalPaths(pub SmallVec<[PathBuf; 2]>)` 表示系统拖进来的 path 集合，`ExternalPaths::paths()` 返回 `&[PathBuf]`。
- 平台层会产生 `FileDropEvent::{Entered, Pending, Submit, Exited}`。macOS 实现会从 `NSFilenamesPboardType` 读取文件路径；Windows 和 Linux X11/Wayland 在 GPUI 源码中也有 `FileDropEvent` 接线。
- GPUI window 会把 `FileDropEvent::Entered` 转成 active drag，drag item 类型是 `ExternalPaths`；`Submit` 转成 mouse up。因此普通 GPUI element 可以用 typed drag/drop API 接收系统文件。
- Fluent API 形态已经存在并被 Zed 使用：`.drag_over::<ExternalPaths>(...)` 控制 drop target hover style，`.on_drop(cx.listener(|..., paths: &ExternalPaths, ...| ...))` 接收最终 drop。注意 `drag_over` 只返回 `StyleRefinement`，不能在回调里新增 child。
- 因此 ChatForm 第一版可以直接在 composer surface 或覆盖层上绑定 `drag_over::<ExternalPaths>` 和 `on_drop::<ExternalPaths>`，不需要新增平台接口或依赖库。
- Drop 范围定义为整个 `ChatForm` surface，包括附件 strip、文本输入区和 footer；drop 后调用与 clipboard/files picker 相同的 `state::attachments::classify_local_paths`，保证图片文件和普通文件分类一致。
- 目录第一版仍按文件分类规则处理：如果 `state::attachments` 决定目录不支持，就拒绝并显示错误；不要在 drag/drop 分支单独实现目录递归或复制逻辑。

## Codex App 参考结论

本次参考使用 `pnpm dlx @electron/asar` 只读抽取 `/Applications/Codex.app/Contents/Resources/app.asar` 到 `/private/tmp/codex-asar-issue159`，未把解包产物提交到仓库。

- Composer 附件行位于输入区上方；没有 visible attachments 时返回 `null`，存在附件时渲染附件 row。
- 附件 row 的可观察 DOM 结构是外层 `hide-scrollbar w-full overflow-x-auto`，内层 `flex min-w-max items-end gap-2`，并带 `data-composer-attachments-row`。父 composer surface 使用 `relative flex w-full flex-col gap-2`，说明附件行和文本输入之间不是独立 card，而是同一个 composer surface 内的纵向间距。
- 普通图片 attachment 的横向动画步进是 `88px`，可推导为 `80px` 缩略图宽度 + `8px` gap。第一版 `ai-chat2` 固定采用 `80px` image thumbnail 和 `8px` attachment gap。
- Codex app 的 appshot/pending capture 卡片使用 `composer-attachment-surface relative flex-shrink-0 overflow-hidden rounded-2xl`，目标尺寸约 `232px x 140px`，横向步进 `240px`，初始动画里 `marginRight: -8`。`ai-chat2` 第一版不做 appshot 卡片，但保留这些值作为后续截图上下文卡片参考。
- Composer 内联的小图片/file mention 图标出现过 `h-5 w-7 shrink-0 rounded-sm border border-token-border object-cover`，约等于 `28px x 20px`。这属于 inline mention 场景；`ai-chat2` 附件 strip 不照搬这个尺寸，当前文件附件使用 `220px x 56px` card，文件 icon 固定 `18px`。
- remove button 独立在 `attachment-remove-button-*` bundle 中，minified 产物无 sourcemap，无法稳定还原全部 class；`ai-chat2` 当前固定 remove button 为 `20px` 圆形 ghost icon button，右上角 `4px` inset overlay，图片和文件附件均阻止 click 事件冒泡，避免删除时触发预览/open。
- Composer state 把 image attachments、file attachments、pasted text、appshot contexts、selected text、added files 等分桶保存，而不是把所有内容塞进纯文本。
- 粘贴/拖拽事件按 file-first 处理：如果 `clipboardData` / `dataTransfer` 包含文件，阻止默认文本粘贴；图片文件进入 image attachment，其他文件进入 file mention/attachment。
- 图片文件判定参考实现使用 MIME 与扩展名结合：支持 `png`、`jpg`、`jpeg`、`gif`、`webp`，排除 svg、heic/heif、bmp、tiff、avif、ico、jp2 等不适合直接作为 composer image preview 的格式。
- 图片附件使用缩略图和 remove button；可预览的内容进入 app 内 preview。Codex app 对普通文件有系统目标或文件管理器路径行为，但这只能作为 fallback/open 参考，不能替代本计划的 macOS Quick Look 目标。
- Codex app 对部分特殊文件类型有 app 内 preview 模块，但不是所有文件都内嵌预览；`ai-chat2` 第一版要求图片内嵌预览，同时要求 macOS 普通文件走 Finder 同级 Quick Look panel。

## 文件和模块结构

初始计划曾按独立子模块拆分如下；实际第一版已按当前职责拆出附件操作流、附件视图、附件常量/格式化和图片预览组件，把分类/入库放入 `state/attachments.rs`。不要新增 `mod.rs`。

实际第一版落地文件：

```text
app/ai-chat2/src/components/chat_form.rs
app/ai-chat2/src/components/chat_form/attachment_flow.rs
app/ai-chat2/src/components/chat_form/attachment_views.rs
app/ai-chat2/src/components/chat_form/attachments.rs
app/ai-chat2/src/components/image_preview.rs
app/ai-chat2/src/components/conversation_detail/attachments.rs
app/ai-chat2/src/components/chat_form/composer_editor.rs
app/ai-chat2/src/components/chat_form/composer_editor/snapshot.rs
app/ai-chat2/src/state.rs
app/ai-chat2/src/state/attachments.rs
app/ai-chat2/src/state/conversations.rs
app/ai-chat2/src/components/conversation_detail.rs
app/ai-chat2/src/features/home/new_conversation.rs
app/ai-chat2/src/features/temporary.rs
app/ai-chat2/src/state/hotkey.rs
app/ai-chat2/src/foundation/assets.rs
app/ai-chat2/locales/en-US/main.ftl
app/ai-chat2/locales/zh-CN/main.ftl
crates/ai-chat-agent/Cargo.toml
crates/ai-chat-agent/src/history.rs
crates/ai-chat-agent/src/runtime.rs
crates/ai-chat-db/src/records.rs
crates/ai-chat-db/src/repository.rs
crates/ai-chat-db/src/tests.rs
crates/window-ext/src/lib.rs
```

初始拆分计划记录如下，作为后续继续拆模块时的参考：

```text
app/ai-chat2/src/components/chat_form.rs
app/ai-chat2/src/components/chat_form/
  attachment_menu.rs
  attachments.rs
  attachment_preview.rs
  composer_editor.rs
  composer_editor/snapshot.rs
app/ai-chat2/src/platform.rs
app/ai-chat2/src/platform/clipboard.rs
app/ai-chat2/src/state.rs
app/ai-chat2/src/state/attachments.rs
app/ai-chat2/src/state/conversations.rs
app/ai-chat2/src/features/home/new_conversation.rs
app/ai-chat2/src/features/temporary.rs
app/ai-chat2/src/components/conversation_detail.rs
app/ai-chat2/locales/en-US/main.ftl
app/ai-chat2/locales/zh-CN/main.ftl
app/ai-chat2/src/foundation/assets.rs
crates/window-ext/Cargo.toml
crates/window-ext/src/lib.rs
crates/window-ext/src/quick_look.rs
```

| 模块 | 实际/计划状态 |
| --- | --- |
| `components/chat_form.rs` | 声明 `mod attachment_flow; mod attachment_views; mod attachments;`；持有 composer attachments、下一附件 local id、preview attachment、处理整体提交/能力 gating/发送按钮状态和 root layout；`+` 菜单 open/dismiss/focus 由 `gpui-component` `DropdownMenu` 管理；不直接写 DB。 |
| `components/chat_form/attachment_flow.rs` | 已落地：处理 `PasteAttachment`、当前剪贴板读取、系统文件选择器、`ExternalPaths` drop 后的 path 添加、attachment add result、remove、图片 preview 打开、文件 Quick Look/open、notification 和 capability support message。 |
| `components/chat_form/attachment_views.rs` | 已落地：渲染 `+` 附件菜单、attachment strip、图片/文件 attachment card、remove button 和 unsupported model warning 行。 |
| `components/chat_form/attachments.rs` | 已落地：集中定义 strip/item 尺寸常量和文件大小格式化。 |
| `components/image_preview.rs` | 已落地：共享当前窗口全屏 overlay 图片预览组件，内部管理自然尺寸、fit zoom、zoom ramp、scroll handle、pinch/scroll 缩放和工具栏；ChatForm 附件和 conversation timeline 用户图片共同复用。 |
| `components/conversation_detail/attachments.rs` | 已落地：按 user message `ContentPart::Image` 顺序提取 image attachment record，渲染用户图片缩略图并接共享 preview。 |
| `components/chat_form/attachment_menu.rs` | 未采用独立文件；菜单视图当前在 `attachment_views.rs`，避免只有一个小菜单时额外拆出碎片模块。 |
| `components/chat_form/image_preview.rs` / `components/chat_form/attachment_preview.rs` | 未保留在 ChatForm 子模块；图片预览实际提升为 `components/image_preview.rs`，避免 conversation timeline 再复制一份预览组件。 |
| `components/chat_form/composer_editor/snapshot.rs` | 扩展 `ComposerAttachment` 为真实附件 snapshot；`ComposerSnapshot::is_empty` 同时看文本、skills 和附件。 |
| `platform/clipboard.rs` | 未采用独立文件；第一版直接使用 GPUI `ClipboardItem` / `ClipboardEntry`，不引入 `clipboard-rs`。 |
| `state/attachments.rs` | 已落地：附件分类、临时 PNG 落盘、入库、`ContentPart` 合成 helper，避免 UI 直接散落 DB/file IO。 |
| `state/conversations.rs` | 已落地：`CreateConversationRequest` / `SendConversationMessageRequest` 接收 composer attachment snapshot；在写入 user item 前插入 `attachments` 表并合成最终 content parts。 |
| `features/home/new_conversation.rs` / `components/conversation_detail.rs` / `features/temporary.rs` | 已落地：继续由页面层负责 conversation 创建/追加和 agent run 启动；发送成功后清空 ChatForm 文本和附件。 |
| `app/ai-chat2/src/platform.rs` / `platform/quick_look.rs` | 未新增 app-local platform facade；当前直接由 `attachment_flow.rs` 调用 `window_ext::preview_file_with_quick_look`，失败后 fallback 到 `cx.open_with_system`。 |
| `crates/window-ext/src/lib.rs` | 已落地 macOS `QLPreviewPanel` bridge，封装 datasource/controller 生命周期、path -> `NSURL` 转换和 shared panel show/reload；非 macOS 返回 unsupported。 |
| `crates/window-ext/Cargo.toml` | 未新增 `objc2-quick-look-ui`；实际复用已有 `objc2` / `objc2-app-kit` / `objc2-foundation` 并通过 runtime bridge 调用 Quick Look。 |

## 自定义类型

`state/attachments.rs` 已定义 app 级、可测试的附件模型；当前实现用本地 `path` 表达文件来源，不再保留早期计划里的独立 `ComposerAttachmentSource`：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComposerAttachmentKind {
    Image,
    File,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ComposerAttachment {
    pub(crate) local_id: u64,
    pub(crate) kind: ComposerAttachmentKind,
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttachmentCapabilityBlock {
    ImagesUnsupported,
    FilesUnsupported,
    ImagesAndFilesUnsupported,
}
```

`components/chat_form.rs` 的 `ChatForm` 当前附件相关本地字段：

```rust
attachments: Vec<ComposerAttachment>,
next_attachment_id: u64,
preview_attachment: Option<ComposerAttachment>,
```

未新增 `attachment_menu.rs` 和 `AttachmentActionOption`。当前只有两个固定菜单项，“添加文件”和“从剪贴板添加”，直接在 `attachment_views.rs` 里用 `PopupMenuItem` 渲染；菜单状态、dismiss、focus 和键盘行为由 `DropdownMenu` 承担。

`attachments.rs` 计划只暴露 app-local render helper，不引入独立 `Global` 或跨 feature state：

```rust
pub(super) enum AttachmentStripAction {
    Remove(ComposerAttachmentId),
    PreviewImage(ComposerAttachmentId),
    OpenFile(ComposerAttachmentId),
    DropExternalPaths(Vec<PathBuf>),
}
```

`app/ai-chat2/src/platform/quick_look.rs` 计划定义 app 层预览结果：

```rust
pub(crate) enum FilePreviewOutcome {
    QuickLookShown,
    OpenedWithSystem,
}

pub(crate) enum FilePreviewError {
    QuickLookUnavailable(String),
    OpenWithSystemUnavailable(String),
}
```

`crates/window-ext/src/quick_look.rs` 计划定义平台层类型：

```rust
pub struct QuickLookPreviewItem {
    pub path: PathBuf,
    pub title: Option<String>,
}

pub enum QuickLookPreviewError {
    UnsupportedPlatform,
    MainThreadUnavailable,
    InvalidFileUrl(PathBuf),
    PanelUnavailable,
    ObjectiveCBridge(String),
}

pub trait QuickLookExt {
    fn show_quick_look_preview(
        &self,
        items: &[QuickLookPreviewItem],
        selected_index: usize,
    ) -> Result<(), QuickLookPreviewError>;
}
```

macOS bridge 内部计划用 `objc2::declare_class!` 声明 datasource/controller：

```rust
// Pseudocode only; final signatures must match objc2-quick-look-ui 0.3.2.
struct QuickLookPanelController {
    items: Vec<Retained<NSURL>>,
}

unsafe impl QLPreviewPanelDataSource for QuickLookPanelController {
    fn numberOfPreviewItemsInPreviewPanel(&self, panel: &QLPreviewPanel) -> NSInteger;
    fn previewPanel_previewItemAtIndex(
        &self,
        panel: &QLPreviewPanel,
        index: NSInteger,
    ) -> Retained<AnyObject>; // NSURL conforms to QLPreviewItem.
}
```

`platform/clipboard.rs` 计划定义粘贴板输出，不让 UI 依赖 `clipboard-rs` 具体类型：

```rust
pub(crate) enum ClipboardAttachmentPayload {
    Files(Vec<PathBuf>),
    Image(ClipboardImagePayload),
    Text(String),
    Empty,
}

pub(crate) struct ClipboardImagePayload {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
```

`composer_editor/snapshot.rs` 中的 snapshot 类型计划调整为：

```rust
pub(crate) struct ComposerAttachmentSnapshot {
    pub(crate) local_id: ComposerAttachmentId,
    pub(crate) kind: ComposerAttachmentKind,
    pub(crate) name: String,
}

pub(crate) struct ComposerSnapshot {
    pub(crate) text: String,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) token_ranges: Vec<ComposerTokenSnapshot>,
    pub(crate) attachments: Vec<ComposerAttachmentSnapshot>,
    pub(crate) send_policy: ComposerSendPolicy,
}
```

`content_parts` 在提交前仍允许只有 text part；最终带 `attachment_id` 的 `ContentPart::Image/File` 由 `state::attachments` 入库后合成，避免 composer 持有还不存在的 DB id。

## UI 组件

- 外层继续使用现有 `ChatForm` 的 `v_flex` surface，不新增嵌套 card。
- `ChatForm` surface 继续使用 `.rounded(px(25.))`、`.border_1()`、`.bg(cx.theme().input_background())`，附件 strip 插在 input surface 内部、`ComposerEditor` 上方，不进入 footer。
- 附件 strip 使用 `h_flex` + 横向 overflow/scroll；无附件时完全不渲染，避免改变输入框上方高度和 padding。
- 整个 `ChatForm` surface 增加 `ExternalPaths` drop target：drag-over 时通过单独 overlay child 显示轻量覆盖层；drop 后不改变 focus，不把 path 文本插入 editor。
- 图片 item 使用专用缩略图容器，不使用通用 card：外层固定 square thumbnail 和删除按钮定位，内层 0 inset `rounded + overflow_hidden` 裁剪 `img(path).ObjectFit::Cover`，另一个 0 inset overlay 绘制圆角边框；点击图片主体打开 app 内全窗口 preview overlay。
- 文件 item 使用 `Icon::new(IconName::File)`、文件名、size 文本和 remove button；点击主体调用 open file，不点击 remove。
- remove 使用 `Button::new(...).ghost()` + `IconName::X`，尺寸固定，不随 hover 改变 layout；hover 只改变背景/前景色。
- `+` 使用 `gpui-component` `DropdownMenu` / `PopupMenuItem`，菜单项为“添加文件”和“从剪贴板添加”；不保留 app-local popover open state。
- 图片预览使用 `components/image_preview.rs` 的当前窗口全窗口 overlay，支持 fit zoom、按钮缩放、触控板 pinch/scroll 和双向滚动；不再使用固定小 dialog。
- 错误和 unsupported model 使用现有 `gpui_component::notification::Notification`；发送按钮 tooltip 显示禁用原因。

## 样式和间距落地值

附件 strip 的当前尺寸落到常量，放在 `components/chat_form/attachments.rs`，不要散落在 render 链里：

```rust
pub(super) const STRIP_GAP: f32 = 8.;
pub(super) const STRIP_BOTTOM_MARGIN: f32 = 8.;
pub(super) const IMAGE_THUMBNAIL_SIZE: f32 = 80.;
pub(super) const FILE_CARD_WIDTH: f32 = 220.;
pub(super) const FILE_CARD_HEIGHT: f32 = 56.;
pub(super) const CARD_RADIUS: f32 = 8.;
pub(super) const REMOVE_BUTTON_SIZE: f32 = 20.;
```

`ChatForm` input surface 结构调整为：

```text
ChatForm surface
  input area v_flex
    AttachmentSupportMessage? // only when current model/runtime cannot send selected attachments
    AttachmentStrip?        // only when attachments is not empty
    ComposerEditor
  footer row
    + / reasoning controls
    model / send-or-stop
```

具体 layout 规则：

- `input area v_flex` 继续使用当前 `px(12)`、`pt(12)`、`mb(4)` 和 `min_h(56)`；warning、附件 strip 和 editor 都是 normal flow sibling，由 parent `.gap(px(STRIP_BOTTOM_MARGIN))` 管理纵向间距，不做 absolute stacking。
- 有附件时，strip 使用 `.w_full().overflow_x_scroll()`；strip 内层使用 `.h_flex().items_end().gap(px(8.))`，每个 item 固定宽度并 `.flex_none()` 来形成横向滚动内容。若 GPUI 当前没有隐藏 scrollbar 的稳定 API，第一版允许显示系统滚动条，不能为了隐藏滚动条改写组件。
- 无附件时不渲染 strip，也不保留占位高度，保持现有 composer 基础高度。
- 图片 attachment item 固定 `.size(px(80.))`、`.flex_none()`、外层 `.rounded(px(8.))`；图片内容在 0 inset 内层 `.rounded(px(8.)).overflow_hidden()` 中 `ObjectFit::Cover`；圆角边框由 0 inset overlay 的 `.border_1()` 绘制，不给图片增加 padding、margin 或 1px inset。
- 文件 attachment card 固定 `.w(px(220.))`、`.h(px(56.))`、`.p_2()`、`.gap_2()`、`.rounded(px(8.))`、`.border_1()`、`.flex_none()`；文件名单行 truncate，size 文本使用 muted foreground。
- 图片和文件的 remove button 都是右上角 absolute overlay，`.size(px(20.))`、`.top(px(4.))`、`.right(px(4.))`；点击 remove button 会 `stop_propagation()`，避免触发图片 preview 或文件 open。
- `+` 菜单由 `gpui-component` `DropdownMenu`/`PopupMenuItem` 渲染和管理；ChatForm 不再持有 `attachment_menu_open` 或手写 `Popover`。
- `ExternalPaths` drag-over overlay 是 `ChatForm` surface 内的 absolute child：默认 `.invisible()`，位置 `.top_0().right_0().bottom_0().left_0()`、`.rounded(px(25.))`、`.border_1()`，使用 theme primary 色和低透明背景；overlay 自身用 `.drag_over::<ExternalPaths>(|style, _, _, _| style.visible())` 变为可见。
- unsupported capability 会在 input area 顶部渲染 normal flow warning 行，并同步禁用 send button；这避免 warning、附件 strip、editor 和 footer 相互重叠。

Codex app 参考值和 `ai-chat2` 第一版落地值对应关系：

| Codex app 观察值 | `ai-chat2` 第一版 |
| --- | --- |
| 父 surface `flex-col gap-2` | 复用现有 ChatForm `v_flex`；附件 strip 在 input area 内，和 editor 间距固定 `8px`。 |
| row 外层 `w-full overflow-x-auto hide-scrollbar` | `w_full + overflow_x_scroll`；隐藏 scrollbar 仅在 GPUI 有稳定 API 时做。 |
| row 内层 `flex min-w-max items-end gap-2` | `h_flex + items_end + gap(8px)`，由固定宽度 item 和 `flex_shrink_0` 达成横向内容宽度。 |
| 普通 image attachment 横向步进 `88px` | image item `80px`，gap `8px`。 |
| appshot card `232px x 140px`，步进 `240px` | 第一版不做 appshot；后续 screenshot context card 以该值为参考。 |
| inline 小预览 `28px x 20px` | 仅作为 inline mention 参考；附件 strip 文件 icon 固定 `18px`。 |
| remove button 单独组件 | `20px` 圆形 ghost icon button，图片和文件右上角 `4px` inset overlay。 |

## Global State 和数据归属

- Pending attachments 属于 `ChatForm` entity 本地状态，不进入 `Global`，也不进入 `AiChat2AppSettings`。
- `state::attachments` 是 stateless app service/helper，读取 `AiChat2Config::data_dir()` 和 `database::repository(cx)`；不新增长期 global store。
- Quick Look native panel state 属于平台层 transient state，不进入 DB、config 或 GPUI `Global`。`window-ext` 内部持有 datasource/controller 只是为了满足 AppKit assign datasource 生命周期。
- 发送成功后由 `ChatForm::clear_after_submit` 同时清空文本和 pending attachments。
- 发送失败时保留 pending attachments，用户可以删除、切换模型或重试。
- Home、Temporary、Conversation detail 都继续通过共享 `ChatFormSubmit` 传递 composer snapshot，不在 feature 层重复分类逻辑。
- 附件文件的本机存储目录复用现有思路：`<data_dir>/attachments/<conversation_id>/<user_item_id>-<local_id>-<sanitized-name>`；clipboard raw image 固定写 PNG。

## 数据库变更

- 不新增 migration，不修改 fresh schema。
- 继续使用 `attachments` 表字段：`conversation_id`、`kind`、`storage_kind`、`mime_type`、`name`、`path`、`size_bytes`、`metadata_json`。
- image attachment 写入：
  - `kind = AttachmentKind::Image`
  - `storage_kind = AttachmentStorageKind::LocalFile`
  - `metadata.source = AttachmentSource::LocalFile { path }`
  - `metadata.width/height` 尽量写入
  - `ContentPart::Image { attachment_id }`
- generic file attachment 写入：
  - `kind = AttachmentKind::File`
  - `storage_kind = AttachmentStorageKind::LocalFile`
  - `metadata.source = AttachmentSource::LocalFile { path }`
  - `metadata.width/height = None`
  - `ContentPart::File { attachment_id }`
- `conversation_items.payload_json` 只保存 `ContentPart` refs，不保存二进制内容或 base64。

## 数据获取方式

- 文件选择：使用 GPUI `cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, ... })`。
- 粘贴板：使用 GPUI `ClipboardItem` / `ClipboardEntry::{ExternalPaths, Image, String}` 读取格式并按 `Files -> Image -> Text` 分类；`clipboard-rs` 暂不引入。
- 文件 metadata：使用 `std::fs::metadata` 获取大小；文件名来自 `Path::file_name`。
- 图片判定：扩展名/MIME 初筛后用 `image` crate 解码确认，并读取宽高；不支持的图片格式按 generic file 或错误提示处理，具体取舍在实现前固定。
- 模型能力：继续使用 `ChatForm::selected_model_choice()` 的 `ModelCapabilitiesSnapshot`，不从 provider metadata 临时推断。
- DB 写入：统一走 `state::conversations` 调 `state::attachments`，最终调用 `FreshRepository::insert_attachment`。

## Icons 和 i18n

计划新增或确认以下 `IconName`：

- `Image` -> `image`
- `File` -> `file`
- `Paperclip` -> `paperclip`
- `ExternalLink` 已存在
- `Plus` 已存在
- `X` 已存在

计划新增 en-US / zh-CN 文案：

- `chat-form-add-file = Add File...` / `添加文件...`
- `chat-form-drop-files = Drop files to attach` / `松开以添加文件`
- `chat-form-attachment-remove = Remove attachment` / `移除附件`
- `chat-form-attachment-open = Open file` / `打开文件`
- `chat-form-attachment-preview = Preview image` / `预览图片`
- `chat-form-file-preview = Preview file` / `预览文件`
- `chat-form-quick-look-failed = Quick Look preview failed` / `Quick Look 预览失败`
- `chat-form-open-with-system = Open with system app` / `使用系统程序打开`
- `chat-form-attachment-size-bytes = { $size } bytes` / `{ $size } 字节`
- `chat-form-paste-attachment-failed = Paste attachment failed` / `粘贴附件失败`
- `chat-form-add-attachment-failed = Add attachment failed` / `添加附件失败`
- `chat-form-model-images-unsupported = Selected model does not support image input` / `当前模型不支持图片输入`
- `chat-form-model-files-unsupported = Selected model does not support file input` / `当前模型不支持文件输入`
- `chat-form-model-attachments-unsupported = Selected model does not support these attachments` / `当前模型不支持这些附件`

## 新增依赖

- 已新增 `crates/ai-chat-agent` 直接依赖 `base64 = "0.22.1"`，用于把本地图片/PDF attachment 转为 Rig provider 可消费的 base64 content。
- 已把 `app/ai-chat2` 的 `image` feature 从仅 `png` 扩展到 `png`、`jpeg`、`gif`、`webp`，用于图片文件 decode/尺寸读取。
- 未引入 `clipboard-rs`：实际采用 GPUI `ClipboardItem` / `ClipboardEntry::{ExternalPaths, Image, String}`，能覆盖第一版 Files -> Image -> Text 分类。
- 未引入 `objc2-quick-look-ui`：实际 Quick Look bridge 复用 `window-ext` 已有 `objc2` / `objc2-app-kit` / `objc2-foundation`，通过 runtime class/protocol 调用 `QLPreviewPanel`。
- MIME/扩展名判断第一版继续手写白名单；如果后续需要 OS MIME sniffing，再评估 `infer` 或 `mime_guess`，并使用完整版本号。

## Clipboard 判定

- 实现采用 GPUI clipboard，不额外引入 `clipboard-rs`。
- `secondary-c` 在 `ComposerEditor` 内读取 `cx.read_from_clipboard()`；如果发现 `ExternalPaths` 或 `Image`，发出 `ComposerEditorEvent::PasteAttachmentRequested` 并消费事件；普通文本粘贴仍由原 `ComposerEditor` 文本路径处理。
- 判定顺序：`Files -> Image -> Text`。
- `Files`：读取文件路径，目录第一版拒绝并提示；文件按 MIME/扩展名和可解码性区分 image/file。图片文件加入 image attachment，普通文件加入 file attachment。
- `Image`：纯 clipboard image 写入 app attachment pending store，再作为 image attachment 进入 composer。
- `Text`：交还 `ComposerEditor` 普通文本粘贴，不进入附件流。

## Drag and Drop 判定

- `ChatForm` render root 增加 drop listener，并增加一个默认 invisible 的 overlay child：

```rust
.on_drop(cx.listener(|form, paths: &ExternalPaths, window, cx| {
    form.add_external_paths(paths.paths(), window, cx);
}))
.child(
    div()
        .invisible()
        .absolute()
        .top(px(DROP_TARGET_INSET))
        .right(px(DROP_TARGET_INSET))
        .bottom(px(DROP_TARGET_INSET))
        .left(px(DROP_TARGET_INSET))
        .drag_over::<ExternalPaths>(|style, _, _, _| style.visible())
        .child(/* optional centered "Drop files to attach" label */)
)
```

- `add_external_paths` 只做 path collection 和错误展示，真正分类复用 `state::attachments::classify_local_paths`。
- `ExternalPaths` 中的多个 path 按原顺序添加；重复 path 第一版允许重复显示，后续如有需要再做去重。
- Drop 到 ChatForm 以外区域不触发添加；不做全窗口 drop 捕获，避免和 sidebar/project panel 等未来区域冲突。
- Drag-over overlay 只表示“这些路径可以被 ChatForm 接收”，不提前做耗时 metadata/image decode；drop 后再分类并提示不支持项。

## Data Flow

1. 用户通过 `secondary-c`、`+` 菜单或把系统文件拖拽到 `ChatForm` 范围添加文件/图片。
2. `ComposerEditor` 对 clipboard image/files 发 `PasteAttachmentRequested`；`+` 菜单读取当前 GPUI clipboard 或打开 `cx.prompt_for_paths`；drop flow 读取 `ExternalPaths.paths()`。
3. 三条入口统一调用 `state::attachments::{add_attachments_from_clipboard, add_attachments_from_paths}` 分类。
4. `ChatForm` 在文本输入上方渲染 attachment strip；无附件时不渲染该行。
5. 用户发送时，`ChatForm` 基于当前 model capabilities 计算是否允许发送。
6. `state::conversations` 把 pending attachments 插入 fresh `attachments` 表，图片写 `AttachmentKind::Image` / `ContentPart::Image`，普通文件写 `AttachmentKind::File` / `ContentPart::File`。
7. `crates/ai-chat-db` 的 timeline snapshot 带出 attachment records，`ai-chat-agent::history` 把 `ContentPart` 和 attachment records 合成 Rig `UserContent`。
8. 最终 user message payload 使用 `ConversationItemPayload::Message { role: User, content }`，文本 part 排在附件 part 前；没有文本时只发送附件 parts。
9. 发送成功后清空 composer 文本和 pending attachments；发送失败保留附件，方便用户重试。

## Preview 和 Open

- 图片缩略图点击打开 app 内 preview dialog，使用 `img(path)` 和 `ObjectFit::Contain`，背景、最大尺寸和 close 行为遵循现有 `gpui-component` dialog 模式。
- 文件 chip 点击在 macOS 第一版先调用 `window_ext::preview_file_with_quick_look(&path)`，打开 `QLPreviewPanel` shared panel，预览当前文件；多选预览后续可把同一 composer 的 file attachments 都传入 items。
- macOS Quick Look 失败时记录 warning 并 fallback 到 `cx.open_with_system(&path)`，保证用户仍能打开文件。后续如果需要更严格的 UX，可把 fallback 改成 notification + 显式按钮。
- Windows/Linux 第一版没有 Finder 级 Quick Look 等价物，点击文件 chip 通过同一 facade 返回 unsupported 后 fallback 到 `cx.open_with_system(&path)`。
- `cx.open_with_system` 是 fallback/open 行为，不计入 macOS Finder 级 preview 的完成标准。

## Capability Gating

- image attachment 需要 selected model 支持 `image_input`。
- file attachment 需要 selected model 支持 `file_input`。
- 两者同时存在时分别计算缺失能力；发送按钮禁用并显示“模型不支持图片/文件输入”原因。
- 即使模型声明支持 file input，第一版仍会拦截当前 runtime 不能序列化的文件类型，并提示“已支持图片、PDF 和文本类文件”。这避免 zip/Office 等二进制文件被静默降级或发送后才失败。
- provider capability 以“官方/平台声明 + 当前 runtime adapter 可发送”为准：OpenAI、Anthropic、Gemini 和 OpenRouter 的图片/文件输入已可放行；Ollama 仍以 `/api/show` 的 `vision` / `tools` 为准；DeepSeek 当前无图片输入；Mistral vision 暂被 runtime adapter 限制，后续应先补 provider adapter 或升级 Rig 后再打开 image input。
- 不做旧 `provider_models.capabilities_json` 兼容迁移；已有缓存模型需要重新刷新 provider model list 才能得到新的 capability snapshot。
- 不把不支持的图片/文件转换成纯文本 path，也不静默丢弃附件。
- `+` 菜单和 paste 可以继续添加附件；gating 只阻止发送，方便用户切换模型后继续发送。
- 添加文件使用 `PathPromptOptions { prompt: None, ... }`，不覆盖 macOS `NSOpenPanel` 默认 prompt；系统文件对话框语言由 app bundle localization 决定。

## Test Plan

- 已覆盖 unit tests：image file extension 判定、文件名 sanitize、unsupported capability gating、当前 runtime 不支持的 file type gating、agent history 把 image attachment 转成 Rig image content、unsupported binary file 明确 rejected。
- 已覆盖 DB tests：`conversation_timeline_records` 带出 attachments；现有 typed JSON roundtrip 覆盖 `AttachmentMetadata`。
- 已执行验证：`cargo fmt`、`cargo check -p ai-chat2`、`cargo test -p ai-chat2 chat_form`、`cargo test -p ai-chat2 attachments`、`cargo test -p ai-chat-agent history`、`cargo test -p ai-chat-db attachment`、`cargo test -p ai-chat-db typed_json_roundtrips_for_repository_records`、`git diff --check`、`cargo clippy -p window-ext -p ai-chat2 -p ai-chat-db -p ai-chat-agent --all-targets --all-features -- -D warnings`。
- 2026-06-14 结构拆分后补充验证：`cargo fmt --check`、`cargo check -p ai-chat2`、`cargo test -p ai-chat2 chat_form`、`cargo test -p ai-chat-agent history`、`cargo test -p ai-chat-db attachment`、`cargo test -p ai-chat-db typed_json_roundtrips_for_repository_records`、`cargo clippy -p window-ext -p ai-chat2 -p ai-chat-db -p ai-chat-agent --all-targets --all-features -- -D warnings`、`git diff --check`。
- 2026-06-14 附件样式、菜单和文件对话框 polish 后补充验证：`cargo fmt`、`cargo check -p ai-chat2`、`cargo test -p ai-chat2 chat_form`、`cargo test -p xtask bundle`、`git diff --check`、`cargo run -p xtask -- bundle ai-chat2`，并检查生成的 `AI Chat 2.app/Contents/Info.plist` 和 `Resources/{en,zh_CN}.lproj`。
- 2026-06-14 provider capability profile 修正后补充验证：`cargo fmt`、`cargo test -p ai-chat-agent model_capabilities`、`cargo test -p ai-chat-agent provider_models`。后续提交前仍需补 `cargo check -p ai-chat2` 和 `git diff --check`。
- 仍需手动 UI 验证：粘贴截图、粘贴 Finder 图片文件、拖拽 Finder 文件到 ChatForm、粘贴 PDF/文本文件、删除附件、图片 app 内预览、macOS 文件 Quick Look panel 预览、Quick Look 失败 fallback、无附件时不显示附件行。
- 如本轮准备提交 PR，可按 CI 预算再补 full workspace validation；当前聚焦子集已通过。

## 文档状态

- 本文档已从实施计划更新为第一版实现记录。
- `app/ai-chat/docs/dev/issue-159/README.md` 已把 ChatForm / attachments / capability gating 状态同步为第一版已实现，完整 timeline multimodal rendering 仍待补。
- `app/ai-chat/docs/dev/issue-137/README.md` 已把 #159 剩余项更新为 ChatForm 多模态输入第一版已落地。
