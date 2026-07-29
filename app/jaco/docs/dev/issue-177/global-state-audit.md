# Jaco 全局状态与底层数据绕过现状审计

关联 issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)

本文只记录当前代码事实和已经确认的问题，不决定目标架构、required/recoverable
分类、迁移顺序或具体 API。测试代码中的临时 Global 和直接底层读取不计入生产统计。

## 1. 统计口径与结论

本次将以下两类状态计为 Jaco Global：

1. Jaco 自己定义并实现 `gpui::Global` 的类型；
2. 由 `gpui_store::SharedStore::install_global_with_backend` 直接安装为 Global 的
   Jaco 状态。

`gpui-component`、`gpui-tokio` 和 `app-theme` 安装的框架级服务不计入 15 份
Jaco-owned Global；Jaco 直接使用的外部 Global 在第 6 节单独记录。

| 统计项 | 数量 | 说明 |
| --- | ---: | --- |
| Jaco-owned GPUI Global | 15 | 12 个显式 `impl Global`，3 个直接安装的 `SharedStore` |
| Jaco 直接使用的外部 Global | 5 | `ThemeRegistry`、`Theme`、组件菜单 `GlobalState`、系统强调色、Tokio runtime |
| 使用 `gpui-store` 的 Global | 6 | 全部是 `SharedStore` |
| `LocalStore` | 0 | Jaco 当前没有使用 |
| 自定义 `StoreBackend` | 3 | config、prompt、global skill |
| `StoreCommitBackend` | 1 | 仅 config |
| `StoreSelection` 字段 | 5 | project 2、prompt 1、skill 2 |
| `StoreBinding` 字段 | 1 | chat form config committed binding |
| SQLite catalog 使用 memory backend | 3 | provider、project、shortcut |

启动安装顺序位于 `app/jaco/src/app.rs:125-145`。Screenshot Global 不在启动期安装，
而是在打开截图选择层时惰性创建。

## 2. Jaco-owned Global 清单

### 2.1 持久化根、catalog 与投影

| Global | 定义与安装 | 内容与底层来源 | `gpui-store` | 当前初始化、同步和错误语义 |
| --- | --- | --- | --- | --- |
| `JacoConfigStore` | `state/config.rs:38-53,447-453` | `JacoConfig`；`config.toml` | `SharedStore<JacoConfig, JacoConfigBackend>`；同时实现 `StoreBackend` 和 `StoreCommitBackend` | 文件 I/O 错误向 `app::init` 返回；TOML 解析错误变为默认配置加 `load_error`。committed update 先写文件，成功后更新 store。没有文件变更订阅和 reload/retry。 |
| `FreshStoreGlobal` | `database.rs:8-16,31-42` | `FreshStore` / `FreshRepository`；SQLite | 否 | 打开数据库失败向 `app::init` 返回。它是原始 persistence service，不是业务 snapshot；`database::repository(cx)` 使整个 app 都能绕过更高层 catalog。 |
| `LayoutStateStore` | `state/layout.rs:38-46,249-257` | `JacoLayoutState`；`state.toml` | 否，普通 `Entity` Global | 首次加载失败向 `app::init` 返回；修改后 300 ms debounce 写文件，写失败只记日志；退出时再次保存。Global 缺失时读取 API 会重新读文件并回退默认值。 |
| `ProviderCatalogGlobal` | `state/providers.rs:7-24,108-115` | provider、每个 provider 的 models、enabled model choices；SQLite | 包装 memory `SharedStore<ProviderCatalogSnapshot>` | 初始化错误被 `unwrap_or_default()` 转成空 snapshot；写 DB 后手工 refresh，refresh 错误被直接丢弃，旧 snapshot 保留但没有错误状态；读取 helper 在 Global 缺失时回退 DB。 |
| `ProjectCatalogGlobal` | `state/projects.rs:13-35,147-156` | `list_sidebar_projects()` 返回的未删除 normal projects；SQLite | 包装 memory `SharedStore<ProjectCatalogSnapshot>` | 初始化错误被转成空列表；写 DB 后手工 refresh，refresh 错误被丢弃。snapshot 不含 scratch projects。 |
| `PromptCatalogStore` | `state/prompts.rs:10-27,29-68` | 全部 prompts；SQLite | `SharedStore<PromptCatalogState, PromptCatalogBackend>` | backend 初始化错误向 `app::init` 返回。mutation 先提交 DB，再 `refresh_from_backend`；refresh 失败会把已经提交的 mutation 作为 `Err` 返回。state 没有 refresh error 或 reconciliation-only retry。 |
| `GlobalSkillCatalogStore` | `state/skills.rs:11-45,99-156` | 用户级 skill 索引；文件系统 | `SharedStore<GlobalSkillCatalogState, GlobalSkillCatalogBackend>` | 扫描错误存入 `last_error`，并通过 `entries: None` 保留最后有效 entries；显式 refresh 只重扫 catalog。这是现有 recoverable snapshot 参考。 |
| `ShortcutCatalogGlobal` | `state/shortcuts.rs:12-34,48-72` | 全部 shortcut records；SQLite | 包装 memory `SharedStore<ShortcutCatalogSnapshot>` | 初始化错误被转成空列表；mutation 提交 DB、同步系统 hotkey 后再 refresh。refresh 失败让整个命令返回 `Err`，但 DB 和 hotkey 运行时可能已经成功。读取 helper 在 Global 缺失时回退 DB。 |
| `WorkspaceStoreGlobal` | `state/workspace.rs:15-23,72-128,252-259` | home route、sidebar projection、展开状态、pending project、`last_error`；SQLite + project catalog | 否，普通 `Entity` Global | 观察 Project catalog，但每次 reload 又查询 projects 和 conversations。加载失败保留旧 snapshot 并设置 `last_error`。它是带显式 recoverable error 的派生状态参考，但 project 数据源仍不唯一。 |

### 2.2 运行时、服务与 UI 生命周期 Global

| Global | 定义与安装 | 内容与来源 | 当前引用和错误语义 |
| --- | --- | --- | --- |
| `McpRuntimeGlobal` | `state/mcp.rs:27-35,654-662` | MCP session manager、server/OAuth/disconnect tasks、status、runtime error；配置来自 `JacoConfigStore`，状态来自 MCP 网络/进程 | settings MCP 页面和 dialog 读写；agent 启动通过它准备 tools。init 本身不加载 persisted rows；具体连接错误保存在 runtime status/`last_error`。 |
| `ConversationRuntimeGlobal` | `state/conversation_runtime.rs:19-27,356-371` | active runs、取消 token、approval broker、run tasks、last errors；SQLite + agent runtime | conversation detail、temporary window、new-conversation flow 读取、订阅和启动/停止 run。启动时 interrupted-run recovery 失败会让 `app::init` 失败。 |
| `GlobalHotkeyState` | `state/hotkey.rs:98-108,133-189` | 系统 hotkey backend、temporary hotkey、registered shortcuts、actions 和 diagnostics | app diagnostics、General settings、shortcut commands、screenshot overlay 使用。它在 catalogs 之后初始化，却重新查 DB。若 initial shortcut load 失败，`init` 在 `set_global` 前返回；`app::init` 只记录日志并继续，Global 实际不存在。 |
| `I18n` | `foundation/i18n.rs:18-27` | 当前 locale 和编译进二进制的 en-US/zh-CN Fluent bundles；语言来自 config Global | 几乎全部 app、component 和 feature UI 使用。config observer 会重新调用 `init_i18n`，以替换整个 Global。无外部文件读取。 |
| `TemporaryWindowLifecycleState` | `app/temporary_window.rs:16-41` | delay-close task、macOS frontmost app、临时窗口显示/隐藏生命周期 | menus、hotkey 和 Temporary view 通过模块 facade 更新；memory-only，无持久化加载。 |
| `ScreenshotOverlayState` | `features/screenshot/overlay.rs:32-35,99-123` | 当前截图选择 session、shortcut snapshot、overlay window handles | 截图打开时惰性安装；overlay 事件和 `GlobalHotkeyState` 消费；memory-only。 |

## 3. Global 引用地图

下表按生产模块归纳 reader、writer 和 observer。经模块 facade 间接访问也计入；测试夹具不计。

| Global | 生产引用位置 | 引用方式 |
| --- | --- | --- |
| `JacoConfigStore` | `components/chat_input.rs`；`features/home/shell.rs`；`features/settings.rs`；`features/temporary.rs`；`app/about.rs`；`features/settings/{general,appearance,mcp}.rs` 与 `settings/mcp/dialog.rs`；`state/{mcp,hotkey,theme,projects,attachments}.rs`；`foundation/i18n.rs`；`database.rs` | committed binding、committed update、read、observe；提供 app settings、chat form、MCP config、data dir |
| `FreshStoreGlobal` | 所有 `database::repository(cx)` 调用；主要位于 `state/{providers,projects,prompts,shortcuts,workspace,temporary,hotkey,conversations,conversation_runtime}.rs` 和 `features/settings/{provider,prompts/dialog,shortcuts/dialog}.rs` | clone `FreshRepository` 后直接 query/mutation；没有订阅或 snapshot 通知 |
| `LayoutStateStore` | `app.rs`；`features/home/shell.rs`；`features/settings.rs` | 读取 window placement/sidebar width，写 bounds/width，退出保存 |
| `McpRuntimeGlobal` | `features/settings/mcp.rs`、`features/settings/mcp/dialog.rs`；`state/conversation_runtime.rs` | settings 订阅/命令；agent run prepare |
| `ConversationRuntimeGlobal` | `components/conversation_detail.rs`；`features/temporary.rs`；`features/home/new_conversation.rs` | subscribe run events、读运行状态、start/stop run |
| `ProviderCatalogGlobal` | `components/{chat_input,run_settings}.rs`；`features/settings/shortcuts.rs` 与 `shortcuts/dialog.rs`；`state/hotkey.rs` | 模型 options、submit/shortcut validation、hotkey model resolution；RunSettings 观察 catalog |
| `ProjectCatalogGlobal` | `features/home/new_conversation.rs`；`features/settings/projects.rs`；`state/workspace.rs` | 两个 `StoreSelection`、额外 observe、workspace reload trigger |
| `PromptCatalogStore` | `features/settings/prompts.rs`；`features/settings/shortcuts.rs` | 一个 `StoreSelection`；shortcut composite snapshot/observe |
| `GlobalSkillCatalogStore` | `components/chat_input.rs`；`features/settings/skills.rs` | 初始读取、observe、两个 `StoreSelection`、手动 refresh |
| `ShortcutCatalogGlobal` | `features/settings/shortcuts.rs` | 页面 observe 和 composite snapshot；hotkey runtime 没有读取它 |
| `WorkspaceStoreGlobal` | `features/home/{shell,sidebar,sidebar/menu,sidebar/search,new_conversation}.rs`；`features/temporary.rs`；`components/conversation_detail.rs` | route/sidebar read、search、project/conversation commands、reload |
| `GlobalHotkeyState` | `app.rs`；`features/settings/{general,shortcuts}.rs`；`state/shortcuts.rs`；`features/screenshot/overlay.rs` | diagnostics、temporary hotkey update、shortcut registration reconciliation、screenshot completion |
| `I18n` | `app/{app,about,menus}.rs`，`components/**`，`features/**`，少量 `state/{conversations,hotkey,projects}.rs` | 读取本地化文本；language commit 后整体替换 Global |
| `TemporaryWindowLifecycleState` | `app/menus.rs`；`state/hotkey.rs`；`features/temporary.rs` | 通过 `app/temporary_window.rs` facade 打开、切换、延迟隐藏 |
| `ScreenshotOverlayState` | `features/screenshot/overlay.rs` | 同模块内 session lifecycle；捕获完成后回调 Hotkey Global |

## 4. `gpui-store` 使用分类

### 4.1 Store 清单

| Store | Backend 类型 | commit 能力 | 下游 projection API | 主要缺口 |
| --- | --- | --- | --- | --- |
| `JacoConfigStore` | file `JacoConfigBackend` | 有，`try_update*` / committed binding | `StoreBinding`、直接 read/observe | malformed TOML 仍以默认业务配置继续启动；没有 reload/retry |
| `ProviderCatalogGlobal` 内部 store | `MemoryBackend` | 无 | 直接 read/observe | DB load/refresh/reconciliation 对 store 不可见 |
| `ProjectCatalogGlobal` 内部 store | `MemoryBackend` | 无 | 2 个 `StoreSelection` + observe | 同上；snapshot 范围不足以覆盖 workspace scratch projects |
| `PromptCatalogStore` | DB projection backend | 无 | 1 个 `StoreSelection` + direct read | post-commit refresh error 被当成 mutation error；无 error state/retry |
| `GlobalSkillCatalogStore` | filesystem projection backend | 无 | 2 个 `StoreSelection` + observe | global scope 已有 recoverable 语义；project/agent 路径仍重新扫描 |
| `ShortcutCatalogGlobal` 内部 store | `MemoryBackend` | 无 | direct read/observe | DB、system hotkey、catalog 三阶段结果没有分别建模 |

三个自定义 backend 的 `Subscription` 都是 `()`；外部变化不会自动推送，刷新完全依赖启动 load
或显式 `refresh_from_backend`。Provider、Project、Shortcut 连 backend 都没有，当前只是把手工加载的
数据库 snapshot 放进 memory store。

### 4.2 Selection 与 Binding

`StoreSelection` 共 5 个字段：

- `NewConversationPage.projects`；
- `ProjectsSettingsPage.projects`；
- `PromptsSettingsPage.prompts`；
- `SkillsSettingsPage.skills`；
- `SkillsSettingsPage.last_error`。

`StoreBinding` 只有 `ChatInputController.chat_form_config`，并且是连接
`JacoConfigBackend` 的 committed binding。Jaco 没有 memory binding，也没有 `LocalStore`。

## 5. 已有 Global 但仍绕过它读取底层来源

### 5.1 已确认可直接消费现有 Global snapshot

#### Provider / model

- Provider settings 初始化直接调用 `list_providers` 和 `list_provider_models`
  （`features/settings/provider.rs:331-439`），而 `ProviderCatalogSnapshot.providers` 已包含完整记录。
- 保存、model enable/disable、model fetch 完成后，state command 已尝试 refresh catalog；settings page
  随后仍重新查 DB 并维护 `providers`/`editors.models` 页面快照
  （`provider.rs:744-764,835-855,949-985`）。
- model fetch 先把 repository 传进异步任务，再按 ID 查询 provider
  （`provider.rs:932-942,1594-1607`）；catalog 已含 `ProviderRecord` 和 secret refs。
- conversation create/send 和 agent 真正启动分别再次查询 provider
  （`state/conversations.rs:72-84,148-174`；
  `state/conversation_runtime.rs:398-416`），而用户提交阶段已经从 provider catalog 解析了
  provider/model/capabilities。

这些路径可能同时持有 catalog snapshot、settings page snapshot、conversation request snapshot 和新查到的
DB record，无法保证同一次操作使用同一 committed provider 版本。

#### Prompt

- Prompt dialog 的唯一性 validation context 直接 `list_prompts()`
  （`features/settings/prompts/dialog.rs:217-231`），失败时 dialog 初始化将依赖降级为空列表；
  `PromptCatalogStore` 已含同一批记录。
- hotkey 触发按 ID 再查 prompt（`state/hotkey.rs:578-595`），但 Prompt catalog 有该记录，
  Shortcut record 也已经持久化 `settings_snapshot.prompt`。

#### Shortcut / hotkey

- `ShortcutCatalogGlobal` 在 hotkey 之前安装（`app.rs:136,145`），但 hotkey 初始化仍执行
  `list_shortcuts()`（`state/hotkey.rs:264-278`）。
- 每次 hotkey 触发再次执行 `get_shortcut()`（`state/hotkey.rs:547-576`）；同一函数只有
  provider/model 已改为使用 provider catalog（`hotkey.rs:603-610`）。
- `reregister_shortcut` 只需已有 committed shortcut，却再次查询 DB
  （`state/shortcuts.rs:150-158`）。

#### Workspace / temporary conversation

- Workspace 明确观察 Project catalog（`state/workspace.rs:84-103`），但 catalog 变化后仅把它当
  reload 信号，`build_sidebar_snapshot` 再次查询 visible projects
  （`workspace.rs:262-303`），搜索又查询一次（`workspace.rs:350-355`）。
- `route_belongs_to_project` 查询当前 conversation（`workspace.rs:239-249`）；对于已经存在于
  `SidebarSnapshot` 的可见 conversation，该关系可从 snapshot 得到。
- Temporary no-project 列表无 query 时与 `WorkspaceStoreGlobal.snapshot.no_project_conversations`
  重复，却始终查询 DB（`state/temporary.rs:15-25`）。有搜索 query 时还需要消息正文搜索，
  当前 Workspace snapshot 不足以替代数据库查询。

#### Skills / 文件系统

- Global skill backend 已扫描用户级 skills 并保存最后有效 snapshot
  （`state/skills.rs:109-137,191-204`）。
- ChatInput 切换到 project scope 后调用 `SkillCatalog::scan(Some(root))`
  （`components/chat_input.rs:327-387`；`state/skills.rs:181-188`）。该 scan 会同时重扫用户目录和
  project 目录（`crates/jaco-agent/src/skills.rs:38-49`），因此绕过 global store 的 entries 和
  `last_error`；失败时 UI 直接替换为空列表。
- Agent 激活 skill 时第三次执行 catalog discovery
  （`crates/jaco-agent/src/runtime.rs:680-699`），而 request 中的 `skill_catalog_hash` 仍为 `None`
  （`state/conversations.rs:285-304`）。UI 选择和实际执行可能看到不同文件版本。

读取用户明确选择的单个 `SKILL.md` 正文是合法的按需内容加载；问题是重复 discovery catalog。

### 5.2 现有 Global 范围不足，不能机械替换

| 当前直接读取 | 已有 Global 的不足 | 当前结论 |
| --- | --- | --- |
| Workspace `list_visible_projects()` | Project catalog 只有 normal/sidebar projects，不含 scratch projects | 先确定 project committed snapshot 边界，再决定扩展一个 catalog 还是拆分 catalog |
| Temporary 带 query 的 conversation 搜索 | Workspace snapshot 不含消息正文搜索索引 | 空 query 可复用 snapshot；非空 query 仍需 search backend 或新 catalog |
| conversation timeline/entry/run/tool invocation 查询 | 没有对应全局 committed catalog | 保留 persistence query，不计为绕过 |
| project-scoped skill discovery | Global skill store 只建模 global scope | 必须合并 global snapshot 与 project-only scan，不能直接删除 project scan |

### 5.3 Command/persistence 层的重复读取，需在设计阶段确定边界

以下读取的数据已存在于 catalog，但它们位于 mutation/事务附近。是否使用 catalog 作为命令输入，
仍要结合并发、数据库约束和 authoritative commit result 决定，本文不先下结论：

- Prompt update 为保留 `enabled`/`sort_order` 再 `get_prompt`
  （`state/prompts.rs:102-121`）；
- Shortcut update/delete/enable 为 runtime diff 再 `get_shortcut`
  （`state/shortcuts.rs:96-148`）；
- Shortcut settings snapshot 再查 prompt/provider/model
  （`state/shortcuts.rs:161-215`）；
- create conversation 按 project ID 再查 project，follow-up 对没有 prompt snapshot 的旧记录再查 prompt
  （`state/conversations.rs:209-245`）；
- conversation/run 的事务写入、timeline 加载与 recovery 继续直接使用 repository。

### 5.4 隐式底层 fallback

生产启动顺序要求这些 Global 已经存在，但 API 仍在 Global 缺失时读取底层来源：

- `providers_with_models` / `enabled_provider_models` 回退 DB
  （`state/providers.rs:151-195`）；
- `list_shortcuts` 回退 DB（`state/shortcuts.rs:68-73`）；
- `restored_window_placement` 回退读取 `state.toml`，失败再使用默认值
  （`state/layout.rs:260-279`）。

这些 fallback 会把初始化顺序或 Global 缺失问题隐藏成另一条看似有效的数据路径。

### 5.5 已发现但是否属于 issue #177 仍待确定

`JacoConfigStore` 持久化并展示 `http_proxy`（`state/config.rs:109,324-325`；
`features/settings/general.rs:340-378`），但全 workspace 没有网络客户端读取该值。Provider model fetch、
MCP HTTP 和 OAuth 仍使用各自默认网络 client。这是“全局配置存在但运行时未消费”，是否纳入本 issue
需要单独确定。

## 6. 外部 Global 与合法底层访问

Jaco 还直接消费以下框架/共享 crate Global，但它们不是 Jaco-owned business snapshot：

- `gpui_component::Theme` 与 `ThemeRegistry`：`state/theme.rs` 和 Appearance settings；
- `gpui_component::GlobalState`：`app/menus.rs` 写入 app menus，`app/title_bar_menu.rs` 读取；
- `app_theme::SystemAccentThemeState`：Home、Settings、Temporary、About 观察系统强调色；
- `gpui_tokio` runtime：为 provider、MCP、agent 等异步运行提供 executor/service；

`gpui_component::init(cx)` 还会安装其他组件库内部 Global，但 Jaco 没有把它们直接作为业务数据使用，
因此不纳入上述 5 个外部 Global。

以下底层访问当前没有可替代的完整 Global，或者本身就是 Global/backend owner 的职责，不计为问题：

- catalog backend 的首次 load/refresh 和数据库 mutation commit；
- config/layout backend 自身的文件读写；
- conversation timeline、entry、run、tool invocation 和 recovery persistence；
- provider/MCP secret 根据 secret refs 按需读取 keychain；
- MCP 环境变量展开、OAuth discovery、HTTP/stdio connection；
- attachment 文件校验、复制、写入和清理；
- 选定 skill 的正文加载；
- 系统 hotkey、clipboard、selection、截图和 OCR；
- tracing 日志目录和日志文件初始化。

## 7. 已确认问题索引

| ID | 问题 | 影响范围 |
| --- | --- | --- |
| GS-01 | Provider、Project、Shortcut 初始化错误被转换成合法空 snapshot | 启动、empty-state UI、后续所有消费者 |
| GS-02 | Provider、Project refresh 失败静默保留旧 snapshot，没有 error/retry | 设置页、模型选择、sidebar/project UI |
| GS-03 | Prompt、Shortcut 在 DB 已提交后因 refresh 失败返回 mutation `Err` | UI 误报保存失败、重复 mutation、unique constraint |
| GS-04 | Shortcut mutation 实际包含 DB commit、system hotkey sync、catalog reconciliation 三份结果，但 API 只返回一个通用 `Result` | shortcut create/update/delete/enable |
| GS-05 | Config TOML 解析失败后用默认 config 继续计算 data dir 并打开数据库 | 可能打开与用户配置不同的默认数据目录；只有提示和 save block，没有 reload |
| GS-06 | Hotkey 初始 DB load 失败时应用继续运行，但 `GlobalHotkeyState` 根本没有安装 | 后续 update helper 静默 no-op，diagnostics 变成默认空值 |
| GS-07 | raw `FreshRepository` 对 feature/component/state 普遍可见 | catalog 不能形成强制的单一 committed source |
| GS-08 | Provider settings、conversation、agent startup 各自重新查询和缓存 provider/model | 同一次操作可能使用不同版本的记录、settings、capabilities |
| GS-09 | Prompt dialog、hotkey 和 shortcut 路径绕过 Prompt/Shortcut catalogs | validation、触发和 UI snapshot 语义不一致 |
| GS-10 | Workspace 把 Project catalog 当通知信号，仍自行查询 project rows | project 数据有两个读取与错误通道 |
| GS-11 | Global、project UI、agent runtime 分别 discovery skills | last-valid/error 语义丢失，UI 选择与实际执行可能漂移 |
| GS-12 | Global 缺失时回退 DB/文件的 helper 隐藏初始化错误 | provider、shortcut、layout |
| GS-13 | 除 Skill/Workspace 外，recoverable catalog 没有可观察 refresh error；所有 backend 都没有 external subscription | 无统一刷新状态或 reconciliation-only retry |
| GS-14 | `http_proxy` 已持久化但没有任何运行时网络消费者 | 设置看似生效，实际未接线；是否纳入 #177 待定 |

后续计划应基于这些事实决定状态分类、catalog 边界和错误契约；本文不把候选方案写成既定设计。
