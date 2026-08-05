use crate::{
    components::chat::form::{
        AddAttachmentControl, AttachmentControlState, ChatForm, ChatFormControls, ChatFormState,
        ControlSlot, PrimaryActionControlState, RunSettingsControls,
    },
    components::chat::input::ComposerEditor,
    components::chat::run_settings::{
        RunSettingsController, RunSettingsInput, RunSettingsSubmitError, resolve_run_settings,
    },
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    components::hotkey_input::{
        HotkeyInput, HotkeyInputEvent, HotkeyInputState, string_to_keystroke,
    },
    foundation::{I18n, assets::IconName},
    state::{self, shortcuts::ShortcutDraft},
};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Disableable, IndexPath, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    form::field as component_form_field,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectState},
    switch::Switch,
    v_flex,
};
use gpui_form::typed::{Form, FormEvent, GardeValidator};
use jaco_core::{ShortcutId, ShortcutInputSource};
use jaco_db::ShortcutRecord;
use std::rc::Rc;

use super::super::form_validation::JacoGardeMessageProvider;
use super::super::push_settings_error;
use super::{
    choices::{InputSourceChoice, PromptChoice},
    form_state::{
        ShortcutEditFormInput, ShortcutEditValidationContext, ShortcutValidationDependencies,
        normalize_shortcut_input,
    },
    rows::{ShortcutManagementRow, input_source_label},
};

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
    form: Entity<Form<ShortcutEditFormInput>>,
    hotkey_input: Entity<HotkeyInputState>,
    prompt_select: Entity<SelectState<Vec<PromptChoice>>>,
    _subscriptions: Vec<Subscription>,
    _control_leases: [gpui_form::ControlLease; 2],
    _run_settings: Entity<RunSettingsController<ShortcutEditFormInput>>,
    chat_form: Entity<ChatFormState>,
    chat_form_controls: ChatFormControls,
    existing_shortcuts: Vec<ShortcutRecord>,
    temporary_hotkey: Option<String>,
    save_task: Option<Task<()>>,
}

pub(super) struct ShortcutDialogChoices {
    pub(super) prompts: Vec<PromptChoice>,
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
        let prompt_choices = choices.prompts;
        let form_input = ShortcutEditFormInput::new(shortcut.as_ref());
        let validation_context =
            ShortcutEditValidationContext::new(ShortcutValidationDependencies {
                shortcut_id: shortcut_id.clone(),
                existing_shortcuts: existing_shortcuts.clone(),
                temporary_hotkey: temporary_hotkey.clone(),
            });
        let selected_prompt = form_input.prompt.clone();
        let selected_prompt_index = prompt_choices
            .iter()
            .position(|choice| choice.value() == &selected_prompt)
            .map(IndexPath::new);
        let form = cx.new(|_| {
            Form::try_new_with_validator(
                form_input,
                GardeValidator::<ShortcutEditFormInput, JacoGardeMessageProvider>::new(
                    validation_context,
                ),
            )
            .expect("build shortcut edit form")
        });
        let hotkey_input = cx.new(|cx| HotkeyInputState::new(window, cx));
        let prompt_select =
            cx.new(|cx| SelectState::new(prompt_choices, selected_prompt_index, window, cx));
        let mut subscriptions = Vec::new();
        let prompt_binding = ShortcutEditFormInput::PROMPT.bind_control(&form, cx);
        let prompt_lease = prompt_binding.lease();
        subscriptions.push(cx.subscribe_in(
            &prompt_select,
            window,
            move |_owner, _, event: &SelectEvent<Vec<PromptChoice>>, window, cx| {
                let SelectEvent::Confirm(value) = event;
                prompt_binding.defer_set(value.clone().flatten(), window, cx);
            },
        ));
        let weak_form = form.downgrade();
        let weak_prompt_select = prompt_select.downgrade();
        subscriptions.push(cx.subscribe_in(
            &form,
            window,
            move |_owner, _, _: &FormEvent, window, cx| {
                let (Some(form), Some(prompt_select)) =
                    (weak_form.upgrade(), weak_prompt_select.upgrade())
                else {
                    return;
                };
                let selected = ShortcutEditFormInput::PROMPT.value(&form, cx);
                prompt_select.update(cx, |state, cx| {
                    state.set_selected_value(&selected, window, cx);
                });
            },
        ));
        let hotkey_field = ShortcutEditFormInput::HOTKEY;
        let hotkey_binding = hotkey_field.bind_control(&form, cx);
        let hotkey_lease = hotkey_binding.lease();
        subscriptions.push(cx.subscribe_in(
            &hotkey_input,
            window,
            move |_owner, _state, event: &HotkeyInputEvent, window, cx| {
                let HotkeyInputEvent::Change(value) = event;
                hotkey_binding.defer_set(value.clone(), window, cx);
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &form,
            window,
            |_owner, _, _: &FormEvent, _window, cx| cx.notify(),
        ));
        let run_settings_field =
            ShortcutEditFormInput::ROOT.then(ShortcutEditFormInput::RUN_SETTINGS);
        let run_settings =
            cx.new(|cx| RunSettingsController::new(form.clone(), run_settings_field, window, cx));
        let run_settings_states = run_settings.read(cx).control_states();
        let placeholder = cx.global::<I18n>().t("chat-form-placeholder");
        let composer = cx.new(|cx| ComposerEditor::new(placeholder, window, cx));
        let attachments = cx.new(|_| AttachmentControlState::default());
        let primary_action = cx.new(|_| PrimaryActionControlState::default());
        let chat_form_controls = ChatFormControls {
            project: ControlSlot::Hidden,
            composer: ControlSlot::Disabled(composer),
            attachments: ControlSlot::Disabled(attachments),
            add_attachment: ControlSlot::Disabled(AddAttachmentControl),
            run_settings: RunSettingsControls {
                model: ControlSlot::Enabled(run_settings_states.model),
                reasoning: ControlSlot::Enabled(run_settings_states.reasoning),
                approval: ControlSlot::Enabled(run_settings_states.approval),
            },
            primary_action: ControlSlot::Disabled(primary_action),
        };
        let chat_form = cx.new(|cx| ChatFormState::new(&chat_form_controls, cx));

        Self {
            mode,
            shortcut_id,
            form,
            hotkey_input,
            prompt_select,
            _subscriptions: subscriptions,
            _control_leases: [prompt_lease, hotkey_lease],
            _run_settings: run_settings,
            chat_form,
            chat_form_controls,
            existing_shortcuts,
            temporary_hotkey,
            save_task: None,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.save_task.is_some() {
            return false;
        }
        let mode = self.mode;
        let shortcut_id = self.shortcut_id.clone();
        let validation_context =
            ShortcutEditValidationContext::new(ShortcutValidationDependencies {
                shortcut_id: shortcut_id.clone(),
                existing_shortcuts: self.existing_shortcuts.clone(),
                temporary_hotkey: self.temporary_hotkey.clone(),
            });
        let result = self.form.update(cx, |form, cx| {
            form.replace_validator(
                GardeValidator::<ShortcutEditFormInput, JacoGardeMessageProvider>::new(
                    validation_context,
                ),
                cx,
            );
            form.prepare(cx)
        });
        let Ok(prepared) = result else {
            return false;
        };
        let (revision, draft) = prepared
            .map(|draft| normalize_shortcut_input(&draft))
            .into_parts();
        let Some(hotkey) = draft.hotkey.clone() else {
            return false;
        };
        let Some(catalog) = cx
            .has_global::<state::providers::ProviderStore>()
            .then(|| state::providers::catalog(cx))
        else {
            let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
            push_settings_error(
                window,
                cx,
                title,
                "provider catalog is unavailable".to_string(),
            );
            return false;
        };
        let choices = catalog.read(cx, |operation| {
            operation
                .data()
                .map(|data| data.enabled_models.clone())
                .unwrap_or_default()
        });
        let resolved = match resolve_run_settings(&draft.run_settings, &Ok(choices)) {
            Ok(resolved) => resolved,
            Err(error) => {
                let message = match error {
                    RunSettingsSubmitError::CatalogUnavailable => {
                        "provider catalog is unavailable".to_string()
                    }
                    RunSettingsSubmitError::ModelRequired => {
                        "validated shortcut model is missing".to_string()
                    }
                    RunSettingsSubmitError::ModelUnavailable(key) => {
                        format!("selected model is unavailable: {key:?}")
                    }
                    RunSettingsSubmitError::ReasoningUnsupported(selection) => {
                        format!("selected reasoning mode is unsupported: {selection:?}")
                    }
                    RunSettingsSubmitError::TokenBudgetInvalid(value) => {
                        format!("selected token budget is outside model limits: {value}")
                    }
                };
                let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
                push_settings_error(window, cx, title, message);
                return false;
            }
        };
        let shortcut_draft = ShortcutDraft {
            hotkey,
            enabled: draft.enabled,
            prompt_id: draft.prompt.clone(),
            provider_id: resolved.provider_model.provider_id,
            model_id: resolved.provider_model.model_id,
            input_source: draft.input_source,
            reasoning_selection: resolved.reasoning_selection,
            approval_mode: resolved.approval_mode,
        };
        let persisted = match mode {
            ShortcutEditMode::Create => state::shortcuts::create_shortcut(cx, shortcut_draft),
            ShortcutEditMode::Edit => {
                let Some(id) = shortcut_id else {
                    let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
                    push_settings_error(window, cx, title, "shortcut id is missing".to_string());
                    return false;
                };
                state::shortcuts::update_shortcut(cx, id, shortcut_draft)
            }
        };
        let entity = cx.entity().downgrade();
        self.save_task = Some(window.spawn(cx, async move |cx| {
            let result = persisted.await;
            let _ = entity.update_in(cx, |dialog, window, cx| {
                dialog.save_task = None;
                match result {
                    Ok(_) => {
                        dialog
                            .form
                            .update(cx, |form, cx| form.rebase_if_revision(revision, draft, cx));
                        window.push_notification(
                            Notification::new()
                                .title(cx.global::<I18n>().t(match mode {
                                    ShortcutEditMode::Create => "notify-shortcut-created",
                                    ShortcutEditMode::Edit => "notify-shortcut-updated",
                                }))
                                .with_type(NotificationType::Success),
                            cx,
                        );
                        window.close_dialog(cx);
                    }
                    Err(error) => {
                        let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
                        push_settings_error(window, cx, title, error.to_string());
                        cx.notify();
                    }
                }
            });
        }));
        false
    }

    fn focus_hotkey(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_input.update(cx, |input, cx| {
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
            .on_click(cx.listener(|this, states: &Vec<bool>, _window, cx| {
                let current = ShortcutEditFormInput::INPUT_SOURCE.value(&this.form, cx);
                let input_source = input_source_from_toggle_states(current, states);
                ShortcutEditFormInput::INPUT_SOURCE.set(&this.form, input_source, cx);
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
        let hotkey_field = ShortcutEditFormInput::HOTKEY;
        let model_field = ShortcutEditFormInput::ROOT
            .then(ShortcutEditFormInput::RUN_SETTINGS)
            .then(RunSettingsInput::MODEL);
        let hotkey_error = field_error_message(hotkey_field.errors(&self.form, cx), cx);
        let model_error = field_error_message(model_field.errors(&self.form, cx), cx);
        let (hotkey, prompt_select, input_source, enabled) = {
            let form = self.form.read(cx);
            (
                form.value().hotkey.clone(),
                self.prompt_select.clone(),
                form.value().input_source,
                form.value().enabled,
            )
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(form_field(
                field_hotkey,
                HotkeyInput::new("shortcut-dialog-hotkey", &self.hotkey_input)
                    .w_full()
                    .value(hotkey.as_deref().and_then(string_to_keystroke))
                    .into_any_element(),
                hotkey_error,
                true,
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
                ChatForm::new(&self.chat_form, self.chat_form_controls.clone()).into_any_element(),
                model_error,
                true,
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
                            .on_click(cx.listener(|this, checked, _window, cx| {
                                ShortcutEditFormInput::ENABLED.set(&this.form, *checked, cx);
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

    window.open_dialog(cx, move |dialog, _window, cx| {
        let editing_ready = shortcut_editing_ready(cx);
        dialog
            .title(title.clone())
            .w(px(640.))
            .on_ok({
                let form = form.clone();
                move |_, window, cx| {
                    shortcut_editing_ready(cx) && confirm_shortcut_edit_dialog(&form, window, cx)
                }
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
                                .label(save_label.clone())
                                .disabled(!editing_ready),
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
        let editing_ready = shortcut_editing_ready(cx);
        let mutation_ready = shortcut_mutation_ready(cx);
        let read_only = cx.global::<I18n>().t("resource-picker-read-only");
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
                                .disabled(!editing_ready)
                                .tooltip(if editing_ready {
                                    edit_label.clone()
                                } else {
                                    read_only.clone()
                                })
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
                                .disabled(!mutation_ready)
                                .tooltip(if mutation_ready {
                                    reregister_label.clone()
                                } else {
                                    read_only.clone()
                                })
                                .on_click({
                                    let shortcut_id = shortcut_id.clone();
                                    move |_, window, cx| {
                                        match state::shortcuts::reregister_shortcut(cx, shortcut_id.clone()) {
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
                                .disabled(!mutation_ready)
                                .tooltip(if mutation_ready {
                                    delete_label.clone()
                                } else {
                                    read_only.clone()
                                })
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

fn shortcut_mutation_ready(cx: &App) -> bool {
    crate::app::critical_resources_ready(cx)
        && state::shortcuts::catalog(cx).read(cx, |operation| {
            matches!(operation, state::shortcuts::ShortcutOperation::Ready(_))
        })
}

fn shortcut_editing_ready(cx: &App) -> bool {
    shortcut_mutation_ready(cx)
        && state::prompts::catalog(cx).read(cx, |operation| {
            matches!(operation, state::prompts::PromptOperation::Ready(_))
        })
        && state::providers::catalog(cx).read(cx, |operation| {
            matches!(operation, state::providers::ProviderOperation::Ready(_))
        })
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
        move |window, cx| {
            let mutation = state::shortcuts::delete_shortcut(cx, shortcut_id.clone());
            let deleted_title = deleted_title.clone();
            let delete_failed_title = delete_failed_title.clone();
            let completion = window.spawn(cx, async move |cx| {
                let result = mutation.await;
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
            });
            crate::app::tasks::retain_window(window, completion, cx);
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

fn field_error_message(
    errors: Vec<gpui_form::typed::ValidationIssue>,
    cx: &App,
) -> Option<SharedString> {
    errors.first().map(|error| {
        crate::features::settings::form_validation::validation_message(error.message(), cx)
    })
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
        ShortcutDialogChoices, ShortcutEditDialogState, ShortcutEditMode,
        confirm_shortcut_edit_dialog, field_error_message, input_source_from_toggle_states,
    };
    use crate::components::chat::run_settings::RunSettingsInput;
    use crate::features::settings::shortcuts::form_state::ShortcutEditFormInput;
    use crate::{database, foundation, state};
    use gpui::{AppContext as _, TestAppContext, VisualTestContext, WindowHandle};
    use tempfile::{TempDir, tempdir};

    #[gpui::test]
    fn missing_hotkey_confirm_keeps_shortcut_dialog_open(cx: &mut TestAppContext) {
        let _dir = init_shortcut_dialog_test(cx);
        let required_message = foundation::I18n::english_for_test().t("gpui-form-error-required");
        let window = open_shortcut_state_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).expect("shortcut dialog root");
        let saved = cx.update(|window, cx| confirm_shortcut_edit_dialog(&form, window, cx));
        assert!(!saved);

        let form_store = form.read_with(&cx, |dialog, _| dialog.form.clone());
        assert_eq!(
            form_store.read_with(&cx, |_store, cx| {
                field_error_message(ShortcutEditFormInput::HOTKEY.errors(&form_store, cx), cx)
                    .map(|message| message.to_string())
            }),
            Some(required_message.clone())
        );
        assert_eq!(
            form_store.read_with(&cx, |_store, cx| {
                let model = ShortcutEditFormInput::ROOT
                    .then(ShortcutEditFormInput::RUN_SETTINGS)
                    .then(RunSettingsInput::MODEL);
                field_error_message(model.errors(&form_store, cx), cx)
                    .map(|message| message.to_string())
            }),
            Some(required_message)
        );
        cx.update(|_, cx| {
            assert!(
                crate::database::with_ready_repository(cx, |repo| repo.list_shortcuts())
                    .expect("list shortcuts")
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn shortcut_dialog_contains_run_settings_group(cx: &mut TestAppContext) {
        let _dir = init_shortcut_dialog_test(cx);
        let window = open_shortcut_state_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).expect("shortcut dialog root");

        assert!(form.read_with(&cx, |dialog, cx| {
            let run_settings = ShortcutEditFormInput::RUN_SETTINGS.value(&dialog.form, cx);
            run_settings.model.is_none()
        }));
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
            database::install_for_test(cx, dir.path());
            cx.set_global(foundation::I18n::english_for_test());
            state::providers::init(cx);
            state::prompts::init(cx);
            state::shortcuts::init(cx);
        });
        cx.run_until_parked();
        dir
    }

    fn open_shortcut_state_window(
        cx: &mut TestAppContext,
    ) -> WindowHandle<ShortcutEditDialogState> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    ShortcutEditDialogState::new(
                        ShortcutEditMode::Create,
                        None,
                        ShortcutDialogChoices {
                            prompts: Vec::new(),
                        },
                        Vec::new(),
                        None,
                        window,
                        cx,
                    )
                })
            })
            .expect("open shortcut state test window")
        })
    }
}
