# Issue #159 ai-chat2 Prompt 设置专项计划

本文档是 `app/ai-chat2` Prompt 设置页的可执行开发计划。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档固定提示词数据模型、Settings
页面结构、数据库 API、GPUI 组件选型、状态流和验证要求。

创建时间：2026-06-12。

当前状态：计划已固定，第一版代码已实现。实现已把 prompt 内容模型从复杂 JSON message shape 收敛为
简单文本，并接入 Settings Prompt CRUD；Prompt Settings 已从选择器式 `ListState` 改为管理页行列表 +
modal。最新 UI 修正已让 Add Prompt 按钮与搜索框同高，并让正文编辑区明确呈现为多行文本。Composer
prompt selector、Shortcut prompt binding、重排序和 enabled 开关仍按本文档边界留后续。

## 目标边界

- 在 Settings 中新增 Prompts 页面，用户可以查看全部提示词、新增提示词、修改提示词、查看提示词和删除提示词。
- v1 只支持简单文本提示词，不支持多 role prompt、多段 content parts、多模态 prompt、变量模板或示例式模板。
- prompts 替代旧 templates。新 UI 继续使用“提示词”文案，不恢复 template/mode 概念。
- 删除提示词使用硬删除。`conversations.prompt_id` 和 `shortcuts.prompt_id` 继续依赖
  `ON DELETE SET NULL`；历史 conversation/run 中的 prompt snapshot 必须保留文本副本。
- 本阶段不实现 Composer prompt selector、Shortcut prompt binding、legacy template 迁移、prompt 重排序、
  enabled 开关或 prompt capability gating。

## 数据模型和数据库计划

当前 fresh schema 中 `prompts.content_json JSON NOT NULL` 搭配
`PromptContent { messages: Vec<PromptMessage> }`，这对 v1 产品目标过度复杂。因为 fresh DB 仍是
pre-main baseline，后续实现按 baseline cleanup 处理，不为旧本地开发期 fresh DB 追加兼容 migration。

目标模型：

```rust
pub struct PromptContent {
    pub text: String,
}
```

目标 schema：

```sql
CREATE TABLE prompts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at DateTime NOT NULL,
    updated_at DateTime NOT NULL
);
```

需要同步修改：

- `crates/ai-chat-core/src/payloads.rs`
  - `PromptContent` 改为简单 `text: String`。
  - 删除 `PromptMessage`，以及 prompt 专用的 role/content-parts 结构。
  - `RunSettingsSnapshot.prompt`、`ConversationSettingsSnapshot.prompt` 和
    `AgentRunInput.prompt_snapshot` 继续使用 `Option<PromptContent>`，保存文本 snapshot。
- `crates/ai-chat-agent/src/runtime.rs`
  - `prompt_preamble` 改为直接读取 `PromptContent.text.trim()`。
  - 空文本返回 `None`，非空文本作为 Rig `AgentBuilder::preamble`。
- `crates/ai-chat-db/src/migrations.rs` / `schema.rs` / `models.rs`
  - `prompts.content_json` 改为 `prompts.content -> Text`。
  - `SqlPromptRow` / `SqlNewPromptRow` 使用 `String` 字段。
  - `TryFrom<SqlPromptRow>` 直接构造 `PromptContent { text: row.content }`。
- `crates/ai-chat-db/src/records.rs`
  - 保留 `PromptRecord { content: PromptContent }` 和 `NewPrompt { content: PromptContent }`。
  - 新增 `UpdatePrompt { name: String, content: PromptContent, enabled: bool, sort_order: i32 }`。
- `crates/ai-chat-db/src/repository.rs`
  - `insert_prompt` 写入 `content.text`。
  - 新增 `list_prompts()`，排序为 `sort_order ASC, name ASC, created_at ASC`。
  - 新增 `update_prompt(id, UpdatePrompt)`，更新 `name`、`content`、`enabled`、`sort_order` 和 `updated_at`。
  - 新增 `delete_prompt(id)`，返回删除行数。

UI/state 层默认新增 prompt 时使用 `enabled = true`，`sort_order = max(sort_order) + 10`。v1 不展示 enabled
开关，也不提供拖拽排序。

## 模块结构

禁止新增 `mod.rs`。新增模块使用同名入口文件和子目录文件。

```text
app/ai-chat2/src/features/settings.rs
  - 增加 `mod prompts;`
  - `SettingsView` 增加 `prompts_settings: Entity<PromptsSettingsPage>`
  - `SettingsPageKey` 增加 `Prompts`
  - `settings_page_specs` 增加 `settings-page-prompts` 和搜索词
  - render match 增加 `SettingsPageKey::Prompts`
  - Prompts page 使用普通 `SettingsPageFrame` 外层滚动；只有 Provider page 继续使用 `no_outer_body_scroll()`

app/ai-chat2/src/features/settings/prompts.rs
  - `PromptsSettingsPage` 顶层 entity
  - 页面管理行列表、search input、create/edit/view/delete orchestration
  - 通过 `PromptCatalogStore` 的 `StoreSelection<Vec<PromptRecord>>` 订阅 prompt rows snapshot
  - 只持有 `search_input: Entity<InputState>` 和派生 prompt records selection，不直接散落 SQL 或数据库连接

app/ai-chat2/src/features/settings/prompts/rows.rs
  - `PromptManagementRow`
  - `PromptManagementEntry`
  - `prompt_management_entries(prompts: &[PromptRecord])`
  - `filter_prompt_entries(entries, query)`
  - 使用 app-local 管理行渲染，不使用 `List` / `ListState` / `ListDelegate`
  - row 展示名称、正文首行预览、更新时间和显式 View/Edit/Delete 操作

app/ai-chat2/src/features/settings/prompts/dialog.rs
  - `PromptEditDialogState`
  - `PromptEditMode::{Create, Edit}`
  - `open_prompt_edit_dialog`
  - `open_prompt_preview_dialog`
  - 名称用单行 `InputState`，正文用 `InputState::multi_line(true).rows(...)` 并在 dialog 中固定多行显示高度

app/ai-chat2/src/state.rs
  - 增加 `pub(crate) mod prompts;`

app/ai-chat2/src/state/prompts.rs
  - `PromptCatalogStore = gpui_store::SharedStore<PromptCatalogState, PromptCatalogSource>`
  - `PromptCatalogSource` 从 fresh DB `prompts` 表加载 projection snapshot
  - `PromptCatalogState { prompts: Vec<PromptRecord> }`
  - `list_prompts(cx)`、`create_prompt(cx, name, text)`、`update_prompt(cx, id, name, text)`、
    `delete_prompt(cx, id)`
  - Settings、后续 Composer prompt selector 和 Shortcut settings 都必须通过该 state 层写入
```

## UI 和组件选型

Settings Prompts 页面使用管理页行列表 + modal 结构，不使用左右分栏详情，也不使用选择器式 `ListState`：

- 顶部 toolbar：左侧搜索输入，右侧 `Add Prompt` 按钮；按钮高度与搜索框保持一致。
- 主体：全宽 prompt 管理行列表，展示全部提示词。
- 空列表或加载失败：在页面主体显示 empty/error state。
- 搜索无结果：显示独立 no-results empty state。
- 行点击：打开只读查看 modal。
- 行右侧显式操作：`View`、`Edit`、`Delete`。
- 查看 modal footer：`Edit`、`Delete`、`Close`。
- 新增/修改：打开编辑 modal，显式保存或取消。
- 删除：复用现有 `components::delete_confirm::open_destructive_confirm_dialog`，硬删除后刷新列表。

组件清单：

| 组件 | 用途 |
| --- | --- |
| `Input` / `InputState` | 列表搜索、prompt 名称输入和正文多行输入 |
| `Button` | Add、View、Edit、Delete、Save、Cancel、Close |
| `DialogFooter` / `DialogAction` / `DialogClose` | 编辑、查看和确认弹窗 footer |
| `Notification` | load/save/delete success/error |
| `ScrollableElement` | 查看 modal 中的正文预览滚动 |
| `Label` | 标题、字段名、empty state 和预览文本 |

图标只使用现有 app-local Lucide：

- `IconName::FilePen`：prompt list row 和查看 modal 语义图标。
- `IconName::Plus`：新增提示词。
- `IconName::Pencil`：编辑提示词。
- `IconName::Trash`：删除提示词。
- `IconName::Search`：列表搜索。

不新增依赖库，不新增 runtime asset，不把 raw SVG path 放进功能模块。

## 数据流

```text
SettingsView
  -> PromptsSettingsPage::new
  -> state::prompts::list_prompts(cx)
  -> local PromptManagementRow list
  -> search_input filters local rows
  -> row click / View action
  -> preview dialog
  -> edit/delete action
  -> state::prompts create/update/delete
  -> FreshRepository insert/update/delete
  -> PromptCatalogStore syncs committed DB rows
  -> PromptsSettingsPage observes StoreSelection
```

保存规则：

- `name` 和 `content.text` 保存前都 `trim()`。
- 空 name 显示 `prompt-validation-name-required`，不写 DB。
- 空 content 显示 `prompt-validation-content-required`，不写 DB。
- 重复 name 由 DB `UNIQUE` 保底；UI 显示 `notify-save-prompt-failed`。
- 保存成功显示 `notify-prompt-saved`，关闭编辑弹窗并刷新列表。
- 删除成功显示 `notify-prompt-deleted`；页面通过 catalog event 重新读取列表。

Snapshot 规则：

- `ConversationSettingsSnapshot.prompt`、`RunSettingsSnapshot.prompt` 和
  `AgentRunInput.prompt_snapshot` 保存 `PromptContent { text }`。
- 删除或编辑 prompt record 不能改变历史 snapshot。
- 后续 Composer prompt selector 选择 prompt 后，应把当时的 `PromptContent` 写入 conversation/run snapshot，
  不在运行时回查可变 prompt 行。

## i18n

新增 key 同步写入 `app/ai-chat2/locales/en-US/main.ftl` 和
`app/ai-chat2/locales/zh-CN/main.ftl`：

- `settings-page-prompts`
- `prompt-search-placeholder`
- `prompt-empty`
- `prompt-search-empty`
- `button-view`
- `button-add-prompt`
- `prompt-dialog-create-title`
- `prompt-dialog-edit-title`
- `prompt-dialog-view-title`
- `prompt-field-name`
- `prompt-field-content`
- `prompt-placeholder-name`
- `prompt-placeholder-content`
- `prompt-delete-title`
- `prompt-delete-message`
- `notify-load-prompts-failed`
- `notify-save-prompt-failed`
- `notify-prompt-saved`
- `notify-delete-prompt-failed`
- `notify-prompt-deleted`
- `prompt-validation-name-required`
- `prompt-validation-content-required`

Settings search keywords 需要覆盖英文、中文和拼音语义：
`prompts prompt system developer instruction text 提示词 系统 开发者 指令 文本`。

## 验证计划

文档落地阶段已运行：

- `git diff --check`

第一版代码实现后已运行：

- `cargo fmt`
- `cargo test -p ai-chat-core prompt`
- `cargo test -p ai-chat-db prompt`
- `cargo test -p ai-chat-agent prompt`
- `cargo test -p ai-chat2 prompts`
- `cargo test -p ai-chat2 settings`
- `cargo check -p ai-chat2`
- `cargo clippy -p ai-chat2 -p ai-chat-core -p ai-chat-db -p ai-chat-agent --all-targets --all-features -- -D warnings`
- `git diff --check`

管理页重设计和最终 UI 尺寸修正后已再次运行：

- `cargo fmt`
- `cargo test -p ai-chat2 prompts`
- `cargo test -p ai-chat2 settings`
- `cargo check -p ai-chat2`
- `cargo clippy -p ai-chat2 --all-targets --all-features -- -D warnings`
- `git diff --check`

验证备注：

- Cargo 当前仍提示上游 `block v0.1.6` future-incompat warning，但上述命令退出码均为 0。
- 手动 GPUI UI 验证仍未运行。

手动验收：

- Settings sidebar 可搜索 `Prompts`、`提示词` 和拼音。
- 空列表显示 `prompt-empty`。
- 新增 prompt 后列表刷新并可查看。
- 编辑 prompt 后列表和查看 modal 显示新内容。
- 删除 prompt 后列表刷新，相关 conversation/shortcut 的 FK 语义不破坏历史 snapshot。
- 空名称、空正文、重复名称都有用户可见错误。
