use super::validation::{canonical_hotkey, validate_shortcut_hotkey};
use crate::{
    components::run_settings::RunSettingsInput,
    features::settings::form_validation::{JacoGardeI18nProvider, JacoValidationContext},
    state::providers::ProviderModelKey,
};
use fluent_bundle::FluentArgs;
use gpui_form::typed::{SubmitTransform, TransformReport};
use jaco_core::{PromptId, ShortcutInputSource};
use jaco_db::ShortcutRecord;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ShortcutValidationDependencies {
    pub(super) shortcut_id: Option<jaco_core::ShortcutId>,
    pub(super) existing_shortcuts: Vec<ShortcutRecord>,
    pub(super) temporary_hotkey: Option<String>,
}

pub(super) type ShortcutEditValidationContext =
    JacoValidationContext<ShortcutValidationDependencies>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(context(ShortcutEditValidationContext))]
#[form(
    store = ShortcutEditFormStore,
    validation(adapter = "garde", i18n = JacoGardeI18nProvider),
    transform(adapter = ShortcutEditTransform)
)]
pub(super) struct ShortcutEditFormInput {
    #[form(required, validate(on_change, on_blur, on_submit))]
    #[garde(custom(validate_hotkey))]
    pub(super) hotkey: Option<String>,
    #[garde(skip)]
    pub(super) prompt: ShortcutPromptSelection,
    #[form(group)]
    #[garde(skip)]
    pub(super) run_settings: RunSettingsInput,
    #[garde(skip)]
    pub(super) input_source: ShortcutInputSource,
    #[garde(skip)]
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
        Self {
            hotkey: shortcut.map(|shortcut| shortcut.hotkey.clone()),
            prompt: ShortcutPromptSelection(selected_prompt),
            run_settings: RunSettingsInput::new(
                selected_model,
                shortcut
                    .and_then(|shortcut| shortcut.settings_snapshot.reasoning_selection.clone()),
                shortcut
                    .map(|shortcut| shortcut.settings_snapshot.tool_policy.approval_mode)
                    .unwrap_or(jaco_core::ToolApprovalMode::RequestApproval),
            ),
            input_source: shortcut
                .map(|shortcut| shortcut.input_source)
                .unwrap_or(ShortcutInputSource::SelectionOrClipboard),
            enabled: shortcut.map(|shortcut| shortcut.enabled).unwrap_or(true),
        }
    }
}

fn validate_hotkey(
    value: &Option<String>,
    context: &ShortcutEditValidationContext,
) -> garde::Result {
    validate_shortcut_hotkey(
        value.clone(),
        context.dependencies.shortcut_id.as_ref(),
        &context.dependencies.existing_shortcuts,
        context.dependencies.temporary_hotkey.as_deref(),
    )
    .map(|_| ())
    .map_err(|error| context.error(error.i18n_key(), &FluentArgs::new()))
}

#[derive(Clone, Debug, Default)]
pub(super) struct ShortcutEditTransform;

impl SubmitTransform<ShortcutEditFormInput> for ShortcutEditTransform {
    type Output = ShortcutEditFormInput;

    fn transform(&self, model: &ShortcutEditFormInput) -> Result<Self::Output, TransformReport> {
        Ok(normalize_shortcut_input(model))
    }
}

fn normalize_shortcut_input(model: &ShortcutEditFormInput) -> ShortcutEditFormInput {
    let hotkey = model
        .hotkey
        .as_ref()
        .map(|hotkey| canonical_hotkey(hotkey).unwrap_or_else(|_| hotkey.trim().to_string()));
    ShortcutEditFormInput {
        hotkey,
        prompt: model.prompt.clone(),
        run_settings: model.run_settings.clone(),
        input_source: model.input_source,
        enabled: model.enabled,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ShortcutPromptSelection(pub(super) Option<PromptId>);
