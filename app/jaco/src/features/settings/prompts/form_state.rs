use gpui_form::typed::{SubmitTransform, TransformReport};
use jaco_core::PromptId;

use super::super::form_validation::{
    JacoGardeMessageProvider, JacoValidationContext, garde_message,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PromptValidationDependencies {
    pub(super) prompt_id: Option<PromptId>,
    pub(super) existing_prompts: Vec<(PromptId, String)>,
}

pub(super) type PromptEditValidationContext = JacoValidationContext<PromptValidationDependencies>;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(context(PromptEditValidationContext))]
#[form(
    store = PromptEditFormStore,
    validation(adapter = "garde", messages = JacoGardeMessageProvider),
    transform(adapter = PromptEditTransform)
)]
pub(super) struct PromptEditFormInput {
    #[form(required, validate(on_change, on_blur, on_submit))]
    #[garde(custom(validate_prompt_name))]
    pub(super) name: String,
    #[form(required, validate(on_change, on_blur, on_submit))]
    #[garde(skip)]
    pub(super) content: String,
}

impl PromptEditFormInput {
    pub(super) fn new(name: String, content: String) -> Self {
        Self { name, content }
    }
}

fn validate_prompt_name(value: &str, context: &PromptEditValidationContext) -> garde::Result {
    let name = value.trim();
    if name.is_empty() {
        return Ok(());
    }
    let duplicate = context
        .dependencies
        .existing_prompts
        .iter()
        .any(|(prompt_id, prompt_name)| {
            context.dependencies.prompt_id.as_ref() != Some(prompt_id) && prompt_name.trim() == name
        });
    if duplicate {
        return Err(garde_message(
            "prompt-validation-name-duplicate",
            std::iter::empty(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub(super) struct PromptEditTransform;

impl SubmitTransform<PromptEditFormInput> for PromptEditTransform {
    type Output = PromptEditFormInput;

    fn transform(&self, model: &PromptEditFormInput) -> Result<Self::Output, TransformReport> {
        Ok(normalize_prompt_input(model))
    }
}

fn normalize_prompt_input(model: &PromptEditFormInput) -> PromptEditFormInput {
    PromptEditFormInput {
        name: model.name.trim().to_string(),
        content: model.content.trim().to_string(),
    }
}
