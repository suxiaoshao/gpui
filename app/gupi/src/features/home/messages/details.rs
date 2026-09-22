//! Window-owned readers for recorded summaries and live tool details.
use super::{
    actions::copy_button,
    activity::Tool,
    presentation::{code_fence, tool_action, tool_title},
    tool_details::{self, Detail, Section},
    *,
};
use gpui_kit::base::TextSelection;
use gpui_kit::component::{
    StyledExt,
    scroll::ScrollableElement,
    text::{TextView, TextViewState},
};

struct Target {
    key: String,
    tool: String,
    preview: Option<String>,
}
struct TextBlock {
    source: String,
    view: Entity<TextViewState>,
    _subscription: Subscription,
}
struct DetailsView {
    tool: Option<Tool>,
    metadata: Vec<Detail>,
    sections: Vec<Section>,
    text: BTreeMap<String, TextBlock>,
    scroll: ScrollHandle,
    follow: bool,
    _subscription: Option<Subscription>,
}

fn code_source(language: &str, text: &str) -> String {
    // Each TextView contains one code block. EOF closes it for rendering;
    // omitting the closing fence keeps cumulative output append-compatible.
    // A longer fence in the payload changes the opener and correctly replaces it.
    format!("{}{language}\n{text}", code_fence(text))
}

impl HomeView {
    pub(super) fn summary_trigger(&self, message: &DisplayMessage, cx: &App) -> impl IntoElement {
        let text = message.text();
        let selector = format!("summary-details-{}", message.id);
        Button::new(format!("summary-details-{}", message.id))
            .debug_selector(move || selector.clone())
            .ghost()
            .small()
            .icon(IconName::FileText)
            .label(t(cx, "conversation-compaction"))
            .tooltip(t(cx, "message-details-open"))
            .on_click(move |_, window, cx| {
                let text = text.clone();
                let copy = text.clone();
                let view = cx.new(|cx| {
                    let mut view = DetailsView::empty();
                    view.sections.push(Section {
                        id: "summary",
                        label: "conversation-compaction",
                        parts: vec![Detail::Code {
                            language: String::new(),
                            text,
                        }],
                    });
                    view.sync_text(window, cx);
                    view
                });
                window.open_dialog(cx, move |dialog, window, cx| {
                    dialog
                        .title(
                            h_flex()
                                .gap_2()
                                .mr_8()
                                .child(t(cx, "conversation-compaction"))
                                .child(copy_button(
                                    format!("summary-copy-all-{}", view.entity_id().as_u64()),
                                    copy.clone(),
                                    t(cx, "message-details-copy-all"),
                                    window,
                                    cx,
                                )),
                        )
                        .width(
                            (window.rem_size() * 48.)
                                .min(window.viewport_size().width - window.rem_size() * 2.),
                        )
                        .on_ok(|_, _, _| false)
                        .content({
                            let view = view.clone();
                            move |content, _, _| content.min_h_0().child(view.clone())
                        })
                });
            })
    }

    pub(super) fn tool_trigger(&self, key: &str, tool: &Tool, cx: &App) -> impl IntoElement {
        let state = self.state.clone();
        let key = key.to_owned();
        let preview = self.views[&key].preview.clone();
        let tool = tool.clone();
        let title = tool_title(&tool, cx);
        let failed = tool.status == ToolStatus::Failed;
        let error = failed.then(|| {
            crate::foundation::session_catalog::text_content(&tool.result)
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or_default()
                .to_owned()
        });
        v_flex()
            .min_w_0()
            .child(
                Button::new(format!("tool-details-{}", tool.id))
                    .ghost()
                    .small()
                    .icon(tool.kind().icon())
                    .label(title)
                    .max_w_full()
                    .when(failed, |button| button.text_color(cx.theme().danger))
                    .tooltip(t(cx, "message-details-open"))
                    .on_click(move |_, window, cx| {
                        let target = Target {
                            key: key.clone(),
                            tool: tool.id.clone(),
                            preview: preview.clone(),
                        };
                        let view = cx.new(|cx| {
                            DetailsView::for_tool(&state, target, tool.clone(), window, cx)
                        });
                        let title = tool.name.clone();
                        window.open_dialog(cx, move |dialog, window, _| {
                            dialog
                                .title(title.clone())
                                .width(
                                    (window.rem_size() * 48.)
                                        .min(window.viewport_size().width - window.rem_size() * 2.),
                                )
                                .on_ok(|_, _, _| false)
                                .content({
                                    let view = view.clone();
                                    move |content, _, _| content.min_h_0().child(view.clone())
                                })
                        });
                    }),
            )
            .when_some(error.filter(|s| !s.is_empty()), |body, error| {
                body.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .truncate()
                        .child(error),
                )
            })
    }
}

fn find_tool(state: &ConversationState, target: &Target) -> Option<Tool> {
    let session = state.sessions.get(&target.key)?;
    // The RPC call ID survives live-message IDs being replaced by history entry IDs.
    for row in project(session, target.preview.as_deref()) {
        if let RowKind::Run {
            messages, active, ..
        } = row.kind
            && let Some(tool) =
                RunContent::project(&messages, if active { &session.tools } else { &[] }, active)
                    .activities
                    .into_iter()
                    .find_map(|activity| match activity {
                        Activity::Tool(tool) if tool.id == target.tool => Some(tool),
                        _ => None,
                    })
        {
            return Some(tool);
        }
    }
    None
}

impl DetailsView {
    fn for_tool(
        state: &Entity<ConversationState>,
        target: Target,
        tool: Tool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::empty();
        view.follow = tool.status == ToolStatus::Running;
        view.set_tool(tool, window, cx);
        view._subscription = Some(cx.subscribe_in(
            state,
            window,
            move |this, state, event: &ConversationEvent, window, cx| {
                let ConversationEvent::Changed(changes) = event else {
                    return;
                };
                if !changes.bodies.contains(&target.key)
                    && !changes.sessions.contains_key(&target.key)
                {
                    return;
                }
                if let Some(tool) = find_tool(state.read(cx), &target) {
                    this.set_tool(tool, window, cx);
                }
            },
        ));
        view
    }
    fn empty() -> Self {
        Self {
            tool: None,
            metadata: vec![],
            sections: vec![],
            text: BTreeMap::new(),
            scroll: ScrollHandle::new(),
            follow: false,
            _subscription: None,
        }
    }
    fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        if self.tool.as_ref() == Some(&tool) {
            return;
        }
        let document = tool_details::document(&tool);
        self.metadata = document.metadata;
        self.sections = document.sections;
        self.tool = Some(tool);
        self.sync_text(window, cx);
        cx.notify();
    }
    fn sync_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut keep = HashSet::new();
        for section in &self.sections {
            // Pi content blocks have ordered positions rather than individual IDs.
            for (index, part) in section.parts.iter().enumerate() {
                let Detail::Code { language, text } = part else {
                    continue;
                };
                let id = format!("{}-{index}", section.id);
                keep.insert(id.clone());
                let source = if section.id == "summary" {
                    text.clone()
                } else {
                    code_source(language, text)
                };
                if let Some(block) = self.text.get_mut(&id) {
                    if source != block.source {
                        block.view.update(cx, |view, cx| {
                            if let Some(delta) = source.strip_prefix(&block.source) {
                                view.push_str(delta, cx);
                            } else {
                                view.set_text(&source, cx);
                            }
                        });
                        block.source = source;
                    }
                } else {
                    let view = cx.new(|cx| TextViewState::markdown(&source, cx));
                    let subscription = cx.observe_in(&view, window, |this, _, window, cx| {
                        if this.follow && !TextSelection::has_selection(window, cx) {
                            this.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    });
                    self.text.insert(
                        id,
                        TextBlock {
                            source,
                            view,
                            _subscription: subscription,
                        },
                    );
                }
            }
        }
        self.text.retain(|id, _| keep.contains(id));
    }
    fn update_follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow = self.tool.is_some()
            && (self.scroll.max_offset().y + self.scroll.offset().y).abs() <= px(1.)
            && !TextSelection::has_selection(window, cx);
    }
}
impl Render for DetailsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().min_w_0().gap_4();
        if let Some(tool) = &self.tool {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(if tool.status == ToolStatus::Failed {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(tool_action(tool, cx)),
            );
        }
        for detail in &self.metadata {
            let text = match detail {
                Detail::Path(value) | Detail::Query(value) => value.clone(),
                Detail::Field(label, value) => format!("{} {value}", t(cx, label)),
                _ => unreachable!(),
            };
            body = body.child(div().text_sm().child(text));
        }
        for section in &self.sections {
            let mut content = v_flex().min_w_0().flex_none().gap_2();
            if section.id != "summary" {
                let copy = section.copy_text();
                content = content.child(
                    h_flex()
                        .gap_2()
                        .child(div().font_semibold().child(t(cx, section.label)))
                        .when(!copy.is_empty(), |head| {
                            head.child(copy_button(
                                format!("detail-{}-copy-{}", cx.entity_id().as_u64(), section.id),
                                copy,
                                t(cx, "conversation-copy"),
                                window,
                                cx,
                            ))
                        }),
                );
            }
            for (index, part) in section.parts.iter().enumerate() {
                content = match part {
                    Detail::Code { .. } => content.child(
                        TextView::new(&self.text[&format!("{}-{index}", section.id)].view)
                            .selectable(true),
                    ),
                    Detail::Image { mime, data } => content.child(tool_details::ToolImage {
                        id: format!("detail-image-{}-{index}", section.id),
                        mime: mime.clone(),
                        data: data.clone(),
                    }),
                    Detail::Notice(label) => content.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(cx, label)),
                    ),
                    Detail::Status(label, value) => {
                        content.child(div().text_sm().child(match value {
                            Some(value) => format!("{} {value}", t(cx, label)),
                            None => t(cx, label),
                        }))
                    }
                    _ => unreachable!(),
                };
            }
            body = body.child(content);
        }
        body.id("message-details-body")
            .debug_selector(|| "message-details-body".into())
            .max_h((window.rem_size() * 32.).min(window.viewport_size().height * 0.65))
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .vertical_scrollbar(&self.scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.follow = false),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|_, _, window, cx| {
                    cx.defer_in(window, |this, window, cx| this.update_follow(window, cx))
                }),
            )
            .on_scroll_wheel(cx.listener(|_, _, window, cx| {
                cx.defer_in(window, |this, window, cx| this.update_follow(window, cx))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{DetailsView, Target, code_source, find_tool};
    use crate::state::{
        conversation::{
            Changes, ConversationEvent, ConversationState, Session, ToolActivity,
            execution::ToolExecution,
        },
        pi,
    };
    use gpui_kit::{AppContext, TestAppContext, component::Root};
    use serde_json::json;

    #[gpui_kit::test]
    fn cumulative_code_output_appends_and_keeps_literal_fences(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut source = code_source("text", "first");
        let view = cx.new(|cx| super::TextViewState::markdown(&source, cx));
        cx.run_until_parked();
        for payload in [
            "first\nsecond",
            "first\nsecond\n",
            "first\nsecond\n```rust\nlet a = 1;\n```",
        ] {
            let next = code_source("text", payload);
            if !payload.contains('`') {
                assert!(next.starts_with(&source), "ordinary output must append");
            } else {
                assert!(
                    !next.starts_with(&source),
                    "a longer delimiter needs replacement"
                );
            }
            view.update(cx, |view, cx| {
                if let Some(delta) = next.strip_prefix(&source) {
                    view.push_str(delta, cx);
                } else {
                    view.set_text(&next, cx);
                }
            });
            source = next;
            cx.run_until_parked();
            view.update(cx, |view, cx| {
                view.select_all(cx);
                // Markdown's rendered code selection terminates the last line.
                let expected = if payload.ends_with('\n') {
                    payload.to_owned()
                } else {
                    format!("{payload}\n")
                };
                assert_eq!(view.selected_text(), expected);
                view.clear_selection(cx);
            });
        }
        assert!(!code_source("rust", "first").starts_with(&code_source("text", "first")));
        assert!(!code_source("text", "replacement").starts_with(&source));
    }

    fn session(output: &str) -> Session {
        let mut session = Session::from_rpc_messages(&[
            json!({"type":"message_end", "message":{"role":"user", "timestamp":1, "content":"test"}}),
            json!({"type":"message_end", "message":{"role":"assistant", "timestamp":2, "stopReason":"toolUse", "content":[{"type":"toolCall", "id":"call", "name":"bash", "arguments":{"command":"printf hello"}}]}}),
        ]);
        session.tools.push(ToolActivity {
            id: "call".into(),
            name: "bash".into(),
            args: json!({"command":"printf hello"}),
            execution: ToolExecution::Running(json!({"content":[{"type":"text", "text":output}]})),
        });
        session
    }
    fn target() -> Target {
        Target {
            key: "source".into(),
            tool: "tool-call".into(),
            preview: None,
        }
    }
    #[gpui_kit::test]
    fn tool_reader_updates_only_its_source_and_survives_message_identity_changes(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(Default::default(), cx);
            pi::init(cx);
        });
        let state = cx.new(|cx| ConversationState::new("unused-pi".into(), cx));
        state.update(cx, |state, _| {
            state.sessions.insert("source".into(), session("first"));
        });
        let mut reader = None;
        let (_, visual) = cx.add_window_view(|window, cx| {
            let tool = find_tool(state.read(cx), &target()).unwrap();
            let view = cx.new(|cx| DetailsView::for_tool(&state, target(), tool, window, cx));
            reader = Some(view.clone());
            Root::new(view, window, cx)
        });
        let reader = reader.unwrap();
        visual.run_until_parked();
        let text = visual.update(|_, cx| reader.read(cx).text["output-0"].view.clone());
        state.update(visual, |state, cx| {
            let mut next = session("first\nsecond");
            // Reconciliation can replace live message IDs with persisted entry IDs.
            for message in &mut next.live {
                message.id = format!("entry-{}", message.id);
            }
            state.sessions.insert("source".into(), next);
            cx.emit(ConversationEvent::Changed(Changes {
                bodies: ["other".into()].into(),
                ..Default::default()
            }));
        });
        visual.run_until_parked();
        visual.update(|_, cx| assert_eq!(reader.read(cx).sections[1].copy_text(), "first"));
        state.update(visual, |_, cx| {
            cx.emit(ConversationEvent::Changed(Changes {
                bodies: ["source".into()].into(),
                ..Default::default()
            }))
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            let reader = reader.read(cx);
            assert_eq!(reader.sections[1].copy_text(), "first\nsecond");
            assert_eq!(reader.text["output-0"].view, text);
            assert!(state.read(cx).sessions["source"].instance.is_none());
        });
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            text.update(cx, |text, cx| text.select_all(cx));
            reader.update(cx, |reader, cx| {
                reader.update_follow(window, cx);
                assert!(!reader.follow, "selecting output must pause following");
            });
        });
        let weak = reader.downgrade();
        visual.update(|window, _| window.remove_window());
        drop(reader);
        visual.run_until_parked();
        assert!(weak.upgrade().is_none());
    }
}
