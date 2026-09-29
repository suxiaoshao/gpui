//! Pi resource identity for the editor. Execution remains with Pi.
use gpui_kit::component::input::{InlineToken, InputContent};
use pi_rpc::protocol::SlashCommand;
use std::{ops::Range, path::Path};

pub(crate) fn file_token(path: &Path, leading_space: bool) -> InlineToken {
    // Classify once when inserting, never while rendering the token.
    let kind = if path.is_dir() { "directory" } else { "file" };
    let path = path.to_string_lossy();
    let reference = format!("@{path}");
    let reference = if reference.contains(|c: char| c.is_whitespace() || matches!(c, '\'' | '"')) {
        crate::state::shortcuts::argument(&reference)
    } else {
        reference
    };
    InlineToken::new(
        format!("{kind}:{path}"),
        format!("{}{reference} ", if leading_space { " " } else { "" }),
    )
    .with_label(format!(
        "@{}",
        Path::new(path.as_ref())
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ))
}

pub(crate) fn file_path(token: &InlineToken) -> Option<&Path> {
    token
        .id()
        .strip_prefix("file:")
        .or_else(|| token.id().strip_prefix("directory:"))
        .map(Path::new)
}

pub(crate) fn token(command: &SlashCommand, separator: bool) -> Option<InlineToken> {
    if !matches!(command.source.as_str(), "skill" | "prompt") {
        return None;
    }
    Some(
        InlineToken::new(
            format!("{}:{}", command.source, command.name),
            format!("/{}{}", command.name, if separator { " " } else { "" }),
        )
        .with_label(
            command
                .name
                .strip_prefix("skill:")
                .unwrap_or(&command.name)
                .to_owned(),
        ),
    )
}

pub(crate) fn command<'a>(text: &str, commands: &'a [SlashCommand]) -> Option<&'a SlashCommand> {
    let name = text.strip_prefix('/')?.split_whitespace().next()?;
    // Keep Pi's first-match precedence, including extension commands.
    commands.iter().find(|command| command.name == name)
}

pub(crate) fn restore(content: &InputContent, commands: &[SlashCommand]) -> InputContent {
    if !content.tokens().is_empty() {
        return content.clone();
    }
    let Some(token) = command(content.text(), commands).and_then(|command| {
        token(
            command,
            content
                .text()
                .get(command.name.len() + 1..)
                .is_some_and(|suffix| suffix.starts_with(' ')),
        )
    }) else {
        return content.clone();
    };
    let end = token.text().len();
    content
        .clone()
        .with_token(0..end, token)
        .unwrap_or_else(|_| content.clone())
}

pub(crate) fn replacement(
    text: &str,
    from_composer: bool,
    commands: &[SlashCommand],
) -> Range<usize> {
    if text.starts_with('/') && (from_composer || command(text, commands).is_some()) {
        0..text.find(char::is_whitespace).unwrap_or(text.len())
    } else {
        0..0
    }
}

/// Locate Pi's quote-aware arguments; only parameter positions are inspected,
/// never expanded or substituted by the GUI.
fn arguments(text: &str) -> Option<Vec<Range<usize>>> {
    let mut quote = None;
    let mut nonempty = false;
    let mut start = 0;
    let mut ranges = Vec::new();
    for (offset, c) in text.char_indices() {
        if let Some(active) = quote {
            if c == active {
                quote = None
            } else {
                nonempty = true
            }
        } else if matches!(c, '\'' | '"') {
            quote = Some(c)
        } else if c.is_whitespace() {
            if nonempty {
                ranges.push(start..offset);
            }
            nonempty = false;
            start = offset + c.len_utf8();
        } else {
            nonempty = true
        }
    }
    if nonempty {
        ranges.push(start..text.len());
    }
    quote.is_none().then_some(ranges)
}

pub(crate) fn file_positions(content: &InputContent) -> Option<Vec<usize>> {
    let args = arguments(content.text())?;
    Some(
        args.iter()
            .enumerate()
            .skip(1)
            .filter_map(|(position, range)| {
                content
                    .tokens()
                    .iter()
                    .any(|span| {
                        file_path(span.token()).is_some()
                            && span.range().start < range.end
                            && span.range().end > range.start
                    })
                    .then_some(position)
            })
            .collect(),
    )
}

pub(crate) fn accepts_files(template: &str, positions: impl IntoIterator<Item = usize>) -> bool {
    static PARAMETERS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r"\$\{(\d+|ARGUMENTS|@):-([^}]*)\}|\$\{@:(\d+)(?::(\d+))?\}|\$(ARGUMENTS|@|\d+)",
        )
        .unwrap()
    });
    let normalized = template
        .strip_prefix('\u{feff}')
        .unwrap_or(template)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let body = normalized
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---").map(|(_, body)| body))
        .unwrap_or(&normalized);
    let consumed: Vec<_> = PARAMETERS
        .captures_iter(body)
        .map(|capture| {
            if let Some(start) = capture.get(3) {
                let start = start.as_str().parse::<usize>().unwrap_or(usize::MAX).max(1);
                let end = capture.get(4).map_or(usize::MAX, |length| {
                    start.saturating_add(length.as_str().parse().unwrap_or(usize::MAX))
                });
                start..end
            } else {
                let target = capture.get(1).or_else(|| capture.get(5)).unwrap().as_str();
                if matches!(target, "@" | "ARGUMENTS") {
                    1..usize::MAX
                } else {
                    let n = target.parse::<usize>().unwrap_or(usize::MAX);
                    n..n.saturating_add(1)
                }
            }
        })
        .collect();
    positions
        .into_iter()
        .all(|position| consumed.iter().any(|range| range.contains(&position)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(name: &str, source: &str) -> SlashCommand {
        serde_json::from_value(serde_json::json!({"name":name,"source":source,"sourceInfo":{}}))
            .unwrap()
    }
    #[test]
    fn resource_restore_and_selection_preserve_pi_precedence_and_existing_arguments() {
        let commands = [
            entry("skill:review", "skill"),
            entry("summary", "extension"),
            entry("summary", "prompt"),
        ];
        let content = restore(&"/skill:review 中文参数".into(), &commands);
        assert_eq!(content.tokens()[0].range(), 0..14);
        assert_eq!(content.tokens()[0].token().label().as_ref(), "review");
        assert!(
            restore(&"/summary 参数".into(), &commands)
                .tokens()
                .is_empty()
        );
        assert_eq!(
            replacement("/skill:review 中文参数", false, &commands),
            0..13
        );
        assert_eq!(replacement("/unknown 参数", false, &commands), 0..0);
        assert_eq!(replacement("/rev 中文参数", true, &commands), 0..4);
    }

    #[test]
    fn template_validation_checks_file_positions_not_just_placeholder_presence() {
        assert_eq!(
            arguments("hello \"with spaces\" 'quoted'\nnext").map(|args| args.len()),
            Some(4)
        );
        assert!(!accepts_files("Summarize", 2..4));
        assert!(!accepts_files("Only $1", 2..4));
        assert!(accepts_files("$2 ${3:-missing}", 2..4));
        assert!(accepts_files("${@:2:2}", 2..4));
        assert!(!accepts_files("${@:2:1}", 2..4));
        assert!(accepts_files("${@:0}", 1..4));
        assert!(accepts_files("$ARGUMENTS", 2..4));
        assert!(!accepts_files(
            "---\ndescription: $ARGUMENTS\n---\nOnly $1",
            2..4
        ));
        assert!(!accepts_files("${1:-$ARGUMENTS}", 2..4));
        assert!(!accepts_files(
            "\u{feff}---\rdescription: $ARGUMENTS\r---\rOnly $1",
            2..4
        ));
    }

    #[test]
    fn file_references_keep_their_actual_argument_positions() {
        let first = file_token(Path::new("/tmp/a 'quoted' \"name\".md"), false);
        let second = file_token(Path::new("/tmp/image.png"), false);
        let prefix = "/summary ";
        let middle = "compared with ";
        let second_start = prefix.len() + first.text().len() + middle.len();
        let content = InputContent::new(format!(
            "{prefix}{}{middle}{}tail",
            first.text(),
            second.text()
        ))
        .with_token(prefix.len()..prefix.len() + first.text().len(), first)
        .unwrap()
        .with_token(second_start..second_start + second.text().len(), second)
        .unwrap();
        assert_eq!(file_positions(&content), Some(vec![1, 4]));
        assert!(accepts_files(
            "$1 and $4",
            file_positions(&content).unwrap()
        ));
        assert!(!accepts_files(
            "$1 and $2",
            file_positions(&content).unwrap()
        ));
        assert_eq!(
            file_path(content.tokens()[1].token()),
            Some(Path::new("/tmp/image.png"))
        );
    }
}
