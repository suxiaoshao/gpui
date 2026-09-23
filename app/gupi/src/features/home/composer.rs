use super::actions::{Kind, Run};
use super::*;
use crate::features::composer::{self, Composer};
use crate::state::conversation::content::BodyState;
use gpui_kit::component::button::DropdownButton;
use gpui_kit::component::input::{InputGroupButton, Textarea};
use gpui_kit::component::menu::PopupMenuItem;
use pi_rpc::protocol::{UiMethod, UiReply};
mod metrics;
mod queue;
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
        _window: &mut Window,
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
        if !session.notices.is_empty() {
            use crate::state::notifications::Severity;
            use gpui_kit::component::collapsible::Collapsible;
            let open = self.views.get(&key).is_some_and(|v| v.notices_open);
            let toggle = key.clone();
            let clear = key.clone();
            let notices = v_flex().gap_2().children(session.notices.iter().map(|n| {
                div()
                    .text_sm()
                    .whitespace_normal()
                    .text_color(match n.severity {
                        Severity::Info => cx.theme().foreground,
                        Severity::Warning => cx.theme().warning,
                        Severity::Error => cx.theme().danger,
                    })
                    .child(n.message.clone())
            }));
            shell = shell.child(
                Collapsible::new()
                    .open(open)
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("session-notices")
                                    .small()
                                    .ghost()
                                    .icon(IconName::Bell)
                                    .label(format!(
                                        "{} ({})",
                                        t(cx, "notification-session-notices"),
                                        session.notices.len()
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(view) = this.views.get_mut(&toggle) {
                                            view.notices_open = !view.notices_open;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("clear-session-notices")
                                    .small()
                                    .ghost()
                                    .icon(IconName::X)
                                    .tooltip(t(cx, "notification-clear"))
                                    .accessibility_label(t(cx, "notification-clear"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.state.update(cx, |s, cx| s.clear_notices(&clear, cx));
                                    })),
                            ),
                    )
                    .content(notices),
            );
        }
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
                            .disabled(self.state.read(cx).temporary)
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
        if session.retry.is_some() || session.summary_retry.is_some() {
            shell = shell.child(self.progress.clone());
        }
        if session.compacting || session.command.compacting() {
            shell = shell.child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(gpui_kit::component::spinner::Spinner::new().small())
                    .child(t(cx, "conversation-compacting")),
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
        let mut editor = composer::surface(cx);
        if let Some(pending) = session.pending_ui.front() {
            editor = editor
                .p_3()
                .gap_2()
                .key_context("GupiExtension")
                .track_focus(&self.extension_focus)
                .on_action(cx.listener(|this, _: &actions::CancelExtension, _, cx| {
                    this.reply_extension(UiReply::cancelled(), cx)
                }))
                .on_action(
                    cx.listener(|this, _: &gpui_kit::component::input::Escape, _, cx| {
                        this.reply_extension(UiReply::cancelled(), cx)
                    }),
                )
                .on_action(cx.listener(|this, _: &actions::ConfirmExtension, _, cx| {
                    let reply = this
                        .state
                        .read(cx)
                        .current()
                        .and_then(|s| s.pending_ui.front())
                        .and_then(|p| match &p.request.method {
                            UiMethod::Select { options, .. } => options
                                .first()
                                .cloned()
                                .map(|value| UiReply::Value { value }),
                            UiMethod::Confirm { .. } => {
                                Some(UiReply::Confirmed { confirmed: true })
                            }
                            _ => None,
                        });
                    if let Some(reply) = reply {
                        this.reply_extension(reply, cx);
                    }
                }));
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
            let owner = cx.entity().downgrade();
            let input = Textarea::new(&self.input)
                .aria_label(t(cx, "conversation-input"))
                .on_paste(move |item, window, cx| {
                    owner
                        .update(cx, |this, cx| this.paste_attachments(item, window, cx))
                        .unwrap_or(false)
                });
            let Some(view) = self.views.get(&key) else {
                return div().into_any_element();
            };
            let mut actions = h_flex().flex_none().items_center().gap_2().child(
                InputGroupButton::new("attach-files")
                    .small()
                    .icon(IconName::Plus)
                    .tooltip(t(cx, "attachment-add"))
                    .accessibility_label(t(cx, "attachment-add"))
                    .disabled(!session.can_edit_draft() || session.attachments_read.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.choose_attachments(cx))),
            );
            if session.stats.data().is_some()
                || session.stats.running()
                || session.stats.error().is_some()
            {
                actions = actions.child(metrics::context(session, cx));
            }
            let running = session.busy() && !session.submitting();
            if running {
                actions = actions.child(
                    Button::new("stop-generation")
                        .small()
                        .icon(IconName::Square)
                        .tooltip_with_action(
                            t(cx, "conversation-stop"),
                            &Run(Kind::Stop),
                            Some("Gupi"),
                        )
                        .accessibility_label(t(cx, "conversation-stop"))
                        .disabled(session.stopping)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(key) = this.shown_key.clone() {
                                this.state.update(cx, |s, cx| s.abort(&key, cx));
                            }
                        })),
                );
            }
            if !running || !session.composer_empty() {
                let label = t(
                    cx,
                    if session.submitting() {
                        "conversation-sending"
                    } else if running {
                        "conversation-queue-steer"
                    } else {
                        "conversation-send"
                    },
                );
                let disabled = preview
                    || session.composer_empty()
                    || !self.state.read(cx).can_submit(&key, cx);
                let send = cx.listener(|this, _, _, cx| {
                    if let Some(key) = this.shown_key.clone() {
                        this.state
                            .update(cx, |s, cx| s.send(&key, StreamingBehavior::Steer, cx));
                    }
                });
                if running {
                    let state = self.state.clone();
                    let target = key.clone();
                    actions = actions.child(
                        DropdownButton::new("queue-send-mode")
                            .small()
                            .primary()
                            .disabled(disabled)
                            .button(
                                Button::new("send-message")
                                    .icon(IconName::ArrowUp)
                                    .tooltip(label.clone())
                                    .accessibility_label(label)
                                    .on_click(send),
                            )
                            .dropdown_menu(move |mut menu, _, cx| {
                                for (label, mode) in [
                                    ("conversation-queue-steer", StreamingBehavior::Steer),
                                    ("conversation-queue-follow-up", StreamingBehavior::FollowUp),
                                ] {
                                    let state = state.clone();
                                    let target = target.clone();
                                    menu = menu.item(PopupMenuItem::new(t(cx, label)).on_click(
                                        move |_, _, cx| {
                                            state.update(cx, |state, cx| {
                                                state.send(&target, mode.clone(), cx)
                                            });
                                        },
                                    ));
                                }
                                menu
                            }),
                    );
                } else {
                    actions = actions.child(
                        InputGroupButton::new("send-message")
                            .primary()
                            .small()
                            .icon(IconName::ArrowUp)
                            .tooltip(label.clone())
                            .accessibility_label(label)
                            .loading(session.submitting())
                            .disabled(disabled)
                            .on_click(send),
                    );
                }
            }
            let mut composer =
                Composer::new("conversation-composer", input, view.model_picker.clone())
                    .actions(actions);
            if let Some(attachments) = self.render_attachments(cx) {
                composer = composer.attachments(attachments);
            }
            if session.stats.data().is_some() {
                composer = composer.leading(div().flex_none().child(metrics::tokens(session, cx)));
            }
            if session.stats.running() || session.stats.error().is_some() {
                composer = composer.leading(metrics::status(
                    session,
                    self.state.clone(),
                    key.clone(),
                    cx,
                ));
            }
            editor = div()
                .relative()
                .key_context("GupiComposer")
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.attach_paths(paths.paths().to_vec(), cx)
                }))
                .child(
                    composer
                        .build()
                        .readonly(preview || !session.can_edit_draft()),
                );
        }
        if session.pending_count > 0 {
            shell = shell.child(self.render_queue(&key, preview, cx));
        }
        shell = shell.child(editor);
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
