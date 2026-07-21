use crate::{
    components::chat_form::{
        AddAttachmentControl, AttachmentControlState, ChatForm, ChatFormControls, ControlSlot,
        PrimaryActionControlState, RunSettingsControls,
    },
    components::chat_input::ComposerEditor,
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    components::hotkey_input::{HotkeyInput, HotkeyInputEvent, string_to_keystroke},
    components::run_settings::{
        RunSettingsController, RunSettingsSubmitError, resolve_run_settings,
    },
    foundation::{I18n, assets::IconName},
    state::{self, shortcuts::ShortcutDraft},
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
    select::{Select, SelectState},
    switch::Switch,
    v_flex,
};
use gpui_form::typed::FormStore as _;
use gpui_form_gpui_component::FormSelect;
use jaco_core::{ShortcutId, ShortcutInputSource};
use jaco_db::ShortcutRecord;
use std::{cell::Cell, rc::Rc};

use super::super::push_settings_error;
use super::{
    choices::{InputSourceChoice, PromptChoice},
    form_state::{
        ShortcutEditFormInput, ShortcutEditFormStore, ShortcutEditValidationContext,
        ShortcutValidationDependencies,
    },
    rows::{ShortcutManagementRow, input_source_label},
};

type ShortcutRecordDialogHandler = Rc<dyn Fn(ShortcutRecord, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum HotkeySyncState {
    #[default]
    Idle,
    FromForm,
    FromComponent,
}

fn bind_hotkey<Form, Owner>(
    field: gpui_form::typed::FormField<Form, Option<String>>,
    state: &Entity<HotkeyInput>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<Vec<Subscription>, gpui_form_gpui_component::FormControlError>
where
    Form: gpui_form::typed::FormStore + EventEmitter<gpui_form::typed::FormEvent<Form::Field>>,
    Owner: 'static,
{
    let initial = field
        .value(cx)
        .map_err(gpui_form_gpui_component::FormControlError::from)?;
    state.update(cx, |input, cx| {
        input.set_hotkey(initial.as_deref().and_then(string_to_keystroke), cx);
    });

    let sync = Rc::new(Cell::new(HotkeySyncState::Idle));
    let mut subscriptions = Vec::new();
    let form_sync = sync.clone();
    let form_state = state.clone();
    let subscribed_field = field.clone();
    subscriptions.push(subscribed_field.clone().subscribe_in(
        window,
        cx,
        move |_owner, window, cx| {
            if form_sync.get() == HotkeySyncState::FromComponent {
                return;
            }
            let field = subscribed_field.clone();
            let form_state = form_state.clone();
            let form_sync = form_sync.clone();
            cx.defer_in(window, move |_owner, _window, cx| {
                let Ok(value) = field.value(cx) else { return };
                form_sync.set(HotkeySyncState::FromForm);
                form_state.update(cx, |input, cx| {
                    input.set_hotkey(value.as_deref().and_then(string_to_keystroke), cx);
                });
                form_sync.set(HotkeySyncState::Idle);
            });
        },
    )?);

    let component_sync = sync;
    let component_field = field;
    subscriptions.push(cx.subscribe_in(
        state,
        window,
        move |_owner, state, event: &HotkeyInputEvent, window, cx| {
            if !matches!(event, HotkeyInputEvent::Change)
                || component_sync.get() == HotkeySyncState::FromForm
            {
                return;
            }
            let draft = state.read(cx).current_hotkey_string();
            let sync = component_sync.clone();
            let field = component_field.clone();
            cx.defer_in(window, move |_owner, _window, cx| {
                if sync.get() == HotkeySyncState::FromForm {
                    return;
                }
                sync.set(HotkeySyncState::FromComponent);
                let _ = field.set_user_value(draft, cx);
                sync.set(HotkeySyncState::Idle);
            });
        },
    ));

    Ok(subscriptions)
}

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
    hotkey_input: Entity<HotkeyInput>,
    prompt_select: Entity<SelectState<Vec<PromptChoice>>>,
    _prompt_control: FormSelect<Vec<PromptChoice>>,
    _subscriptions: Vec<Subscription>,
    _run_settings: Entity<RunSettingsController<ShortcutEditFormStore>>,
    chat_form: Entity<ChatForm>,
    existing_shortcuts: Vec<ShortcutRecord>,
    temporary_hotkey: Option<String>,
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
        let validation_context = ShortcutEditValidationContext::new(
            ShortcutValidationDependencies {
                shortcut_id: shortcut_id.clone(),
                existing_shortcuts: existing_shortcuts.clone(),
                temporary_hotkey: temporary_hotkey.clone(),
            },
            cx,
        );
        let form = cx.new(|cx| {
            ShortcutEditFormStore::from_value_with_validation_context(
                form_input,
                validation_context,
                cx,
            )
        });
        let hotkey_input = cx.new(|cx| {
            let hotkey = ShortcutEditFormStore::hotkey_field(&form)
                .value(cx)
                .unwrap_or_default();
            HotkeyInput::new("shortcut-dialog-hotkey", window, cx)
                .w_full()
                .default_value(hotkey.as_deref().and_then(string_to_keystroke))
        });
        let prompt_field = ShortcutEditFormStore::prompt_field(&form).project_value(
            "selection",
            |selection| Some(Some(selection.0.clone())),
            |selection, value| {
                selection.0 = value.flatten();
                true
            },
        );
        let prompt_control = FormSelect::new(
            prompt_field,
            move |window, cx| SelectState::new(prompt_choices, None, window, cx),
            window,
            cx,
        )
        .expect("shortcut prompt form entity is alive");
        let prompt_select = (*prompt_control).clone();
        let mut subscriptions = Vec::new();
        let form_for_locale = form.downgrade();
        subscriptions.push(cx.observe_global::<I18n>(move |_dialog, cx| {
            let form = form_for_locale.clone();
            cx.defer(move |cx| {
                let Some(form) = form.upgrade() else { return };
                form.update(cx, |form, cx| {
                    let context = form.validation_context().relocalized(cx);
                    form.set_validation_context(context, cx);
                });
            });
        }));
        subscriptions.extend(
            bind_hotkey(
                ShortcutEditFormStore::hotkey_field(&form),
                &hotkey_input,
                window,
                cx,
            )
            .expect("shortcut hotkey form entity is alive"),
        );
        let run_settings_field = ShortcutEditFormStore::run_settings_field(&form);
        let run_settings = cx.new(|cx| RunSettingsController::new(run_settings_field, window, cx));
        let run_settings_states = run_settings.read(cx).control_states();
        let placeholder = cx.global::<I18n>().t("chat-form-placeholder");
        let composer = cx.new(|cx| ComposerEditor::new(placeholder, window, cx));
        let attachments = cx.new(|_| AttachmentControlState::default());
        let primary_action = cx.new(|_| PrimaryActionControlState::default());
        let chat_form = cx.new(|cx| {
            ChatForm::new(
                ChatFormControls {
                    project: ControlSlot::Hidden,
                    composer: ControlSlot::Disabled(composer.clone()),
                    attachments: ControlSlot::Disabled(attachments),
                    add_attachment: ControlSlot::Disabled(AddAttachmentControl),
                    run_settings: RunSettingsControls {
                        model: ControlSlot::Enabled(run_settings_states.model),
                        reasoning: ControlSlot::Enabled(run_settings_states.reasoning),
                        approval: ControlSlot::Enabled(run_settings_states.approval),
                    },
                    primary_action: ControlSlot::Disabled(primary_action),
                },
                window,
                cx,
            )
        });

        Self {
            mode,
            shortcut_id,
            form,
            hotkey_input,
            prompt_select,
            _prompt_control: prompt_control,
            _subscriptions: subscriptions,
            _run_settings: run_settings,
            chat_form,
            existing_shortcuts,
            temporary_hotkey,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mode = self.mode;
        let shortcut_id = self.shortcut_id.clone();
        let validation_context = ShortcutEditValidationContext::new(
            ShortcutValidationDependencies {
                shortcut_id: shortcut_id.clone(),
                existing_shortcuts: self.existing_shortcuts.clone(),
                temporary_hotkey: self.temporary_hotkey.clone(),
            },
            cx,
        );
        let result = self.form.update(cx, |form, cx| {
            form.set_validation_context(validation_context, cx);
            let revision = form.revision();
            form.prepare_submit(cx).map(|draft| (revision, draft))
        });
        let Ok((revision, draft)) = result else {
            return false;
        };
        let Some(hotkey) = draft.hotkey.clone() else {
            return false;
        };
        let Some(catalog) = cx
            .has_global::<state::providers::ProviderCatalogGlobal>()
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
        let choices = catalog.read_cloned(cx, |snapshot| &snapshot.enabled_models);
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
            prompt_id: draft.prompt.0.clone(),
            provider_id: resolved.provider_model.provider_id,
            model_id: resolved.provider_model.model_id,
            input_source: draft.input_source,
            reasoning_selection: resolved.reasoning_selection,
            approval_mode: resolved.approval_mode,
        };
        let persisted = match mode {
            ShortcutEditMode::Create => state::shortcuts::create_shortcut(cx, shortcut_draft),
            ShortcutEditMode::Edit => {
                let Some(id) = shortcut_id.as_ref() else {
                    let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
                    push_settings_error(window, cx, title, "shortcut id is missing".to_string());
                    return false;
                };
                state::shortcuts::update_shortcut(cx, id, shortcut_draft)
            }
        };
        if let Err(error) = persisted {
            let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
            push_settings_error(window, cx, title, error.to_string());
            return false;
        }
        self.form
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
        true
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
                let current = ShortcutEditFormStore::input_source_field(&this.form)
                    .value(cx)
                    .unwrap_or(ShortcutInputSource::SelectionOrClipboard);
                let input_source = input_source_from_toggle_states(current, states);
                let _ = ShortcutEditFormStore::input_source_field(&this.form)
                    .set_user_value(input_source, cx);
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
        let hotkey_field = ShortcutEditFormStore::hotkey_field(&self.form);
        let model_field = ShortcutEditFormStore::run_settings_field(&self.form).project(
            "model",
            |settings| &settings.model,
            |settings, model| settings.model = model,
        );
        let hotkey_error = field_error_message(hotkey_field.errors(cx).unwrap_or_default(), cx);
        let model_error = field_error_message(model_field.errors(cx).unwrap_or_default(), cx);
        let (hotkey_input, prompt_select, input_source, enabled) = {
            let form = self.form.read(cx);
            (
                self.hotkey_input.clone(),
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
                hotkey_input.into_any_element(),
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
                self.chat_form.clone().into_any_element(),
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
                                let _ = ShortcutEditFormStore::enabled_field(&this.form)
                                    .set_user_value(*checked, cx);
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

fn field_error_message(
    errors: Vec<gpui_form::typed::ValidationIssue>,
    cx: &App,
) -> Option<SharedString> {
    errors.first().map(|error| {
        crate::features::settings::form_validation::validation_message(&error.message, cx)
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
    use crate::features::settings::shortcuts::form_state::ShortcutEditFormStore;
    use crate::{database::FreshStoreGlobal, foundation, state};
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
                field_error_message(
                    ShortcutEditFormStore::hotkey_field(&form_store)
                        .errors(cx)
                        .unwrap_or_default(),
                    cx,
                )
                .map(|message| message.to_string())
            }),
            Some(required_message.clone())
        );
        assert_eq!(
            form_store.read_with(&cx, |_store, cx| {
                let model = ShortcutEditFormStore::run_settings_field(&form_store).project(
                    "model",
                    |settings| &settings.model,
                    |settings, model| settings.model = model,
                );
                field_error_message(model.errors(cx).unwrap_or_default(), cx)
                    .map(|message| message.to_string())
            }),
            Some(required_message)
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
    fn shortcut_dialog_contains_run_settings_group(cx: &mut TestAppContext) {
        let _dir = init_shortcut_dialog_test(cx);
        let window = open_shortcut_state_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let form = window.root(&mut cx).expect("shortcut dialog root");

        assert!(form.read_with(&cx, |dialog, cx| {
            let run_settings = ShortcutEditFormStore::run_settings_field(&dialog.form)
                .value(cx)
                .expect("shortcut run settings field is available");
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
            cx.set_global(FreshStoreGlobal::open_in_dir(dir.path()).unwrap());
            cx.set_global(foundation::I18n::english_for_test());
            state::shortcuts::init(cx);
        });
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
