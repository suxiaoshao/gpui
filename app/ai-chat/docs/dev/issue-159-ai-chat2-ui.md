# Issue #159 ai-chat2 UI 全量工作清单

本文档是 GitHub issue #159 的细化开发清单和全量能力追踪板。父级协调文档仍是
`app/ai-chat/docs/dev/issue-137-llm-abstractions.md`；本文档负责把
`app/ai-chat2` UI、壳、本机状态、可观测性和旧 `app/ai-chat` 能力映射拆成可执行事项。

最后同步时间：2026-05-28。

当前分支：`codex/issue-159-ai-chat2-ui`。

当前状态：进行中。当前分支已包含基础设施壳、app chrome、About、Sidebar/home skeleton 和 Home root/sidebar
结构修正，但尚未创建 PR，尚未合入 `codex/issue-137-llm-abstractions`。

已完成提交：

- `b749528 feat(ai-chat2): wire infrastructure shell`
- `0843e15 feat(ai-chat2): add app chrome and bundle shell`
- `e7077fc feat(ai-chat2): implement about window`
- `6d4a34f feat(ai-chat2): add sidebar home shell`

## 状态定义

| 状态 | 含义 |
| --- | --- |
| 已完成 | 代码可运行，且不是占位。 |
| 占位 | 入口、窗口、菜单或资源已接线，但没有真实业务行为。 |
| 后端已具备 | `ai-chat-core` / `ai-chat-db` / `ai-chat-agent` 已有模型或 API，但 `ai-chat2` UI 未消费。 |
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

## 已完成

| 事项 | 当前位置 | 说明 |
| --- | --- | --- |
| `app/ai-chat2` binary package | `app/ai-chat2` | 已加入 workspace，可独立 `cargo run -p ai-chat2`。 |
| basic tracing | `app/ai-chat2/src/app.rs` | 已初始化基础 `tracing_subscriber::fmt` subscriber；当前只进入 subscriber 输出，不写 log 文件。 |
| fresh DB bootstrap | `app/ai-chat2/src/database.rs` | 使用 `ai_chat_db::FreshStore::open_in_dir` 打开 fresh store；不读取 legacy store。 |
| TOML + DB settings | `app/ai-chat2/src/state/config.rs` | TOML 保存本机启动层配置；language/theme/hotkey/default project 来自 fresh DB app settings。 |
| local layout state | `app/ai-chat2/src/state/layout.rs` | 本机 `state.toml` 保存 `sidebar_width`，加载时 clamp，拖拽后 debounce 保存；不进入 fresh DB 或 `ai-chat-core` payload。 |
| typed app settings payload | `crates/ai-chat-core` / `crates/ai-chat-db` | `AppSettingsPayload` 已结构化，DB roundtrip 已覆盖。 |
| theme 初始化 | `app/ai-chat2/src/state/theme.rs` | 初始化 system accent，按 DB settings 和窗口 appearance 应用 theme。 |
| i18n 初始化 | `app/ai-chat2/src/foundation/i18n.rs` | 支持 `en-US` / `zh-CN`，未知语言按 system fallback。 |
| runtime assets | `app/ai-chat2/src/foundation/assets.rs` | 组合 app-local Lucide、app icon 和 `gpui-component-assets`。 |
| global hotkey runtime | `app/ai-chat2/src/state/hotkey.rs` | 注册 temporary hotkey 和 enabled shortcuts，记录 diagnostics。 |
| app menu | `app/ai-chat2/src/app/menus.rs` | 已接 `About`、`Open Main`、`Open Temporary Conversation`、`Settings`、`Quit`、Window menu 和 macOS Hide/Show actions。 |
| About window | `app/ai-chat2/src/app/about.rs` | 已实现精简真实 About：app icon、名称、描述、版本、license 和 GitHub 链接。 |
| titlebar menu bar | `app/ai-chat2/src/app/title_bar_menu.rs` | 非 macOS 渲染 app icon + component menu bar；macOS 使用系统菜单。 |
| main window shell | `app/ai-chat2/src/features/home/shell.rs` | 主窗口 UI root 已移出 `app.rs`：titlebar、可调宽 Sidebar、空内容区和 `gpui-component` sheet/dialog/notification layers 都挂在 Home root。 |
| home sidebar component | `app/ai-chat2/src/features/home/sidebar.rs` | Sidebar 已拆成独立组件；当前底部只有 Settings item，仍打开占位 Settings 窗口。 |
| dock/app reopen | `app/ai-chat2/src/app.rs` | app reopen 会 show/create main window。 |
| close-hide behavior | `app/ai-chat2/src/app.rs` | macOS/Windows 主窗口关闭时隐藏窗口。 |
| Minimize/Zoom action | `app/ai-chat2/src/app.rs` / `placeholder_windows.rs` | 主窗口和占位窗口已处理 Window menu action。 |
| bundle metadata | `app/ai-chat2/Cargo.toml` | 已有 `[package.metadata.bundle]`。 |
| app icon | `app/ai-chat2/build-assets/icon/` | 本轮复用旧 `ai-chat` 图标作为 v1 shell icon。 |
| macOS bundle localization | `app/ai-chat2/locales/macos/` | 已有 `en-US` 和 `zh-Hans` InfoPlist strings。 |
| Windows icon build script | `app/ai-chat2/build.rs` | Windows build 时从 base PNG 派生 multi-frame `.ico`。 |
| `xtask bundle ai-chat2` | `crates/xtask/src/cli.rs` | `BundleApp::AiChat2` 已加入，CLI parse test 已覆盖。 |

## 占位

| 事项 | 当前位置 | 当前行为 | 后续需要 |
| --- | --- | --- | --- |
| Settings window | `app/ai-chat2/src/app/placeholder_windows.rs` | 只显示“设置持久化已接入，设置界面暂不实现”。 | 实现 language/theme/hotkey/provider/model/prompt/shortcut/tool policy 设置 UI。 |
| Temporary Conversation window | `app/ai-chat2/src/app/placeholder_windows.rs` | 只显示“临时对话运行时暂不接入”。 | 接入真实 temporary chat、prompt/model/provider 和 agent run。 |
| temporary hotkey action | `app/ai-chat2/src/state/hotkey.rs` | 触发后只记录 `last_pressed` 和基础 tracing 事件；当前不写 log 文件。 | 打开/切换真实 temporary conversation。 |
| shortcut hotkey action | `app/ai-chat2/src/state/hotkey.rs` | 触发后只记录 diagnostics 和基础 tracing 事件；当前不写 log 文件。 | 按 shortcut 的 prompt/provider/model/input/action 执行 agent run。 |

## 基础设施 / 本地状态 / 可观测性

| 能力 | `ai-chat2` 状态 | 说明 |
| --- | --- | --- |
| app-local config | 已完成 | `config.toml` 只保存启动层本机配置；业务 settings 仍来自 fresh DB。 |
| layout state | 已完成 | `state.toml` 目前只保存 Sidebar 宽度；这是本机 UI state，不是 app settings。 |
| basic tracing subscriber | 已完成 | 当前只初始化基础 tracing subscriber，供 stdout/stderr 或宿主 subscriber 消费。 |
| file-backed logging | 未开始 | 旧 app 写 `~/Library/Logs/<APP_NAME>/data.log`；`ai-chat2` 还没有 logs dir、file writer 或日志文件错误处理。 |
| open/copy diagnostics | 未开始 | 没有打开日志目录、复制诊断信息、导出 runtime snapshot 或用户可见 diagnostics 面板。 |
| user-visible startup/runtime errors | 未开始 | 当前错误主要通过 startup result 和 tracing 暴露；没有统一的 UI error surface。 |
| main/settings window placement | 未开始 | 旧 app 保存 main/settings window bounds；`ai-chat2` 当前只保存 Sidebar 宽度。 |
| hotkey diagnostics UI | 未开始 | runtime 只保留内存 diagnostics；Settings 真实页面尚未展示注册失败、最近触发或重注册状态。 |

## 后端已具备但 UI 未接

| 能力 | 后端位置 | UI 缺口 |
| --- | --- | --- |
| projects | `ai-chat-db` repositories / fresh schema | 没有 project sidebar、open folder、recent projects、scratch project 或 default project flow。 |
| conversations | `ai-chat-db` repositories / fresh schema | 没有 conversation list、create/archive/delete/search/title/status UI。 |
| canonical timeline | `conversation_items` | 没有按 `seq` 渲染 timeline，也没有 streaming append/update UI。 |
| attachments | `attachments` + typed payloads | 没有 file/image/audio attach、preview、generated output 或 storage UI。 |
| agent runs | `ai-chat-agent::AgentRuntime` + `agent_runs` | 没有 run/cancel/retry/resend UI，也没有 active run state display。 |
| provider steps | `provider_steps` | 没有 provider/model picker、provider step debug surface 或 continuation display。 |
| tool invocations | `tool_invocations` + `ToolRegistry` | 没有 tool call/progress/result timeline UI。 |
| approvals | `approval_decisions` + agent runtime | 没有 approval prompt、approve/deny/cancel/expired UI。 |
| usage | `usage_events` | 没有 token/usage summary 或 rollup UI。 |
| prompts | `prompts` | 没有 prompt CRUD、selection、snapshot display。 |
| providers | `providers` | 没有 provider settings UI、secret refs UI 或 enabled/disabled control。 |
| provider models | `provider_models` | 没有 manual refresh、model cache display 或 capability detail UI。 |
| app settings | `app_settings` | DB 已保存 language/theme/hotkey/default project；没有真实 settings page。 |
| file-backed skills | `ai-chat-agent::skills` | 没有 skill catalog、activation display 或 skill snapshot timeline UI。 |
| MCP helpers | `ai-chat-agent::mcp` | 没有 MCP config UI、connected server status 或 MCP tool approval UI。 |

## 未开始 UI 清单

| 区域 | 事项 |
| --- | --- |
| Project navigation | project-first sidebar、open folder、recent projects、scratch project、default project、project metadata/status。 |
| Conversation navigation | conversation list、new conversation、archive/delete、search/filter、title edit、status display、last item preview。 |
| Composer | prompt selector、provider/model selector、capability warning、text input、multi-part input、send/run、cancel、retry、resend。 |
| Timeline text | user/assistant text item、streaming text delta、multi-block assistant output、copy/export affordance。 |
| Reasoning | multiple reasoning blocks、reasoning summary、collapsed/expanded state、provider-specific reasoning capability gating。 |
| Tools | local/MCP/provider-hosted tool call、progress、result、error、structured output、attachment result、tool name collision display。 |
| Approvals | approval request card、approve/deny/cancel actions、pending/expired/decided states、recovery after restart。 |
| Status and errors | queued/running/waiting/completed/failed/canceled 状态、retry/cancel affordance、user-visible error item。 |
| Usage | per-run usage summary、provider/model token counts、usage event rollup display。 |
| Attachments and multimodal | image/file/audio input、generated files/images、preview/download/open, provider unsupported-state messaging。 |
| Settings | language、theme、temporary hotkey、default project、providers、provider models manual refresh、prompts、shortcuts、tool/MCP policy。 |
| Shortcuts | shortcut CRUD、input source selection、prompt/provider/model binding、capability validation、registration/runtime status。 |
| Temporary chat | real temporary conversation window、selected text/screenshot input、save/promote to conversation。 |
| Screenshot/input capture | screenshot overlay、OCR fallback、image-capable model data URL path、unsupported model warnings。 |
| Legacy access | read-only legacy viewer、manual export/import 或 backup-only policy；当前没有任何 legacy data UI。 |
| Export/import | fresh conversation export、generated output export、legacy manual import/export。 |
| Capability gating | tool calling、MCP、image/file/audio input、image generation、structured output、reasoning、provider-specific extensions。 |

## 旧 `app/ai-chat` 对比

| 旧 `app/ai-chat` 能力 | 旧实现位置 | `ai-chat2` 状态 | 迁移说明 |
| --- | --- | --- | --- |
| app bootstrap / tracing | `app.rs` | 已替代 | 启动流程已由 `ai-chat2` app shell 承接；file-backed logging 作为单独未开始项追踪。 |
| app menu | `app/menus.rs` | 已完成 | 已按 shell 需要迁移；未接旧 temporary 业务。 |
| tray/status item | `app/tray.rs` | 暂不做 | 不在当前 #159 UI 壳范围；如需要应单独定义 scope。 |
| titlebar menu | `components/title_bar_menu.rs` | 已完成 | 已复制为 app-local titlebar menu bar。 |
| bundle/app icon | `build-assets` / `build.rs` / `xtask` | 已完成 | 图标暂时复用旧 `ai-chat`，后续可替换品牌图标。 |
| About window | `features/about.rs` | 已完成 | `ai-chat2` 已实现精简真实 About，不照搬旧 tray/about 文案。 |
| Settings window | `features/settings.rs` | 占位 | 旧 settings page 尚未迁移；新 UI 应按 fresh settings/prompts/providers 重新设计。 |
| Provider settings | `features/settings/provider_settings.rs` | 未开始 | 需要接 fresh `providers` / `provider_models`，并支持手动刷新 model cache。 |
| Appearance settings | `features/settings/appearance_settings.rs` | 未开始 | 主题状态已接入；settings UI 未实现。 |
| General settings | `features/settings/general_settings.rs` | 未开始 | language/default project/hotkey UI 未实现。 |
| Template settings | `features/settings/template_settings.rs` | 不照搬 | 新模型删除 templates，改为 prompts。 |
| Shortcut settings | `features/settings/shortcut_settings.rs` | 未开始 | 新 shortcuts 不绑定 template/mode，应绑定 prompt/provider/model/input/action。 |
| shortcut form/list/status dialogs | `features/settings/shortcut_settings/{form,list,dialogs,validation}.rs` | 未开始 | 需要按 fresh shortcut schema 重做 CRUD、状态弹窗、冲突/注册失败/capability mismatch 和重注册 UI。 |
| reusable hotkey input | `components/hotkey_input.rs` | 未开始 | Settings 真实页面需要重做 hotkey 输入、解析、清空和冲突反馈。 |
| provider ext settings help | `components/ext_setting_help.rs` | 未开始 | provider-specific settings 应由 fresh provider/model config 和 capability UI 重新表达。 |
| home shell | `features/home.rs` / `features/home/shell.rs` | 已完成 | `ai-chat2` 已有最小主页骨架：可调宽 Sidebar、空内容区、titlebar 和 gpui-component overlay layers；project/conversation 业务未接。 |
| workspace sidebar width | `state/workspace.rs` / `state/workspace/persistence.rs` | 已替代 | 旧 workspace state 的 sidebar width 已由 `ai-chat2` 本机 `state.toml` layout state 承接。 |
| main/settings window placement | `state/workspace.rs` / `state/workspace/persistence.rs` | 未开始 | 旧 app 保存 main/settings window bounds；`ai-chat2` 当前未保存窗口位置和尺寸。 |
| workspace tabs and drafts | `state/workspace.rs` / `state/workspace/tabs.rs` | 不照搬 | 旧 tab/draft/open-folder workspace shape 不适合 project-first fresh timeline，需要重新设计。 |
| folder sidebar | `features/home/sidebar.rs` | 不照搬 | 新模型没有 folders；改为 project-first navigation。 |
| add folder dialog | `components/add_folder.rs` | 不照搬 | 新模型没有 folders，不迁移旧 add folder flow。 |
| add conversation dialog | `components/add_conversation.rs` | 未开始 | 新 conversation create flow 应围绕 project、prompt、provider/model 和 canonical timeline 重新设计。 |
| tabs/conversation panel | `features/home/tabs.rs` | 未开始 | 新 UI 需要按 fresh conversation/timeline 重写。 |
| conversation list/search | `features/home/search.rs` / `search_list.rs` | 未开始 | 需要基于 fresh conversations 和 `conversation_items.search_text` 后续设计。 |
| delete confirmation | `components/delete_confirm.rs` | 未开始 | project/conversation/prompt/provider/shortcut 等 destructive actions 需要新的确认策略。 |
| conversation export | `features/home/export.rs` | 未开始 | 新 export 应读取 canonical `conversation_items`。 |
| chat form | `components/chat_form.rs` | 未开始 | 新 composer 不应暴露 conversation mode/template controls。 |
| chat form provider ext settings | `components/chat_form/ext_settings.rs` | 未开始 | 新 composer/provider settings 需要按 provider capability 和 typed extension 重做。 |
| mode select | `components/chat_form/mode_select.rs` | 不照搬 | 新模型所有 conversation 都 contextual。 |
| template picker | `components/chat_form/template_picker.rs` | 不照搬 | 新 UI 使用 prompt selector。 |
| model select | `components/chat_form/model_select.rs` | 未开始 | 需要接 fresh `provider_models` cache 和 capability gating。 |
| message rendering | `components/message.rs` | 未开始 | 新 timeline 要覆盖 reasoning/tool/approval/status/usage/attachments。 |
| temporary chat | `features/temporary.rs` | 占位 | 只有占位窗口；未接 selected text/screenshot/save flow。 |
| temporary window runtime | `features/hotkey/temporary_window.rs` | 未开始 | 未实现前台 app restore、显示器定位、延迟隐藏、切换/移动真实 temporary window。 |
| hotkey backend/registry | `features/hotkey/backend.rs` / `registry.rs` | 占位 | `ai-chat2` 已有初始注册和内存 diagnostics；保存后重注册、状态 UI 和真实执行尚未实现。 |
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
- bundle GUI smoke：打开 `target/release/bundle/macos/AI Chat 2.app`，验证主窗口、macOS 菜单、Settings/About/Temporary 占位窗口和 Open Main。

About 页面实现后已运行：

- `cargo fmt`
- `cargo test -p ai-chat2 about`
- `cargo check -p ai-chat2`
- `git diff --check`
- `cargo run -p xtask -- bundle ai-chat2`
- bundle GUI smoke：打开 `target/release/bundle/macos/AI Chat 2.app`，验证 About 真实窗口、重复打开复用已有窗口、GitHub 按钮触发 URL 打开、Settings/Temporary 占位窗口仍可打开。

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

文档-only 更新只需运行 `git diff --check`。
