//! Tool-specific bodies inside the existing disclosure, using captured RPC data only.
use super::{activity::Tool, presentation::fenced, *};
use crate::foundation::tool_presentation::{ToolKind, read_path};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::prelude::FluentBuilder;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, PartialEq)]
enum Detail {
    Path(String),
    Query(String),
    Field(&'static str, String),
    Output,
    Code { language: String, text: String },
    Notice(&'static str),
    Status(&'static str, Option<String>),
    Image { mime: String, data: String },
}

impl Detail {
    fn code(language: &str, text: &str) -> Self {
        Self::Code {
            language: language.into(),
            text: text.into(),
        }
    }
}

impl HomeView {
    pub(super) fn tool_details(&self, key: &str, tool: &Tool, cx: &App) -> AnyElement {
        let kind = tool.kind();
        let mut primary = vec![];
        let mut fields = vec![];
        let mut status = vec![];
        let mut input = vec![];
        let mut output = vec![];
        let mut extra = vec![];
        let mut in_output = false;
        let mut in_extra = false;
        for detail in project(tool) {
            match detail {
                Detail::Output => in_output = true,
                Detail::Path(value) => {
                    if kind == ToolKind::Search {
                        fields.push(inline_code(&value));
                    } else {
                        primary.push(inline_code(&value));
                    }
                }
                Detail::Query(value) => primary.push(inline_code(&value)),
                Detail::Field(label, value) => {
                    fields.push(format!("{} {}", t(cx, label), inline_code(&value)));
                }
                Detail::Status(label, value) => status.push(match value {
                    Some(value) => format!("{} {}", t(cx, label), inline_code(&value)),
                    None => t(cx, label),
                }),
                Detail::Notice("tool-detail-additional") => {
                    in_extra = true;
                    extra.push(detail);
                }
                detail => {
                    if in_extra {
                        extra.push(detail);
                    } else if in_output {
                        output.push(detail);
                    } else {
                        input.push(detail);
                    }
                }
            }
        }
        let mut header_content = vec![];
        if matches!(kind, ToolKind::Shell | ToolKind::Other) {
            header_content.append(&mut input);
        }
        // File mutations present the submitted content/diff as the body, and
        // the execution result beneath it. Other tools return the main body.
        if matches!(kind, ToolKind::Edit | ToolKind::Write)
            || (kind == ToolKind::Shell && tool.status == ToolStatus::Failed)
        {
            extra.splice(0..0, output);
        } else {
            input.append(&mut output);
        }
        let has_header = !primary.is_empty() || !fields.is_empty() || !header_content.is_empty();
        let has_body = !input.is_empty();
        let has_footer = !extra.is_empty() || !status.is_empty();
        if !has_header && !has_body && !has_footer {
            return div().into_any_element();
        }
        // Only expanded details own this surface. The Marker and disclosure
        // button remain outside, with their existing appearance and behavior.
        let mut card = v_flex()
            .w_full()
            .min_w_0()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .overflow_hidden();
        if has_header {
            card = card.child(
                v_flex()
                    .min_w_0()
                    .gap_1()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().muted)
                    .when(!primary.is_empty(), |header| {
                        header.child(
                            self.text_view(
                                key,
                                format!("tool-primary-{}", tool.id),
                                primary.join(" · "),
                            )
                            .embedded(),
                        )
                    })
                    .when(!header_content.is_empty(), |header| {
                        header.child(self.tool_detail_content(
                            key,
                            &tool.id,
                            "header",
                            header_content,
                            cx,
                        ))
                    })
                    .when(!fields.is_empty(), |header| {
                        header.child(
                            self.text_view(
                                key,
                                format!("tool-fields-{}", tool.id),
                                fields.join(" · "),
                            )
                            .embedded(),
                        )
                    }),
            );
        }
        if has_body {
            card = card.child(
                div()
                    .min_w_0()
                    .px_3()
                    .py_2()
                    .when(has_header, |body| {
                        body.border_t_1().border_color(cx.theme().border)
                    })
                    .child(self.tool_detail_content(key, &tool.id, "body", input, cx)),
            );
        }
        if has_footer {
            card = card.child(
                v_flex()
                    .min_w_0()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .when(has_header || has_body, |footer| {
                        footer.border_t_1().border_color(cx.theme().border)
                    })
                    .when(!extra.is_empty(), |footer| {
                        footer.child(self.tool_detail_content(key, &tool.id, "footer", extra, cx))
                    })
                    .when(!status.is_empty(), |footer| {
                        footer.child(
                            self.text_view(
                                key,
                                format!("tool-status-{}", tool.id),
                                status.join("\n\n"),
                            )
                            .embedded(),
                        )
                    }),
            );
        }
        card.into_any_element()
    }

    fn tool_detail_content(
        &self,
        key: &str,
        tool_id: &str,
        section: &str,
        content: Vec<Detail>,
        cx: &App,
    ) -> AnyElement {
        let mut body = v_flex().w_full().min_w_0().gap_2();
        let mut markdown = vec![];
        for (index, detail) in content.into_iter().enumerate() {
            match detail {
                Detail::Code { language, text } => markdown.push(fenced(&language, &text)),
                Detail::Notice(label) => markdown.push(t(cx, label)),
                Detail::Image { mime, data } => {
                    if !markdown.is_empty() {
                        body = body.child(
                            self.text_view(
                                key,
                                format!("tool-body-{section}-{tool_id}-{index}"),
                                markdown.join("\n\n"),
                            )
                            .embedded(),
                        );
                        markdown.clear();
                    }
                    body = body.child(ToolImage {
                        id: format!("{key}-tool-image-{section}-{tool_id}-{index}"),
                        mime,
                        data,
                    });
                }
                Detail::Path(..)
                | Detail::Query(..)
                | Detail::Field(..)
                | Detail::Status(..)
                | Detail::Output => {
                    unreachable!("metadata was separated above")
                }
            }
        }
        if !markdown.is_empty() {
            body = body.child(
                self.text_view(
                    key,
                    format!("tool-body-{section}-{tool_id}-tail"),
                    markdown.join("\n\n"),
                )
                .embedded(),
            );
        }
        body.into_any_element()
    }
}

fn project(tool: &Tool) -> Vec<Detail> {
    let mut body = vec![];
    let args = tool.args.as_ref().unwrap_or(&Value::Null);
    let kind = tool.kind();
    if kind == ToolKind::Other {
        if let Some(args) = &tool.args {
            body.push(Detail::Notice("tool-detail-input"));
            body.push(Detail::code("json", &pretty(args)));
        }
    } else {
        if let Some(path) = read_path(args) {
            body.push(Detail::Path(path.into()));
        }
        if kind == ToolKind::Search
            && let Some(pattern) = args["pattern"].as_str()
        {
            body.push(Detail::Query(pattern.into()));
        }
        let fields: &[(&str, &str)] = match kind {
            ToolKind::Read | ToolKind::Skill => &[
                ("offset", "tool-detail-offset"),
                ("limit", "tool-detail-line-limit"),
            ],
            ToolKind::Shell => &[("timeout", "tool-detail-timeout")],
            ToolKind::Search | ToolKind::List => &[
                ("glob", "tool-detail-glob"),
                ("ignoreCase", "tool-detail-ignore-case"),
                ("literal", "tool-detail-literal"),
                ("context", "tool-detail-context"),
                ("limit", "tool-detail-result-limit"),
            ],
            _ => &[],
        };
        for (field, label) in fields {
            if let Some(value) = args.get(field) {
                body.push(Detail::Field(
                    label,
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                ));
            }
        }
        if kind == ToolKind::Shell {
            if let Some(command) = args["command"].as_str() {
                body.push(Detail::code(
                    if tool.name == "powershell" {
                        "powershell"
                    } else {
                        "bash"
                    },
                    command,
                ));
            }
        } else if kind == ToolKind::Write
            && let Some(content) = args["content"].as_str()
        {
            body.push(Detail::code(&file_language(args), content));
        }
    }

    let details = &tool.result["details"];
    let patch = (kind == ToolKind::Edit)
        .then(|| {
            details["patch"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|text| Detail::code("diff", text))
                // Older results have Pi's display diff, not a unified patch.
                .or_else(|| {
                    details["diff"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(|text| Detail::code("text", text))
                })
        })
        .flatten();
    let has_patch = patch.is_some();
    body.extend(patch);
    if kind == ToolKind::Edit && !has_patch {
        // Before execution (or when an extension returns no diff), keep the
        // requested edits inspectable without reading the current disk file.
        if let Some(edits) = args.get("edits") {
            body.push(Detail::code("json", &pretty(edits)));
        } else {
            for (field, label) in [
                ("oldText", "tool-detail-old-text"),
                ("newText", "tool-detail-new-text"),
            ] {
                if let Some(text) = args[field].as_str() {
                    body.push(Detail::Notice(label));
                    body.push(Detail::code(&file_language(args), text));
                }
            }
        }
    }

    // Content blocks retain their order. Updates are snapshots, never appended
    // to earlier tool results; history follows this same presentation path.
    let result_language =
        if matches!(kind, ToolKind::Read | ToolKind::Skill) && tool.status != ToolStatus::Failed {
            file_language(args)
        } else {
            "text".into()
        };
    body.push(Detail::Output);
    if tool.status == ToolStatus::Failed {
        body.push(Detail::Notice("tool-detail-error"));
    } else if kind == ToolKind::Other
        && tool.result["content"]
            .as_array()
            .is_some_and(|content| !content.is_empty())
    {
        body.push(Detail::Notice("tool-detail-output"));
    }
    if let Some(content) = tool.result["content"].as_array() {
        for part in content {
            match part["type"].as_str() {
                Some("text") => {
                    if let Some(text) = part["text"].as_str().filter(|s| !s.is_empty()) {
                        body.push(if kind == ToolKind::Other {
                            match serde_json::from_str::<Value>(text) {
                                Ok(value) => Detail::code("json", &pretty(&value)),
                                Err(_) => Detail::code("text", text),
                            }
                        } else {
                            Detail::code(&result_language, text)
                        });
                    }
                }
                Some("image") => {
                    if let (Some(mime), Some(data)) =
                        (part["mimeType"].as_str(), part["data"].as_str())
                    {
                        body.push(Detail::Image {
                            mime: mime.into(),
                            data: data.into(),
                        });
                    } else {
                        body.push(Detail::Notice("tool-detail-image-unavailable"));
                    }
                }
                _ => body.push(Detail::code("json", &pretty(part))),
            }
        }
    } else if let Some(text) = tool.result["content"].as_str().filter(|s| !s.is_empty()) {
        body.push(Detail::code(&result_language, text));
    }

    if kind == ToolKind::Other {
        if !details.is_null() {
            body.push(Detail::Notice("tool-detail-additional"));
            body.push(Detail::code("json", &pretty(details)));
        }
    } else {
        if details["truncation"]["truncated"] == true {
            body.push(Detail::Status("tool-detail-truncated", None));
        }
        if details["linesTruncated"] == true {
            body.push(Detail::Status("tool-detail-lines-truncated", None));
        }
        for (field, label) in [
            ("matchLimitReached", "tool-detail-match-limit"),
            ("resultLimitReached", "tool-detail-results-limited"),
            ("entryLimitReached", "tool-detail-entries-limited"),
            ("fullOutputPath", "tool-detail-full-output"),
        ] {
            if let Some(value) = details.get(field).filter(|v| !v.is_null()) {
                body.push(Detail::Status(
                    label,
                    Some(
                        value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    ),
                ));
            }
        }
    }
    body
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn file_language(args: &Value) -> String {
    let name = read_path(args)
        .unwrap_or_default()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let extension = name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let language = match extension.as_str() {
        "h" => "c",
        "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "mjs" | "cjs" | "jsx" => "javascript",
        "yml" => "yaml",
        "patch" => "diff",
        _ => &extension,
    };
    gpui_kit::component::highlighter::LanguageRegistry::singleton()
        .language(language)
        .map(|_| language.to_owned())
        .unwrap_or_else(|| "text".into())
}

fn inline_code(value: &str) -> String {
    let fence = "`".repeat(value.split(|c| c != '`').map(str::len).max().unwrap_or(0) + 1);
    format!("{fence} {} {fence}", value.replace(['\n', '\r'], " "))
}

#[derive(IntoElement)]
struct ToolImage {
    id: String,
    mime: String,
    data: String,
}
struct ImageState {
    mime: String,
    data: String,
    image: Option<Arc<Image>>,
}

impl ToolImage {
    fn decode(&self) -> Option<Arc<Image>> {
        let format = ImageFormat::from_mime_type(&self.mime)?;
        let bytes = STANDARD.decode(&self.data).ok()?;
        Some(Arc::new(Image::from_bytes(format, bytes)))
    }
}

impl RenderOnce for ToolImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ImageState {
            mime: self.mime.clone(),
            data: self.data.clone(),
            image: self.decode(),
        });
        state.update(cx, |state, _| {
            if state.mime != self.mime || state.data != self.data {
                state.image = self.decode();
                state.mime = self.mime;
                state.data = self.data;
            }
        });
        let unavailable = t(cx, "tool-detail-image-unavailable");
        if let Some(image) = state.read(cx).image.clone() {
            // Stable preview bounds avoid changing the virtual row height when
            // asynchronous image decoding completes. Contain never crops.
            img(image)
                .w_full()
                .h_64()
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || div().child(unavailable.clone()).into_any_element())
                .into_any_element()
        } else {
            div().child(unavailable).into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Detail, Tool, ToolImage, ToolKind, ToolStatus, pretty, project};
    use serde_json::{Value, json};

    fn tool(name: &str, args: Value, result: Value) -> Tool {
        Tool {
            id: "call".into(),
            entries: vec![],
            name: name.into(),
            args: Some(args),
            result,
            status: ToolStatus::Complete,
        }
    }

    #[test]
    fn edit_prefers_actual_patch_and_retains_failure_output() {
        let patch = "--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let mut edit = tool(
            "edit",
            json!({"path":"a.rs", "edits":[{"oldText":"requested", "newText":"replacement"}]}),
            json!({
                "content":[{"type":"text", "text":"Failed after editing"}],
                "details":{"patch":patch, "diff":"display-only"}
            }),
        );
        edit.status = ToolStatus::Failed;
        let body = project(&edit);
        assert!(body.contains(&Detail::code("diff", patch)));
        assert!(body.contains(&Detail::code("text", "Failed after editing")));
        assert!(!body.iter().any(|part| matches!(part, Detail::Code { text, .. } if text.contains("requested") || text == "display-only")));
        edit.result["details"] = json!({"diff":"+1 old Pi display format"});
        assert!(project(&edit).contains(&Detail::code("text", "+1 old Pi display format")));
    }

    #[test]
    fn read_skill_write_and_failed_read_use_the_right_content_language() {
        let read = tool(
            "read",
            json!({"file_path":"C:\\work\\a.rs", "offset":4, "limit":20}),
            json!({
                "content":[{"type":"text", "text":"fn main() {}"}]
            }),
        );
        let body = project(&read);
        assert!(body.contains(&Detail::Field("tool-detail-offset", "4".into())));
        assert!(body.contains(&Detail::Field("tool-detail-line-limit", "20".into())));
        assert!(body.contains(&Detail::code("rs", "fn main() {}")));
        let mut failed = read;
        failed.status = ToolStatus::Failed;
        assert!(project(&failed).contains(&Detail::code("text", "fn main() {}")));
        let skill = tool(
            "read",
            json!({"path":"skills/test/SKILL.md"}),
            json!({"content":[{"type":"text", "text":"# Skill"}]}),
        );
        assert_eq!(skill.kind(), ToolKind::Skill);
        assert!(project(&skill).contains(&Detail::code("md", "# Skill")));
        let write = tool(
            "write",
            json!({"path":"existing.py", "content":"print(1)"}),
            json!({"content":[{"type":"text", "text":"Written"}]}),
        );
        assert!(project(&write).contains(&Detail::code("py", "print(1)")));
        assert!(
            !project(&write)
                .iter()
                .any(|part| matches!(part, Detail::Code { language, .. } if language == "diff"))
        );
    }

    #[test]
    fn search_preserves_text_and_reports_upstream_limits_without_inventing_hits() {
        let search = tool(
            "grep",
            json!({"pattern":"a:b", "glob":"*.rs", "ignoreCase":true, "context":2}),
            json!({
                "content":[{"type":"text", "text":"C:\\work\\a.rs:10: a:b"}],
                "details":{"truncation":{"truncated":true},"linesTruncated":true,"matchLimitReached":100}
            }),
        );
        let body = project(&search);
        assert!(body.contains(&Detail::code("text", "C:\\work\\a.rs:10: a:b")));
        assert!(body.contains(&Detail::Query("a:b".into())));
        assert!(body.contains(&Detail::Status("tool-detail-truncated", None)));
        assert!(body.contains(&Detail::Status("tool-detail-lines-truncated", None)));
        assert!(body.contains(&Detail::Status(
            "tool-detail-match-limit",
            Some("100".into())
        )));
        let empty = tool(
            "find",
            json!({"pattern":"*.rs"}),
            json!({"content":[{"type":"text", "text":"No matches found"}]}),
        );
        assert!(project(&empty).contains(&Detail::code("text", "No matches found")));
    }

    #[test]
    fn shell_separates_command_from_cumulative_output_and_keeps_full_output_path() {
        let mut shell = tool(
            "powershell",
            json!({"command":"Get-ChildItem", "timeout":30}),
            json!({
                "content":[{"type":"text", "text":"one"}]
            }),
        );
        shell.status = ToolStatus::Running;
        assert!(project(&shell).contains(&Detail::code("powershell", "Get-ChildItem")));
        shell.result = json!({"content":[{"type":"text", "text":"one\ntwo"}], "details":{"fullOutputPath":"/tmp/output.txt"}});
        let body = project(&shell);
        assert!(body.contains(&Detail::code("text", "one\ntwo")));
        assert!(!body.contains(&Detail::code("text", "one")));
        assert!(body.contains(&Detail::Status(
            "tool-detail-full-output",
            Some("/tmp/output.txt".into())
        )));
    }

    #[test]
    fn custom_tools_keep_mixed_content_order_and_details() {
        let custom = tool(
            "plugin",
            json!({"custom":1}),
            json!({"content":[
            {"type":"text", "text":"{\"count\":1}"},
            {"type":"image", "mimeType":"image/png", "data":"abc="},
            {"type":"text", "text":"after"}
        ], "details":{"customResult":true}}),
        );
        let body = project(&custom);
        assert_eq!(
            body,
            vec![
                Detail::Notice("tool-detail-input"),
                Detail::code("json", &pretty(&json!({"custom":1}))),
                Detail::Output,
                Detail::Notice("tool-detail-output"),
                Detail::code("json", &pretty(&json!({"count":1}))),
                Detail::Image {
                    mime: "image/png".into(),
                    data: "abc=".into()
                },
                Detail::code("text", "after"),
                Detail::Notice("tool-detail-additional"),
                Detail::code("json", &pretty(&json!({"customResult":true}))),
            ]
        );
    }

    #[test]
    fn image_decodes_rpc_bytes_and_bad_payload_falls_back() {
        let mut image = ToolImage {
            id: "image".into(),
            mime: "image/png".into(),
            data: "iVBORw0KGgo=".into(),
        };
        assert_eq!(image.decode().unwrap().bytes, b"\x89PNG\r\n\x1a\n");
        image.data = "invalid base64".into();
        assert!(image.decode().is_none());
        image.mime = "image/unknown".into();
        assert!(image.decode().is_none());
    }

    #[test]
    fn component_diff_highlighting_distinguishes_additions_and_deletions() {
        use gpui_kit::component::highlighter::{HighlightTheme, SyntaxHighlighter};
        let patch = "--- a/test\n+++ b/test\n@@ -1 +1 @@\n-old\n+new\n";
        let mut highlighter = SyntaxHighlighter::new("diff");
        let mut rope = highlighter.text().clone();
        rope.insert(0, patch);
        assert!(highlighter.update(None, &rope, None));
        for theme in [
            HighlightTheme::default_light(),
            HighlightTheme::default_dark(),
        ] {
            let styles = highlighter.styles(&(0..patch.len()), theme.as_ref());
            let color = |needle: &str| {
                let offset = patch.find(needle).unwrap();
                styles
                    .iter()
                    .find(|(range, _)| range.contains(&offset))
                    .and_then(|(_, style)| style.color)
            };
            let removed = color("-old");
            let added = color("+new");
            assert!(removed.is_some() && added.is_some());
            assert_ne!(removed, added);
        }
    }
}
