use crate::{Result, ToolDefinition, ToolRegistry, ToolRunPolicy};
use ai_chat_core::{ToolApprovalPolicy, ToolExecutionPolicy, ToolSource};
use rig_core::tool::rmcp::McpTool;
use rmcp::{model::Tool as RmcpToolDefinition, service::ServerSink};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpConfigLayer {
    pub servers: Vec<McpServerConfig>,
}

impl McpConfigLayer {
    pub fn merge_ordered(layers: impl IntoIterator<Item = McpConfigLayer>) -> Vec<McpServerConfig> {
        let mut servers = BTreeMap::new();
        for layer in layers {
            for server in layer.servers {
                servers.insert(server.server_id.clone(), server);
            }
        }
        servers.into_values().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerConfig {
    pub server_id: String,
    pub display_name: Option<String>,
    pub transport: McpServerTransport,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum McpServerTransport {
    Stdio(McpStdioTransport),
    StreamableHttp(McpStreamableHttpTransport),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStdioTransport {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpStreamableHttpTransport {
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub oauth: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct McpConnector;

impl McpConnector {
    pub fn new() -> Self {
        Self
    }

    pub fn register_rmcp_tools(
        &self,
        registry: &mut ToolRegistry,
        server_id: impl Into<String>,
        tools: impl IntoIterator<Item = RmcpToolDefinition>,
        client: ServerSink,
        approval_policy: ToolApprovalPolicy,
        execution_policy: ToolExecutionPolicy,
    ) -> Result<()> {
        let server_id = server_id.into();
        for tool in tools {
            let original_name = tool.name.to_string();
            let description = tool
                .description
                .clone()
                .map(|description| description.to_string());
            let parameters = tool.schema_as_json_value();
            let mcp_tool = McpTool::from_mcp_server(tool, client.clone());
            registry.register_mcp_tool(
                ToolDefinition {
                    source: ToolSource::Mcp {
                        server_id: server_id.clone(),
                    },
                    namespace: Some(server_id.clone()),
                    name: original_name,
                    description: description.unwrap_or_default(),
                    parameters,
                    policy: ToolRunPolicy {
                        approval_policy,
                        execution_policy,
                        timeout_ms: None,
                    },
                },
                mcp_tool,
            )?;
        }
        Ok(())
    }
}
