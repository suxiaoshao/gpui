use crate::{
    components::{
        delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
        hotkey_input::{HotkeyInput, string_to_keystroke},
        model_picker::{ModelOption, model_select_groups},
    },
    foundation::{I18n, assets::IconName},
    state::{
        self,
        providers::{ProviderModelChoice, ProviderModelKey},
        shortcuts::ShortcutDraft,
    },
};
use ai_chat_core::{ShortcutId, ShortcutInputSource};
use ai_chat_db::ShortcutRecord;
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IndexPath, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants, Toggle, ToggleGroup, ToggleVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    select::{SearchableVec, Select, SelectDelegate, SelectGroup, SelectItem, SelectState},
    switch::Switch,
    v_flex,
};
use std::rc::Rc;

use super::super::push_settings_error;
use super::{
    choices::{InputSourceChoice, PromptChoice},
    rows::{ShortcutManagementRow, input_source_label},
    validation::{ShortcutValidationError, validate_shortcut_hotkey},
};

type ShortcutRecordDialogHandler = Rc<dyn Fn(ShortcutRecord, &mut Window, &mut App) + 'static>;
type ModelSelectState = SelectState<SearchableVec<SelectGroup<ModelOption>>>;

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
    hotkey_input: Entity<HotkeyInput>,
    prompt_select: Entity<SelectState<Vec<PromptChoice>>>,
    model_select: Entity<ModelSelectState>,
    input_source: ShortcutInputSource,
    enabled: bool,
    existing_shortcuts: Vec<ShortcutRecord>,
    temporary_hotkey: Option<String>,
    validation_error: Option<SharedString>,
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
        let selected_prompt = shortcut
            .as_ref()
            .and_then(|shortcut| shortcut.prompt_id.clone());
        let selected_model = shortcut
            .as_ref()
            .and_then(|shortcut| {
                Some(ProviderModelKey {
                    provider_id: shortcut.provider_id.as_ref()?.clone(),
                    model_id: shortcut.model_id.as_ref()?.clone(),
                })
            })
            .or_else(|| choices.models.first().map(ProviderModelChoice::key));
        let selected_input_source = shortcut
            .as_ref()
            .map(|shortcut| shortcut.input_source)
            .unwrap_or(ShortcutInputSource::SelectionOrClipboard);
        let hotkey = shortcut
            .as_ref()
            .and_then(|shortcut| string_to_keystroke(&shortcut.hotkey));
        let enabled = shortcut
            .as_ref()
            .map(|shortcut| shortcut.enabled)
            .unwrap_or(true);
        let prompt_selected_index = choices
            .prompts
            .iter()
            .position(|choice| choice.value() == &selected_prompt)
            .map(|row| IndexPath::default().row(row));
        let model_options = model_select_groups(&choices.models);
        let model_selected_index = selected_model
            .as_ref()
            .and_then(|selected_model| model_options.position(selected_model));

        let hotkey_input = cx.new(|cx| {
            HotkeyInput::new("shortcut-dialog-hotkey", window, cx)
                .w_full()
                .default_value(hotkey)
        });
        let prompt_select = cx.new(|cx| {
            SelectState::new(choices.prompts, prompt_selected_index, window, cx).searchable(true)
        });
        let model_select = cx.new(|cx| {
            SelectState::new(model_options, model_selected_index, window, cx).searchable(true)
        });

        Self {
            mode,
            shortcut_id,
            hotkey_input,
            prompt_select,
            model_select,
            input_source: selected_input_source,
            enabled,
            existing_shortcuts,
            temporary_hotkey,
            validation_error: None,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let hotkey = self
            .hotkey_input
            .read(cx)
            .current_hotkey_string()
            .map(|hotkey| hotkey.to_string());
        let hotkey = match validate_shortcut_hotkey(
            hotkey,
            self.shortcut_id.as_ref(),
            &self.existing_shortcuts,
            self.temporary_hotkey.as_deref(),
        ) {
            Ok(hotkey) => hotkey,
            Err(err) => {
                self.validation_error = Some(validation_message(err, cx));
                cx.notify();
                return false;
            }
        };
        let prompt_id = self
            .prompt_select
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();
        let Some(model_key) = self.model_select.read(cx).selected_value().cloned() else {
            self.validation_error = Some(validation_message(
                ShortcutValidationError::ModelRequired,
                cx,
            ));
            cx.notify();
            return false;
        };
        let draft = ShortcutDraft {
            hotkey,
            enabled: self.enabled,
            prompt_id,
            provider_id: model_key.provider_id,
            model_id: model_key.model_id,
            input_source: self.input_source,
        };
        let result = match self.mode {
            ShortcutEditMode::Create => state::shortcuts::create_shortcut(cx, draft),
            ShortcutEditMode::Edit => {
                let Some(shortcut_id) = self.shortcut_id.as_ref() else {
                    let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
                    push_settings_error(window, cx, title, "shortcut id is missing");
                    return false;
                };
                state::shortcuts::update_shortcut(cx, shortcut_id, draft)
            }
        };

        match result {
            Ok(_) => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t(match self.mode {
                            ShortcutEditMode::Create => "notify-shortcut-created",
                            ShortcutEditMode::Edit => "notify-shortcut-updated",
                        }))
                        .with_type(NotificationType::Success),
                    cx,
                );
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
                push_settings_error(window, cx, title, err);
                false
            }
        }
    }

    fn focus_hotkey(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }

    fn render_input_source_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        ToggleGroup::new("shortcut-dialog-input-source")
            .segmented()
            .outline()
            .w_full()
            .children(input_source_choices(cx).into_iter().map(|choice| {
                Toggle::new(input_source_toggle_id(choice.value()))
                    .label(choice.label())
                    .checked(self.input_source == choice.value())
                    .flex_1()
                    .h(px(40.))
            }))
            .on_click(cx.listener(|this, states: &Vec<bool>, _window, cx| {
                this.input_source = input_source_from_toggle_states(this.input_source, states);
                cx.notify();
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
        v_flex()
            .w_full()
            .gap_4()
            .child(form_field(
                field_hotkey,
                self.hotkey_input.clone().into_any_element(),
            ))
            .child(form_field(
                field_prompt.clone(),
                Select::new(&self.prompt_select)
                    .placeholder(field_prompt)
                    .w_full()
                    .into_any_element(),
            ))
            .child(form_field(
                field_model.clone(),
                Select::new(&self.model_select)
                    .placeholder(field_model)
                    .search_placeholder(cx.global::<I18n>().t("chat-form-model-search-placeholder"))
                    .menu_max_h(rems(18.))
                    .w_full()
                    .into_any_element(),
            ))
            .child(form_field(
                field_input_source,
                self.render_input_source_toggle(cx),
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
                            .checked(self.enabled)
                            .on_click(cx.listener(|this, checked, _window, cx| {
                                this.enabled = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.validation_error.clone(), |this, error| {
                this.child(Label::new(error).text_sm().text_color(cx.theme().danger))
            })
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

fn form_field(label: impl Into<SharedString>, input: AnyElement) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(Label::new(label.into()).text_sm().font_medium())
        .child(input)
        .into_any_element()
}

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
        confirm_shortcut_edit_dialog, input_source_from_toggle_states, open_shortcut_edit_dialog,
        validation_message,
    };
    use crate::{
        database::FreshStoreGlobal, foundation, state, state::providers::ProviderModelChoice,
    };
    use ai_chat_core::conservative_model_capabilities;
    use gpui::{AppContext as _, Render, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::{IndexPath, WindowExt};
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
            form.read_with(&cx, |form, _| form
                .validation_error
                .as_ref()
                .map(ToString::to_string)),
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

        let model_select = form.read_with(&cx, |form, _| form.model_select.clone());
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
                ai_chat_core::ShortcutInputSource::SelectionOrClipboard,
                &[true, true],
            ),
            ai_chat_core::ShortcutInputSource::Screenshot
        );
        assert_eq!(
            input_source_from_toggle_states(
                ai_chat_core::ShortcutInputSource::Screenshot,
                &[true, true],
            ),
            ai_chat_core::ShortcutInputSource::SelectionOrClipboard
        );
        assert_eq!(
            input_source_from_toggle_states(
                ai_chat_core::ShortcutInputSource::Screenshot,
                &[false, false],
            ),
            ai_chat_core::ShortcutInputSource::Screenshot
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
