use std::{collections::HashMap, net::SocketAddr};

use anyhow::Context as _;
use axum::{
    Json, Router,
    extract::{Query, Request},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8787;
const ACCESS_TOKEN: &str = "mcp-test-access-token";
const REFRESH_TOKEN: &str = "mcp-test-refresh-token";

#[derive(Debug, Clone)]
struct AppState {
    public_base_url: String,
}

#[derive(Debug, Clone)]
struct EchoServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EchoRequest {
    #[schemars(description = "Text to echo from the authenticated MCP test server")]
    text: String,
}

impl EchoServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl EchoServer {
    #[tool(description = "Echo text from the authenticated MCP test server")]
    fn echo(&self, Parameters(EchoRequest { text }): Parameters<EchoRequest>) -> String {
        format!("auth ok: {text}")
    }
}

#[tool_handler]
impl ServerHandler for EchoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Local MCP auth test server")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let host = std::env::var("MCP_AUTH_TEST_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = std::env::var("MCP_AUTH_TEST_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("invalid bind address {host}:{port}"))?;
    let public_base_url =
        std::env::var("MCP_AUTH_TEST_BASE_URL").unwrap_or_else(|_| format!("http://{addr}"));

    let cancellation_token = CancellationToken::new();
    let service: StreamableHttpService<EchoServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(EchoServer::new()),
            Default::default(),
            StreamableHttpServerConfig::default()
                .with_stateful_mode(false)
                .with_json_response(true)
                .with_sse_keep_alive(None)
                .with_cancellation_token(cancellation_token.child_token()),
        );

    let state = AppState {
        public_base_url: public_base_url.clone(),
    };

    let mcp_router = Router::new()
        .nest_service("/mcp", service)
        .route_layer(middleware::from_fn(require_bearer_token));

    let app = Router::new()
        .route("/health", get(health))
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route("/register", post(register_client))
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .merge(mcp_router)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    println!("mcp-auth-test-server listening on http://{actual_addr}");
    println!("MCP endpoint: http://{actual_addr}/mcp");
    println!("Access token for static-header tests: {ACCESS_TOKEN}");
    println!("OAuth flow: dynamic registration + authorization_code + refresh_token");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation_token.cancel();
        })
        .await?;

    Ok(())
}

async fn require_bearer_token(headers: HeaderMap, request: Request, next: Next) -> Response {
    let expected = format!("Bearer {ACCESS_TOKEN}");
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected)
    {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        [(
            header::WWW_AUTHENTICATE,
            r#"Bearer resource_metadata="http://127.0.0.1:8787/.well-known/oauth-protected-resource/mcp", scope="read write""#,
        )],
        Json(serde_json::json!({
            "error": "authorization_required",
            "error_description": "Use OAuth or Authorization: Bearer mcp-test-access-token"
        })),
    )
        .into_response()
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "service": "mcp-auth-test-server"
    }))
}

async fn protected_resource_metadata(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "resource": format!("{}/mcp", state.public_base_url),
        "authorization_servers": [state.public_base_url],
        "scopes_supported": ["read", "write", "offline_access"],
        "bearer_methods_supported": ["header"],
        "resource_documentation": format!("{}/health", state.public_base_url)
    }))
}

async fn authorization_server_metadata(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "issuer": state.public_base_url,
        "authorization_endpoint": format!("{}/authorize", state.public_base_url),
        "token_endpoint": format!("{}/token", state.public_base_url),
        "registration_endpoint": format!("{}/register", state.public_base_url),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["read", "write", "offline_access"],
        "authorization_response_iss_parameter_supported": true
    }))
}

async fn register_client(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let redirect_uris = body
        .get("redirect_uris")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    Json(serde_json::json!({
        "client_id": "mcp-test-public-client",
        "client_name": body.get("client_name").cloned().unwrap_or_else(|| serde_json::json!("jaco test client")),
        "redirect_uris": redirect_uris,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }))
}

async fn authorize(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let Some(redirect_uri) = query.get("redirect_uri") else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_redirect_uri"})),
        )
            .into_response();
    };
    let state_value = query.get("state").cloned().unwrap_or_default();

    let mut url = match url::Url::parse(redirect_uri) {
        Ok(url) => url,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_redirect_uri",
                    "error_description": error.to_string()
                })),
            )
                .into_response();
        }
    };
    url.query_pairs_mut()
        .append_pair("code", "mcp-test-auth-code")
        .append_pair("state", &state_value)
        .append_pair("iss", &state.public_base_url);

    Redirect::temporary(url.as_str()).into_response()
}

#[derive(Debug, Deserialize)]
struct TokenForm {
    grant_type: String,
    code: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
}

async fn token(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Form(form): axum::Form<TokenForm>,
) -> Response {
    if form.client_id.as_deref() != Some("mcp-test-public-client") {
        return oauth_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "unknown client_id",
        );
    }

    match form.grant_type.as_str() {
        "authorization_code" if form.code.as_deref() == Some("mcp-test-auth-code") => {
            token_response(&state.public_base_url)
        }
        "refresh_token" if form.refresh_token.as_deref() == Some(REFRESH_TOKEN) => {
            token_response(&state.public_base_url)
        }
        "authorization_code" => oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "expected mcp-test-auth-code",
        ),
        "refresh_token" => oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "expected mcp-test-refresh-token",
        ),
        _ => oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "supported grants: authorization_code, refresh_token",
        ),
    }
}

fn token_response(issuer: &str) -> Response {
    Json(serde_json::json!({
        "access_token": ACCESS_TOKEN,
        "token_type": "Bearer",
        "expires_in": 3600,
        "refresh_token": REFRESH_TOKEN,
        "scope": "read write offline_access",
        "iss": issuer
    }))
    .into_response()
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": error,
            "error_description": description
        })),
    )
        .into_response()
}
