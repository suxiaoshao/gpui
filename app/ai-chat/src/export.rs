use crate::{
    database::{Conversation, Message},
    errors::AiChatResult,
};
use std::path::{Path, PathBuf};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportType {
    Json,
    Csv,
    Txt,
}

impl ExportType {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Txt => "txt",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Csv => "CSV",
            Self::Txt => "TXT",
        }
    }
}

pub(crate) fn suggested_export_file_name(
    conversation: &Conversation,
    export_type: ExportType,
) -> String {
    format!(
        "{}.{}",
        sanitize_file_stem(&conversation.title, conversation.id),
        export_type.extension()
    )
}

pub(crate) fn export_conversation_to_path(
    conversation: &Conversation,
    export_type: ExportType,
    path: &Path,
    sources_label: &str,
) -> AiChatResult<PathBuf> {
    let path = unique_path(path);
    let bytes = match export_type {
        ExportType::Json => serde_json::to_vec_pretty(conversation)?,
        ExportType::Csv => export_csv(conversation)?.into_bytes(),
        ExportType::Txt => export_txt(conversation, sources_label).into_bytes(),
    };
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn export_csv(conversation: &Conversation) -> AiChatResult<String> {
    let mut lines = Vec::with_capacity(conversation.messages.len() + 1);
    lines.push(csv_record(&[
        "id",
        "conversation_id",
        "conversation_path",
        "provider",
        "role",
        "status",
        "content_json",
        "send_content_json",
        "error",
        "created_time",
        "updated_time",
        "start_time",
        "end_time",
    ]));
    for message in &conversation.messages {
        lines.push(csv_record(&message_csv_fields(message)?));
    }
    Ok(lines.join("\n"))
}

fn message_csv_fields(message: &Message) -> AiChatResult<Vec<String>> {
    Ok(vec![
        message.id.to_string(),
        message.conversation_id.to_string(),
        message.conversation_path.clone(),
        message.provider.clone(),
        message.role.to_string(),
        message.status.to_string(),
        serde_json::to_string(&message.content)?,
        serde_json::to_string(&message.send_content)?,
        message.error.clone().unwrap_or_default(),
        format_time(message.created_time),
        format_time(message.updated_time),
        format_time(message.start_time),
        format_time(message.end_time),
    ])
}

fn export_txt(conversation: &Conversation, sources_label: &str) -> String {
    let mut output = String::new();
    output.push_str(&format!("# {}\n\n", conversation.title));
    output.push_str(&format!("ID: {}\n", conversation.id));
    output.push_str(&format!("Path: {}\n", conversation.path));
    if let Some(info) = conversation.info.as_deref().filter(|info| !info.is_empty()) {
        output.push_str(&format!("Info: {info}\n"));
    }
    output.push_str("\n---\n\n");

    for message in &conversation.messages {
        output.push_str(&format!(
            "[{}] {} ({})\n",
            format_time(message.created_time),
            message.role,
            message.provider
        ));
        output.push_str(&format!("Status: {}\n\n", message.status));
        output.push_str(&message.content.display_markdown(sources_label));
        if let Some(error) = message.error.as_deref().filter(|error| !error.is_empty()) {
            output.push_str("\n\nError:\n");
            output.push_str(error);
        }
        output.push_str("\n\n---\n\n");
    }

    output
}

fn csv_record(fields: &[impl AsRef<str>]) -> String {
    fields
        .iter()
        .map(|field| csv_field(field.as_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_field(value: &str) -> String {
    if !value.contains([',', '"', '\n', '\r']) {
        return value.to_string();
    }

    format!("\"{}\"", value.replace('"', "\"\""))
}

fn sanitize_file_stem(title: &str, conversation_id: i32) -> String {
    let sanitized = title
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();

    if sanitized.is_empty() {
        format!("conversation-{conversation_id}")
    } else {
        sanitized
    }
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("conversation");
    let extension = path.extension().and_then(|extension| extension.to_str());

    for ix in 2.. {
        let file_name = match extension {
            Some(extension) if !extension.is_empty() => format!("{stem}({ix}).{extension}"),
            _ => format!("{stem}({ix})"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("unbounded suffix search should always return")
}

fn format_time(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ExportType, csv_record, sanitize_file_stem, suggested_export_file_name, unique_path,
    };
    use crate::database::Conversation;
    use time::OffsetDateTime;

    fn conversation(title: &str) -> Conversation {
        Conversation {
            id: 7,
            path: format!("/{title}"),
            folder_id: None,
            title: title.to_string(),
            icon: "🤖".to_string(),
            created_time: OffsetDateTime::UNIX_EPOCH,
            updated_time: OffsetDateTime::UNIX_EPOCH,
            info: None,
            messages: vec![],
        }
    }

    #[test]
    fn file_stem_sanitizes_invalid_path_characters() {
        assert_eq!(sanitize_file_stem("a/b:c*?", 1), "a_b_c__");
        assert_eq!(sanitize_file_stem("...", 42), "conversation-42");
    }

    #[test]
    fn suggested_export_file_name_uses_conversation_title_and_extension() {
        assert_eq!(
            suggested_export_file_name(&conversation("Daily Notes"), ExportType::Json),
            "Daily Notes.json"
        );
    }

    #[test]
    fn csv_record_escapes_commas_quotes_and_newlines() {
        assert_eq!(
            csv_record(&["plain", "a,b", "say \"hi\"", "two\nlines"]),
            "plain,\"a,b\",\"say \"\"hi\"\"\",\"two\nlines\""
        );
    }

    #[test]
    fn unique_path_appends_suffix_without_overwriting_existing_file() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("ai-chat-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let first = dir.join("chat.txt");
        std::fs::write(&first, "exists")?;

        assert_eq!(unique_path(&first), dir.join("chat(2).txt"));
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }
}
