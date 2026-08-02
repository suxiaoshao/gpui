use gpui::{App, AppContext as _, TestAppContext};
use gpui_form::{
    FormModel, FormState as _, PreparedSubmit, SubmitError, SubmitTransform, ValidationAdapter,
    ValidationAdapterReport, ValidationIssue, ValidationMessage, ValidationScope, ValidationSource,
    ValidationTrigger,
};

struct Validator;

impl ValidationAdapter<Input> for Validator {
    type Context = ();

    fn validate(
        model: &Input,
        trigger: ValidationTrigger,
        _scope: &ValidationScope,
        _context: &Self::Context,
        _cx: &App,
    ) -> ValidationAdapterReport {
        if model.blocked {
            ValidationAdapterReport::new(vec![ValidationIssue::field(
                InputForm::BLOCKED.path(),
                trigger,
                ValidationSource::App("submit".into()),
                "blocked",
                ValidationMessage::literal("blocked"),
            )])
        } else {
            ValidationAdapterReport::default()
        }
    }
}

struct Transform;

impl SubmitTransform<Input> for Transform {
    type Output = String;

    fn transform(model: &Input) -> Self::Output {
        model.value.trim().to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, FormModel)]
#[form(
    state = InputForm,
    validation(adapter = Validator),
    transform(adapter = Transform)
)]
struct Input {
    value: String,
    #[form(validate(on_submit))]
    blocked: bool,
}

#[gpui::test]
fn prepare_submit_returns_revision_and_output_from_one_snapshot(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            InputForm::from_value_with_validation_context(
                Input {
                    value: "  ready  ".into(),
                    blocked: false,
                },
                (),
                cx,
            )
        })
    });

    cx.update(|cx| {
        let PreparedSubmit { revision, output } = form
            .update(cx, |form, cx| form.prepare_submit(cx))
            .expect("valid form prepares");
        assert_eq!(revision, form.read(cx).revision());
        assert_eq!(output, "ready");

        InputForm::VALUE.set(&form, "later".into(), cx);
        assert!(!form.update(cx, |form, cx| {
            form.rebase_if_revision(
                revision,
                Input {
                    value: output,
                    blocked: false,
                },
                cx,
            )
        }));
    });
}

#[gpui::test]
fn submit_rejects_validation_issues(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            InputForm::from_value_with_validation_context(
                Input {
                    value: "value".into(),
                    blocked: true,
                },
                (),
                cx,
            )
        })
    });

    cx.update(|cx| {
        let error = form
            .update(cx, |form, cx| form.prepare_submit(cx))
            .expect_err("blocked model is invalid");
        assert!(matches!(error, SubmitError::Validation(_)));
    });
}
