use super::validation::{ShortcutValidationError, canonical_hotkey, validate_shortcut_hotkey};
use crate::{
    components::run_settings::{RunSettingsFormStore, RunSettingsInput},
    state::providers::ProviderModelKey,
};
use gpui::{App, AppContext as _};
use gpui_form::{
    FieldError, FormField, FormMeta, SubmitTransform, TransformContext, TransformReport,
    ValidationAdapter, ValidationAdapterReport, ValidationContext, ValidationIssue,
    ValidationScope, ValidationSource, ValidationTrigger,
};
use gpui_form_gpui_component::SelectFieldValue;
use jaco_core::{PromptId, ShortcutInputSource};
use jaco_db::ShortcutRecord;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = ShortcutEditFormStore,
    validation(adapter = ShortcutEditValidator, context = ShortcutEditValidationContext),
    transform(adapter = ShortcutEditTransform)
)]
pub(super) struct ShortcutEditFormInput {
    #[form(component = "value", required, validate(on_submit))]
    pub(super) hotkey: Option<String>,
    #[form(component = "value")]
    pub(super) prompt: ShortcutPromptSelection,
    #[form(group(store = "RunSettingsFormStore"))]
    pub(super) run_settings: RunSettingsInput,
    #[form(component = "value")]
    pub(super) input_source: ShortcutInputSource,
    #[form(component = "value")]
    pub(super) enabled: bool,
}

impl ShortcutEditFormInput {
    pub(super) fn new(shortcut: Option<&ShortcutRecord>) -> Self {
        let selected_prompt = shortcut.and_then(|shortcut| shortcut.prompt_id.clone());
        let selected_model = shortcut.and_then(|shortcut| {
            Some(ProviderModelKey {
                provider_id: shortcut.provider_id.as_ref()?.clone(),
                model_id: shortcut.model_id.as_ref()?.clone(),
            })
        });
        let input_source = shortcut
            .map(|shortcut| shortcut.input_source)
            .unwrap_or(ShortcutInputSource::SelectionOrClipboard);
        let enabled = shortcut.map(|shortcut| shortcut.enabled).unwrap_or(true);
        let hotkey = shortcut.map(|shortcut| shortcut.hotkey.clone());

        Self {
            hotkey,
            prompt: ShortcutPromptSelection(selected_prompt),
            run_settings: RunSettingsInput::new(
                selected_model,
                shortcut
                    .and_then(|shortcut| shortcut.settings_snapshot.reasoning_selection.clone()),
                shortcut
                    .map(|shortcut| shortcut.settings_snapshot.tool_policy.approval_mode)
                    .unwrap_or(jaco_core::ToolApprovalMode::RequestApproval),
            ),
            input_source,
            enabled,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ShortcutEditValidationContext {
    pub(super) shortcut_id: Option<jaco_core::ShortcutId>,
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
        run_settings: draft.run_settings.clone(),
        input_source: draft.input_source,
        enabled: draft.enabled,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ShortcutPromptSelection(pub(super) Option<PromptId>);

impl SelectFieldValue for ShortcutPromptSelection {
    type Selected = Option<PromptId>;

    fn to_selected_value(&self) -> Option<Self::Selected> {
        Some(self.0.clone())
    }

    fn from_selected_value(selected: Option<Self::Selected>, previous: &Self) -> Self {
        Self(selected.unwrap_or_else(|| previous.0.clone()))
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
        ValidationSource::App("jaco-shortcut".into()),
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

pub(super) fn field_errors<Field>(field: &Field, form_meta: &FormMeta) -> Vec<FieldError>
where
    Field: FormField,
{
    field
        .visible_errors(form_meta)
        .into_iter()
        .cloned()
        .collect()
}
