use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state::{self, providers::ProviderModelChoice, shortcuts::ShortcutDraft},
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::field as component_form_field,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    select::Select,
    switch::Switch,
    v_flex,
};
use gpui_form::{FormStore as _, SubmitError};
use jaco_core::{ShortcutId, ShortcutInputSource};
use jaco_db::ShortcutRecord;
use std::rc::Rc;

use super::super::push_settings_error;
use super::{
    choices::{InputSourceChoice, PromptChoice},
    form_state::{
        ShortcutEditFormInput, ShortcutEditFormStore, ShortcutEditValidationContext, field_errors,
    },
    rows::{ShortcutManagementRow, input_source_label},
};

#[cfg(test)]
use super::validation::ShortcutValidationError;

type ShortcutRecordDialogHandler = Rc<dyn Fn(ShortcutRecord, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShortcutEditMode {
    Create,
    Edit,
}

impl ShortcutEditMode {
    fn title_key(self) -> &'static str {
        match self {
            Self::Create => "dialog-add-shortcut-title",
            Self::Edit => "dialog-edit-shortcut-title",
        }
    }
}

pub(super) struct ShortcutEditDialogState {
    mode: ShortcutEditMode,
    shortcut_id: Option<ShortcutId>,
    form: Entity<ShortcutEditFormStore>,
    existing_shortcuts: Vec<ShortcutRecord>,
    temporary_hotkey: Option<String>,
}

enum ShortcutSaveError {
    Notify {
        title_key: &'static str,
        message: String,
    },
}

pub(super) struct ShortcutDialogChoices {
    pub(super) prompts: Vec<PromptChoice>,
    pub(super) models: Vec<ProviderModelChoice>,
}

impl ShortcutEditDialogState {
    fn new(
        mode: ShortcutEditMode,
        shortcut: Option<ShortcutRecord>,
        choices: ShortcutDialogChoices,
        existing_shortcuts: Vec<ShortcutRecord>,
        temporary_hotkey: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let shortcut_id = shortcut.as_ref().map(|shortcut| shortcut.id.clone());
        let form_input =
            ShortcutEditFormInput::new(shortcut.as_ref(), choices.prompts, choices.models);
        let validation_context = ShortcutEditValidationContext {
            shortcut_id: shortcut_id.clone(),
            existing_shortcuts: existing_shortcuts.clone(),
            temporary_hotkey: temporary_hotkey.clone(),
        };
        let form = cx.new(|cx| {
            ShortcutEditFormStore::from_value_with_validation_context(
                form_input,
                validation_context,
                window,
                cx,
            )
        });

        Self {
            mode,
            shortcut_id,
            form,
            existing_shortcuts,
            temporary_hotkey,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mode = self.mode;
        let shortcut_id = self.shortcut_id.clone();
        let existing_shortcuts = self.existing_shortcuts.clone();
        let temporary_hotkey = self.temporary_hotkey.clone();
        let validation_context = ShortcutEditValidationContext {
            shortcut_id: shortcut_id.clone(),
            existing_shortcuts,
            temporary_hotkey,
        };
        let result = self.form.update(cx, |form, cx| {
            form.set_validation_context(validation_context, cx);
            form.submit_sync(
                move |draft, window, cx| {
                    let Some(hotkey) = draft.hotkey else {
                        return Err(ShortcutSaveError::Notify {
                            title_key: "notify-save-shortcut-failed",
                            message: "validated shortcut hotkey is missing".to_string(),
                        });
                    };
                    let prompt_id = draft.prompt.selected;
                    let Some(model_key) = draft.model.selected else {
                        return Err(ShortcutSaveError::Notify {
                            title_key: "notify-save-shortcut-failed",
                            message: "validated shortcut model is missing".to_string(),
                        });
                    };
                    let shortcut_draft = ShortcutDraft {
                        hotkey,
                        enabled: draft.enabled,
                        prompt_id,
                        provider_id: model_key.provider_id,
                        model_id: model_key.model_id,
                        input_source: draft.input_source,
                    };
                    let result = match mode {
                        ShortcutEditMode::Create => {
                            state::shortcuts::create_shortcut(cx, shortcut_draft)
                        }
                        ShortcutEditMode::Edit => {
                            let Some(shortcut_id) = shortcut_id.as_ref() else {
                                return Err(ShortcutSaveError::Notify {
                                    title_key: "notify-save-shortcut-failed",
                                    message: "shortcut id is missing".to_string(),
                                });
                            };
                            state::shortcuts::update_shortcut(cx, shortcut_id, shortcut_draft)
                        }
                    };

                    result.map_err(|err| ShortcutSaveError::Notify {
                        title_key: "notify-save-shortcut-failed",
                        message: err.to_string(),
                    })?;
                    window.push_notification(
                        Notification::new()
                            .title(cx.global::<I18n>().t(match mode {
                                ShortcutEditMode::Create => "notify-shortcut-created",
                                ShortcutEditMode::Edit => "notify-shortcut-updated",
                            }))
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
            Err(SubmitError::Handler(ShortcutSaveError::Notify { title_key, message })) => {
                let title = cx.global::<I18n>().t(title_key);
                push_settings_error(window, cx, title, message);
                false
            }
        }
    }

    fn focus_hotkey(&self, window: &mut Window, cx: &mut Context<Self>) {
        let hotkey_input = self.form.read(cx).hotkey_state();
        hotkey_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }

    fn render_input_source_toggle(
        &self,
        input_source: ShortcutInputSource,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        ToggleGroup::new("shortcut-dialog-input-source")
            .segmented()
            .outline()
            .w_full()
            .children(input_source_choices(cx).into_iter().map(|choice| {
                Toggle::new(input_source_toggle_id(choice.value()))
                    .label(choice.label())
                    .checked(input_source == choice.value())
                    .flex_1()
                    .h(px(40.))
            }))
            .on_click(cx.listener(|this, states: &Vec<bool>, window, cx| {
                let current = this.form.read(cx).input_source_value();
                let input_source = input_source_from_toggle_states(current, states);
                this.form.update(cx, |form, cx| {
                    form.set_input_source_value(
                        input_source,
                        gpui_form::FieldChangeCause::UserInput,
                        window,
                        cx,
                    );
                });
            }))
            .into_any_element()
    }
}

impl Render for ShortcutEditDialogState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (field_hotkey, field_prompt, field_model, field_input_source, field_enabled) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("shortcut-field-hotkey"),
                i18n.t("shortcut-field-prompt"),
                i18n.t("shortcut-field-model"),
                i18n.t("shortcut-field-input-source"),
                i18n.t("shortcut-field-enabled"),
            )
        };
        let (
            hotkey_input,
            prompt_select,
            model_select,
            hotkey_error,
            model_error,
            hotkey_required,
            model_required,
            input_source,
            enabled,
        ) = {
            let form = self.form.read(cx);
            (
                form.hotkey_state(),
                form.prompt_state().read(cx).select.clone(),
                form.model_state().read(cx).select.clone(),
                field_error_message(field_errors(&form.hotkey), cx),
                field_error_message(field_errors(&form.model), cx),
                form.hotkey_required(),
                form.model_required(),
                form.input_source_value(),
                form.enabled_value(),
            )
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(form_field(
                field_hotkey,
                hotkey_input.into_any_element(),
                hotkey_error,
                hotkey_required,
                cx,
            ))
            .child(form_field(
                field_prompt.clone(),
                Select::new(&prompt_select)
                    .placeholder(field_prompt)
                    .w_full()
                    .into_any_element(),
                None,
                false,
                cx,
            ))
            .child(form_field(
                field_model.clone(),
                Select::new(&model_select)
                    .placeholder(field_model)
                    .search_placeholder(cx.global::<I18n>().t("chat-form-model-search-placeholder"))
                    .menu_max_h(rems(18.))
                    .w_full()
                    .into_any_element(),
                model_error,
                model_required,
                cx,
            ))
            .child(form_field(
                field_input_source,
                self.render_input_source_toggle(input_source, cx),
                None,
                false,
                cx,
            ))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(Label::new(field_enabled).text_sm().font_medium())
                    .child(
                        Switch::new("shortcut-dialog-enabled")
                            .checked(enabled)
                            .on_click(cx.listener(|this, checked, window, cx| {
                                this.form.update(cx, |form, cx| {
                                    form.set_enabled_value(
                                        *checked,
                                        gpui_form::FieldChangeCause::UserInput,
                                        window,
                                        cx,
                                    );
                                });
                            })),
                    ),
            )
    }
}

pub(super) fn open_shortcut_edit_dialog(
    mode: ShortcutEditMode,
    shortcut: Option<ShortcutRecord>,
    choices: ShortcutDialogChoices,
    existing_shortcuts: Vec<ShortcutRecord>,
    temporary_hotkey: Option<String>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ShortcutEditDialogState> {
    let title = cx.global::<I18n>().t(mode.title_key());
    let cancel_label = cx.global::<I18n>().t("button-cancel");
    let save_label = cx.global::<I18n>().t("provider-action-save");
    let form = cx.new(|cx| {
        ShortcutEditDialogState::new(
            mode,
            shortcut,
            choices,
            existing_shortcuts,
            temporary_hotkey,
            window,
            cx,
        )
    });
    let form_to_focus = form.clone();
    let form_to_return = form.clone();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .w(px(640.))
            .on_ok({
                let form = form.clone();
                move |_, window, cx| confirm_shortcut_edit_dialog(&form, window, cx)
            })
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("shortcut-dialog-cancel").label(cancel_label.clone()),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("shortcut-dialog-save")
                                .primary()
                                .icon(IconName::Keyboard)
                                .label(save_label.clone()),
                        ),
                    ),
            )
    });

    window.defer(cx, move |window, cx| {
        form_to_focus.update(cx, |form, cx| form.focus_hotkey(window, cx));
    });

    form_to_return
}

fn confirm_shortcut_edit_dialog(
    form: &Entity<ShortcutEditDialogState>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    form.update(cx, |form, cx| form.save(window, cx))
}

pub(super) fn open_shortcut_preview_dialog(
    shortcut: ShortcutRecord,
    row: ShortcutManagementRow,
    window: &mut Window,
    cx: &mut App,
    on_edit: ShortcutRecordDialogHandler,
    on_delete: ShortcutRecordDialogHandler,
) {
    let title = cx.global::<I18n>().t("dialog-view-shortcut-title");
    let edit_label = cx.global::<I18n>().t("button-edit");
    let reregister_label = cx.global::<I18n>().t("shortcut-action-reregister");
    let delete_label = cx.global::<I18n>().t("button-delete");
    let close_label = cx.global::<I18n>().t("button-cancel");
    let shortcut_id = shortcut.id.clone();
    let on_edit_handler = on_edit.clone();
    let on_delete_handler = on_delete.clone();

    window.open_dialog(cx, move |dialog, _window, cx| {
        dialog
            .title(title.clone())
            .w(px(680.))
            .child(render_shortcut_preview(row.clone(), cx))
            .footer(
                DialogFooter::new()
                    .child(
                        DialogAction::new().child(
                            Button::new("shortcut-dialog-edit")
                                .icon(IconName::Pencil)
                                .label(edit_label.clone())
                                .on_click({
                                    let shortcut = shortcut.clone();
                                    let on_edit = on_edit_handler.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        on_edit(shortcut.clone(), window, cx);
                                    }
                                }),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("shortcut-dialog-reregister")
                                .icon(IconName::RefreshCcw)
                                .label(reregister_label.clone())
                                .on_click({
                                    let shortcut_id = shortcut_id.clone();
                                    move |_, window, cx| {
                                        match state::shortcuts::reregister_shortcut(cx, &shortcut_id) {
                                            Ok(_) => {
                                                window.push_notification(
                                                    Notification::new()
                                                        .title(cx.global::<I18n>().t("notify-shortcut-reregistered"))
                                                        .with_type(NotificationType::Success),
                                                    cx,
                                                );
                                            }
                                            Err(err) => {
                                                let title = cx.global::<I18n>().t("notify-shortcut-register-failed");
                                                push_settings_error(window, cx, title, err);
                                            }
                                        }
                                    }
                                }),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("shortcut-dialog-delete")
                                .danger()
                                .icon(IconName::Trash)
                                .label(delete_label.clone())
                                .on_click({
                                    let shortcut = shortcut.clone();
                                    let on_delete = on_delete_handler.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        on_delete(shortcut.clone(), window, cx);
                                    }
                                }),
                        ),
                    )
                    .child(
                        DialogClose::new().child(
                            Button::new("shortcut-dialog-close").label(close_label.clone()),
                        ),
                    ),
            )
    });
}

pub(super) fn open_shortcut_delete_confirm(
    shortcut: ShortcutRecord,
    window: &mut Window,
    cx: &mut App,
) {
    let mut args = FluentArgs::new();
    args.set("hotkey", shortcut.hotkey.clone());
    let title = cx.global::<I18n>().t("dialog-delete-shortcut-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("dialog-delete-shortcut-message", &args);
    let deleted_title = cx.global::<I18n>().t("notify-shortcut-deleted");
    let delete_failed_title = cx.global::<I18n>().t("notify-delete-shortcut-failed");
    let shortcut_id = shortcut.id.clone();

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| match state::shortcuts::delete_shortcut(cx, &shortcut_id) {
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

fn render_shortcut_preview(row: ShortcutManagementRow, cx: &mut App) -> AnyElement {
    let i18n = cx.global::<I18n>();
    v_flex()
        .w_full()
        .gap_2()
        .child(detail_row(
            i18n.t("shortcut-field-hotkey"),
            row.hotkey_label,
        ))
        .child(detail_row(
            i18n.t("shortcut-field-prompt"),
            row.prompt_label,
        ))
        .child(detail_row(
            i18n.t("shortcut-field-model"),
            format!("{} / {}", row.provider_label, row.model_label),
        ))
        .child(detail_row(
            i18n.t("shortcut-field-input-source"),
            row.input_source_label,
        ))
        .child(detail_row(
            i18n.t("shortcut-field-action"),
            row.action_label,
        ))
        .child(detail_row(
            i18n.t("shortcut-field-enabled"),
            row.status_label,
        ))
        .child(detail_row(
            i18n.t("shortcut-field-updated"),
            row.updated_label,
        ))
        .max_h(px(420.))
        .overflow_y_scrollbar()
        .into_any_element()
}

fn detail_row(label: impl Into<SharedString>, value: impl Into<SharedString>) -> AnyElement {
    h_flex()
        .w_full()
        .min_w_0()
        .items_start()
        .gap_3()
        .child(
            Label::new(label.into())
                .w(px(150.))
                .flex_none()
                .text_sm()
                .font_medium(),
        )
        .child(Label::new(value.into()).flex_1().min_w_0().text_sm())
        .into_any_element()
}

fn form_field(
    label: impl Into<SharedString>,
    input: AnyElement,
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

fn field_error_message(errors: Vec<gpui_form::FieldError>, cx: &App) -> Option<SharedString> {
    errors
        .first()
        .map(|error| cx.global::<I18n>().t(error.message_key.as_ref()).into())
}

#[cfg(test)]
fn validation_message(error: ShortcutValidationError, cx: &App) -> SharedString {
    cx.global::<I18n>().t(error.i18n_key()).into()
}

fn input_source_choices(cx: &App) -> Vec<InputSourceChoice> {
    vec![
        InputSourceChoice::new(
            ShortcutInputSource::SelectionOrClipboard,
            input_source_label(
                ShortcutInputSource::SelectionOrClipboard,
                cx.global::<I18n>(),
            ),
        ),
        InputSourceChoice::new(
            ShortcutInputSource::Screenshot,
            input_source_label(ShortcutInputSource::Screenshot, cx.global::<I18n>()),
        ),
    ]
}

fn input_source_toggle_id(source: ShortcutInputSource) -> &'static str {
    match source {
        ShortcutInputSource::SelectionOrClipboard => "shortcut-dialog-input-source-selection",
        ShortcutInputSource::Screenshot => "shortcut-dialog-input-source-screenshot",
    }
}

fn input_source_from_toggle_states(
    current: ShortcutInputSource,
    states: &[bool],
) -> ShortcutInputSource {
    const SOURCES: [ShortcutInputSource; 2] = [
        ShortcutInputSource::SelectionOrClipboard,
        ShortcutInputSource::Screenshot,
    ];

    for (ix, source) in SOURCES.into_iter().enumerate() {
        if source != current && states.get(ix).copied().unwrap_or(false) {
            return source;
        }
    }

    for (ix, source) in SOURCES.into_iter().enumerate() {
        if states.get(ix).copied().unwrap_or(false) {
            return source;
        }
    }

    current
}

#[cfg(test)]
mod tests {
    use super::{
        ShortcutDialogChoices, ShortcutEditMode, ShortcutValidationError,
        confirm_shortcut_edit_dialog, field_error_message, field_errors,
        input_source_from_toggle_states, open_shortcut_edit_dialog, validation_message,
    };
    use crate::{
        database::FreshStoreGlobal, foundation, state, state::providers::ProviderModelChoice,
    };
    use gpui::{AppContext as _, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::{IndexPath, WindowExt};
    use jaco_core::conservative_model_capabilities;
    use tempfile::{TempDir, tempdir};

    #[gpui::test]
    fn missing_hotkey_confirm_keeps_shortcut_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_shortcut_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        let expected_error = cx.update(|_, cx| {
            validation_message(ShortcutValidationError::HotkeyRequired, cx).to_string()
        });
        let form = cx.update(|window, cx| {
            open_shortcut_edit_dialog(
                ShortcutEditMode::Create,
                None,
                ShortcutDialogChoices {
                    prompts: Vec::new(),
                    models: Vec::new(),
                },
                Vec::new(),
                None,
                window,
                cx,
            )
        });
        let saved = cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            confirm_shortcut_edit_dialog(&form, window, cx)
        });
        assert!(!saved);

        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
        });

        assert_eq!(
            form.read_with(&cx, |dialog, cx| {
                let form = dialog.form.read(cx);
                field_error_message(field_errors(&form.hotkey), cx)
                    .map(|message| message.to_string())
            }),
            Some(expected_error)
        );
        cx.update(|_, cx| {
            assert!(
                crate::database::repository(cx)
                    .list_shortcuts()
                    .expect("list shortcuts")
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn model_select_updates_shortcut_dialog_selection(cx: &mut TestAppContext) {
        let _dir = init_shortcut_dialog_test(cx);
        let window = open_test_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let models = vec![
            model_choice("provider-1", "openai", "OpenAI", "gpt-5"),
            model_choice("provider-2", "ollama", "Ollama", "llama3.2"),
        ];
        let expected_key = models[1].key();
        let form = cx.update(|window, cx| {
            open_shortcut_edit_dialog(
                ShortcutEditMode::Create,
                None,
                ShortcutDialogChoices {
                    prompts: Vec::new(),
                    models,
                },
                Vec::new(),
                None,
                window,
                cx,
            )
        });

        let model_select = form.read_with(&cx, |dialog, cx| {
            dialog.form.read(cx).model_state().read(cx).select.clone()
        });
        cx.update(|window, cx| {
            model_select.update(cx, |select, cx| {
                select.set_selected_index(Some(IndexPath::default().section(1).row(0)), window, cx);
            });
        });

        assert_eq!(
            model_select.read_with(&cx, |select, _| select.selected_value().cloned()),
            Some(expected_key)
        );
    }

    #[test]
    fn input_source_toggle_states_keep_single_selection() {
        assert_eq!(
            input_source_from_toggle_states(
                jaco_core::ShortcutInputSource::SelectionOrClipboard,
                &[true, true],
            ),
            jaco_core::ShortcutInputSource::Screenshot
        );
        assert_eq!(
            input_source_from_toggle_states(
                jaco_core::ShortcutInputSource::Screenshot,
                &[true, true],
            ),
            jaco_core::ShortcutInputSource::SelectionOrClipboard
        );
        assert_eq!(
            input_source_from_toggle_states(
                jaco_core::ShortcutInputSource::Screenshot,
                &[false, false],
            ),
            jaco_core::ShortcutInputSource::Screenshot
        );
    }

    fn init_shortcut_dialog_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            foundation::init_i18n(cx);
            state::shortcuts::init(cx);
        });
        dir
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<gpui_component::Root> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| TestView);
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .expect("open shortcut dialog test window")
        })
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

    fn model_choice(
        provider_id: &str,
        provider_kind: &str,
        provider_display_name: &str,
        model_id: &str,
    ) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider_id.to_string(),
            provider_kind: provider_kind.to_string(),
            provider_display_name: provider_display_name.to_string(),
            model_id: model_id.to_string(),
            model_display_name: None,
            capabilities: conservative_model_capabilities(provider_kind),
        }
    }
}
