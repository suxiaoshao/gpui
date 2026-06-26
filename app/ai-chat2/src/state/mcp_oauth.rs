use std::time::Duration;

use ai_chat_agent::McpOAuthStatusSnapshot;
use gpui::{App, AsyncWindowContext, TaskExt as _};
use rmcp::transport::{
    StoredCredentials,
    auth::{AuthorizationCallback, AuthorizationManager, OAuthClientConfig, OAuthState},
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::oneshot,
};
use url::Url;

const CREDENTIALS_USERNAME: &str = "mcp-oauth";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_CALLBACK_REQUEST_BYTES: usize = 64 * 1024;
const CALLBACK_RESPONSE_OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body>OAuth authorization completed. You can close this window.</body></html>";
const CALLBACK_RESPONSE_ERROR: &[u8] = b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body>OAuth authorization failed. You can close this window.</body></html>";

#[derive(Clone, Debug)]
pub(crate) struct AuthorizationCodePkceConfig {
    pub(crate) scopes: Vec<String>,
    pub(crate) client_id: Option<String>,
    pub(crate) client_metadata_url: Option<String>,
    pub(crate) callback_port: Option<u16>,
    pub(crate) callback_url: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AuthorizedCredentials {
    pub(crate) credentials: StoredCredentials,
    pub(crate) status: McpOAuthStatusSnapshot,
}

pub(crate) fn credentials_key(server_url: &str) -> Result<String, String> {
    let url = Url::parse(server_url).map_err(|err| err.to_string())?;
    Ok(format!("mcp-oauth:{}", canonical_server_uri(&url)))
}

pub(crate) async fn delete_credentials(
    server_url: &str,
    cx: &mut AsyncWindowContext,
) -> Result<(), String> {
    let key = credentials_key(server_url)?;
    let task = cx
        .update(move |_, cx| cx.delete_credentials(&key))
        .map_err(|err| err.to_string())?;
    task.await.map_err(|err| err.to_string())
}

pub(crate) fn delete_credentials_detached(server_url: &str, cx: &mut App) -> Result<(), String> {
    let key = credentials_key(server_url)?;
    cx.delete_credentials(&key).detach_and_log_err(cx);
    Ok(())
}

pub(crate) async fn read_credentials(
    server_url: &str,
    cx: &mut AsyncWindowContext,
) -> Result<Option<StoredCredentials>, String> {
    let key = credentials_key(server_url)?;
    let task = cx
        .update(move |_, cx| cx.read_credentials(&key))
        .map_err(|err| err.to_string())?;
    let Some((_, bytes)) = task.await.map_err(|err| err.to_string())? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|err| err.to_string())
}

pub(crate) async fn write_credentials(
    server_url: &str,
    credentials: &StoredCredentials,
    cx: &mut AsyncWindowContext,
) -> Result<(), String> {
    let key = credentials_key(server_url)?;
    let bytes = serde_json::to_vec(credentials).map_err(|err| err.to_string())?;
    let task = cx
        .update(move |_, cx| cx.write_credentials(&key, CREDENTIALS_USERNAME, &bytes))
        .map_err(|err| err.to_string())?;
    task.await.map_err(|err| err.to_string())
}

pub(crate) fn write_credentials_detached(
    server_url: &str,
    credentials_value: serde_json::Value,
    cx: &mut App,
) -> Result<(), String> {
    let credentials = serde_json::from_value::<StoredCredentials>(credentials_value)
        .map_err(|err| err.to_string())?;
    let key = credentials_key(server_url)?;
    let bytes = serde_json::to_vec(&credentials).map_err(|err| err.to_string())?;
    cx.write_credentials(&key, CREDENTIALS_USERNAME, &bytes)
        .detach_and_log_err(cx);
    Ok(())
}

pub(crate) async fn authorize_with_browser(
    server_url: String,
    config: AuthorizationCodePkceConfig,
    cx: &mut AsyncWindowContext,
) -> Result<AuthorizedCredentials, String> {
    let start = gpui_tokio::Tokio::spawn(cx, async move {
        let (redirect_uri, callback_rx) =
            start_loopback_callback(config.callback_port, config.callback_url.as_deref()).await?;
        let scopes = config.scopes.iter().map(String::as_str).collect::<Vec<_>>();

        if let Some(client_id) = config.client_id.as_deref().filter(|id| !id.is_empty()) {
            let mut manager = AuthorizationManager::new(server_url).await?;
            let metadata = manager.discover_metadata().await?;
            manager.set_metadata(metadata);
            manager.configure_client(
                OAuthClientConfig::new(client_id, redirect_uri.clone())
                    .with_scopes(config.scopes.clone()),
            )?;
            let authorization_url = manager.get_authorization_url(&scopes).await?;
            Ok(PendingAuthorization::Manager {
                manager,
                authorization_url,
                callback_rx,
            })
        } else {
            let mut state = OAuthState::new(server_url, None).await?;
            state
                .start_authorization_with_metadata_url(
                    &scopes,
                    &redirect_uri,
                    Some("AI Chat 2"),
                    config.client_metadata_url.as_deref(),
                )
                .await?;
            let authorization_url = state.get_authorization_url().await?;
            Ok(PendingAuthorization::State {
                state,
                authorization_url,
                callback_rx,
            })
        }
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err: rmcp::transport::auth::AuthError| err.to_string())?;

    let authorization_url = start.authorization_url().to_string();
    cx.update(move |_, cx| cx.open_url(&authorization_url))
        .map_err(|err| err.to_string())?;

    gpui_tokio::Tokio::spawn(cx, async move { start.complete().await })
        .await
        .map_err(|err| err.to_string())?
}

enum PendingAuthorization {
    State {
        state: OAuthState,
        authorization_url: String,
        callback_rx: oneshot::Receiver<Result<String, String>>,
    },
    Manager {
        manager: AuthorizationManager,
        authorization_url: String,
        callback_rx: oneshot::Receiver<Result<String, String>>,
    },
}

impl PendingAuthorization {
    fn authorization_url(&self) -> &str {
        match self {
            Self::State {
                authorization_url, ..
            }
            | Self::Manager {
                authorization_url, ..
            } => authorization_url,
        }
    }

    async fn complete(self) -> Result<AuthorizedCredentials, String> {
        match self {
            Self::State {
                mut state,
                callback_rx,
                ..
            } => {
                let callback_url = wait_for_callback(callback_rx).await?;
                state
                    .handle_callback_url(&callback_url)
                    .await
                    .map_err(|err| err.to_string())?;
                let (client_id, token_response) = state
                    .get_credentials()
                    .await
                    .map_err(|err| err.to_string())?;
                let credentials =
                    StoredCredentials::new(client_id, token_response, Vec::new(), None);
                Ok(AuthorizedCredentials {
                    status: status_from_credentials(&credentials),
                    credentials,
                })
            }
            Self::Manager {
                manager,
                callback_rx,
                ..
            } => {
                let callback_url = wait_for_callback(callback_rx).await?;
                let callback = AuthorizationCallback::from_redirect_url(&callback_url)
                    .map_err(|err| err.to_string())?;
                manager
                    .exchange_code_for_token_with_issuer(
                        &callback.code,
                        &callback.csrf_token,
                        callback.issuer.as_deref(),
                    )
                    .await
                    .map_err(|err| err.to_string())?;
                let (client_id, token_response) = manager
                    .get_credentials()
                    .await
                    .map_err(|err| err.to_string())?;
                let credentials = StoredCredentials::new(
                    client_id,
                    token_response,
                    manager.get_current_scopes().await,
                    None,
                );
                Ok(AuthorizedCredentials {
                    status: status_from_credentials(&credentials),
                    credentials,
                })
            }
        }
    }
}

fn status_from_credentials(credentials: &StoredCredentials) -> McpOAuthStatusSnapshot {
    if credentials.token_response.is_some() {
        McpOAuthStatusSnapshot::Authorized {
            scopes: credentials.granted_scopes.clone(),
            expires_at_unix_ms: None,
        }
    } else {
        McpOAuthStatusSnapshot::AuthorizationRequired
    }
}

async fn wait_for_callback(
    callback_rx: oneshot::Receiver<Result<String, String>>,
) -> Result<String, String> {
    tokio::time::timeout(CALLBACK_TIMEOUT, callback_rx)
        .await
        .map_err(|_| "OAuth callback timed out".to_string())?
        .map_err(|_| "OAuth callback listener stopped".to_string())?
}

async fn start_loopback_callback(
    preferred_port: Option<u16>,
    configured_redirect_uri: Option<&str>,
) -> Result<(String, oneshot::Receiver<Result<String, String>>), rmcp::transport::auth::AuthError> {
    let listener = TcpListener::bind(("127.0.0.1", preferred_port.unwrap_or(0)))
        .await
        .map_err(|err| rmcp::transport::auth::AuthError::InternalError(err.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|err| rmcp::transport::auth::AuthError::InternalError(err.to_string()))?
        .port();
    let redirect_uri = configured_redirect_uri
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("http://127.0.0.1:{port}/callback"));
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let result = receive_callback(listener, port).await;
        let _ = tx.send(result);
    });

    Ok((redirect_uri, rx))
}

async fn receive_callback(listener: TcpListener, port: u16) -> Result<String, String> {
    let (mut stream, _) = listener.accept().await.map_err(|err| err.to_string())?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > MAX_CALLBACK_REQUEST_BYTES {
            let _ = stream.write_all(CALLBACK_RESPONSE_ERROR).await;
            return Err("OAuth callback request is too large".to_string());
        }
    }

    let request = String::from_utf8(request).map_err(|err| err.to_string())?;
    let result = callback_url_from_request(&request, port);
    let response = if result.is_ok() {
        CALLBACK_RESPONSE_OK
    } else {
        CALLBACK_RESPONSE_ERROR
    };
    let _ = stream.write_all(response).await;
    result
}

fn callback_url_from_request(request: &str, port: u16) -> Result<String, String> {
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "OAuth callback request is empty".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "OAuth callback method is missing".to_string())?;
    let target = parts
        .next()
        .ok_or_else(|| "OAuth callback target is missing".to_string())?;
    if method != "GET" {
        return Err(format!("OAuth callback method `{method}` is not supported"));
    }

    let callback_url = if target.starts_with("http://") || target.starts_with("https://") {
        target.to_string()
    } else {
        format!("http://127.0.0.1:{port}{target}")
    };
    Url::parse(&callback_url).map_err(|err| err.to_string())?;
    Ok(callback_url)
}

fn canonical_server_uri(url: &Url) -> String {
    let mut canonical = url.clone();
    canonical.set_fragment(None);
    canonical.set_query(None);
    canonical.as_str().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::{callback_url_from_request, canonical_server_uri, credentials_key};
    use url::Url;

    #[test]
    fn credentials_key_uses_canonical_server_uri_without_query_or_fragment() {
        let key = credentials_key("https://example.com/mcp/?token=secret#frag").unwrap();
        assert_eq!(key, "mcp-oauth:https://example.com/mcp");
    }

    #[test]
    fn canonical_server_uri_preserves_path() {
        let url = Url::parse("http://127.0.0.1:8787/mcp").unwrap();
        assert_eq!(canonical_server_uri(&url), "http://127.0.0.1:8787/mcp");
    }

    #[test]
    fn callback_url_from_relative_request_target() {
        let request = "GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(
            callback_url_from_request(request, 49152).unwrap(),
            "http://127.0.0.1:49152/callback?code=abc&state=xyz"
        );
    }
}
