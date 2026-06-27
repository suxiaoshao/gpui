# MCP Auth Test Server

Local Streamable HTTP MCP server for testing ai-chat2 MCP bearer/OAuth auth paths.

## Run

```sh
cargo run --manifest-path tools/mcp-auth-test-server/Cargo.toml
```

Defaults:

- Bind: `127.0.0.1:8787`
- MCP endpoint: `http://127.0.0.1:8787/mcp`
- Static bearer token: `mcp-test-access-token`
- OAuth public client id returned by dynamic registration: `mcp-test-public-client`
- OAuth authorization code accepted by `/token`: `mcp-test-auth-code`
- OAuth refresh token: `mcp-test-refresh-token`

Optional environment overrides:

```sh
MCP_AUTH_TEST_HOST=127.0.0.1
MCP_AUTH_TEST_PORT=8787
MCP_AUTH_TEST_BASE_URL=http://127.0.0.1:8787
```

## ai-chat2 Static Bearer Config

Set:

```sh
export MCP_AUTH_TEST_TOKEN=mcp-test-access-token
```

Then add to `config.toml`:

```toml
[mcp_servers.auth_test]
enabled = true
transport = "streamable_http"
url = "http://127.0.0.1:8787/mcp"
bearer_token_env_var = "MCP_AUTH_TEST_TOKEN"
```

## ai-chat2 OAuth Config

OAuth browser flow is meant for phase 2 testing:

```toml
[mcp_servers.auth_test_oauth]
enabled = true
transport = "streamable_http"
url = "http://127.0.0.1:8787/mcp"

[mcp_servers.auth_test_oauth.oauth]
flow = "authorization_code_pkce"
scopes = ["read", "write"]
resource = "http://127.0.0.1:8787/mcp"
```

The test server supports:

- `/.well-known/oauth-protected-resource`
- `/.well-known/oauth-protected-resource/mcp`
- `/.well-known/oauth-authorization-server`
- `/register`
- `/authorize`
- `/token`

`/authorize` auto-approves and redirects back to the supplied `redirect_uri` with `code`, `state`, and `iss`.

## Smoke Tests

```sh
curl -i http://127.0.0.1:8787/health

curl -i -X POST http://127.0.0.1:8787/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"curl","version":"0.1"}}}'

curl -i -X POST http://127.0.0.1:8787/mcp \
  -H 'Authorization: Bearer mcp-test-access-token' \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'

curl -i -X POST http://127.0.0.1:8787/mcp \
  -H 'Authorization: Bearer mcp-test-access-token' \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hello"}}}'
```
