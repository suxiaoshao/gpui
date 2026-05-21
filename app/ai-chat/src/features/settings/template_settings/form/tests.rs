use super::{PromptFormValue, TemplateValidationMessages, build_template_submission};
use crate::{database::Role, llm::CapabilityRequirement};

fn prompt(role: Option<Role>, prompt: &str) -> PromptFormValue {
    PromptFormValue {
        role,
        prompt: prompt.to_string(),
    }
}

fn err(result: Result<crate::database::NewConversationTemplate, String>) -> String {
    match result {
        Ok(_) => panic!("expected submission validation to fail"),
        Err(err) => err,
    }
}

fn messages() -> TemplateValidationMessages<'static> {
    TemplateValidationMessages {
        required_template: "name and icon required",
        required_prompt_role: "role required",
        required_prompt_content: "prompt required",
    }
}

#[test]
fn submission_requires_name_and_icon() {
    let err = err(build_template_submission(
        " ",
        "🧩",
        "",
        vec![prompt(Some(Role::User), "hello")],
        vec![],
        messages(),
    ));

    assert_eq!(err, "name and icon required");
}

#[test]
fn submission_requires_prompt_role() {
    let err = err(build_template_submission(
        "Name",
        "🧩",
        "",
        vec![prompt(None, "hello")],
        vec![],
        messages(),
    ));

    assert_eq!(err, "role required 1");
}

#[test]
fn submission_requires_prompt_content() {
    let err = err(build_template_submission(
        "Name",
        "🧩",
        "",
        vec![prompt(Some(Role::User), " ")],
        vec![],
        messages(),
    ));

    assert_eq!(err, "prompt required 1");
}

#[test]
fn submission_trims_values_and_maps_empty_description_to_none() {
    let submission = build_template_submission(
        " Name ",
        " 🧩 ",
        "   ",
        vec![prompt(Some(Role::Developer), " hello ")],
        vec![CapabilityRequirement::ImageInput],
        messages(),
    )
    .unwrap();

    assert_eq!(submission.name, "Name");
    assert_eq!(submission.icon, "🧩");
    assert_eq!(submission.description, None);
    assert_eq!(submission.prompts.len(), 1);
    assert_eq!(submission.prompts[0].role, Role::Developer);
    assert_eq!(submission.prompts[0].prompt, "hello");
    assert_eq!(
        submission.required_capabilities,
        vec![CapabilityRequirement::ImageInput]
    );
}
