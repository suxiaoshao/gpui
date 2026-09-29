//! Resource presentation preserves the Markdown text used by conversation find.
use crate::foundation::assets::IconName;
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
    text::{MarkdownNode, MarkdownParseContext, MarkdownPlugin, markdown_ast},
};
use gpui_kit::{App, IntoElement, Styled, Window};
use std::path::PathBuf;

pub(super) struct Skill<'a> {
    pub name: &'a str,
    pub path: &'a str,
    pub body: &'a str,
    pub arguments: &'a str,
}

/// Match Pi's complete skill envelope, never arbitrary XML-looking prose.
pub(super) fn skill(text: &str) -> Option<Skill<'_>> {
    static PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"\A<skill name="([^"]+)" location="([^"]+)">\n([\s\S]*?)\n</skill>(?:\n\n([\s\S]+))?\z"#).unwrap()
    });
    let captures = PATTERN.captures(text)?;
    Some(Skill {
        name: captures.get(1)?.as_str(),
        path: captures.get(2)?.as_str(),
        body: captures.get(3)?.as_str(),
        arguments: captures.get(4).map_or("", |m| m.as_str()),
    })
}

/// Only complete file-reference lines become links. AST parsing excludes
/// code fences, inline code and ordinary prose without guessing at @ words.
pub(super) fn file_references(text: &str) -> String {
    if !text.contains('@') {
        return text.to_owned();
    }
    let Ok(ast) = markdown::to_mdast(text, &markdown::ParseOptions::gfm()) else {
        return text.to_owned();
    };
    let mut replacements = vec![];
    if let markdown_ast::Node::Root(root) = ast {
        for node in root.children {
            let markdown_ast::Node::Paragraph(paragraph) = node else {
                continue;
            };
            let Some(position) = paragraph.position else {
                continue;
            };
            let mut offset = position.start.offset;
            for line in text[position.start.offset..position.end.offset].split_inclusive('\n') {
                // Composer tokens include separators outside the quoted path.
                // Keep those in the source but exclude them from the AST span and URL.
                let value = line.trim_matches([' ', '\t', '\r', '\n']);
                let start = offset + line.len() - line.trim_start_matches([' ', '\t']).len();
                let plain = paragraph.children.iter().any(|child| {
                    matches!(child, markdown_ast::Node::Text(_))
                        && child.position().is_some_and(|position| {
                            position.start.offset <= start
                                && position.end.offset >= start + value.len()
                        })
                });
                if plain
                    && let Some(path) = reference_path(value)
                    && let Ok(url) = url::Url::from_file_path(path)
                {
                    let label = value
                        .replace('\\', "\\\\")
                        .replace('[', "\\[")
                        .replace(']', "\\]");
                    replacements.push((start..start + value.len(), format!("[{label}](<{url}>)")));
                }
                offset += line.len();
            }
        }
    }
    let mut result = text.to_owned();
    for (range, replacement) in replacements.into_iter().rev() {
        result.replace_range(range, &replacement);
    }
    result
}

fn reference_path(value: &str) -> Option<PathBuf> {
    let mut decoded = String::new();
    let reference = if value.starts_with(['\'', '"']) {
        // Inverse of shortcuts::argument: adjacent quoted segments, without
        // shell-style backslash escapes. Reject prose after the quoted path.
        let mut quote = None;
        for ch in value.chars() {
            match quote {
                Some(active) if ch == active => quote = None,
                Some(_) => decoded.push(ch),
                None if matches!(ch, '\'' | '"') => quote = Some(ch),
                None => return None,
            }
        }
        if quote.is_some() {
            return None;
        }
        decoded.as_str()
    } else {
        value
    };
    let path = PathBuf::from(reference.strip_prefix('@')?);
    path.is_absolute().then_some(path)
}

pub(super) struct ResourceLinks;

struct ResourcePath {
    path: PathBuf,
    directory: bool,
}

impl MarkdownPlugin for ResourceLinks {
    fn name(&self) -> &str {
        "gupi-file-link"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        context: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let markdown_ast::Node::Link(link) = node else {
            return None;
        };
        // Leave web URLs and rich/nested link labels to the ordinary renderer.
        let path = if link.url.starts_with("file:") {
            url::Url::parse(&link.url).ok()?.to_file_path().ok()?
        } else {
            let path = PathBuf::from(&link.url);
            if !path.is_absolute() {
                return None;
            }
            path
        };
        let mut label = String::new();
        for child in &link.children {
            let markdown_ast::Node::Text(text) = child else {
                return None;
            };
            label.push_str(&text.value);
        }
        if label.is_empty() {
            return None;
        }
        Some(
            MarkdownNode::new(
                self.name(),
                ResourcePath {
                    directory: path.is_dir(),
                    path,
                },
            )
            .text(label)
            .markdown(context.node_source(node)?.to_owned()),
        )
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        let resource = node.data::<ResourcePath>().expect("path link data");
        let path = resource.path.clone();
        Button::new(format!(
            "file-link-{}",
            node.source_range().map_or(0, |r| r.start)
        ))
        .ghost()
        .xsmall()
        .icon(if resource.directory {
            IconName::Folder
        } else {
            IconName::FileText
        })
        .max_w_full()
        .min_w_0()
        .label(if reference_path(node.as_text()).as_ref() == Some(&path) {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            node.as_text().to_owned()
        })
        .tooltip(path.to_string_lossy().to_string())
        .on_click(move |_, _, cx| cx.open_with_system(&path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_projection_round_trips_composer_paths_without_token_separators() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "report.txt",
            "report notes.txt",
            "报告 'one' \"two\".txt",
            "report.txt ",
        ] {
            let path = root.path().join(name);
            for leading_space in [false, true] {
                let token = crate::foundation::composer_resources::file_token(&path, leading_space);
                for suffix in ["", "\n", "\nfollowing text", "  \nfollowing text"] {
                    let source = format!("{}{suffix}", token.text());
                    let display = file_references(&source);
                    let ast = markdown::to_mdast(&display, &markdown::ParseOptions::gfm()).unwrap();
                    let links: Vec<_> = ast
                        .children()
                        .unwrap()
                        .iter()
                        .flat_map(|node| node.children().into_iter().flatten())
                        .filter_map(|node| match node {
                            markdown_ast::Node::Link(link) => Some(link),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(links.len(), 1, "source={source:?}, display={display:?}");
                    assert_eq!(
                        links[0].url,
                        url::Url::from_file_path(&path).unwrap().as_str()
                    );
                    let label: String = links[0]
                        .children
                        .iter()
                        .map(|node| node.to_string())
                        .collect();
                    assert_eq!(label, token.text().trim());
                    assert!(display.ends_with(&format!(" {suffix}")), "{display:?}");
                }
            }
        }
    }

    #[test]
    fn file_projection_keeps_prose_and_code_and_only_tags_complete_reference_lines() {
        let source = "Please inspect\n@/tmp/report [one].txt\n\n`@/tmp/not-a-file`\n\n```\n@/tmp/code\n```\n\nemail @someone";
        let display = file_references(source);
        assert!(
            display.starts_with(
                "Please inspect\n[@/tmp/report \\[one\\].txt](<file:///tmp/report%20[one].txt>)"
            ),
            "{display}"
        );
        assert!(display.contains("```\n@/tmp/code\n```"));
        assert!(display.contains("`@/tmp/not-a-file`"));
        assert!(display.ends_with("email @someone"));
        let inline = "`inline code\n@/tmp/inside-code\nend`";
        assert_eq!(file_references(inline), inline);
        let token = crate::foundation::composer_resources::file_token(
            std::path::Path::new("/tmp/report notes.txt"),
            false,
        );
        for source in [
            format!("`{}`", token.text()),
            format!("```\n{}\n```", token.text()),
            format!("inspect {}", token.text()),
            format!("{}is a reference", token.text()),
            "\"@/tmp/unclosed.txt".into(),
        ] {
            assert_eq!(file_references(&source), source);
        }
    }
    #[test]
    fn skill_projection_requires_complete_pi_envelope_and_preserves_recorded_content() {
        let text = "<skill name=\"review\" location=\"/tmp/SKILL.md\">\nRecorded instructions\n</skill>\n\nPlease review";
        let skill = skill(text).unwrap();
        assert_eq!(skill.name, "review");
        assert_eq!(skill.body, "Recorded instructions");
        assert_eq!(skill.arguments, "Please review");
        assert!(super::skill(&format!("example: {text}")).is_none());
    }
}
