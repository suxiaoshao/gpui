use super::*;
use gpui_kit::component::dialog::{Cancel, Confirm, DialogButtonProps};

impl ResourcesView {
    fn show_editor_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, cx| {
            let save = owner.clone();
            let cancel = owner.clone();
            let close = owner.clone();
            let Some(entity) = owner.upgrade() else {
                return dialog;
            };
            entity.update(cx, |this, cx| {
                let Some(editor) = &this.editor else {
                    return dialog;
                };
                let busy = this.controller.read(cx).busy() || this.config.read(cx).busy(cx);
                let dirty = editor.form.read(cx).is_dirty();
                let path = editor.path.display().to_string();
                dialog
                    .title(
                        editor
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    )
                    .width(px(720.).min(window.viewport_size().width - px(32.)))
                    .overlay_closable(false)
                    .close_button(!busy)
                    .child(
                        v_flex()
                            .gap_3()
                            .debug_selector(|| "resource-editor-dialog".into())
                            .child(
                                div()
                                    .id("editor-path")
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(path.clone())
                                    .tooltip(move |window, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(path.clone())
                                            .build(window, cx)
                                    }),
                            )
                            .child(
                                CodeEditor::new(&editor.input)
                                    .h(px(400.).min(window.viewport_size().height * 0.5))
                                    .readonly(!editor.editable)
                                    .disabled(busy),
                            ),
                    )
                    .footer(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("resource-editor-cancel")
                                    .label(t(cx, "action-cancel"))
                                    .disabled(busy)
                                    .debug_selector(|| "resource-editor-cancel".into())
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(Cancel), cx)
                                    }),
                            )
                            .child(
                                Button::new("resource-editor-save")
                                    .primary()
                                    .label(t(cx, "settings-resource-save"))
                                    .loading(this.controller.read(cx).mutation.is_running())
                                    .disabled(
                                        busy || !editor.editable || (!editor.create && !dirty),
                                    )
                                    .debug_selector(|| "resource-editor-save".into())
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(
                                            Box::new(Confirm { secondary: false }),
                                            cx,
                                        )
                                    }),
                            ),
                    )
                    .on_ok(move |_, _, cx| {
                        let _ = save.update(cx, |this, cx| this.save_editor(cx));
                        // The successful write closes the dialog; an error leaves the draft visible.
                        false
                    })
                    .on_cancel(move |_, window, cx| {
                        cancel
                            .update(cx, |this, cx| this.cancel_editor(window, cx))
                            .unwrap_or(true)
                    })
                    .on_close(move |_, _, cx| {
                        let _ = close.update(cx, |this, cx| {
                            this.editor = None;
                            cx.notify();
                        });
                    })
            })
        });
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy() || self.config.read(cx).busy(cx) {
            return;
        }
        let Some(editor) = &self.editor else { return };
        if !editor.editable || (!editor.create && !editor.form.read(cx).is_dirty()) {
            return;
        }
        let path = editor.path.clone();
        let create = editor.create;
        if let Ok(prepared) = editor.form.update(cx, |form, cx| form.prepare(cx)) {
            self.change(
                Change::Save {
                    path,
                    text: prepared.value().text.clone(),
                    create,
                },
                cx,
            );
        }
    }

    fn cancel_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.controller.read(cx).busy() || self.config.read(cx).busy(cx) {
            return false;
        }
        if !self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.form.read(cx).is_dirty())
        {
            return true;
        }
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = owner.clone();
            dialog
                .title(t(cx, "settings-editor-discard"))
                .button_props(DialogButtonProps::default().show_cancel(true))
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.editor = None;
                        cx.notify();
                    });
                    // Pop the confirmation, then let its normal close remove the editor beneath it.
                    window.close_dialog(cx);
                    true
                })
        });
        false
    }

    pub(super) fn request_open(&mut self, next: Open, window: &mut Window, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy()
            || self.config.read(cx).busy(cx)
            || self.open.is_running()
            || self.editor.is_some()
        {
            return;
        }
        self.creating = None;
        self.error = None;
        if next.create {
            self.open = refresh::Operation::new();
            let text = if next.kind == Kind::Skill {
                format!(
                    "---\nname: {}\ndescription: \n---\n\n",
                    next.path
                        .parent()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy()
                )
            } else {
                String::new()
            };
            self.editor = Some(Self::editor(next, text, window, cx));
            self.show_editor_dialog(window, cx);
            cx.notify();
            return;
        }
        let worker = cx.background_spawn({
            let path = next.path.clone();
            async move {
                match std::fs::read_to_string(&path) {
                    Ok(text) => Ok(text),
                    Err(e)
                        if e.kind() == std::io::ErrorKind::NotFound
                            && path
                                .file_name()
                                .is_some_and(|n| n == "SYSTEM.md" || n == "APPEND_SYSTEM.md") =>
                    {
                        Ok(String::new())
                    }
                    Err(e) => Err(io::Error::from(e)),
                }
            }
        });
        let task = cx.spawn_in(window, async move |owner, cx| {
            let result = worker.await;
            let _ = owner.update_in(cx, |this, window, cx| {
                match result {
                    Ok(text) => {
                        this.editor = Some(Self::editor(next, text, window, cx));
                        this.open.transition(Complete(Ok(())));
                        this.show_editor_dialog(window, cx);
                    }
                    Err(error) => {
                        this.open.transition(Complete(Err(error)));
                    }
                }
                cx.notify();
            });
        });
        match &self.open {
            refresh::Operation::Idle(_) => self.open.transition(Load(task)),
            refresh::Operation::Unavailable(_) => self.open.transition(Retry(task)),
            _ => self.open.transition(Refresh(task)),
        }
        cx.notify();
    }

    fn editor(next: Open, text: String, window: &mut Window, cx: &mut Context<Self>) -> Editor {
        let form = cx.new(|_| Form::new(TextDraft { text: text.clone() }));
        let input = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .default_value(text)
        });
        let (binding, writer) = TextDraft::TEXT.bind_control_in(
            &form,
            &input,
            |input, projection, window, cx| {
                if let ControlProjection::Value(text) = projection {
                    input.set_value(text, window, cx);
                }
            },
            window,
            cx,
        );
        let input_sub = cx.subscribe_in(&input, window, move |_, input, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                writer.defer_set(input.read(cx).value().to_string(), window, cx);
            }
        });
        let form_sub = cx.observe(&form, |_, _, cx| cx.notify());
        Editor {
            path: next.path,
            editable: next.editable,
            create: next.create,
            form,
            input,
            _binding: binding,
            _subscriptions: vec![input_sub, form_sub],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Open, ResourcesView, TextDraft};
    use crate::{
        foundation::pi_resources::Kind,
        pi::PiProbeController,
        state::config::{AppConfig, AppLanguage, ConfigController},
    };
    use gpui_form::Form;
    use gpui_kit::component::{
        Root, WindowExt,
        dialog::{Cancel, Confirm},
    };
    use gpui_kit::{
        AppContext, Context, Entity, Focusable, IntoElement, Modifiers, ParentElement, Render,
        Styled, Subscription, TestAppContext, Window, div, px, size,
    };

    struct Fixture {
        _resources: Entity<ResourcesView>,
        _subscription: Subscription,
    }
    impl Render for Fixture {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(div().child("Settings remain behind the editor"))
                .children(Root::render_dialog_layer(window, cx))
        }
    }

    #[gpui_kit::test]
    async fn file_dialog_prefills_original_text_and_cancel_does_not_write(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
        });
        let mut owner = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let form = cx.new(|_| Form::new(AppConfig::default()));
            let config = cx.new(|cx| ConfigController::new(&form, cx));
            let pi = cx.new(|_| PiProbeController::new());
            let resources = cx.new(|cx| ResourcesView::new(config, pi, window, cx));
            owner = Some(resources.clone());
            let fixture = cx.new(|cx| Fixture {
                _subscription: cx.observe(&resources, |_, _, cx| cx.notify()),
                _resources: resources,
            });
            Root::new(fixture, window, cx)
        });
        let owner = owner.unwrap();
        cx.simulate_resize(size(px(900.), px(700.)));
        let dir = tempfile::tempdir().unwrap();
        for (name, kind) in [
            ("SKILL.md", Kind::Skill),
            ("review.md", Kind::Prompt),
            ("SYSTEM.md", Kind::Prompt),
            ("APPEND_SYSTEM.md", Kind::Prompt),
        ] {
            let path = dir.path().join(name);
            let original = format!("---\ndescription: Original\n---\n# {name}\n\n原始正文与 $@\n");
            std::fs::write(&path, &original).unwrap();
            cx.update(|window, cx| {
                owner.update(cx, |view, cx| {
                    view.request_open(
                        Open {
                            path: path.clone(),
                            kind,
                            editable: true,
                            create: false,
                        },
                        window,
                        cx,
                    )
                })
            });
            cx.condition(&owner, |view, _| view.editor.is_some()).await;
            // Check the initial native control value, not just the form or a later projection.
            cx.update(|window, cx| {
                let editor = owner.read(cx).editor.as_ref().unwrap();
                assert_eq!(editor.input.read(cx).value().as_ref(), original);
                assert_eq!(TextDraft::TEXT.get(&editor.form, cx), original);
                assert!(!editor.form.read(cx).is_dirty());
                assert!(window.has_active_dialog(cx));
                window.draw(cx).clear(cx);
            });
            assert!(cx.debug_bounds("resource-editor-dialog").is_some());
            let cancel = cx.debug_bounds("resource-editor-cancel").unwrap();
            cx.simulate_click(cancel.center(), Modifiers::default());
            cx.update(|window, cx| {
                assert!(owner.read(cx).editor.is_none());
                assert!(!window.has_active_dialog(cx));
            });
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
        let path = dir.path().join("review.md");
        let original = std::fs::read_to_string(&path).unwrap();
        cx.update(|window, cx| {
            owner.update(cx, |view, cx| {
                view.request_open(
                    Open {
                        path: path.clone(),
                        kind: Kind::Prompt,
                        editable: true,
                        create: false,
                    },
                    window,
                    cx,
                )
            })
        });
        cx.condition(&owner, |view, _| view.editor.is_some()).await;
        let input = cx.update(|_, cx| owner.read(cx).editor.as_ref().unwrap().input.clone());
        cx.update(|window, cx| {
            window.activate_window();
            input.focus_handle(cx).focus(window, cx);
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        });
        cx.simulate_input("Changed draft");
        cx.run_until_parked();
        cx.update(|window, cx| {
            let form = owner.read(cx).editor.as_ref().unwrap().form.clone();
            assert_eq!(
                form.update(cx, |form, cx| form.prepare(cx))
                    .unwrap()
                    .value()
                    .text,
                "Changed draft"
            );
            window.draw(cx).clear(cx);
        });
        let cancel = cx.debug_bounds("resource-editor-cancel").unwrap();
        cx.simulate_click(cancel.center(), Modifiers::default());
        cx.update(|window, cx| {
            assert!(owner.read(cx).editor.is_some());
            window.draw(cx).clear(cx);
            // Cancel the discard confirmation: retain the editor and its changed content.
            window.dispatch_action(Box::new(Cancel), cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert_eq!(input.read(cx).value().as_ref(), "Changed draft");
            assert!(window.has_active_dialog(cx));
            window.draw(cx).clear(cx);
        });
        let cancel = cx.debug_bounds("resource-editor-cancel").unwrap();
        cx.simulate_click(cancel.center(), Modifiers::default());
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_action(Box::new(Confirm { secondary: false }), cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(owner.read(cx).editor.is_none());
            assert!(!window.has_active_dialog(cx));
        });
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
}
