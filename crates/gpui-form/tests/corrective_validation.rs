use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use gpui::{App, AppContext as _, TestAppContext};
use gpui_form::{
    FormModel, FormState as _, ValidationAdapter, ValidationAdapterReport, ValidationIssue,
    ValidationMessage, ValidationScope, ValidationSource, ValidationTrigger,
};

#[derive(Clone, Debug, Default)]
struct ValidationContext {
    reject_name: bool,
    calls: Arc<AtomicUsize>,
}

struct Validator;

impl ValidationAdapter<Model> for Validator {
    type Context = ValidationContext;

    fn validate(
        model: &Model,
        trigger: ValidationTrigger,
        _scope: &ValidationScope,
        context: &Self::Context,
        _cx: &App,
    ) -> ValidationAdapterReport {
        context.calls.fetch_add(1, Ordering::SeqCst);
        let mut issues = vec![ValidationIssue::form(
            trigger,
            ValidationSource::App("form".into()),
            "form_issue",
            ValidationMessage::literal("form issue"),
        )];
        if context.reject_name && model.name == "taken" {
            issues.push(ValidationIssue::field(
                ModelForm::NAME.path(),
                trigger,
                ValidationSource::App("name".into()),
                "taken",
                ValidationMessage::literal("taken"),
            ));
        }
        ValidationAdapterReport::new(issues)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = ModelForm, validation(adapter = Validator, context = ValidationContext))]
struct Model {
    #[form(validate(on_dynamic, on_submit))]
    name: String,
    enabled: bool,
}

#[gpui::test]
fn field_scope_does_not_replace_form_wide_adapter_bucket(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ModelForm::from_value_with_validation_context(
                Model {
                    name: "taken".into(),
                    enabled: true,
                },
                ValidationContext {
                    reject_name: true,
                    ..Default::default()
                },
                cx,
            )
        })
    });

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx)
        });
        assert_eq!(form.read(cx).validation_report().issues().len(), 2);

        ModelForm::NAME.set(&form, "available".into(), cx);
        ModelForm::NAME.validate(&form, ValidationTrigger::Dynamic, cx);
        let report = form.read(cx).validation_report();
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.code == "form_issue")
        );
        assert!(!report.issues().iter().any(|issue| issue.code == "taken"));
    });
}

#[gpui::test]
fn replacing_validation_context_does_not_run_policy_implicitly(cx: &mut TestAppContext) {
    let context = ValidationContext::default();
    let calls = context.calls.clone();
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ModelForm::from_value_with_validation_context(
                Model {
                    name: "taken".into(),
                    enabled: true,
                },
                context,
                cx,
            )
        })
    });
    let after_mount = calls.load(Ordering::SeqCst);

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.set_validation_context(
                ValidationContext {
                    reject_name: true,
                    calls: calls.clone(),
                },
                cx,
            )
        });
        assert_eq!(calls.load(Ordering::SeqCst), after_mount);
        assert!(form.read(cx).validation_context().reject_name);
    });
}

struct InvalidPathValidator;

impl ValidationAdapter<InvalidPathModel> for InvalidPathValidator {
    type Context = ();

    fn validate(
        _model: &InvalidPathModel,
        trigger: ValidationTrigger,
        _scope: &ValidationScope,
        _context: &Self::Context,
        _cx: &App,
    ) -> ValidationAdapterReport {
        ValidationAdapterReport::new(vec![ValidationIssue::field(
            gpui_form::FieldPath::field("unknown"),
            trigger,
            ValidationSource::App("invalid".into()),
            "invalid",
            ValidationMessage::literal("invalid"),
        )])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FormModel)]
#[form(state = InvalidPathForm, validation(adapter = InvalidPathValidator))]
struct InvalidPathModel {
    value: String,
}

#[gpui::test]
fn invalid_adapter_paths_become_blocking_internal_issues(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            InvalidPathForm::from_value_with_validation_context(
                InvalidPathModel {
                    value: "value".into(),
                },
                (),
                cx,
            )
        })
    });

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.validate(ValidationTrigger::Submit, ValidationScope::Form, cx)
        });
        let report = form.read(cx).validation_report();
        assert!(!report.is_valid());
        assert!(report.issues().iter().any(|issue| {
            issue.source == ValidationSource::Internal
                && issue.code == "form_schema_path_resolution"
        }));
    });
}
