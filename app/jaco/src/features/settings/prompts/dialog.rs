use crate::{
    components::delete_confirm::{DestructiveAction, open_async_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state,
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::field as component_form_field,
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_form::{Form, FormVersion, GardeValidator, PrepareError as SubmitError};
use gpui_form_gpui_component::FormInput;
use jaco_core::PromptId;
use jaco_db::PromptRecord;

use super::super::form_validation::{JacoGardeMessageProvider, validation_message};
use super::super::push_settings_error;
use super::form_state::{
    PromptEditFormInput, PromptEditValidationContext, PromptValidationDependencies,
    normalize_prompt_input,
};
use super::rows::prompt_updated_label;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PromptEditMode {
    Create,
    Edit,
}

impl PromptEditMode {
    fn title_key(self) -> &'static str {
        match self {
            Self::Create => "prompt-dialog-create-title",
            Self::Edit => "prompt-dialog-edit-title",
        }
    }
}

pub(super) struct PromptEditDialogState {
    mode: PromptEditMode,
    prompt_id: Option<PromptId>,
    form: Entity<Form<PromptEditFormInput>>,
    name_input: FormInput,
    content_input: FormInput,
    save_task: Option<Task<()>>,
}

impl PromptEditDialogState {
    fn new(
        mode: PromptEditMode,
        prompt: Option<PromptRecord>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = prompt
            .as_ref()
            .map(|prompt| prompt.name.clone())
            .unwrap_or_default();
        let content = prompt
            .as_ref()
            .map(|prompt| prompt.content.text.clone())
            .unwrap_or_default();
        let form_input = PromptEditFormInput::new(name, content);
        let validation_context =
            prompt_edit_validation_context(prompt.as_ref().map(|prompt| prompt.id.clone()), cx)
                .unwrap_or_else(|_| {
                    PromptEditValidationContext::new(PromptValidationDependencies::default())
                });
        let form = cx.new(|_| {
            Form::new(form_input).with_validator(GardeValidator::<
                PromptEditFormInput,
                JacoGardeMessageProvider,
            >::new(validation_context))
        });
        let name_input = FormInput::new(
            &form,
            PromptEditFormInput::NAME,
            |window, cx| {
                InputState::new(window, cx)
                    .placeholder(cx.global::<I18n>().t("prompt-placeholder-name"))
            },
            window,
            cx,
        );
        let content_input = FormInput::new(
            &form,
            PromptEditFormInput::CONTENT,
            |window, cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder(cx.global::<I18n>().t("prompt-placeholder-content"))
            },
            window,
            cx,
        );
        Self {
            mode,
            prompt_id: prompt.map(|prompt| prompt.id),
            form,
            name_input,
            content_input,
            save_task: None,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.save_task.is_some() {
            return false;
        }
        let mode = self.mode;
        let prompt_id = self.prompt_id.clone();
        let validation_context = match prompt_edit_validation_context(prompt_id.clone(), cx) {
            Ok(validation_context) => validation_context,
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-save-prompt-failed");
                push_settings_error(window, cx, title, err.to_string());
                return false;
            }
        };
        let prepared = self.form.update(cx, |form, cx| {
            form.replace_validator(
                GardeValidator::<PromptEditFormInput, JacoGardeMessageProvider>::new(
                    validation_context,
                ),
                cx,
            );
            form.prepare(cx)
        });
        let (version, draft) = match prepared {
            Ok(prepared) => prepared
                .map(|draft| normalize_prompt_input(&draft))
                .into_parts(),
            Err(SubmitError::Validation(_) | SubmitError::ValidationPending) => {
                return false;
            }
            Err(_) => return false,
        };
        let mutation = match mode {
            PromptEditMode::Create => {
                state::prompts::create_prompt(cx, draft.name.clone(), draft.content.clone())
            }
            PromptEditMode::Edit => {
                let Some(prompt_id) = prompt_id else {
                    let title = cx.global::<I18n>().t("notify-save-prompt-failed");
                    push_settings_error(window, cx, title, "prompt id is missing");
                    return false;
                };
                state::prompts::update_prompt(
                    cx,
                    prompt_id,
                    draft.name.clone(),
                    draft.content.clone(),
                )
            }
        };
        let entity = cx.entity().downgrade();
        self.save_task = Some(window.spawn(cx, async move |cx| {
            let result = mutation.await;
            let _ = entity.update_in(cx, |dialog, window, cx| {
                dialog.save_task = None;
                match result {
                    Ok(_) => {
                        dialog.finish_successful_save(version, draft, window, cx);
                    }
                    Err(error) => {
                        let title = cx.global::<I18n>().t("notify-save-prompt-failed");
                        push_settings_error(window, cx, title, error.to_string());
                        cx.notify();
                    }
                }
            });
        }));
        cx.notify();
        false
    }

    fn is_saving(&self) -> bool {
        self.save_task.is_some()
    }

    fn finish_successful_save(
        &mut self,
        version: FormVersion,
        draft: PromptEditFormInput,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let rebased = self.rebase_saved_prompt_if_current(version, draft, cx);
        window.push_notification(
            Notification::new()
                .title(cx.global::<I18n>().t("notify-prompt-saved"))
                .with_type(NotificationType::Success),
            cx,
        );
        if rebased {
            window.close_dialog(cx);
        } else {
            cx.notify();
        }
        rebased
    }

    fn rebase_saved_prompt_if_current(
        &mut self,
        version: FormVersion,
        draft: PromptEditFormInput,
        cx: &mut Context<Self>,
    ) -> bool {
        self.form
            .update(cx, |form, cx| form.rebase_if_current(version, draft, cx))
    }

    fn focus_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.name_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }
}

impl Render for PromptEditDialogState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name_error = PromptEditFormInput::NAME
            .errors(&self.form, cx)
            .into_iter()
            .next()
            .map(|issue| validation_message(issue.message(), cx));
        let content_error = PromptEditFormInput::CONTENT
            .errors(&self.form, cx)
            .into_iter()
            .next()
            .map(|issue| validation_message(issue.message(), cx));
        let name_required = PromptEditFormInput::NAME.schema().is_required();
        let content_required = PromptEditFormInput::CONTENT.schema().is_required();
        v_flex()
            .w_full()
            .gap_4()
            .child(form_field(
                cx.global::<I18n>().t("prompt-field-name"),
                Input::new(&self.name_input).w_full().min_w_0(),
                name_error,
                name_required,
                cx,
            ))
            .child(form_field(
                cx.global::<I18n>().t("prompt-field-content"),
                Input::new(&self.content_input)
                    .w_full()
                    .min_w_0()
                    .h(px(220.)),
                content_error,
                content_required,
                cx,
            ))
    }
}
fn prompt_edit_validation_context(
    prompt_id: Option<PromptId>,
    cx: &App,
) -> jaco_db::Result<PromptEditValidationContext> {
    let existing_prompts = state::prompts::list_prompts(cx)?
        .into_iter()
        .map(|prompt| (prompt.id, prompt.name))
        .collect();
    Ok(PromptEditValidationContext::new(
        PromptValidationDependencies {
            prompt_id,
            existing_prompts,
        },
    ))
}

pub(super) fn open_prompt_edit_dialog(
    mode: PromptEditMode,
    prompt: Option<PromptRecord>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<PromptEditDialogState> {
    let title = cx.global::<I18n>().t(mode.title_key());
    let cancel_label = cx.global::<I18n>().t("button-cancel");
    let save_label = cx.global::<I18n>().t("provider-action-save");
    let form = cx.new(|cx| PromptEditDialogState::new(mode, prompt, window, cx));
    let form_to_focus = form.clone();
    let form_to_return = form.clone();

    window.open_dialog(cx, move |dialog, _window, cx| {
        let mutable = prompt_resource_is_ready(cx);
        let saving = form.read(cx).is_saving();
        dialog
            .title(title.clone())
            .close_button(false)
            .on_cancel({
                let form = form.clone();
                move |_, _window, cx| !form.read(cx).is_saving()
            })
            .w(px(620.))
            .on_ok({
                let form = form.clone();
                move |_, window, cx| {
                    prompt_resource_is_ready(cx) && confirm_prompt_edit_dialog(&form, window, cx)
                }
            })
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("prompt-dialog-cancel")
                                .label(cancel_label.clone())
                                .disabled(saving),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("prompt-dialog-save")
                                .primary()
                                .icon(IconName::FilePen)
                                .label(save_label.clone())
                                .loading(saving)
                                .disabled(!mutable || saving),
                        ),
                    ),
            )
    });

    window.defer(cx, move |window, cx| {
        form_to_focus.update(cx, |form, cx| form.focus_name(window, cx));
    });

    form_to_return
}

fn confirm_prompt_edit_dialog(
    form: &Entity<PromptEditDialogState>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| form.save(window, cx))
}

pub(super) fn open_prompt_preview_dialog(prompt: PromptRecord, window: &mut Window, cx: &mut App) {
    let title = cx.global::<I18n>().t("prompt-dialog-view-title");
    let edit_label = cx.global::<I18n>().t("button-edit");
    let delete_label = cx.global::<I18n>().t("button-delete");
    let close_label = cx.global::<I18n>().t("button-cancel");

    window.open_dialog(cx, move |dialog, _window, cx| {
        let mutable = prompt_resource_is_ready(cx);
        let read_only = cx.global::<I18n>().t("resource-picker-read-only");
        let edit_tooltip = if mutable {
            edit_label.clone()
        } else {
            read_only.clone()
        };
        let delete_tooltip = if mutable {
            delete_label.clone()
        } else {
            read_only
        };
        dialog
            .title(title.clone())
            .w(px(680.))
            .child(render_prompt_preview(prompt.clone(), cx))
            .footer(
                DialogFooter::new()
                    .child(
                        DialogAction::new().child(
                            Button::new("prompt-dialog-edit")
                                .icon(IconName::Pencil)
                                .label(edit_label.clone())
                                .disabled(!mutable)
                                .tooltip(edit_tooltip)
                                .on_click({
                                    let prompt = prompt.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        open_prompt_edit_dialog(
                                            PromptEditMode::Edit,
                                            Some(prompt.clone()),
                                            window,
                                            cx,
                                        );
                                    }
                                }),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("prompt-dialog-delete")
                                .danger()
                                .icon(IconName::Trash)
                                .label(delete_label.clone())
                                .disabled(!mutable)
                                .tooltip(delete_tooltip)
                                .on_click({
                                    let prompt = prompt.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        open_prompt_delete_confirm(prompt.clone(), window, cx);
                                    }
                                }),
                        ),
                    )
                    .child(
                        DialogClose::new()
                            .child(Button::new("prompt-dialog-close").label(close_label.clone())),
                    ),
            )
    });
}

fn prompt_resource_is_ready(cx: &App) -> bool {
    crate::app::critical_resources_ready(cx)
        && state::prompts::catalog(cx).read(cx, |operation| {
            matches!(operation, state::prompts::PromptOperation::Ready(_))
        })
}

pub(super) fn open_prompt_delete_confirm(prompt: PromptRecord, window: &mut Window, cx: &mut App) {
    let mut args = FluentArgs::new();
    args.set("name", prompt.name.clone());
    let title = cx.global::<I18n>().t("prompt-delete-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("prompt-delete-message", &args);
    let deleted_title = cx.global::<I18n>().t("notify-prompt-deleted");
    let delete_failed_title = cx.global::<I18n>().t("notify-delete-prompt-failed");
    let prompt_id = prompt.id.clone();

    open_async_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| {
            let mutation = state::prompts::delete_prompt(cx, prompt_id.clone());
            let deleted_title = deleted_title.clone();
            let delete_failed_title = delete_failed_title.clone();
            window.spawn(cx, async move |cx| {
                let result = mutation.await;
                let succeeded = result.is_ok();
                let _ = cx.update(|window, cx| match result {
                    Ok(_) => {
                        window.push_notification(
                            Notification::new()
                                .title(deleted_title)
                                .with_type(NotificationType::Success),
                            cx,
                        );
                    }
                    Err(err) => {
                        push_settings_error(window, cx, delete_failed_title, err);
                    }
                });
                succeeded
            })
        },
        window,
        cx,
    );
}

fn render_prompt_preview(prompt: PromptRecord, cx: &mut App) -> AnyElement {
    let updated_label = prompt_updated_label(prompt.updated_at);

    v_flex()
        .w_full()
        .gap_4()
        .child(render_prompt_preview_header(&prompt, updated_label, cx))
        .child(
            div()
                .max_h(px(380.))
                .overflow_y_scrollbar()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().tokens.background.background)
                .p_3()
                .child(
                    div()
                        .text_sm()
                        .line_height(relative(1.45))
                        .child(prompt.content.text),
                ),
        )
        .into_any_element()
}

fn render_prompt_preview_header(
    prompt: &PromptRecord,
    updated_label: String,
    cx: &mut App,
) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_3()
        .child(
            div()
                .flex()
                .size_10()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(cx.theme().tokens.accent.background.opacity(0.65))
                .child(Icon::new(IconName::FilePen).text_color(cx.theme().accent_foreground)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    Label::new(prompt.name.clone())
                        .text_lg()
                        .font_medium()
                        .truncate(),
                )
                .child(
                    Label::new(updated_label)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .into_any_element()
}

fn form_field(
    label: impl Into<SharedString>,
    input: impl IntoElement,
    error: Option<SharedString>,
    required: bool,
    cx: &mut App,
) -> AnyElement {
    component_form_field()
        .label(label.into())
        .required(required)
        .child(
            v_flex()
                .w_full()
                .gap_2()
                .child(input)
                .when_some(error, |this, error| {
                    this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::super::form_state::PromptEditFormInput;
    use super::{
        PromptEditDialogState, PromptEditMode, confirm_prompt_edit_dialog, validation_message,
    };
    use crate::{database, foundation, state};
    use gpui::{AppContext as _, Entity, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::input::{InputEvent, InputState};
    use tempfile::{TempDir, tempdir};
    use tokio::sync::oneshot;

    #[gpui::test]
    fn invalid_create_confirm_keeps_prompt_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_prompt_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let form = cx.update(|window, cx| {
            cx.new(|cx| PromptEditDialogState::new(PromptEditMode::Create, None, window, cx))
        });

        let saved = cx.update(|window, cx| confirm_prompt_edit_dialog(&form, window, cx));
        assert!(!saved);

        assert!(form.read_with(&cx, |dialog, cx| {
            !PromptEditFormInput::NAME
                .errors(&dialog.form, cx)
                .is_empty()
        }));
        cx.update(|_, cx| {
            assert!(
                crate::database::with_ready_repository(cx, |repo| repo.list_prompts())
                    .expect("list prompts")
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn stale_save_completion_keeps_newer_prompt_edit(cx: &mut TestAppContext) {
        let _dir = init_prompt_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let dialog = cx.update(|window, cx| {
            cx.new(|cx| PromptEditDialogState::new(PromptEditMode::Create, None, window, cx))
        });

        let (version, saved) = cx.update(|_, cx| {
            let form = dialog.read(cx).form.clone();
            PromptEditFormInput::NAME.set(&form, "Saved name".to_string(), cx);
            PromptEditFormInput::CONTENT.set(&form, "Saved content".to_string(), cx);
            form.update(cx, |form, cx| form.prepare(cx))
                .expect("valid prompt snapshot")
                .into_parts()
        });
        cx.update(|_, cx| {
            let form = dialog.read(cx).form.clone();
            PromptEditFormInput::CONTENT.set(&form, "Newer content".to_string(), cx);
        });

        let rebased = cx.update(|_, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.rebase_saved_prompt_if_current(version, saved, cx)
            })
        });

        assert!(!rebased);
        assert_eq!(
            dialog.read_with(&cx, |dialog, cx| {
                PromptEditFormInput::CONTENT.get(&dialog.form, cx)
            }),
            "Newer content"
        );
        assert!(dialog.read_with(&cx, |dialog, cx| dialog.form.read(cx).is_dirty()));
    }

    #[gpui::test]
    fn duplicate_name_confirm_keeps_prompt_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_prompt_dialog_test(cx);
        cx.update(|cx| cx.set_global(foundation::I18n::for_locale_tag("en-US")));
        let task = cx.update(|cx| {
            state::prompts::create_prompt(
                cx,
                "Existing Prompt".to_string(),
                "Original content".to_string(),
            )
        });
        cx.foreground_executor()
            .block_test(task)
            .expect("create existing prompt");
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let form = cx.update(|window, cx| {
            cx.new(|cx| PromptEditDialogState::new(PromptEditMode::Create, None, window, cx))
        });
        let (name_input, content_input) = form.read_with(&cx, |dialog, _cx| {
            (
                (*dialog.name_input).clone(),
                (*dialog.content_input).clone(),
            )
        });
        set_input_value(name_input, "Existing Prompt", &mut cx);
        set_input_value(content_input, "New content", &mut cx);

        let saved = cx.update(|window, cx| confirm_prompt_edit_dialog(&form, window, cx));
        assert!(!saved);
        let (report_before_locale_change, revision_before_locale_change, english_error) = form
            .read_with(&cx, |dialog, cx| {
                let form = dialog.form.read(cx);
                let report = form.validation_report();
                let error = report.issues().first().expect("duplicate prompt error");
                (
                    report.clone(),
                    form.revision(),
                    validation_message(error.message(), cx),
                )
            });
        cx.update(|_, cx| cx.set_global(foundation::I18n::for_locale_tag("zh-CN")));
        let (report_after_locale_change, revision_after_locale_change, chinese_error) = form
            .read_with(&cx, |dialog, cx| {
                let form = dialog.form.read(cx);
                let report = form.validation_report();
                let error = report.issues().first().expect("duplicate prompt error");
                (
                    report.clone(),
                    form.revision(),
                    validation_message(error.message(), cx),
                )
            });
        assert_eq!(report_after_locale_change, report_before_locale_change);
        assert_eq!(revision_after_locale_change, revision_before_locale_change);
        assert_ne!(chinese_error, english_error);
        assert_eq!(
            form.read_with(&cx, |dialog, cx| {
                if !PromptEditFormInput::NAME
                    .errors(&dialog.form, cx)
                    .is_empty()
                {
                    Some("name")
                } else if !PromptEditFormInput::CONTENT
                    .errors(&dialog.form, cx)
                    .is_empty()
                {
                    Some("content")
                } else {
                    None
                }
            }),
            Some("name")
        );
        assert_eq!(
            cx.update(|_, cx| {
                crate::database::with_ready_repository(cx, |repo| repo.list_prompts())
                    .expect("list prompts")
                    .len()
            }),
            1
        );
    }

    #[gpui::test]
    fn pending_save_rejects_repeated_prompt_confirm(cx: &mut TestAppContext) {
        let _dir = init_prompt_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = cx.update(|window, cx| {
            cx.new(|cx| PromptEditDialogState::new(PromptEditMode::Create, None, window, cx))
        });
        let (_sender, receiver) = oneshot::channel::<()>();

        cx.update(|window, cx| {
            let task = window.spawn(cx, async move |_| {
                let _ = receiver.await;
            });
            form.update(cx, |dialog, _| dialog.save_task = Some(task));
        });

        assert!(form.read_with(&cx, |dialog, _| dialog.is_saving()));
        let saved = cx.update(|window, cx| confirm_prompt_edit_dialog(&form, window, cx));
        assert!(!saved);
        assert!(form.read_with(&cx, |dialog, _| dialog.is_saving()));
    }

    fn init_prompt_dialog_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            database::install_for_test(cx, dir.path());
            foundation::init_i18n(cx);
            state::hotkey::set_test_hotkey_state(cx);
            state::prompts::init(cx);
        });
        cx.run_until_parked();
        dir
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |_window, cx| cx.new(|_| TestView))
                .expect("open prompt dialog test window")
        })
    }

    fn set_input_value(
        input: Entity<InputState>,
        value: impl Into<String>,
        cx: &mut VisualTestContext,
    ) {
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value(value.into(), window, cx);
                cx.emit(InputEvent::Change);
            });
        });
    }

    struct TestView;

    impl Render for TestView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }
}
