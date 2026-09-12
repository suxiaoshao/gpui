use super::*;
use crate::state::conversation::content::BodyState;
use gpui_kit::component::input::Textarea;
use gpui_kit::prelude::FluentBuilder;
use pi_rpc::protocol::{UiMethod, UiReply};
mod metrics;
impl HomeView {
    fn reply_extension(&mut self, reply: UiReply, cx: &mut Context<Self>) {
        if let Some((key, id)) = self.shown_request.clone() {
            self.state.update(cx, |s, cx| s.reply(&key, &id, reply, cx));
        }
    }
    pub(super) fn submit_extension(&mut self, cx: &mut Context<Self>) {
        let Some((key, id)) = self.shown_request.as_ref() else {
            return;
        };
        let valid = self
            .state
            .read(cx)
            .sessions
            .get(key)
            .and_then(|s| s.pending_ui.front())
            .is_some_and(|p| {
                &p.request.id == id
                    && matches!(
                        p.request.method,
                        UiMethod::Input { .. } | UiMethod::Editor { .. }
                    )
            });
        if valid {
            self.reply_extension(
                UiReply::Value {
                    value: self.extension_input.read(cx).value().to_string(),
                },
                cx,
            );
        }
    }
    pub(super) fn render_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(session) = self.state.read(cx).current() else {
            return div().into_any_element();
        };
        let key = self.shown_key.clone().unwrap_or_default();
        let preview = self
            .views
            .get(&key)
            .and_then(|v| v.preview.as_deref())
            .is_some_and(|id| !session.history().on_current_path(id));
        let mut shell = v_flex().w_full().max_w(px(820.)).gap_2();
        let runtime_error = match session.body_state() {
            BodyState::New
            | BodyState::Ready
            | BodyState::Refreshing(_)
            | BodyState::RefreshFailed(_) => session.error.as_deref(),
            BodyState::Loading(_) | BodyState::Failed(_) => None,
        };
        if let Some(error) = runtime_error {
            shell = shell.child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().danger)
                            .child(error.to_owned()),
                    )
                    .child(
                        Button::new("retry-session")
                            .small()
                            .label(t(cx, "conversation-reconnect"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(key) = this.shown_key.clone() {
                                    this.state.update(cx, |s, cx| {
                                        s.connect(&key, cx);
                                        s.refresh(&key, cx);
                                    });
                                }
                            })),
                    ),
            );
        }
        if let Some(error) = &self.state.read(cx).storage_error {
            shell = shell.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(format!("{}: {error}", t(cx, "conversation-save-error"))),
            );
        }
        if session.interrupted {
            shell = shell.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(cx, "conversation-interrupted")),
            );
        }
        if let Some(title) = &session.extension_title {
            shell = shell.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(title.clone()),
            );
        }
        for widget in session.widgets.values().filter(|w| !w.below) {
            shell = shell.child(div().text_sm().child(widget.lines.join("\n")));
        }
        let mut editor = v_flex()
            .w_full()
            .min_w_0()
            .p_2()
            .rounded_2xl()
            .shadow_sm()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background);
        if let Some(pending) = session.pending_ui.front() {
            editor = editor.p_3().gap_2();
            let title = match &pending.request.method {
                UiMethod::Select { title, .. }
                | UiMethod::Confirm { title, .. }
                | UiMethod::Input { title, .. }
                | UiMethod::Editor { title, .. } => title.clone(),
                _ => String::new(),
            };
            editor = editor.child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title));
            match &pending.request.method {
                UiMethod::Select { options, .. } => {
                    for (i, option) in options.iter().enumerate() {
                        let option = option.clone();
                        editor = editor.child(
                            Button::new(("extension-option", i))
                                .small()
                                .label(option.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.reply_extension(
                                        UiReply::Value {
                                            value: option.clone(),
                                        },
                                        cx,
                                    )
                                })),
                        );
                    }
                }
                UiMethod::Confirm { message, .. } => {
                    editor = editor.child(div().text_sm().child(message.clone())).child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("extension-yes")
                                    .small()
                                    .label(t(cx, "action-confirm"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reply_extension(
                                            UiReply::Confirmed { confirmed: true },
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("extension-no")
                                    .small()
                                    .label(t(cx, "conversation-no"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reply_extension(
                                            UiReply::Confirmed { confirmed: false },
                                            cx,
                                        )
                                    })),
                            ),
                    );
                }
                UiMethod::Input { .. } | UiMethod::Editor { .. } => {
                    editor = editor
                        .child(Textarea::new(&self.extension_input).appearance(false))
                        .child(
                            Button::new("extension-submit")
                                .small()
                                .label(t(cx, "conversation-submit"))
                                .on_click(cx.listener(|this, _, _, cx| this.submit_extension(cx))),
                        );
                }
                _ => {}
            }
            editor =
                editor.child(
                    Button::new("extension-cancel")
                        .ghost()
                        .small()
                        .label(t(cx, "action-cancel"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.reply_extension(UiReply::cancelled(), cx)
                        })),
                );
        } else {
            editor = editor.child(
                div().key_context("GupiComposer").child(
                    Textarea::new(&self.input)
                        .appearance(false)
                        .disabled(preview)
                        .readonly(session.submitting())
                        .aria_label(t(cx, "conversation-input")),
                ),
            );
            let Some(view) = self.views.get(&key) else {
                return div().into_any_element();
            };
            let choices = div()
                .min_w_0()
                .max_w(px(340.))
                .child(view.model_picker.clone());
            let mut actions = h_flex().flex_none().items_center().gap_2();
            if session.stats.data().is_some()
                || session.stats.running()
                || session.stats.error().is_some()
            {
                actions = actions.child(metrics::context(session, cx));
            }
            if session.busy() && !session.submitting() {
                actions = actions.child(
                    Button::new("stop-generation")
                        .small()
                        .size_8()
                        .rounded_full()
                        .icon(IconName::Square)
                        .tooltip(t(cx, "conversation-stop"))
                        .accessibility_label(t(cx, "conversation-stop"))
                        .disabled(session.stopping)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(key) = this.shown_key.clone() {
                                this.state.update(cx, |s, cx| s.abort(&key, cx));
                            }
                        })),
                );
            } else {
                let label = t(
                    cx,
                    if session.submitting() {
                        "conversation-sending"
                    } else {
                        "conversation-send"
                    },
                );
                actions = actions.child(
                    Button::new("send-message")
                        .primary()
                        .small()
                        .size_8()
                        .rounded_full()
                        .icon(IconName::ArrowUp)
                        .tooltip(label.clone())
                        .accessibility_label(label)
                        .loading(session.submitting())
                        .disabled(
                            preview
                                || session.submitting()
                                || session.draft.trim().is_empty()
                                || session.command.running()
                                || session.model_change.running()
                                || session.model_change.unconfirmed(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(key) = this.shown_key.clone() {
                                this.state
                                    .update(cx, |s, cx| s.send(&key, StreamingBehavior::Steer, cx));
                            }
                        })),
                );
            }
            editor = editor.child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .pt_1()
                    .when(session.stats.data().is_some(), |row| {
                        row.child(div().flex_none().child(metrics::tokens(session, cx)))
                    })
                    .when(
                        session.stats.running() || session.stats.error().is_some(),
                        |row| {
                            row.child(metrics::status(
                                session,
                                self.state.clone(),
                                key.clone(),
                                cx,
                            ))
                        },
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(px(300.))
                            .justify_end()
                            .items_center()
                            .gap_2()
                            .child(choices)
                            .child(actions),
                    ),
            );
            if session.pending_count > 0 {
                editor = editor.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, "conversation-queued")),
                );
            }
        }
        let mut composer = v_flex().w_full().min_w_0();
        if session.info.path.as_os_str().is_empty() && session.pending_ui.is_empty() {
            let name = project_name(&session.info.cwd);
            let project_width = label_width(&name, window, cx) + px(40.);
            // Electron uses a 28px project control and 5px vertical inset.
            // Keep all three visible insets equal; the 12px overlap is separate.
            composer = composer.child(
                v_flex()
                    .mx_3()
                    .pb_3()
                    .rounded_t_xl()
                    .bg(cx.theme().muted)
                    .child(
                        h_flex().p(px(5.)).items_center().child(
                            div().w(project_width).max_w_full().h_7().child(
                                Button::new("choose-project")
                                    .ghost()
                                    .small()
                                    .h_7()
                                    .rounded_full()
                                    .icon(IconName::Folder)
                                    .label(name)
                                    .tooltip(session.info.cwd.to_string_lossy().into_owned())
                                    .accessibility_label(t(cx, "conversation-project"))
                                    .disabled(session.busy() || session.command.running())
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.pick_directory(cx)),
                                    ),
                            ),
                        ),
                    ),
            );
            editor = editor.relative().mt(-px(12.));
        }
        shell = shell.child(composer.child(editor));
        for widget in session.widgets.values().filter(|w| w.below) {
            shell = shell.child(div().text_sm().child(widget.lines.join("\n")));
        }
        if !session.statuses.is_empty() {
            shell = shell.child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .px_1()
                    .child(h_flex().min_w_0().flex_1().flex_wrap().gap_3().children(
                        session.statuses.values().map(|status| {
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(status.clone())
                        }),
                    )),
            );
        }
        div()
            .w_full()
            .flex()
            .justify_center()
            .px_5()
            .pt_2()
            .pb_4()
            .child(shell)
            .into_any_element()
    }
}

fn project_name(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

// Measure labels with the current font; each control adds its own icon/padding.
fn label_width(label: &str, window: &Window, cx: &App) -> Pixels {
    let text: SharedString = label.to_owned().into();
    window
        .text_system()
        .shape_line(
            text.clone(),
            cx.theme().font_size,
            &[TextRun {
                len: text.len(),
                font: window.text_style().font(),
                color: cx.theme().foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
        .width
}
