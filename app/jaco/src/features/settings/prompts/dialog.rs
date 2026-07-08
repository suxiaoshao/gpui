use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state,
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::field as component_form_field,
    h_flex,
    input::Input,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_form::{FieldError, FormStore as _, SubmitError};
use jaco_core::PromptId;
use jaco_db::PromptRecord;

use super::super::push_settings_error;
use super::form_state::{
    PromptEditFormInput, PromptEditFormStore, PromptEditValidationContext, field_errors,
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
    form: Entity<PromptEditFormStore>,
}

enum PromptSaveError {
    Notify {
        title_key: &'static str,
        message: String,
    },
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
                .unwrap_or_default();
        let form = cx.new(|cx| {
            PromptEditFormStore::from_value_with_validation_context(
                form_input,
                validation_context,
                window,
                cx,
            )
        });

        Self {
            mode,
            prompt_id: prompt.map(|prompt| prompt.id),
            form,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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
        let result = self.form.update(cx, |form, cx| {
            form.set_validation_context(validation_context, cx);
            form.submit_sync(
                move |draft, window, cx| {
                    let result = match mode {
                        PromptEditMode::Create => {
                            state::prompts::create_prompt(cx, draft.name, draft.content)
                        }
                        PromptEditMode::Edit => {
                            let Some(prompt_id) = prompt_id.clone() else {
                                return Err(PromptSaveError::Notify {
                                    title_key: "notify-save-prompt-failed",
                                    message: "prompt id is missing".to_string(),
                                });
                            };
                            state::prompts::update_prompt(cx, &prompt_id, draft.name, draft.content)
                        }
                    };

                    result.map_err(|err| PromptSaveError::Notify {
                        title_key: "notify-save-prompt-failed",
                        message: err.to_string(),
                    })?;
                    window.push_notification(
                        Notification::new()
                            .title(cx.global::<I18n>().t("notify-prompt-saved"))
                            .with_type(NotificationType::Success),
                        cx,
                    );
                    Ok(())
                },
                window,
                cx,
            )
        });

        match result {
            Ok(()) => true,
            Err(SubmitError::Invalid(_)) | Err(SubmitError::Busy) => false,
            Err(SubmitError::Handler(PromptSaveError::Notify { title_key, message })) => {
                let title = cx.global::<I18n>().t(title_key);
                push_settings_error(window, cx, title, message);
                false
            }
        }
    }

    fn focus_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        let name_input = self.form.read(cx).name_state();
        name_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }
}

impl Render for PromptEditDialogState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name_input, content_input, name_error, content_error, name_required, content_required) = {
            let form = self.form.read(cx);
            (
                form.name_state(),
                form.content_state(),
                field_error_message(field_errors(&form.name), cx),
                field_error_message(field_errors(&form.content), cx),
                form.name_required(),
                form.content_required(),
            )
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(form_field(
                cx.global::<I18n>().t("prompt-field-name"),
                Input::new(&name_input).w_full().min_w_0(),
                name_error,
                name_required,
                cx,
            ))
            .child(form_field(
                cx.global::<I18n>().t("prompt-field-content"),
                Input::new(&content_input).w_full().min_w_0().h(px(220.)),
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
    let existing_prompts = crate::database::repository(cx)
        .list_prompts()?
        .into_iter()
        .map(|prompt| (prompt.id, prompt.name))
        .collect();
    Ok(PromptEditValidationContext {
        prompt_id,
        existing_prompts,
    })
}

fn field_error_message(errors: Vec<FieldError>, cx: &App) -> Option<SharedString> {
    errors
        .first()
        .map(|error| cx.global::<I18n>().t(error.message_key.as_ref()).into())
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

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .w(px(620.))
            .on_ok({
                let form = form.clone();
                move |_, window, cx| confirm_prompt_edit_dialog(&form, window, cx)
            })
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(Button::new("prompt-dialog-cancel").label(cancel_label.clone())),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("prompt-dialog-save")
                                .primary()
                                .icon(IconName::FilePen)
                                .label(save_label.clone()),
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

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| match state::prompts::delete_prompt(cx, &prompt_id) {
            Ok(_) => {
                window.push_notification(
                    Notification::new()
                        .title(deleted_title.clone())
                        .with_type(NotificationType::Success),
                    cx,
                );
            }
            Err(err) => {
                push_settings_error(window, cx, delete_failed_title.clone(), err);
            }
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
                .bg(cx.theme().background)
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
                .bg(cx.theme().accent.opacity(0.65))
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
    use super::super::form_state::PromptEditFormField;
    use super::{
        PromptEditMode, confirm_prompt_edit_dialog, field_errors, open_prompt_edit_dialog,
    };
    use crate::{database::FreshStoreGlobal, foundation, state};
    use gpui::{AppContext as _, Entity, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::{
        WindowExt,
        input::{InputEvent, InputState},
    };
    use tempfile::{TempDir, tempdir};

    #[gpui::test]
    fn invalid_create_confirm_keeps_prompt_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_prompt_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let form = cx
            .update(|window, cx| open_prompt_edit_dialog(PromptEditMode::Create, None, window, cx));

        let saved = cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            confirm_prompt_edit_dialog(&form, window, cx)
        });
        assert!(!saved);

        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
        });

        assert!(form.read_with(&cx, |dialog, cx| {
            let form = dialog.form.read(cx);
            !field_errors(&form.name).is_empty()
        }));
        cx.update(|_, cx| {
            assert!(
                crate::database::repository(cx)
                    .list_prompts()
                    .expect("list prompts")
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn duplicate_name_confirm_keeps_prompt_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_prompt_dialog_test(cx);
        cx.update(|cx| {
            state::prompts::create_prompt(
                cx,
                "Existing Prompt".to_string(),
                "Original content".to_string(),
            )
            .expect("create existing prompt");
        });
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let form = cx
            .update(|window, cx| open_prompt_edit_dialog(PromptEditMode::Create, None, window, cx));
        let (name_input, content_input) = form.read_with(&cx, |dialog, cx| {
            let form = dialog.form.read(cx);
            (form.name_state(), form.content_state())
        });
        set_input_value(name_input, "Existing Prompt", &mut cx);
        set_input_value(content_input, "New content", &mut cx);

        let saved = cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            confirm_prompt_edit_dialog(&form, window, cx)
        });
        assert!(!saved);

        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
        });
        assert_eq!(
            form.read_with(&cx, |dialog, cx| {
                let form = dialog.form.read(cx);
                if !field_errors(&form.name).is_empty() {
                    Some(PromptEditFormField::Name)
                } else if !field_errors(&form.content).is_empty() {
                    Some(PromptEditFormField::Content)
                } else {
                    None
                }
            }),
            Some(PromptEditFormField::Name)
        );
        assert_eq!(
            cx.update(|_, cx| crate::database::repository(cx)
                .list_prompts()
                .expect("list prompts")
                .len()),
            1
        );
    }

    fn init_prompt_dialog_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            foundation::init_i18n(cx);
            state::prompts::init(cx).expect("init prompt catalog");
        });
        dir
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<gpui_component::Root> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| TestView);
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
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
