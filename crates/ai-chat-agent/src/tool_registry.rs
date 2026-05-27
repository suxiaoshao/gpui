use crate::{AgentRuntimeError, Result};
use ai_chat_core::*;
use async_trait::async_trait;
use rig_core::{
    completion::ToolDefinition as RigToolDefinition,
    tool::{ToolDyn, ToolError},
    wasm_compat::WasmBoxedFuture,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tokio::time::timeout;

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
struct ToolEntry {
    definition: ToolDefinition,
    runtime_tool_name: String,
    executor: Arc<dyn ToolExecutor>,
}

#[derive(Clone)]
pub(crate) struct RegisteredRuntimeTool {
    pub definition: RegisteredToolDefinition,
    pub executor: Arc<dyn ToolExecutor>,
    pub timeout: std::time::Duration,
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

    pub fn register_mcp_tool<T>(&mut self, definition: ToolDefinition, tool: T) -> Result<()>
    where
        T: ToolDyn + 'static,
    {
        self.register_tool_definition(definition, Arc::new(RigToolExecutor::new(tool)))
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
            executor,
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

    pub(crate) fn runtime_tools(
        &self,
        default_timeout: std::time::Duration,
    ) -> Vec<RegisteredRuntimeTool> {
        let mut registry = self.clone();
        if !registry.finalized {
            registry.finalize_names();
        }
        registry
            .entries
            .into_iter()
            .map(|entry| {
                let timeout = entry
                    .definition
                    .policy
                    .timeout_ms
                    .map(std::time::Duration::from_millis)
                    .unwrap_or(default_timeout);
                RegisteredRuntimeTool {
                    definition: RegisteredToolDefinition {
                        source: entry.definition.source,
                        namespace: entry.definition.namespace,
                        tool_name: entry.definition.name,
                        runtime_tool_name: entry.runtime_tool_name,
                        description: entry.definition.description,
                        parameters: entry.definition.parameters,
                        policy: entry.definition.policy,
                    },
                    executor: entry.executor,
                    timeout,
                }
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

    pub fn into_rig_tools(mut self, default_timeout: std::time::Duration) -> Vec<Box<dyn ToolDyn>> {
        if !self.finalized {
            self.finalize_names();
        }
        self.entries
            .into_iter()
            .map(|entry| {
                Box::new(RegisteredRigTool {
                    runtime_tool_name: entry.runtime_tool_name,
                    description: entry.definition.description,
                    parameters: entry.definition.parameters,
                    executor: entry.executor,
                    timeout: entry
                        .definition
                        .policy
                        .timeout_ms
                        .map(std::time::Duration::from_millis)
                        .unwrap_or(default_timeout),
                }) as Box<dyn ToolDyn>
            })
            .collect()
    }
}

struct RigToolExecutor {
    tool: Arc<dyn ToolDyn>,
}

impl RigToolExecutor {
    fn new(tool: impl ToolDyn + 'static) -> Self {
        Self {
            tool: Arc::new(tool),
        }
    }
}

#[async_trait]
impl ToolExecutor for RigToolExecutor {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let args = serde_json::to_string(&arguments)?;
        let output = self
            .tool
            .call(args)
            .await
            .map_err(|err| AgentRuntimeError::Mcp(err.to_string()))?;
        Ok(ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: output.clone(),
            }],
            structured_output: serde_json::from_str::<serde_json::Value>(&output)
                .ok()
                .map(|value| StructuredOutput { value }),
            raw_output: None,
            is_error: false,
        })
    }
}

struct RegisteredRigTool {
    runtime_tool_name: String,
    description: String,
    parameters: serde_json::Value,
    executor: Arc<dyn ToolExecutor>,
    timeout: std::time::Duration,
}

impl ToolDyn for RegisteredRigTool {
    fn name(&self) -> String {
        self.runtime_tool_name.clone()
    }

    fn definition(&self, _prompt: String) -> WasmBoxedFuture<'_, RigToolDefinition> {
        Box::pin(async move {
            RigToolDefinition {
                name: self.runtime_tool_name.clone(),
                description: self.description.clone(),
                parameters: self.parameters.clone(),
            }
        })
    }

    fn call(&self, args: String) -> WasmBoxedFuture<'_, std::result::Result<String, ToolError>> {
        Box::pin(async move {
            let arguments =
                serde_json::from_str::<serde_json::Value>(&args).map_err(ToolError::JsonError)?;
            let output = timeout(self.timeout, self.executor.execute(arguments))
                .await
                .map_err(|_| tool_call_error("tool execution timed out"))?
                .map_err(|err| tool_call_error(err.to_string()))?;
            Ok(tool_output_to_model_text(&output))
        })
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

fn tool_call_error(message: impl Into<String>) -> ToolError {
    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    struct RuntimeToolError(String);

    ToolError::ToolCallError(Box::new(RuntimeToolError(message.into())))
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
