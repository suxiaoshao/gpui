use super::{PromptFormValue, build_template_submission};
use crate::database::Role;

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

#[test]
fn submission_requires_name_and_icon() {
    let err = err(build_template_submission(
        " ",
        "🧩",
        "",
        vec![prompt(Some(Role::User), "hello")],
        "name and icon required",
        "role required",
        "prompt required",
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
        "name and icon required",
        "role required",
        "prompt required",
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
        "name and icon required",
        "role required",
        "prompt required",
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
        "name and icon required",
        "role required",
        "prompt required",
    )
    .unwrap();

    assert_eq!(submission.name, "Name");
    assert_eq!(submission.icon, "🧩");
    assert_eq!(submission.description, None);
    assert_eq!(submission.prompts.len(), 1);
    assert_eq!(submission.prompts[0].role, Role::Developer);
    assert_eq!(submission.prompts[0].prompt, "hello");
}
