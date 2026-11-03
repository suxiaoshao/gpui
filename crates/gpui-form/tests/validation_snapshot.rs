use std::{cell::Cell, rc::Rc};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{
    Form, FormSchema, ValidationMessage, ValidationRequest, ValidationSink, ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Draft {
    #[form(validate(on_submit, on_external))]
    value: String,
}

struct SnapshotValidator {
    runs: Rc<Cell<u32>>,
}

impl SnapshotValidator {
    pub(crate) fn rules(self) -> Vec<gpui_form::ValidationRule<Draft>> {
        vec![
            gpui_form::ValidationRule::new("SnapshotValidator", move |request, out| {
                self.validate(request, out)
            })
            .dependency(Draft::ROOT)
            .triggers(gpui_form::ValidationTriggers {
                submit: true,
                blur: false,
                ..Default::default()
            }),
        ]
    }
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
            .with_rules(SnapshotValidator { runs: runs.clone() }.rules())
            .unwrap()
        });

        Draft::VALUE.set(&form, "invalid".into(), cx).unwrap();
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

impl SameCodeAcrossTriggers {
    pub(crate) fn rules(self) -> Vec<gpui_form::ValidationRule<TriggerDraft>> {
        vec![
            gpui_form::ValidationRule::new("SameCodeAcrossTriggers", move |request, out| {
                self.validate(request, out)
            })
            .dependency(TriggerDraft::ROOT)
            .triggers(gpui_form::ValidationTriggers {
                submit: true,
                blur: true,
                ..Default::default()
            }),
        ]
    }
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
fn one_rule_replaces_its_results_across_triggers(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(TriggerDraft {
                value: String::new(),
            })
            .with_rules(SameCodeAcrossTriggers.rules())
            .expect("valid rules")
        });

        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        TriggerDraft::VALUE.validate(&form, ValidationTrigger::Blur, cx);

        let errors = TriggerDraft::VALUE.errors(&form, cx);
        assert_eq!(errors.len(), 1);
        assert!(
            errors
                .iter()
                .any(|issue| issue.trigger() == ValidationTrigger::Blur)
        );
    });
}

#[gpui::test]
fn reset_restores_initial_rule_activation_and_external_rechecks_submit_rules(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let form = cx.new(|_| {
            Form::new(Draft {
                value: "invalid".into(),
            })
            .with_rules(
                SnapshotValidator {
                    runs: Rc::new(Cell::new(0)),
                }
                .rules(),
            )
            .unwrap()
        });
        assert!(Draft::VALUE.errors(&form, cx).is_empty());
        assert!(form.update(cx, |f, cx| f.prepare(cx)).is_err());
        assert_eq!(Draft::VALUE.errors(&form, cx).len(), 1);
        form.update(cx, |f, cx| f.reset(gpui_form::DraftPolicy::Discard, cx))
            .unwrap();
        assert!(Draft::VALUE.errors(&form, cx).is_empty());
        form.update(cx, |f, cx| f.validate(ValidationTrigger::External, cx));
        assert_eq!(Draft::VALUE.errors(&form, cx).len(), 1);
        Draft::VALUE.set(&form, "valid".into(), cx).unwrap();
        assert!(Draft::VALUE.errors(&form, cx).is_empty());
    });
}
