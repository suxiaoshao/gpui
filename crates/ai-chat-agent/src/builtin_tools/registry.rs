use crate::{
    Result, ToolRegistry,
    builtin_tools::{
        filesystem::{EditFileTool, ListDirectoryTool, ReadFileTool, WriteFileTool},
        search::{FindPathTool, GrepTool},
        types::{BuiltinToolContext, BuiltinToolName, PathAccessRequest},
    },
};
use ai_chat_core::{ToolPolicySnapshot, ToolSource};
use std::path::{Path, PathBuf};

pub fn register_enabled_builtin_tools(
    registry: &mut ToolRegistry,
    policy: &ToolPolicySnapshot,
    project_root: Option<&Path>,
) -> Result<()> {
    if !policy
        .enabled_sources
        .iter()
        .any(|source| matches!(source, ToolSource::Local))
    {
        return Ok(());
    }

    let context = BuiltinToolContext {
        project_root: project_root.map(Path::to_path_buf),
    };
    registry.register_local_tool(ReadFileTool::new(context.clone()))?;
    registry.register_local_tool(ListDirectoryTool::new(context.clone()))?;
    registry.register_local_tool(FindPathTool::new(context.clone()))?;
    registry.register_local_tool(GrepTool::new(context.clone()))?;
    registry.register_local_tool(WriteFileTool::new(context.clone()))?;
    registry.register_local_tool(EditFileTool::new(context))?;
    Ok(())
}

pub fn access_requests_for_builtin_tool(
    tool_name: &str,
    arguments: &serde_json::Value,
    policy: &ToolPolicySnapshot,
) -> Result<Option<Vec<PathAccessRequest>>> {
    let Some(tool) = BuiltinToolName::from_tool_name(tool_name) else {
        return Ok(None);
    };
    let context = context_from_policy(policy);
    let access_requests = match tool {
        BuiltinToolName::ReadFile
        | BuiltinToolName::ListDirectory
        | BuiltinToolName::WriteFile
        | BuiltinToolName::EditFile => {
            crate::builtin_tools::filesystem::access_requests(tool, arguments, &context)?
        }
        BuiltinToolName::FindPath | BuiltinToolName::Grep => {
            crate::builtin_tools::search::access_requests(tool, arguments, &context)?
        }
    };
    Ok(Some(access_requests))
}

pub fn context_from_policy(policy: &ToolPolicySnapshot) -> BuiltinToolContext {
    BuiltinToolContext {
        project_root: policy
            .permission_scope
            .as_ref()
            .and_then(|scope| scope.project_roots.first())
            .map(PathBuf::from),
    }
}
