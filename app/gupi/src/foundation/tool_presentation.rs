//! Shared tool semantics for the conversation and history views.
use super::assets::IconName;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolKind {
    Read,
    Skill,
    Write,
    Edit,
    Shell,
    Search,
    List,
    Other,
}
impl ToolKind {
    pub fn classify(name: &str, args: Option<&Value>) -> Self {
        match name {
            "read" if args.and_then(skill_name).is_some() => Self::Skill,
            "read" => Self::Read,
            "write" => Self::Write,
            "edit" => Self::Edit,
            "bash" | "powershell" => Self::Shell,
            "grep" | "find" => Self::Search,
            "ls" => Self::List,
            _ => Self::Other,
        }
    }
    pub fn icon(self) -> IconName {
        match self {
            Self::Read => IconName::FileText,
            Self::Skill => IconName::BookOpen,
            Self::Write => IconName::FilePlus,
            Self::Edit => IconName::FilePenLine,
            Self::Shell => IconName::Terminal,
            Self::Search => IconName::Search,
            Self::List => IconName::Folder,
            Self::Other => IconName::Wrench,
        }
    }
    pub fn action_key(self) -> &'static str {
        match self {
            Self::Read => "conversation-tool-action-read",
            Self::Skill => "conversation-tool-action-skill",
            Self::Write => "conversation-tool-action-write",
            Self::Edit => "conversation-tool-action-edit",
            Self::Shell => "conversation-tool-action-bash",
            Self::Search | Self::List => "conversation-tool-action-search",
            Self::Other => "conversation-tool-action-other",
        }
    }
}

pub(crate) fn read_path(args: &Value) -> Option<&str> {
    args["path"].as_str().or_else(|| args["file_path"].as_str())
}

pub(crate) fn skill_name(args: &Value) -> Option<&str> {
    // Match Pi TUI's file-name rule, including Windows paths on any host.
    let mut parts = read_path(args)?
        .rsplit(['/', '\\'])
        .filter(|part| !part.is_empty());
    (parts.next()? == "SKILL.md").then(|| parts.next().unwrap_or("SKILL.md"))
}
