# Issue #159 ai-chat2 UI 全量工作清单

本文档是 GitHub issue #159 的细化开发清单和全量能力追踪板。父级协调文档仍是
`app/ai-chat/docs/dev/issue-137-llm-abstractions.md`；本文档负责把
`app/ai-chat2` UI、壳、本机状态、可观测性和旧 `app/ai-chat` 能力映射拆成可执行事项。
侧边栏专项计划见 `app/ai-chat/docs/dev/issue-159-ai-chat2-sidebar.md`。
Agent conversation page 专项计划见
`app/ai-chat/docs/dev/issue-159-ai-chat2-agent-conversation-page.md`。
Temporary Conversation Window 专项计划见
`app/ai-chat/docs/dev/issue-159-ai-chat2-temporary-window.md`。
Prompt 设置专项计划见
`app/ai-chat/docs/dev/issue-159-ai-chat2-prompt-settings.md`。

最后同步时间：2026-06-12。

当前实现基线：`codex/issue-137-llm-abstractions`。foundation 已通过 PR #164 合入集成分支。
当前增量分支为 `codex/issue-159-ai-chat2-ui`；live GitHub 查询显示 #137/#159 仍 open，当前分支暂无新的
open PR，后续仍需要开 PR 合入集成分支。

当前状态：进行中。已合入的 foundation 包含基础设施壳、app chrome、file-backed logging、About、Sidebar/home
skeleton、Home root/sidebar 结构修正、ChatForm 视觉预览、`ComposerEditor` 第一版输入内核、cursor/scroll
修正、Unicode/grapheme-aware 编辑、真实 Settings shell + General/Appearance/Projects、New Conversation 默认页、
新对话项目选择器、no-project 默认语义、Codex-style composer/project tray polish、main/settings window placement
持久化、基础 parity 修复、Provider settings 第一阶段（保存校验、secret refs/GPUI credentials、
真实 model fetch、`gpui-component::ListState` provider/model lists、provider list panel/row separator
polish、右侧 detail 整体滚动和 model switch 事件修复）、DB-backed Composer model picker
（读取 enabled provider/model cache、能力标签、Provider Settings deep-link），以及 provider model
capability source / reasoning control 第一版（Ollama/Gemini/OpenRouter API discovery、OpenAI/Anthropic/
DeepSeek/Mistral docs-derived profile、Composer token budget selector 和 provider-specific reasoning
params），以及 provider brand logo 资产框架（Simple Icons 来源的 app-owned SVG、`ProviderVisual`
fallback 和 Settings/ChatForm 渲染接线）、project-first sidebar 第一版（新对话/搜索入口、置顶、
项目展开、无项目对话、hover action、项目菜单、conversation search、右侧 conversation route 和
project/conversation soft-delete）。GitHub #159 仍 open；PR #164 已从
`codex/issue-159-ai-chat2-ui` 合入 `codex/issue-137-llm-abstractions`。本轮已补齐
Sidebar action row 视觉一致性：顶部“新对话/搜索”和底部
“设置”统一使用 hover-only shortcut badge，并把跨平台快捷键改为 GPUI `secondary` 语义。
Agent Conversation Page 首版已在 `dba4f7c` 实现：New Conversation 发送后创建 fresh conversation
和首条 user item；无项目发送创建每会话 scratch project；sidebar 即时刷新并打开右侧 conversation page；
conversation page 使用 GPUI 原生 `ListState` / `list` timeline + 显式滚动条 + 底部 ChatForm；运行中禁用发送按钮；
真实 `AgentRuntime` 通过 observer 事件刷新页面；timeline 支持 user bubble、agent final markdown/details
collapse、hover copy/time、Codex-style timestamp、复制成功按钮 `Check` 两秒和失败通知。2026-06-11
本地增量已补齐 Codex-style stop generation：运行中 ChatForm 发送按钮切换为停止按钮，点击后 cancel
当前 run token，并在 100ms grace 后强制把仍未结束的 run 终态化为 `Canceled`、移除 active run 并发
`RunFinished`。完整多模态 timeline、Shortcut settings、manual provider model editor、retry/resend、approval action、
rich tool UI 仍未完成；Prompt Settings 第一版已实现，采用 `prompts.content TEXT` 简化模型、管理页行列表、modal 查看/编辑、正文多行编辑和硬删除。
真实 Temporary Conversation Window 首版已实现：顶部单行搜索、no-project
conversation 列表、右侧 new/detail、键盘 focus、`secondary-n` 和真实 agent run 已接线。Temporary 首版已把
Home-only 的 ChatForm、composer/picker、conversation detail/timeline 和纯格式化函数抽到
`components` / `foundation` / `state`，避免 `features/temporary` 横向调用 `features/home`。2026-06-12
已补齐 macOS IME 层级修复：Temporary Window 保留 `WindowKind::PopUp` 的 popup lifecycle，但通过
`window-ext` 将实际 window level 从 `NSPopUpWindowLevel = 101` 覆盖到 `NSModalPanelWindowLevel = 8`，
对齐 Raycast/uTools 搜索窗层级带，避免输入法候选窗被 101 层级干扰。
本轮后续整理已把 sidebar 热路径的
project/conversation pin/remove 状态从 `metadata_json` 拆到 fresh DB columns，repository 和
`ai-chat2` 状态层直接读写列；由于 fresh DB 仍未进入 `main`，该变更按 pre-main baseline schema
清理处理。

已完成提交：

- `b749528 feat(ai-chat2): wire infrastructure shell`
- `0843e15 feat(ai-chat2): add app chrome and bundle shell`
- `e7077fc feat(ai-chat2): implement about window`
- `6d4a34f feat(ai-chat2): add sidebar home shell`
- `ef1c3f4 refactor(ai-chat2): split home root and sidebar`
- `d7a5751 ai-chat2: add chat form preview`
- `e6e766e feat(ai-chat2): implement composer editor`
- `34ccb6f fix(ai-chat2): refine composer cursor styling`
- `26a89fa Fix ai-chat2 composer scrolling`
- `09b2f22 feat(ai-chat2): add unicode-aware composer editing`
- `ed59682 feat(ai-chat2): add settings shell`
- `57bb3d5 fix(ai-chat2): align basic parity behaviors`
- `edb4a3d feat(ai-chat2): add project settings management`
- `b3374b4 feat(ai-chat2): add new conversation page`
- `5e574c7 Implement ai-chat2 provider settings foundation`
- `4d4110b feat(ai-chat2): wire provider settings model fetch`
- 本轮实现：DB-backed Composer model picker、provider model capability source 和 reasoning controls
- 本轮实现：provider brand assets / `ProviderVisual` / app-assets proc macro refactor
- 本轮实现：project-first sidebar 第一版和 Sidebar shortcut action row polish
- `4eb9e5e fix(ai-chat2): satisfy clippy on foundation branch`
- `dba4f7c Implement ai-chat2 agent conversation page`
- 本轮实现：Codex-style stop generation（ChatForm stop 按钮、100ms grace、run key 防迟到 finish、
  `AgentRuntime::cancel_run` 终态化 run/provider step/tool invocation）
- 本轮实现：Temporary Window macOS IME 层级修复（保留 `WindowKind::PopUp` 生命周期，实际 window
  level 从 `NSPopUpWindowLevel = 101` 覆盖到 `NSModalPanelWindowLevel = 8`）

## 状态定义

| 状态 | 含义 |
| --- | --- |
| 已完成 | 代码可运行，且不是占位。 |
| 占位 | 入口、窗口、菜单或资源已接线，但没有真实业务行为。 |
| 后端已具备 | `ai-chat-core` / `ai-chat-db` / `ai-chat-agent` 已有模型或 API，但 `ai-chat2` UI 未消费。 |
| 已有专项计划 | 已固定开发计划或专项进度板，但对应代码能力尚未实现。 |
| 未开始 | 旧 `app/ai-chat` 有对应能力，新 `app/ai-chat2` 还没有。 |
| 暂不做 | 不属于当前 #159 UI 阶段，除非后续重新定义 scope。 |
| 不照搬 | 旧 `app/ai-chat` 概念或实现形状不适合 fresh app，应按新模型重做或删除入口。 |
| 已替代 | 旧能力已由 `ai-chat-core` / `ai-chat-db` / `ai-chat-agent` / `ai-chat2` 新边界承接，不应复制旧实现。 |

占位必须显式记录。能打开一个窗口、菜单项或快捷键入口，不代表对应业务能力已经完成。旧能力也必须明确标注为
“继续实现”、“不照搬”或“已替代”，避免后续凭印象漏项或误迁移旧架构。

## #159 目标边界

#159 仍是完整 `ai-chat2` UI issue。目标不是复刻旧 `app/ai-chat`，而是基于 fresh
database、`ai-chat-agent` 和 canonical `conversation_items` 实现新的 project-first agent UI。

必须保留的方向：

- 项目是实际文件夹，scratch/no-project chat 也需要 app 创建真实 scratch folder。
- 所有 conversation 都是 contextual，不再提供 `assistant-only`、`single`、`contextual` mode 控件。
- 新 UI 使用 prompts，不再继续 template 概念。
- Shortcuts 绑定 prompt、provider/model、input source 和 action，不绑定 template 或 mode。
- Timeline 渲染以 `conversation_items` 为真相，不为了展示 tool/reasoning/approval/usage 再依赖 execution tables join。
- Agent loop、tool registry、MCP helper、approval persistence 复用 `ai-chat-agent`，不在 GPUI 层重新实现。
- 旧 `app/ai-chat` 仍是 legacy app，迁移期必须保持可运行。

## 渐进式 UI 开发约束

`ai-chat2` UI 迁移是分阶段完成的，经常会先做 demo / preview / 简化版中间状态。即使如此，代码也必须按最终功能边界拆分文件和模块：

- 输入框、模型选择、思考程度选择、附件入口、prompt selector、picker/list/popover、timeline item、run controls 等未来会独立演进的 UI 单元，应从第一版预览实现开始就放在对应子模块中。
- 不能因为“本轮先做简化实现”而把多个最终独立功能堆进单个大文件；简化的是数据源和行为接线，不是模块边界。
- 当一个 UI 单元会同时被 Home、Temporary、Shortcut、Settings 或其他窗口消费时，必须放到
  `components`、`foundation` 或 `state` 等共享层；不要让 `features/*` 之间横向 import 对方内部模块。
- demo / preview 数据可以临时存在，但必须用清晰命名标识，例如 `preview_*`，并与未来真实 store/provider/db 接口隔离，避免后续替换数据源时重拆 UI 结构。
- 只有真正一次性、不会进入最终产品结构的占位 UI 才可以保持轻量；这类代码必须在模块名、类型名或状态表中明确标注为 placeholder/preview。

## 已完成

| 事项 | 当前位置 | 说明 |
| --- | --- | --- |
| `app/ai-chat2` binary package | `app/ai-chat2` | 已加入 workspace，可独立 `cargo run -p ai-chat2`。 |
| basic tracing + file-backed logging | `app/ai-chat2/src/app.rs` | 已初始化 `tracing_subscriber::fmt` subscriber，并按 app 规范写入 `~/Library/Logs/top.sushao.ai-chat2/data.log`（非 macOS 写入 local data logs）。 |
| fresh DB bootstrap | `app/ai-chat2/src/database.rs` | 使用 `ai_chat_db::FreshStore::open_in_dir` 打开 fresh store；不读取 legacy store。 |
| TOML + DB settings | `app/ai-chat2/src/state/config.rs` | TOML 保存本机启动层配置；未知字段按兼容语义忽略，malformed TOML 会记录 error、重写默认配置并继续启动；language/theme/hotkey/http proxy/default project 来自 fresh DB app settings。 |
| local layout state | `app/ai-chat2/src/state/layout.rs` | 本机 `state.toml` 保存 `sidebar_width`、main/settings window bounds、window mode 和 display id；加载时 clamp/离屏 fallback，拖拽后 debounce 保存，quit 前同步 flush；不进入 fresh DB 或 `ai-chat-core` payload。 |
| typed app settings payload | `crates/ai-chat-core` / `crates/ai-chat-db` | `AppSettingsPayload` 已结构化，DB roundtrip 已覆盖；包含 language、theme、temporary hotkey、HTTP proxy 和 default project。 |
| theme 初始化 | `app/ai-chat2/src/state/theme.rs` | 初始化 system accent，按 DB settings 和窗口 appearance 应用 theme。 |
| i18n 初始化 | `app/ai-chat2/src/foundation/i18n.rs` | 支持 `en-US` / `zh-CN`，未知语言按 system fallback。 |
| runtime assets | `app/ai-chat2/src/foundation/assets.rs` | 组合 app-local Lucide、provider logo、app icon 和 `gpui-component-assets`；provider brand logo 使用 app-owned runtime SVG asset 和 `ProviderLogoName`，不塞进 Lucide `IconName`。Branded built-in providers 已全覆盖，custom OpenAI-compatible 保留 generic fallback。 |
| global hotkey runtime | `app/ai-chat2/src/state/hotkey.rs` | 注册 temporary hotkey 和 enabled shortcuts，记录 diagnostics；Settings 保存 temporary hotkey 后会同步更新 runtime 注册。 |
| app menu | `app/ai-chat2/src/app/menus.rs` | 已接 `About`、`Open Main`、`Open Temporary Conversation`、真实 `Settings`、`Quit`、Window menu 和 macOS Hide/Show actions。 |
| About window | `app/ai-chat2/src/app/about.rs` | 已实现精简真实 About：app icon、名称、描述、版本、license 和 GitHub 链接。 |
| titlebar menu bar | `app/ai-chat2/src/app/title_bar_menu.rs` | 非 macOS 渲染 app icon + component menu bar；macOS 使用系统菜单。 |
| main window shell | `app/ai-chat2/src/features/home/shell.rs` | 主窗口 UI root 已移出 `app.rs`：titlebar、可调宽 Sidebar、空内容区和 `gpui-component` sheet/dialog/notification layers 都挂在 Home root。 |
| home sidebar component | `app/ai-chat2/src/features/home/sidebar.rs` / `features/home/sidebar/row.rs` | Sidebar 已拆成独立组件；project-first sidebar 第一版已接线。顶部“新对话/搜索”和底部“设置”统一使用 app-local shortcut action row，hover 时显示 `Kbd` shortcut badge，快捷键使用 GPUI `secondary` 语义。 |
| New Conversation 默认页 + 项目选择器 | `app/ai-chat2/src/features/home/new_conversation.rs` | Home 右侧未选择具体对话时显示默认新对话页：中性 AI Chat 标题、现有 `ChatForm` 和仅在该页出现的项目选择器。项目选择器读取 normal projects，支持“不使用项目”，选中项目后写入 `default_project_id` 并刷新 composer skill catalog；添加项目只选择现有文件夹，不提供新建空项目。Composer 使用 opaque input surface 压住下层项目条；项目条按 Codex app 参考使用 neutral muted surface，不使用 secondary/action 色。 |
| dock/app reopen | `app/ai-chat2/src/app.rs` | app reopen 会 show/create main window。 |
| close-hide behavior | `app/ai-chat2/src/app.rs` | macOS/Windows 主窗口关闭时隐藏窗口。 |
| Minimize/Zoom action | `app/ai-chat2/src/app.rs` / `features/temporary.rs` | 主窗口和 Temporary 窗口已处理 Window menu action。 |
| bundle metadata | `app/ai-chat2/Cargo.toml` | 已有 `[package.metadata.bundle]`。 |
| app icon | `app/ai-chat2/build-assets/icon/` | 本轮复用旧 `ai-chat` 图标作为 v1 shell icon。 |
| macOS bundle localization | `app/ai-chat2/locales/macos/` | 已有 `en-US` 和 `zh-Hans` InfoPlist strings。 |
| Windows icon build script | `app/ai-chat2/build.rs` | Windows build 时从 base PNG 派生 multi-frame `.ico`。 |
| `xtask bundle ai-chat2` | `crates/xtask/src/cli.rs` | `BundleApp::AiChat2` 已加入，CLI parse test 已覆盖。 |
| ComposerEditor v1 | `app/ai-chat2/src/components/chat_form/composer_editor.rs` / `composer_editor/*` | 已接入 ChatForm，支持文本输入、IME range、选择/光标、cursor blink/styling、编辑快捷键、plain text 剪贴板、Enter 发送、Shift+Enter 换行、soft wrap、内部滚动、Unicode/grapheme-aware movement/delete/word boundary、`$skill-name` token 和 `ComposerSnapshot`。 |
| Settings shell + General/Appearance/Projects | `app/ai-chat2/src/features/settings.rs` / `settings/{general,appearance,projects,layout}.rs` | 已搬运旧 app 的 Settings shell 体验：titlebar menu、搜索、可调 sidebar、page frame、General language/HTTP proxy/temporary hotkey/config file，Appearance theme mode、主题预览网格、Material You color picker/add/delete；Projects 可列出 normal projects 并通过系统目录选择器添加项目，不显示 scratch/anonymous project。 |
| Temporary Conversation window | `app/ai-chat2/src/app/temporary_window.rs` / `features/temporary.rs` / `state/temporary.rs` | 已实现首版：菜单打开/复用真实窗口；global temporary hotkey 恢复 toggle；窗口外壳已迁移为 popup-like、不可 resize、按鼠标所在 display 定位/移动，失去 activation 时隐藏并进入延迟 remove；macOS 实际 window level 已从 `NSPopUpWindowLevel = 101` 覆盖到 `NSModalPanelWindowLevel = 8`，对齐 launcher 搜索窗层级并修复 IME 候选窗干扰；顶部单行搜索，左侧仅列 visible scratch/no-project active conversations，右侧复用 `ConversationDetailPage` 或无项目 `ChatForm` 新对话；搜索、上下选择、Tab 到 composer、`secondary-n` 到新对话和发送后创建 scratch conversation + agent run 已接线。迁移记录见 `issue-159-ai-chat2-temporary-window.md`。 |
| shared chat/conversation components | `app/ai-chat2/src/components/{chat_form,conversation_detail,picker}.rs` / `foundation/conversation_format.rs` | 已从 Home-only 模块抽到共享层，Home 和 Temporary 都从 `components` / `foundation` 消费；Temporary 不横向依赖 `features/home`。 |

## 占位

| 事项 | 当前位置 | 当前行为 | 后续需要 |
| --- | --- | --- | --- |
| shortcut hotkey action | `app/ai-chat2/src/state/hotkey.rs` | 触发后只记录 diagnostics 和 tracing/log event。 | 按 shortcut 的 prompt/provider/model/input/action 执行 agent run。 |
| ChatForm runtime wiring | `app/ai-chat2/src/components/chat_form.rs` / `chat_form/*` | New Conversation 默认页和 Temporary Window 已接 Codex 风格 composer 外框和真实 `ComposerEditor`；项目选择器在页面层处理，不进入通用 ChatForm。model picker 已读取 fresh DB enabled provider/model cache，reasoning selector 从 `ModelCapabilitiesSnapshot.reasoning.control` 派生，支持 level、boolean、always-on 和 token budget，`SendRequested` 已携带 `ChatFormSubmit`。Agent Conversation Page 和 Temporary 首版已消费 `ChatFormSubmit` 创建/追加 conversation 并启动真实 run；运行中输入仍可编辑，主按钮切换为 stop，点击后只停止当前 conversation 的 active run。 | `+` 仍是 local event；prompt selector、附件、retry/resend 后续继续补。输入内核进度见 `issue-159-ai-chat2-composer-editor.md`。 |
| Project-first sidebar | `app/ai-chat/docs/dev/issue-159-ai-chat2-sidebar.md` | 已完成第一版：顶部新对话/搜索入口、底部设置入口、置顶对话/项目、项目展开、项目更多菜单、conversation search、conversation route、project/conversation soft-delete，以及 shortcut action row 视觉对齐已接线。Agent Conversation Page 首版已接 New Conversation 发送后的 sidebar 即时刷新，并把 scratch project conversation 归入无项目区。 | 后续继续补 last item preview 和更完整的 project metadata/status UI。 |

## 基础设施 / 本地状态 / 可观测性

| 能力 | `ai-chat2` 状态 | 说明 |
| --- | --- | --- |
| app-local config | 已完成 | `config.toml` 只保存启动层本机配置；业务 settings 仍来自 fresh DB。 |
| layout state | 已完成 | `state.toml` 保存 Sidebar 宽度、main/settings window placement，并在 quit 前同步 flush；这是本机 UI state，不是 app settings。 |
| basic tracing subscriber | 已完成 | 已初始化 stdout/stderr pretty layer 和 file-backed layer。 |
| file-backed logging | 已完成 | 旧 app 规范已迁移：macOS 写 `~/Library/Logs/top.sushao.ai-chat2/data.log`，非 macOS 写 local data logs。 |
| open/copy diagnostics | 未开始 | 没有打开日志目录、复制诊断信息、导出 runtime snapshot 或用户可见 diagnostics 面板。 |
| user-visible startup/runtime errors | 未开始 | startup init 错误已从 `app::run` 返回给进程，config parse 错误会记录并重写默认配置；仍没有统一的 UI error surface。 |
| main/settings window placement | 已完成 | 已保存 main/settings window bounds、mode 和 display id，并复用旧 app 离屏/无效 display fallback 语义；About/Temporary 窗口暂不持久化。 |
| hotkey diagnostics UI | 未开始 | runtime 只保留内存 diagnostics；Settings 保存 temporary hotkey 已改为 runtime 注册成功后再持久化，避免 DB 保存未注册 hotkey；Settings 真实页面尚未展示注册失败、最近触发或重注册状态。 |

### Hotkey UI 后续实现注意

- PR #164 review 指出的 `temporary_hotkey` 保存顺序问题已修复：确认 temporary hotkey 时先 parse/register
  runtime，成功后再写 app settings；注册失败不关闭 dialog、不写 DB，DB 保存失败时尝试回滚 runtime。完整
  hotkey diagnostics UI 仍未实现，后续需要展示注册失败、最近触发和重注册状态。迁移记录见
  `issue-159-ai-chat2-temporary-window.md`。

## 后端能力 / UI 接线状态

| 能力 | 后端位置 | UI 状态 |
| --- | --- | --- |
| projects | `ai-chat-db` repositories / fresh schema | Settings 已可列出 normal projects 并添加文件夹项目；New Conversation 默认页已可选择 normal project、添加现有文件夹、支持不使用项目，并按选择持久化或清空 `default_project_id`。Agent Conversation Page 首版已在无项目发送时创建每会话 scratch project，并让其 conversation 归入无项目区。仍缺 open folder、recent projects 和更完整 project metadata/status UI。 |
| conversations | `ai-chat-db` repositories / fresh schema | 已实现首版：New Conversation 创建 conversation + 首条 user item，已有 conversation 可追加 user item；sidebar 已有 conversation route/search/delete 第一版。 |
| canonical timeline | `conversation_items` | 已实现首版：Conversation page 按 snapshot 渲染 GPUI 原生 `ListState` / `list` timeline、显式滚动条、user bubble、agent final markdown/details collapse 和 observer invalidation。多模态/rich output 后续继续补。 |
| attachments | `attachments` + typed payloads | 没有 file/image/audio attach、preview、generated output 或 storage UI。 |
| agent runs | `ai-chat-agent::AgentRuntime` + `agent_runs` | 已实现首版：New Conversation 和 Conversation page 发送会启动真实 `AgentRuntime`，runtime observer 触发页面 reload；active run 期间 ChatForm 主按钮切换为 stop。停止会 cancel token，并在 100ms grace 后强制将仍未结束的 run、active provider step 和 active tool invocation 标为 `Canceled`，同时避免用户取消弹 “runtime canceled”。retry/resend UI 仍留后续。 |
| provider steps | `provider_steps` | Composer 已能选择 DB-backed provider/model 作为 run 输入，Agent Conversation Page 首版已启动真实 run 并由 agent persistence 写入 provider steps；仍没有 provider step debug surface 或 continuation display。 |
| tool invocations | `tool_invocations` + `ToolRegistry` | 已实现首版：v1 compact tool call/result details 从 `conversation_items` 渲染。完整 rich tool UI 留后续。 |
| approvals | `approval_decisions` + agent runtime | 没有 approval prompt、approve/deny/cancel/expired UI。 |
| usage | `usage_events` | 没有 token/usage summary 或 rollup UI。 |
| prompts | `prompts` | Prompt Settings 第一版已实现：`prompts.content_json JSON` 改为 `content TEXT`，`PromptContent` 收敛为简单文本，Settings 新增 Prompts 管理页并提供搜索、管理行、modal 查看、新增、编辑、正文多行编辑、硬删除；Composer prompt selector 和 Shortcut prompt binding 后续继续补。详见 `app/ai-chat/docs/dev/issue-159-ai-chat2-prompt-settings.md`。 |
| providers | `providers` | Provider settings 第一阶段已实现：`app/ai-chat/docs/dev/issue-159-ai-chat2-provider-settings.md`。已接 Settings Provider 页、Provider i18n、未保存 provider 默认 disabled、provider enabled 保存、保存前本地校验、未保存状态标签、GPUI credentials secret write/read、`ListState` provider list、provider list panel/row separator 视觉、provider brand logo / fallback visual，以及 Composer 侧 enabled provider/model 读取。Agent Conversation Page 首版已抽出 provider secret read helper，并按已保存 provider/model dispatch agent runtime。仍缺 manual model editor、manual capability override 和 Rig completion client validation。 |
| provider models | `provider_models` | 已补 per-model enabled DB 合同、Settings 内 model enabled toggle、Provider 双栏独立滚动布局、右侧 detail 整体滚动、`ListState` model list、真实远端模型刷新、保留 enabled 的 fetch upsert、provider-specific capability source/enrichment，以及 Composer model picker 读取/搜索/能力标签/reasoning selector/provider logo。manual model editor 和 manual capability override persistence 仍未完成。 |
| app settings | `app_settings` | General/Appearance 已消费 language、theme、temporary hotkey 和 HTTP proxy；New Conversation 默认页已消费并更新/清空 default project；Provider settings 第一阶段已落地并有专项文档继续跟踪；Prompt Settings 第一版已接入；Shortcut settings 仍未接。 |
| file-backed skills | `ai-chat-agent::skills` | Composer 已读取 `SkillCatalog` 并在 snapshot 输出 skill activation request；没有 skill catalog UI、activation display 或 skill snapshot timeline UI。 |
| MCP helpers | `ai-chat-agent::mcp` | 没有 MCP config UI、connected server status 或 MCP tool approval UI。 |

## 未开始 UI 清单

| 区域 | 事项 |
| --- | --- |
| Project navigation | New Conversation 默认页已有 default/no-project selector；project-first sidebar 第一版已实现项目列表、置顶、展开、菜单、显示目录、重命名和移除；无项目发送时每会话 scratch project 已实现首版。仍缺 recent projects 和更完整的 project metadata/status UI。 |
| Conversation navigation | conversation list、new conversation 入口、delete、search/filter 和右侧 conversation route 已实现第一版；New Conversation 发送后的 conversation create/open 和已有 conversation send 已实现首版；仍缺 title edit、status display、last item preview。 |
| Composer | 已有 Home 右侧视觉外框和 `ComposerEditor` 第一版输入内核，已补 cursor、scroll 和 Unicode/grapheme-aware 编辑；provider/model data source 已接 fresh DB enabled cache，reasoning selector 已从 provider model capability 派生，model picker row/trigger 已接 provider logo visual。Agent Conversation Page 首版已接 conversation create/send/run，并已补 stop generation；真实工作仍包括 prompt selector、多 part input、capability warning、附件、retry、resend 和 `$` completion UI。输入内核专项清单见 `issue-159-ai-chat2-composer-editor.md`。 |
| Timeline text | 已实现首版：user bubble、assistant final markdown、copy hover、Codex-style timestamp 和 GPUI 原生 `ListState` / `list` 虚拟列表；streaming delta、multi-block rich assistant output 和 export affordance 后续继续补。 |
| Reasoning | 已实现首版：agent details 默认展开/收起规则和 markdown/text details block；multiple reasoning blocks、provider-specific gating 的完整体验后续继续补。 |
| Tools | 已实现首版：local/MCP/provider-hosted tool call/result 的 v1 compact details；progress、structured output rich view、attachment result 和 tool name collision display 后续继续补。 |
| Approvals | approval request card、approve/deny/cancel actions、pending/expired/decided states、recovery after restart。 |
| Status and errors | queued/running/waiting/completed/failed/canceled 状态、retry affordance、user-visible error item。 |
| Usage | per-run usage summary、provider/model token counts、usage event rollup display。 |
| Attachments and multimodal | image/file/audio input、generated files/images、preview/download/open, provider unsupported-state messaging。 |
| Settings | General/Appearance/Projects 已实现；Provider settings 第一阶段已实现并补齐 i18n/default disabled/滚动布局/save validation/secret credentials/model fetch/ListState lists/provider list panel polish/provider brand visual；Prompt Settings 第一版已实现，使用简单文本 `content TEXT`、管理页行列表、modal 查看/编辑、正文多行编辑和硬删除；Composer 空状态可 deep-link 到 Provider settings；仍缺 manual provider model editor、Rig completion validation、shortcuts 和 tool/MCP policy。 |
| Shortcuts | shortcut CRUD、input source selection、prompt/provider/model binding、capability validation、registration/runtime status。 |
| Temporary chat | Temporary Conversation Window 首版已完成；仍缺 selected text/screenshot input、save/promote to conversation。 |
| Screenshot/input capture | screenshot overlay、OCR fallback、image-capable model data URL path、unsupported model warnings。 |
| Legacy access | read-only legacy viewer、manual export/import 或 backup-only policy；当前没有任何 legacy data UI。 |
| Export/import | fresh conversation export、generated output export、legacy manual import/export。 |
| Capability gating | tool calling、MCP、image/file/audio input、image generation、structured output、reasoning、provider-specific extensions。 |
| Provider branding | 已完成第一版：Lucide v1 移除品牌图标后，`ai-chat2` 使用 app-owned runtime SVG、`ProviderLogoName` 和 `ProviderVisual` fallback；来源策略是 Simple Icons first，Simple Icons 缺失、明显过期或品牌 guideline 要求时用官方 SVG override；没有官方紧凑 SVG 或官方下载不可用时，允许使用可追溯第三方 SVG。Settings provider row/header 与 ChatForm model row/trigger 已优先显示品牌 logo。Branded built-in providers 已全覆盖；custom OpenAI-compatible 不是固定品牌，继续使用 `Server` fallback。 |

## 旧 `app/ai-chat` 对比

| 旧 `app/ai-chat` 能力 | 旧实现位置 | `ai-chat2` 状态 | 迁移说明 |
| --- | --- | --- | --- |
| app bootstrap / tracing | `app.rs` | 已替代 | 启动流程和 file-backed logging 已由 `ai-chat2` app shell 承接；startup init 错误会返回给进程。 |
| app menu | `app/menus.rs` | 已完成 | 已按 shell 需要迁移；未接旧 temporary 业务。 |
| tray/status item | `app/tray.rs` | 暂不做 | 不在当前 #159 UI 壳范围；如需要应单独定义 scope。 |
| titlebar menu | `components/title_bar_menu.rs` | 已完成 | 已复制为 app-local titlebar menu bar。 |
| bundle/app icon | `build-assets` / `build.rs` / `xtask` | 已完成 | 图标暂时复用旧 `ai-chat`，后续可替换独立 app 图标。 |
| About window | `features/about.rs` | 已完成 | `ai-chat2` 已实现精简真实 About，不照搬旧 tray/about 文案。 |
| Settings window | `features/settings.rs` | 已完成 | 已迁移真实 Settings shell；当前注册 General/Appearance/Provider/Projects/Prompts。 |
| Provider settings | `features/settings/provider.rs` + `features/settings/provider/*.rs` | 第一阶段已完成 | 已接 fresh `providers` / `provider_models` 基础 UI、Provider i18n、默认 disabled、独立滚动布局、provider save 前本地校验、未保存状态标签、GPUI credentials secret write/read、真实 model fetch、model enabled toggle、`ListState` provider/model lists、provider list panel/row separator、右侧 detail 整体滚动和 provider brand visual；provider model refresh 已写入 capability source 和 reasoning control；仍需 manual model editor、manual capability override 和 Rig completion client validation。 |
| Appearance settings | `features/settings/appearance_settings.rs` | 已完成 | 已迁移 theme mode、light/dark theme preview grid、Material You color picker/add/delete 和默认 Material You theme visibility。 |
| General settings | `features/settings/general_settings.rs` | 已完成 | 已迁移 language、HTTP proxy、temporary hotkey 专用输入和 open config file；default project picker 不在本轮。 |
| Template settings | `features/settings/template_settings.rs` | 不照搬 | 新模型删除 templates，改为 prompts。 |
| Prompt settings | `features/settings/prompts.rs` + `features/settings/prompts/*.rs` | 第一版已完成 | 已新增 Settings Prompts 管理页，使用搜索输入、全宽管理行、显式 View/Edit/Delete 操作、查看弹窗、编辑弹窗、正文多行编辑和现有 destructive confirm；prompt 内容模型已改为 `PromptContent { text }` + `prompts.content TEXT NOT NULL`，不支持复杂 role/content-parts prompt。详见 `issue-159-ai-chat2-prompt-settings.md`。 |
| Shortcut settings | `features/settings/shortcut_settings.rs` | 未开始 | 新 shortcuts 不绑定 template/mode，应绑定 prompt/provider/model/input/action。 |
| shortcut form/list/status dialogs | `features/settings/shortcut_settings/{form,list,dialogs,validation}.rs` | 未开始 | 需要按 fresh shortcut schema 重做 CRUD、状态弹窗、冲突/注册失败/capability mismatch 和重注册 UI。 |
| reusable hotkey input | `components/hotkey_input.rs` | 已完成 | 已搬运 app-local `HotkeyInput` 并用于 General temporary hotkey；完整 shortcut 冲突/状态 UI 仍属于 Shortcuts settings。 |
| provider ext settings help | `components/ext_setting_help.rs` | 未开始 | provider-specific settings 应由 fresh provider/model config 和 capability UI 重新表达。 |
| home shell | `features/home.rs` / `features/home/shell.rs` | 已完成 | `ai-chat2` 已有最小主页骨架：可调宽 Sidebar、空内容区、titlebar 和 gpui-component overlay layers；project/conversation 业务未接。 |
| workspace sidebar width | `state/workspace.rs` / `state/workspace/persistence.rs` | 已替代 | 旧 workspace state 的 sidebar width 已由 `ai-chat2` 本机 `state.toml` layout state 承接。 |
| main/settings window placement | `state/workspace.rs` / `state/workspace/persistence.rs` | 已替代 | `ai-chat2` 本机 `state.toml` 已保存 main/settings bounds、mode 和 display id，并复用旧离屏 fallback 语义。 |
| workspace tabs and drafts | `state/workspace.rs` / `state/workspace/tabs.rs` | 不照搬 | 旧 tab/draft/open-folder workspace shape 不适合 project-first fresh timeline，需要重新设计。 |
| folder sidebar | `features/home/sidebar.rs` | 不照搬 | 新模型没有 folders；改为 project-first navigation。 |
| add folder dialog | `components/add_folder.rs` | 不照搬 | 新模型没有 folders，不迁移旧 add folder flow。 |
| add conversation dialog | `components/add_conversation.rs` | 未开始 | 新 conversation create flow 应围绕 project、prompt、provider/model 和 canonical timeline 重新设计。 |
| tabs/conversation panel | `features/home/tabs.rs` | 未开始 | 新 UI 需要按 fresh conversation/timeline 重写。 |
| conversation list/search | `features/home/search.rs` / `search_list.rs` | 未开始 | 需要基于 fresh conversations 和 `conversation_items.search_text` 后续设计。 |
| delete confirmation | `components/delete_confirm.rs` | 未开始 | project/conversation/prompt/provider/shortcut 等 destructive actions 需要新的确认策略。 |
| conversation export | `features/home/export.rs` | 未开始 | 新 export 应读取 canonical `conversation_items`。 |
| chat form | `components/chat_form.rs` | 已替代 | `ai-chat2` 已有 ChatForm 视觉外框、真实 `ComposerEditor` 第一版输入内核、DB-backed provider/model picker 和 provider-neutral reasoning selector，并已补 cursor、scroll 和 Unicode/grapheme-aware 编辑；Agent Conversation Page 首版已接 conversation create/send 和 agent run，且运行中主按钮已切换为 stop；仍不接 prompt selector、attachments、retry/resend；新 composer 不应暴露 conversation mode/template controls；真实输入进度见 `issue-159-ai-chat2-composer-editor.md`。 |
| chat form provider ext settings | `components/chat_form/ext_settings.rs` | 已有专项计划 | Provider settings 专项计划已固定 capability cache 和 typed extension 的来源；composer 接线仍未实现。 |
| mode select | `components/chat_form/mode_select.rs` | 不照搬 | 新模型所有 conversation 都 contextual。 |
| template picker | `components/chat_form/template_picker.rs` | 不照搬 | 新 UI 使用 prompt selector。 |
| model select | `components/chat_form/model_select.rs` | 已完成 | `ai-chat2` model picker 已接 fresh `providers` / `provider_models` cache、enabled filtering、provider 分组、capability tags、search 和 reasoning selection derivation；发送事件先携带 snapshot，真实 conversation/run 接线留给后续。 |
| message rendering | `components/message.rs` | 未开始 | 新 timeline 要覆盖 reasoning/tool/approval/status/usage/attachments。 |
| temporary chat | `features/temporary.rs` / `app/temporary_window.rs` | 已替代 legacy 首版 | 真实窗口、no-project 历史、搜索、new/detail 和 agent run 已接线；selected text/screenshot input、save/promote to normal conversation 仍未接。 |
| temporary window runtime | `features/temporary.rs` / `state/hotkey.rs` | 部分实现 | 已恢复旧版 global temporary hotkey toggle、popup-like window 外壳、鼠标所在显示器定位/移动、失去 activation 后隐藏/延迟 remove、macOS 前台 app restore 和非 resizable 语义；默认尺寸保留 `960x620`。tray、shortcut execution、selected text/screenshot input 和 save/promote flow 仍未实现。迁移记录见 `issue-159-ai-chat2-temporary-window.md`。 |
| hotkey backend/registry | `features/hotkey/backend.rs` / `registry.rs` | 部分实现 | `ai-chat2` 已有初始注册、temporary hotkey 设置后重注册、内存 diagnostics 和 temporary window dispatch；快捷键状态 UI 与 shortcut 执行仍未实现。 |
| shortcut execution flow | `features/hotkey/shortcut_flow.rs` | 未开始 | 未执行 selected text、clipboard fallback、screenshot input、prompt/provider/model/action 和通知状态。 |
| screenshot/OCR shortcut | `features/screenshot.rs` / `features/screenshot/overlay.rs` | 未开始 | 新快捷键只记录 diagnostics，不执行 screenshot overlay、OCR fallback 或 image input。 |
| platform capture/display helpers | `platform/{capture,display,gpui_ext}.rs` | 未开始 | screenshot、display targeting 和 GPUI platform helpers 需要按 `ai-chat2` scope 决定复用或重写。 |
| foundation capabilities helper | `foundation/capabilities.rs` | 已替代 | capability model 已进入 shared crates，UI 后续读取 typed capability model。 |
| foundation search helper | `foundation/search.rs` | 未开始 | pinyin/search helper 是否需要迁移取决于 fresh project/conversation search UI。 |
| legacy LLM provider adapters | `llm/provider/*` | 已替代 | provider execution 不应复制旧 `llm` adapters，应走 `ai-chat-agent` 和 provider step 观测边界。 |
| legacy LLM runner/preset/run persistence | `llm/{runner,preset,run_persistence}.rs` | 已替代 | runner 和 run persistence 由 `ai-chat-agent` / fresh execution tables / `conversation_items` 承接，不复制旧结构。 |
| legacy Diesel schema/model/service | `database/{schema,model,service,migrations}.rs` | 已替代 | fresh persistence 已拆到 `ai-chat-db`；旧 v1-v6 store 保持 intact，不作为 `ai-chat2` source of truth。 |
| legacy chat tree/runtime/models | `state/chat/*` | 不照搬 | 旧 folder tree、conversation runtime cache 和 legacy message state 不适合 fresh project-first timeline。 |
| legacy database UI | legacy `database` / `state` | 未开始 | 旧 store 保持 intact；没有 read-only viewer、backup-only display 或 manual import/export UI。 |

## 不应照搬的旧概念

- `folders`：新模型项目优先，不再使用 folder sidebar 作为 conversation grouping truth。
- `conversation_templates`：新模型使用 prompts；template compatibility UI 不能原样迁移。
- `assistant-only` / `single` / `contextual` modes：新模型所有 conversations 都 contextual。
- legacy `messages.content` / `messages.send_content` / `input_content_parts`：新 UI 不能以这些 legacy tables 作为 agent timeline 真相。
- provider request JSON as history：provider request snapshot 只用于 debug/replay，不能作为聊天历史或 UI 渲染源。
- legacy Diesel model/service/migration 层：fresh persistence 属于 `ai-chat-db`，不能在 `app/ai-chat2` 里复制旧 store ownership。
- legacy `llm` provider runner、preset 和 run persistence：执行层属于 `ai-chat-agent`，UI 只消费 fresh transcript 和 runtime 状态。
- legacy workspace folder tree、tabs、drafts 和 open-folder 状态：需要按 project-first navigation 和 canonical timeline 重做。
- legacy search/filter shape：旧 search helper 和 folder/conversation list shape 不能决定 fresh project/conversation search 的数据模型。

## 验证记录

最近完成 shell 工作时已运行：

- `cargo fmt`
- `cargo test -p ai-chat2`
- `cargo check -p ai-chat2`
- `cargo test -p xtask`
- `cargo run -p xtask -- bundle ai-chat2`
- `git diff --check`
- bundle GUI smoke：打开 `target/release/bundle/macos/AI Chat 2.app`，验证主窗口、macOS 菜单、Settings/About/Temporary 窗口和 Open Main。

About 页面实现后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 about`
- `cargo check -p ai-chat2`
- `git diff --check`
- `cargo run -p xtask -- bundle ai-chat2`
- bundle GUI smoke：打开 `target/release/bundle/macos/AI Chat 2.app`，验证 About 真实窗口、重复打开复用已有窗口、GitHub 按钮触发 URL 打开、Settings/Temporary 窗口仍可打开。

主页 Sidebar 骨架实现后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 sidebar`
- `cargo test -p ai-chat2 layout`
- `cargo check -p ai-chat2`
- `git diff --check`
- `cargo run -p xtask -- bundle ai-chat2`
- bundle GUI smoke：打开 `target/release/bundle/macos/AI Chat 2.app`，确认主窗口左侧 Sidebar、底部仅 Settings item、点击 Settings 打开占位窗口、拖拽分隔条写入 `state.toml`，重启后按保存宽度恢复。

Home root / Sidebar 结构修正后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 sidebar`
- `cargo test -p ai-chat2 home`
- `cargo check -p ai-chat2`
- `git diff --check`
- `cargo run -p xtask -- bundle ai-chat2`
- bundle GUI smoke：打开 `target/release/bundle/macos/AI Chat 2.app`，确认 Home root 正常显示、Sidebar Settings item 仍打开 Settings 占位窗口、重复打开 app 后主窗口数量仍为 1。`actool` 的 Liquid Glass 图标注入警告不阻塞 bundle，保留普通图标。

ChatForm 视觉预览实现后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 chat_form`
- `cargo check -p ai-chat2`
- `git diff --check`
- 未做 UI 截图或手动验证；本轮按用户要求只做代码级验证，由本地手动查看效果。

ComposerEditor 第一版实现后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 composer_editor`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2`
- `cargo check -p ai-chat2`
- `cargo clippy -p ai-chat2 --all-targets --all-features -- -D warnings`
- `git diff --check`
- 未做手动 macOS 中文输入法、候选框、双击/拖拽或 bundle GUI 验证。

Settings shell + General/Appearance 以及 5 个 parity 修复后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 settings`
- `cargo test -p ai-chat2 theme`
- `cargo test -p ai-chat2 hotkey`
- `cargo test -p ai-chat2 layout`
- `cargo test -p ai-chat2 config`
- `cargo test -p ai-chat2 chat_form`
- `cargo check -p ai-chat2`
- `git diff --check`
- 未做手动 resize/move/reopen、Settings 交互或 bundle GUI 验证。

Settings Projects 页面后已运行：

- `cargo fmt`
- `cargo test -p ai-chat-db project`
- `cargo test -p ai-chat2 projects`
- `cargo test -p ai-chat2 settings`
- `cargo test -p ai-chat2 assets`
- `cargo check -p ai-chat2`
- `git diff --check`
- 未做手动 Settings Projects 添加文件夹或 bundle GUI 验证。

New Conversation 默认页、no-project 项目选择器和视觉修正后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 home`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 projects`
- `cargo test -p ai-chat2 settings`
- `cargo test -p ai-chat2 assets`
- `cargo check -p ai-chat2`
- `git diff --check`
- 未做手动添加文件夹、重复项目选择、重开窗口或 bundle GUI 验证。
- 此前额外尝试 `cargo clippy -p ai-chat2 --all-targets --all-features -- -D warnings`，但当前分支已有未处理 clippy lint：`composer_editor/element.rs` 和 `settings/appearance.rs` 的 `too_many_arguments`，以及 `state/hotkey.rs` 的 `collapsible_if`；本轮未修改这些无关代码。

Codex-style project tray 颜色/层级 polish 后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 home`
- `cargo check -p ai-chat2`
- `git diff --check`
- 未做手动 GUI 截图验证。

2026-05-31 状态同步记录：

- live GitHub 状态：#159 仍 open；PR 列表中没有 `codex/issue-159-ai-chat2-ui` 对应 PR。
- 远程分支状态：本地 `codex/issue-159-ai-chat2-ui` 与 `origin/codex/issue-159-ai-chat2-ui`
  一致，领先 `origin/codex/issue-137-llm-abstractions` 10 个提交。
- 5/29 文档后新增提交：`34ccb6f` cursor styling、`26a89fa` composer scrolling、`09b2f22`
  Unicode-aware composer editing。
- 本次只同步文档状态；未运行 Rust tests。

2026-06-01 状态同步记录：

- live GitHub 状态：#137、#155-#159 仍 open；PR 列表中没有 `codex/issue-159-ai-chat2-ui`
  对应 PR。
- 远程分支状态：本轮推送后 `codex/issue-159-ai-chat2-ui` 领先
  `origin/codex/issue-137-llm-abstractions` 14 个提交。
- 5/31 文档后新增提交：`ed59682` Settings shell + General/Appearance、`57bb3d5`
  basic parity fixes（main/settings window placement、composer focus、quit flush、config tolerance、
  default Material You visibility）、`edb4a3d` Settings Projects 列表/添加项目，以及本次 New
  Conversation 默认页、no-project 项目选择器和 Codex-style project tray polish。
- 当时仍不接真实 project/conversation navigation、prompt/provider/model data source、agent run/timeline、
  `$` completion UI、Shortcuts settings 或 Temporary Conversation runtime。
- 本次包含 New Conversation 默认页、项目 selector 和视觉 polish；验证见上方记录。

2026-06-01 Provider settings 专项计划记录：

- 新增 `app/ai-chat/docs/dev/issue-159-ai-chat2-provider-settings.md`，固定 Rig-first provider
  scope、Alma/Zed 参考边界、DB/secret/Rig 对齐、模型刷新和 capability cache 计划。
- 后续实现已开始：`ai-chat-db` repository API 与 `provider_models.enabled` 已落地，`ai-chat2`
  已新增 Settings Provider 页骨架、registry/draft/capability 模块和 DB-backed enabled model helper。
  截至该记录，当时真实 GPUI keychain、Rig client factory、远端 model fetcher、manual model editor
  和 DB-backed composer model picker 仍未完成；2026-06-02 记录已补充 GPUI credentials、真实 model
  fetch、`gpui_tokio` 和 `ListState` 进展。
- 追加补强：专项计划已扩展到 implementation-ready 粒度，固定 `provider.rs + provider/*.rs`
  模块结构、`gpui-component` 组件清单、app-local entity 结构体、`provider_models.enabled`
  schema/API 合同和 UI 状态流。

2026-06-02 Provider settings 修复记录：

- Provider settings 可见文案已接入 `I18n`，Provider settings page title 使用 `settings-page-provider`，
  provider 品牌名保持原文。
- 未保存 built-in provider draft 默认 disabled，已保存 provider 继续使用 DB row 的 enabled 状态。
- Settings frame 为 Provider 页新增 no-outer-scroll 模式，Provider 左侧 list 和右侧 detail 使用独立滚动条。
- 验证：`cargo fmt`、`cargo test -p ai-chat2 provider`、`cargo check -p ai-chat2`、`git diff --check`。

2026-06-02 Provider settings model fetch / ListState 记录：

- 提交 `4d4110b feat(ai-chat2): wire provider settings model fetch` 已推送到
  `origin/codex/issue-159-ai-chat2-ui`。
- live GitHub 状态：#159 仍 open；`codex/issue-159-ai-chat2-ui` 当前没有 PR。
- 保存路径已整理为当前 UI input -> 本地校验 -> 写 GPUI credentials -> insert/update provider；
  已保存 provider 修改后显示“未保存”，保存成功后刷新 snapshot。
- 模型刷新已接真实链路：Settings 读取 DB provider row 和 GPUI credentials，通过
  `gpui_tokio::Tokio::spawn` 调用 `ai_chat_agent::fetch_provider_models`，成功后
  `replace_fetched_provider_models` 写入 DB 并保留已有 model enabled 状态。
- 新增 `crates/gpui-tokio`，为 GPUI async 中运行 Rig/reqwest/Tokio I/O 提供 repo-local runtime bridge。
- Provider/model list 已迁到 `gpui-component::ListState` 内置搜索。Provider 选择收敛为
  `ListEvent::Select/Confirm -> ProviderSettingsPage` 的单向业务状态流；delegate 不再直接更新页面。
- no-listing provider 当前返回 manual-model-required notification；manual model editor、manual capability
  override persistence 和真实 agent runtime 接线仍未完成；Composer DB-backed model picker 已固定专项计划但尚未实现。
- 验证：`cargo fmt`、`cargo test -p ai-chat2 provider`、`cargo check -p ai-chat2`、`git diff --check`。

2026-06-02 Provider settings list/scroll polish 记录：

- 右侧 Provider detail 改为固定 header + Configuration/Models 共用 detail scroll viewport；避免外层
  Settings body scroll、右侧 detail scroll 和 model list scroll 互相叠加。
- Model enabled switch 改为 model `ListState` 发 `ListEvent::Confirm`，`ProviderSettingsPage` 从当前
  filtered rows 读取目标 model 后即时保存，避免 delegate 直接弱引用页面状态。
- 左侧 provider list 改为和 model list / chat-form picker 一致的整体 panel：List 内置搜索框和 rows
  同处一个 panel；provider row 去掉单独 border/gap，用行间 separator 区分，选中态只来自 ListState。
- 验证：`cargo fmt`、`cargo test -p ai-chat2 provider`、`cargo check -p ai-chat2`、`git diff --check`。

2026-06-02 Composer DB-backed model picker 专项计划记录：

- 新增 `app/ai-chat/docs/dev/issue-159-ai-chat2-composer-model-picker.md`，固定 ChatForm 从
  preview-only model picker 迁到 fresh DB provider/model cache 的实施计划。
- 计划明确数据流：Provider Settings 写入 `providers` / `provider_models`，Composer 只通过
  `state::providers::enabled_provider_models(cx)` 读取 `provider.enabled && model.enabled` 的模型。
- 本阶段不扩 `AppSettingsPayload`、不读取 keychain、不创建 conversation、不调用 `ai-chat-agent`；
  选择结果先作为 New Conversation 页面内存状态和后续 run 输入合同。
- 计划固定 `ProviderModelKey`、`ChatFormSubmit`、`ModelOption`、reasoning selection capability derivation、
  `gpui-component::ListState` picker、app-local `IconName` 使用和验证命令。

2026-06-03 Composer DB-backed model picker 实现记录：

- `ChatForm` 已从 preview model 切到 `state::providers::enabled_provider_models(cx)`，初始化读取一次，
  打开 model picker 前刷新一次；无可用模型或 DB load error 时 send disabled。
- `ProviderModelKey` 使用 provider_id + model_id 作为稳定选择 key，`ChatFormEvent::SendRequested`
  改为携带 `ChatFormSubmit`，包含 composer snapshot、provider/model snapshot 和 reasoning selection。
- model picker 已按 provider 分组，row 使用 provider logo/fallback visual、model display name、provider + raw model id
  副标题、最多 3 个 capability `Tag`；search 覆盖 provider/model/capability tokens。
- reasoning picker 从 `ModelCapabilitiesSnapshot.reasoning.control` 派生，空能力时 disabled。
- 空模型 footer 使用 `Settings` icon 打开 Settings Provider 页；`preview_models.rs` 已删除。
- 验证：`cargo fmt`、`cargo test -p ai-chat2 chat_form`、`cargo test -p ai-chat2 provider`、
  `cargo test -p ai-chat2 settings`、`cargo check -p ai-chat2`、`git diff --check`。

2026-06-03 Provider model capability source 和 reasoning control 实现记录：

- 新增 `app/ai-chat/docs/dev/model-reasoning-capabilities.md`，固定 provider reasoning 能力来源原则：
  不把所有 provider 压成 OpenAI-style `low / medium / high`，并区分 API-discovered、
  official-doc-derived、heuristic、manual 和 OpenRouter-normalized 来源。
- `ai-chat-core` 新增 `CapabilitySourceSnapshot`、`ReasoningControlSnapshot`、
  `ReasoningSelectionSnapshot` 和 `RunSettingsSnapshot.reasoning_selection`；旧 reasoning payload
  仍能按 legacy source 反序列化。
- `ai-chat-agent` 已为 Ollama/Gemini/OpenRouter model fetch 写入 API-discovered capability enrichment，
  并为 OpenAI/Anthropic/DeepSeek/Mistral 写入 docs-derived reasoning profile。`AgentRuntime`
  已能把 `ReasoningSelectionSnapshot` 合并进 provider-specific additional params。
- `ai-chat2` Composer reasoning picker 已从 `ReasoningControl` 派生 options/default，支持 level、
  boolean、always-on 和 token budget numeric input。
- 验证：`cargo fmt`、`cargo test -p ai-chat-core reasoning`、
  `cargo test -p ai-chat-agent provider_models`、`cargo test -p ai-chat-agent model_capabilities`、
  `cargo test -p ai-chat-agent reasoning_params`、`cargo test -p ai-chat2 chat_form`、
  `cargo test -p ai-chat2 provider`、`cargo test -p ai-chat2 settings`、`cargo test -p ai-chat-db`、
  `cargo check -p ai-chat2`、`git diff --check`。

文档-only 更新只需运行 `git diff --check`。

2026-06-03 Provider brand assets / app-assets macro 实现记录：

- 新增 `crates/app-assets-macros`，`crates/app-assets` 保持运行时轻量并 re-export
  `define_lucide_icons!` / `define_svg_icons!`；旧 Lucide 宏调用形状继续可用，自定义 SVG 通过
  `#[svg("provider-icons/name.svg", source = "simple-icons", slug = "...")]` 生成 enum、
  `IconNamed`、metadata 和 `AssetSource`。
- `app/ai-chat2` 新增 `ProviderLogoName`、`ProviderLogoAssets`、`ProviderVisual` 和
  `provider_visual_for_kind`。Provider logo 与 Lucide UI pictogram 分离；branded built-in providers
  已全覆盖，custom OpenAI-compatible 继续走 `Server` fallback。
- 来源策略是 Simple Icons first；Simple Icons 缺失、明显过期或品牌 guideline 要求时使用官方 SVG
  override。已 vendor Simple Icons 来源的 Anthropic、Google Gemini、Ollama、OpenRouter、DeepSeek、
  Moonshot AI、Mistral AI 和 Perplexity 单色 SVG 到 `app/ai-chat2/assets/provider-icons/`；
  新增 OpenAI（theSVG OpenAI）、Azure OpenAI（theSVG Azure OpenAI）、Groq（theSVG Groq，提取前景
  mark 后单色化）、xAI（theSVG xAI/Grok；xAI 官方 brand package 当前环境下载返回 403）、Together
  （Together AI 官方 brand package）和 Z.AI（Wikimedia SVG，记录来源为 `chat.z.ai`；提取 Z mark
  后单色化）。所有 SVG 以 repo-vendored 文件为准，不使用 CDN 或运行时联网。
- 新增 SVG 来源表：

  | Provider | 文件 | 来源类型 | 来源 URL | 说明 |
  | --- | --- | --- | --- | --- |
  | OpenAI | `provider-icons/openai.svg` | 第三方 theSVG | https://thesvg.org/icon/openai | compact mark，已单色化为 `currentColor` |
  | Azure OpenAI | `provider-icons/azure-openai.svg` | 第三方 theSVG | https://thesvg.org/icon/azure-azure-openai | 保留 Azure 渐变色 |
  | xAI | `provider-icons/xai.svg` | 第三方 theSVG fallback | https://thesvg.org/icon/xai-grok | 官方 https://x.ai/legal/brand-guidelines 下载包当前环境返回 403 |
  | Groq | `provider-icons/groq.svg` | 第三方 theSVG | https://thesvg.org/icon/groq | 提取前景 G-shaped mark，单色 `currentColor` |
  | Together | `provider-icons/together.svg` | 官方 brand package | https://www.together.ai/brand | 保留官方多色 mark |
  | Z.AI | `provider-icons/zai.svg` | Wikimedia | https://commons.wikimedia.org/wiki/File:Z.ai_(company_logo).svg | Wikimedia 记录来源为 `chat.z.ai`，已提取 Z mark 并单色化 |

- Settings Provider list/header 与 ChatForm model picker row/trigger 已优先渲染 provider logo，
  没有 logo 时使用 generic Lucide fallback。
- 渲染修正：`gpui_component::Icon` 会把 SVG 按 text color 语义渲染；Groq/Z.AI 的原始反白底图会在
  provider list 中塌成实心方块。当前 vendored SVG 只保留前景 mark 并使用 `currentColor`，资产测试
  覆盖这两个 provider 不再引入反白背景。
- 验证：`cargo test -p app-assets`、`cargo fmt`、`cargo test -p ai-chat2 assets`、
  `cargo test -p ai-chat2 provider`、`cargo test -p ai-chat2 chat_form`、
  `cargo check -p ai-chat2`、`git diff --check`。

2026-06-04 foundation PR 记录：

- live GitHub 状态：#137 和 #159 仍 open；`codex/issue-159-ai-chat2-ui` 已创建 PR #164 指向
  `codex/issue-137-llm-abstractions`，尚未合入。
- 本 PR 聚焦不依赖真实 agent runtime 的 foundation：`ai-chat2` app shell、Settings、Projects、
  Provider/model cache、Composer model/reasoning controls、provider brand assets、project-first sidebar
  和相关 support crate/API。
- 本轮补充 clippy 修正提交 `4eb9e5e`，清理 `ai-chat2` / `ai-chat-agent` 当前 scoped clippy lint。
- 验证：`cargo fmt --check`、`cargo fmt`、`cargo check -p ai-chat2`、`cargo build -p ai-chat2`、
  `cargo test -p ai-chat-agent -p ai-chat-core -p ai-chat-db`、
  `cargo clippy -p ai-chat2 -p ai-chat-agent -p ai-chat-core -p ai-chat-db --all-targets --all-features -- -D warnings`、
  `git diff --check`。
- 未运行 full workspace validation 或手动 GPUI UI 验证。

2026-06-05 live 状态同步：

- GitHub #137、#155-#159 仍 open。
- PR #164 `feat(ai-chat2): add non-agent foundation` 已于 2026-06-05 02:40:06 UTC / 10:40:06
  Asia/Shanghai 合入 `codex/issue-137-llm-abstractions`，merge commit 为
  `738df0b68b0c927a65a084c028d0a7de4dc71dce`。
- #159 仍是当前 UI/timeline 主线。foundation 已进入集成分支；下一步应优先接真实
  conversation create/send runtime、agent run/retry/resend controls 和 canonical timeline 渲染。

2026-06-05 Agent Conversation Page 专项计划记录：

- 新增 `app/ai-chat/docs/dev/issue-159-ai-chat2-agent-conversation-page.md`，固定 New Conversation
  发送后立即创建 conversation、无项目时每会话创建匿名 scratch project、sidebar 即时刷新、右侧打开
  conversation page、启动真实 `AgentRuntime`、runtime observer invalidation、GPUI 原生 timeline
  计划、user bubble、agent final markdown/details collapse、hover copy/time、i18n、icon、Cargo feature
  和测试计划。
- 本次仅新增/更新开发文档，未实现产品代码；`app/ai-chat2` 仍未接真实 conversation create/send
  runtime 或 timeline 渲染。

2026-06-06 Agent Conversation Page 首版实现记录：

- live GitHub 状态：#137、#155-#159 仍 open；当前 `codex/issue-159-ai-chat2-ui` 暂无 PR。
- 远程分支状态：`origin/codex/issue-159-ai-chat2-ui` head 为 `dba4f7c`
  `Implement ai-chat2 agent conversation page`；相对 `origin/codex/issue-137-llm-abstractions`
  的 `git rev-list --left-right --count` 为 `1 1`，说明当前增量尚未进入集成分支。
- 本轮实现 New Conversation 发送后创建 conversation/user item、无项目 scratch project、sidebar 即时刷新、
  conversation page、已有 conversation 继续发送、真实 `AgentRuntime` 启动、runtime observer 刷新、
  GPUI 原生 `ListState` / `list` timeline 和显式滚动条、user bubble、agent final markdown/details collapse、hover copy/time、
  Codex-style timestamp、复制成功 `Check` 两秒和失败通知。
- 2026-06-06 记录时，stop/cancel 与 retry/resend、prompt selector、attachments/multimodal input、approval action、
  rich tool UI、Temporary Conversation runtime、last item preview 和完整 project status UI 尚未完成；2026-06-11
  已补 stop/cancel 和 Temporary Conversation Window 首版，剩余 retry/resend 等后续继续推进。
- 验证：`cargo fmt`、`cargo check -p ai-chat2`、`cargo test -p ai-chat2 timestamp_label`、
  `cargo test -p ai-chat-agent -p ai-chat-core -p ai-chat-db`、`git diff --check`。

2026-06-08 sidebar 状态列化记录：

- 本轮把 `projects.pinned`、`projects.removed`、`conversations.pinned` 提升为 fresh DB columns，
  `ProjectMetadata` / `ConversationMetadata` 不再保存这些 sidebar UI 状态。
- `ai-chat-db` repository 新增直接读写列的 pin/remove API，并让 sidebar 可见性查询在 SQL 层过滤
  removed projects。
- `app/ai-chat2` 的 ProjectCatalogStore、WorkspaceStore、Settings Projects 和 conversation pin/delete
  流程已切换为读取 `ProjectRecord` / `ConversationRecord` 的列化状态。
- fresh DB 尚未合入 `main`，本轮按 pre-main baseline schema 清理处理，不承诺旧开发期 fresh DB 的
  migration 兼容；需要保留本地开发数据时应先手动导出或重建。
- 验证：`cargo fmt`、`cargo test -p ai-chat-db`、`cargo test -p ai-chat-core`、
  `cargo check -p ai-chat2`、`git diff --check`。

2026-06-11 Codex-style stop generation 实现记录：

- ChatForm 运行中不再显示 disabled send，而是同尺寸 stop 按钮；按钮使用 `Square` icon 和
  `chat-form-stop-tooltip`，点击发出 `ChatFormEvent::StopRequested`。运行中 Enter 仍只忽略 submit，
  不触发 stop。
- `ConversationRuntimeStore` 的 active run 增加本地 `ActiveRunKey`、cancel token callback 和
  cancel-requested 状态；`finish_run` 与强制 stop 都校验 key，避免旧 task 迟到 finish 误删新 run。
- `stop_run` 首次调用时 cancel 当前 token，并启动 100ms grace task；如果同一个 active run 仍未结束，
  则调用 `AgentRuntime::cancel_run`，把 run、active provider step 和 active tool invocation 标为
  `Canceled`，run error 保持 `None`，移除 active run 并发 `ConversationChanged + RunFinished`。
- `agent_run_id` 尚未回填时，会从当前 conversation 最新非终态 run 兜底取消；用户取消不写入
  `last_errors`，避免弹 “runtime canceled” 错误通知。正常 provider/tool 失败仍保留错误通知。
- 验证：`cargo fmt --package ai-chat2 --package ai-chat-agent`、`cargo fmt --package ai-chat2`、
  `cargo test -p ai-chat-agent cancel`、`cargo test -p ai-chat2 conversation_runtime`、
  `cargo test -p ai-chat2 chat_form`、`cargo test -p ai-chat2`、`cargo check -p ai-chat2`、
  `cargo clippy -p ai-chat2 --all-targets -- -D warnings`、`git diff --check`。

2026-06-11 Temporary Conversation Window 首版实现记录：

- 本轮把 Home-only `ChatForm`、composer editor、model/reasoning picker、conversation detail/timeline/message
  和 timestamp/format helper 抽到 `components` / `foundation`，Home 与 Temporary 共享这些模块。
- 新增 `state::temporary` 和 `FreshRepository::list_no_project_conversations`，只读取 visible
  `ProjectKind::Scratch` 下的 active conversations；搜索匹配 conversation title 和
  `conversation_items.search_text`，不匹配 normal project conversations。
- 新增真实 `app::temporary_window` 和 `features::temporary`：菜单与 temporary hotkey 打开/复用窗口，
  顶部单行搜索，左侧 `ListState` no-project 历史列表，右侧复用 `ConversationDetailPage` 或无项目
  `ChatForm` 新对话。
- 已接键盘流：搜索 focus 时 up/down 切换列表并同步右侧 detail，Tab 直接 focus 当前右侧 composer，
  `secondary-n` 切到新对话并 focus composer。新对话发送走 `CreateConversationRequest { project_id:
  None, ... }`，刷新左侧列表，打开新 conversation，并启动真实 `AgentRuntime` run。
- 仍未做 selected text/screenshot input、save/promote to normal conversation、retry/resend、approval action、
  attachments/multimodal input 和 rich tool UI。
- 验证：`cargo fmt`、`cargo test -p ai-chat-db no_project`、`cargo test -p ai-chat2 temporary`、
  `cargo test -p ai-chat2 chat_form`、`cargo test -p ai-chat2 conversation`、`cargo check -p ai-chat2`、
  `git diff --check`。手动 GPUI UI 验证仍未运行。

2026-06-12 Prompt Settings 第一版实现记录：

- live GitHub 状态：#137 和 #159 仍 open；`codex/issue-159-ai-chat2-ui` 相关 PR 列表中只有已合入的
  PR #164，当前本地 Prompt Settings 增量暂无新 PR。
- `prompts.content_json JSON` 已按 fresh DB pre-main 规则改为 `prompts.content TEXT NOT NULL`；
  `PromptContent` 收敛为 `{ text }`，不再保留复杂 role/content-parts prompt shape。
- 新增 `PromptCatalogStore` 和 repository CRUD，Settings Prompts 页面通过 state 层 list/create/update/delete，
  不直接散落 SQL 或 DB connection。
- Settings Prompts 已按管理页模式实现：顶部搜索 + Add Prompt、全宽管理行、显式 View/Edit/Delete、
  查看 modal、编辑 modal、现有 destructive confirm 硬删除；不使用左右分栏，也不使用选择器式
  `ListState`。
- UI 修正：Add Prompt 按钮恢复默认高度，与搜索框同高；content 编辑区保留
  `InputState::multi_line(true).rows(10)` 并固定多行显示高度，避免看起来像单行输入。
- 验证：`cargo fmt`、`cargo test -p ai-chat-core prompt`、`cargo test -p ai-chat-db prompt`、
  `cargo test -p ai-chat-agent prompt`、`cargo test -p ai-chat2 prompts`、
  `cargo test -p ai-chat2 settings`、`cargo check -p ai-chat2`、
  `cargo clippy -p ai-chat2 -p ai-chat-core -p ai-chat-db -p ai-chat-agent --all-targets --all-features -- -D warnings`、
  `git diff --check`。手动 GPUI UI 验证仍未运行。
