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
    FormComponentEventSink, FormField, FormMeta, RequiredValue, SubmitTransform, SubscriptionSet,
    TransformContext, TransformReport, ValidationAdapter, ValidationAdapterReport,
    ValidationContext, ValidationIssue, ValidationScope, ValidationSource, ValidationTrigger,
};

use super::{
    choices::PromptChoice,
    validation::{ShortcutValidationError, canonical_hotkey, validate_shortcut_hotkey},
};

type BoolInputBinding = gpui_form_gpui_component::BoolBinding;

pub(super) type ShortcutPromptSelectStateInner = SelectState<Vec<PromptChoice>>;
pub(super) type ShortcutModelSelectStateInner =
    SelectState<SearchableVec<SelectGroup<ModelOption>>>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = ShortcutEditFormStore,
    validation(adapter = ShortcutEditValidator, context = ShortcutEditValidationContext),
    transform(adapter = ShortcutEditTransform)
)]
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

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ShortcutEditValidationContext {
    pub(super) shortcut_id: Option<ai_chat_core::ShortcutId>,
    pub(super) existing_shortcuts: Vec<ShortcutRecord>,
    pub(super) temporary_hotkey: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ShortcutEditValidator;

impl ValidationAdapter<ShortcutEditFormInput> for ShortcutEditValidator {
    type Context = ShortcutEditValidationContext;

    fn validate(
        &self,
        draft: &ShortcutEditFormInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let mut issues = Vec::new();
        let hotkey_path = gpui_form::FieldPath::from_static(ShortcutEditFormField::Hotkey.key());
        let model_path = gpui_form::FieldPath::from_static(ShortcutEditFormField::Model.key());

        if scope_includes_path(&scope, &hotkey_path)
            && let Err(error) = validate_shortcut_hotkey(
                draft.hotkey.clone(),
                context.external.shortcut_id.as_ref(),
                &context.external.existing_shortcuts,
                context.external.temporary_hotkey.as_deref(),
            )
        {
            issues.push(shortcut_issue(hotkey_path, trigger, error));
        }

        if scope_includes_path(&scope, &model_path) && draft.model.selected.is_none() {
            issues.push(shortcut_issue(
                model_path,
                trigger,
                ShortcutValidationError::ModelRequired,
            ));
        }

        ValidationAdapterReport::new(issues)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ShortcutEditTransform;

impl SubmitTransform<ShortcutEditFormInput, ShortcutEditFormInput> for ShortcutEditTransform {
    fn preview(
        &self,
        draft: &ShortcutEditFormInput,
        _context: &TransformContext,
    ) -> Result<ShortcutEditFormInput, TransformReport> {
        Ok(normalize_shortcut_input(draft))
    }

    fn transform_on_submit(
        &self,
        draft: &ShortcutEditFormInput,
        _context: &TransformContext,
    ) -> Result<ShortcutEditFormInput, TransformReport> {
        Ok(normalize_shortcut_input(draft))
    }
}

fn normalize_shortcut_input(draft: &ShortcutEditFormInput) -> ShortcutEditFormInput {
    let hotkey = draft
        .hotkey
        .as_ref()
        .map(|hotkey| canonical_hotkey(hotkey).unwrap_or_else(|_| hotkey.trim().to_string()));

    ShortcutEditFormInput {
        hotkey,
        prompt: draft.prompt.clone(),
        model: draft.model.clone(),
        input_source: draft.input_source,
        enabled: draft.enabled,
    }
}

fn shortcut_issue(
    path: gpui_form::FieldPath,
    trigger: ValidationTrigger,
    error: ShortcutValidationError,
) -> ValidationIssue {
    ValidationIssue::field(
        path,
        trigger,
        ValidationSource::App("ai-chat2-shortcut".into()),
        shortcut_error_code(&error),
        error.i18n_key(),
    )
}

fn shortcut_error_code(error: &ShortcutValidationError) -> &'static str {
    match error {
        ShortcutValidationError::HotkeyRequired => "hotkey_required",
        ShortcutValidationError::HotkeyInvalid => "hotkey_invalid",
        ShortcutValidationError::HotkeyPlainKey => "hotkey_plain_key",
        ShortcutValidationError::TemporaryConflict => "temporary_conflict",
        ShortcutValidationError::BindingConflict => "binding_conflict",
        ShortcutValidationError::ModelRequired => "model_required",
    }
}

fn scope_includes_path(scope: &ValidationScope, path: &gpui_form::FieldPath) -> bool {
    match scope {
        ValidationScope::Form => true,
        ValidationScope::Field(field_path) => field_path == path,
        ValidationScope::Group(group_path) => path.starts_with(group_path),
        ValidationScope::ArrayItem {
            path: array_path, ..
        } => path.starts_with(array_path),
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

impl RequiredValue for ShortcutModelSelection {
    fn is_empty_value(&self) -> bool {
        self.selected.is_none()
    }
}

pub(super) struct ShortcutHotkeyBinding;

impl FormComponentBinding<Option<String>> for ShortcutHotkeyBinding {
    type State = HotkeyInput;
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

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        state.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        true
    }

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |form, _state, event: &HotkeyInputEvent, window, cx| match event {
                HotkeyInputEvent::Change => sink.emit(
                    form,
                    FormComponentEvent::Change(FieldChangeCause::UserInput),
                    window,
                    cx,
                ),
            },
        ));
        subscriptions
    }
}

pub(super) struct ShortcutPromptSelectState {
    pub(super) select: Entity<ShortcutPromptSelectStateInner>,
    choices: Vec<PromptChoice>,
}

pub(super) struct ShortcutPromptSelectBinding;

impl FormComponentBinding<ShortcutPromptSelection> for ShortcutPromptSelectBinding {
    type State = ShortcutPromptSelectState;
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
            ShortcutPromptSelectState { select, choices }
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

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let select = state.read(cx).select.clone();
        let focus_handle = {
            let select = select.read(cx);
            select.focus_handle(cx)
        };
        focus_handle.focus(window, cx);
        true
    }

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let select = state.read(cx).select.clone();
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |form, _select, event: &SelectEvent<Vec<PromptChoice>>, window, cx| {
                let SelectEvent::Confirm(_) = event;
                sink.emit(
                    form,
                    FormComponentEvent::Change(FieldChangeCause::UserInput),
                    window,
                    cx,
                );
            },
        ));
        subscriptions
    }
}

pub(super) struct ShortcutModelSelectState {
    pub(super) select: Entity<ShortcutModelSelectStateInner>,
    choices: Vec<ProviderModelChoice>,
}

pub(super) struct ShortcutModelSelectBinding;

impl FormComponentBinding<ShortcutModelSelection> for ShortcutModelSelectBinding {
    type State = ShortcutModelSelectState;
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
            ShortcutModelSelectState { select, choices }
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

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let select = state.read(cx).select.clone();
        let focus_handle = {
            let select = select.read(cx);
            select.focus_handle(cx)
        };
        focus_handle.focus(window, cx);
        true
    }

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let select = state.read(cx).select.clone();
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &select,
            window,
            move |form,
                  _select,
                  event: &SelectEvent<SearchableVec<SelectGroup<ModelOption>>>,
                  window,
                  cx| {
                let SelectEvent::Confirm(_) = event;
                sink.emit(
                    form,
                    FormComponentEvent::Change(FieldChangeCause::UserInput),
                    window,
                    cx,
                );
            },
        ));
        subscriptions
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
