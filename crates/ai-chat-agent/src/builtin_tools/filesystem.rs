use crate::{
    LocalTool, Result, ToolDefinition, ToolExecutor, ToolRunPolicy,
    builtin_tools::{
        approval::{normalize_for_access, path_access_request, resolve_tool_path},
        types::{
            BuiltinToolContext, BuiltinToolName, DirectoryEntryOutput, EditFileInput,
            EditFileOutput, FileEntryKind, ListDirectoryInput, ListDirectoryOutput, ReadFileInput,
            ReadFileOutput, WriteFileInput, WriteFileOutput, edit_file_schema,
            list_directory_schema, output_with_structured, read_file_schema, write_file_schema,
        },
    },
};
use ai_chat_core::{ToolAccessKind, ToolExecutionPolicy, ToolInvocationOutput, ToolSource};
use async_trait::async_trait;
use ignore::WalkBuilder;
use similar::TextDiff;
use std::{
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_MAX_LINES: u32 = 400;
const DEFAULT_MAX_ENTRIES: usize = 200;

#[derive(Clone, Debug)]
pub struct ReadFileTool {
    context: BuiltinToolContext,
}

#[derive(Clone, Debug)]
pub struct ListDirectoryTool {
    context: BuiltinToolContext,
}

#[derive(Clone, Debug)]
pub struct WriteFileTool {
    context: BuiltinToolContext,
}

#[derive(Clone, Debug)]
pub struct EditFileTool {
    context: BuiltinToolContext,
}

impl ReadFileTool {
    pub fn new(context: BuiltinToolContext) -> Self {
        Self { context }
    }
}

impl ListDirectoryTool {
    pub fn new(context: BuiltinToolContext) -> Self {
        Self { context }
    }
}

impl WriteFileTool {
    pub fn new(context: BuiltinToolContext) -> Self {
        Self { context }
    }
}

impl EditFileTool {
    pub fn new(context: BuiltinToolContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl ToolExecutor for ReadFileTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let input: ReadFileInput = serde_json::from_value(arguments)?;
        let path = normalized_path(&input.path, &self.context)?;
        let content = fs::read_to_string(&path)?;
        let output = read_file_output(&path, content, input.start_line, input.max_lines);
        output_with_structured(
            format!(
                "Read {} lines from {}",
                output.end_line.saturating_sub(output.start_line) + 1,
                output.path
            ),
            output,
        )
    }
}

impl LocalTool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: BuiltinToolName::ReadFile.as_str().to_string(),
            description: "Read a UTF-8 text file, optionally limited by line range.".to_string(),
            parameters: read_file_schema(),
            policy: local_tool_policy(),
        }
    }
}

#[async_trait]
impl ToolExecutor for ListDirectoryTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let input: ListDirectoryInput = serde_json::from_value(arguments)?;
        let path = normalized_path(&input.path, &self.context)?;
        let max_entries = input.max_entries.unwrap_or(DEFAULT_MAX_ENTRIES);
        let (entries, truncated) = if input.recursive {
            recursive_entries(&path, input.include_hidden, max_entries)?
        } else {
            direct_entries(&path, input.include_hidden, max_entries)?
        };
        let output = ListDirectoryOutput {
            path: path.to_string_lossy().into_owned(),
            entries,
            truncated,
        };
        output_with_structured(
            format!(
                "Listed {} entries under {}{}",
                output.entries.len(),
                output.path,
                if output.truncated { " (truncated)" } else { "" }
            ),
            output,
        )
    }
}

impl LocalTool for ListDirectoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: BuiltinToolName::ListDirectory.as_str().to_string(),
            description: "List files and directories under a path.".to_string(),
            parameters: list_directory_schema(),
            policy: local_tool_policy(),
        }
    }
}

#[async_trait]
impl ToolExecutor for WriteFileTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let input: WriteFileInput = serde_json::from_value(arguments)?;
        let path = writable_path(&input.path, &self.context)?;
        let existed = path.exists();
        if existed && !input.overwrite {
            return Ok(error_output(format!(
                "Refusing to overwrite existing file {} without overwrite=true",
                path.display()
            )));
        }
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            if input.create_parent_dirs {
                fs::create_dir_all(parent)?;
            } else {
                return Ok(error_output(format!(
                    "Parent directory does not exist: {}",
                    parent.display()
                )));
            }
        }

        let old_content = if existed {
            Some(fs::read_to_string(&path)?)
        } else {
            None
        };
        fs::write(&path, input.content.as_bytes())?;
        let diff = old_content
            .as_ref()
            .map(|old| unified_diff(old, &input.content, &path));
        let output = WriteFileOutput {
            path: path.to_string_lossy().into_owned(),
            created: !existed,
            bytes_written: input.content.len() as u64,
            diff,
        };
        output_with_structured(
            format!(
                "{} {} ({} bytes)",
                if output.created { "Created" } else { "Wrote" },
                output.path,
                output.bytes_written
            ),
            output,
        )
    }
}

impl LocalTool for WriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: BuiltinToolName::WriteFile.as_str().to_string(),
            description: "Create or overwrite a UTF-8 text file.".to_string(),
            parameters: write_file_schema(),
            policy: local_tool_policy(),
        }
    }
}

#[async_trait]
impl ToolExecutor for EditFileTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        let input: EditFileInput = serde_json::from_value(arguments)?;
        let path = normalized_path(&input.path, &self.context)?;
        let old_content = fs::read_to_string(&path)?;
        let count = old_content.matches(&input.old_text).count() as u32;
        if count == 0 {
            return Ok(error_output(format!(
                "Could not find requested text in {}",
                path.display()
            )));
        }
        if let Some(expected) = input.expected_replacements
            && expected != count
        {
            return Ok(error_output(format!(
                "Expected {expected} replacements in {}, found {count}",
                path.display()
            )));
        }
        let (new_content, replacements) = if input.replace_all {
            (old_content.replace(&input.old_text, &input.new_text), count)
        } else {
            (old_content.replacen(&input.old_text, &input.new_text, 1), 1)
        };
        let diff = unified_diff(&old_content, &new_content, &path);
        fs::write(&path, new_content.as_bytes())?;
        let output = EditFileOutput {
            path: path.to_string_lossy().into_owned(),
            replacements,
            diff,
        };
        output_with_structured(
            format!(
                "Edited {} ({} replacement(s))",
                output.path, output.replacements
            ),
            output,
        )
    }
}

impl LocalTool for EditFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: BuiltinToolName::EditFile.as_str().to_string(),
            description: "Edit a UTF-8 text file by replacing exact text.".to_string(),
            parameters: edit_file_schema(),
            policy: local_tool_policy(),
        }
    }
}

pub fn access_requests(
    tool: BuiltinToolName,
    arguments: &serde_json::Value,
    context: &BuiltinToolContext,
) -> Result<Vec<crate::builtin_tools::types::PathAccessRequest>> {
    match tool {
        BuiltinToolName::ReadFile => {
            let input: ReadFileInput = serde_json::from_value(arguments.clone())?;
            Ok(vec![path_access_request(
                ToolAccessKind::Read,
                input.path,
                context.project_root.as_deref(),
                Some("read_file"),
            )?])
        }
        BuiltinToolName::ListDirectory => {
            let input: ListDirectoryInput = serde_json::from_value(arguments.clone())?;
            Ok(vec![path_access_request(
                ToolAccessKind::Read,
                input.path,
                context.project_root.as_deref(),
                Some("list_directory"),
            )?])
        }
        BuiltinToolName::WriteFile => {
            let input: WriteFileInput = serde_json::from_value(arguments.clone())?;
            Ok(vec![path_access_request(
                ToolAccessKind::Write,
                input.path,
                context.project_root.as_deref(),
                Some("write_file"),
            )?])
        }
        BuiltinToolName::EditFile => {
            let input: EditFileInput = serde_json::from_value(arguments.clone())?;
            Ok(vec![path_access_request(
                ToolAccessKind::Write,
                input.path,
                context.project_root.as_deref(),
                Some("edit_file"),
            )?])
        }
        BuiltinToolName::FindPath | BuiltinToolName::Grep => Ok(Vec::new()),
    }
}

fn local_tool_policy() -> ToolRunPolicy {
    ToolRunPolicy {
        approval_policy: ai_chat_core::ToolApprovalPolicy::Never,
        execution_policy: ToolExecutionPolicy::Foreground,
        timeout_ms: None,
    }
}

fn normalized_path(path: &str, context: &BuiltinToolContext) -> Result<PathBuf> {
    let resolved = resolve_tool_path(path, context.project_root.as_deref())?;
    normalize_for_access(&resolved)
}

fn writable_path(path: &str, context: &BuiltinToolContext) -> Result<PathBuf> {
    let resolved = resolve_tool_path(path, context.project_root.as_deref())?;
    normalize_for_access(&resolved)
}

fn read_file_output(
    path: &Path,
    content: String,
    start_line: Option<u32>,
    max_lines: Option<u32>,
) -> ReadFileOutput {
    let lines = content.lines().collect::<Vec<_>>();
    let total_lines = lines.len() as u32;
    let start = start_line.unwrap_or(1).max(1);
    let max = max_lines.unwrap_or(DEFAULT_MAX_LINES).max(1);
    let start_index = start.saturating_sub(1) as usize;
    let end_index = (start_index + max as usize).min(lines.len());
    let selected = if start_index < lines.len() {
        lines[start_index..end_index].join("\n")
    } else {
        String::new()
    };
    ReadFileOutput {
        path: path.to_string_lossy().into_owned(),
        content: selected,
        start_line: start,
        end_line: end_index as u32,
        total_lines,
        truncated: end_index < lines.len(),
    }
}

fn direct_entries(
    path: &Path,
    include_hidden: bool,
    max_entries: usize,
) -> Result<(Vec<DirectoryEntryOutput>, bool)> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if !include_hidden && is_hidden(&path) {
            continue;
        }
        if entries.len() >= max_entries {
            return Ok((entries, true));
        }
        entries.push(directory_entry_output(&path)?);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((entries, false))
}

fn recursive_entries(
    path: &Path,
    include_hidden: bool,
    max_entries: usize,
) -> Result<(Vec<DirectoryEntryOutput>, bool)> {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(!include_hidden)
        .git_ignore(true)
        .parents(true);
    let mut entries = Vec::new();
    for result in builder.build().skip(1) {
        let entry = result.map_err(|err| std::io::Error::other(err.to_string()))?;
        if entries.len() >= max_entries {
            return Ok((entries, true));
        }
        entries.push(directory_entry_output(entry.path())?);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((entries, false))
}

fn directory_entry_output(path: &Path) -> Result<DirectoryEntryOutput> {
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        FileEntryKind::Symlink
    } else if file_type.is_dir() {
        FileEntryKind::Directory
    } else if file_type.is_file() {
        FileEntryKind::File
    } else {
        FileEntryKind::Other
    };
    Ok(DirectoryEntryOutput {
        path: path.to_string_lossy().into_owned(),
        kind,
        size_bytes: file_type.is_file().then_some(metadata.len()),
    })
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

fn unified_diff(old: &str, new: &str, path: &Path) -> String {
    TextDiff::from_lines(old, new)
        .unified_diff()
        .header(
            &format!("a/{}", path.to_string_lossy()),
            &format!("b/{}", path.to_string_lossy()),
        )
        .to_string()
}

fn error_output(message: String) -> ToolInvocationOutput {
    ToolInvocationOutput {
        content: vec![ai_chat_core::ContentPart::Text { text: message }],
        structured_output: None,
        raw_output: None,
        is_error: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolExecutor;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn write_read_and_edit_file_roundtrip() {
        let dir = tempdir().unwrap();
        let context = BuiltinToolContext {
            project_root: Some(dir.path().to_path_buf()),
        };
        let write = WriteFileTool::new(context.clone())
            .execute(json!({
                "path": "notes.txt",
                "content": "alpha\nbeta\n",
            }))
            .await
            .unwrap();
        assert!(!write.is_error);
        let write_output: WriteFileOutput =
            serde_json::from_value(write.structured_output.unwrap().value).unwrap();
        assert!(write_output.created);

        let read = ReadFileTool::new(context.clone())
            .execute(json!({
                "path": "notes.txt",
                "startLine": 2,
                "maxLines": 1,
            }))
            .await
            .unwrap();
        let read_output: ReadFileOutput =
            serde_json::from_value(read.structured_output.unwrap().value).unwrap();
        assert_eq!(read_output.content, "beta");

        let edit = EditFileTool::new(context)
            .execute(json!({
                "path": "notes.txt",
                "oldText": "beta",
                "newText": "gamma",
            }))
            .await
            .unwrap();
        assert!(!edit.is_error);
        let edit_output: EditFileOutput =
            serde_json::from_value(edit.structured_output.unwrap().value).unwrap();
        assert_eq!(edit_output.replacements, 1);
        assert!(edit_output.diff.contains("gamma"));
    }

    #[tokio::test]
    async fn v1_tool_inputs_reject_unknown_fields() {
        let context = BuiltinToolContext {
            project_root: Some(tempdir().unwrap().path().to_path_buf()),
        };
        let err = ReadFileTool::new(context)
            .execute(json!({
                "path": "notes.txt",
                "unexpected": true,
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }
}
