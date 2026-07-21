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
    input::{Input, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    v_flex,
};
use gpui_form::typed::{FormFieldId as _, FormStore as _, SubmitError};
use gpui_form_gpui_component::FormInput;
use jaco_core::PromptId;
use jaco_db::PromptRecord;

use super::super::form_validation::validation_message;
use super::super::push_settings_error;
use super::form_state::{
    PromptEditFormInput, PromptEditFormInputField, PromptEditFormStore,
    PromptEditValidationContext, PromptValidationDependencies,
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
    name_input: FormInput,
    content_input: FormInput,
    _subscriptions: Vec<Subscription>,
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
                    PromptEditValidationContext::new(PromptValidationDependencies::default(), cx)
                });
        let form = cx.new(|cx| {
            PromptEditFormStore::from_value_with_validation_context(
                form_input,
                validation_context,
                cx,
            )
        });
        let name_input = FormInput::new(
            PromptEditFormStore::name_field(&form),
            |window, cx| {
                InputState::new(window, cx)
                    .placeholder(cx.global::<I18n>().t("prompt-placeholder-name"))
            },
            window,
            cx,
        )
        .expect("prompt name form entity is alive");
        let content_input = FormInput::new(
            PromptEditFormStore::content_field(&form),
            |window, cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder(cx.global::<I18n>().t("prompt-placeholder-content"))
            },
            window,
            cx,
        )
        .expect("prompt content form entity is alive");
        let form_for_locale = form.downgrade();
        let locale_subscription = cx.observe_global::<I18n>(move |_dialog, cx| {
            let form = form_for_locale.clone();
            cx.defer(move |cx| {
                let Some(form) = form.upgrade() else {
                    return;
                };
                form.update(cx, |form, cx| {
                    let context = form.validation_context().relocalized(cx);
                    form.set_validation_context(context, cx);
                });
            });
        });

        Self {
            mode,
            prompt_id: prompt.map(|prompt| prompt.id),
            form,
            name_input,
            content_input,
            _subscriptions: vec![locale_subscription],
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
        let prepared = self.form.update(cx, |form, cx| {
            form.set_validation_context(validation_context, cx);
            let revision = form.revision();
            form.prepare_submit(cx).map(|draft| (revision, draft))
        });
        let (revision, draft) = match prepared {
            Ok(prepared) => prepared,
            Err(
                SubmitError::Validation(_)
                | SubmitError::ValidationPending
                | SubmitError::Transform(_),
            ) => {
                return false;
            }
        };
        let result = match mode {
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
                    &prompt_id,
                    draft.name.clone(),
                    draft.content.clone(),
                )
            }
        };
        if let Err(error) = result {
            let title = cx.global::<I18n>().t("notify-save-prompt-failed");
            push_settings_error(window, cx, title, error.to_string());
            return false;
        }
        self.form
            .update(cx, |form, cx| form.rebase_if_revision(revision, draft, cx));
        window.push_notification(
            Notification::new()
                .title(cx.global::<I18n>().t("notify-prompt-saved"))
                .with_type(NotificationType::Success),
            cx,
        );
        true
    }

    fn focus_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.name_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }
}

impl Render for PromptEditDialogState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name_error = PromptEditFormStore::name_field(&self.form)
            .errors(cx)
            .ok()
            .and_then(|issues| issues.into_iter().next())
            .map(|issue| validation_message(&issue.message, cx));
        let content_error = PromptEditFormStore::content_field(&self.form)
            .errors(cx)
            .ok()
            .and_then(|issues| issues.into_iter().next())
            .map(|issue| validation_message(&issue.message, cx));
        let name_required = PromptEditFormInputField::Name.schema().is_required();
        let content_required = PromptEditFormInputField::Content.schema().is_required();
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
    let existing_prompts = crate::database::repository(cx)
        .list_prompts()?
        .into_iter()
        .map(|prompt| (prompt.id, prompt.name))
        .collect();
    Ok(PromptEditValidationContext::new(
        PromptValidationDependencies {
            prompt_id,
            existing_prompts,
        },
        cx,
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
    use super::super::form_state::{PromptEditFormInputField, PromptEditFormStore};
    use super::{PromptEditDialogState, PromptEditMode, confirm_prompt_edit_dialog};
    use crate::{database::FreshStoreGlobal, foundation, state};
    use gpui::{AppContext as _, Entity, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::input::{InputEvent, InputState};
    use tempfile::{TempDir, tempdir};

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
            !PromptEditFormStore::name_field(&dialog.form)
                .errors(cx)
                .unwrap_or_default()
                .is_empty()
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
        assert_eq!(
            form.read_with(&cx, |dialog, cx| {
                if !PromptEditFormStore::name_field(&dialog.form)
                    .errors(cx)
                    .unwrap_or_default()
                    .is_empty()
                {
                    Some(PromptEditFormInputField::Name)
                } else if !PromptEditFormStore::content_field(&dialog.form)
                    .errors(cx)
                    .unwrap_or_default()
                    .is_empty()
                {
                    Some(PromptEditFormInputField::Content)
                } else {
                    None
                }
            }),
            Some(PromptEditFormInputField::Name)
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

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let _ = window;
                cx.new(|_| TestView)
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
