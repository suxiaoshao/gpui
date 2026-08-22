pub(crate) mod approval;
pub(crate) mod builtin;

use crate::{AgentRuntimeError, Result};
use async_trait::async_trait;
use jaco_core::*;
use rig::{
    agent::{AgentBuilder, WithBuilderTools},
    message::ToolResultContent,
    tool::{DynamicTool, ToolExecutionError, ToolOutput},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolRunPolicy {
    pub approval_policy: ToolApprovalPolicy,
    pub execution_policy: ToolExecutionPolicy,
    pub timeout_ms: Option<u64>,
}

impl Default for ToolRunPolicy {
    fn default() -> Self {
        Self {
            approval_policy: ToolApprovalPolicy::Never,
            execution_policy: ToolExecutionPolicy::Foreground,
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDefinition {
    pub source: ToolSource,
    pub namespace: Option<String>,
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub policy: ToolRunPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisteredToolDefinition {
    pub source: ToolSource,
    pub namespace: Option<String>,
    pub tool_name: String,
    pub runtime_tool_name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub policy: ToolRunPolicy,
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput>;
}

#[async_trait]
pub trait LocalTool: ToolExecutor {
    fn definition(&self) -> ToolDefinition;
}

#[derive(Clone)]
enum ToolEntryRuntime {
    Local(Arc<dyn ToolExecutor>),
    Rmcp {
        tool: Box<rmcp::model::Tool>,
        server: rmcp::service::ServerSink,
    },
}

#[derive(Clone)]
struct ToolEntry {
    definition: ToolDefinition,
    runtime_tool_name: String,
    runtime: ToolEntryRuntime,
}

pub(crate) struct RmcpToolRegistration {
    tool: rmcp::model::Tool,
    server: rmcp::service::ServerSink,
    timeout: std::time::Duration,
}

pub(crate) struct RigToolBundle {
    dynamic_tools: Vec<DynamicTool>,
    rmcp_tools: Vec<RmcpToolRegistration>,
    definitions: Vec<RegisteredToolDefinition>,
}

impl RigToolBundle {
    pub(crate) fn definitions(&self) -> &[RegisteredToolDefinition] {
        &self.definitions
    }

    pub(crate) fn install(self, builder: AgentBuilder) -> AgentBuilder<WithBuilderTools> {
        let mut builder = builder.dynamic_tools(self.dynamic_tools);
        for registration in self.rmcp_tools {
            builder = builder.rmcp_tools_with_timeout(
                vec![registration.tool],
                registration.server,
                registration.timeout,
            );
        }
        builder
    }
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    entries: Vec<ToolEntry>,
    finalized: bool,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_local_tool<T>(&mut self, tool: T) -> Result<()>
    where
        T: LocalTool + 'static,
    {
        let definition = tool.definition();
        self.register_tool_definition(definition, Arc::new(tool))
    }

    pub fn register_mcp_tool(
        &mut self,
        definition: ToolDefinition,
        tool: rmcp::model::Tool,
        server: rmcp::service::ServerSink,
    ) -> Result<()> {
        ensure_tool_name(&definition.name)?;
        self.finalized = false;
        self.entries.push(ToolEntry {
            runtime_tool_name: definition.name.clone(),
            definition,
            runtime: ToolEntryRuntime::Rmcp {
                tool: Box::new(tool),
                server,
            },
        });
        Ok(())
    }

    pub fn register_tool_definition(
        &mut self,
        definition: ToolDefinition,
        executor: Arc<dyn ToolExecutor>,
    ) -> Result<()> {
        ensure_tool_name(&definition.name)?;
        self.finalized = false;
        self.entries.push(ToolEntry {
            runtime_tool_name: definition.name.clone(),
            definition,
            runtime: ToolEntryRuntime::Local(executor),
        });
        Ok(())
    }

    pub fn finalize_names(&mut self) {
        let sanitized_names = self
            .entries
            .iter()
            .map(|entry| sanitize_tool_name(&entry.definition.name))
            .collect::<Vec<_>>();

        let mut name_counts = BTreeMap::<String, usize>::new();
        for name in &sanitized_names {
            *name_counts.entry(name.clone()).or_default() += 1;
        }

        let mut assigned_counts = BTreeMap::<String, usize>::new();
        for (entry, sanitized_name) in self.entries.iter_mut().zip(sanitized_names) {
            let candidate = if name_counts[&sanitized_name] == 1 {
                sanitized_name
            } else {
                let namespace = entry
                    .definition
                    .namespace
                    .clone()
                    .unwrap_or_else(|| tool_source_namespace(&entry.definition.source));
                format!("{}__{}", sanitize_tool_name(&namespace), sanitized_name)
            };
            let assigned_count = assigned_counts.entry(candidate.clone()).or_default();
            *assigned_count += 1;
            entry.runtime_tool_name = if *assigned_count == 1 {
                candidate
            } else {
                format!("{candidate}__{assigned_count}")
            };
        }
        self.finalized = true;
    }

    pub fn registered_definitions(&self) -> Vec<RegisteredToolDefinition> {
        self.entries
            .iter()
            .map(|entry| RegisteredToolDefinition {
                source: entry.definition.source.clone(),
                namespace: entry.definition.namespace.clone(),
                tool_name: entry.definition.name.clone(),
                runtime_tool_name: entry.runtime_tool_name.clone(),
                description: entry.definition.description.clone(),
                parameters: entry.definition.parameters.clone(),
                policy: entry.definition.policy.clone(),
            })
            .collect()
    }

    pub fn lookup(&self, runtime_tool_name: &str) -> Option<RegisteredToolDefinition> {
        self.entries
            .iter()
            .find(|entry| entry.runtime_tool_name == runtime_tool_name)
            .map(|entry| RegisteredToolDefinition {
                source: entry.definition.source.clone(),
                namespace: entry.definition.namespace.clone(),
                tool_name: entry.definition.name.clone(),
                runtime_tool_name: entry.runtime_tool_name.clone(),
                description: entry.definition.description.clone(),
                parameters: entry.definition.parameters.clone(),
                policy: entry.definition.policy.clone(),
            })
    }

    pub(crate) fn into_rig_tool_bundle(
        mut self,
        default_timeout: std::time::Duration,
    ) -> RigToolBundle {
        if !self.finalized {
            self.finalize_names();
        }
        let mut dynamic_tools = Vec::new();
        let mut rmcp_tools = Vec::new();
        let mut definitions = Vec::new();

        for entry in self.entries {
            let timeout = entry
                .definition
                .policy
                .timeout_ms
                .map(std::time::Duration::from_millis)
                .unwrap_or(default_timeout);
            let registered_definition = RegisteredToolDefinition {
                source: entry.definition.source,
                namespace: entry.definition.namespace,
                tool_name: entry.definition.name,
                runtime_tool_name: entry.runtime_tool_name.clone(),
                description: entry.definition.description.clone(),
                parameters: entry.definition.parameters.clone(),
                policy: entry.definition.policy,
            };
            definitions.push(registered_definition);

            match entry.runtime {
                ToolEntryRuntime::Local(executor) => {
                    dynamic_tools.push(DynamicTool::new(
                        entry.runtime_tool_name,
                        entry.definition.description,
                        entry.definition.parameters,
                        move |context, arguments| {
                            let executor = executor.clone();
                            Box::pin(async move {
                                let output =
                                    tokio::time::timeout(timeout, executor.execute(arguments))
                                        .await
                                        .map_err(|_| {
                                            ToolExecutionError::timeout("tool execution timed out")
                                        })?
                                        .map_err(|error| {
                                            ToolExecutionError::other(error.to_string())
                                                .with_source(error)
                                        })?;
                                context.insert_result(output.clone());
                                jaco_output_to_rig_tool_output(&output)
                            })
                        },
                    ));
                }
                ToolEntryRuntime::Rmcp { mut tool, server } => {
                    tool.name = entry.runtime_tool_name.into();
                    rmcp_tools.push(RmcpToolRegistration {
                        tool: *tool,
                        server,
                        timeout,
                    });
                }
            }
        }

        RigToolBundle {
            dynamic_tools,
            rmcp_tools,
            definitions,
        }
    }
}

fn ensure_tool_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(AgentRuntimeError::Invariant(
            "tool name cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn tool_source_namespace(source: &ToolSource) -> String {
    match source {
        ToolSource::Local => "local".to_string(),
        ToolSource::Mcp { server_id } => format!("mcp_{server_id}"),
        ToolSource::ProviderHosted { provider_id } => format!("provider_{provider_id}"),
    }
}

fn sanitize_tool_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "tool".to_string()
    } else {
        out
    }
}

pub(crate) fn tool_output_to_model_text(output: &ToolInvocationOutput) -> String {
    if let Some(structured) = output.structured_output.as_ref() {
        return structured.value.to_string();
    }
    output
        .content
        .iter()
        .filter_map(ContentPart::search_text)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn jaco_output_to_rig_tool_output(
    output: &ToolInvocationOutput,
) -> std::result::Result<ToolOutput, ToolExecutionError> {
    let mut content = output
        .content
        .iter()
        .filter_map(|part| part.search_text().map(ToolResultContent::text))
        .collect::<Vec<_>>();
    if let Some(structured) = output.structured_output.as_ref() {
        content.push(ToolResultContent::json(structured.value.clone()));
    }
    if content.is_empty() {
        return Ok(ToolOutput::text(""));
    }
    ToolOutput::content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct EchoTool {
        source: ToolSource,
        namespace: Option<String>,
        name: String,
    }

    impl EchoTool {
        fn new(name: &str, source: ToolSource, namespace: Option<&str>) -> Self {
            Self {
                source,
                namespace: namespace.map(ToString::to_string),
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl ToolExecutor for EchoTool {
        async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
            Ok(ToolInvocationOutput {
                content: vec![ContentPart::Text {
                    text: arguments.to_string(),
                }],
                structured_output: Some(StructuredOutput { value: arguments }),
                raw_output: None,
                is_error: false,
            })
        }
    }

    #[async_trait]
    impl LocalTool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                source: self.source.clone(),
                namespace: self.namespace.clone(),
                name: self.name.clone(),
                description: "Echo arguments".to_string(),
                parameters: serde_json::json!({"type": "object"}),
                policy: ToolRunPolicy::default(),
            }
        }
    }

    #[test]
    fn duplicate_tool_names_are_namespaced() {
        let mut registry = ToolRegistry::new();
        registry
            .register_local_tool(EchoTool::new(
                "echo",
                ToolSource::Mcp {
                    server_id: "server-a".to_string(),
                },
                Some("server-a"),
            ))
            .unwrap();
        registry
            .register_local_tool(EchoTool::new(
                "echo",
                ToolSource::Mcp {
                    server_id: "server-b".to_string(),
                },
                Some("server-b"),
            ))
            .unwrap();
        registry.finalize_names();
        let names = registry
            .registered_definitions()
            .into_iter()
            .map(|definition| definition.runtime_tool_name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["server_a__echo", "server_b__echo"]);
    }

    #[test]
    fn sanitized_tool_name_collisions_are_namespaced() {
        let mut registry = ToolRegistry::new();
        registry
            .register_local_tool(EchoTool::new(
                "read-file",
                ToolSource::Mcp {
                    server_id: "server-a".to_string(),
                },
                Some("server-a"),
            ))
            .unwrap();
        registry
            .register_local_tool(EchoTool::new(
                "read_file",
                ToolSource::Mcp {
                    server_id: "server-b".to_string(),
                },
                Some("server-b"),
            ))
            .unwrap();

        registry.finalize_names();
        let names = registry
            .registered_definitions()
            .into_iter()
            .map(|definition| definition.runtime_tool_name)
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["server_a__read_file", "server_b__read_file"]);
    }

    #[test]
    fn namespaced_tool_name_collisions_get_stable_suffixes() {
        let mut registry = ToolRegistry::new();
        registry
            .register_local_tool(EchoTool::new(
                "read-file",
                ToolSource::Mcp {
                    server_id: "server-a".to_string(),
                },
                Some("server-a"),
            ))
            .unwrap();
        registry
            .register_local_tool(EchoTool::new(
                "read_file",
                ToolSource::Mcp {
                    server_id: "server_a".to_string(),
                },
                Some("server_a"),
            ))
            .unwrap();

        registry.finalize_names();
        let definitions = registry.registered_definitions();
        let names = definitions
            .iter()
            .map(|definition| definition.runtime_tool_name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["server_a__read_file", "server_a__read_file__2"]);
        assert_eq!(
            registry.lookup("server_a__read_file").unwrap().tool_name,
            "read-file"
        );
        assert_eq!(
            registry.lookup("server_a__read_file__2").unwrap().tool_name,
            "read_file"
        );
    }
}
