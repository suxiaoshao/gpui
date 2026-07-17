use gpui::App;
use gpui_form::{
    FieldError, FormField, FormMeta, SubmitTransform, TransformContext, TransformReport,
    ValidationAdapter, ValidationAdapterReport, ValidationContext, ValidationIssue,
    ValidationScope, ValidationSource, ValidationTrigger,
};
use jaco_core::PromptId;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(
    store = PromptEditFormStore,
    validation(adapter = PromptEditValidator, context = PromptEditValidationContext),
    transform(adapter = PromptEditTransform)
)]
pub(super) struct PromptEditFormInput {
    #[form(component = "value", required)]
    pub(super) name: String,
    #[form(component = "value", required)]
    pub(super) content: String,
}

impl PromptEditFormInput {
    pub(super) fn new(name: String, content: String) -> Self {
        Self { name, content }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PromptEditValidationContext {
    pub(super) prompt_id: Option<PromptId>,
    pub(super) existing_prompts: Vec<(PromptId, String)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PromptEditValidator;

impl ValidationAdapter<PromptEditFormInput> for PromptEditValidator {
    type Context = PromptEditValidationContext;

    fn validate(
        &self,
        draft: &PromptEditFormInput,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        let mut issues = Vec::new();
        let name_path = gpui_form::FieldPath::from_static(PromptEditFormField::Name.key());
        let content_path = gpui_form::FieldPath::from_static(PromptEditFormField::Content.key());
        let name = draft.name.trim();
        let content = draft.content.trim();

        if scope_includes_path(&scope, &name_path) {
            if name.is_empty() {
                issues.push(prompt_issue(
                    name_path.clone(),
                    trigger,
                    "required",
                    "prompt-validation-name-required",
                ));
            } else if context
                .external
                .existing_prompts
                .iter()
                .any(|(prompt_id, prompt_name)| {
                    context.external.prompt_id.as_ref() != Some(prompt_id)
                        && prompt_name.trim() == name
                })
            {
                issues.push(prompt_issue(
                    name_path,
                    trigger,
                    "duplicate",
                    "prompt-validation-name-duplicate",
                ));
            }
        }

        if scope_includes_path(&scope, &content_path) && content.is_empty() {
            issues.push(prompt_issue(
                content_path,
                trigger,
                "required",
                "prompt-validation-content-required",
            ));
        }

        ValidationAdapterReport::new(issues)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct PromptEditTransform;

impl SubmitTransform<PromptEditFormInput, PromptEditFormInput> for PromptEditTransform {
    fn preview(
        &self,
        draft: &PromptEditFormInput,
        _context: &TransformContext,
    ) -> Result<PromptEditFormInput, TransformReport> {
        Ok(normalize_prompt_input(draft))
    }

    fn transform_on_submit(
        &self,
        draft: &PromptEditFormInput,
        _context: &TransformContext,
    ) -> Result<PromptEditFormInput, TransformReport> {
        Ok(normalize_prompt_input(draft))
    }
}

fn normalize_prompt_input(draft: &PromptEditFormInput) -> PromptEditFormInput {
    PromptEditFormInput {
        name: draft.name.trim().to_string(),
        content: draft.content.trim().to_string(),
    }
}

fn prompt_issue(
    path: gpui_form::FieldPath,
    trigger: ValidationTrigger,
    code: &'static str,
    message_key: &'static str,
) -> ValidationIssue {
    ValidationIssue::field(
        path,
        trigger,
        ValidationSource::App("jaco-prompt".into()),
        code,
        message_key,
    )
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
