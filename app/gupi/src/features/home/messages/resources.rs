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
                let value = line.trim_end_matches(['\r', '\n']);
                let plain = paragraph.children.iter().any(|child| {
                    matches!(child, markdown_ast::Node::Text(_))
                        && child.position().is_some_and(|position| {
                            position.start.offset <= offset
                                && position.end.offset >= offset + value.len()
                        })
                });
                if plain
                    && let Some(path) = value
                        .strip_prefix('@')
                        .filter(|p| PathBuf::from(p).is_absolute())
                    && let Ok(url) = url::Url::from_file_path(path)
                {
                    let label = value
                        .replace('\\', "\\\\")
                        .replace('[', "\\[")
                        .replace(']', "\\]");
                    replacements
                        .push((offset..offset + value.len(), format!("[{label}](<{url}>)")));
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

pub(super) struct ResourceLinks;

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
            MarkdownNode::new(self.name(), path)
                .text(label)
                .markdown(context.node_source(node)?.to_owned()),
        )
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        let path = node.data::<PathBuf>().expect("file link data").clone();
        Button::new(format!(
            "file-link-{}",
            node.source_range().map_or(0, |r| r.start)
        ))
        .ghost()
        .xsmall()
        .icon(IconName::FileText)
        .max_w_full()
        .min_w_0()
        .label(if node.as_text().strip_prefix('@') == path.to_str() {
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
