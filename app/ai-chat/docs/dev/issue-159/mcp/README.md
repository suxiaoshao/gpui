# Issue #159 MCP Settings/runtime

本目录保存 `ai-chat2` MCP 设置、运行时、OAuth 和工具注册计划。

## 文档

| 文档 | 用途 |
| --- | --- |
| `settings-and-runtime.md` | MCP 总设计和阶段 1 设计记录：配置模型、rmcp/Rig 接线、Settings UI、数据流、OAuth 后续项和验证计划。 |
| `phase-2.md` | 阶段 2 开发计划：MCP approval resume、OAuth runtime/UI 和运行时加固；项目级 MCP 配置已确认不进入阶段 2。 |

## 阶段状态

阶段 1 已完成：

- `config.toml only` 的 MCP server definitions source of truth；不新增 `mcp_servers` / `mcp_tools` / `mcp_oauth_tokens` SQLite 表。
- `rmcp 1.8.0` + `rig-core 0.39.0` 接入，支持 stdio 和非 OAuth streamable HTTP MCP server。
- Settings MCP 页面支持搜索、刷新/测试连接、server status、auth status、server info、tools/list。
- Settings MCP 页面支持 Add/Edit/Delete/Enable/Disable，并写回 `config.toml`。
- Add/Edit 默认 UI 已收敛为 Codex 风格简化表单：只暴露名称、stdio/http 常用字段；参数、环境变量、环境变量传递、HTTP headers、env-backed headers 都使用结构化 rows。
- agent run setup 会在 provider tool declarations finalize 前连接 enabled MCP server、拉取 tools/list、注册 `ToolSource::Mcp { server_id }` 工具；runtime session fingerprint 只在进程内使用，不写入 run payload 或数据库。
- fresh DB 已记录 MCP tool invocation 的 source、server id、raw tool name、runtime tool name、arguments、result、error 和 approval。

阶段 2 当前已完成：

- MCP tool approval resume：`approve_and_resume_tool(...)` 已改成 source-neutral executor，支持从保存的 run snapshot 恢复 MCP tool registry，并执行已批准的 MCP tool call。
- OAuth authorization-code UI/runtime：Settings Add/Edit 已收敛为 `需要 OAuth` 开关 + 授权状态卡片 + `授权` / `重新授权` 按钮；授权流程使用 `127.0.0.1:<random-port>` loopback listener、rmcp OAuth state、GPUI credentials token storage。
- OAuth HTTP runtime：run setup / Settings Test 会从 GPUI credentials 读取 rmcp `StoredCredentials`，只注入 agent-side runtime config，不进入 `config.toml`、SQLite 或 run payload；agent streamable HTTP path 使用 rmcp `AuthClient`。
- 配置清理：关闭 `需要 OAuth` 并保存、删除 OAuth server、HTTP OAuth URL 变更时，会删除对应 GPUI credentials，避免旧授权污染后续配置。
- OAuth refresh mirror：agent-side rmcp `CredentialStore` 被包装为 mirror store，refresh 后会把新的 `StoredCredentials` 作为 runtime event 发回 app，并写回 GPUI credentials。
- OAuth 状态操作：授权卡片区分授权中、已授权、需要授权、需要重新授权和失败状态；已授权时提供次要 `取消授权` 动作，删除 credentials 并断开 stale session。
- 运行时加固：`notifications/tools/list_changed` 会更新 Settings 状态；复用已有 session 前会重新执行 `tools/list`，避免下一次 run 使用旧 tool cache；单个 server 的测试错误写入对应 row，不再把全局错误复制到所有详情页。

阶段 2 后续 advanced/验证项：

- Scope upgrade 完整 flow：当前 insufficient scope 会进入“需要重新授权”状态并让用户手动重新授权；默认 UI 仍不暴露 scopes。后续如果要做 `Upgrade Access` 增量 scope flow，需要单独设计 advanced path。
- 更完整手动验证：用真实 OAuth MCP server 覆盖 browser login、refresh mirror、取消授权、scope failure 和 approval resume 的端到端流程。
- 项目级 MCP 配置已确认阶段 2 不做；不读取 project file、不合并 project-scoped definitions、不新增 trust prompt 或 project-scoped session key。
- Prompts、Resources、Sampling、Elicitation 已确认先不做；阶段 2 不实现，也不在当前文档中展开阶段 3。

## 通用原则

- MCP server definitions 的 source of truth 是 `config.toml`，不新增 SQLite source table。
- Secrets 不写入 TOML 或 chat DB。OAuth token 只写 GPUI credentials；TOML 只保存 OAuth definition。
- Settings 默认 Add/Edit UI 按 Codex 自定义 MCP 表单收敛，只暴露 stdio/http 常用字段。OAuth 默认 UI 只暴露 `需要 OAuth` 开关和授权状态卡片；scopes、resource、client id、callback 等高级 OAuth 字段仍属于 TOML advanced path。`required`、timeouts、tool allow/deny、per-server/per-tool approval override 也仍属于 TOML advanced path。
- 参数、环境变量、环境变量传递、HTTP headers 和 env-backed headers 使用结构化 rows，不再让用户输入字符串再解析。
- App 启动和打开 Settings 不 eager start enabled stdio server；只有 Settings `Test`/`Refresh`、OAuth `授权`/`重新授权`、agent run setup 会按需创建 runtime session 或 OAuth flow。
- 同一 app process 内，server id + runtime-only fingerprint 匹配时可以跨 conversation 复用 live session；config 变化、server disabled、app 退出、显式 disconnect 或 OAuth 取消授权时关闭 stale session。
- OAuth callback 是 desktop client 接收 authorization code 的临时入口，不是 MCP server。阶段 2 已确认按 Codex/Zed 策略实现：客户端临时绑定 `127.0.0.1:<random-port>` 的 loopback callback listener。
- 关闭 `需要 OAuth` 并保存时，删除 TOML OAuth definition，同时删除该 server 对应的 GPUI credentials token，避免旧授权状态污染后续配置。
- MCP tool 的默认审批继承 ChatForm 当前 run 的审批模式；per-server/per-tool approval override 暂时只作为 TOML advanced path 保留。
- 现阶段不做 Codex-style composer prewarm。后续只有在 OAuth status、错误展示和 retry/resend 稳定后，再重新评估受控 prewarm。

## 项目管理和信任边界

阶段 1 已完成的项目相关边界：

- MCP definitions 是 app-level config，不跟随 conversation 或 project 写入 chat DB。
- Conversation runtime 只在 run start 时读取当前 enabled MCP config 并注册本次 run 可用的 MCP tools；MCP definitions 只保存在 `config.toml`，live sessions/tool cache 只保存在 runtime，agent run DB 不保存 MCP config/hash。
- ChatForm 的审批模式是 per-run UI state，不按 conversation/project 跨重启持久化。
- 项目级 skill catalog 已有独立 project-aware 路径；MCP Settings 仍只管理 app-level MCP server，不展示 project `.agents` 来源。

阶段 2 已确认暂不做的项目级 MCP 事项：

- 不读取 `.codex/config.toml`、`.zed/settings.json`、project metadata 或其它项目目录内的 MCP definitions。
- 不实现全局 MCP 与项目 MCP 的合并规则。
- 不实现 project MCP trust prompt。
- 不新增 project-scoped MCP session key、trust id、project trust store 或 SQLite migration。
- approval resume 当前不依赖持久化 MCP config/hash；后续如果需要跨重启恢复 MCP tool call，需要重新设计不泄漏 secret 的 source-neutral resume 边界。

后续如果重新开启 project-level MCP，需要单独开阶段计划并重新确认配置来源、继承/覆盖规则、首次信任 UI、跨重启 trust 持久化位置和 project 切换时的 session 复用边界。
