# Issue #159 ai-chat2 Global Shortcuts 设置专项计划

本文档是 `app/ai-chat2` Global Shortcuts 设置页和快捷键执行流的可执行开发计划。父级 UI
清单仍是 `app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档固定后续实现时的页面结构、
fresh shortcut 数据模型使用方式、状态流、全局热键刷新、自动发送流程、i18n、icon、依赖和验证要求。

创建时间：2026-06-13。

当前状态：第一版已于 2026-06-13 落地。Settings Shortcuts 页面、fresh shortcuts CRUD、
runtime 注册刷新、selected text/clipboard 触发、screenshot overlay、image attachment 路径和 OCR fallback
已实现；不新增也不展示 shortcut `name` / `title` 字段。

## 目标边界

- 在 Settings 中新增 Shortcuts 页面，用户可以查看全部全局快捷键、新增快捷键、查看快捷键、修改快捷键、
  删除快捷键、启停快捷键、查看注册/运行状态，并手动重新注册失败的快捷键。
- v1 全局快捷键触发后执行旧 `app/ai-chat` 的核心全局快捷键行为：选中文字/剪贴板或截图输入
  -> Temporary Window 的新临时对话 -> 启动 agent run。
- v1 只暴露 `ShortcutAction::OpenTemporaryConversation` 语义，即“临时窗口的新临时对话”。不做指定已有
  conversation 的快捷键，不做 conversation picker，不暴露 `SendToConversation`。
- Shortcuts 绑定 prompt、provider/model、input source 和 action，不绑定旧 template，也不恢复旧
  `Mode::{Contextual, Single, AssistantOnly}` 控件。`ai-chat2` 所有 conversations 继续是 contextual。
- shortcut record 不新增 `name` / `title` 字段。页面每一行也不展示任何“标题”字段或由记录生成的标题。
  行主信息直接由 hotkey、prompt、provider/model、input source、action、状态、更新时间和操作按钮组成。
  不用 hotkey、prompt 或 model 派生出一个 synthetic title，也不在 dialog/view model 中保留 title/name。
- General 页面已有 `temporary_hotkey` 仍留在 General；Shortcuts 页面只管理 `shortcuts` 表中的用户快捷动作，
  但必须把 temporary hotkey 纳入冲突校验。

## 非目标

- 本阶段不迁移旧 `global_shortcut_bindings` 数据到 fresh database。
- 不实现已有 conversation 绑定、conversation 选择器、shortcut 重排序或批量导入导出。
- 不实现 Prompt selector 在 Composer 中的 UI；Shortcuts 设置页只消费已存在的 prompts。
- 不把 shortcut definitions 写入 app config 文件；持久化真相是 fresh `shortcuts` 表。
- 不把截图 binary data 写进 message text、prompt text 或 `settings_snapshot_json`。

## 落地状态

- Settings 页面已新增 Shortcuts 项，使用 Prompt Settings 同类 toolbar + rows/empty/error 管理形态，不恢复旧大表格。
- `crates/ai-chat-db` 已补齐 `UpdateShortcut`、`update_shortcut`、`set_shortcut_enabled`、`delete_shortcut`。
- `app/ai-chat2/src/state/shortcuts.rs` 已作为 shortcut mutation facade，负责 DB mutation、settings snapshot 构建和 runtime 注册同步。
- `app/ai-chat2/src/state/hotkey.rs` 已把 pressed shortcut 从 diagnostics-only 改成真实执行：
  - `SelectionOrClipboard`：先读 selected text，空时 fallback GPUI clipboard。
  - `Screenshot`：打开 screenshot overlay；模型支持 image input 时写入 `attachments` + `ContentPart::Image`，否则走 OCR text。
  - 每次成功触发都创建新的 no-project scratch conversation，并以 `AgentRunTriggerKind::Shortcut` 启动 agent run。
- `app/ai-chat2/src/features/settings/shortcuts/{rows,dialog,choices,validation}.rs` 已实现列表行、查看/新增/编辑/删除弹窗、选择项和 hotkey 冲突校验。
- `app/ai-chat2/src/platform/{capture,display}.rs` 和 `features/screenshot/overlay.rs` 已承接截图选择、显示器定位和平台捕获。
- `app/ai-chat2/locales/{en-US,zh-CN}/main.ftl` 已补 Shortcuts 页面、状态、通知和校验文案。
- 新增依赖已固定完整版本：`image = "0.25.10"`、`get-selected-text = "0.1.6"`、`xcap = "0.9.4"`。

## 当前基础

- `crates/ai-chat-core` 已有：
  - `ShortcutInputSource::{SelectionOrClipboard, Screenshot}`
  - `ShortcutAction::{OpenTemporaryConversation, SendToConversation { conversation_id }}`
  - `AgentRunTriggerKind::Shortcut`
  - `RunSettingsSnapshot`
- `crates/ai-chat-db` 已有 `shortcuts` 表和 `ShortcutRecord` / `NewShortcut`：
  - `hotkey TEXT NOT NULL UNIQUE`
  - `enabled BOOLEAN NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))`
  - `prompt_id TEXT REFERENCES prompts(id) ON DELETE SET NULL`
  - `provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL`
  - `model_id TEXT`
  - `input_source TEXT NOT NULL CHECK (...)`
  - `action_json JSON NOT NULL`
  - `settings_snapshot_json JSON NOT NULL DEFAULT '{}'`
- `app/ai-chat2` 已有：
  - `components::hotkey_input::HotkeyInput` 和 `format_hotkey_label`
  - `state::hotkey::GlobalHotkeyState`，目前会注册 temporary hotkey 和 enabled shortcuts；shortcut
    触发后会按 fresh shortcut record 执行临时对话 agent run。
  - `state::prompts::PromptCatalogStore`
  - `state::providers::ProviderCatalogStore` 和 `enabled_provider_models`
  - Temporary Window 首版和 `state::conversations::create_conversation`

## 数据模型和数据库计划

不新增 shortcut 标题字段。目标 record 继续使用现有 shape：

```rust
pub struct ShortcutRecord {
    pub id: ShortcutId,
    pub hotkey: String,
    pub enabled: bool,
    pub prompt_id: Option<PromptId>,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ProviderModelId>,
    pub input_source: ShortcutInputSource,
    pub action: ShortcutAction,
    pub settings_snapshot: RunSettingsSnapshot,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

需要补齐的 repository API：

- `UpdateShortcut`
  - `hotkey: String`
  - `enabled: bool`
  - `prompt_id: Option<PromptId>`
  - `provider_id: Option<ProviderId>`
  - `model_id: Option<ProviderModelId>`
  - `input_source: ShortcutInputSource`
  - `action: ShortcutAction`
  - `settings_snapshot: RunSettingsSnapshot`
- `update_shortcut(id, UpdateShortcut) -> Result<ShortcutRecord>`
- `delete_shortcut(id) -> Result<usize>`
- `set_shortcut_enabled(id, enabled) -> Result<ShortcutRecord>`
- `list_shortcuts()` 排序保持 deterministic：`created_at ASC` 即可；不新增 sort_order。

保存规则：

- `hotkey` 保存前 canonicalize 成 `super+shift+k` 这类 `+` 分隔格式。
- 空 hotkey、不可 parse hotkey、与 temporary hotkey 冲突、与其他 shortcut 冲突都在 UI/state 层拦截，不写 DB。
- DB 的 `hotkey UNIQUE` 继续作为最后防线。
- prompt 可为空；provider/model v1 必须选择一个 enabled model 后才能保存。
- `action_json` v1 写 `ShortcutAction::OpenTemporaryConversation`。
- `settings_snapshot_json` 保存去 secret 的 run settings snapshot，包括：
  - prompt snapshot：当前 prompt 的 `PromptContent { text }`，如果未选择 prompt 则 `None`
  - provider/model id 和 `ModelCapabilitiesSnapshot`
  - provider settings payload
  - reasoning selection，先使用默认 `None`
  - tool policy，使用 `state::conversations` 当前 default tool policy 语义
- prompt/provider/model 后续被修改或删除，不改已有 shortcut snapshot；列表状态可以显示 Prompt/Model unavailable。

## 模块结构

禁止新增 `mod.rs`。新增模块使用同名入口文件和子目录文件。

```text
app/ai-chat2/src/features/settings.rs
  - 增加 `mod shortcuts;`
  - `SettingsView` 增加 `shortcuts_settings: Entity<ShortcutsSettingsPage>`
  - `SettingsPageKey` 增加 `Shortcuts`
  - `settings_page_specs` 增加 `settings-page-shortcuts` 和搜索词
  - render match 增加 `SettingsPageKey::Shortcuts`
  - Shortcuts page 使用普通 `SettingsPageFrame` 外层滚动；只有 Provider page 继续使用 `no_outer_body_scroll()`

app/ai-chat2/src/features/settings/shortcuts.rs
  - `ShortcutsSettingsPage` 顶层 entity
  - 持有 search input、shortcut records、prompt records、provider/model choices 和 runtime diagnostics
  - 订阅 `state::shortcuts::ShortcutCatalogStore`
  - 订阅 `state::prompts::PromptCatalogStore`
  - 订阅 `state::providers::ProviderCatalogStore`
  - 只通过 state 层 list/create/update/delete/toggle/reregister，不直接散落 SQL 写入

app/ai-chat2/src/features/settings/shortcuts/rows.rs
  - `ShortcutManagementRow`
  - `ShortcutManagementEntry`
  - `shortcut_management_rows(...)`
  - `filter_shortcut_rows(rows, query)`
  - row 不包含 `title` / `name` 字段；主信息由 hotkey、prompt label、provider/model label、
    input source label、action label、status label、updated label 组成

app/ai-chat2/src/features/settings/shortcuts/dialog.rs
  - `ShortcutEditDialogState`
  - `ShortcutEditMode::{Create, Edit}`
  - `open_shortcut_edit_dialog`
  - `open_shortcut_preview_dialog`
  - `open_shortcut_delete_confirm`
  - 编辑 dialog 内使用 HotkeyInput、prompt select、model select、input source select、enabled switch

app/ai-chat2/src/features/settings/shortcuts/choices.rs
  - `PromptChoice`
  - `ShortcutModelChoice`
  - `InputSourceChoice`
  - SelectItem implementations and search text

app/ai-chat2/src/features/settings/shortcuts/validation.rs
  - `ShortcutValidationError`
  - `canonical_hotkey`
  - `validate_shortcut_draft`
  - `shortcut_status`

app/ai-chat2/src/state.rs
  - 增加 `pub(crate) mod shortcuts;`

app/ai-chat2/src/state/shortcuts.rs
  - `ShortcutCatalogGlobal(Entity<ShortcutCatalogStore>)`
  - `ShortcutCatalogEvent::Changed(ShortcutCatalogChange)`
  - `ShortcutCatalogChange::{Created, Updated, Deleted, EnabledChanged, Reregistered}`
  - `list_shortcuts(cx)`
  - `create_shortcut(cx, draft)`
  - `update_shortcut(cx, id, draft)`
  - `delete_shortcut(cx, id)`
  - `set_shortcut_enabled(cx, id, enabled)`
  - `reregister_shortcut(cx, id)`
```

自动发送和截图相关实现模块：

```text
app/ai-chat2/src/state/hotkey.rs
  - 保存/delete/toggle/reregister shortcut 时同步刷新 runtime 注册
  - pressed shortcut 改为 dispatch 到 shortcut execution flow

app/ai-chat2/src/features/hotkey.rs 或 app/ai-chat2/src/state/hotkey/shortcut_flow.rs
  - 解析 shortcut record
  - 采集 selected text / clipboard / screenshot
  - 创建临时 conversation 并启动 agent run

app/ai-chat2/src/features/screenshot.rs
app/ai-chat2/src/features/screenshot/overlay.rs
  - 从旧 `app/ai-chat` 迁移截图 overlay 交互，但引用 fresh shortcut record 和 ai-chat2 state

app/ai-chat2/src/platform.rs
app/ai-chat2/src/platform/capture.rs
  - 从旧 `app/ai-chat` 迁移 capture boundary
  - OCR 继续使用共享 `platform_ext::ocr`
```

## Settings 页面结构

Shortcuts 页面采用 Prompt Settings 同类管理页，而不是旧 `ai-chat` 的大表格。

页面结构：

- 顶部 toolbar：
  - 左侧搜索 `Input`
  - 右侧 `Add Shortcut` `Button`
- 主体：
  - 空列表：empty state + `Add Shortcut`
  - 搜索无结果：no-results state
  - 加载失败：error state + reload
  - 正常：全宽 shortcut 管理行列表

不新增单独的列表标题区域。每一行也不展示 `title` / `name` 字段。

行展示：

- hotkey：用 `format_hotkey_label` 或 `Kbd` 风格展示。
- prompt：显示 prompt name；未绑定显示 `None`；prompt id 失效显示 unavailable。
- model：显示 provider visual icon + provider display name + model display label；失效显示 unavailable。
- input source：Selection/Clipboard 或 Screenshot。
- action：v1 固定 Temporary Conversation。
- status：Enabled、Disabled、Invalid Hotkey、Hotkey Conflict、Prompt Unavailable、Model Unavailable、
  Capability Mismatch、Registration Failed。
- updated time：沿用 Prompt row 的 `YYYY-MM-DD HH:MM` 格式。
- actions：View、Edit、Reregister、Delete。
- enabled switch：直接切换 `shortcuts.enabled`，成功后刷新 runtime 注册。

点击行打开只读查看 dialog。右侧显式按钮需要 `cx.stop_propagation()`，避免触发行查看。

## Dialog 设计

新增/编辑 dialog：

- Hotkey：
  - `HotkeyInput`
  - 保存前 canonicalize
  - plain key 不接受
- Prompt：
  - `Select<PromptChoice>`
  - 允许 None
  - 只列 enabled prompts
- Model：
  - `Select<ShortcutModelChoice>`
  - 只列 enabled provider + enabled model
  - row 使用 provider visual icon、model display label、capability tags
- Input source：
  - `Select<InputSourceChoice>`
  - `SelectionOrClipboard`
  - `Screenshot`
- Enabled：
  - `Switch`
- Action：
  - v1 只显示 Temporary Conversation，不给用户选择其他 action。

查看 dialog：

- 展示 hotkey、prompt、provider/model、input source、action、enabled、status、registration message、
  last pressed diagnostics 和 snapshot summary。
- Footer：Edit、Reregister、Delete、Close。

删除：

- 复用 `components::delete_confirm::open_destructive_confirm_dialog`。
- 删除为硬删除；删除后必须 unregister runtime hotkey，并发 `ShortcutCatalogEvent::Changed`。

## 状态和全局数据管理

`ShortcutCatalogStore` 是 Settings、hotkey runtime 和未来入口共享的唯一 mutation facade。

数据读取：

```text
ShortcutsSettingsPage::new
  -> state::shortcuts::list_shortcuts(cx)
  -> state::prompts::list_prompts(cx)
  -> state::providers::providers_with_models(cx)
  -> state::GlobalHotkeyState::diagnostics_snapshot(cx)
```

保存：

```text
ShortcutEditDialogState::save
  -> validate_shortcut_draft
  -> ShortcutCatalogStore create/update
  -> FreshRepository insert/update
  -> GlobalHotkeyState update shortcut runtime registration
  -> ShortcutCatalogEvent::Changed
  -> ShortcutsSettingsPage reloads records
```

启停：

```text
row Switch
  -> ShortcutCatalogStore set_shortcut_enabled
  -> FreshRepository set_shortcut_enabled
  -> GlobalHotkeyState register/unregister shortcut
  -> ShortcutCatalogEvent::Changed
```

注册失败策略：

- DB 保存成功但 OS hotkey registration 失败时，不回滚 DB。
- runtime diagnostics 记录 `registration_errors[shortcut_id] = message`。
- Settings row 显示 `RegistrationFailed`，用户可 Edit 或 Reregister。
- 如果 DB 更新失败，则不改变 runtime；如果 runtime 已局部改变，必须 best-effort rollback，并显示 error notification。

## 自动发送流程

pressed shortcut v1 流程：

```text
GlobalHotkeyState receives hotkey event
  -> shortcut id lookup
  -> ensure shortcut enabled
  -> validate prompt/provider/model availability from DB/cache
  -> resolve input by ShortcutInputSource
  -> build ContentPart list and title_seed
  -> open/activate Temporary Window
  -> create no-project scratch conversation
  -> start AgentRuntime run
  -> route Temporary Window to created conversation
```

Selection/Clipboard：

- 先用 `get-selected-text` 读取当前前台 app 选中文字。
- 选中文字为空时 fallback `cx.read_from_clipboard()`。
- 两者都为空时显示 `notify-shortcut-trigger-empty-input-*`，不创建 conversation。
- 生成 `ContentPart::Text { text }`，`title_seed = text`。

Screenshot：

- 打开截图 overlay。
- 用户取消截图时静默返回。
- capture 失败显示 screenshot capture notification。
- 如果 selected model 支持 image input：
  - 将截图编码为 PNG，写到 app data 下的 attachment 文件目录。
  - 插入 `attachments` row：
    - `kind = AttachmentKind::Image`
    - `storage_kind = AttachmentStorageKind::LocalFile`
    - `mime_type = "image/png"`
    - `name = "screenshot.png"`
    - `path = <written png path>`
    - `metadata.width/height` 来自 captured image
  - user item content 使用 `ContentPart::Image { attachment_id }`，必要时附带一段 text label。
- 如果 model 不支持 image input：
  - 使用 `platform_ext::ocr::recognize_text`。
  - OCR 结果为空时显示 empty input notification。
  - OCR 失败显示 OCR failure notification。
  - OCR 成功后写 `ContentPart::Text { text }`。

Busy / concurrency：

- 如果 Temporary Window 当前 conversation 有 active run，忽略 shortcut 并显示 busy notification。
- 如果截图 overlay 已经 active，忽略新的 screenshot shortcut。
- shortcut 触发不复用当前已有临时 conversation；每次成功触发都创建新的 no-project scratch conversation。

## Prompt/provider/model snapshot

shortcut 保存时需要构建 `settings_snapshot`，执行时优先使用该 snapshot 作为历史事实，同时用当前 DB/cache 检查可执行性。

执行时：

- 当前 prompt id 仍存在：使用最新 prompt record 构建新的 run snapshot。
- prompt id 不存在但旧 snapshot 有 prompt：允许状态显示 Prompt Unavailable；是否继续执行由 v1 策略决定为阻止执行，避免用户以为在用当前 prompt。
- provider/model 不存在或 disabled：阻止执行，显示 Model Unavailable。
- capability mismatch：阻止执行，显示 Capability Mismatch。

`state::conversations::CreateConversationRequest` 后续需要支持：

- `prompt_id: Option<PromptId>`
- `prompt_snapshot: Option<PromptContent>`
- `trigger_kind: AgentRunTriggerKind::Shortcut`

这样 shortcut 创建的 conversation/run 能把 prompt 和 trigger 作为 fresh DB 真相写入，而不是只在 UI 层拼接。

## 组件和 icon

组件清单：

| 组件 | 用途 |
| --- | --- |
| `Input` / `InputState` | 搜索 |
| `HotkeyInput` | 录制全局快捷键 |
| `Select` / `SelectState` | prompt、model、input source 选择 |
| `Switch` | enabled 切换 |
| `Button` | Add、View、Edit、Reregister、Delete、Save、Cancel |
| `DialogFooter` / `DialogAction` / `DialogClose` | 查看、编辑和确认弹窗 |
| `Notification` | load/save/delete/register/trigger success/error |
| `Tag` | status 和 capability summary |
| `Kbd` 或 `format_hotkey_label` | hotkey 展示 |
| `ScrollableElement` | 查看 dialog 或长列表内容滚动 |

图标只使用 app-local Lucide 或现有 provider visual：

- `IconName::Keyboard`：shortcut row / hotkey field。
- `IconName::Plus`：新增 shortcut。
- `IconName::Search`：搜索。
- `IconName::FilePen`：查看/配置语义。
- `IconName::Pencil`：编辑。
- `IconName::RefreshCcw`：重新注册。
- `IconName::Trash`：删除。
- `IconName::CircleCheck`：可用状态。
- `IconName::CircleAlert`：需处理状态。
- `IconName::Server` 或 `provider_visual_icon(...)`：provider/model。

不新增 runtime SVG asset，不在功能模块中散落 raw SVG path。

## i18n 计划

后续代码实现时同步写入：

- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`

建议 key：

- `settings-page-shortcuts`
- `shortcut-search-placeholder`
- `shortcut-empty`
- `shortcut-search-empty`
- `button-add-shortcut`
- `dialog-add-shortcut-title`
- `dialog-edit-shortcut-title`
- `dialog-view-shortcut-title`
- `dialog-delete-shortcut-title`
- `dialog-delete-shortcut-message`
- `shortcut-field-hotkey`
- `shortcut-field-prompt`
- `shortcut-field-model`
- `shortcut-field-input-source`
- `shortcut-field-action`
- `shortcut-field-enabled`
- `shortcut-action-temporary-conversation`
- `shortcut-input-selection-or-clipboard`
- `shortcut-input-screenshot`
- `shortcut-status-enabled`
- `shortcut-status-disabled`
- `shortcut-status-hotkey-invalid`
- `shortcut-status-hotkey-conflict`
- `shortcut-status-prompt-unavailable`
- `shortcut-status-model-unavailable`
- `shortcut-status-capability-mismatch`
- `shortcut-status-registration-failed`
- `shortcut-registration-registered`
- `shortcut-registration-not-registered`
- `shortcut-action-reregister`
- `notify-load-shortcuts-failed`
- `notify-save-shortcut-failed`
- `notify-shortcut-created`
- `notify-shortcut-updated`
- `notify-delete-shortcut-failed`
- `notify-shortcut-deleted`
- `notify-shortcut-register-failed`
- `notify-shortcut-reregistered`
- `notify-shortcut-trigger-busy-title`
- `notify-shortcut-trigger-busy-message`
- `notify-shortcut-trigger-empty-input-title`
- `notify-shortcut-trigger-empty-input-message`
- `notify-shortcut-trigger-model-unavailable-title`
- `notify-shortcut-trigger-screenshot-title`
- `notify-shortcut-trigger-ocr-title`
- `shortcut-validation-hotkey-required`
- `shortcut-validation-hotkey-invalid`
- `shortcut-validation-temporary-conflict`
- `shortcut-validation-binding-conflict`
- `shortcut-validation-model-required`

Settings search keywords 需要覆盖英文、中文和拼音语义：

```text
shortcuts shortcut hotkey global prompt provider model selection clipboard screenshot ocr
快捷键 全局快捷键 热键 提示词 模型 提供商 选中文字 剪贴板 截图
```

## 依赖计划

后续代码实现时需要在 `app/ai-chat2/Cargo.toml` 增加或确认：

- 已有：
  - `global-hotkey = { version = "0.8.0", features = ["serde", "tracing"] }`
  - `platform-ext.workspace = true`
- 本轮已新增：
  - `get-selected-text = "0.1.6"`
  - `image = { version = "0.25.10", default-features = false, features = ["png"] }`
  - `xcap = "0.9.4"`，仅放在 `target.'cfg(any(target_os = "windows", target_os = "macos"))'.dependencies`

新增依赖必须使用完整版本号。Linux screenshot capture 如果没有现成 backend，本阶段按 unsupported-state
处理，不能在 workflow 或代码里散落临时平台 hack。

## 验证计划

已运行：

- `cargo fmt`
- `cargo check -p ai-chat2`
- `cargo test -p ai-chat-db typed_json_roundtrips_for_repository_records`
- `cargo test -p ai-chat2 shortcuts`
- `cargo test -p ai-chat2 hotkey`
- `cargo test -p ai-chat2`
- `cargo clippy -p ai-chat2 -p ai-chat-core -p ai-chat-db -p ai-chat-agent --all-targets --all-features -- -D warnings`

合入前建议继续补跑：

- `git diff --check`

手动验收：

- Settings sidebar 可搜索 `Shortcuts`、`快捷键`、`hotkey`。
- 空列表显示 empty state。
- 新增 selection/clipboard shortcut 后列表刷新并能查看。
- 编辑 hotkey 后 runtime 注册随 DB 更新。
- 与 temporary hotkey 冲突时不能保存。
- 关闭 enabled 后 hotkey 不再注册。
- 注册失败时 row 显示 RegistrationFailed，并可点击 Reregister。
- 触发 selection/clipboard shortcut 后，Temporary Window 打开新临时对话并启动 agent run。
- 触发 screenshot shortcut 后，支持 image input 的 model 写入 attachment image；不支持 image input 的 model 走 OCR text。
- Temporary Window 有 active run 时触发 shortcut 显示 busy notification，不启动第二个 run。

## 父文档同步

- `issue-159-ai-chat2-ui.md` 需要链接本文档，并把 Shortcut settings 状态从“未开始/已有专项计划”改为“第一版已完成”。
- `issue-137-llm-abstractions.md` 只保留简短引用：Shortcut Settings 第一版已落地，后续仍缺 Composer prompt selector、save/promote temporary flow 等 #159 剩余项。
