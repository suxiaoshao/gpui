# ai-chat2 packaged .app 手动冒烟测试（2026-06-27）

本文记录 2026-06-27 针对 `ai-chat2` packaged macOS `.app` 的手动冒烟测试。测试目标是确认当前
`codex/issue-137-llm-abstractions` 集成分支可以被打包为本地 `.app`，并在隔离数据目录中完成基础 UI、
Provider/model、真实 agent run 和设置页验证。

## 测试边界

- 目标 artifact：`target/release/bundle/macos/AI Chat 2.app`。
- 打包命令：`cargo run -p xtask -- bundle ai-chat2`。
- UI 验证工具：Computer Use，附着目标必须是 workspace 下的 `.app` 完整路径。
- 真实数据只作为只读迁移来源；测试运行不得写入真实
  `/Users/sushao/Library/Application Support/top.sushao.ai-chat2`。
- API key 不写入文档、不截图、不在终端输出。测试库仅保留 provider/model 记录里的 secret ref，
  由运行时按现有 Keychain ref 读取。

## 数据隔离方案

- 临时根目录：`/private/tmp/gpui-ai-chat2-smoke-2026-06-27`。
- 本轮运行目录：`/private/tmp/gpui-ai-chat2-smoke-2026-06-27/run-184331`。
- 运行时 HOME：`/private/tmp/gpui-ai-chat2-smoke-2026-06-27/run-184331/home`。
- 测试数据目录：`/private/tmp/gpui-ai-chat2-smoke-2026-06-27/run-184331/data`。
- 测试 `config.toml`：只写 `[storage].data_dir` 指向测试数据目录，不迁移真实 `state.toml`、窗口状态或
  MCP server 配置。
- 测试 DB：从真实 `ai_chat_fresh.sqlite3` 复制后清理会话、附件、runs、prompts、projects 和
  shortcuts，只保留 providers 和 provider_models。
- 快捷键：测试库中不保留 shortcuts，避免注册真实全局快捷键。
- 第二轮补测改用真实 `HOME` 读取 macOS login Keychain，同时通过 app-local 环境变量隔离 app 数据：
  `AI_CHAT2_CONFIG_DIR=/private/tmp/gpui-ai-chat2-full-smoke-2026-06-27/run-190605/config`，
  `AI_CHAT2_LOG_DIR=/private/tmp/gpui-ai-chat2-full-smoke-2026-06-27/run-190605/logs`，并在临时
  `config.toml` 中把 `[storage].data_dir` 指向
  `/private/tmp/gpui-ai-chat2-full-smoke-2026-06-27/run-190605/data`。这样可以使用真实 Keychain secret，
  但不会写入真实 `~/Library/Application Support/top.sushao.ai-chat2` 或真实 app log 目录。

## Provider 测试矩阵

- Ollama：使用本机可用 model（当前 DB 中有 `qwen3.5:*`）。
- OpenAI：优先使用 `gpt-5.4`。
- DeepSeek：优先使用 `deepseek-v4-pro`（对应用户提到的 4.0-pro 系列）。

## 计划验证项

- `.app` 能被成功构建，bundle 路径与 metadata 正确。
- 用隔离配置、数据和日志目录启动 `.app`，确认附着到 workspace 下的目标 bundle。
- 主窗口能打开，sidebar / New Conversation / Settings 基本可见。
- Provider Settings 能读取迁移后的 OpenAI、DeepSeek、Ollama provider 和 model 列表。
- 新对话页能选择 provider/model 并发送无敏感内容的 smoke prompt。
- 至少记录一次真实 agent run 的成功或失败；失败时记录用户可见错误和相关日志摘要。
- Settings 页基本导航不崩溃：General、Appearance、Providers、Prompts、Shortcuts、Skills、MCP。

## 执行记录

- 2026-06-27：文档创建，准备打包与隔离测试数据。
- 2026-06-27：`cargo run -p xtask -- bundle ai-chat2` 成功生成
  `target/release/bundle/macos/AI Chat 2.app`。命令仅输出既有 `block v0.1.6`
  future-incompat warning。
- 2026-06-27：从真实 fresh DB 复制到临时数据目录后清理测试副本。清理后测试 DB 计数为：
  providers=3、provider_models=50、conversations=0、projects=0、prompts=0、shortcuts=0。
- 2026-06-27：使用
  `HOME=/private/tmp/gpui-ai-chat2-smoke-2026-06-27/run-184331/home`
  启动 packaged executable。日志确认 `data_dir` 指向临时数据目录，`temporary_hotkey=None`，
  `registered_shortcuts=0`。
- 2026-06-27：Computer Use 确认附着到
  `/Users/sushao/Documents/code/gpui/target/release/bundle/macos/AI Chat 2.app/`
  pid 13508，不是 `/Applications` 下的旧安装。
- 2026-06-27：主窗口、New Conversation、sidebar、Settings window 正常打开。
- 2026-06-27：Provider Settings 能读取迁移后的 OpenAI、DeepSeek、Ollama provider；OpenAI/DeepSeek
  显示 `已保存` 且没有泄露 API key；Ollama 显示 `http://localhost:11434` 和
  `qwen3.5:4b`、`qwen3.5:9b`、`qwen3.5:9b-nvfp4`。
- 2026-06-27：model picker 能显示并切换 DeepSeek/Ollama model；`deepseek-v4-pro` 和
  `qwen3.5:4b` 可见。
- 2026-06-27：Ollama `qwen3.5:4b` run 成功完成。DB 中第二个 `agent_runs.status=completed`，
  `provider_steps` 记录 `provider_id=Ollama`、`model_id=qwen3.5:4b`、`status=completed`；
  日志记录 completion 为 `packaged app ollama ok`。
- 2026-06-27：Settings 其它分栏 smoke：Appearance 主题页正常；Projects/Prompts/Shortcuts 空状态正常；
  Skills 空状态正常；MCP 页显示 `config.toml 中没有 MCP 服务器`，符合本次没有迁移 MCP 配置的隔离策略。
- 2026-06-27：测试结束后 Ctrl-C 退出 packaged app 进程；`pgrep -fl 'ai-chat2|AI Chat 2'` 无残留进程。
- 2026-06-27：为安全复用真实 Keychain，补充 `AI_CHAT2_CONFIG_DIR` / `AI_CHAT2_LOG_DIR` 测试覆盖；
  重新执行 `cargo fmt`、`cargo test -p ai-chat2 directory_override` 并重新
  `cargo run -p xtask -- bundle ai-chat2`，bundle 仍成功生成。
- 2026-06-27：第二轮从真实 fresh DB 复制到
  `/private/tmp/gpui-ai-chat2-full-smoke-2026-06-27/run-190605/data/ai_chat_fresh.sqlite3`，清理后
  providers=3、provider_models=50、conversations=0、projects=0、prompts=0、shortcuts=0。
- 2026-06-27：第二轮启动命令保留真实 `HOME`，只覆盖 `AI_CHAT2_CONFIG_DIR`、`AI_CHAT2_LOG_DIR` 和
  临时 `[storage].data_dir`。启动日志确认 database path 和 log path 均位于
  `/private/tmp/gpui-ai-chat2-full-smoke-2026-06-27/run-190605`，`registered_shortcuts=0`。
- 2026-06-27：DeepSeek `deepseek-v4-pro` 使用真实 Keychain secret 成功完成，日志 completion 为
  `deepseek packaged ok`，DB 中 run 状态为 `completed`。
- 2026-06-27：OpenAI `gpt-5.4` 能在 model picker 中搜索和选中，发送后 DB 创建
  `agent_runs.status=running`，但约 1 分 34 秒内 runtime 日志没有进入 `invoke_agent` streaming；
  手动点击停止后 DB 状态变为 `canceled`，UI 显示 `Agent 运行已取消`。
- 2026-06-27：Ollama `qwen3.5:4b` 成功完成，日志 completion 为 `ollama packaged ok`，DB 第二轮汇总为
  `completed=2`、`canceled=1`。
- 2026-06-27：Settings General/Appearance/Providers/Projects/Prompts/Skills/Shortcuts/MCP 均可打开；
  Appearance 可在暗色与跟随系统间切换并回滚；Provider 页能显示 OpenAI/DeepSeek `已保存` 和模型开关；
  Prompts 在临时 DB 中完成新增和编辑保存；Skills 能列出并搜索；Shortcuts 新增表单可打开但未保存全局快捷键。
- 2026-06-27：Projects 页面空状态和 Add 入口可见；按数据安全边界未继续选择真实目录。DB 中出现的 3 条
  projects 是 no-project conversation 自动创建的 scratch project，路径均在临时 data 的
  `scratch-projects/` 下。
- 2026-06-27：MCP Settings 在临时 `config.toml` 中成功新增 `smoke-true` stdio server，命令为
  `/usr/bin/true`；页面显示未连接、0 个工具；随后切换为 disabled，临时 config 中
  `[mcp_servers.smoke-true].enabled=false`。
- 2026-06-27：应用菜单中 `打开临时对话` 可触发临时对话窗口；窗口打开后显示无项目会话列表和右侧当前会话详情。
- 2026-06-27：第二轮测试结束后 Ctrl-C 退出 packaged app 进程；提权确认
  `pgrep -fl 'ai-chat2|AI Chat 2'` 无残留进程。

## 第二轮安全测试矩阵

| 区域 | 结果 | 说明 |
| --- | --- | --- |
| 打包 | 通过 | `cargo run -p xtask -- bundle ai-chat2` 成功生成 workspace 下 `.app`。 |
| 启动隔离 | 通过 | 真实 `HOME` 仅用于 Keychain；config、data、log 全部落在 `/private/tmp/.../run-190605`。 |
| 主窗口/新对话 | 通过 | 新建会话、sidebar 列表刷新、conversation detail 打开均正常。 |
| Sidebar 搜索 | 通过 | 搜索 `ollama` 后只保留匹配会话，点选结果可跳转。 |
| Model picker | 通过 | 可搜索并切换 `deepseek-v4-pro`、`gpt-5.4`、`qwen3.5:4b`。 |
| DeepSeek run | 通过 | `deepseek-v4-pro` 完成真实 run，说明 Keychain secret 可读。 |
| Ollama run | 通过 | `qwen3.5:4b` 完成真实 run。 |
| OpenAI run | 失败/已取消 | `gpt-5.4` 创建 run 后长期停在 running，手动 stop 后进入 canceled。 |
| Stop generation | 通过 | OpenAI 卡住时点击停止，UI 和 DB 都进入 canceled。 |
| Settings General | 通过 | 页面打开，显示语言、HTTP proxy、临时会话快捷键、配置文件入口；未点击“打开”。 |
| Settings Appearance | 通过 | 暗色切换生效，随后回滚到跟随系统。 |
| Settings Providers | 通过 | OpenAI/DeepSeek/Ollama 配置和模型列表可读；未保存新的 secret。 |
| Settings Projects | 未完整测 | 空状态和 Add 入口可用；按数据安全边界未执行目录选择。 |
| Settings Prompts | 通过 | 临时 DB 中完成新增、编辑保存；未执行删除确认。 |
| Settings Skills | 通过 | catalog 列表、搜索和空状态可用；未展开读取正文。 |
| Settings Shortcuts | 部分通过 | 新增弹窗可打开；为避免注册全局快捷键，未保存。 |
| Settings MCP | 通过 | 新增无害 stdio server、列表展示、详情、disable 写回均正常。 |
| Temporary Window | 部分通过 | 菜单项可打开窗口；窗口显示无项目会话列表和详情，未继续发送临时 run。 |
| 附件/截图/剪贴板/选中文本 | 未测 | 这些动作会读取真实文件、屏幕或剪贴板，本轮按数据安全要求跳过。 |
| 内置工具/审批 | 未完整测 | 未绑定临时真实项目目录，且工具调用由模型触发；本轮只覆盖 approval mode selector 和 run stop。 |

## 产品侧待定位问题

### PKG-SMOKE-001：窗口操作时日志出现 `RefCell already borrowed`

现象：主窗口测试 run 后多次打开 Settings，UI 仍能打开并继续操作，但日志出现：

```text
ERROR open_ai_chat2_settings_window: gpui::window: RefCell already borrowed
```

本轮看到两组，每组对应 `gpui/src/window.rs` 的 1459、1525、1537 行。需要复核
`features/settings::open_ai_chat2_settings_window` 是否在窗口 activation / show 流程中嵌套借用了
同一个 window state。

第二轮补充：在 Settings/MCP/Temporary Window 测试过程中又记录到一次非 fatal 日志：

```text
ERROR gpui::window: RefCell already borrowed
```

对应 `gpui/src/window.rs:1553`，应用未崩溃并继续响应。需要把 Settings window open 和 temporary window
open/close 路径放在一起复核。

修复记录（2026-06-27）：全局搜索 `activate_window()`、`show_without_activation()`、`move_and_resize()`
和 `WindowExt::show()` 后确认同类问题集中在 `ai-chat` / `ai-chat2` 的主窗口、Settings、About 和临时窗口
reveal 路径。根因不是业务状态错误，而是应用在 `WindowHandle::update(...)` 持有 GPUI window data mutable
borrow 时调用 AppKit `makeKeyAndOrderFront` / `orderFront` / `setFrame` 等平台操作；这些操作可能同步触发
GPUI 的 `on_request_frame` / `on_moved` / activation callback，并再次 `handle.update(...)` 同一个窗口，
从而记录 `RefCell already borrowed`。

本次修复：

- `crates/window-ext` 新增 `NativeWindowHandle`，允许在 `WindowHandle::update(...)` 内只提取原生窗口句柄。
- `ai-chat2` 的 main/about/settings/temporary window 复用路径改为：update 内只更新 GPUI focus/state 并提取
  `NativeWindowHandle`；update 结束后通过 `cx.defer` 调用原生 `show()` / `move_and_resize()` /
  `set_window_level()`。
- `ai-chat` 的同构 main/about/settings/temporary window 路径同步修复，避免旧 app 继续保留相同生命周期缺陷。
- 新建普通窗口不再额外手动 `show()` / `activate_window()`，继续依赖 GPUI `WindowOptions` 的默认
  `show=true`、`focus=true` 初始打开路径。

已验证：

- `rg -n "\bwindow\.(show|show_without_activation|move_and_resize)\(" app/ai-chat app/ai-chat2` 无残留命中。
- `rg -n "activate_window\(" app/ai-chat app/ai-chat2` 无残留命中。
- `cargo fmt`
- `cargo check -p ai-chat2 -p ai-chat`
- `cargo test -p ai-chat2 -p ai-chat`
- `cargo clippy -p ai-chat2 -p ai-chat --all-targets -- -D warnings`
- `cargo run -p xtask -- bundle ai-chat2` 重新生成
  `target/release/bundle/macos/AI Chat 2.app`。期间 `actool` 因当前 CoreSimulator/ibtool 环境报错，xtask
  按预期跳过 Liquid Glass 图标注入并保留普通图标，bundle 命令最终成功退出。

待复核：重新执行 packaged UI 冒烟，覆盖 Settings/Temporary Window 路径，确认日志中不再出现
`RefCell already borrowed`。

### PKG-SMOKE-002：assistant 文本首部在 GUI 中疑似被裁剪

现象：Ollama run 的 DB 和日志都记录完整 completion：

```text
packaged app ollama ok
```

但 GUI 中可见文本显示为 `aged app ollama ok`，开头 `pack` 不可见。需要复核 conversation timeline /
assistant markdown block 的左边界、clip 或滚动区域是否存在首部裁剪。

第二轮补充：DeepSeek 日志 completion 为 `deepseek packaged ok`，GUI 可见为 `seek packaged ok`；
Ollama 日志 completion 为 `ollama packaged ok`，GUI 可见为 `ama packaged ok`。同一问题在两个 provider 上复现，
更像 timeline/markdown layout 问题，而不是 provider 输出问题。

### PKG-SMOKE-003：OpenAI `gpt-5.4` run 长时间停在 running 且未进入 streaming 日志

现象：第二轮保留真实 `HOME` 后，OpenAI `gpt-5.4` 能被 model picker 搜索和选中，发送后 DB 创建 run：

```text
status=running, providerId=019f0491-837b-7812-a3c7-8be2e0603f8b, modelId=gpt-5.4
```

但约 1 分 34 秒内日志没有出现 `invoke_agent` 的 `Current conversation Turns` 记录，也没有 provider
错误输出。手动点击停止按钮后，DB 更新为：

```text
status=canceled
```

UI 显示 `Agent 运行已取消`。这证明 stop/cancel 路径可用，但 OpenAI run 卡住点需要继续定位：
可能在 secret 读取、provider/runtime 构建、请求发出前等待，或底层 provider 没有把错误写到当前日志。
