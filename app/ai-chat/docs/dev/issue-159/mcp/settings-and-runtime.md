# Issue #159 ai-chat2 MCP 设置、OAuth 和运行时计划

本文档固定 `app/ai-chat2` MCP 总设计和阶段 1 设计记录。阶段状态、通用原则和项目管理边界见 `README.md`；阶段 2 开发计划见 `phase-2.md`。本文档只描述 `ai-chat2` 的实现路径，不改变 legacy `app/ai-chat`。

## 范围

长期目标不是做一个只支持静态 header 的窄版 MCP。目标仍是完整接入 MCP 设置 UI、stdio/streamable HTTP 运行时、rmcp OAuth、MCP 工具注册、工具审批/恢复，以及持久化事实记录。

2026-06-24 V1 已确认先收窄为 `config.toml only`：MCP server definitions 的 source of truth 只放在 `config.toml`，不新增 SQLite 表，也不把 OAuth token 写入 TOML。V1 需要打通非 OAuth MCP server 的配置读取、Settings 管理 UI、按需连接、tools/list 和 run 前工具注册。OAuth browser flow/token storage/refresh/scope upgrade/logout、ClientCredentials UI、prewarm 和 MCP approval resume 先记录为后续项，不在本轮实现里用半成品兜底。

V1 必须完成：

- Settings 新增 `MCP` tab，用于查看 `config.toml` 中声明的 MCP server、搜索、测试连接、展示 connected status、auth status、server info 和 tools/list。
- Settings MCP tab 必须提供 Add/Edit/Delete/Enable/Disable 管理能力，并写回 `config.toml`；编辑未暴露的 OAuth 字段时必须保留原 TOML 中已存在的 OAuth definition，不允许静默删除。
- 支持 `stdio` 和 `streamable_http` 两类传输。
- streamable HTTP V1 支持 env-backed bearer/static headers；OAuth server definition 可在 TOML 中声明并参与校验，但本轮不执行 browser OAuth flow。
- agent run 启动前连接 enabled MCP server，拉取 tools/list，把工具注册进现有 `ToolRegistry`。
- MCP tool invocation 继续通过 `ToolSource::Mcp { server_id }`、raw tool name、runtime tool name、arguments、result、error 和 approval 写入现有 fresh DB。
- `AgentRunInput` 不写入 MCP config snapshot/hash；MCP config 只存在于 `config.toml` 和 runtime-only session identity。

V1 不做：

- MCP Prompts、Resources、Sampling、Elicitation UI。
- 项目级 MCP 配置。2026-06-25 已确认阶段 2 仍不做 project-level MCP definitions、project trust prompt 或 project-scoped session key；后续如需支持，单独开阶段计划。
- app 启动即自动拉起所有 enabled stdio server。默认只在 Settings `Test`/`Refresh` 或 agent run 前按需连接。
- OAuth browser flow、token storage、refresh、scope upgrade 和 logout 在 V1 不做；阶段 2 已补齐 authorization-code browser flow、GPUI credentials token storage、rmcp `AuthClient` runtime、refreshed credentials mirror、取消授权和基础 scope failure 状态。完整 `Upgrade Access` 增量 scope flow 仍作为后续 advanced path。
- OAuth 配置表单首版不做；Add/Edit dialog 首版改为 Codex 风格的最小必需配置表单，只覆盖非 OAuth stdio / streamable HTTP 的常用字段，并且所有数组/映射字段都使用结构化 row editor，不再让用户输入多行字符串再由 app 解析。已有 OAuth definition 可以继续从 TOML 读取、展示 auth status、参与校验/快照，但 UI 暂不编辑 OAuth 字段。
- MCP server 的 `required`、timeouts、tool allow/deny、per-server/per-tool approval override 等高级字段首版不在 Settings Add/Edit dialog 暴露；如果原 TOML 已存在这些字段，编辑保存必须保留。后续如需高级配置，先设计单独 Advanced path，不要混入默认表单。
- `ClientCredentials` UI 不做；TOML 解析和校验保留为后续 advanced path。
- Codex-style composer prewarm 首版不做；不会在 composer 首次非空输入时预启动 MCP server。
- MCP tool approval resume 不在 V1 中完成；在 source-neutral resume 落地前，如果本次 run 的 ChatForm 审批模式或 TOML advanced override 让 MCP tool 进入 `prompt` approval，仍不能完成 approve-and-resume。默认 UI 不提供每个 MCP server 单独审批设置，用户需要用 ChatForm 审批模式控制默认行为；TOML advanced override 只作为后续/高级路径保留。

## 当前代码事实

已有能力：

- `Cargo.toml` 已声明：
  - `rig-core = { version = "0.39.0", features = ["rmcp"] }`
  - `rmcp = { version = "1.8.0", features = ["auth", "client", "macros", "transport-child-process", "transport-streamable-http-client-reqwest"] }`
- `crates/ai-chat-agent/src/mcp.rs` 已有 agent-side MCP 配置、session manager 和工具注册：
  - `McpConfigLayer`
  - `McpServerConfig`
  - `McpServerTransport::{Stdio, StreamableHttp}`
  - `McpServerRuntimeConfig`
  - `McpSessionManager`
  - `McpRuntimeEvent`
  - `McpConnector::register_rmcp_tools(...)`
- `app/ai-chat2/src/state/config.rs` 已有 `[mcp_servers.<id>]` 配置入口，并能转换为 `McpConfigLayer` 和 `McpServerRuntimeConfig`。
- `app/ai-chat2/src/state/mcp.rs` 已安装 `McpRuntimeGlobal`，持有 live `McpSessionManager`、server status cache 和 run setup 入口。
- `state::mcp` 的 run setup 已按 `required` 分流 app-layer preflight error：required server 配置/环境错误会阻断本次 run 并持久化 setup failure；non-required server 会记录 failed status 并跳过，不阻断其它 MCP server 或 provider call。
- `app/ai-chat2/src/features/settings/mcp.rs` 已实现 MCP Settings V1：读取 `config.toml` server、搜索、刷新/测试连接、展示 transport/connection/auth tags、server info 和 tools/list，并提供 Add/Edit/Delete/Enable/Disable 管理入口。
- `app/ai-chat2/src/features/settings/mcp/dialog.rs` 已实现 Add/Edit dialog、Delete confirm、非 OAuth stdio / streamable HTTP 简化字段表单、结构化参数/env/header rows、滚动内容区和固定 footer，并通过 `state::config` helper 写回 `config.toml`；编辑 HTTP server 时会保留首版未暴露的 OAuth definition。
- `ConversationRuntimeStore` 已把 run setup 移到 async start phase，在 `AgentRuntime::begin_run(...)` 之前调用 MCP runtime 准备工具；MCP config 不进入 `AgentRuntimeSnapshot`。
- `crates/ai-chat-agent/src/tool_registry.rs` 已支持 `ToolSource::Mcp { server_id }`，并会为 MCP 工具分配 namespace。
- `ai-chat-db` 已能持久化 MCP tool invocation 的 source、server id、original tool name、runtime tool name、arguments、result、error 和 approval。
- `ConversationRuntimeStore` 已能启动/停止 run、处理 approval、把 runtime events 映射成 conversation UI refresh。

阶段 2 已补齐：

- MCP approval resume 已从 local-only 扩展为 source-neutral runtime tool execution；批准 MCP tool 后会用 run snapshot 恢复 registry 并执行对应 MCP tool。
- OAuth authorization-code flow 已接入 Settings 简化 UI：`需要 OAuth` 开关、授权状态卡片、`授权` / `重新授权` 按钮。
- OAuth token 写入 GPUI credentials，key 使用 `mcp-oauth:{canonical_server_uri}`；关闭 OAuth 并保存、删除 server、OAuth URL 变化时会删除对应 credentials。
- agent streamable HTTP OAuth path 已用 rmcp `AuthClient`；Settings Test / agent run setup 会读取 GPUI credentials 中的 rmcp `StoredCredentials`，只注入 runtime config，不进入 TOML、SQLite 或 `AgentRunInput`。

尚未完成：

- refreshed credentials mirror 回 GPUI credentials 已完成：agent-side mirror `CredentialStore` 在 rmcp refresh 保存新 credentials 时发 runtime event，app 收到后写回 GPUI credentials。
- OAuth 授权卡片已提供显式 `取消授权`；insufficient scope 已映射为“需要重新授权”状态。完整 `Upgrade Access` 增量 scope flow 仍是后续 advanced path。
- `notifications/tools/list_changed` 已能更新 Settings 状态；复用已有 session 前会重新执行 `tools/list`，避免下一次 run 使用 stale tool cache。
- `default_tool_policy()` 默认仍只包含 `ToolSource::Local`；MCP run setup 只在实际连接成功后把对应 `ToolSource::Mcp { server_id }` 加入本次 run snapshot，不改变全局默认策略。

## 协议和源码参考

### MCP 官方协议

2026-06-24 验证：`https://modelcontextprotocol.io/specification` 当前重定向到 [2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)，页面标记为 latest。相关页面：

- [Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)：client 先发送 `initialize`，server 返回 capability 和 server info，client 再发 `notifications/initialized`。
- [Transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)：当前应实现 `stdio` 和 `streamable_http`。旧 SSE 不作为目标。
- [Tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)：server 通过 `tools/list` 暴露工具，通过 `tools/call` 执行工具；server 可以发 `notifications/tools/list_changed`。host UI 必须清楚展示暴露给模型的 tools，并提供确认/拒绝能力。
- [Authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)：HTTP server 可以要求 OAuth。ai-chat2 通过 `rmcp` 的 `auth` feature 实现，不手写 OAuth state machine。

### rmcp 和 Rig

当前 workspace 锁定版本的参考路径：

- `/Users/sushao/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rig-core-0.39.0/src/tool/rmcp.rs`
- `/Users/sushao/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rig-core-0.39.0/src/tool/server.rs`
- `/Users/sushao/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rig-core-0.39.0/src/agent/builder.rs`
- `/Users/sushao/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rmcp-1.8.0/src/transport/streamable_http_client.rs`
- `/Users/sushao/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rmcp-1.8.0/src/transport/auth.rs`
- `/Users/sushao/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rmcp-1.8.0/src/transport/common/auth/streamable_http_client.rs`

结论：

- `rmcp` 已提供 client transport：
- `TokioChildProcess` 用于 stdio。
- `StreamableHttpClientTransport` 用于 streamable HTTP。
  - `AuthClient<C>` 包装任意 `StreamableHttpClient`，在请求前调用 `AuthorizationManager::get_access_token()`。
- `rmcp::transport::auth` 已提供：
  - `OAuthState`
  - `AuthorizationManager`
  - `AuthorizationSession`
  - `AuthClient`
  - `ScopeUpgradeConfig`
  - `StoredCredentials`
  - `ClientCredentialsConfig`
- `OAuthState::start_authorization(...)` 会做 discovery、scope selection、dynamic registration 或 URL-based client id。
- `OAuthState::handle_callback_url(...)` 会完成 code exchange，并校验 state / optional issuer。
- `AuthorizationManager::refresh_token()` 和 `get_access_token()` 支持 refresh。
- `AuthorizationManager::request_scope_upgrade(...)` 支持 403 insufficient scope 后重新授权。
- `rmcp` HTTP config 会处理 reserved headers 和 session/protocol version header。ai-chat2 不应让用户覆盖 `accept`、`content-type`、`mcp-session-id`、`mcp-protocol-version` 等协议 header。
- `rig_core::tool::rmcp::McpTool` 已能把 `rmcp::model::Tool` 包装为 Rig tool；本仓库 `McpConnector` 已利用这一点注册工具。
- Rig 也有 MCP `ToolServerHandle` 风格的动态 tool server，但 ai-chat2 当前 persistence 和 approval 都围绕自己的 `ToolRegistry`、`RegisteredToolDefinition`、`tool_invocations`。本阶段继续走现有 `ToolRegistry`，不要切换到 Rig 的 tool server。

### Zed 和 Codex

Zed 参考路径：

- `/Users/sushao/.cargo/git/checkouts/zed-a70e2ad075855582/1d217ee/crates/project/src/context_server_store.rs`
- `/Users/sushao/.cargo/git/checkouts/zed-a70e2ad075855582/1d217ee/crates/zed_credentials_provider/src/zed_credentials_provider.rs`
- `/Users/sushao/Documents/code/zed/docs/src/ai/mcp.md`

Codex 参考路径：

- `/Users/sushao/Documents/code/codex/codex-rs/rmcp-client/src/rmcp_client.rs`
- `/Users/sushao/Documents/code/codex/codex-rs/rmcp-client/src/oauth.rs`
- `/Users/sushao/Documents/code/codex/codex-rs/rmcp-client/src/perform_oauth_login.rs`
- `/Users/sushao/Documents/code/codex/codex-rs/core/src/mcp_skill_dependencies.rs`

Codex Electron app 参考路径，2026-06-24 已在本机安装的 app 上验证：
`com.openai.codex` version `26.616.81150` / build `4306`:

- `/Applications/Codex.app/Contents/Resources/app.asar`
- 解包分析副本：`/private/tmp/codex-asar-build`
- `webview/assets/use-start-new-conversation-BwhKpfnA.js`：New Chat 只派发导航。
- `webview/assets/composer-DlMDPaCL.js`：composer 非空时 prewarm，提交时发送 `start-conversation`。
- `webview/assets/thread-context-inputs-B6tQCr7t.js`：消费匹配的 prewarmed thread，否则回退到 start thread。
- `.vite/build/main-dSxbxAhH.js`：处理 `thread-prewarm-start`。
- `.vite/build/src-DBVh5FZA.js`：把 prewarm 转发给 app-server state，并在第一轮 turn 前抑制 `thread/started`。

已采纳决策：

- Config shape 继续支持 Codex 风格和后续高级能力：`enabled`、`required`、timeouts、tool allow/deny、per-tool approval、env-backed headers。Settings 默认 Add/Edit UI 只暴露 Codex 自定义 MCP 常用字段，不把完整 TOML schema 全部推给用户。
- OAuth token persistence 采用 Zed/Codex 边界：rmcp `StoredCredentials` JSON 存到 system keychain via GPUI credentials，不写入 `config.toml`，也不写入 chat DB table。
- 后续 browser login flow 参考 Codex：本地 callback server、redirect URI、open browser、等待 callback、持久化 token。
- Session/tool runtime 沿用当前 ai-chat-agent persistence model，不照搬 Zed 的整套 context-server store。
- 默认 startup timeout 为 30s，默认 tool timeout 为 300s。
- 生命周期参考 Codex Electron，不参考 Codex CLI/TUI。`New Chat` 本身不启动 stdio MCP server；submit/test/connect 是第一批会创建 runtime session 的用户可见时机。
- 2026-06-24 已确认：V1 先做 `config.toml only` 的 MCP 管理 UI 和非 OAuth MCP runtime。2026-06-25 阶段 2 已接入 authorization-code browser flow、GPUI credentials token storage、rmcp `AuthClient` runtime、refresh mirror、取消授权和基础 scope failure 状态；完整 `Upgrade Access` 增量 scope flow 与 `ClientCredentials` UI 仍保持后续 advanced path。
- 2026-06-24 已确认：V1 不做 Codex-style prewarm。
- 2026-06-26 已确认：`AgentRunInput` 不持久化 MCP config snapshot/hash；approval resume 使用当前 MCP runtime/config，必要时要求用户 retry/resend。

### 启动和复用策略

首版实现策略：

- App 启动时不启动 enabled MCP server。
- 打开 Settings 时不启动所有 enabled MCP server。
- Settings `Test` 只启动当前选中的 server，执行 initialize + `tools/list`，并且不 prune 其它 live sessions。
- V1 不提供 `Connect OAuth`；阶段 2 已改为在 HTTP Add/Edit dialog 中提供 `需要 OAuth` 开关和显式 `授权` / `重新授权` / `取消授权` 动作。
- 首次 agent submit 在 run setup 阶段连接 enabled server，并且必须发生在 provider tool declaration finalized 之前。
- 同一个 app process 内，如果 server id 和 runtime-only fingerprint 匹配，已有 live session 可以跨 conversation 复用。
- config 变化、server disabled、app 退出、显式 disconnect/logout 时关闭 stale session。
- 不要求跨进程复用。
- 首个 MCP PR 不做 Codex-style composer prewarm。后续只有在 OAuth status、错误展示和 retry/resend 稳定后，才能重新设计受控 prewarm。

## 依赖计划

更新 workspace `rmcp` feature set：

```toml
rmcp = { version = "1.8.0", features = [
  "auth",
  "client",
  "macros",
  "transport-child-process",
  "transport-streamable-http-client-reqwest",
] }
```

只在 app/agent 代码真实使用时新增直接依赖：

```toml
http = "1.4.2"
rmcp.workspace = true
tokio = { version = "1.52.3", features = ["io-util", "net", "sync", "time"] }
url = "2.5.8"
```

用途：

- `http`：仅当 `features/settings/mcp/validation.rs` 直接解析 `HeaderName` / `HeaderValue` 时加到 `app/ai-chat2`。
- `rmcp`：`app/ai-chat2` 直接使用 rmcp OAuth `AuthorizationManager`、`OAuthState`、`StoredCredentials` 和 callback 类型。
- `tokio`：`app/ai-chat2` 的 `state::mcp` 用 `tokio::sync::Mutex` 持有 live `McpSessionManager`，OAuth loopback callback 用 `tokio::net` / `tokio::io` / `tokio::time`；实际异步任务仍通过 `gpui_tokio::Tokio::spawn(...)` 跑在 GPUI 管理的 Tokio runtime 上。
- `url`：加到 `app/ai-chat2`，用于 URL validation、OAuth callback URL parsing 和 canonical OAuth storage key。`crates/ai-chat-agent` 现有直接依赖 `url = "2.5.8"` 保留给 agent-side URL 工作。

V1 暂不新增：

- `oauth2 = "5.0.0"`：只有 OAuth status 代码需要直接导入 `TokenResponse` 时再加。
- 本地 OAuth callback server 当前使用 `tokio::net::TcpListener` 实现；不新增 `tiny_http` 或 `axum`。

不要新增 `webbrowser`；使用 GPUI `cx.open_url(&authorization_url)`，让浏览器打开动作留在 desktop app platform 边界内。

不要新增 `keyring`；使用现有 GPUI credentials API：

- `cx.write_credentials(...)`
- `cx.read_credentials(...)`
- `cx.delete_credentials(...)`

结构化 Add/Edit 表单本身不新增依赖。不要为了把用户输入的 shell 字符串拆成数组而新增 `shell-words` / `shlex` 之类库；UI source of truth 直接是 `Vec<String>` 和 key/value row draft。row id 使用 dialog-local monotonic `u64`，不新增 `uuid`。

## 配置模型

MCP server definitions stay in `config.toml`; secrets and OAuth tokens never go into TOML.

示例：

```toml
[mcp_servers.filesystem]
enabled = true
required = false
display_name = "Filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
cwd = "/tmp"
startup_timeout_ms = 30000
tool_timeout_ms = 300000
default_tools_approval_mode = "prompt"

[mcp_servers.filesystem.env]
NODE_ENV = "production"

[mcp_servers.github]
enabled = true
required = false
display_name = "GitHub"
transport = "streamable_http"
url = "https://api.githubcopilot.com/mcp/"
startup_timeout_ms = 30000
tool_timeout_ms = 300000
enabled_tools = ["search_issues", "create_issue"]
disabled_tools = ["delete_repository"]
default_tools_approval_mode = "prompt"

[mcp_servers.github.headers]
X-Client = "ai-chat2"

[mcp_servers.github.env_headers]
X-Workspace-Token = "WORKSPACE_TOKEN"

[mcp_servers.github.oauth]
flow = "authorization_code_pkce"
scopes = ["read:user", "repo"]
resource = "https://api.githubcopilot.com/mcp/"
callback_port = 0

[mcp_servers.github.tools.create_issue]
approval_mode = "prompt"
```

按传输类型区分的必填字段：

- `stdio` 要求 `command`。
- `streamable_http` 要求 `url`。

共享字段：

- `enabled: bool`，默认 `true`。
- `required: bool`，默认 `false`。
- `display_name: Option<String>`.
- `startup_timeout_ms: Option<u64>`，默认 `30000`。
- `tool_timeout_ms: Option<u64>`，默认 `300000`。
- `enabled_tools: Option<Vec<String>>`，按 raw MCP tool name 表示的 allow-list。
- `disabled_tools: Vec<String>`，按 raw MCP tool name 表示的 deny-list。
- `default_tools_approval_mode: Option<McpToolApprovalMode>`.
- `tools: BTreeMap<String, McpToolOverrideTomlConfig>`.

Stdio 字段：

- `command: String`.
- `args: Vec<String>`.
- `env: BTreeMap<String, String>`，保存 literal environment variables。
- `env_vars: Vec<String>`，声明从 app process environment 继承的变量名。
- `cwd: Option<PathBuf>`.

Streamable HTTP 字段：

- `url: String`.
- `headers: BTreeMap<String, String>`，保存非 secret literal headers。
- `env_headers: BTreeMap<String, String>`，把 header name 映射到 environment variable name。
- `bearer_token_env_var: Option<String>`，用于 `Authorization: Bearer <value>`。
- `oauth: Option<McpOAuthTomlConfig>`.

OAuth 字段：

```rust
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "flow", rename_all = "snake_case")]
pub(crate) enum McpOAuthTomlConfig {
    AuthorizationCodePkce {
        #[serde(default)]
        scopes: Vec<String>,
        client_id: Option<String>,
        client_metadata_url: Option<String>,
        resource: Option<String>,
        callback_port: Option<u16>,
        callback_url: Option<String>,
    },
    ClientCredentials {
        client_id: String,
        client_secret_env_var: String,
        #[serde(default)]
        scopes: Vec<String>,
        resource: Option<String>,
    },
}
```

`AuthorizationCodePkce` 是默认 UI 路径。`ClientCredentials` 是 TOML 支持的 advanced 路径；主 browser flow 稳定后 UI 再暴露。

### Settings Add/Edit 字段边界

Settings Add/Edit dialog 不等同于 TOML schema editor。TOML 仍然是完整 source of truth，但默认 UI 只暴露用户配置自定义 MCP 时最常用、最容易理解的字段。

`stdio` UI 字段：

| UI 字段 | TOML 字段 | Rust draft 类型 | 规则 |
| --- | --- | --- | --- |
| 名称 | config key / `server_id` | `Entity<InputState>` | 创建时必填、唯一；编辑时默认不可改名，后续 rename flow 单独做。 |
| 启动命令 | `command` | `Entity<InputState>` | 必填，保存为单个 command，不解析 shell 字符串。 |
| 参数 | `args` | `Vec<StringListDraftRow>` | 结构化数组，一行一个参数；不从空格或多行字符串解析。 |
| 环境变量 | `env` | `Vec<KeyValueDraftRow>` | 结构化 key/value 数组，保存 literal env。 |
| 环境变量传递 | `env_vars` | `Vec<StringListDraftRow>` | 结构化数组，一行一个 process env var name。 |
| 工作目录 | `cwd` | `Entity<InputState>` | 可选；非空时保存 `PathBuf` 字符串。 |

`streamable_http` UI 字段：

| UI 字段 | TOML 字段 | Rust draft 类型 | 规则 |
| --- | --- | --- | --- |
| 名称 | config key / `server_id` | `Entity<InputState>` | 与 stdio 相同。 |
| URL | `url` | `Entity<InputState>` | 必填，只允许 `http` / `https`。 |
| Bearer token 环境变量 | `bearer_token_env_var` | `Entity<InputState>` | 保存环境变量名，不保存 token 明文。 |
| HTTP headers | `headers` | `Vec<KeyValueDraftRow>` | 结构化 header name/value 数组。 |
| 来自环境变量的 headers | `env_headers` | `Vec<KeyValueDraftRow>` | 结构化 header name/env var name 数组。 |

默认 UI 不显示但必须保留的 TOML 字段：

- `display_name`：首版 UI 以 config key 作为名称；如果旧 TOML 已有 `display_name`，编辑保存必须原样保留，后续再决定是否增加“显示名称”字段。
- `enabled`：在 Settings 列表行用 switch 管理，不放入 Add/Edit dialog。
- `required`：首版不暴露；保留 TOML 值。
- `startup_timeout_ms` / `tool_timeout_ms`：首版不暴露；保留 TOML 值，默认仍为 30s / 300s。
- `enabled_tools` / `disabled_tools` / `tools`：首版不暴露；保留 TOML 值。
- `default_tools_approval_mode`：首版不暴露；MCP tool 的默认审批模式继承 ChatForm 当前 run 的审批模式。
- `oauth`：首版不暴露；保留 TOML definition，并在后续 OAuth runtime/UI 中接入。

保存行为必须是 merge，而不是重建整段 TOML 后丢弃隐藏字段。创建新 server 时使用字段默认值；编辑已有 server 时只替换当前 UI 暴露字段。

### Rust 配置类型

扩展 `app/ai-chat2/src/state/config.rs`：

```rust
pub(crate) struct McpServerTomlConfig {
    pub(crate) enabled: bool,
    pub(crate) required: bool,
    pub(crate) display_name: Option<String>,
    pub(crate) transport: McpTransportKind,
    pub(crate) command: Option<String>,
    pub(crate) args: Vec<String>,
    pub(crate) url: Option<String>,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) env_headers: BTreeMap<String, String>,
    pub(crate) bearer_token_env_var: Option<String>,
    pub(crate) oauth: Option<McpOAuthTomlConfig>,
    pub(crate) env: BTreeMap<String, String>,
    pub(crate) env_vars: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) startup_timeout_ms: Option<u64>,
    pub(crate) tool_timeout_ms: Option<u64>,
    pub(crate) enabled_tools: Option<Vec<String>>,
    pub(crate) disabled_tools: Vec<String>,
    pub(crate) default_tools_approval_mode: Option<McpToolApprovalMode>,
    pub(crate) tools: BTreeMap<String, McpToolOverrideTomlConfig>,
}

pub(crate) enum McpToolApprovalMode {
    Auto,
    Prompt,
    Deny,
}

pub(crate) struct McpToolOverrideTomlConfig {
    pub(crate) approval_mode: Option<McpToolApprovalMode>,
}
```

映射规则：

- `McpServerTomlConfig::to_agent_config(...)` 只把 transport/launch/auth-neutral 字段映射到 `ai_chat_agent::McpServerConfig`。
- Tool filters、required flag、approval policy、OAuth status 和 UI 字段属于 app/runtime layer。
- 不要把更丰富的 app state 硬塞进现有最小 agent config。

## 模块结构

### ai-chat-agent

保留 `crates/ai-chat-agent/src/mcp.rs` 作为模块入口，并在 `crates/ai-chat-agent/src/mcp/` 下新增子模块：

```text
crates/ai-chat-agent/src/mcp.rs
crates/ai-chat-agent/src/mcp/client.rs
crates/ai-chat-agent/src/mcp/config_hash.rs
crates/ai-chat-agent/src/mcp/oauth.rs
crates/ai-chat-agent/src/mcp/session.rs
crates/ai-chat-agent/src/mcp/status.rs
crates/ai-chat-agent/src/mcp/tests.rs
```

不新增 `mod.rs`。

新增 public/runtime types：

```rust
pub struct McpSessionManager {
    sessions: BTreeMap<McpSessionKey, McpServerSession>,
    connector: McpConnector,
}

pub struct McpSessionKey {
    pub server_id: String,
    pub fingerprint: String,
}

pub struct McpServerSession {
    pub sink: rmcp::service::ServerSink,
    pub service: McpRunningService,
    pub auth_manager: Option<Arc<Mutex<rmcp::transport::auth::AuthorizationManager>>>,
    pub tools: Vec<McpToolSnapshot>,
    pub status: McpServerStatusSnapshot,
}

pub struct McpRunningService {
    service: rmcp::service::RunningService<rmcp::RoleClient, McpClientHandler>,
}

pub struct McpClientHandler;

pub struct McpServerRuntimeConfig {
    pub server: McpServerConfig,
    pub required: bool,
    pub startup_timeout: Duration,
    pub tool_timeout: Duration,
    pub enabled_tools: Option<BTreeSet<String>>,
    pub disabled_tools: BTreeSet<String>,
    pub default_approval_policy: ToolApprovalPolicy,
    pub execution_policy: ToolExecutionPolicy,
}

pub struct McpServerStatusSnapshot {
    pub server_id: String,
    pub display_name: Option<String>,
    pub transport: McpServerTransportKindSnapshot,
    pub state: McpServerConnectionState,
    pub auth: McpOAuthStatusSnapshot,
    pub server_info: Option<McpServerInfoSnapshot>,
    pub tools: Vec<McpToolSnapshot>,
    pub last_error: Option<String>,
}

pub enum McpServerConnectionState {
    Disabled,
    NotConnected,
    Connecting,
    Connected,
    Failed,
}

pub enum McpOAuthStatusSnapshot {
    NotConfigured,
    SignedOut,
    SigningIn,
    Authorized {
        scopes: Vec<String>,
        expires_at_unix_ms: Option<u64>,
    },
    AuthorizationRequired,
    ScopeUpgradeRequired {
        required_scope: String,
        authorization_url: String,
    },
    Failed {
        message: String,
    },
}

pub struct McpPreparedTools {
    pub statuses: Vec<McpServerStatusSnapshot>,
}

pub enum McpRuntimeEvent {
    ServerStatusChanged(McpServerStatusSnapshot),
    ToolsChanged { server_id: String, tools: Vec<McpToolSnapshot> },
    OAuthChanged { server_id: String, status: McpOAuthStatusSnapshot },
}
```

职责：

- 从 `McpServerConfig` 构建 rmcp transport。
- 初始化 MCP lifecycle。
- 拉取 `tools/list`。
- 处理 `notifications/tools/list_changed`：刷新 status cache，并通知 app state。
- 实现 `McpClientHandler` 作为 rmcp client service，用于接收 server notifications；最低要求是把 `tools/list_changed` 转发为 `McpRuntimeEvent`。
- 通过 `McpConnector` 把允许的 tools 注册到传入的 `ToolRegistry`。
- 在工具可能被调用期间保持 `ServerSink` / running service 存活。
- config 变化、app 退出或 server disabled 时关闭 stdio child process。
- 对 OAuth streamable HTTP session，用 `rmcp::transport::auth::AuthClient` 包装 streamable client。

### ai-chat2 状态

新增：

```text
app/ai-chat2/src/state/mcp.rs
app/ai-chat2/src/state/mcp_oauth.rs
```

不新增 `mod.rs`。

状态类型：

```rust
pub(crate) struct McpRuntimeGlobal(Entity<McpRuntimeStore>);

pub(crate) struct McpRuntimeStore {
    sessions: BTreeMap<String, McpServerSessionState>,
    rows: Vec<McpServerStatusRow>,
    selected_server_id: Option<String>,
    refresh_task: Option<Task<()>>,
    server_tasks: BTreeMap<String, Task<()>>,
    oauth_flows: BTreeMap<String, McpOAuthFlowState>,
    _subscriptions: Vec<Subscription>,
}

pub(crate) struct McpServerStatusRow {
    pub(crate) server_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) required: bool,
    pub(crate) transport: McpTransportKind,
    pub(crate) connection: McpConnectionState,
    pub(crate) auth: McpOAuthStatus,
    pub(crate) tool_count: usize,
    pub(crate) last_error: Option<String>,
}

pub(crate) enum McpOAuthFlowState {
    Idle,
    Starting,
    WaitingForBrowser {
        authorization_url: String,
        callback_url: String,
        task: Task<()>,
    },
    Exchanging,
    Failed(String),
}
```

职责：

- 从 `AiChat2ConfigStore` 派生 server rows。
- 持有 UI 可见的 connection/auth status。
- 观察 config 变化并关闭 stale sessions。
- 暴露 actions：
  - `refresh_all_servers`
  - `test_server(server_id)`
  - `connect_oauth(server_id)`
  - `cancel_oauth(server_id)`
  - `disconnect_oauth(server_id)`
  - `refresh_oauth_token(server_id)`
  - `disconnect_server(server_id)`
  - `prepare_tool_registry_for_run(&mut AgentRunRequest)`
  - `persist_oauth_if_needed(server_id)`
- config 持久化仍在 `state::config`；MCP runtime state 不持久化。

初始化：

- `state::mcp::init(cx)` 在 `state::config::init(cx)` 之后安装 `McpRuntimeGlobal`。
- App launch 不得 eager start 每个 stdio server。
- 首次 run 时按需连接 enabled servers，并在 runtime-only fingerprint 匹配时复用 live sessions。

全局数据管理：

- `AiChat2ConfigStore = SharedStore<AiChat2Config, AiChat2ConfigBackend>` 仍是 MCP definitions 的 source of truth，并负责保存 `config.toml`。
- `McpRuntimeGlobal(Entity<McpRuntimeStore>)` 持有 connection state、OAuth flow state、tool-list cache 和 live rmcp sessions。
- `McpRuntimeStore` 观察 `AiChat2ConfigStore`；它不拥有 persisted config。
- Settings UI 同时观察 config 和 runtime store。MCP definitions 不直接访问 `ai-chat-db`。
- Conversation runtime 请求 `McpRuntimeStore` 为 run 准备 tools；它不直接创建 rmcp transports。
- `AgentRuntimeSnapshot` 不保存 MCP config snapshot/hash；MCP runtime state 不持久化到 run input。
- OAuth credentials 通过 GPUI credentials 读写，不 mirror 到 config 或 SQLite。

### OAuth 存储

直接使用 GPUI credentials。不要引入 DB table 或 keyring crate。

Storage key：

```text
mcp-oauth:{server_id}:{canonical_resource_url}
```

Credential username：

```text
mcp-oauth
```

存储的 JSON：

```rust
pub(crate) struct McpStoredOAuthSession {
    pub(crate) server_id: String,
    pub(crate) url: String,
    pub(crate) client_id: String,
    pub(crate) token_response: rmcp::transport::auth::OAuthTokenResponse,
    pub(crate) granted_scopes: Vec<String>,
    pub(crate) token_received_at: Option<u64>,
    pub(crate) expires_at_unix_ms: Option<u64>,
}
```

运行时行为：

- Settings login flow 通过 rmcp `OAuthState` / `AuthorizationManager` 获得 `StoredCredentials`。
- 通过 `cx.write_credentials(...)` 存储序列化后的 rmcp `StoredCredentials`。
- 创建 OAuth HTTP transport 时，app 通过 `cx.read_credentials(...)` 读取已存 credentials，注入 agent runtime-only `oauth_credentials`；agent 侧 seed `AuthorizationManager` + `InMemoryCredentialStore`，再构建 `AuthClient`。
- transport 使用过程中 `AuthorizationManager` 可能刷新 token；agent-side mirror `CredentialStore` 会把 refreshed `StoredCredentials` 通过 runtime event 发回 app，并写入 GPUI credentials。
- 授权卡片的 `取消授权`、关闭 OAuth 并保存、删除 server、OAuth URL 变化都会调用 `cx.delete_credentials(...)` 并丢弃 stale session。

本地 callback 服务：

- 在 `app/ai-chat2/src/state/mcp_oauth.rs` 中用 `tokio::net::TcpListener` 实现。
- 绑定 `127.0.0.1:<callback_port>`；`0` 表示由 OS 分配端口。
- 默认 callback path：`/callback`。
- 支持 `callback_url` override，以兼容要求预注册 redirect URI 的 provider。Listener 仍然本地绑定；override 只控制 redirect URI。
- 解析 `code`、`state`、`iss`、`error`、`error_description`。
- 尽可能把完整 redirect URL 传给 `OAuthState::handle_callback_url(...)`。
- 返回一个很小的本地化成功/失败页面。app UI 仍是 source of truth。

## Conversation Runtime 接线

当前问题：

- `ConversationRuntimeStore::start_run` 目前同步调用 `AgentRuntime::begin_run(&mut request, None)`。
- `begin_run` 会注册 built-in tools，并 finalize tool names。
- MCP connection 是 async，而且必须发生在最终 tool declarations 发给 provider 之前。

必须重构：

1. 把 run setup 移到 `ConversationRuntimeStore` 内部的 async start phase。
2. 继续在 `state/conversations.rs` 按现有方式构建 `AgentRunRequest`。
3. 调用 `state::mcp::runtime(cx).prepare_tool_registry_for_run(&mut request).await`。
4. 在该方法内部：
   - 计算 runtime-only per-server fingerprint，
   - 连接 enabled servers，
   - 必要时执行 OAuth refresh，
   - 拉取 allowed tool list，
   - 把 tools 注册到 `request.tool_registry`，
5. `AgentRuntime::begin_run(...)` 注册 built-in tools 后，只 finalize 一次所有 tool names。
6. required server 失败时持久化 setup failure。

更新 core payload model：

```rust
pub struct AgentRuntimeSnapshot {
    pub engine: AgentEngineKind,
    pub engine_version: String,
    pub skill_catalog_hash: Option<String>,
    pub tool_name_strategy: ToolNameStrategy,
}
```

MCP config 不进入 core payload model；如果未来需要审计展示，必须新增 redacted/audit-only shape，不能复用 runtime config。

失败策略：

- non-required server 失败时展示 settings/runtime warning，并跳过该 server。
- required server 失败时，在 provider call 前让 setup fail，并持久化 agent run error。
- 如果 OAuth required 但未授权，non-required server 跳过；required server 以 actionable error 失败。
- tool call 或连接期间如果遇到 insufficient scope，当前阶段把 server OAuth status 更新为 `ScopeUpgradeRequired`，Settings 显示“需要重新授权”，用户重新授权后 retry/resend。完整 `Upgrade Access` 增量 scope flow 后续单独设计。

审批恢复：

- 用 source-neutral executor 替换当前 local-only resume path：
  - local built-in tools 路由到当前 built-in executor，
  - MCP tools 路由到 `McpRuntimeStore` / `McpSessionManager`。
- Resume 必须：
  - 读取 `ToolInvocationRecord`，
  - 使用已存的 `server_id` 和 raw `tool_name`，
  - 使用当前 MCP config/runtime 重新准备 tools，
  - 用已存 arguments 调用同一个 MCP tool，
  - 持久化 output/error，并 append tool result item，
  - 像现在一样继续 parent run。

## 设置 UI

### 文件结构

```text
app/ai-chat2/src/features/settings.rs
app/ai-chat2/src/features/settings/mcp.rs
app/ai-chat2/src/features/settings/mcp/detail.rs
app/ai-chat2/src/features/settings/mcp/dialog.rs
app/ai-chat2/src/features/settings/mcp/form_rows.rs
app/ai-chat2/src/features/settings/mcp/form_state.rs
app/ai-chat2/src/features/settings/mcp/row.rs
app/ai-chat2/src/features/settings/mcp/tags.rs
app/ai-chat2/src/features/settings/mcp/validation.rs
```

不新增 `mod.rs`。

职责划分：

- `mcp.rs`：Settings MCP page entity、toolbar、搜索、选中 server、调用 refresh/test/create/edit/delete/enable actions。
- `detail.rs`：右侧详情面板、config summary、server info、tools/list 和 error rendering。
- `row.rs`：server list row rendering、row actions 和 enabled switch。
- `tags.rs`：transport / connection / auth / tool count tag 以及 row search text。
- `dialog.rs`：Add/Edit dialog 和 Delete confirm 的 overlay wiring、footer、save/cancel action、滚动容器。
- `form_state.rs`：Codex 风格表单 draft、自定义 row id、从 TOML config 初始化 draft、保存时 merge 回 TOML config。
- `form_rows.rs`：结构化 list/key-value row editor 的 app-local renderer。
- `validation.rs`：表单级校验和字段错误模型；复用 `state::config` 中可共享的 server id、env var、header、URL 校验 helper。
- `form_parse.rs`：下一步删除，或只保留 legacy migration/test helper；默认 UI 不再通过多行字符串解析 `args`、headers/env 或 tool overrides。

### 设置 shell 变更

- `SettingsPageKey::Mcp` 和 `SettingsView::mcp_settings` 保持现状。
- page spec：
  - title key: `settings-page-mcp`
  - search text 包含 `MCP`、`Model Context Protocol`、`OAuth`、`Tools` 和本地化关键词。
  - icon: `IconName::Plug`.
- MCP 页面保持 no outer body scroll，沿用 Provider/Skills 模式，因为该页面自己管理 list/detail 滚动。

### 页面状态

```rust
pub(super) struct McpSettingsPage {
    search_input: Entity<InputState>,
    selected_server_id: Option<String>,
    _subscriptions: Vec<Subscription>,
}
```

Add/Edit dialog 用独立 entity 管理本地 draft。draft 不直接写全局 config，只有 Save 通过校验后才 merge 回 `AiChat2ConfigStore`。

```rust
pub(super) struct McpServerEditDialogState {
    mode: McpServerEditMode,
    original_config: Option<McpServerTomlConfig>,
    draft: McpServerFormDraft,
    validation_errors: Vec<McpFormValidationError>,
    next_row_id: u64,
    scroll_handle: ScrollHandle,
}

pub(super) enum McpServerEditMode {
    Create,
    Edit { original_server_id: String },
}

pub(super) struct McpServerFormDraft {
    transport: McpTransportKind,
    server_id_input: Entity<InputState>,
    command_input: Entity<InputState>,
    cwd_input: Entity<InputState>,
    args: Vec<StringListDraftRow>,
    env: Vec<KeyValueDraftRow>,
    env_vars: Vec<StringListDraftRow>,
    url_input: Entity<InputState>,
    bearer_token_env_var_input: Entity<InputState>,
    headers: Vec<KeyValueDraftRow>,
    env_headers: Vec<KeyValueDraftRow>,
}

pub(super) struct StringListDraftRow {
    id: u64,
    input: Entity<InputState>,
}

pub(super) struct KeyValueDraftRow {
    id: u64,
    key_input: Entity<InputState>,
    value_input: Entity<InputState>,
}
```

说明：

- `server_id_input` 对用户显示为“名称”。创建时它就是 `[mcp_servers.<id>]` 的 key；编辑时默认 disabled，不做隐式 rename。
- 不再有 `display_name_input`、`enabled`、`required`、timeouts、tool allow/deny、default approval、tool overrides 的 dialog state。
- 结构化 row 使用稳定 `u64` id，避免删除/插入行后输入 state 混乱；不引入 uuid 依赖。
- 每个 row 的 `InputState` 在 dialog 创建时建立；Add row 时只创建新 row entity。

### 使用组件

使用现有 `gpui-component`：

- `gpui_component::dialog` / `WindowExt::open_dialog`：新增、编辑和删除确认。
- `gpui_component::button::Button`：新增、编辑、删除、测试、刷新、保存、取消、添加行、删除行。紧凑行操作使用 icon-only buttons，并通过 tooltip 或 aria/label 补语义。
- `gpui_component::button::{Toggle, ToggleGroup}`：transport 分段选择；只做 `stdio` / `streamable_http` 单选。
- `gpui_component::input::{Input, InputState}`：搜索、短文本字段、数组元素、key/value 单元格。
- `gpui_component::label::Label`：字段 label 和 inline error。
- `gpui_component::scroll::ScrollableElement`：详情面板和 Add/Edit dialog 内容区域；dialog 必须使用 `ScrollHandle`、`track_scroll`、`overflow_y_scroll` 和 `vertical_scrollbar`。
- `gpui_component::tag::Tag`：connection/auth/transport status labels。`Badge` 只用于 overlay counts/dots，不作为主要 status pill。
- `gpui_component::notification::{Notification, NotificationType}`：创建、保存、删除、测试失败提示，继续使用现有 notification layer。

自定义 app-local renderer：

```rust
fn render_string_list_field(
    field_id: &'static str,
    label: SharedString,
    rows: &[StringListDraftRow],
    placeholder: SharedString,
    add_label: SharedString,
    remove_row: impl Fn(u64, &mut Window, &mut App) + 'static,
    add_row: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement;

fn render_key_value_list_field(
    field_id: &'static str,
    label: SharedString,
    rows: &[KeyValueDraftRow],
    key_placeholder: SharedString,
    value_placeholder: SharedString,
    add_label: SharedString,
    remove_row: impl Fn(u64, &mut Window, &mut App) + 'static,
    add_row: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement;
```

这些 renderer 只属于 MCP form，不做成跨 app 通用组件。后续如果 prompt/provider/shortcut 也需要相同模式，再抽到 `app/ai-chat2/src/components/structured_form.rs`。

### 页面布局

工具栏：

- 搜索框使用 `IconName::Search`。
- Refresh button 使用 `IconName::RefreshCcw`。
- 新增按钮使用 `IconName::Plus`。

Server 列表：

- Server name 显示 config key，若 `display_name` 已存在可作为 secondary text，但默认 Add/Edit 不编辑它。
- Transport tag：
  - `IconName::Terminal` 表示 stdio。
  - `IconName::Cloud` 表示 streamable HTTP。
- Connection tag：disabled、not connected、connecting、connected、failed。
- Auth tag：not configured、signed out、signing in、authorized、expired、scope required、failed。
- Enabled switch 保留在列表行，作为唯一默认 UI enable/disable 入口。
- Tool count。
- Row actions：
  - Test: `IconName::RefreshCcw`
  - Edit: `IconName::Pencil`
  - Delete: `IconName::Trash`
  - 后续 OAuth actions: `IconName::LogIn` / `IconName::LogOut`

详情面板：

- Effective config summary。
- Auth status tag。阶段 2 只补 `需要 OAuth` 开关、授权状态卡片和授权/重新授权/取消授权动作；OAuth flow/scopes/resource、callback、client id 等复杂字段不进默认 UI。
- initialize 返回的 server info。
- Tool list：
  - raw tool name,
  - model-visible runtime name if connected,
  - description,
  - enabled/disabled source,
  - effective approval policy,
  - last list refresh time.
- 最近一次 startup 或 tool-list error。

### Add/Edit 弹窗

通用：

- 标题：`mcp-dialog-create-title` / `mcp-dialog-edit-title`。
- Dialog body 固定最大高度，内容区可滚动，footer 固定在底部。
- Transport 使用双段 `ToggleGroup`：`stdio`、`streamable_http`。
- 切换 transport 不清空另一类字段；Save 只校验和写入当前 transport 的字段。用户切回原 transport 时草稿仍保留。

`stdio` 字段：

- `名称`：必填，创建时可编辑，编辑时 disabled。映射 config key / server id。
- `启动命令`：必填，单行。
- `参数`：结构化数组，一行一个参数，Add row 使用 `IconName::Plus`，Remove row 使用 `IconName::Trash`。
- `环境变量`：结构化 key/value rows。
- `环境变量传递`：结构化数组，一行一个 env var name。
- `工作目录`：可选单行。

`streamable_http` 字段：

- `名称`：同 stdio。
- `URL`：必填，单行。
- `Bearer token 环境变量`：可选 env var name；不保存 bearer token 明文。
- `HTTP headers`：结构化 header name/value rows。
- `来自环境变量的 headers`：结构化 header name/env var name rows。

默认不显示：

- `display_name`。
- `required`。
- `startup_timeout_ms` / `tool_timeout_ms`。
- `enabled_tools` / `disabled_tools` / `tools`。
- `default_tools_approval_mode`。
- `oauth` 配置区域。

隐藏字段保存策略：

- Create：隐藏字段使用 `McpServerTomlConfig` 默认值；`enabled` 默认 `true`。
- Edit：`form_state.rs` 从 `original_config` clone 后只覆盖当前 UI 暴露字段；未暴露字段必须原样保留。
- Edit 从 HTTP 切到 stdio 时，`oauth` 因 transport 不再是 HTTP 而不写入有效运行时配置；如果要保留已存在 OAuth definition，必须在文档和 UI warning 中明确“当前 transport 下不会生效”。首版建议切换 transport 时仍保留原 TOML 字段但 runtime validation 只按 active transport 消费。

### 校验规则

`validation.rs` 返回结构化错误，dialog 顶部显示 summary，字段下方显示相关错误。

```rust
pub(super) struct McpFormValidationError {
    pub(super) field: McpFormField,
    pub(super) message_key: &'static str,
    pub(super) args: Vec<(&'static str, SharedString)>,
}

pub(super) enum McpFormField {
    ServerId,
    Command,
    Argument { row_id: u64 },
    EnvKey { row_id: u64 },
    EnvValue { row_id: u64 },
    EnvVar { row_id: u64 },
    Cwd,
    Url,
    BearerTokenEnvVar,
    HeaderName { row_id: u64 },
    HeaderValue { row_id: u64 },
    EnvHeaderName { row_id: u64 },
    EnvHeaderVar { row_id: u64 },
}
```

通用规则：

- 名称必填、trim 后非空、唯一。
- 名称必须满足现有 server id 规则；首版使用 `^[A-Za-z0-9_-]+$`，如果 `state::config` 已有更严格 helper，以 helper 为准。
- 编辑已有 server 时不允许改名；后续 rename flow 必须同时更新 config key、runtime session、selection 和 status cache。
- 空白草稿 row 的处理：如果 key/value 两侧都为空，则忽略；如果只填一侧，则报错。
- 所有数组/映射字段拒绝重复 key/name。`headers` 和 `env_headers` 要按 header name case-insensitive 去重；`env` 和 `env_vars` 按 env var name 去重。

`stdio`：

- `command` 必填。
- `args` 中空字符串不通过 UI 保存；确实需要空字符串参数时只能走 `config.toml` advanced path。
- `env` key 必须是合法 env var name；value 可以为空字符串。
- `env_vars` 每项必须是合法 env var name。
- `cwd` 可选；trim 后为空则写 `None`，非空时保存为 `PathBuf` 字符串，不在 UI 保存阶段要求路径存在。

`streamable_http`：

- `url` 必填，必须能 parse，且 scheme 为 `http` 或 `https`。
- `bearer_token_env_var` 非空时必须是合法 env var name。
- Header names 必须能 parse 为 `http::HeaderName`。
- Header values 必须能 parse 为 `http::HeaderValue`。
- `env_headers` 的 value 必须是合法 env var name。
- 拒绝 reserved protocol headers：
  - `accept`
  - `content-type`
  - `mcp-session-id`
  - `mcp-protocol-version`
  - `last-event-id`
  - 当 `bearer_token_env_var` 或 OAuth 已配置时，拒绝用户手写 `authorization`。

审批规则：

- Add/Edit dialog 不提供 per-server approval 选择。
- MCP tool 的默认审批模式继承 ChatForm 当前 run 的 `ToolApprovalMode`。
- `RequestApproval` 映射为 MCP prompt/on-request；`AutoApprove` 和 `FullAccess` 映射为 MCP auto/never。
- 如果 TOML 中已有 `default_tools_approval_mode` 或 per-tool override，首版 UI 不编辑它，但保存时必须保留；runtime 的 effective policy 必须清楚记录“ChatForm 继承值 + TOML advanced override”的优先级，避免 UI 让用户误以为每个 MCP server 可单独调审批。

不要新增会悄悄猜测非法 transport/auth fields 的 runtime fallback logic。

## 数据流

### 数据归属

| 数据 | Source of truth | Runtime cache | UI 读取 | 持久化写入 |
| --- | --- | --- | --- | --- |
| MCP server definitions | `AiChat2ConfigStore` / `config.toml` | `McpRuntimeStore` 派生 rows | Settings MCP page | 只写 `config.toml` |
| OAuth access/refresh tokens | GPUI credentials | live session 内的 rmcp `AuthorizationManager` | 只读 status snapshots | 只写 GPUI credentials |
| OAuth in-progress browser flow | 无 | `McpOAuthFlowState` | `McpOAuthDialog` / row status | callback 成功前不写入 |
| Connected session 和 tool list | 无 | `McpSessionManager` / `McpRuntimeStore` | Settings detail 和 run setup | 不写入 |
| MCP session identity | 当前 effective runtime config | `McpSessionManager` 内部 fingerprint | 无 | 不写入 |
| Tool invocation fact | fresh DB | repository/event projection | conversation timeline | `tool_invocations` 和 `conversation_items` |

### Settings 编辑

1. 用户在 Settings 编辑 MCP server。
2. `McpServerEditDialogState` 把结构化 row draft 交给 `validation.rs` 校验。
3. 校验通过后，`form_state.rs` 把 draft merge 到现有 `McpServerTomlConfig`；未暴露的 OAuth、timeouts、tool filters、approval override 等字段不被删除。
4. Dialog 通过 `state::config::upsert_mcp_server(...)` 更新 `AiChat2ConfigStore`。
5. Config store 持久化 `config.toml`。
6. `McpRuntimeStore` 观察到 config 变化。
7. 对 changed/removed/disabled servers，runtime store 关闭 stale sessions。
8. Status rows 刷新为 `NotConnected` 或 `Disabled`。
9. 不写 chat DB。

### OAuth 连接

1. 用户在已保存 HTTP MCP server 的 OAuth 卡片点击 `授权` 或 `重新授权`。
2. `McpRuntimeStore` 创建 per-server OAuth flow。
3. 本地 callback 服务在 `127.0.0.1` 启动。
4. `OAuthState::start_authorization(...)` 执行 metadata discovery，并创建 authorization URL。
5. UI 通过 `cx.open_url(&authorization_url)` 打开浏览器。
6. 本地 callback 服务收到 redirect，并把完整 callback URL 转发给 `OAuthState::handle_callback_url(...)`。
7. Runtime 通过 GPUI credentials 持久化 rmcp `StoredCredentials`。
8. UI row 切换为 `Authorized`。
9. 如果该 server 已连接，先 drop 再用 OAuth `AuthClient` reconnect。

### 测试连接

1. 用户点击 `Test`。
2. `McpRuntimeStore` 只连接该 server。
3. Runtime 执行 initialize 和 `tools/list`。
4. Status row 更新 server info、tool count 和可能的 error。
5. test 期间刷新过的 OAuth token 会 mirror 回 keychain。
6. 不写 chat DB。

### Agent run

1. Conversation code 构建 `AgentRunRequest`。
2. MCP runtime 计算 runtime-only per-server fingerprint，并连接 enabled servers。
3. 必要时加载/刷新 OAuth tokens。
4. MCP runtime 把 allowed MCP tools 注册到 `request.tool_registry`。
5. `AgentRuntime::begin_run` 注册 built-in tools 并 finalize names。
6. Provider/Rig 收到最终 tool declarations。
7. Tool invocation persistence 记录 `ToolSource::Mcp { server_id }`、raw MCP tool name 和 runtime tool name。
8. Approval/tool result rows 通过现有 timeline plumbing 渲染。

### OAuth scope failure

1. MCP call 通过 rmcp streamable HTTP error 返回 403 insufficient scope。
2. Runtime 提取或记录 scope failure 状态。
3. Server status 变为 `ScopeUpgradeRequired`。
4. 默认 UI 仍不暴露 scopes；授权卡片显示“需要重新授权”，主操作为 `重新授权`。
5. Callback flow 完成后更新 persisted token scopes。
6. 用户 retry/resend 当前 conversation turn，或在支持时重新执行 failed tool flow。
7. 完整 `Upgrade Access` 增量 scope flow 后续作为 advanced path 设计。

## 数据获取和刷新

- Config load 在 app state initialization 期间通过 `AiChat2ConfigStore::install_global_with_backend(...)` 完成。
- Settings MCP page 渲染 config rows 和 `McpRuntimeStore` status rows 合并后的 projection。
- Settings list rendering 不能启动 stdio process，也不能发网络请求。
- Add/Edit dialog 只读取当前 config 初始化本地 draft；用户编辑过程中不触发 runtime/network fetch。
- `Test` 和 agent run setup 通过 MCP `initialize` + `tools/list` 获取 server info 和 tool list。
- Tool list cache 以 `server_id` 和 runtime-only fingerprint 为 key。
- `notifications/tools/list_changed` 会更新该 server 的 Settings status；复用已有 session 前会重新执行 `tools/list`，避免 stale tool cache 进入下一次 run。未连接 server 不后台启动。
- OAuth status 从 runtime flow state、stored credentials metadata 和 live `AuthorizationManager` 派生；expired/refresh-needed status 在 explicit refresh、`Test` 或 run setup 时更新。
- Provider/model fetching 与 MCP fetching 保持分离；MCP Settings 不复用 provider model refresh code path，只复用 notification/error pattern。

## 数据库影响

MCP definitions、runtime config snapshot/hash 或 OAuth tokens 不需要 SQLite schema migration。

现有 tables 已足够：

- `agent_runs.output_json`
- `tool_invocations.source`
- `tool_invocations.server_id`
- `tool_invocations.tool_name`
- `tool_invocations.runtime_tool_name`
- `tool_invocations.input_json`
- `tool_invocations.output_json`
- `tool_invocations.error_json`
- `tool_invocations.approval_json`
- `conversation_items.payload_json`

Data model 变更：

- `AgentRuntimeSnapshot` 不包含 MCP config snapshot/hash。
- MCP definitions 属于 `config.toml`，live sessions/tool cache 属于 runtime，tool invocation facts 继续写入 normalized tables。
- 如果未来需要在 timeline/debug surfaces 展示 MCP 配置，必须新增 redacted audit payload，不能持久化 full runtime config。

不要新增这些表：

- `mcp_servers`
- `mcp_tools`
- `mcp_oauth_tokens`

原因：

- MCP definitions 属于 app config，不属于 conversation data。
- OAuth tokens 是 secrets，属于 platform credential store。
- Tool invocation facts 已经有 normalized DB columns。

## 图标

`app/ai-chat2/src/foundation/assets.rs` 的 app-local `IconName` 已存在并可复用：

- `Server`：generic MCP server fallback。
- `Terminal`：stdio server。
- `Cloud`：streamable HTTP server。
- `CircleCheck`：connected / authorized。
- `CircleAlert`：failed。
- `RefreshCcw`：refresh/test。
- `Plus`：add。
- `Pencil`：edit。
- `Trash`：delete。
- `ShieldCheck` / `ShieldAlert`：approval/security status。
- `Plug`：Settings MCP page。
- `LogIn` / `LogOut`：`取消授权` 等 OAuth session 操作；默认授权按钮继续优先使用 `Shield` / `RefreshCcw`。
- `KeyRound`：后续 credential/OAuth status。
- `Link` / `Unlink`：保留给后续 OAuth callback/open/advanced actions；默认 OAuth UI 当前不使用。

以上 Lucide SVG 均已确认存在于 `third_party/lucide/icons/`。

## i18n

新增 keys 到：

- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`

结构化表单需要的 key groups：

```text
settings-page-mcp
mcp-search-placeholder
mcp-action-add-server
mcp-action-edit-server
mcp-action-delete-server
mcp-action-test-server
mcp-action-refresh-servers
mcp-dialog-create-title
mcp-dialog-edit-title
mcp-section-effective-config
mcp-section-server-info
mcp-section-tools
mcp-section-stdio
mcp-section-http
mcp-field-transport
mcp-field-server-id
mcp-field-name
mcp-field-command
mcp-field-args
mcp-action-add-arg
mcp-placeholder-arg
mcp-field-env
mcp-action-add-env
mcp-placeholder-env-key
mcp-placeholder-env-value
mcp-field-env-vars
mcp-action-add-env-var
mcp-placeholder-env-var
mcp-field-cwd
mcp-field-url
mcp-field-bearer-token-env-var
mcp-field-headers
mcp-action-add-header
mcp-placeholder-header-name
mcp-placeholder-header-value
mcp-field-env-headers
mcp-action-add-env-header
mcp-placeholder-env-header-var
mcp-field-version
mcp-field-protocol-version
mcp-field-instructions
mcp-transport-stdio
mcp-transport-streamable-http
mcp-status-disabled
mcp-status-not-connected
mcp-status-connecting
mcp-status-connected
mcp-status-failed
mcp-auth-not-configured
mcp-auth-signed-out
mcp-auth-signing-in
mcp-auth-authorized
mcp-auth-authorization-required
mcp-auth-scope-upgrade-required
mcp-auth-failed
mcp-empty-no-servers
mcp-empty-search
mcp-empty-no-tools
mcp-config-source
mcp-config-source-toml
mcp-tools-count-suffix
mcp-value-empty
mcp-value-yes
mcp-value-no
mcp-oauth-required-title
mcp-oauth-required-description
mcp-oauth-authorized
mcp-oauth-not-authorized
mcp-oauth-authorize
mcp-oauth-reauthorize
mcp-oauth-signing-in
mcp-oauth-authorization-required
mcp-oauth-scope-upgrade-required
mcp-oauth-failed
mcp-oauth-sign-out
mcp-delete-title
mcp-delete-description
mcp-notify-server-created
mcp-notify-server-saved
mcp-notify-server-deleted
mcp-notify-save-failed
mcp-notify-delete-failed
mcp-validation-summary
mcp-validation-name-required
mcp-validation-name-invalid
mcp-validation-name-duplicate
mcp-validation-command-required
mcp-validation-arg-empty
mcp-validation-env-name-invalid
mcp-validation-env-name-duplicate
mcp-validation-env-row-incomplete
mcp-validation-url-required
mcp-validation-url-invalid
mcp-validation-url-scheme
mcp-validation-header-name-invalid
mcp-validation-header-value-invalid
mcp-validation-header-duplicate
mcp-validation-header-reserved
mcp-validation-header-row-incomplete
mcp-validation-bearer-env-invalid
mcp-validation-cwd-invalid
```

现有 timeouts、tool allow/deny、approval mode 相关 keys 如果已经在 locale 文件中存在，可以暂时保留给 TOML advanced 后续 UI；默认 Add/Edit dialog 不再引用它们。OAuth 默认 UI 当前只需要 `需要 OAuth`、授权状态、授权/重新授权/取消授权相关 keys；OAuth flow/field、callback validation、copy/open 等高级 keys 留给后续 advanced path。server/tool name、row number、重复 key/name 和 validation field label 的插值使用 `FluentArgs`。

## 实现顺序

1. 已完成：依赖更新。
   - 启用 `rmcp/auth`，
   - 在真实使用处新增 direct `http`、`tokio sync`、`url` deps。
2. 已完成：配置模型。
   - 扩展 `McpServerTomlConfig`，
   - 增加 validation helpers 和 config tests，
   - 增加 reserved header checks。
3. 已完成：OAuth 存储。
   - 在 `state/mcp_oauth.rs` 实现 keychain read/write/delete helpers，
   - 增加 storage-key canonicalization 和 callback URL parser tests。
4. 已完成：OAuth callback。
   - 在 `state/mcp_oauth.rs` 实现 `tokio::net::TcpListener` callback listener，
   - 已覆盖 relative callback URL parser；provider error、wrong path 和 callback URL override 仍可继续补测。
5. V1 已完成非 OAuth 路径：Agent MCP session manager。
   - 增加 stdio 和 streamable HTTP connection，
   - OAuth `AuthClient` path 已在阶段 2 接入，
   - list tools 并通过 `McpConnector` 注册，
   - 暴露 status snapshots。
6. V1 已完成非 OAuth 路径：App MCP runtime store。
   - 安装 global store，
   - 从 config 派生 rows，
   - 实现 test/refresh actions，
   - OAuth authorization-code connect、取消授权、scope failure 状态和 refreshed credentials mirror 已在阶段 2 接入；完整 `Upgrade Access` 增量 scope flow 后续再做。
7. V1 已完成：Settings MCP 管理 UI 调整为 Codex 风格简化表单。
   - 保留 page、toolbar、row/status/detail rendering，
   - 保留 Add/Edit/Delete/Enable/Disable 和 `config.toml` 持久化，
   - 新增 `form_state.rs`、`form_rows.rs`、`validation.rs`，
   - 删除默认 UI 的多行字符串解析入口，
   - 隐藏 `required`、timeouts、tool filters、approval override 和 OAuth config，
   - 参数、环境变量、环境变量传递、HTTP headers、env-backed headers 全部改为结构化 rows，
   - 保存时 merge 到原 TOML config，保留未暴露字段，
   - 增加结构化 row、字段校验、隐藏字段保留和 ChatForm 审批继承 tests。
8. V1 已完成：Conversation runtime。
   - 在 `AgentRuntime::begin_run` 之前重构 async setup，
   - 准备 MCP tools，但不把 MCP config 写入 run input，
   - 处理 required/non-required server failures。
9. 后续：审批恢复。
   - 用 source-neutral tool execution 替换 local-only approved resume，
   - 使用当前 MCP runtime/config 准备 resume 所需工具，失败时提示用户 retry/resend，
   - 增加 MCP resume tests。
10. 部分完成：Tool list change notifications。
   - 处理 `notifications/tools/list_changed`，
   - 更完整的 status cache invalidation、retry UX 和测试后续再补。
11. 已完成：文档/状态。
   - implementation lands 或 scope changes 后更新 `app/ai-chat/docs/dev/issue-137/README.md`。

## 验证计划

自动化检查：

```text
cargo fmt
cargo test -p ai-chat2 mcp
cargo test -p ai-chat2 mcp_form
cargo test -p ai-chat2 oauth
cargo test -p ai-chat-agent mcp
cargo test -p ai-chat-agent approval
cargo check -p ai-chat2 -p ai-chat-agent -p ai-chat-core -p ai-chat-db
cargo clippy -p ai-chat2 -p ai-chat-agent --all-targets -- -D warnings
git diff --check
```

手动检查：

- 添加 stdio MCP server，确认 Settings status 进入 connected。
- 在 stdio 表单中用多行结构化参数、literal env、env var pass-through 和 cwd 保存后，确认 `config.toml` 写成数组/table，而不是解析后的一段字符串。
- 添加带 env-backed bearer token 的 streamable HTTP MCP server，确认 secret 不写入 `config.toml`。
- 在 HTTP 表单中添加 literal headers 和 env-backed headers，确认重复 header、reserved header、非法 env var 会阻止保存。
- 编辑已有 TOML advanced 字段的 server，保存 UI 暴露字段后确认 `required`、timeouts、tool filters、approval override 和 `oauth` definition 没被删除。
- 禁用 server，确认对应 stdio process/session shutdown。
- 启动 agent run，确认 MCP tools 出现在 provider tool declarations 中。
- 切换 ChatForm 审批模式后启动 agent run，确认 MCP tool 的默认审批跟随本次 run 的 ChatForm policy，而不是来自隐藏的 per-server UI 状态。
- 触发 MCP tool call，确认 `tool_invocations.source` 是 `Mcp { server_id }`。

后续 OAuth/approval 手动检查：

- 添加需要 OAuth 的 streamable HTTP MCP server，并完成 browser login。
- 确认 OAuth token 能通过 GPUI credentials 跨 app restart 保留。
- 强制 token refresh，确认 refreshed credentials mirror 回 GPUI credentials。
- 触发 insufficient scope，确认 Settings 进入“需要重新授权”状态；完整 `Upgrade Access` 增量 scope flow 仍是后续 advanced path。
- 批准一个 prompted MCP tool call，确认 resume 执行同一个 raw server/tool。

## 后续实现记录

以下项目已在阶段 2 补齐：

- OAuth 简化 UI：为 HTTP server 增加 `需要 OAuth` 开关；开启后展示授权状态卡片和授权/重新授权/取消授权按钮。关闭并保存时删除 TOML OAuth definition 和该 server 对应的 GPUI credentials token，避免旧授权状态污染后续配置。authorization-code PKCE、dynamic registration / configured client id、scopes、resource、callback port/callback URL 等复杂字段只保留 TOML advanced path，不进入默认表单。
- OAuth authorization-code runtime：实现 rmcp `OAuthState` / `AuthorizationManager` / `AuthClient` path、`127.0.0.1` callback listener、GPUI credentials token storage 和 refreshed credentials mirror。
- MCP approval resume：把 local-only `approve_and_resume_tool(...)` 重构为 source-neutral executor；使用当前 MCP runtime/config 准备工具，失败时要求 retry/resend。

以下项目仍是后续实现入口：

- OAuth advanced scope upgrade：当前 insufficient scope 进入“需要重新授权”状态；如果后续要做 `Upgrade Access` 增量 scope flow，需要设计 scope 展示、授权 URL 生成和 retry/replay 边界。
- `ClientCredentials` UI：为 TOML 已支持的 `ClientCredentials` flow 增加高级表单、secret env var 校验、状态展示和测试路径。
- Advanced MCP config UI：如果后续要暴露 `required`、timeouts、tool allow/deny、per-server/per-tool approval override，必须作为单独 Advanced path 设计，并明确这些字段与 ChatForm 审批继承的优先级。
- Codex-style prewarm：在 OAuth status、错误展示和 retry/resend 稳定后，再评估是否在 composer 首次非空输入、打开已有 conversation 或显式 warmup action 中预热 MCP server。
- MCP config audit display：如果未来要在 run history 中展示 MCP 配置，必须做 redacted audit payload，并展示 server id、transport kind、tool name 等非 secret metadata。

## 已确认问题

- `required = true` 暂不进入默认 Add/Edit dialog。后续是否进入 Advanced path 需要和 tool filters、timeouts、approval override 一起设计。
- callback URL override 应该是 app-global、per-server，还是两者都支持？当前计划支持 per-server TOML；默认 UI 暂不暴露。
- `display_name` 是否需要作为“显示名称”单独暴露还未确认；本轮按 Codex 风格只暴露名称/config key，并保留已存在 `display_name`。
