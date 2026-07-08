use sha2::{Digest, Sha256};

use std::collections::{BTreeMap, BTreeSet};

use jaco_core::{McpToolApprovalModeSnapshot, ToolApprovalPolicy, ToolExecutionPolicy};

use super::{McpServerConfig, McpServerRuntimeConfig, McpServerTransport};

#[derive(serde::Serialize)]
struct McpServerRuntimeFingerprint<'a> {
    server: &'a McpServerConfig,
    required: bool,
    startup_timeout_ms: u128,
    tool_timeout_ms: u128,
    enabled_tools: &'a Option<BTreeSet<String>>,
    disabled_tools: &'a BTreeSet<String>,
    default_approval_mode: &'a McpToolApprovalModeSnapshot,
    default_approval_policy: ToolApprovalPolicy,
    execution_policy: ToolExecutionPolicy,
    tool_approval_overrides: &'a BTreeMap<String, McpToolApprovalModeSnapshot>,
    oauth_credentials: Option<&'a serde_json::Value>,
}

pub(crate) fn mcp_server_fingerprint(config: &McpServerRuntimeConfig) -> String {
    let oauth_credentials = match &config.server.transport {
        McpServerTransport::StreamableHttp(http) => http.oauth_credentials.as_ref(),
        McpServerTransport::Stdio(_) => None,
    };
    let fingerprint = McpServerRuntimeFingerprint {
        server: &config.server,
        required: config.required,
        startup_timeout_ms: config.startup_timeout.as_millis(),
        tool_timeout_ms: config.tool_timeout.as_millis(),
        enabled_tools: &config.enabled_tools,
        disabled_tools: &config.disabled_tools,
        default_approval_mode: &config.default_approval_mode,
        default_approval_policy: config.default_approval_policy,
        execution_policy: config.execution_policy,
        tool_approval_overrides: &config.tool_approval_overrides,
        oauth_credentials,
    };
    let bytes = serde_json::to_vec(&fingerprint)
        .expect("MCP runtime fingerprint contains only serializable data");
    format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
}
