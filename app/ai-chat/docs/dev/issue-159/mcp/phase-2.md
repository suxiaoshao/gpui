# MCP 阶段 2 开发计划

本文档承接 `ai-chat2` MCP 阶段 1 已完成的 `config.toml only` 管理 UI 和非 OAuth runtime，定义阶段 2 的开发边界、模块结构、数据流、状态管理、数据库影响、i18n、图标、依赖和验证计划。

## 阶段 1 基线

阶段 1 已完成：

- `app/ai-chat2/src/state/config.rs` 和 `app/ai-chat2/src/state/config/mcp.rs` 提供 `[mcp_servers]` 配置模型、校验和 `config.toml` 读写。
- `app/ai-chat2/src/state/mcp.rs` 安装 `McpRuntimeGlobal`，持有 live `McpSessionManager`、status cache、tools/list cache，并为 agent run setup 注册 MCP tools。
- `crates/ai-chat-agent/src/mcp.rs` 提供 `McpConfigLayer`、`McpServerRuntimeConfig`、stdio / streamable HTTP session manager、tools/list 和 `McpConnector::register_rmcp_tools(...)`。
- `app/ai-chat2/src/features/settings/mcp.rs` 和 `mcp/*` 子模块提供 MCP Settings 页面、列表、详情、Add/Edit/Delete、Enable/Disable、结构化表单和校验。
- `app/ai-chat2/src/state/conversation_runtime.rs` 在 `AgentRuntime::begin_run(...)` 前执行 MCP run setup，并注册当前 enabled MCP tools；MCP runtime config 不写入 run payload。
- `crates/ai-chat-agent/src/tool_registry.rs`、`crates/ai-chat-agent/src/persistence/tool_hook.rs`、`crates/ai-chat-db` 已记录 MCP tool invocation facts。

阶段 1 明确未完成：

- OAuth browser flow、token storage、refresh、scope upgrade、logout。
- MCP tool approval resume；当前 `crates/ai-chat-agent/src/runtime/approval_resume.rs` 只支持 `ToolSource::Local`。
- 项目级 MCP definitions 和 project trust model；已确认不进入阶段 2。
- Codex-style composer prewarm。
- MCP Prompts、Resources、Sampling、Elicitation UI。

## 阶段 2 范围

### 决策状态

| 事项 | 状态 | 阶段 2 文档处理 |
| --- | --- | --- |
| MCP approval resume | 已确定要做 | 直接进入实现计划，优先级最高。 |
| OAuth authorization-code browser flow | 已实现第一版，callback 采用 Codex/Zed 的 loopback 策略 | 使用 `127.0.0.1:<random-port>` 临时 listener；实现基于 `tokio::net::TcpListener`，不引入 `tiny_http` 或 `axum`。 |
| OAuth Settings UI | 已确定简化 | 默认表单只暴露“需要 OAuth”开关；开启后展示授权状态卡片和授权/重新授权按钮，不暴露 scopes、resource、callback、client id 等高级字段。 |
| OAuth 关闭清理 | 已确认删除 | 用户关闭“需要 OAuth”并保存时删除 `config.toml` 中该 server 的 OAuth definition，同时删除原 URL 和保存后 URL 可能对应的 GPUI credentials，避免配置文件和授权数据污染。 |
| Project-level MCP definitions | 已确认阶段 2 不做 | 不读取项目目录配置、不合并 project-scoped definitions、不新增 trust prompt、trust store 或 project-scoped session key。 |
| Prompts / Resources / Sampling / Elicitation | 已确认先不做 | 阶段 2 不实现，也不在本文档中规划阶段 3；后续如果要做，单独开新阶段。 |

阶段 2 已落地目标：

- 让 MCP tool approval 从“可创建审批请求”推进到“批准后能执行同一个 MCP tool 并恢复 run”。
- 完成 rmcp OAuth authorization-code browser flow，并把 token 安全存入 GPUI credentials。
- HTTP OAuth runtime 通过 GPUI credentials 注入 rmcp `StoredCredentials`，agent 侧用 rmcp `AuthClient` 建立 streamable HTTP transport。
- 完成 refreshed credentials mirror：rmcp refresh 后的新 `StoredCredentials` 会通过 runtime event 写回 GPUI credentials。
- 完成 OAuth 取消授权：已授权状态卡片提供 `取消授权`，删除 credentials 并断开 stale session。
- 完成基础 scope failure 状态：insufficient scope 会进入“需要重新授权”状态；默认 UI 仍不暴露 scopes，完整 `Upgrade Access` 增量 scope flow 归入后续 advanced path。
- 补齐 MCP runtime 的状态刷新和错误展示：`tools/list_changed` 更新 UI，复用已有 session 前刷新 `tools/list`，server 测试错误写入对应 row。

阶段 2 剩余验证：

- 用真实 OAuth MCP server 做 browser login、refresh mirror、取消授权、scope failure 和 approval resume 的端到端手动验证。

阶段 2 不做：

- 不把 OAuth token 写入 `config.toml`、SQLite 或 runtime snapshot。
- 不在 app 启动时自动拉起所有 enabled stdio server。
- 不从已持久化的旧 MCP config 静默 reconnect 已变更配置；当前实现不持久化 MCP config/hash。
- 不把高级 TOML schema 全部塞回默认 Add/Edit 表单。
- 不在默认 OAuth UI 暴露 scopes、resource、client id、client metadata URL、callback port、callback URL override、dynamic registration 等复杂配置；这些只走 TOML advanced path，并且 OAuth 保持开启时 Add/Edit 保存必须保留原值。用户关闭 OAuth 并保存时，按已确认策略删除 TOML OAuth definition 和对应 GPUI credentials。
- 不做 project-level MCP definitions：不读取 `.codex/config.toml`、`.zed/settings.json`、project metadata 或其它 project file；不实现 global + project 合并、project trust prompt、project trust persistence 或 project-scoped session 复用。
- 不做 MCP Prompts、Resources、Sampling、Elicitation；当前不建立阶段 3 结构或占位类型。

## OAuth 背景和参考实现

### OAuth 在 MCP 中的角色

MCP HTTP server 如果要求 OAuth，实际有三个参与者：

- MCP server：资源服务器，最终接收 `Authorization: Bearer <access_token>`。
- Authorization server：用户登录、授权、发放 access/refresh token。
- ai-chat2：OAuth client。它代表用户完成浏览器登录，保存 token，后续调用 MCP server 时带上 Bearer token。

OAuth callback server 不是 MCP server，也不是远端服务。它只是 desktop client 在一次授权流程里临时启动的 redirect 接收点：

1. ai-chat2 生成 PKCE verifier/challenge 和 state。
2. ai-chat2 准备 `redirect_uri`。
3. 用户浏览器打开 authorization URL。
4. 用户在 authorization server 登录并同意授权。
5. authorization server 把浏览器重定向到 `redirect_uri?code=...&state=...`。
6. ai-chat2 接收 callback，校验 state，再用 authorization code + PKCE verifier 换 token。
7. token 写入 GPUI credentials；后续 MCP HTTP request 通过 rmcp `AuthClient` 附加 Bearer token。

桌面 app 通常没有公网 HTTPS callback URL，所以 native app 常用 loopback redirect：临时绑定 `127.0.0.1:<random-port>/callback`，把这个地址传给 authorization server。该 listener 只服务本次 OAuth flow，收到一次有效 callback 或任务取消后关闭。

### Codex 做法

参考文件：

- `/Users/sushao/Documents/code/codex/codex-rs/rmcp-client/src/perform_oauth_login.rs`
- `/Users/sushao/Documents/code/codex/codex-rs/rmcp-client/src/rmcp_client.rs`
- `/Users/sushao/Documents/code/codex/codex-rs/rmcp-client/src/oauth.rs`

Codex 的 OAuth browser flow：

- `perform_oauth_login.rs::OauthLoginFlow::new(...)` 默认绑定 `127.0.0.1:0`，由 OS 分配随机端口。
- `local_redirect_uri(...)` 从本地 callback server 的实际地址生成 `http://127.0.0.1:<port>/callback`。
- `append_callback_id_to_redirect_uri(...)` 会把由 MCP server URL 派生的 callback id 加到 path 上，避免不同 server 的 callback path 混淆。
- `spawn_callback_server(...)` 用 `tiny_http` 接收 callback，解析 `code` / `state` / `error`，成功后返回一个极小的完成页面并关闭。
- `start_authorization(...)` 复用 rmcp `OAuthState` / `AuthorizationManager`：没有预配置 client id 时走 `OAuthState::start_authorization(...)`，有 client id 时手动配置 client 后获取 authorization URL。
- `finish(...)` 收到 callback 后调用 `oauth_state.handle_callback(...)`，再 `get_credentials()`，然后保存 `StoredOAuthTokens`。
- 后续 streamable HTTP 连接通过 `rmcp::transport::auth::AuthClient` 包装；`OAuthPersistor` 会在 refresh 后把最新 credentials 再写回持久化存储。

Codex 的 token storage 与 ai-chat2 不完全一致：Codex 用 keyring/加密存储，必要时 fallback 到文件；ai-chat2 阶段 2 应使用 GPUI credentials，不新增 `keyring` crate。

### Zed 做法

参考文件：

- `/Users/sushao/.cargo/git/checkouts/zed-a70e2ad075855582/1d217ee/crates/project/src/context_server_store.rs`
- `/Users/sushao/.cargo/git/checkouts/zed-a70e2ad075855582/1d217ee/crates/context_server/src/oauth.rs`
- `/Users/sushao/.cargo/git/checkouts/zed-a70e2ad075855582/1d217ee/crates/zed_credentials_provider/src/zed_credentials_provider.rs`

Zed 的 OAuth browser flow：

- `ContextServerStore::authenticate_server(...)` 只允许对 `AuthRequired` 的 HTTP context server 发起 OAuth。
- `run_oauth_flow(...)` 先生成 PKCE challenge 和 state。
- `oauth::start_callback_server()` 启动 loopback HTTP callback server，返回真实 `redirect_uri` 和 callback future。
- Zed 用自己的 `context_server::oauth` 实现 discovery、client registration、authorization URL、code exchange 和 refresh provider，不走 rmcp OAuth helper。
- `cx.open_url(auth_url.as_str())` 打开浏览器。
- callback 返回后，Zed 校验 state，再调用 token endpoint 交换 token。
- `store_session(...)` 把完整 `OAuthSession` 写入 system keychain，key 为 `mcp-oauth:{canonical_server_uri}`。
- `McpOAuthTokenProvider` 在访问 token 过期或 HTTP 401 时尝试 refresh；refresh 后通过 channel 把新 session 写回 keychain。

Zed 的实现价值在于状态机和 keychain 边界；ai-chat2 不应照搬它的整套自研 OAuth 协议代码。ai-chat2 更适合参考 Codex：协议状态复用 rmcp，UI/credentials 边界参考 Zed/Codex。

### ai-chat2 阶段 2 结论和待确认点

已确定：

- OAuth token 不写 `config.toml`、SQLite 或 runtime snapshot。
- OAuth token 写 GPUI credentials。
- OAuth HTTP transport 优先接 rmcp `OAuthState` / `AuthorizationManager` / `AuthClient`。
- Settings 默认 UI 只展示 `需要 OAuth` 开关、授权状态卡片和显式授权/重新授权操作，不在 app 启动或打开 Settings 时自动授权。
- 默认 UI 不暴露 advanced `callback_url` override、固定 `callback_port`、scopes、resource、client id 或 dynamic registration；这些只作为 TOML advanced path 保留。

## 模块结构

继续使用现有模块入口，不新增 `mod.rs`。

阶段 2 已确定要新增或修改的文件：

```text
app/ai-chat2/src/state/config/mcp.rs
app/ai-chat2/src/state/mcp.rs
app/ai-chat2/src/state/mcp_oauth.rs
app/ai-chat2/src/state/conversation_runtime.rs
app/ai-chat2/src/state.rs

app/ai-chat2/src/features/settings/mcp.rs
app/ai-chat2/src/features/settings/mcp/detail.rs
app/ai-chat2/src/features/settings/mcp/dialog.rs
app/ai-chat2/src/features/settings/mcp/form_rows.rs
app/ai-chat2/src/features/settings/mcp/form_state.rs
app/ai-chat2/src/features/settings/mcp/row.rs
app/ai-chat2/src/features/settings/mcp/tags.rs
app/ai-chat2/src/features/settings/mcp/validation.rs

crates/ai-chat-agent/src/mcp.rs
crates/ai-chat-agent/src/runtime/approval_resume.rs
crates/ai-chat-agent/src/persistence/tool_hook.rs
crates/ai-chat-agent/src/tool_registry.rs
```

已确定新增文件职责：

- `state/mcp_oauth.rs`：当前实现把 OAuth storage、callback listener 和 rmcp browser flow 放在单文件内，包含 GPUI credentials read/write/delete、canonical key、`127.0.0.1` loopback listener、`OAuthState` / `AuthorizationManager` 授权流程。文件继续增长时再拆成 `state/mcp_oauth.rs` + `state/mcp_oauth/{storage,callback,flow}.rs`。
- `features/settings/mcp/dialog.rs`：渲染 `需要 OAuth` switch、授权状态卡片和 `授权` / `重新授权` 按钮；默认 UI 不暴露 advanced OAuth 字段。
- `crates/ai-chat-agent/src/runtime/approval_resume.rs`：已承担 source-neutral approval resume 和 runtime tool reconstruction；后续如果继续增长，再拆 `runtime/tool_execution.rs`。
- `state.rs`：新增 `pub(crate) mod mcp_oauth;`，保持 app state module 入口一致。

## 自定义类型

OAuth storage：

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

pub(crate) struct McpOAuthStorageKey {
    pub(crate) server_id: String,
    pub(crate) canonical_resource_url: String,
}
```

OAuth flow state：

```rust
pub(crate) enum McpOAuthFlowState {
    Idle,
    Discovering,
    WaitingForBrowser {
        authorization_url: String,
        callback_url: String,
    },
    Exchanging,
    Authorized(McpOAuthStatus),
    Failed(String),
}

pub(crate) enum McpOAuthStatus {
    NotConfigured,
    SignedOut,
    SigningIn,
    Authorized {
        scopes: Vec<String>,
        expires_at_unix_ms: Option<u64>,
    },
    Expired,
    ScopeUpgradeRequired {
        scopes: Vec<String>,
    },
    Failed(String),
}
```

Settings OAuth UI：

```rust
pub(crate) struct McpOAuthUiState {
    pub(crate) enabled: bool,
    pub(crate) status: McpOAuthStatus,
    pub(crate) action_in_flight: Option<McpOAuthAction>,
}

pub(crate) enum McpOAuthAction {
    Authorize,
    Reauthorize,
}
```

该类型只作为 Settings 渲染投影，不作为新的持久化 source of truth。`enabled` 从当前 server TOML 中是否存在 `oauth` 派生；`status` 从 TOML OAuth definition、授权任务状态和 live runtime status 派生。`Refresh` / `Logout` 后续补齐时再扩展 action enum。

Approval resume：

```rust
pub(crate) enum ApprovedToolExecutionRequest {
    Local {
        tool_name: String,
        input_json: serde_json::Value,
    },
    Mcp {
        server_id: String,
        raw_tool_name: String,
        input_json: serde_json::Value,
    },
}

pub(crate) enum ApprovedToolExecutionOutcome {
    Succeeded {
        output_json: serde_json::Value,
    },
    Failed {
        error_json: serde_json::Value,
    },
}
```

Approval resume error：

```rust
pub(crate) enum McpApprovalResumeError {
    UnsupportedSource {
        source: String,
    },
    MissingLiveSession {
        server_id: String,
    },
    ExecutionFailed {
        server_id: String,
        raw_tool_name: String,
        error_json: serde_json::Value,
    },
}
```

Project-level MCP 本阶段不定义自定义类型。后续如果重新开启 project-level MCP，再单独设计 `McpConfigScope`、trust snapshot、project config parser、merge result 和 UI draft 类型。

## 数据流

Approval resume：

1. 用户在 timeline 点击 approve。
2. `app/ai-chat2/src/state/conversation_runtime.rs` 调用 `AgentRuntime::approve_and_resume_tool(...)`。
3. `crates/ai-chat-agent/src/runtime/approval_resume.rs` 读取 pending approval、tool invocation 和 parent agent run。
4. `runtime/approval_resume.rs` 根据 `ToolSource` 生成 source-neutral runtime tool request。
5. Local tool 走现有 built-in executor；MCP tool 使用 `server_id`、raw MCP tool name 和保存的 arguments 调用 live `McpSessionManager`。
6. MCP resume 使用当前 live `McpSessionManager`；如果缺少 live MCP session，则写 actionable error，要求用户 retry/resend，而不是从旧 run payload 重建 MCP config。
7. executor 写回 `tool_invocations` 和 `conversation_items`。
8. tool 成功后创建 resume run，让模型读取已有 ToolCall / ApprovalDecision / ToolResult 继续回答。

OAuth authorization-code：

1. 用户在 HTTP MCP Add/Edit dialog 中打开 `需要 OAuth` 并保存。
2. 已保存 server 的 OAuth 卡片展示 `授权` / `重新授权` 按钮；新增但未保存的 draft 不写 GPUI credentials。
3. `features/settings/mcp/dialog.rs` 调用 `McpRuntimeStore::authenticate_server(server_id)`。
4. `state/mcp_oauth.rs` 临时绑定 `127.0.0.1:<random-port>`，生成 redirect URI。
5. `state/mcp_oauth.rs` 使用 rmcp `OAuthState` 或 `AuthorizationManager` 做 discovery、scope selection、dynamic registration / preconfigured client，并生成 authorization URL。
6. UI 通过 `cx.open_url(...)` 打开浏览器。
7. loopback listener 接收一次 redirect，把完整 callback URL 交给 rmcp `handle_callback_url(...)` / `exchange_code_for_token_with_issuer(...)`。
8. `state/mcp_oauth.rs` 把 rmcp `StoredCredentials` JSON 写入 GPUI credentials，key 为 `mcp-oauth:{canonical_server_uri}`。
9. `McpRuntimeStore` 更新 auth status；下一次 Settings Test / agent run setup 会从 GPUI credentials 注入 runtime config，并由 agent 侧 rmcp `AuthClient` 连接 HTTP MCP server。

OAuth Settings：

1. Add/Edit dialog 的 HTTP transport 区域展示一个简化 OAuth section。
2. `需要 OAuth` switch off：`draft.oauth` 为 `None`，不展示授权状态卡片；用户保存后删除 TOML OAuth definition，并删除该 server 对应的 GPUI credentials token，避免旧授权状态污染后续配置。
3. `需要 OAuth` switch on：如果原 TOML 已有 OAuth definition，保留原 advanced 字段；否则创建最小 `AuthorizationCodePkce` definition，`scopes` 为空，`client_id`、`client_metadata_url`、`resource`、`callback_port`、`callback_url` 均为 `None`。
4. OAuth section 开启后展示授权状态卡片：
   - `SignedOut` / `NotConfigured`：显示“尚未授权”，按钮为“授权”。
   - `Authorized`：显示“已授权”，按钮为“重新授权”。
   - `SigningIn` / `Discovering` / `WaitingForBrowser` / `Exchanging`：显示进行中状态，按钮 disabled 或 loading。
   - `Expired` / `ScopeUpgradeRequired` / `Failed`：显示可操作错误状态，按钮为“重新授权”。
5. 用户点击“授权”或“重新授权”时，只触发 `McpRuntimeStore::authenticate_server(server_id)`；不打开高级配置表单。
6. 保存 dialog 只写 TOML OAuth definition，不写 token。token 始终由授权流程写 GPUI credentials。
7. 保存时如果 OAuth 从 enabled 变成 disabled，`state/mcp_oauth.rs` 同步删除 credentials，并断开该 server 的 stale HTTP OAuth session。

Agent run：

1. Conversation code 构建 `AgentRunRequest`。
2. `state::mcp::prepare_run_request(...)` 读取 `AiChat2ConfigStore` 的 enabled definitions，构造本次运行的 agent-side MCP runtime config。
3. `state::mcp::attach_oauth_credentials(...)` 对 OAuth HTTP server 从 GPUI credentials 读取 rmcp `StoredCredentials`，只写入 agent runtime config 的 `oauth_credentials` 字段，不写入 run payload 或数据库。
4. `McpSessionManager` 连接 server：非 OAuth HTTP 直接用 `StreamableHttpClientTransport`；OAuth HTTP 用 rmcp `AuthorizationManager` + `InMemoryCredentialStore` + `AuthClient`。
5. Runtime 拉取 tools/list，注册 MCP tools，设置本次 run 的 enabled MCP sources。
6. `AgentRuntime::begin_run(...)` finalize tool names 后把 tools 交给 provider。

Project-level MCP 本阶段没有数据流：agent run setup 只读取 app-level `config.toml`，不会读取 project file，也不会进入 project trust prompt。

## 全局数据管理

- `AiChat2ConfigStore` 继续是 global MCP definitions 的唯一 source of truth。
- `McpRuntimeGlobal(Entity<McpRuntimeStore>)` 持有 live sessions、status cache、tool-list cache、OAuth flow state 和 runtime-only fingerprint 到 session 的映射。
- `McpOAuth` storage 使用 GPUI credentials；runtime store 只持有派生状态，不拥有 token truth。
- `ConversationRuntimeStore` 只负责编排当前 conversation run，不直接创建 rmcp transport。
- `AgentRuntimeSnapshot` 不记录 MCP config/hash；数据库只保留 run input 的常规字段和 tool invocation facts。
- 不新增 project-level MCP trust state、project trust store 或 project-scoped config cache。

## 数据获取和刷新

- Settings MCP page 渲染只读取 `AiChat2ConfigStore` snapshot 和 `McpRuntimeStore` status projection，不启动进程、不发网络请求。
- `Test` / `Refresh` 是显式 runtime fetch：连接单个 server，执行 initialize 和 `tools/list`，更新 status cache。
- OAuth status 当前从 TOML OAuth definition、Settings 授权任务状态和 live runtime status 派生；重启后未连接前不会 eager 读取 keychain。
- OAuth token refresh 当前由 rmcp `AuthorizationManager::get_access_token()` 在 agent runtime 内按需执行；agent 侧 mirror `CredentialStore` 会把 refreshed `StoredCredentials` 通过 runtime event 发回 app，并写入 GPUI credentials。
- `notifications/tools/list_changed` 会更新已连接 server 的 Settings 状态；复用已有 session 前会重新执行 `tools/list`，避免下一次 run 使用 stale tool cache。未连接 server 不后台启动。
- Agent run setup 是 provider call 前的最后一次 MCP fetch：连接 enabled server、刷新必要 OAuth token、拉取 tools/list、注册 tool registry。
- 阶段 2 不读取 project-level MCP config。

## 数据库变更

阶段 2 默认不新增 SQLite migration。

继续使用现有表：

- `agent_runs.input_json`：不保存 MCP config/hash。
- `tool_invocations`：保存 MCP source、server id、raw/runtime tool name、input/output/error/approval。
- `conversation_items`：保存 ToolCall / ApprovalRequest / ApprovalDecision / ToolResult。
- `approval_decisions`：保存 pending/approved/denied。

阶段 2 只有出现以下产品决策时才新增 migration：

- 审批模式需要按 conversation/project sticky 保存。
- 需要持久化 MCP tool catalog 历史，而不仅是 tool invocation facts。

Project-level MCP trust 持久化不属于阶段 2；如果后续重新开启 project-level MCP，再单独评估是否需要 migration。

## 设置 UI

复用阶段 1 组件：

- `gpui_component::button::Button`：Add、Test、Authorize、Reauthorize、Refresh、Delete confirm。
- 现有 settings dialog wiring：OAuth 开关和授权状态卡片。
- `gpui_component::toggle_group::ToggleGroup`：继续用于 stdio/http transport 切换。
- `gpui_component::switch::Switch`：继续用于 server enable/disable；新增用于 `需要 OAuth` 开关。
- `gpui_component::tag::Tag`：transport、connection、auth、scope。
- `gpui_component::scroll::ScrollableElement`：OAuth details、tool list。
- `gpui_component::group_box::GroupBox` 或 app-local outline panel：渲染 OAuth 开关卡片和授权状态卡片，保持和 Settings 现有表单 section 的间距、圆角、边框一致。
- `gpui_component::notification::{Notification, NotificationType}`：save/test/authorization/approval resume failure。

新增 app-local renderer：

```rust
fn McpServerEditDialogState::render_oauth_section(...) -> AnyElement;

fn McpServerEditDialogState::authorize_oauth(...) -> ();

fn render_resume_error_hint(
    error: &McpApprovalResumeError,
    cx: &mut Context<McpSettingsPage>,
) -> AnyElement;
```

图标：

- `IconName::Plug`：MCP page。
- `IconName::Terminal`：stdio。
- `IconName::Cloud`：streamable HTTP。
- `IconName::RefreshCcw`：Test / Refresh / 重新授权。
- `IconName::Shield`：`需要 OAuth` 开关卡片和“授权”按钮。
- `IconName::LogIn` / `IconName::LogOut`：暂不在默认 OAuth UI 使用；默认授权按钮优先使用 `Shield`，和当前设计图一致。
- `IconName::KeyRound`：credential / token status。
- `IconName::ShieldCheck` / `IconName::ShieldAlert`：approval status。
- `IconName::CircleCheck` / `IconName::CircleAlert`：connected / failed。
- `IconName::ExternalLink`：打开浏览器授权或文档链接。该 icon 已存在于 app-local `IconName`。

## i18n

新增文案写入：

- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`

阶段 2 key groups：

```text
mcp-oauth-required-title
mcp-oauth-required-description
mcp-oauth-authorize
mcp-oauth-reauthorize
mcp-oauth-authorized
mcp-oauth-not-authorized
mcp-oauth-signing-in
mcp-oauth-authorization-required
mcp-oauth-scope-upgrade-required
mcp-oauth-failed
mcp-oauth-sign-out
mcp-approval-resume-unsupported
mcp-approval-resume-config-changed
mcp-approval-resume-retry-required
```

所有 server id、tool name、scope、URL 使用 `FluentArgs` 插值，不拼接硬编码字符串。

## 依赖

已存在依赖，不是阶段 2 新增：

```toml
# workspace dependencies
rmcp = { version = "1.8.0", features = ["auth", "client", "macros", "transport-child-process", "transport-streamable-http-client-reqwest"] }
rig-core = { version = "0.39.0", features = ["rmcp"] }
gpui-tokio = { path = "./crates/gpui-tokio" }

# app/ai-chat2
gpui-tokio.workspace = true
rmcp.workspace = true
url = "2.5.8"
http = "1.4.2"
tokio = { version = "1.52.3", features = ["io-util", "net", "sync", "time"] }

# crates/ai-chat-agent
tokio = { version = "1.52.3", features = ["process", "sync", "time"] }
```

- `gpui-tokio` 是 GPUI app 接入 Tokio runtime 的桥接层；Settings、Chat runtime、OAuth callback 等从 GPUI 状态或任务里启动异步工作时，应继续使用现有 `gpui_tokio::Tokio::spawn(...)` / `cx.spawn(...)` 模式。
- 直接 `tokio` 依赖不是新的运行时方案，而是因为 app/agent 代码直接使用 `tokio::sync`、`tokio::process`、`tokio::time` 等类型和 feature；`gpui-tokio` 不替代这些 API 声明，也不应要求业务 crate 从 `gpui-tokio` 间接获取 Tokio 类型。
- 阶段 2 不新增第二套 Tokio runtime；OAuth callback listener 复用 gpui-tokio 管理的 Tokio runtime，并用 `tokio::net::TcpListener` 接收一次 loopback callback。
- 不新增 `tiny_http` 或 `axum`；当前 callback listener 不需要完整 HTTP framework。
- 不新增 `keyring`；token storage 走 GPUI credentials。
- 不新增 `oauth2`，除非实现时证明 `rmcp` 暴露类型不足以完成状态展示或测试；出现这种情况先回到文档确认。

## 实现顺序

1. 已完成：MCP approval resume 支持 source-neutral runtime tool reconstruction 和 MCP tool execution。
2. 已完成：`state/mcp_oauth.rs` 实现 GPUI credentials storage、`127.0.0.1` loopback listener 和 rmcp authorization-code flow。
3. 已完成：HTTP Add/Edit dialog 增加 `需要 OAuth` switch、授权状态卡片、`授权` / `重新授权` 动作；高级 OAuth 字段只保留 TOML advanced path；关闭 OAuth 并保存时删除对应 GPUI credentials。
4. 已完成：agent streamable HTTP path 接入 rmcp `AuthClient`，run setup / Settings Test 从 GPUI credentials 注入 runtime-only `oauth_credentials`。
5. 已完成：token refresh mirror 回 GPUI credentials。
6. 已完成：取消授权、`tools/list_changed` 状态刷新和更完整 status/error rendering。
7. 后续 advanced path：完整 `Upgrade Access` 增量 scope flow；默认 UI 当前只把 insufficient scope 映射为“需要重新授权”。

## 验证计划

自动化：

```text
cargo fmt
cargo test -p ai-chat-agent mcp
cargo test -p ai-chat-agent approval
cargo test -p ai-chat2 mcp
cargo test -p ai-chat2 oauth
cargo test -p ai-chat-db
cargo check -p ai-chat2 -p ai-chat-agent -p ai-chat-core -p ai-chat-db
git diff --check
```

手动：

- 配置 stdio MCP server，触发需要审批的 MCP tool call，批准后确认 tool 执行并 resume run。
- 修改 MCP config 后再批准旧 pending MCP call，确认拒绝 resume 并提示 retry/resend。
- 配置需要 OAuth 的 streamable HTTP server，完成 browser login，重启 app 后确认 token 从 GPUI credentials 恢复。
- 强制 token refresh，确认 refreshed credentials mirror 回 GPUI credentials。
- 触发 insufficient scope，确认 Settings 进入“需要重新授权”状态；完整 `Upgrade Access` 增量 scope flow 不属于默认 UI。
- 检查 Add/Edit dialog 中 OAuth 只暴露开关和授权状态卡片；不会出现 scopes、resource、client id、callback 等高级字段。
- 关闭 `需要 OAuth` 并保存，或在授权卡片点击 `取消授权`，确认 GPUI credentials 中该 server token 被删除、runtime stale OAuth session 被关闭；关闭 OAuth 时 TOML OAuth definition 也应被删除。
- 检查 Settings 列表和详情中 OAuth status、tool list、last error 不会溢出或误显示 secret。

## 待确认问题

阶段 2 当前没有阻塞实现的待确认问题。

已确认暂不做：

- Prompts、Resources、Sampling、Elicitation 暂不进入阶段 2，也不在本文档中展开阶段 3。

## 后续阶段暂存问题

Project-level MCP 已确认不进入阶段 2。后续如果重新开启，需要新建独立计划并重新确认：

- project-level MCP definitions 的来源：`config.toml` profile、project metadata、`.codex/config.toml`、`.zed/settings.json`，还是 ai-chat2 自己的项目文件。
- global + project MCP 的合并规则：同名 server 覆盖、冲突报错，还是按 scope namespace 共存。
- 首次加载 project config 时的信任 UI：需要展示 server id、transport、command/url、env/header secret 引用和工具风险。
- project trust 是否跨重启持久化；如果持久化，存放在 config、project DB，还是新的 trust store。
- project 切换时的 runtime-only session fingerprint 和 live session 复用边界。
