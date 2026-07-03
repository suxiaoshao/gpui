use ai_chat_core::{ContentPart, StructuredOutput, ToolAccessKind, ToolInvocationOutput};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinToolName {
    ReadFile,
    ListDirectory,
    FindPath,
    Grep,
    WriteFile,
    EditFile,
}

impl BuiltinToolName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadFile => "read_file",
            Self::ListDirectory => "list_directory",
            Self::FindPath => "find_path",
            Self::Grep => "grep",
            Self::WriteFile => "write_file",
            Self::EditFile => "edit_file",
        }
    }

    pub fn from_tool_name(name: &str) -> Option<Self> {
        match name {
            "read_file" => Some(Self::ReadFile),
            "list_directory" => Some(Self::ListDirectory),
            "find_path" => Some(Self::FindPath),
            "grep" => Some(Self::Grep),
            "write_file" => Some(Self::WriteFile),
            "edit_file" => Some(Self::EditFile),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BuiltinToolContext {
    pub project_root: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathAccessRequest {
    pub kind: ToolAccessKind,
    pub target: String,
    pub normalized_path: PathBuf,
    pub within_project: bool,
    pub reason_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadFileInput {
    pub path: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub max_lines: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileOutput {
    pub path: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub total_lines: u32,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListDirectoryInput {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub max_entries: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryOutput {
    pub path: String,
    pub kind: FileEntryKind,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDirectoryOutput {
    pub path: String,
    pub entries: Vec<DirectoryEntryOutput>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindPathInput {
    pub query: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSearchMatchOutput {
    pub path: String,
    pub kind: FileEntryKind,
    pub score: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindPathOutput {
    pub query: String,
    pub matches: Vec<PathSearchMatchOutput>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrepInput {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub context_lines: Option<u32>,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRangeOutput {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepContextLineOutput {
    pub line_number: u32,
    pub line: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepMatchOutput {
    pub path: String,
    pub line_number: u32,
    pub line: String,
    pub ranges: Vec<TextRangeOutput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<GrepContextLineOutput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<GrepContextLineOutput>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepOutput {
    pub pattern: String,
    pub matches: Vec<GrepMatchOutput>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub create_parent_dirs: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileOutput {
    pub path: String,
    pub created: bool,
    pub bytes_written: u64,
    pub diff: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditFileInput {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    #[serde(default)]
    pub replace_all: bool,
    #[serde(default)]
    pub expected_replacements: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditFileOutput {
    pub path: String,
    pub replacements: u32,
    pub diff: String,
}

pub fn read_file_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "path": { "type": "string" },
            "startLine": { "type": "integer", "minimum": 1 },
            "maxLines": { "type": "integer", "minimum": 1 },
        },
        "required": ["path"],
    })
}

pub fn list_directory_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "path": { "type": "string" },
            "recursive": { "type": "boolean" },
            "includeHidden": { "type": "boolean" },
            "maxEntries": { "type": "integer", "minimum": 1 },
        },
        "required": ["path"],
    })
}

pub fn find_path_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "query": { "type": "string" },
            "path": { "type": "string" },
            "includeHidden": { "type": "boolean" },
            "maxResults": { "type": "integer", "minimum": 1 },
        },
        "required": ["query"],
    })
}

pub fn grep_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "pattern": { "type": "string" },
            "path": { "type": "string" },
            "glob": { "type": "string" },
            "caseSensitive": { "type": "boolean" },
            "contextLines": { "type": "integer", "minimum": 0 },
            "includeHidden": { "type": "boolean" },
            "maxResults": { "type": "integer", "minimum": 1 },
        },
        "required": ["pattern"],
    })
}

pub fn write_file_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "path": { "type": "string" },
            "content": { "type": "string" },
            "overwrite": { "type": "boolean" },
            "createParentDirs": { "type": "boolean" },
        },
        "required": ["path", "content"],
    })
}

pub fn edit_file_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "path": { "type": "string" },
            "oldText": { "type": "string", "minLength": 1 },
            "newText": { "type": "string" },
            "replaceAll": { "type": "boolean" },
            "expectedReplacements": { "type": "integer", "minimum": 1 },
        },
        "required": ["path", "oldText", "newText"],
    })
}

pub fn output_with_structured(
    summary: impl Into<String>,
    structured: impl Serialize,
) -> crate::Result<ToolInvocationOutput> {
    Ok(ToolInvocationOutput {
        content: vec![ContentPart::Text {
            text: summary.into(),
        }],
        structured_output: Some(StructuredOutput {
            value: serde_json::to_value(structured)?,
        }),
        raw_output: None,
        is_error: false,
    })
}
