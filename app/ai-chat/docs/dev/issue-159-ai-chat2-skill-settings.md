# Issue #159 ai-chat2 Skill 设置专项计划

本文档是 `app/ai-chat2` 全局 Skill 设置页的可执行开发计划。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档固定 Settings 页面结构、
catalog 数据源、状态流、组件选型、i18n、icon、验证要求和明确不做的范围。
ChatForm 输入框 `$` skill completion、inline token 视觉和整体删除策略由
`app/ai-chat/docs/dev/issue-159-ai-chat2-composer-editor.md` 跟踪；本文档只固定 Settings catalog
和共享 skill 数据源边界。

创建时间：2026-06-23。

当前状态：第一版已实现并通过 focused 验证。

实现记录（2026-06-23）：

- 新增 `app/ai-chat2/src/state/skills.rs`，提供全局 skill catalog store、刷新和 `SKILL.md` 内容读取。
- Settings 新增 `Skills` 分栏，侧边栏使用 `IconName::Sparkles`，页面提供搜索、刷新、列表内展开 raw `SKILL.md` 内容。
- 列表使用 GPUI 原生 `list` + `ListState::measure_all()`，每个 skill 保持一个 list item；展开内容通过
  `gpui_component::collapsible::Collapsible` 放在同一卡片内。
- 每行只展示具体 `SKILL.md` 文件路径，不额外展示包含关系明确的 directory path。
- “查看内容”按钮放在卡片底部左侧；raw content panel 使用 row 专属 `ScrollHandle`。内容面板容器必须
  `occlude()` 外层 list hitbox，避免外层 list 在 bubble 阶段先消费 wheel；内容滚动区只 `track_scroll(...)`，
  不使用内置 `overflow_y_scroll()` 自动消费 wheel，滚动位移由自定义 handler 单一路径写入。到顶/到底后的剩余
  滚动量由内容面板手动转发给外层 `ListState::scroll_by(...)`，实现浏览器式 scroll chaining。
- 不新增数据库表，不实现 enable/disable、安装/卸载、编辑、打开文件/目录。
- 验证：`cargo fmt`、`cargo test -p ai-chat2 skill`、`cargo test -p ai-chat2 settings`、`cargo check -p ai-chat2`、`cargo clippy -p ai-chat2 --all-targets -- -D warnings`、`git diff --check`。
- 2026-06-23 追加并实现 ChatForm skill 接入边界：Settings 仍只负责 catalog 浏览；ChatForm/ComposerEditor 复用
  `state::skills` 数据源，已补 `$` completion、`$skill_name` inline token 视觉和 token 整体删除，详见
  `issue-159-ai-chat2-composer-editor.md`。

## 产品决策

- 在 Settings 中新增 `Skills` 分栏，展示全局 skill catalog。
- 本阶段不实现启用/禁用开关，不写 enable/disable 状态，也不做安装、卸载、编辑、打开目录或打开文件。
- 点击“查看内容”采用列表内展开，不打开 dialog。
  - 原因：这是 catalog 浏览/核对，不是编辑或确认流程；内联展开能保留当前搜索结果和列表上下文，也贴近参考图里的交互。
  - 每次只展开用户点击的行；展开其他行时可以保留多个已展开内容，除非实际 UI 验证发现列表过长影响使用。
- “全局 skill”在 v1 中定义为不依赖当前项目选择的 catalog：复用当前 runtime 已有的
  `SkillCatalog::scan(None)` 语义。
  - 当前代码实际扫描用户级 `~/.agents/skills/<name>/SKILL.md`。
  - 项目级 `<project>/.agents/skills/<name>/SKILL.md` 继续只属于项目上下文 composer，不在本设置页展示。
  - `SkillSourceKind::BuiltIn` / `Plugin` 保留在 UI 类型和 badge 设计中；后续如果 `ai-chat2` 增加 app-bundled
    或 plugin skill roots，只扩展 `state::skills` 数据源，不改 Settings 页面结构。
- 内容预览显示原始 `SKILL.md` 文本，保留 frontmatter、Markdown 标记和相对路径说明，不把它转换成 rendered Markdown。

待确认但不阻塞 v1 文档计划的问题：

- 是否要把 Codex 专属 `~/.codex/skills`、`.codex/plugins/cache/**/skills` 或当前 Codex 会话注入的技能根纳入
  `ai-chat2` 的全局 skill catalog。当前 `ai-chat2` runtime 没有扫描这些路径，本计划不默认纳入。

## 目标边界

Settings `Skills` 页面需要提供：

- 全局 skill 列表。
- 手动刷新。
- 搜索。
- 每一项展示名称、描述、来源和具体 `SKILL.md` 文件路径。
- 点击“查看内容”后在列表行内展开具体 `SKILL.md` 内容。
- 文件读取失败、catalog 扫描失败和空列表状态的用户可见反馈。

本阶段不做：

- Skill 开关、启用策略、禁用列表或持久化偏好。
- Skill 安装、删除、编辑、重命名、排序或复制。
- 项目级 skill 切换、按 project root 查看、project/user 冲突解析 UI。
- Composer `$skill` completion UI。本 Settings 页面不实现；对应实现已转入
  `app/ai-chat/docs/dev/issue-159-ai-chat2-composer-editor.md`。
- Skill activation timeline 的新展示。
- MCP server 列表、MCP tool 状态或 MCP 配置 UI。
- `skills` / `skill_roots` 数据库表。

## 当前实现基线

后端已有 `crates/ai-chat-agent/src/skills.rs`：

- `SkillCatalogEntry`
  - `name: String`
  - `description: Option<String>`
  - `skill_file_path: PathBuf`
  - `directory_path: PathBuf`
  - `source_kind: SkillSourceKind`
- `SkillCatalog::scan(project_root: Option<&Path>)`
  - `None`：扫描用户级 `~/.agents/skills`
  - `Some(project_root)`：在用户级根之外再扫描 `<project_root>/.agents/skills`
- `SkillCatalog::entries()` 返回 catalog entries。
- `SkillCatalog::catalog_hash()` 可用于后续判断列表是否变化。
- `SkillLoader::load(entry)` 读取 `SKILL.md` 全文并生成 `SkillActivationItem`，包含 content hash。

`app/ai-chat2` 当前 `ComposerEditor` 已有 skill token 与 completion 数据流：

- `ComposerEditor::new(...)` 不再直接扫描文件系统。
- `ChatForm::refresh_skill_catalog(project_root)` 通过 `state::skills` 获取 catalog entries，再调用
  `ComposerEditor::set_skill_entries(...)`。
- New Conversation / Conversation Detail / Temporary 已按当前项目或 no-project 调用 refresh；有 project root 时使用
  `SkillCatalog::scan(Some(project_root))`，无项目时使用 `SkillCatalog::scan(None)`。
- token 解析已在 `composer_editor/token.rs` 中输出 `SkillActivationRequest`。
- `$` completion UI、`$skill_name` inline token 视觉和 token 整体删除已由
  `ComposerEditor` 消费 `state::skills` entries 实现。

Fresh database 设计已固定：

- Skill/MCP source 不写入 chat database。
- 已加载进对话上下文的 skill 内容作为 `SkillActivation` transcript snapshot 持久化。
- 不新增 `skills`、`skill_roots`、`mcp_servers`、`mcp_tools` 这类可从文件/配置和运行时连接恢复的 source tables。

## 模块结构

禁止新增 `mod.rs`。新增模块使用同名入口文件和子目录文件。

```text
app/ai-chat2/src/app.rs
  - init 顺序中增加 `state::skills::init(cx)`，放在 `state::prompts::init(cx)?` 附近即可。

app/ai-chat2/src/state.rs
  - 增加 `pub(crate) mod skills;`

app/ai-chat2/src/state/skills.rs
  - app-level global skill catalog store。
  - 负责扫描全局 skill、刷新 catalog、加载单个 skill 内容。
  - 不依赖 Settings UI，不持有页面展开状态。
  - Composer 接入使用 project-aware helper：
    `SkillCatalogScope::{Global, Project { root: PathBuf }}` 和
    `load_catalog_entries(scope) -> ai_chat_agent::Result<Vec<GlobalSkillEntry>>`。
    Settings 继续只订阅 global store；ChatForm project scope 由页面上下文驱动。

app/ai-chat2/src/features/settings.rs
  - 增加 `mod skills;`
  - `SettingsView` 增加 `skills_settings: Entity<SkillsSettingsPage>`
  - `SettingsPageKey` 增加 `Skills`
  - `settings_page_specs_for_i18n` 增加 `settings-page-skills`
  - render match 增加 `SettingsPageKey::Skills`
  - Settings sidebar 使用 `IconName::Sparkles`
  - 页面排序建议：`General`、`Appearance`、`Provider`、`Projects`、`Prompts`、`Skills`、`Shortcuts`

app/ai-chat2/src/features/settings/skills.rs
  - `SkillsSettingsPage` 顶层 entity。
  - 持有搜索输入、catalog selection、原生 `gpui::ListState`、derived rows/items、行展开状态和内容读取任务。
  - 渲染 toolbar、empty/error/no-results 和 virtualized skill row list。

app/ai-chat2/src/features/settings/skills/rows.rs
  - `SkillCatalogRow`
  - `SkillCatalogEntryView`
  - `SkillContentPanelState`
  - `skill_catalog_rows(entries: &[GlobalSkillEntry])`
  - `skill_catalog_list_items(rows: &[SkillCatalogRow]) -> Vec<PathBuf>`
  - `filter_skill_catalog_rows(rows, query)`
  - 使用 GPUI 原生 `gpui::list` + `gpui::ListState::measure_all()` 承载可变高度虚拟列表。
  - 使用 `gpui_component::collapsible::Collapsible` 在同一卡片内展示内容。
  - 行内部使用 app-local row 组合，不使用 `Dialog`，不使用开关。
```

## 类型设计

`state::skills`：

```rust
pub(crate) type GlobalSkillCatalogStore =
    gpui_store::SharedStore<GlobalSkillCatalogState, GlobalSkillCatalogBackend>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GlobalSkillCatalogState {
    entries: Vec<GlobalSkillEntry>,
    last_refreshed_at: Option<time::OffsetDateTime>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GlobalSkillEntry {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) source_kind: SkillSourceKind,
    pub(crate) skill_file_path: PathBuf,
    pub(crate) directory_path: PathBuf,
    pub(crate) search_text: String,
}

pub(crate) struct LoadedSkillContent {
    pub(crate) content: String,
    pub(crate) content_sha256: String,
}
```

`GlobalSkillCatalogBackend`：

- `backend_id = "filesystem:global-skills"`。
- `load()` 扫描 `SkillCatalog::scan(None)` 并映射为 `Vec<GlobalSkillEntry>`。
- 排序使用 `source_kind ASC, name ASC, skill_file_path ASC`。
- `reconcile()` 只在 entries / error / refresh timestamp 变化时通知 UI。

`features/settings/skills.rs`：

```rust
pub(super) struct SkillsSettingsPage {
    search_input: Entity<InputState>,
    skills: StoreSelection<Vec<GlobalSkillEntry>>,
    last_error: StoreSelection<Option<String>>,
    list: ListState,
    rows: Vec<SkillCatalogRow>,
    items: Vec<PathBuf>,
    expanded: BTreeMap<PathBuf, SkillContentPanelState>,
    load_tasks: BTreeMap<PathBuf, Task<()>>,
    _subscriptions: Vec<Subscription>,
}

enum SkillContentPanelState {
    Loading,
    Loaded {
        content: SharedString,
        content_sha256: SharedString,
    },
    Failed {
        message: SharedString,
    },
}
```

`features/settings/skills/rows.rs`：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillCatalogRow {
    pub(super) key: PathBuf,
    pub(super) entry: GlobalSkillEntry,
    pub(super) name: SharedString,
    pub(super) description: Option<SharedString>,
    pub(super) source_label: SharedString,
    pub(super) skill_file_path: SharedString,
    pub(super) search_text: String,
}

#[derive(IntoElement, Clone)]
pub(super) struct SkillCatalogEntryView {
    row: SkillCatalogRow,
    content: Option<SkillContentPanelState>,
    on_toggle_content: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
}
```

行 key 使用 `skill_file_path`，避免未来多 source 出现同名 skill 时 UI id 冲突。runtime 当前仍按 name 激活 skill，
本页面只是 catalog 浏览，不改变激活语义。

列表规模不能假设很小。即使当前只有用户级 `~/.agents/skills`，后续全局 catalog 可能合并 bundled、
plugin 或其他 roots，所以第一版就使用 GPUI 原生 `ListState::measure_all()` 虚拟列表，而不是普通
`v_flex().children(...)`。

每个 skill 在虚拟列表里始终只对应一个 `PathBuf` item。展开内容不是额外插入的新 list item，而是通过
`gpui_component::collapsible::Collapsible` 渲染在同一个 `SkillCatalogEntryView` 卡片内部；展开/收起只对当前
row 调用 `ListState::remeasure_items`。没有使用 `gpui_component::list::ListDelegate`，因为该组件要求每个 item
同高，不适合这种 row 内部可变高度内容；也没有使用 `Accordion`，因为本页允许多个 skill 同时展开，不需要
accordion 的互斥状态管理。

## 数据流

```text
app init
  -> state::skills::init(cx)
  -> GlobalSkillCatalogStore loads SkillCatalog::scan(None)
  -> SettingsView creates SkillsSettingsPage
  -> SkillsSettingsPage StoreSelection<Vec<GlobalSkillEntry>>
  -> search_input filters derived SkillCatalogRow list
  -> sync native ListState row path items
  -> Refresh button
       -> GlobalSkillCatalogStore::refresh_from_backend(cx)
       -> rows update
       -> sync ListState splice/remeasure
  -> View Content button
       -> if row is collapsed: set Loading and spawn file read
       -> state::skills::load_skill_content(path)
       -> Loaded/Failed stored in SkillsSettingsPage.expanded
       -> same row's Collapsible content re-renders inline panel
  -> second click
       -> remove expanded state for that path
       -> remeasure that row

ChatForm / ComposerEditor 后续接入：
  -> ChatForm owns current SkillCatalogScope
  -> no project: read GlobalSkillCatalogStore snapshot and subscribe to changes
  -> project: background load state::skills::load_catalog_entries(Project { root })
  -> ChatForm calls ComposerEditor::set_skill_entries(entries)
  -> ComposerEditor uses entries for token parse, "$" completion rows, and SkillActivationRequest payload
```

刷新规则：

- 手动刷新只重新扫描 catalog metadata，不预加载每个 skill body。
- 已展开行在刷新后如果路径仍存在，保留展开状态并继续显示已加载内容；如果路径消失，移除展开状态。
- 刷新失败时保留上一次成功 entries，页面显示非阻塞错误提示和通知。

内容读取规则：

- 只在展开时读取 `SKILL.md`。
- 读取时复用 `SkillLoader::load` 或等价 helper，以保证 hash 逻辑和 runtime skill activation 一致。
- 内容 panel 展示原始文本和 `content_sha256` 的短 hash（例如前 12 位）用于 debug。
- 不读取 `references/`、`scripts/`、`assets/` 等额外文件；本页只展示 entry 的 `SKILL.md`。

搜索规则：

- 显示字段只展示 `skill_file_path`；搜索索引仍包含 name、description、skill_file_path、directory_path、source label，
  便于用户按 root 路径查找。
- v1 不搜索 `SKILL.md` body，避免为了列表搜索预读所有文件内容。
- 搜索大小写不敏感，复用 `foundation::search::field_matches_query`。

## UI 和组件选型

Settings `Skills` 页面使用 virtualized 管理页列表 + inline expandable content：

- 顶部 toolbar：
  - 左侧 `Input` 搜索框，prefix `IconName::Search`，`cleanable(true)`。
  - 右侧 `Button` 刷新，icon `IconName::RefreshCcw`，刷新中禁用或显示 loading。
- 主体：
  - GPUI 原生 `list` / `ListState` 管理行列表，复用 Prompts/Projects settings 的 bordered row 视觉。
  - 每行直接展示 name/source/description/`SKILL.md` file path 和“查看内容”按钮；不放重复的 skill 行图标。
  - path 只显示具体 `SKILL.md` 绝对路径，不同时展示 directory path；路径 chip 尽量占满卡片宽度，放不下时 truncate。
  - “查看内容”按钮放在行底部左侧 footer，不放在标题行最右侧。
- Inline content panel：
  - 作为 `Collapsible::content(...)` 渲染在同一个 row/card 内部，不作为独立 list item 插入列表。
  - 背景使用 muted/subtle surface，固定最大高度；内容区域使用 row 专属 `ScrollHandle` +
    `track_scroll` / `vertical_scrollbar`，避免多个虚拟列表 row 共享同一个调用位置的 scrollbar state。
  - 面板外层容器使用 `occlude()`，因为 `gpui::list` 的 wheel listener 在子元素 paint 后注册，bubble 阶段会先于
    子元素 listener 执行；只在子元素里 `stop_propagation()` 不足以阻止外层 list 滚动。
  - 内容区域不能再加 `overflow_y_scroll()`，否则同一个 div 还会注册 GPUI 内置 scroll listener，和自定义 handler
    形成内层双消费。
  - 内容区域的 `on_scroll_wheel` 手动推进自身 `ScrollHandle`，并始终 `stop_propagation()`；只有到顶/到底后仍有
    剩余 delta 时，才显式转发给外层 `ListState::scroll_by(...)`，实现浏览器式 scroll chaining。
  - 如果本次滚轮 delta 超过内层可滚范围，handler 计算 residual，并通过 `SkillsSettingsPage` 调用外层
    `ListState::scroll_by(...)`；这样只在内层已经到顶/到底时才让外层继续滚动。
  - 内容以等宽字体显示原始 `SKILL.md` 文本，支持选中复制。
- Empty/error:
  - 空 catalog：`settings_empty_message(skill-empty)`。
  - 搜索无结果：`settings_empty_message(skill-search-empty)`。
  - 扫描失败：保留已加载列表，并通过 `NotificationType::Error` + inline muted error text 提示。

组件清单：

| 组件 | 用途 |
| --- | --- |
| `Input` / `InputState` | skill 搜索 |
| `Button` | Refresh、View Content |
| `gpui::list` / `gpui::ListState` | skill catalog 可变高度虚拟化列表和可扩展行渲染 |
| `Collapsible` | 同一卡片内展开/收起 raw `SKILL.md` 内容 |
| `Icon` | 按钮、loading 和 error 图标 |
| `Label` | 名称、描述、路径、empty/error 文案 |
| `Notification` | refresh/load content 失败 |
| `ScrollableElement` | inline content panel 的垂直滚动 |
| app-local row composition | 每个 list item 的视觉结构，不引入通用 `Table` / `DataTable` |

不使用：

- `Dialog`：内容查看不是 modal 流程。
- `Switch` / `Toggle`：本阶段不做开启/关闭逻辑。
- `TextView::markdown`：本页要展示原始 `SKILL.md` 内容，不渲染 Markdown。
- 普通 `v_flex().children(...)` 渲染整个列表：catalog 规模不能假设很小。
- `gpui_component::list::ListDelegate`：组件库要求同高 item，本页展开内容是可变高度。
- `Accordion`：本页允许多个 skill 同时展开，且不需要统一的互斥 accordion state。
- 直接使用底层 `VirtualList`：原生 `gpui::list` 已覆盖当前可变高度和滚动控制需求。

## 图标

只使用 `app/ai-chat2/src/foundation/assets.rs` 已有 app-local Lucide `IconName`：

- Settings sidebar：`IconName::Sparkles`
- Search：`IconName::Search`
- Refresh：`IconName::RefreshCcw`
- View content collapsed：`IconName::ChevronDown`
- View content expanded：`IconName::ChevronUp`
- Load failed / scan failed：`IconName::CircleAlert`

不新增 Lucide variant、不新增 runtime asset、不在 feature module 写 raw SVG path。

## i18n

新增 key 同步写入 `app/ai-chat2/locales/en-US/main.ftl` 和
`app/ai-chat2/locales/zh-CN/main.ftl`：

- `settings-page-skills`
- `skill-search-placeholder`
- `button-refresh-skills`
- `button-view-skill-content`
- `button-hide-skill-content`
- `skill-empty`
- `skill-search-empty`
- `skill-description-empty`
- `skill-source-user`
- `skill-source-project`
- `skill-source-built-in`
- `skill-source-plugin`
- `skill-content-loading`
- `skill-content-hash`
- `notify-refresh-skills-failed`
- `notify-load-skill-content-failed`

Settings search keywords 需要覆盖英文、中文和拼音语义：

```text
skills skill catalog instruction global agent 技能 能力 指令 全局 tishici jineng quanju
```

## 数据库和配置

数据库：

- 不新增 migration。
- 不新增 `skills`、`skill_roots` 或 `skill_content` 表。
- 不改 `conversation_items` schema。
- Skill activation snapshot 仍只在实际对话加载 skill 时写入。

配置：

- 不新增 `config.toml` 字段，因为本阶段没有 enable/disable、排序、filter preference 或 root preference。
- 刷新结果不持久化为 config cache。

全局数据管理：

- 使用 `state::skills::GlobalSkillCatalogStore` 作为 app-level catalog source。
- Settings 页面通过 `StoreSelection<Vec<GlobalSkillEntry>>` 订阅列表。
- Composer 后续应从 `state::skills` 读取 catalog，不再直接在 `ComposerEditor` 内调用 `SkillCatalog::scan(...)`。
- no-project ChatForm 订阅 `GlobalSkillCatalogStore`。
- project-aware ChatForm 使用 `state::skills::load_catalog_entries(SkillCatalogScope::Project { root })` 后台加载；
  结果是当前 ChatForm 的瞬时 UI state，不写 DB，不写 config。
- Settings refresh 只刷新 global store；project-aware catalog refresh 由页面项目上下文变化或后续显式 refresh 触发。

## 新增依赖

不新增依赖库。

理由：

- catalog scan、frontmatter parse、content hash 已在 `ai-chat-agent` 内存在。
- UI 所需 `Input`、`Button`、`Icon`、`Notification`、scrollbar 均来自当前 `gpui-component`。
- 搜索复用 `foundation::search::field_matches_query`。
- 时间戳如需要使用现有 `time` 依赖。

## 验证计划

文档落地阶段只运行：

- `git diff --check`

第一版代码实现后运行：

- `cargo fmt`
- `cargo test -p ai-chat-agent skill`
- `cargo test -p ai-chat2 skill`
- `cargo test -p ai-chat2 settings`
- `cargo check -p ai-chat2`
- `git diff --check`

如果实现时把 Composer catalog 也迁移到 `state::skills`：

- 追加 `cargo test -p ai-chat2 composer`

如果后续引入 bundled/plugin skill roots：

- 追加 `cargo test -p ai-chat-agent skill_catalog`
- 补充 root precedence / duplicate name 测试。
