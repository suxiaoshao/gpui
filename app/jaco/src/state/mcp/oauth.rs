use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use gpui::{
    App, AppContext, AsyncApp, AsyncWindowContext, Context, Entity, Global, Subscription, Task,
};
use jaco_agent::McpOAuthStatusSnapshot;
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

use crate::state::config::{
    self, JacoConfig, McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind,
};

const CREDENTIALS_USERNAME: &str = "mcp-oauth";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_CALLBACK_REQUEST_BYTES: usize = 64 * 1024;
const CALLBACK_RESPONSE_OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body>OAuth authorization completed. You can close this window.</body></html>";
const CALLBACK_RESPONSE_ERROR: &[u8] = b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body>OAuth authorization failed. You can close this window.</body></html>";

type CredentialCleanupCompletion = Box<dyn FnOnce(CredentialCleanupResult, &mut App) + 'static>;

struct CredentialCleanupRequest {
    keys: Vec<CredentialsKey>,
    failure_count: usize,
    first_error: Option<String>,
    completion: CredentialCleanupCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialCleanupDecision {
    Delete,
    Referenced,
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CredentialCleanupKeyOutcome {
    Deleted,
    Referenced,
    Deferred,
    Failed(String),
}

struct CredentialCleanupOwner {
    pending: Vec<CredentialCleanupRequest>,
    task: Option<Task<()>>,
    _config_subscription: Option<Subscription>,
    #[cfg(test)]
    deleted_keys: Vec<CredentialsKey>,
}

#[derive(Clone)]
struct CredentialCleanupGlobal(Entity<CredentialCleanupOwner>);

impl Global for CredentialCleanupGlobal {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CredentialCleanupResult {
    pub(crate) failure_count: usize,
    pub(crate) first_error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthorizationCodePkceConfig {
    pub(crate) scopes: Vec<String>,
    pub(crate) client_id: Option<String>,
    pub(crate) client_metadata_url: Option<String>,
    pub(crate) resource: Option<String>,
    pub(crate) callback_port: Option<u16>,
    pub(crate) callback_url: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AuthorizedCredentials {
    pub(crate) credentials: StoredCredentials,
    pub(crate) status: McpOAuthStatusSnapshot,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CredentialsKey(String);

impl CredentialsKey {
    fn new(server_id: &str, server_url: &str, oauth: &McpOAuthTomlConfig) -> Result<Self, String> {
        let url = Url::parse(server_url).map_err(|err| err.to_string())?;
        let canonical_url = canonical_server_uri(&url);
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("server_id", server_id);
        serializer.append_pair("url", &canonical_url);
        match oauth {
            McpOAuthTomlConfig::AuthorizationCodePkce {
                scopes,
                client_id,
                client_metadata_url,
                resource,
                ..
            } => {
                serializer.append_pair("flow", "authorization_code_pkce");
                append_optional_pair(&mut serializer, "client_id", client_id.as_deref());
                append_optional_pair(
                    &mut serializer,
                    "client_metadata_url",
                    client_metadata_url.as_deref(),
                );
                append_optional_pair(&mut serializer, "resource", resource.as_deref());
                for scope in scopes {
                    serializer.append_pair("scope", scope);
                }
            }
            McpOAuthTomlConfig::ClientCredentials { .. } => {
                return Err(
                    "OAuth client_credentials does not use stored browser credentials".into(),
                );
            }
        }
        Ok(Self(format!("mcp-oauth:v2:{}", serializer.finish())))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn credentials_key_for_server(
    server_id: &str,
    server: &McpServerTomlConfig,
) -> Result<Option<CredentialsKey>, String> {
    if server.transport != McpTransportKind::StreamableHttp {
        return Ok(None);
    }
    let Some(oauth) = server.oauth.as_ref() else {
        return Ok(None);
    };
    let Some(server_url) = server.url.as_deref() else {
        return Ok(None);
    };
    CredentialsKey::new(server_id, server_url, oauth).map(Some)
}

pub(crate) fn credentials_key_is_referenced(key: &CredentialsKey, config: &JacoConfig) -> bool {
    config.mcp_servers.iter().any(|(server_id, server)| {
        credentials_key_for_server(server_id, server)
            .ok()
            .flatten()
            .as_ref()
            == Some(key)
    })
}

pub(crate) fn credentials_key_for_oauth_value(
    server_id: &str,
    server_url: &str,
    oauth: &serde_json::Value,
) -> Result<Option<CredentialsKey>, String> {
    let oauth = serde_json::from_value::<McpOAuthTomlConfig>(oauth.clone())
        .map_err(|err| format!("invalid MCP OAuth config for `{server_id}`: {err}"))?;
    CredentialsKey::new(server_id, server_url, &oauth).map(Some)
}

fn append_optional_pair(
    serializer: &mut url::form_urlencoded::Serializer<'_, String>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        serializer.append_pair(key, value);
    }
}

#[cfg(test)]
pub(crate) fn legacy_credentials_key_for_test(server_url: &str) -> Result<String, String> {
    let url = Url::parse(server_url).map_err(|err| err.to_string())?;
    Ok(format!("mcp-oauth:{}", canonical_server_uri(&url)))
}

pub(crate) async fn delete_credentials(
    key: &CredentialsKey,
    cx: &mut AsyncWindowContext,
) -> Result<(), String> {
    let key = key.as_str().to_string();
    let task = cx
        .update(move |_, cx| cx.delete_credentials(&key))
        .map_err(|err| err.to_string())?;
    task.await.map_err(|err| err.to_string())
}

async fn delete_credentials_in_app(key: &CredentialsKey, cx: &mut AsyncApp) -> Result<(), String> {
    let key = key.as_str().to_string();
    let read_key = key.clone();
    let read_task = cx.update(move |cx| cx.read_credentials(&read_key));
    if read_task.await.map_err(|err| err.to_string())?.is_none() {
        return Ok(());
    }
    cx.update(move |cx| cx.delete_credentials(&key))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) fn init_credential_cleanup(cx: &mut App) {
    if cx.has_global::<CredentialCleanupGlobal>() {
        return;
    }
    let owner = cx.new(|cx| {
        let mut owner = CredentialCleanupOwner {
            pending: Vec::new(),
            task: None,
            _config_subscription: None,
            #[cfg(test)]
            deleted_keys: Vec::new(),
        };
        owner._config_subscription = Some(config::store(cx).observe(
            cx,
            |owner: &mut CredentialCleanupOwner, operation, cx| {
                owner.on_config_operation_changed(operation, cx)
            },
        ));
        owner
    });
    cx.set_global(CredentialCleanupGlobal(owner));
}

pub(crate) fn schedule_credential_cleanup(
    mut keys: Vec<CredentialsKey>,
    completion: impl FnOnce(CredentialCleanupResult, &mut App) + 'static,
    cx: &mut App,
) {
    if keys.is_empty() {
        cx.defer(move |cx| {
            completion(
                CredentialCleanupResult {
                    failure_count: 0,
                    first_error: None,
                },
                cx,
            );
        });
        return;
    }
    init_credential_cleanup(cx);
    keys.sort();
    keys.dedup();
    let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
    owner.update(cx, |owner, cx| {
        owner.pending.push(CredentialCleanupRequest {
            keys,
            failure_count: 0,
            first_error: None,
            completion: Box::new(completion),
        });
        owner.process_pending(cx);
    });
}

impl CredentialCleanupOwner {
    fn on_config_operation_changed(
        &mut self,
        operation: &config::ConfigOperation,
        cx: &mut Context<Self>,
    ) {
        if !operation.is_running() && operation.data().is_some() {
            self.process_terminal(cx);
        }
    }

    fn process_pending(&mut self, cx: &mut Context<Self>) {
        if self.task.is_some() {
            return;
        }
        let can_process = config::store(cx).read(cx, |operation| {
            !operation.is_running() && operation.data().is_some()
        });
        if can_process {
            self.process_terminal(cx);
        }
    }

    fn process_terminal(&mut self, cx: &mut Context<Self>) {
        if self.pending.is_empty() || self.task.is_some() {
            return;
        }
        let requests = std::mem::take(&mut self.pending);
        let keys = requests
            .iter()
            .flat_map(|request| request.keys.iter().cloned())
            .collect::<BTreeSet<_>>();
        let task = cx.spawn(async move |owner, cx| {
            let mut outcomes = BTreeMap::new();
            for key in keys {
                let decision = cx.update(|cx| {
                    config::store(cx)
                        .read(cx, |operation| credential_cleanup_decision(&key, operation))
                });
                let outcome = match decision {
                    CredentialCleanupDecision::Delete => {
                        match delete_credentials_in_app(&key, cx).await {
                            Ok(()) => CredentialCleanupKeyOutcome::Deleted,
                            Err(error) => CredentialCleanupKeyOutcome::Failed(error),
                        }
                    }
                    CredentialCleanupDecision::Referenced => {
                        CredentialCleanupKeyOutcome::Referenced
                    }
                    CredentialCleanupDecision::Deferred => CredentialCleanupKeyOutcome::Deferred,
                };
                outcomes.insert(key, outcome);
            }
            let _ = owner.update(cx, |owner, cx| {
                owner.finish_task(requests, outcomes, cx);
            });
        });
        self.task = Some(task);
    }

    fn finish_task(
        &mut self,
        requests: Vec<CredentialCleanupRequest>,
        outcomes: BTreeMap<CredentialsKey, CredentialCleanupKeyOutcome>,
        cx: &mut Context<Self>,
    ) {
        self.task.take();
        #[cfg(test)]
        self.deleted_keys
            .extend(outcomes.iter().filter_map(|(key, outcome)| {
                matches!(outcome, CredentialCleanupKeyOutcome::Deleted).then_some(key.clone())
            }));

        let mut completions = Vec::new();
        for mut request in requests {
            let mut deferred = Vec::new();
            for key in request.keys {
                match outcomes.get(&key) {
                    Some(CredentialCleanupKeyOutcome::Failed(error)) => {
                        request.failure_count += 1;
                        if request.first_error.is_none() {
                            request.first_error = Some(error.clone());
                        }
                    }
                    Some(CredentialCleanupKeyOutcome::Deferred) | None => deferred.push(key),
                    Some(
                        CredentialCleanupKeyOutcome::Deleted
                        | CredentialCleanupKeyOutcome::Referenced,
                    ) => {}
                }
            }
            if deferred.is_empty() {
                completions.push((
                    request.completion,
                    CredentialCleanupResult {
                        failure_count: request.failure_count,
                        first_error: request.first_error,
                    },
                ));
            } else {
                request.keys = deferred;
                self.pending.push(request);
            }
        }
        self.process_pending(cx);
        if !completions.is_empty() {
            cx.defer(move |cx| {
                for (completion, result) in completions {
                    completion(result, cx);
                }
            });
        }
    }
}

fn credential_cleanup_decision(
    key: &CredentialsKey,
    operation: &config::ConfigOperation,
) -> CredentialCleanupDecision {
    if operation.is_running() {
        return CredentialCleanupDecision::Deferred;
    }
    match operation.data() {
        Some(config) if credentials_key_is_referenced(key, config) => {
            CredentialCleanupDecision::Referenced
        }
        Some(_) => CredentialCleanupDecision::Delete,
        None => CredentialCleanupDecision::Deferred,
    }
}

pub(crate) async fn read_credentials(
    key: &CredentialsKey,
    cx: &mut AsyncApp,
) -> Result<Option<StoredCredentials>, String> {
    let key = key.as_str().to_string();
    let task = cx.update(move |cx| cx.read_credentials(&key));
    let Some((_, bytes)) = task.await.map_err(|err| err.to_string())? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|err| err.to_string())
}

pub(crate) async fn write_credentials(
    key: &CredentialsKey,
    credentials: &StoredCredentials,
    cx: &mut AsyncWindowContext,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(credentials).map_err(|err| err.to_string())?;
    let key = key.as_str().to_string();
    let task = cx
        .update(move |_, cx| cx.write_credentials(&key, CREDENTIALS_USERNAME, &bytes))
        .map_err(|err| err.to_string())?;
    task.await.map_err(|err| err.to_string())
}

pub(crate) async fn write_credentials_value(
    key: &CredentialsKey,
    credentials_value: serde_json::Value,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let credentials = serde_json::from_value::<StoredCredentials>(credentials_value)
        .map_err(|err| err.to_string())?;
    let bytes = serde_json::to_vec(&credentials).map_err(|err| err.to_string())?;
    let key = key.as_str().to_string();
    cx.update(move |cx| cx.write_credentials(&key, CREDENTIALS_USERNAME, &bytes))
        .await
        .map_err(|err| err.to_string())
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
        let resource = config.resource.as_deref();

        if let Some(client_id) = config.client_id.as_deref().filter(|id| !id.is_empty()) {
            let mut manager = AuthorizationManager::new(server_url).await?;
            let metadata = manager.discover_metadata().await?;
            manager.set_metadata(metadata);
            manager.configure_client(
                OAuthClientConfig::new(client_id, redirect_uri.clone())
                    .with_scopes(config.scopes.clone()),
            )?;
            let authorization_url = manager.get_authorization_url(&scopes).await?;
            let authorization_url = authorization_url_with_resource(&authorization_url, resource);
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
                    Some("Jaco"),
                    config.client_metadata_url.as_deref(),
                )
                .await?;
            let authorization_url = state.get_authorization_url().await?;
            let authorization_url = authorization_url_with_resource(&authorization_url, resource);
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

fn authorization_url_with_resource(authorization_url: &str, resource: Option<&str>) -> String {
    let Some(resource) = resource
        .map(str::trim)
        .filter(|resource| !resource.is_empty())
    else {
        return authorization_url.to_string();
    };

    match Url::parse(authorization_url) {
        Ok(mut url) => {
            let query_pairs = url
                .query_pairs()
                .filter(|(key, _)| key != "resource")
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>();
            url.set_query(None);
            {
                let mut query = url.query_pairs_mut();
                for (key, value) in query_pairs {
                    query.append_pair(&key, &value);
                }
                query.append_pair("resource", resource);
            }
            url.to_string()
        }
        Err(_) => {
            let separator = if authorization_url.contains('?') {
                if authorization_url.ends_with('?') || authorization_url.ends_with('&') {
                    ""
                } else {
                    "&"
                }
            } else {
                "?"
            };
            let resource =
                url::form_urlencoded::byte_serialize(resource.as_bytes()).collect::<String>();
            format!("{authorization_url}{separator}resource={resource}")
        }
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
    let config = resolve_callback_listener_config(preferred_port, configured_redirect_uri)
        .map_err(rmcp::transport::auth::AuthError::InternalError)?;
    let listener = TcpListener::bind((config.bind_host.as_str(), config.bind_port))
        .await
        .map_err(|err| rmcp::transport::auth::AuthError::InternalError(err.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|err| rmcp::transport::auth::AuthError::InternalError(err.to_string()))?
        .port();
    let redirect_uri = config
        .redirect_uri
        .unwrap_or_else(|| format!("http://{}:{port}/callback", config.bind_host));
    let callback_base_url = Url::parse(&redirect_uri)
        .map_err(|err| rmcp::transport::auth::AuthError::InternalError(err.to_string()))?;
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let result = receive_callback(listener, callback_base_url).await;
        let _ = tx.send(result);
    });

    Ok((redirect_uri, rx))
}

#[derive(Debug, PartialEq, Eq)]
struct CallbackListenerConfig {
    bind_host: String,
    bind_port: u16,
    redirect_uri: Option<String>,
}

fn resolve_callback_listener_config(
    preferred_port: Option<u16>,
    configured_redirect_uri: Option<&str>,
) -> Result<CallbackListenerConfig, String> {
    let Some(configured_redirect_uri) = configured_redirect_uri else {
        return Ok(CallbackListenerConfig {
            bind_host: "127.0.0.1".to_string(),
            bind_port: preferred_port.unwrap_or(0),
            redirect_uri: None,
        });
    };

    let url = Url::parse(configured_redirect_uri).map_err(|err| err.to_string())?;
    if url.scheme() != "http" {
        return Err(format!(
            "OAuth callback URL `{configured_redirect_uri}` must use http loopback"
        ));
    }
    if url.fragment().is_some() {
        return Err(format!(
            "OAuth callback URL `{configured_redirect_uri}` must not include a fragment"
        ));
    }
    let bind_host = match url.host() {
        Some(url::Host::Ipv4(ip)) if ip.is_loopback() => ip.to_string(),
        Some(url::Host::Ipv6(ip)) if ip.is_loopback() => ip.to_string(),
        Some(url::Host::Domain(domain)) if domain.eq_ignore_ascii_case("localhost") => {
            domain.to_string()
        }
        Some(host) => {
            return Err(format!(
                "OAuth callback URL `{configured_redirect_uri}` must use a loopback host, got `{host}`"
            ));
        }
        None => {
            return Err(format!(
                "OAuth callback URL `{configured_redirect_uri}` must include a host"
            ));
        }
    };
    let bind_port = url.port().ok_or_else(|| {
        format!("OAuth callback URL `{configured_redirect_uri}` must include an explicit port")
    })?;
    if let Some(preferred_port) = preferred_port
        && preferred_port != bind_port
    {
        return Err(format!(
            "OAuth callback_port `{preferred_port}` does not match callback_url port `{bind_port}`"
        ));
    }

    Ok(CallbackListenerConfig {
        bind_host,
        bind_port,
        redirect_uri: Some(url.to_string()),
    })
}

async fn receive_callback(listener: TcpListener, callback_base_url: Url) -> Result<String, String> {
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
    let result = callback_url_from_request(&request, &callback_base_url);
    let response = if result.is_ok() {
        CALLBACK_RESPONSE_OK
    } else {
        CALLBACK_RESPONSE_ERROR
    };
    let _ = stream.write_all(response).await;
    result
}

fn callback_url_from_request(request: &str, callback_base_url: &Url) -> Result<String, String> {
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
        Url::parse(target).map_err(|err| err.to_string())?
    } else {
        callback_base_url
            .join(target)
            .map_err(|err| err.to_string())?
    };
    if callback_url.path() != callback_base_url.path() {
        return Err(format!(
            "OAuth callback path `{}` does not match expected path `{}`",
            callback_url.path(),
            callback_base_url.path()
        ));
    }
    Ok(callback_url.to_string())
}

fn canonical_server_uri(url: &Url) -> String {
    let mut canonical = url.clone();
    canonical.set_fragment(None);
    canonical.set_query(None);
    canonical.as_str().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::{
        CallbackListenerConfig, CredentialCleanupGlobal, authorization_url_with_resource,
        callback_url_from_request, canonical_server_uri, credentials_key_for_server,
        credentials_key_is_referenced, init_credential_cleanup, legacy_credentials_key_for_test,
        resolve_callback_listener_config, schedule_credential_cleanup,
    };
    use crate::state::config::{
        self, ConfigOperation, ConfigProblem, JacoConfig, McpOAuthTomlConfig, McpServerTomlConfig,
        McpTransportKind,
    };
    use gpui::{Task, TestAppContext};
    use gpui_operation::{Complete, Refresh, Settle, Transition};
    use url::Url;

    #[test]
    fn credentials_key_uses_canonical_server_uri_without_query_or_fragment() {
        let key =
            legacy_credentials_key_for_test("https://example.com/mcp/?token=secret#frag").unwrap();
        assert_eq!(key, "mcp-oauth:https://example.com/mcp");
    }

    #[test]
    fn credentials_key_includes_server_id_and_oauth_audience() {
        let mut server = oauth_server("https://example.com/mcp", None);
        let first = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let second = credentials_key_for_server("server-b", &server)
            .unwrap()
            .unwrap();
        assert_ne!(first, second);

        if let Some(McpOAuthTomlConfig::AuthorizationCodePkce { resource, .. }) =
            server.oauth.as_mut()
        {
            *resource = Some("https://api.example.com/audience".to_string());
        }
        let audience_key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        assert_ne!(first, audience_key);
    }

    #[test]
    fn draft_cancel_and_post_commit_cleanup_skip_latest_config_references() {
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let mut config = JacoConfig::default();
        config.mcp_servers.insert("server-a".to_string(), server);

        assert!(credentials_key_is_referenced(&key, &config));
        config.mcp_servers.clear();
        assert!(!credentials_key_is_referenced(&key, &config));
    }

    #[gpui::test]
    fn credential_cleanup_waits_for_ready_then_deletes_unreferenced_key(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();

        let completion_count = Rc::new(Cell::new(0));
        let completion_count_for_cleanup = Rc::clone(&completion_count);
        cx.update(|cx| {
            config::install_for_test(cx, path, JacoConfig::default()).unwrap();
            init_credential_cleanup(cx);
            let data = config::store(cx).read(cx, |operation| operation.data().unwrap().clone());
            config::store(cx).update(cx, |operation| {
                operation.transition(Refresh(Task::ready(())));
            });
            schedule_credential_cleanup(
                vec![key.clone()],
                move |result, _cx| {
                    assert_eq!(result.failure_count, 0);
                    completion_count_for_cleanup.set(completion_count_for_cleanup.get() + 1);
                },
                cx,
            );
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert_eq!(owner.read(cx).pending.len(), 1);
            assert!(owner.read(cx).task.is_none());

            config::store(cx).update(cx, |operation| {
                operation.transition(Complete(Ok(data)));
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).pending.is_empty());
            assert!(owner.read(cx).task.is_none());
            assert_eq!(owner.read(cx).deleted_keys, vec![key]);
        });
        assert_eq!(completion_count.get(), 1);
    }

    #[gpui::test]
    fn credential_cleanup_waits_for_ready_then_retains_referenced_key(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let mut referenced_config = JacoConfig::default();
        referenced_config
            .mcp_servers
            .insert("server-a".to_string(), server);
        let completed = Rc::new(Cell::new(false));
        let completed_for_cleanup = Rc::clone(&completed);

        cx.update(|cx| {
            config::install_for_test(cx, path, referenced_config).unwrap();
            init_credential_cleanup(cx);
            let data = config::store(cx).read(cx, |operation| operation.data().unwrap().clone());
            config::store(cx).update(cx, |operation| {
                operation.transition(Refresh(Task::ready(())));
            });
            schedule_credential_cleanup(
                vec![key],
                move |result, _cx| {
                    assert_eq!(result.failure_count, 0);
                    completed_for_cleanup.set(true);
                },
                cx,
            );
            config::store(cx).update(cx, |operation| {
                operation.transition(Complete(Ok(data)));
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).pending.is_empty());
            assert!(owner.read(cx).task.is_none());
            assert!(owner.read(cx).deleted_keys.is_empty());
        });
        assert!(completed.get());
    }

    #[gpui::test]
    fn credential_cleanup_running_publications_do_not_duplicate_or_drop_request(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let completion_count = Rc::new(Cell::new(0));
        let completion_count_for_first = Rc::clone(&completion_count);
        let completion_count_for_second = Rc::clone(&completion_count);

        cx.update(|cx| {
            config::install_for_test(cx, path, JacoConfig::default()).unwrap();
            init_credential_cleanup(cx);
            let data = config::store(cx).read(cx, |operation| operation.data().unwrap().clone());
            config::store(cx).update(cx, |operation| {
                operation.transition(Refresh(Task::ready(())));
            });
            schedule_credential_cleanup(
                vec![key.clone()],
                move |result, _cx| {
                    assert_eq!(result.failure_count, 0);
                    completion_count_for_first.set(completion_count_for_first.get() + 1);
                },
                cx,
            );
            schedule_credential_cleanup(
                vec![key.clone()],
                move |result, _cx| {
                    assert_eq!(result.failure_count, 0);
                    completion_count_for_second.set(completion_count_for_second.get() + 1);
                },
                cx,
            );
            config::store(cx).update(cx, |_| {});
            config::store(cx).update(cx, |_| {});
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert_eq!(owner.read(cx).pending.len(), 2);
            assert!(owner.read(cx).task.is_none());

            config::store(cx).update(cx, |operation| {
                operation.transition(Complete(Ok(data)));
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).pending.is_empty());
            assert!(owner.read(cx).task.is_none());
            assert_eq!(owner.read(cx).deleted_keys, vec![key]);
        });
        assert_eq!(completion_count.get(), 2);
    }

    #[gpui::test]
    fn credential_cleanup_rechecks_config_before_delete_and_requeues_when_running(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let completed = Rc::new(Cell::new(false));
        let completed_for_cleanup = Rc::clone(&completed);

        cx.update(|cx| {
            config::install_for_test(cx, path, JacoConfig::default()).unwrap();
            init_credential_cleanup(cx);
            schedule_credential_cleanup(
                vec![key.clone()],
                move |result, _cx| {
                    assert_eq!(result.failure_count, 0);
                    completed_for_cleanup.set(true);
                },
                cx,
            );
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).task.is_some());
            config::store(cx).update(cx, |operation| {
                operation.transition(Refresh(Task::ready(())));
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert_eq!(owner.read(cx).pending.len(), 1);
            assert!(owner.read(cx).task.is_none());
            assert!(owner.read(cx).deleted_keys.is_empty());
        });
        assert!(!completed.get());

        cx.update(|cx| {
            let data = config::store(cx).read(cx, |operation| operation.data().unwrap().clone());
            config::store(cx).update(cx, |operation| {
                operation.transition(Complete(Ok(data)));
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).pending.is_empty());
            assert!(owner.read(cx).task.is_none());
            assert_eq!(owner.read(cx).deleted_keys, vec![key]);
        });
        assert!(completed.get());
    }

    #[gpui::test]
    fn credential_cleanup_completion_can_schedule_another_cleanup(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let completion_count = Rc::new(Cell::new(0));
        let first_completion_count = Rc::clone(&completion_count);
        let second_completion_count = Rc::clone(&completion_count);

        cx.update(|cx| {
            config::install_for_test(cx, path, JacoConfig::default()).unwrap();
            init_credential_cleanup(cx);
            schedule_credential_cleanup(
                vec![key.clone()],
                move |result, cx| {
                    assert_eq!(result.failure_count, 0);
                    first_completion_count.set(first_completion_count.get() + 1);
                    schedule_credential_cleanup(
                        vec![key],
                        move |result, _cx| {
                            assert_eq!(result.failure_count, 0);
                            second_completion_count.set(second_completion_count.get() + 1);
                        },
                        cx,
                    );
                },
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).pending.is_empty());
            assert!(owner.read(cx).task.is_none());
            assert_eq!(owner.read(cx).deleted_keys.len(), 2);
        });
        assert_eq!(completion_count.get(), 2);
    }

    #[gpui::test]
    fn credential_cleanup_without_config_data_remains_pending_until_data_is_available(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let server = oauth_server("https://example.com/mcp", None);
        let key = credentials_key_for_server("server-a", &server)
            .unwrap()
            .unwrap();
        let completed = Rc::new(Cell::new(false));
        let completed_for_cleanup = Rc::clone(&completed);

        let data = cx.update(|cx| {
            config::install_for_test(cx, path, JacoConfig::default()).unwrap();
            let data = config::store(cx).read(cx, |operation| operation.data().unwrap().clone());
            let mut unavailable = ConfigOperation::new();
            unavailable.transition(Settle(Err(ConfigProblem::ResolveDirectory {
                message: "unavailable for test".to_string(),
            })));
            config::store(cx).set(cx, unavailable);
            init_credential_cleanup(cx);
            schedule_credential_cleanup(
                vec![key.clone()],
                move |result, _cx| {
                    assert_eq!(result.failure_count, 0);
                    completed_for_cleanup.set(true);
                },
                cx,
            );
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert_eq!(owner.read(cx).pending.len(), 1);
            assert!(owner.read(cx).task.is_none());
            data
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert_eq!(owner.read(cx).pending.len(), 1);
            assert!(owner.read(cx).task.is_none());
        });
        assert!(!completed.get());
        cx.update(|cx| {
            let mut ready = ConfigOperation::new();
            ready.transition(Settle(Ok(data)));
            config::store(cx).set(cx, ready);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = cx.global::<CredentialCleanupGlobal>().0.clone();
            assert!(owner.read(cx).pending.is_empty());
            assert!(owner.read(cx).task.is_none());
            assert_eq!(owner.read(cx).deleted_keys, vec![key]);
        });
        assert!(completed.get());
    }

    #[test]
    fn canonical_server_uri_preserves_path() {
        let url = Url::parse("http://127.0.0.1:8787/mcp").unwrap();
        assert_eq!(canonical_server_uri(&url), "http://127.0.0.1:8787/mcp");
    }

    #[test]
    fn callback_url_from_relative_request_target() {
        let callback_base_url = Url::parse("http://127.0.0.1:49152/callback").unwrap();
        let request = "GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(
            callback_url_from_request(request, &callback_base_url).unwrap(),
            "http://127.0.0.1:49152/callback?code=abc&state=xyz"
        );
    }

    #[test]
    fn callback_url_from_request_uses_configured_callback_origin() {
        let callback_base_url = Url::parse("http://localhost:49152/oauth/callback").unwrap();
        let request = "GET /oauth/callback?code=abc&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";

        assert_eq!(
            callback_url_from_request(request, &callback_base_url).unwrap(),
            "http://localhost:49152/oauth/callback?code=abc&state=xyz"
        );
    }

    #[test]
    fn callback_url_from_request_rejects_wrong_path() {
        let callback_base_url = Url::parse("http://127.0.0.1:49152/oauth/callback").unwrap();
        let request = "GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";

        assert!(
            callback_url_from_request(request, &callback_base_url)
                .unwrap_err()
                .contains("does not match expected path")
        );
    }

    #[test]
    fn callback_listener_config_uses_callback_url_port() {
        let config =
            resolve_callback_listener_config(None, Some("http://127.0.0.1:49152/oauth/callback"))
                .unwrap();

        assert_eq!(
            config,
            CallbackListenerConfig {
                bind_host: "127.0.0.1".to_string(),
                bind_port: 49152,
                redirect_uri: Some("http://127.0.0.1:49152/oauth/callback".to_string()),
            }
        );
    }

    #[test]
    fn callback_listener_config_rejects_callback_port_mismatch() {
        let err = resolve_callback_listener_config(
            Some(49153),
            Some("http://127.0.0.1:49152/oauth/callback"),
        )
        .unwrap_err();

        assert!(err.contains("does not match callback_url port"));
    }

    #[test]
    fn callback_listener_config_rejects_non_loopback_callback_url() {
        let err =
            resolve_callback_listener_config(None, Some("http://example.com:49152/oauth/callback"))
                .unwrap_err();

        assert!(err.contains("must use a loopback host"));
    }

    #[test]
    fn callback_listener_config_rejects_missing_callback_url_port() {
        let err = resolve_callback_listener_config(None, Some("http://127.0.0.1/oauth/callback"))
            .unwrap_err();

        assert!(err.contains("must include an explicit port"));
    }

    #[test]
    fn authorization_url_with_resource_replaces_default_resource() {
        let url = authorization_url_with_resource(
            "https://auth.example.com/authorize?client_id=client&resource=https%3A%2F%2Fold.example.com%2Fmcp&state=abc",
            Some("https://api.example.com/mcp"),
        );
        let url = Url::parse(&url).unwrap();
        let pairs = url.query_pairs().collect::<Vec<_>>();

        assert_eq!(
            pairs
                .iter()
                .filter(|(key, _)| key.as_ref() == "resource")
                .count(),
            1
        );
        assert!(
            pairs
                .iter()
                .any(|(key, value)| key.as_ref() == "client_id" && value.as_ref() == "client")
        );
        assert!(
            pairs
                .iter()
                .any(|(key, value)| key.as_ref() == "state" && value.as_ref() == "abc")
        );
        assert!(pairs.iter().any(|(key, value)| key.as_ref() == "resource"
            && value.as_ref() == "https://api.example.com/mcp"));
    }

    #[test]
    fn authorization_url_with_resource_ignores_blank_resource() {
        let url = "https://auth.example.com/authorize?client_id=client";

        assert_eq!(authorization_url_with_resource(url, Some("  ")), url);
    }

    fn oauth_server(url: &str, resource: Option<String>) -> McpServerTomlConfig {
        McpServerTomlConfig {
            transport: McpTransportKind::StreamableHttp,
            url: Some(url.to_string()),
            oauth: Some(McpOAuthTomlConfig::AuthorizationCodePkce {
                scopes: Vec::new(),
                client_id: None,
                client_metadata_url: None,
                resource,
                callback_port: None,
                callback_url: None,
            }),
            ..Default::default()
        }
    }
}
