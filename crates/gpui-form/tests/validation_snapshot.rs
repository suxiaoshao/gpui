use std::{cell::Cell, rc::Rc};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{
    Form, FormSchema, ValidationMessage, ValidationRequest, ValidationSink, ValidationTrigger,
    Validator,
};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Draft {
    #[form(validate(on_submit, on_external))]
    value: String,
}

struct SnapshotValidator {
    runs: Rc<Cell<u32>>,
}

impl Validator<Draft> for SnapshotValidator {
    fn validate(&self, request: ValidationRequest<'_, Draft>, out: &mut ValidationSink<'_, Draft>) {
        self.runs.set(self.runs.get() + 1);
        if request.model().value == "invalid" && request.includes(&Draft::VALUE) {
            out.at(Draft::VALUE)
                .error("invalid", ValidationMessage::literal("invalid snapshot"));
        }
    }
}

#[gpui::test]
fn validation_request_owns_the_model_snapshot_and_default_validation_is_submit_only(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let runs = Rc::new(Cell::new(0));
        let form = cx.new(|_| {
            Form::new(Draft {
                value: "initial".into(),
            })
            .with_validator(SnapshotValidator { runs: runs.clone() })
        });

        Draft::VALUE.set(&form, "invalid".into(), cx);
        assert_eq!(
            runs.get(),
            0,
            "ordinary set must not run business validation"
        );

        form.update(cx, |form, cx| {
            form.validate(ValidationTrigger::External, cx);
        });
        assert_eq!(runs.get(), 1);
        assert_eq!(Draft::VALUE.errors(&form, cx).len(), 1);
    });
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct TriggerDraft {
    #[form(validate(on_blur, on_submit))]
    value: String,
}

struct SameCodeAcrossTriggers;

impl Validator<TriggerDraft> for SameCodeAcrossTriggers {
    fn validate(
        &self,
        request: ValidationRequest<'_, TriggerDraft>,
        out: &mut ValidationSink<'_, TriggerDraft>,
    ) {
        if request.includes(&TriggerDraft::VALUE) {
            out.at(TriggerDraft::VALUE)
                .error("same-code", ValidationMessage::literal("invalid"));
        }
    }
}

#[gpui::test]
fn validation_results_with_the_same_code_remain_independent_across_triggers(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(TriggerDraft {
                value: String::new(),
            })
            .with_validator(SameCodeAcrossTriggers)
        });

        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        TriggerDraft::VALUE.validate(&form, ValidationTrigger::Blur, cx);

        let errors = TriggerDraft::VALUE.errors(&form, cx);
        assert_eq!(errors.len(), 2);
        assert!(
            errors
                .iter()
                .any(|issue| issue.trigger() == ValidationTrigger::Submit)
        );
        assert!(
            errors
                .iter()
                .any(|issue| issue.trigger() == ValidationTrigger::Blur)
        );
    });
}
