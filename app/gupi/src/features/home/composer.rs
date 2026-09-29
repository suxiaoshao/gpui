use super::actions::{Kind, Run};
use super::*;
use crate::features::composer::Composer;
use crate::state::conversation::content::BodyState;
use gpui_kit::component::button::DropdownButton;
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants as _};
use gpui_kit::component::input::{InputGroupButton, InputToken, Textarea};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
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
                    value: if matches!(
                        self.state
                            .read(cx)
                            .current()
                            .and_then(|s| s.pending_ui.front())
                            .map(|p| &p.request.method),
                        Some(UiMethod::Input { .. })
                    ) {
                        self.extension_line.read(cx).value().to_string()
                    } else {
                        self.extension_input.read(cx).value().to_string()
                    },
                },
                cx,
            );
        }
    }
    fn extension_cancel(&self, cx: &Context<Self>) -> Button {
        Button::new("extension-cancel")
            .ghost()
            .label(t(cx, "action-cancel"))
            .on_click(cx.listener(|this, _, _, cx| this.reply_extension(UiReply::cancelled(), cx)))
    }

    fn extension_submit(&self, cx: &Context<Self>) -> Button {
        Button::new("extension-submit")
            .primary()
            .ml_auto()
            .label(t(cx, "conversation-submit"))
            .on_click(cx.listener(|this, _, _, cx| this.submit_extension(cx)))
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
        let mut editor = v_flex().w_full().min_w_0();
        if let Some(pending) = session.pending_ui.front() {
            editor = editor
                .gap_4()
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
                .on_action(
                    cx.listener(|this, _: &actions::ConfirmExtension, window, cx| {
                        if let Some(selection) = this
                            .state
                            .read(cx)
                            .current()
                            .and_then(|s| s.pending_ui.front())
                            .and_then(|p| p.selection.clone())
                        {
                            selection.update(cx, |state, cx| {
                                state.submit(window, cx);
                            });
                            return;
                        }
                        let reply = this
                            .state
                            .read(cx)
                            .current()
                            .and_then(|s| s.pending_ui.front())
                            .and_then(|p| match &p.request.method {
                                UiMethod::Confirm { .. } => {
                                    Some(UiReply::Confirmed { confirmed: true })
                                }
                                _ => None,
                            });
                        if let Some(reply) = reply {
                            this.reply_extension(reply, cx);
                        }
                    }),
                );
            let title = match &pending.request.method {
                UiMethod::Select { title, .. }
                | UiMethod::Confirm { title, .. }
                | UiMethod::Input { title, .. }
                | UiMethod::Editor { title, .. } => title.clone(),
                _ => String::new(),
            };

            match &pending.request.method {
                UiMethod::Select { options, .. } => {
                    use gpui_kit::component::questionnaire::{
                        Questionnaire, QuestionnaireActions, QuestionnaireChoice,
                        QuestionnaireChoices, QuestionnaireError, QuestionnaireItem,
                        QuestionnaireSubmit, QuestionnaireTitle,
                    };
                    if let Some(selection) = &pending.selection {
                        editor = editor.child(
                            Questionnaire::new(selection).child(
                                QuestionnaireItem::new(selection, "answer")
                                    .child(QuestionnaireTitle::new(selection, "answer"))
                                    .child(
                                        div()
                                            .id("extension-choices")
                                            .test_support()
                                            // Keep outward focus rings inside the scroll clip;
                                            // matching margins preserve the title alignment.
                                            .m(rems(-0.25))
                                            .p_1()
                                            .max_h(rems(16.))
                                            .overflow_y_scroll()
                                            .child(
                                                QuestionnaireChoices::new(selection, "answer")
                                                    .children(options.iter().enumerate().map(
                                                        |(ix, _)| {
                                                            QuestionnaireChoice::new(
                                                                selection,
                                                                "answer",
                                                                ix.to_string(),
                                                            )
                                                            .min_w_0()
                                                            .content_style(
                                                                StyleRefinement::default()
                                                                    .min_w_0()
                                                                    .whitespace_normal(),
                                                            )
                                                        },
                                                    )),
                                            ),
                                    )
                                    .child(
                                        QuestionnaireError::new(selection, "answer")
                                            .child(t(cx, "resource-answer-required")),
                                    )
                                    .child(
                                        QuestionnaireActions::new(selection)
                                            .child(self.extension_cancel(cx))
                                            .child(QuestionnaireSubmit::new(selection).ml_auto()),
                                    ),
                            ),
                        );
                    }
                }
                UiMethod::Input { .. } => {
                    use gpui_kit::component::questionnaire::{
                        Questionnaire, QuestionnaireActions, QuestionnaireInput, QuestionnaireItem,
                        QuestionnaireSubmit, QuestionnaireTitle,
                    };
                    if let Some(questionnaire) = &self.input_questionnaire {
                        let mut actions = QuestionnaireActions::new(questionnaire)
                            .child(self.extension_cancel(cx));
                        if self.extension_line.read(cx).value().trim().is_empty() {
                            // Questionnaire treats blank text as unanswered; Pi
                            // explicitly permits it. Keep that one submit path raw.
                            actions = actions.child(self.extension_submit(cx));
                        } else {
                            actions =
                                actions.child(QuestionnaireSubmit::new(questionnaire).ml_auto());
                        }
                        editor = editor.child(
                            Questionnaire::new(questionnaire).child(
                                QuestionnaireItem::new(questionnaire, "answer")
                                    .child(QuestionnaireTitle::new(questionnaire, "answer"))
                                    .child(QuestionnaireInput::new(questionnaire, "answer"))
                                    .child(actions),
                            ),
                        );
                    }
                }
                UiMethod::Confirm { message, .. } => {
                    use gpui_kit::component::form::{field, v_form};
                    editor = editor
                        .child(v_form().child(field().label(title).description(message.clone())))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(self.extension_cancel(cx))
                                .child(
                                    Button::new("extension-no")
                                        .ml_auto()
                                        .label(t(cx, "conversation-no"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.reply_extension(
                                                UiReply::Confirmed { confirmed: false },
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    Button::new("extension-yes")
                                        .primary()
                                        .label(t(cx, "action-confirm"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.reply_extension(
                                                UiReply::Confirmed { confirmed: true },
                                                cx,
                                            )
                                        })),
                                ),
                        );
                }
                UiMethod::Editor { .. } => {
                    use gpui_kit::component::form::{field, v_form};
                    editor = editor
                        .child(
                            v_form().child(
                                field()
                                    .label(title.clone())
                                    .child(Textarea::new(&self.extension_input).aria_label(title)),
                            ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(self.extension_cancel(cx))
                                .child(self.extension_submit(cx)),
                        );
                }
                _ => {}
            }
        } else {
            let owner = cx.entity().downgrade();
            let input = Textarea::new(&self.input)
                .aria_label(t(cx, "conversation-input"))
                .token(|context, _, _| {
                    InputToken::new(context).icon(if context.token().id().starts_with("skill:") {
                        IconName::BookOpen
                    } else {
                        IconName::FileText
                    })
                })
                .on_token_click(cx.listener(
                    |this,
                     event: &gpui_kit::component::input::InlineTokenClickEvent,
                     window,
                     cx| {
                        if let Some(path) =
                            crate::foundation::composer_resources::file_path(event.token())
                        {
                            cx.open_with_system(path);
                            return;
                        }
                        let command = this
                            .state
                            .read(cx)
                            .current()
                            .and_then(|s| s.commands.data())
                            .and_then(|commands| {
                                crate::foundation::composer_resources::command(
                                    event.token().text(),
                                    commands,
                                )
                            })
                            .cloned();
                        if let Some(command) = command {
                            window.open_dialog(cx, move |dialog, _, cx| {
                                let path = command
                                    .source_info
                                    .get("path")
                                    .and_then(|v| v.as_str())
                                    .map(PathBuf::from);
                                dialog
                                    .title(format!("/{}", command.name))
                                    .child(
                                        div()
                                            .child(command.description.clone().unwrap_or_default()),
                                    )
                                    .when_some(path, |dialog, path| {
                                        dialog.child(
                                            Button::new("open-resource-file")
                                                .small()
                                                .label(t(cx, "resource-open-file"))
                                                .on_click(move |_, _, cx| {
                                                    cx.open_with_system(&path)
                                                }),
                                        )
                                    })
                            });
                        }
                    },
                ))
                .on_paste(move |item, window, cx| {
                    owner
                        .update(cx, |this, cx| this.paste_attachments(item, window, cx))
                        .unwrap_or(false)
                });
            let Some(view) = self.views.get(&key) else {
                return div().into_any_element();
            };
            let file_owner = cx.weak_entity();
            let mut actions = h_flex().flex_none().items_center().gap_2().child(
                InputGroupButton::new("attach-files")
                    .small()
                    .icon(IconName::Plus)
                    .tooltip(t(cx, "attachment-add"))
                    .accessibility_label(t(cx, "attachment-add"))
                    .disabled(!session.can_edit_draft() || session.attachments_read.is_some())
                    .dropdown_menu(move |menu, _, cx| {
                        let search = file_owner.clone();
                        let choose = file_owner.clone();
                        menu.item(PopupMenuItem::new(t(cx, "resource-search-files")).on_click(
                            move |_, window, cx| {
                                let search = search.clone();
                                window.defer(cx, move |window, cx| {
                                    let _ = search
                                        .update(cx, |home, cx| home.open_files(None, window, cx));
                                });
                            },
                        ))
                        .item(
                            PopupMenuItem::new(t(cx, "resource-choose-files")).on_click(
                                move |_, window, cx| {
                                    let _ = choose
                                        .update(cx, |home, cx| home.choose_attachments(window, cx));
                                },
                            ),
                        )
                    }),
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
                // Undo may restore the original @ query. Keep editing focus
                // in the composer while the input performs its own history edit.
                .capture_action(
                    cx.listener(|this, _: &gpui_kit::component::input::Undo, _, _| {
                        this.slash.dismiss_file_query();
                    }),
                )
                .capture_action(
                    cx.listener(|this, _: &gpui_kit::component::input::Redo, _, _| {
                        this.slash.dismiss_file_query();
                    }),
                )
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    this.attach_paths(paths.paths().to_vec(), window, cx)
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
        shell = if session.pending_ui.is_empty() {
            shell.child(editor)
        } else {
            shell.child(
                GroupBox::new()
                    .id("extension-questionnaire")
                    .outline()
                    .min_w_0()
                    .child(editor),
            )
        };
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
