# Issue #159 ai-chat2 ComposerEditor 自研输入进度板

本文档是 `app/ai-chat2` 真实输入框 `ComposerEditor` 的专项进度板。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档只跟踪 composer 输入内核、token
输入、completion 和 agent loop payload 组装，不跟踪 ChatForm 外框、toolbar 或 picker 视觉预览。

最后同步时间：2026-06-23。

当前状态：第一版输入内核和后续可用性修正已实现。`app/ai-chat2` 已接入 app-local
`ComposerEditor`，替换 ChatForm 的非 input 占位，并完成文本输入、IME range 映射、选择/光标、
cursor blink/styling、编辑快捷键、plain text 剪贴板、Enter 发送、Shift+Enter 换行、soft wrap、
内部滚动、Unicode/grapheme-aware movement/delete/word boundary、`$skill-name` token 解析和
`ComposerSnapshot` 输出。2026-06-23 已补 ChatForm skill completion：输入 `$` 或 `$prefix` 时打开
skill 候选列表，确认后插入 `$skill_name`；`$skill_name` 以 inline token 视觉展示，并且
Backspace/Delete/输入/粘贴/IME finalize 会按完整 token 范围删除或替换；单击 token 会打开
app-local 只读 Skill 详情 dialog。

当前
`gpui-component::Input` / `InputState` 只作为参考材料，不作为最终 composer 输入框。

## 当前决策

- `ChatForm` 继续负责 Codex 风格外框、toolbar、picker 和按钮；输入区已由 app-local `ComposerEditor` 承接。
- 真实输入能力由 `ComposerEditor` 承接，已支持普通文本、Unicode/grapheme-aware 编辑、`$skill-name` token metadata、completion UI 和 snapshot payload；agent loop 接线继续消费 `ComposerSnapshot`。
- 第一版实现优先匹配 Zed agent input 的基础编辑体验：IME、选择、全选、双击选词、光标快捷键、选择快捷键、剪贴板和 undo/redo。
- 不为了短期接线把 `gpui-component::Input` 作为最终输入框；如果后续组件库提供合适的 inline token/decoration API，再单独评估是否收敛。
- `$` skill completion 归属 `ComposerEditor`，不是 ChatForm toolbar picker；ChatForm 只负责把当前项目上下文和
  skill catalog 数据传给 composer。
- Skill token 仍是普通文本 buffer 里的 `$skill_name`，发送 payload 从 token metadata 生成
  `SkillActivationRequest`；inline 特殊展示不能成为唯一数据源。
- token 视觉按 Zed `Crease::Inline` 的产品形态做成真正的 inline element chip，包含 `Sparkles` 图标和 skill
  名称；completion row 不重复展示统一图标。

## ChatForm Skill Completion 计划

实现状态（2026-06-23）：已实现。实际代码路径为
`app/ai-chat2/src/components/chat_form.rs`、`composer_editor.rs`、`composer_editor/completion.rs`、
`composer_editor/token.rs`、`composer_editor/element.rs`、`composer_editor/skill_detail.rs` 和
`state/skills.rs`。确认候选后会在
`$skill_name` 后自动补一个空格；如果 trigger 后面已经是空白，则不重复插入空格。

### 产品行为

- 触发条件：
  - 光标位于一个 word boundary 后并输入 `$` 时打开 skill completion。
  - 光标位于 `$prefix` 内继续输入、删除或移动时，completion query 跟随 prefix 更新。
  - `$` 前一个字符如果是 word char，则不触发，避免 `foo$bar` 被误识别。
  - IME marked range 存在时不打开 completion，等 composition 结束后再重新计算 trigger。
- 关闭条件：
  - Esc、点击 composer 外部、提交消息、光标离开当前 `$prefix`、prefix 被删除为空且用户继续输入非 skill-name 字符。
  - catalog 为空时可打开空状态，不发错误通知。
- 选择行为：
  - Enter/Tab/click 确认当前选项，用 `$skill_name` 替换 trigger range。
  - Up/Down 在 completion list 中移动选中项；completion 打开时这些按键优先给 list，未打开时仍走 composer 光标移动。
  - 选择后光标位于 token 后方，并保持 ChatForm focus 在 composer。
  - 选择后自动补一个空格；如果 token 后面已经是空白，不重复补空格。
- token 视觉：
  - 已识别 `$skill_name` 以 inline element chip 绘制：淡 accent background、边框、圆角、`Sparkles` 图标和 skill 名称。
  - token 文本仍显示真实 `$skill_name`，复制/剪切仍输出 plain text，不引入隐藏 link 文本。
  - 单击 token 打开应用内只读 Skill 详情 dialog；不调用系统默认应用打开 `SKILL.md`。
- token 删除和替换：
  - Backspace 在 token 右边界时删除整个 token。
  - Delete 在 token 左边界时删除整个 token。
  - 光标或 selection 落在 token 内部时，删除、粘贴、普通输入、IME finalize 都先把 range 扩展到完整 token。
  - 鼠标单击 token 内部时把光标吸附到最近边界；双击仍选中整个 token。
  - Undo/redo 必须恢复 text、selection、tokens 和 completion open/closed 派生状态。

### 模块结构

禁止新增 `mod.rs`。新增代码放在现有 `composer_editor/` 目录下：

```text
app/ai-chat2/src/components/chat_form.rs
  - 保留 ChatForm 外框和项目上下文所有权。
  - `refresh_skill_catalog(project_root)` 不再让 ComposerEditor 直接扫描文件系统；
    改为通过 `state::skills` 获取 entries 后调用 composer setter。
  - 增加 `ChatFormSkillCompletionPlacement::{AboveForm,BelowForm}` 和 form bounds 记录。
  - 在 ChatForm 外层渲染 skill completion popup，宽度对齐 ChatForm，使用
    `deferred(anchored().snap_to_window_with_margin(...))` 防止超出窗口。
  - conversation detail 设置为 `AboveForm`，new conversation / temporary new conversation 设置为 `BelowForm`。
  - 增加 `skill_catalog_scope: state::skills::SkillCatalogScope`。
  - 增加 `skill_catalog_task: Task<()>`，用于 project-aware catalog 后台加载。
  - 订阅 `GlobalSkillCatalogStore`，当 scope 是 Global / no-project 时同步 composer skill entries。

app/ai-chat2/src/components/chat_form/composer_editor.rs
  - 增加 `mod completion;`
  - `ComposerEditor` 持有 completion 状态和 completion list entity。
  - 增加 `set_skill_entries(entries: &[state::skills::GlobalSkillEntry], cx)`。
  - 删除或降级当前直接调用 `SkillCatalog::scan(project_root)` 的 `refresh_skill_catalog`。
  - 在文本变更、selection 变更、focus/blur 和 action 处理后调用 `sync_skill_completion(...)`。
  - 暴露 `skill_completion_open()` 和 `render_skill_completion_panel(max_height, ...)` 给 ChatForm。
  - 不再在 editor root 内渲染 absolute overlay，避免被输入框层级遮挡。
  - 单击 token 时通过 `LayoutCache::token_hit_for_position(...)` 命中 token，并打开 Skill 详情 dialog。

app/ai-chat2/src/components/chat_form/composer_editor/completion.rs
  - `$` trigger 检测、query 过滤、completion list delegate、row 渲染和 confirm/cancel 辅助。

app/ai-chat2/src/components/chat_form/composer_editor/token.rs
  - 扩展 token range helper，负责 atomic token 删除/替换边界。
  - 保持 `ComposerToken` payload 结构兼容现有 `ComposerSnapshot`。

app/ai-chat2/src/components/chat_form/composer_editor/element.rs
  - 使用 `LineFragment::element(width, len_utf8)` 将 token chip 纳入软换行和 hit-test 计算，等价复刻
    Zed `Crease::Inline` 的 inline element 行为。
  - 继续维护 IME candidate bounds 和文本布局缓存；skill completion popup 不再依赖 caret 定位。

app/ai-chat2/src/components/chat_form/composer_editor/skill_detail.rs
  - 负责 token 点击后的只读 Skill 详情 dialog。
  - 复用 `state::skills::load_skill_content(...)` 后台读取 `SKILL.md` 内容。
  - 展示 name、description、source tag、具体 `SKILL.md` 路径、content sha256 和 markdown 内容。
  - 提供复制路径按钮，不实现打开/关闭 skill 逻辑。

app/ai-chat2/src/state/skills.rs
  - 增加 project-aware catalog helper，但不新增数据库/cache 表。
```

### 类型设计

`state::skills`：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SkillCatalogScope {
    Global,
    Project { root: PathBuf },
}

pub(crate) fn load_catalog_entries(
    scope: SkillCatalogScope,
) -> ai_chat_agent::Result<Vec<GlobalSkillEntry>>;
```

- `Global` 复用 `SkillCatalog::scan(None)` 语义。
- `Project { root }` 复用 `SkillCatalog::scan(Some(root.as_path()))` 语义，保持当前 runtime 行为：user skills +
  project `.agents/skills` 合并。
- `GlobalSkillCatalogStore` 继续只管理全局 catalog，Settings 和 no-project ChatForm 共享它。
- project-aware catalog 暂不做全局缓存，先由持有项目上下文的 ChatForm 后台加载；如果后续 profiling 发现频繁重复扫描，
  再加 app-level project catalog cache。

`completion.rs`：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillCompletionTrigger {
    pub(super) range: Range<usize>, // includes '$' and current prefix
    pub(super) query: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillCompletionRow {
    pub(super) skill: ComposerSkill,
    name: SharedString,
    description: SharedString,
    source_label: SharedString,
    search_text: String,
}

pub(super) struct SkillCompletionDelegate {
    ix: Option<IndexPath>,
    all_rows: Vec<Rc<SkillCompletionRow>>,
    rows: Vec<Rc<SkillCompletionRow>>,
    empty_label: SharedString,
    on_confirm: Rc<dyn Fn(SkillCompletionRow, &mut Window, &mut App)>,
    on_cancel: Rc<dyn Fn(&mut Window, &mut App)>,
}
```

`ComposerEditor` 新增字段：

```rust
skills: BTreeMap<String, ComposerSkill>,
completion_list: Entity<ListState<SkillCompletionDelegate>>,
completion_trigger: Option<SkillCompletionTrigger>,
completion_needs_selection_sync: bool,
```

`element.rs`：

```rust
pub(super) struct PrepaintState {
    lines: Vec<PaintLine>,
    cursor: Option<Bounds<Pixels>>,
    selections: Vec<PaintQuad>,
}
```

实现中 token chip 是参与 wrapping 的 inline element fragment，不再用普通文本 background 模拟 token。

`token.rs` helper：

```rust
pub(super) fn token_at_offset(tokens: &[ComposerToken], offset: usize) -> Option<&ComposerToken>;
pub(super) fn token_before_offset(tokens: &[ComposerToken], offset: usize) -> Option<&ComposerToken>;
pub(super) fn token_after_offset(tokens: &[ComposerToken], offset: usize) -> Option<&ComposerToken>;
pub(super) fn expand_range_to_token_boundaries(
    tokens: &[ComposerToken],
    range: Range<usize>,
) -> Range<usize>;
```

### 数据流

```text
app init
  -> state::skills::init(cx)
  -> GlobalSkillCatalogStore loads global SkillCatalog::scan(None)

ChatForm::new
  -> creates ComposerEditor
  -> subscribes GlobalSkillCatalogStore
  -> pushes global entries into composer when scope == Global

New Conversation / Conversation Detail / Temporary
  -> calls ChatForm::refresh_skill_catalog(project_root)
  -> ChatForm stores SkillCatalogScope
  -> Global: use GlobalSkillCatalogStore snapshot
  -> Project: background load state::skills::load_catalog_entries(Project { root })
  -> ComposerEditor::set_skill_entries(entries)
  -> ComposerEditor refreshes token metadata and completion rows

User types "$"
  -> EntityInputHandler::replace_text_in_range mutates text
  -> refresh_tokens()
  -> sync_skill_completion()
  -> SkillCompletionDelegate filters rows by prefix
  -> ChatForm renders a deferred anchored popup aligned to the form width

User confirms row
  -> confirm_skill_completion(row)
  -> replace trigger range with "$skill_name"
  -> refresh_tokens()
  -> close completion
  -> ComposerSnapshot keeps content text + SkillActivationRequest
```

### 组件和渲染

- completion overlay 不使用 `gpui-component::popover::Popover`：
  - `Popover` 适合有固定 trigger element 的 toolbar/menu；这里的触发来自 editor 文本状态，且弹层需要对齐整个
    ChatForm 而不是对齐 `$` caret。
  - 使用 app-local popup：ChatForm 通过 `on_prepaint` 记录自身 bounds，再用
    `deferred(anchored().snap_to_window_with_margin(px(8.)))` 按页面语义定位。
  - conversation detail 使用 `AboveForm`，popup 在 ChatForm 上方；new conversation 和 temporary new
    conversation 使用 `BelowForm`，popup 在 ChatForm 下方。
  - popup 宽度使用 ChatForm bounds width；最大高度按窗口剩余空间和 `px(360.)` 取较小值，空间不足时不渲染。
- completion list 使用 `gpui_component::list::{List, ListState, ListDelegate}`：
  - 行高固定，适合 component list。
  - 不启用 List 自带 search input；query 来自 `$prefix`。
  - 最大高度由 ChatForm 按窗口剩余空间传入，超过后 list 自己滚动。
- completion row 使用 app-local `SkillCompletionItem`：
  - 单行展示：skill name + description + source `Tag`。
  - 不展示完整 `SKILL.md` 路径；路径只保留在 `search_text` 中用于匹配。
- token 视觉在 `ComposerEditorElement` 里绘制，不用 `Tag` 组件：
  - `Tag` 是独立元素，不能直接参与 composer 的 inline 文本布局。
  - token chip 通过 `LineFragment::element(width, len_utf8)` 参与软换行，绘制时作为 `AnyElement` prepaint/paint。
  - 命中测试同样基于 token fragment width，单击 chip 不再把 cursor 放进 token 内部，而是打开详情 dialog。

### 图标

- completion row 不放统一 icon，避免每行重复 `Sparkles` 造成噪音。
- inline `$skill_name` token 使用 `IconName::Sparkles`，对齐 Zed mention chip 的识别度。
- Skill 详情 dialog 使用：
  - `IconName::RefreshCcw`：内容加载中。
  - `IconName::CircleAlert`：读取失败。
  - `IconName::Copy`：复制 `SKILL.md` 路径。
- 不新增 Lucide icon variant，不新增 SVG asset。

### i18n

新增 key 写入 `app/ai-chat2/locales/en-US/main.ftl` 和
`app/ai-chat2/locales/zh-CN/main.ftl`：

- `chat-form-skill-completion-empty`
- `skill-detail-dialog-title`
- `button-copy-path`

复用已有 key：

- `button-close`
- `skill-content-loading`
- `skill-description-empty`
- `skill-source-builtin`
- `skill-source-user`
- `skill-source-project`
- `skill-source-plugin`

不新增“按 $ 插入技能”这类常驻说明文案；completion 是即时交互，不在主界面放教程文本。

### 数据库、配置和依赖

- 不新增数据库 migration。
- 不新增 `skills`、`skill_roots`、`skill_completion` 或 composer token 表。
- 不改 `conversation_items` schema。
- 不新增 `config.toml` 字段；completion open/selected/query 是瞬时 UI state。
- 不新增依赖库；继续使用现有 `ai-chat-agent`、`gpui`、`gpui-component`、`gpui-store`、`unicode-segmentation`。

### 验证计划

代码实现后运行：

- `cargo fmt`
- `cargo test -p ai-chat2 composer_editor`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 skill`
- `cargo check -p ai-chat2`
- `cargo clippy -p ai-chat2 --all-targets -- -D warnings`
- `git diff --check`

新增/更新测试重点：

- `$` trigger 只在 word boundary 生效。
- `$prefix` 过滤 skill rows，支持 name/description/path/source 搜索。
- completion confirm 替换 trigger range 并生成 `ComposerToken`。
- Esc/cursor 离开 prefix 会关闭 completion。
- Backspace/Delete 在 token 边界删除整个 `$skill_name`。
- selection/typing/paste/IME finalize 覆盖 token 内部时扩展到完整 token。
- token visual prepaint 产生背景 quad，且不改变 plain text copy/snapshot。
- ChatForm global scope 使用 `GlobalSkillCatalogStore`，project scope 调用 project-aware loader。

## Zed Agent Input 对照

本轮本地参考代码：

- `/Users/sushao/Documents/code/zed/crates/agent_ui/src/message_editor.rs`
- `/Users/sushao/Documents/code/zed/crates/agent_ui/src/mention_set.rs`
- `/Users/sushao/Documents/code/zed/crates/editor/src/input.rs`
- `/Users/sushao/Documents/code/zed/crates/editor/src/selection.rs`
- `/Users/sushao/Documents/code/zed/crates/editor/src/element.rs`
- `/Users/sushao/Documents/code/zed/assets/keymaps/default-macos.json`
- `/Users/sushao/Documents/code/zed/assets/keymaps/default-linux.json`
- `/Users/sushao/Documents/code/zed/assets/keymaps/default-windows.json`

Zed 的 `MessageEditor` 不是普通 input。它创建 `editor::Editor`，配置 placeholder、soft wrap、
completion provider、context menu、focus 事件、agent send/cancel 事件，并使用 editor 内核提供 IME、
selection、undo/redo、keyboard action 和 mouse selection。Zed 的 skill/context mention 不是语法高亮；
它先插入 `MentionUri::as_link()`，再通过 `Crease::Inline` 把该范围折叠成可渲染 mention chip，发送时从
crease snapshot 还原 content blocks。

`ai-chat2` 不需要完整代码编辑器能力，但需要复刻 agent input 的基础交互质量。

## 目标边界

专项长期目标：

- 单一文本 buffer，带 UTF-8 / UTF-16 range 映射，供 IME 和 GPUI input handler 使用。
- IME composition、marked range、候选框定位和 composition undo grouping。
- 鼠标定位、拖拽选择、双击选词、三击选行、全选、shift 扩展选择。
- 键盘光标移动、word/line/document movement、shift selection、删除到词/行。
- `cmd-z` / `cmd-shift-z`，非 macOS `ctrl-z` / `ctrl-y` / `ctrl-shift-z`。
- copy/cut/paste、paste as plain text、选区序列化。
- multiline composer 的 soft wrap、auto-grow、placeholder、scroll 和 send/newline 策略；第一版已完成 auto-grow、placeholder、send/newline、soft wrap、内部滚动和 cursor into view。
- `$skill-name` token 的 range 追踪、特殊样式、无效 token 清理、删除/复制/payload 还原。
- `$` skill completion 已完成；后续 `@` context picker 和 slash command 可以分阶段加入。

不做或后置：

- 语法高亮、Markdown/code diagnostics、code folding、line number、LSP、go-to-definition。
- 多 cursor、column selection、Vim/modal editing、kill ring、复杂 editor inlay hints。
- Zed 的完整 file/thread/selection/pasted-image mention 类型；第一阶段只实现 `$skill-name`。
- 与 agent loop 的真实发送本轮不在输入内核文档内完成，但 payload shape 必须为后续接线预留。

## 功能矩阵

| 能力 | Zed 对照 | ai-chat2 目标 | 状态 |
| --- | --- | --- | --- |
| 文本 buffer | `Buffer::local` + `MultiBuffer::singleton` | app-local 单 buffer；保存 UTF-8 文本和 selection | 已完成 |
| UTF-16 range 映射 | `EntityInputHandler` | IME API 使用 UTF-16，内部统一转 UTF-8 offset | 已完成 |
| IME marked range | `replace_and_mark_text_in_range` / `marked_text_range` | composition 文本带 marked range 和下划线渲染 | 已完成 |
| IME 候选框定位 | `bounds_for_range` | 基于当前 layout 返回候选框 bounds | 已完成，待手动 IME 验证 |
| composition undo grouping | editor tests 覆盖 composition undo/redo | composition 更新合并为一个 history group；composition 中 undo 取消 marked range | 已完成 |
| 单击定位 | `EditorElement` mouse down | 根据 hit test 设置 cursor，清除 selection | 已完成 |
| 拖拽选择 | `selection.rs` begin/extend selection | 鼠标拖拽扩展 selection，支持越界 autoscroll 可后置 | 已完成；越界 autoscroll 后置 |
| 双击选词 | click count 2 | 按 word boundary 选择 `$skill-name` 或普通词 | 已完成 |
| 三击选行 | click count 3 | 选择当前 visual/text line | 已完成 |
| 全选 | `SelectAll` | macOS `cmd-a`，其他平台 `ctrl-a` | 已完成 |
| shift-click 扩选 | selection extension | 从 anchor 到点击位置扩展 selection | 已完成 |
| 左右移动 | `MoveLeft` / `MoveRight` | caret 左右移动，清除或保持 selection | 已完成，grapheme-aware |
| 上下移动 | `MoveUp` / `MoveDown` | visual line 上下移动，保持 preferred column | 已完成 |
| word movement | macOS `alt-left/right`，其他平台 `ctrl-left/right` | 按词边界移动；token 作为独立词段 | 已完成 |
| line movement | macOS `cmd-left/right`、`ctrl-a/e`，Home/End | 移动到行首/行尾 | 已完成 |
| document movement | macOS `cmd-up/down` | 移动到 buffer 开头/结尾 | 已完成 |
| shift selection movement | Zed keymap shift variants | 对应 movement 的选择版本 | 已完成 |
| Backspace/Delete | editor actions | 删除 selection、前/后字符，token range 被破坏时清理 token metadata | 已完成，grapheme-aware |
| 删除到词/行 | `DeleteToPreviousWordStart` 等 | macOS alt/cmd delete，其他平台 ctrl delete/backspace | 已完成 |
| Enter send | `AcpThread > Editor && !use_modifier_to_send` | 默认 Enter 发送；Shift+Enter 换行 | 已完成 |
| modifier send | `use_modifier_to_send` key context | 后续设置项：macOS cmd-enter 发送，其他平台 ctrl-enter 发送，Enter 换行 | 后置 |
| undo/redo | `editor::Undo` / `Redo` | 文本、selection、token metadata 一起回滚 | 已完成 |
| copy/cut | `MessageEditor::copy/cut` | 序列化 selection；token 复制/剪切固定为显示文本 plain text | 已完成，plain text |
| paste | `MessageEditor::paste` | plain text paste；后续图片/文件 paste 交给附件入口 | 已完成，plain text |
| context menu | Zed custom Cut/Copy/Paste/Paste Raw | 基础编辑菜单；是否接 app menu action 后续实现 | 后置 |
| placeholder | `set_placeholder_text` | 空文本显示 muted placeholder | 已完成 |
| soft wrap | `set_soft_wrap` | 按 composer 宽度换行，不横向滚动 | 已完成 |
| auto-grow | `EditorMode::auto_height` 类场景 | 高度随内容增长，超过上限后内部滚动 | 已完成 2-8 行自动高度；超过上限后内部滚动 |
| scroll | editor scroll state | 支持长输入内部滚动和光标可见 | 已完成 |
| `$skill-name` token parse | Zed skill mention completion + crease | 插入 skill token，保存 range + skill id/path/source snapshot | 已完成 |
| token 特殊样式 | Zed `Crease::Inline` chip | `$skill_name` 应有明确 inline token 视觉，而不是仅依赖普通文本颜色 | 已完成，使用 inline `LineFragment::element` chip |
| token 点击详情 | Zed mention click 打开内部 buffer | 单击 token 应在 app 内查看 `SKILL.md`，不跳系统默认应用 | 已完成，使用只读 Skill 详情 dialog |
| token 删除策略 | Zed crease invalidation | token 边界 Backspace/Delete 删除整个 token；覆盖 token 内部的输入/粘贴/IME range 先扩展到完整 token | 已完成 |
| token payload | `build_chunks_from_creases` | 发送时输出 plain text + skill snapshot blocks，不能只依赖渲染文本 | 已完成，输出 `ComposerSnapshot` |
| `$` skill completion | `PromptCompletionProvider` | 输入 `$` 或 token prefix 时打开 skill picker，确认后插入 `$skill_name`，并和 token 视觉/整体删除共用同一 metadata | 已完成 |
| `@` context completion | Zed context mention | 后续接 project/file/context picker | 后置 |
| slash command | Zed slash command validation/hint | 后续接 MCP prompts 或 app commands；不阻塞第一版 ComposerEditor | 后置 |
| read-only input attempted | Zed `InputAttempted` | agent running/read-only 时可记录输入尝试，用于后续 queue/提示 | 后置 |

## 实现阶段

1. 输入内核骨架：已完成 `ComposerEditor` 模块、文本 buffer、selection、history、focus handle、基础 render。
2. GPUI input handler：已完成 IME 所需 range 映射、marked range、replace/unmark 和 candidate bounds。
3. 光标与选择：已完成 hit test、caret layout、mouse selection、double/triple click、keyboard movement。
4. 编辑动作：已完成插入、删除、剪贴板、undo/redo、composition grouping 和跨平台 key bindings。
5. Composer 布局：已接入 ChatForm 输入区，保留现有外框和 toolbar；auto-grow、soft wrap、内部滚动和 cursor into view 已完成。
6. Token 模型：已完成 `$skill-name` token range、inline token 视觉、invalid cleanup、selection/copy/delete 和整体删除/替换行为。
7. Completion：已接 `$` skill picker，并接入 `state::skills` catalog；后续再接 `@` context picker；slash command 后置。
8. Payload：已定义 `ComposerSnapshot`，包含 plain text、token ranges、skill snapshots、attachments 占位和 send policy。
9. Agent loop 接线：由后续 UI/runtime 任务把 snapshot 转成 `ai-chat-agent` input，不在输入内核第一阶段完成。

## 验收与测试

第一阶段必须有代码级测试覆盖：

- UTF-8 / UTF-16 range roundtrip，包含中文、emoji 和混合文本。
- IME composition replace、finalize、cancel、undo/redo grouping。
- word boundary、double-click selection、triple-click line selection。
- keyboard movement 和 shift selection 的核心路径。
- undo/redo 同步文本、selection 和 token metadata。
- token partial edit invalidation、whole token delete、payload snapshot。

UI 行为验证在可运行后补充：

- macOS 中文输入法 composition 和候选框位置。
- 鼠标拖拽、双击、三击、全选、copy/cut/paste。
- `cmd-z` / `cmd-shift-z` 和非 macOS `ctrl-z` / `ctrl-y` / `ctrl-shift-z`。
- Enter 发送、Shift+Enter 换行、后续 modifier-to-send 设置。

## 当前记录

- 2026-05-29：已完成 ChatForm 视觉预览，输入区仍是非 input 占位。
- 2026-05-29：确认最终 `ComposerEditor` 先走 app-local 自研路线；`gpui-component::InputState`
  只作为参考，不作为最终输入能力承诺。
- 2026-05-29：已完成 `ComposerEditor` 第一版实现并接入 ChatForm；已运行 `cargo fmt`、
  `cargo test -p ai-chat2 composer_editor`、`cargo test -p ai-chat2 chat_form`、
  `cargo test -p ai-chat2`、`cargo check -p ai-chat2`、
  `cargo clippy -p ai-chat2 --all-targets --all-features -- -D warnings` 和 `git diff --check`。
  未做手动 macOS 中文输入法、候选框、双击/拖拽和 bundle GUI 验证。
- 2026-05-31：已同步 5/29 之后的 composer 修正：`34ccb6f` 调整 cursor styling，
  `26a89fa` 补 composer scrolling，`09b2f22` 引入 Unicode/grapheme-aware editing 和
  对中文、emoji、grapheme cluster、soft wrap、scroll cursor visibility 的测试覆盖。
  本次只同步文档状态；未运行 Rust tests。
- 2026-06-23：补充 ChatForm Skill Completion 计划。当时代码已经能解析 `$skill-name` 并输出
  `SkillActivationRequest`，但 `$` completion UI、明确的 inline chip-like token 视觉和 token 原子删除/替换尚未实现。
  计划固定到 `composer_editor/completion.rs`、`composer_editor/token.rs`、`composer_editor/element.rs`、
  `components/chat_form.rs` 和 `state::skills.rs`；本次只更新文档，未运行 Rust tests。
- 2026-06-23：实现 ChatForm Skill Completion。`ChatForm` 通过 `state::skills` 同步 global/project catalog；
  `ComposerEditor` 新增 `$` completion overlay、`$skill_name` inline token 背景和 token 整体删除/替换。
  已验证 `cargo test -p ai-chat2 skill_token_edits_are_atomic`、
  `cargo test -p ai-chat2 token_boundaries_expand_overlapping_edits`、
  `cargo test -p ai-chat2 trigger_matches_dollar_prefix_at_word_boundary`。
- 2026-06-23：调整 Skill Completion popup 定位。`ComposerEditor` 只保留 completion 状态和 list entity；
  popup 改由 `ChatForm` 通过 `deferred(anchored())` 渲染，宽度对齐 ChatForm，conversation detail 向上弹，
  new conversation / temporary new conversation 向下弹，并按窗口剩余空间限制最大高度。
- 2026-06-23：确认 skill completion 后自动在 `$skill_name` 后补空格；当 trigger 后已有空白时不重复补空格。
- 2026-06-23：将 `$skill_name` token 视觉从文本背景模拟改为 inline element chip。`element.rs` 通过
  `LineFragment::element(width, len_utf8)` 让 token chip 参与软换行、cursor hit-test 和 selection paint；
  单击 token 打开 app-local 只读 Skill 详情 dialog，dialog 使用 `state::skills::load_skill_content(...)`
  读取并展示 `SKILL.md` 内容。
