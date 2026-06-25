use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    path::PathBuf,
    time::Duration,
};

use ai_chat_agent::{
    McpConfigLayer, McpServerConfig, McpServerRuntimeConfig, McpServerTransport, McpStdioTransport,
    McpStreamableHttpTransport,
};
use ai_chat_core::{
    McpOAuthConfigSnapshot, McpServerRuntimeConfigSnapshot, McpServerTransportSnapshot,
    McpToolApprovalModeSnapshot, McpToolOverrideSnapshot, ToolApprovalPolicy, ToolExecutionPolicy,
};
use gpui::App;
use serde::{Deserialize, Serialize};

use crate::errors::{AiChat2Error, AiChat2Result};

use super::{AiChat2Config, read, store};

pub(crate) const DEFAULT_MCP_STARTUP_TIMEOUT_MS: u64 = 30_000;
pub(crate) const DEFAULT_MCP_TOOL_TIMEOUT_MS: u64 = 300_000;
const MAX_MCP_TIMEOUT_MS: u64 = 60 * 60 * 1000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct McpServerTomlConfig {
    #[serde(default = "default_mcp_server_enabled")]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) required: bool,
    pub(crate) display_name: Option<String>,
    pub(crate) transport: McpTransportKind,
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) headers: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) env_headers: BTreeMap<String, String>,
    pub(crate) bearer_token_env_var: Option<String>,
    pub(crate) oauth: Option<McpOAuthTomlConfig>,
    #[serde(default)]
    pub(crate) env: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) env_vars: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) startup_timeout_ms: Option<u64>,
    pub(crate) tool_timeout_ms: Option<u64>,
    pub(crate) enabled_tools: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) disabled_tools: Vec<String>,
    pub(crate) default_tools_approval_mode: Option<McpToolApprovalMode>,
    #[serde(default)]
    pub(crate) tools: BTreeMap<String, McpToolOverrideTomlConfig>,
}

impl Default for McpServerTomlConfig {
    fn default() -> Self {
        Self {
            enabled: default_mcp_server_enabled(),
            required: false,
            display_name: None,
            transport: McpTransportKind::Stdio,
            command: None,
            args: Vec::new(),
            url: None,
            headers: BTreeMap::new(),
            env_headers: BTreeMap::new(),
            bearer_token_env_var: None,
            oauth: None,
            env: BTreeMap::new(),
            env_vars: Vec::new(),
            cwd: None,
            startup_timeout_ms: None,
            tool_timeout_ms: None,
            enabled_tools: None,
            disabled_tools: Vec::new(),
            default_tools_approval_mode: None,
            tools: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum McpTransportKind {
    #[default]
    Stdio,
    StreamableHttp,
}

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum McpToolApprovalMode {
    Auto,
    Prompt,
    Deny,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct McpToolOverrideTomlConfig {
    pub(crate) approval_mode: Option<McpToolApprovalMode>,
}

impl AiChat2Config {
    pub(crate) fn mcp_config_layer(&self) -> AiChat2Result<McpConfigLayer> {
        let mut servers = Vec::new();
        for (server_id, server) in &self.mcp_servers {
            if server.enabled {
                servers.push(server.to_agent_config(server_id)?);
            }
        }
        Ok(McpConfigLayer { servers })
    }

    #[cfg(test)]
    pub(crate) fn mcp_runtime_config_snapshot(
        &self,
    ) -> AiChat2Result<ai_chat_core::McpRuntimeConfigSnapshot> {
        let mut servers = Vec::new();
        for (server_id, server) in &self.mcp_servers {
            if server.enabled {
                servers.push(server.to_runtime_config_snapshot(server_id)?);
            }
        }
        Ok(ai_chat_core::McpRuntimeConfigSnapshot { servers })
    }
}

impl McpServerTomlConfig {
    pub(crate) fn to_agent_config(&self, server_id: &str) -> AiChat2Result<McpServerConfig> {
        self.validate(server_id)?;
        let transport = match self.transport {
            McpTransportKind::Stdio => {
                let command = self.command.clone().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing command"))
                })?;
                McpServerTransport::Stdio(McpStdioTransport {
                    command,
                    args: self.args.clone(),
                })
            }
            McpTransportKind::StreamableHttp => {
                let url = self.url.clone().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing url"))
                })?;
                McpServerTransport::StreamableHttp(McpStreamableHttpTransport {
                    url,
                    headers: self.resolved_headers(server_id)?,
                    oauth: self
                        .oauth
                        .as_ref()
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(|err| {
                            AiChat2Error::Config(format!(
                                "invalid MCP OAuth config for `{server_id}`: {err}"
                            ))
                        })?,
                    oauth_credentials: None,
                })
            }
        };

        Ok(McpServerConfig {
            server_id: server_id.to_string(),
            display_name: self.display_name.clone(),
            transport,
            env: self.resolved_env(server_id)?,
            cwd: self.cwd.clone(),
        })
    }

    pub(crate) fn to_server_runtime_config(
        &self,
        server_id: &str,
        inherited_default_approval_mode: McpToolApprovalModeSnapshot,
    ) -> AiChat2Result<McpServerRuntimeConfig> {
        let default_approval_mode = self
            .default_tools_approval_mode
            .map(McpToolApprovalMode::to_snapshot)
            .unwrap_or(inherited_default_approval_mode);
        Ok(McpServerRuntimeConfig {
            server: self.to_agent_config(server_id)?,
            required: self.required,
            startup_timeout: Duration::from_millis(
                self.startup_timeout_ms
                    .unwrap_or(DEFAULT_MCP_STARTUP_TIMEOUT_MS),
            ),
            tool_timeout: Duration::from_millis(
                self.tool_timeout_ms.unwrap_or(DEFAULT_MCP_TOOL_TIMEOUT_MS),
            ),
            enabled_tools: self
                .enabled_tools
                .as_ref()
                .map(|tools| tools.iter().cloned().collect::<BTreeSet<_>>()),
            disabled_tools: self.disabled_tools.iter().cloned().collect(),
            default_approval_mode: default_approval_mode.clone(),
            default_approval_policy: approval_policy_for_mcp_mode(default_approval_mode.clone()),
            execution_policy: ToolExecutionPolicy::Foreground,
            tool_approval_overrides: self
                .tools
                .iter()
                .filter_map(|(tool_name, tool)| {
                    tool.approval_mode
                        .map(McpToolApprovalMode::to_snapshot)
                        .map(|approval_mode| (tool_name.clone(), approval_mode))
                })
                .collect(),
        })
    }

    pub(crate) fn to_runtime_config_snapshot(
        &self,
        server_id: &str,
    ) -> AiChat2Result<McpServerRuntimeConfigSnapshot> {
        self.validate(server_id)?;
        let transport = match self.transport {
            McpTransportKind::Stdio => {
                let command = self.command.clone().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing command"))
                })?;
                McpServerTransportSnapshot::Stdio {
                    command,
                    args: self.args.clone(),
                    cwd: self
                        .cwd
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string()),
                    env: self.env.clone(),
                    env_vars: self.env_vars.clone(),
                }
            }
            McpTransportKind::StreamableHttp => {
                let url = self.url.clone().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing url"))
                })?;
                McpServerTransportSnapshot::StreamableHttp {
                    url,
                    headers: self.headers.clone(),
                    env_headers: self.env_headers.clone(),
                    bearer_token_env_var: self.bearer_token_env_var.clone(),
                    oauth: self.oauth.as_ref().map(McpOAuthTomlConfig::to_snapshot),
                }
            }
        };

        Ok(McpServerRuntimeConfigSnapshot {
            server_id: server_id.to_string(),
            display_name: self.display_name.clone(),
            enabled: self.enabled,
            required: self.required,
            transport,
            startup_timeout_ms: self
                .startup_timeout_ms
                .unwrap_or(DEFAULT_MCP_STARTUP_TIMEOUT_MS),
            tool_timeout_ms: self.tool_timeout_ms.unwrap_or(DEFAULT_MCP_TOOL_TIMEOUT_MS),
            enabled_tools: self.enabled_tools.clone(),
            disabled_tools: self.disabled_tools.clone(),
            default_tools_approval_mode: self
                .default_tools_approval_mode
                .map(McpToolApprovalMode::to_snapshot),
            tools: self
                .tools
                .iter()
                .map(|(tool_name, tool)| (tool_name.clone(), tool.to_snapshot()))
                .collect(),
        })
    }

    pub(crate) fn validate(&self, server_id: &str) -> AiChat2Result<()> {
        validate_server_id(server_id)?;
        validate_timeout(server_id, "startup_timeout_ms", self.startup_timeout_ms)?;
        validate_timeout(server_id, "tool_timeout_ms", self.tool_timeout_ms)?;
        validate_tool_filter(server_id, "enabled_tools", self.enabled_tools.as_deref())?;
        validate_tool_filter(server_id, "disabled_tools", Some(&self.disabled_tools))?;
        for tool_name in self.tools.keys() {
            validate_raw_tool_name(server_id, "tools", tool_name)?;
        }

        match self.transport {
            McpTransportKind::Stdio => {
                if self.command.as_deref().is_none_or(str::is_empty) {
                    return Err(AiChat2Error::Config(format!(
                        "mcp server `{server_id}` is missing command"
                    )));
                }
                for env_var in &self.env_vars {
                    validate_env_var_name(server_id, env_var)?;
                }
            }
            McpTransportKind::StreamableHttp => {
                let url = self.url.as_deref().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing url"))
                })?;
                validate_http_url(server_id, url)?;
                for (header_name, header_value) in &self.headers {
                    validate_http_header(
                        server_id,
                        header_name,
                        header_value,
                        self.oauth.is_some(),
                    )?;
                }
                for (header_name, env_var) in &self.env_headers {
                    validate_http_header_name(server_id, header_name, self.oauth.is_some())?;
                    validate_env_var_name(server_id, env_var)?;
                }
                if let Some(env_var) = self.bearer_token_env_var.as_deref() {
                    validate_env_var_name(server_id, env_var)?;
                    ensure_authorization_header_available(
                        server_id,
                        &self.headers,
                        &self.env_headers,
                    )?;
                }
                if self.oauth.is_some() {
                    ensure_authorization_header_available(
                        server_id,
                        &self.headers,
                        &self.env_headers,
                    )?;
                }
                if let Some(oauth) = &self.oauth {
                    oauth.validate(server_id)?;
                }
            }
        }

        Ok(())
    }

    fn resolved_env(&self, server_id: &str) -> AiChat2Result<BTreeMap<String, String>> {
        let mut env = self.env.clone();
        for env_var in &self.env_vars {
            validate_env_var_name(server_id, env_var)?;
            let value = std::env::var(env_var).map_err(|_| {
                AiChat2Error::Config(format!(
                    "mcp server `{server_id}` references missing environment variable `{env_var}`"
                ))
            })?;
            env.insert(env_var.clone(), value);
        }
        Ok(env)
    }

    fn resolved_headers(&self, server_id: &str) -> AiChat2Result<BTreeMap<String, String>> {
        let mut headers = self.headers.clone();
        for (header_name, env_var) in &self.env_headers {
            let value = env::var(env_var).map_err(|_| {
                AiChat2Error::Config(format!(
                    "mcp server `{server_id}` header `{header_name}` references missing environment variable `{env_var}`"
                ))
            })?;
            headers.insert(header_name.clone(), value);
        }
        if let Some(env_var) = self.bearer_token_env_var.as_deref() {
            let value = env::var(env_var).map_err(|_| {
                AiChat2Error::Config(format!(
                    "mcp server `{server_id}` bearer token references missing environment variable `{env_var}`"
                ))
            })?;
            headers.insert("Authorization".to_string(), format!("Bearer {value}"));
        }
        Ok(headers)
    }
}

impl McpOAuthTomlConfig {
    fn validate(&self, server_id: &str) -> AiChat2Result<()> {
        match self {
            Self::AuthorizationCodePkce { callback_url, .. } => {
                if let Some(url) = callback_url.as_deref() {
                    validate_http_url(server_id, url)?;
                }
            }
            Self::ClientCredentials {
                client_id,
                client_secret_env_var,
                ..
            } => {
                if client_id.trim().is_empty() {
                    return Err(AiChat2Error::Config(format!(
                        "mcp server `{server_id}` OAuth client_id is required"
                    )));
                }
                validate_env_var_name(server_id, client_secret_env_var)?;
            }
        }
        Ok(())
    }

    fn to_snapshot(&self) -> McpOAuthConfigSnapshot {
        match self {
            Self::AuthorizationCodePkce {
                scopes,
                client_id,
                client_metadata_url,
                resource,
                callback_port,
                callback_url,
            } => McpOAuthConfigSnapshot::AuthorizationCodePkce {
                scopes: scopes.clone(),
                client_id: client_id.clone(),
                client_metadata_url: client_metadata_url.clone(),
                resource: resource.clone(),
                callback_port: *callback_port,
                callback_url: callback_url.clone(),
            },
            Self::ClientCredentials {
                client_id,
                client_secret_env_var,
                scopes,
                resource,
            } => McpOAuthConfigSnapshot::ClientCredentials {
                client_id: client_id.clone(),
                client_secret_env_var: client_secret_env_var.clone(),
                scopes: scopes.clone(),
                resource: resource.clone(),
            },
        }
    }
}

impl McpToolApprovalMode {
    pub(crate) fn to_snapshot(self) -> McpToolApprovalModeSnapshot {
        match self {
            Self::Auto => McpToolApprovalModeSnapshot::Auto,
            Self::Prompt => McpToolApprovalModeSnapshot::Prompt,
            Self::Deny => McpToolApprovalModeSnapshot::Deny,
        }
    }
}

impl McpToolOverrideTomlConfig {
    fn to_snapshot(&self) -> McpToolOverrideSnapshot {
        McpToolOverrideSnapshot {
            approval_mode: self.approval_mode.map(McpToolApprovalMode::to_snapshot),
        }
    }
}

pub(crate) fn upsert_mcp_server(
    cx: &mut App,
    original_server_id: Option<&str>,
    server_id: String,
    server: McpServerTomlConfig,
) -> AiChat2Result<()> {
    server.validate(&server_id)?;
    let duplicate = read(cx, |config| {
        config.mcp_servers.contains_key(&server_id)
            && original_server_id.is_none_or(|original_server_id| original_server_id != server_id)
    });
    if duplicate {
        return Err(AiChat2Error::Config(format!(
            "mcp server `{server_id}` already exists"
        )));
    }
    let config_store = store(cx);
    config_store.try_update_field(
        cx,
        |config| &mut config.mcp_servers,
        |servers| {
            if let Some(original_server_id) = original_server_id
                && original_server_id != server_id
            {
                servers.remove(original_server_id);
            }
            servers.insert(server_id, server);
        },
    )?;
    Ok(())
}

pub(crate) fn delete_mcp_server(cx: &mut App, server_id: &str) -> AiChat2Result<bool> {
    let config_store = store(cx);
    let mut removed = false;
    config_store.try_update_field(
        cx,
        |config| &mut config.mcp_servers,
        |servers| {
            removed = servers.remove(server_id).is_some();
        },
    )?;
    Ok(removed)
}

pub(crate) fn set_mcp_server_enabled(
    cx: &mut App,
    server_id: &str,
    enabled: bool,
) -> AiChat2Result<()> {
    let exists = read(cx, |config| config.mcp_servers.contains_key(server_id));
    if !exists {
        return Err(AiChat2Error::Config(format!(
            "mcp server `{server_id}` was not found"
        )));
    }
    let config_store = store(cx);
    config_store.try_update_field(
        cx,
        |config| &mut config.mcp_servers,
        |servers| {
            if let Some(server) = servers.get_mut(server_id) {
                server.enabled = enabled;
            }
        },
    )?;
    Ok(())
}

fn validate_server_id(server_id: &str) -> AiChat2Result<()> {
    if is_valid_mcp_server_id(server_id) {
        return Ok(());
    }
    Err(AiChat2Error::Config(format!(
        "mcp server id `{server_id}` must match ^[A-Za-z0-9_-]+$"
    )))
}

pub(crate) fn is_valid_mcp_server_id(server_id: &str) -> bool {
    !server_id.is_empty()
        && server_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn validate_timeout(server_id: &str, field: &str, value: Option<u64>) -> AiChat2Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if (1..=MAX_MCP_TIMEOUT_MS).contains(&value) {
        return Ok(());
    }
    Err(AiChat2Error::Config(format!(
        "mcp server `{server_id}` {field} must be between 1 and {MAX_MCP_TIMEOUT_MS}"
    )))
}

fn validate_tool_filter(
    server_id: &str,
    field: &str,
    tools: Option<&[String]>,
) -> AiChat2Result<()> {
    for tool_name in tools.into_iter().flatten() {
        validate_raw_tool_name(server_id, field, tool_name)?;
    }
    Ok(())
}

fn validate_raw_tool_name(server_id: &str, field: &str, tool_name: &str) -> AiChat2Result<()> {
    if !tool_name.trim().is_empty() {
        return Ok(());
    }
    Err(AiChat2Error::Config(format!(
        "mcp server `{server_id}` {field} contains an empty tool name"
    )))
}

fn validate_env_var_name(server_id: &str, env_var: &str) -> AiChat2Result<()> {
    if is_valid_mcp_env_var_name(env_var) {
        return Ok(());
    }
    Err(AiChat2Error::Config(format!(
        "mcp server `{server_id}` references invalid environment variable `{env_var}`"
    )))
}

pub(crate) fn is_valid_mcp_env_var_name(env_var: &str) -> bool {
    let mut bytes = env_var.bytes();
    bytes.next().is_some_and(|first| {
        (first.is_ascii_alphabetic() || first == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

fn validate_http_url(server_id: &str, value: &str) -> AiChat2Result<()> {
    let url = url::Url::parse(value).map_err(|err| {
        AiChat2Error::Config(format!(
            "mcp server `{server_id}` has invalid URL `{value}`: {err}"
        ))
    })?;
    if matches!(url.scheme(), "http" | "https") {
        return Ok(());
    }
    Err(AiChat2Error::Config(format!(
        "mcp server `{server_id}` URL `{value}` must use http or https"
    )))
}

fn validate_http_header(
    server_id: &str,
    name: &str,
    value: &str,
    oauth_configured: bool,
) -> AiChat2Result<()> {
    validate_http_header_name(server_id, name, oauth_configured)?;
    http::HeaderValue::from_str(value).map_err(|err| {
        AiChat2Error::Config(format!(
            "mcp server `{server_id}` header `{name}` has invalid value: {err}"
        ))
    })?;
    Ok(())
}

fn validate_http_header_name(
    server_id: &str,
    name: &str,
    oauth_configured: bool,
) -> AiChat2Result<()> {
    let header = http::HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
        AiChat2Error::Config(format!(
            "mcp server `{server_id}` header `{name}` has invalid name: {err}"
        ))
    })?;
    let normalized = header.as_str();
    if is_reserved_mcp_header(normalized) {
        return Err(AiChat2Error::Config(format!(
            "mcp server `{server_id}` header `{name}` is reserved by MCP"
        )));
    }
    if oauth_configured && normalized.eq_ignore_ascii_case("authorization") {
        return Err(AiChat2Error::Config(format!(
            "mcp server `{server_id}` cannot define Authorization header when OAuth is configured"
        )));
    }
    Ok(())
}

pub(crate) fn is_reserved_mcp_header(name: &str) -> bool {
    [
        "accept",
        "content-type",
        "mcp-session-id",
        "mcp-protocol-version",
        "last-event-id",
    ]
    .into_iter()
    .any(|reserved| name.eq_ignore_ascii_case(reserved))
}

fn ensure_authorization_header_available(
    server_id: &str,
    headers: &BTreeMap<String, String>,
    env_headers: &BTreeMap<String, String>,
) -> AiChat2Result<()> {
    let has_authorization = headers
        .keys()
        .chain(env_headers.keys())
        .any(|name| name.eq_ignore_ascii_case("authorization"));
    if has_authorization {
        return Err(AiChat2Error::Config(format!(
            "mcp server `{server_id}` has multiple Authorization sources"
        )));
    }
    Ok(())
}

fn approval_policy_for_mcp_mode(mode: McpToolApprovalModeSnapshot) -> ToolApprovalPolicy {
    match mode {
        McpToolApprovalModeSnapshot::Auto => ToolApprovalPolicy::Never,
        McpToolApprovalModeSnapshot::Prompt | McpToolApprovalModeSnapshot::Deny => {
            ToolApprovalPolicy::OnRequest
        }
    }
}

fn default_mcp_server_enabled() -> bool {
    true
}
