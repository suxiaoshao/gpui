# mcp-auth-test-server：RMCP 3.1.4 升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（独立 workspace 本地自动化通过；RMCP 2↔3 E2E 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)
- Owner directory：`tools/mcp-auth-test-server`
- Cargo boundary：独立 `[workspace]`、独立 `Cargo.lock`；不属于 root Cargo workspace。
- Root-owned surfaces consumed：`S-08`、`S-14`、`S-17`、`S-19`
- Owner-local IDs：`F-MCPAUTH-01`–`F-MCPAUTH-03`、`R-MCPAUTH-01`–`R-MCPAUTH-05`、`T-MCPAUTH-01`–`T-MCPAUTH-05`、`WP-MCPAUTH-01`–`WP-MCPAUTH-03`
- Owns：authenticated MCP test server、OAuth fixture、standalone manifest/lockfile 和 RMCP 3 server-side migration。
- Does not own：root workspace RMCP pin、Jaco RMCP 2 client implementation、production OAuth provider 或 root `Cargo.lock`。

本工具不与 Jaco 共享 RMCP Rust types，因此它独立升级到 `rmcp 3.1.4`；主 workspace 仍精确 pin
`rmcp = "=2.2.0"`。两者的兼容性由 MCP wire handshake/OAuth/tool-call 验证，而不是通过统一 crate major 获得。

## 精确依赖目标

| Dependency | Current | Issue #205 target | Features/kind | Classification |
| --- | --- | --- | --- | --- |
| `rmcp` | `2.2.0` | `3.1.4` | `auth`, `macros`, `server`, `transport-streamable-http-server` | Breaking |
| `anyhow` | `1.0.104` | 保留 `1.0.104` | runtime | Retained |
| `axum` | `0.8.9` | 保留 `0.8.9` | `default-features = false`; `form`, `http1`, `json`, `query`, `tokio` | Retained |
| `schemars` | `1.2.2` | 保留 `1.2.2` | runtime | Retained |
| `serde` | `1.0.229` | 保留 `1.0.229` | `derive` | Retained |
| `serde_json` | `1.0.151` | 保留 `1.0.151` | runtime | Retained |
| `tokio` | `1.53.1` | 保留 `1.53.1` | `macros`, `net`, `rt-multi-thread`, `signal` | Retained |
| `tokio-util` | `0.7.19` | 保留 `0.7.19` | runtime | Retained |
| `url` | `2.5.8` | 保留 `2.5.8` | runtime | Retained |

所有依赖继续来自 crates.io；`F-MCPAUTH-02` 必须由 Cargo 生成，禁止从 root lockfile 复制或手改。

## RMCP 3 migration contract

当前唯一已确认的 source rename 位于 `src/main.rs:88-92`：

```rust
// RMCP 2.2
StreamableHttpServerConfig::default().with_stateful_mode(false)

// RMCP 3.1.4 target
StreamableHttpServerConfig::default().with_legacy_session_mode(false)
```

RMCP 3.1.4 将该 flag 明确限定为 legacy protocol versions 的 session mode。传入 `false` 保持本工具当前
stateless behavior；不得借重命名切换为 stateful server。`StreamableHttpService<EchoServer,
LocalSessionManager>`、tool macros、`ServerHandler`、JSON response、SSE keepalive `None` 和 cancellation token
继续使用 RMCP 3.1.4 已验证的对应 API。新版本默认提供 loopback Host 防 DNS-rebinding guard；本工具继续绑定
`127.0.0.1` 并验证合法 Host 与恶意 Host，不复制自定义 guard。

## Owner-local 目标与文件

```text
tools/mcp-auth-test-server/
├── Cargo.toml                                      # F-MCPAUTH-01 [Modify] rmcp 2.2.0 -> 3.1.4
├── Cargo.lock                                      # F-MCPAUTH-02 [Regenerate with Cargo] standalone resolution
└── src/main.rs                                     # F-MCPAUTH-03 [Modify] legacy session builder rename; verify macros/types
```

- `R-MCPAUTH-01`：legacy RMCP clients 继续使用 stateless Streamable HTTP；server 不要求或保存跨请求 session ID。
- `R-MCPAUTH-02`：`/.well-known/oauth-protected-resource{,/mcp}`、authorization-server metadata、dynamic registration、authorization code、access token 与 refresh token fixture 保持现有 contract。
- `R-MCPAUTH-03`：静态 Bearer token 和 OAuth token 均可完成 initialize handshake、`tools/list` 与 `echo` tool call。
- `R-MCPAUTH-04`：Jaco 的 RMCP 2.2 client 能与此 RMCP 3.1.4 server 通过 wire protocol 互操作；不共享/转换 Rust SDK model types。
- `R-MCPAUTH-05`：token、authorization code 和 refresh token 不进入新增日志或 completion evidence。

## Owner-local Work Packages

### WP-MCPAUTH-01：升级 standalone RMCP resolution

1. 在 `F-MCPAUTH-01` 将 `rmcp` 精确更新为 `3.1.4`，完整保留四个 features；其余八个 direct dependency 不变。
2. 使用该 manifest 的 Cargo 命令重新生成 `F-MCPAUTH-02`；确认 lockfile 只属于 standalone workspace，root `Cargo.lock` 不因本 WP 变化。
3. 检查 feature tree 只有一个 RMCP 3.1.4 server resolution；不将 root `[patch.crates-io]` 或 workspace dependencies 引入工具。

完成条件：standalone manifest/lockfile 均解析 RMCP 3.1.4，retained dependencies 和 features 精确匹配表格。

### WP-MCPAUTH-02：迁移 server API 且保持 OAuth/stateless behavior

1. 在 `F-MCPAUTH-03` 将 `.with_stateful_mode(false)` 替换为 `.with_legacy_session_mode(false)`；保留 `.with_json_response(true)`、`.with_sse_keep_alive(None)` 和 cancellation token。
2. 按 RMCP 3.1.4 的实际 compiler/API 更新必要的 import、macro 或 model signature；若出现表中未记录的 behavior change，先同步 root/owner plan，不添加 RMCP 2 compatibility module。
3. 验证未认证 `/mcp` 返回 `401` 与 resource metadata challenge，static Bearer request 可进入 MCP service，Ctrl-C 仍先 cancel RMCP service 再完成 Axum graceful shutdown。
4. 验证 loopback 合法 Host 通过、非预期 Host 被拒绝；不得为了旧 fixture 关闭新 DNS-rebinding guard。

完成条件：binary 在 RMCP 3.1.4 编译，`R-MCPAUTH-01`、`R-MCPAUTH-02`、`R-MCPAUTH-05` 成立，旧 builder 名残留为零。

### WP-MCPAUTH-03：执行 RMCP 2 client ↔ RMCP 3 server mixed-version 验证

1. 启动 standalone server，health check 成功后让 Jaco RMCP 2.2 client 访问 `/mcp`；首次无 token 请求必须触发 `401` 和 protected-resource discovery。
2. 由 Jaco client 完成 dynamic registration、authorization-code exchange，使用 access token initialize；再用 refresh token 获取新 access token并重新建立连接。
3. 在 static-token 和 OAuth-token 两条路径分别完成 `tools/list`，确认存在 `echo`，调用 `echo({"text":"mixed-version"})` 并断言文本结果为 `auth ok: mixed-version`。
4. 结束 client/server，确认 cancellation/graceful shutdown 收束；记录协议版本、两侧 crate versions 和结果，但不记录任何 token/code 值。

完成条件：`R-MCPAUTH-03`–`R-MCPAUTH-05` 均有实际 wire evidence；若 handshake/OAuth/tool call 任一失败，不把主 workspace 升到 RMCP 3 作为绕过，先定位协议/adapter差异并更新计划。

## Focused Validation 与 handoff

| T-ID | Command/scenario | Expected evidence |
| --- | --- | --- |
| `T-MCPAUTH-01` | `cargo check --manifest-path tools/mcp-auth-test-server/Cargo.toml --locked` | RMCP 3 server binary 编译 |
| `T-MCPAUTH-02` | `cargo test --manifest-path tools/mcp-auth-test-server/Cargo.toml --locked` | standalone target/test harness 成功 |
| `T-MCPAUTH-03` | `cargo clippy --manifest-path tools/mcp-auth-test-server/Cargo.toml --all-targets --all-features --locked -- -D warnings` | server、OAuth fixture 无 warning |
| `T-MCPAUTH-04` | `cargo tree --manifest-path tools/mcp-auth-test-server/Cargo.toml -i rmcp@3.1.4 -e features --locked` | 唯一 RMCP 3.1.4，四个 required features 可追溯 |
| `T-MCPAUTH-05` | `WP-MCPAUTH-03` mixed-version manual/integration scenario | RMCP 2 client ↔ RMCP 3 server handshake、OAuth refresh、list/call、shutdown 全部成功且日志无 secret |

Jaco client 侧 unit tests 和 RMCP 2 tree evidence 记录在 `crates/jaco-agent` owner plan；root 仅汇总最终跨 owner 结果。
