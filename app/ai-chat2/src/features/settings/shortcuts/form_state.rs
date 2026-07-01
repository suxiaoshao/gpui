use crate::{
    components::{
        hotkey_input::{HotkeyInput, HotkeyInputEvent, string_to_keystroke},
        model_picker::{ModelOption, model_select_groups},
    },
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use ai_chat_core::{PromptId, ShortcutInputSource};
use ai_chat_db::ShortcutRecord;
use gpui::*;
use gpui_component::{
    IndexPath,
    select::{SearchableVec, SelectDelegate, SelectEvent, SelectGroup, SelectItem, SelectState},
};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FormComponentBinding, FormComponentEvent,
    FormField, FormMeta,
};

type BoolInputBinding = gpui_form_gpui_component::BoolBinding;

use super::choices::PromptChoice;

pub(super) type ShortcutPromptSelectStateInner = SelectState<Vec<PromptChoice>>;
pub(super) type ShortcutModelSelectStateInner =
    SelectState<SearchableVec<SelectGroup<ModelOption>>>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ShortcutEditFormStore)]
pub(super) struct ShortcutEditFormInput {
    #[form(binding = "ShortcutHotkeyBinding", required)]
    pub(super) hotkey: Option<String>,
    #[form(binding = "ShortcutPromptSelectBinding")]
    pub(super) prompt: ShortcutPromptSelection,
    #[form(binding = "ShortcutModelSelectBinding", required)]
    pub(super) model: ShortcutModelSelection,
    #[form(component = "value")]
    pub(super) input_source: ShortcutInputSource,
    #[form(binding = "BoolInputBinding")]
    pub(super) enabled: bool,
}

impl ShortcutEditFormInput {
    pub(super) fn new(
        shortcut: Option<&ShortcutRecord>,
        prompts: Vec<PromptChoice>,
        models: Vec<ProviderModelChoice>,
    ) -> Self {
        let selected_prompt = shortcut.and_then(|shortcut| shortcut.prompt_id.clone());
        let selected_model = shortcut
            .and_then(|shortcut| {
                Some(ProviderModelKey {
                    provider_id: shortcut.provider_id.as_ref()?.clone(),
                    model_id: shortcut.model_id.as_ref()?.clone(),
                })
            })
            .or_else(|| models.first().map(ProviderModelChoice::key));
        let input_source = shortcut
            .map(|shortcut| shortcut.input_source)
            .unwrap_or(ShortcutInputSource::SelectionOrClipboard);
        let enabled = shortcut.map(|shortcut| shortcut.enabled).unwrap_or(true);
        let hotkey = shortcut.map(|shortcut| shortcut.hotkey.clone());

        Self {
            hotkey,
            prompt: ShortcutPromptSelection {
                selected: selected_prompt,
                choices: prompts,
            },
            model: ShortcutModelSelection {
                selected: selected_model,
                choices: models,
            },
            input_source,
            enabled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShortcutPromptSelection {
    pub(super) selected: Option<PromptId>,
    choices: Vec<PromptChoice>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShortcutModelSelection {
    pub(super) selected: Option<ProviderModelKey>,
    choices: Vec<ProviderModelChoice>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShortcutSelectBindingEvent {
    Change,
}

pub(super) struct ShortcutHotkeyBinding;

impl FormComponentBinding<Option<String>> for ShortcutHotkeyBinding {
    type State = HotkeyInput;
    type Event = HotkeyInputEvent;
    type Draft = Option<String>;

    fn new_state(
        initial: &Option<String>,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let hotkey = initial.as_deref().and_then(string_to_keystroke);
        cx.new(|cx| {
            HotkeyInput::new("shortcut-dialog-hotkey", window, cx)
                .w_full()
                .default_value(hotkey)
        })
    }

    fn draft_from_value(value: &Option<String>) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).current_hotkey_string()
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<Option<String>, Box<FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &Option<String>,
        _cause: FieldChangeCause,
        _window: &mut Window,
        cx: &mut App,
    ) {
        let hotkey = value.as_deref().and_then(string_to_keystroke);
        state.update(cx, |input, cx| {
            input.set_hotkey(hotkey, cx);
        });
    }

    fn event_kind(event: &Self::Event) -> Option<FormComponentEvent> {
        match event {
            HotkeyInputEvent::Change => {
                Some(FormComponentEvent::Change(FieldChangeCause::UserInput))
            }
        }
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        state.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        true
    }
}

pub(super) struct ShortcutPromptSelectState {
    pub(super) select: Entity<ShortcutPromptSelectStateInner>,
    choices: Vec<PromptChoice>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ShortcutSelectBindingEvent> for ShortcutPromptSelectState {}

pub(super) struct ShortcutPromptSelectBinding;

impl FormComponentBinding<ShortcutPromptSelection> for ShortcutPromptSelectBinding {
    type State = ShortcutPromptSelectState;
    type Event = ShortcutSelectBindingEvent;
    type Draft = ShortcutPromptSelection;

    fn new_state(
        initial: &ShortcutPromptSelection,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let choices = initial.choices.clone();
        let selected_index = prompt_selected_index(&choices, &initial.selected);
        cx.new(|cx| {
            let select = cx.new(|cx| {
                SelectState::new(choices.clone(), selected_index, window, cx).searchable(true)
            });
            let subscription = cx.subscribe_in(
                &select,
                window,
                |_this, _select, event: &SelectEvent<Vec<PromptChoice>>, _window, cx| {
                    let SelectEvent::Confirm(_) = event;
                    cx.emit(ShortcutSelectBindingEvent::Change);
                    cx.notify();
                },
            );
            ShortcutPromptSelectState {
                select,
                choices,
                _subscriptions: vec![subscription],
            }
        })
    }

    fn draft_from_value(value: &ShortcutPromptSelection) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        let state = state.read(cx);
        ShortcutPromptSelection {
            selected: state.select.read(cx).selected_value().cloned().flatten(),
            choices: state.choices.clone(),
        }
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<ShortcutPromptSelection, Box<FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &ShortcutPromptSelection,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, cx| {
            if state.choices != value.choices {
                state.choices = value.choices.clone();
                state.select.update(cx, |select, cx| {
                    select.set_items(value.choices.clone(), window, cx);
                });
            }
            state.select.update(cx, |select, cx| {
                select.set_selected_value(&value.selected, window, cx);
            });
        });
    }

    fn event_kind(event: &Self::Event) -> Option<FormComponentEvent> {
        match event {
            ShortcutSelectBindingEvent::Change => {
                Some(FormComponentEvent::Change(FieldChangeCause::UserInput))
            }
        }
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let select = state.read(cx).select.clone();
        let focus_handle = {
            let select = select.read(cx);
            select.focus_handle(cx)
        };
        focus_handle.focus(window, cx);
        true
    }
}

pub(super) struct ShortcutModelSelectState {
    pub(super) select: Entity<ShortcutModelSelectStateInner>,
    choices: Vec<ProviderModelChoice>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ShortcutSelectBindingEvent> for ShortcutModelSelectState {}

pub(super) struct ShortcutModelSelectBinding;

impl FormComponentBinding<ShortcutModelSelection> for ShortcutModelSelectBinding {
    type State = ShortcutModelSelectState;
    type Event = ShortcutSelectBindingEvent;
    type Draft = ShortcutModelSelection;

    fn new_state(
        initial: &ShortcutModelSelection,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let choices = initial.choices.clone();
        let model_options = model_select_groups(&choices);
        let selected_index = initial
            .selected
            .as_ref()
            .and_then(|selected| model_options.position(selected));
        cx.new(|cx| {
            let select = cx.new(|cx| {
                SelectState::new(model_options, selected_index, window, cx).searchable(true)
            });
            let subscription = cx.subscribe_in(
                &select,
                window,
                |_this,
                 _select,
                 event: &SelectEvent<SearchableVec<SelectGroup<ModelOption>>>,
                 _window,
                 cx| {
                    let SelectEvent::Confirm(_) = event;
                    cx.emit(ShortcutSelectBindingEvent::Change);
                    cx.notify();
                },
            );
            ShortcutModelSelectState {
                select,
                choices,
                _subscriptions: vec![subscription],
            }
        })
    }

    fn draft_from_value(value: &ShortcutModelSelection) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        let state = state.read(cx);
        ShortcutModelSelection {
            selected: state.select.read(cx).selected_value().cloned(),
            choices: state.choices.clone(),
        }
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<ShortcutModelSelection, Box<FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &ShortcutModelSelection,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, cx| {
            if state.choices != value.choices {
                state.choices = value.choices.clone();
                state.select.update(cx, |select, cx| {
                    select.set_items(model_select_groups(&value.choices), window, cx);
                });
            }
            state.select.update(cx, |select, cx| {
                if let Some(selected) = value.selected.as_ref() {
                    select.set_selected_value(selected, window, cx);
                } else {
                    select.set_selected_index(None, window, cx);
                }
            });
        });
    }

    fn event_kind(event: &Self::Event) -> Option<FormComponentEvent> {
        match event {
            ShortcutSelectBindingEvent::Change => {
                Some(FormComponentEvent::Change(FieldChangeCause::UserInput))
            }
        }
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let select = state.read(cx).select.clone();
        let focus_handle = {
            let select = select.read(cx);
            select.focus_handle(cx)
        };
        focus_handle.focus(window, cx);
        true
    }
}

fn prompt_selected_index(
    choices: &[PromptChoice],
    selected: &Option<PromptId>,
) -> Option<IndexPath> {
    choices
        .iter()
        .position(|choice| choice.value() == selected)
        .map(|row| IndexPath::default().row(row))
}

pub(super) fn field_errors<Field>(field: &Field) -> Vec<FieldError>
where
    Field: FormField,
{
    field
        .visible_errors(&FormMeta::default())
        .into_iter()
        .cloned()
        .collect()
}
