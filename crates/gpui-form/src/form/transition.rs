use std::{borrow::Cow, sync::Arc};

use gpui::Task;
use gpui_operation::Transition;

use crate::{
    control::{ControlId, ControlLease, ControlLifetime},
    form::{FormEvent, FormRevision, FormRuntime},
    schema::path::FieldPath,
    validation::report::ValidationIssue,
    validation::{FormValidationRuntime, ValidationScope, ValidationSnapshot},
};

#[must_use]
pub(crate) enum FormTransitionEffect {
    Unchanged,
    #[allow(dead_code)]
    Notify,
    Publish(FormEvent),
}

#[must_use]
pub(crate) enum ValidationTransitionEffect {
    Unchanged,
    Changed(ValidationScope),
}

pub(crate) struct CommitFieldValue<Model> {
    pub(crate) candidate: Model,
    pub(crate) event_path: FieldPath,
    pub(crate) validation_path: FieldPath,
    pub(crate) validation: ValidationSnapshot,
}

pub(crate) struct ReplaceModel<Model> {
    pub(crate) value: Model,
    pub(crate) validation: ValidationSnapshot,
}

pub(crate) struct ResetModel {
    pub(crate) validation: ValidationSnapshot,
}

pub(crate) struct RebaseModel<Model> {
    pub(crate) value: Model,
    pub(crate) validation: ValidationSnapshot,
}

pub(crate) struct RebaseModelIfRevision<Model> {
    pub(crate) expected: FormRevision,
    pub(crate) value: Model,
    pub(crate) validation: ValidationSnapshot,
}

pub(crate) struct ReplaceValidationContext<Context>(pub(crate) Context);

pub(crate) struct ReplaceSynchronousValidation(pub(crate) ValidationSnapshot);

pub(crate) struct NextAsyncValidationAttempt;

pub(crate) struct StartAsyncValidation {
    pub(crate) path: FieldPath,
    pub(crate) source: Cow<'static, str>,
    pub(crate) attempt: u64,
    pub(crate) task: Task<()>,
}

pub(crate) struct CompleteAsyncValidation {
    pub(crate) path: FieldPath,
    pub(crate) source: Cow<'static, str>,
    pub(crate) attempt: u64,
    pub(crate) issue: Option<ValidationIssue>,
}

pub(crate) struct CancelAsyncValidation {
    pub(crate) path: FieldPath,
    pub(crate) source: Cow<'static, str>,
}

pub(crate) struct InvalidateValidationPath(pub(crate) FieldPath);

pub(crate) struct SetControlValidationIssue {
    pub(crate) id: ControlId,
    pub(crate) lease: Arc<ControlLease>,
    pub(crate) issue: ValidationIssue,
}

pub(crate) struct ClearControlValidationIssue {
    pub(crate) id: ControlId,
}

impl<Model, Context> Transition<CommitFieldValue<Model>> for &mut FormRuntime<Model, Context>
where
    Model: Clone + PartialEq + 'static,
    Context: Clone + 'static,
{
    type Output = FormTransitionEffect;

    fn transition(self, message: CommitFieldValue<Model>) -> Self::Output {
        if self.value == message.candidate {
            return FormTransitionEffect::Unchanged;
        }

        self.value = message.candidate;
        self.revision = self.revision.next();
        let _ = self
            .validation
            .transition(InvalidateValidationPath(message.validation_path));
        let _ = self
            .validation
            .transition(ReplaceSynchronousValidation(message.validation));
        FormTransitionEffect::Publish(FormEvent::ValueChanged {
            path: message.event_path,
            revision: self.revision,
        })
    }
}

impl<Model, Context> Transition<ReplaceModel<Model>> for &mut FormRuntime<Model, Context>
where
    Model: Clone + PartialEq + 'static,
    Context: Clone + 'static,
{
    type Output = FormTransitionEffect;

    fn transition(self, message: ReplaceModel<Model>) -> Self::Output {
        self.value = message.value;
        self.revision = self.revision.next();
        self.validation.clear_for_model_replacement();
        let _ = self
            .validation
            .transition(ReplaceSynchronousValidation(message.validation));
        FormTransitionEffect::Publish(FormEvent::ModelReplaced {
            revision: self.revision,
        })
    }
}

impl<Model, Context> Transition<ResetModel> for &mut FormRuntime<Model, Context>
where
    Model: Clone + PartialEq + 'static,
    Context: Clone + 'static,
{
    type Output = FormTransitionEffect;

    fn transition(self, message: ResetModel) -> Self::Output {
        self.value = self.baseline.clone();
        self.revision = self.revision.next();
        self.validation.clear_for_model_replacement();
        let _ = self
            .validation
            .transition(ReplaceSynchronousValidation(message.validation));
        FormTransitionEffect::Publish(FormEvent::ModelReplaced {
            revision: self.revision,
        })
    }
}

impl<Model, Context> Transition<RebaseModel<Model>> for &mut FormRuntime<Model, Context>
where
    Model: Clone + PartialEq + 'static,
    Context: Clone + 'static,
{
    type Output = FormTransitionEffect;

    fn transition(self, message: RebaseModel<Model>) -> Self::Output {
        self.value = message.value.clone();
        self.baseline = message.value;
        self.revision = self.revision.next();
        self.validation.clear_for_model_replacement();
        let _ = self
            .validation
            .transition(ReplaceSynchronousValidation(message.validation));
        FormTransitionEffect::Publish(FormEvent::ModelReplaced {
            revision: self.revision,
        })
    }
}

impl<Model, Context> Transition<RebaseModelIfRevision<Model>> for &mut FormRuntime<Model, Context>
where
    Model: Clone + PartialEq + 'static,
    Context: Clone + 'static,
{
    type Output = (FormTransitionEffect, bool);

    fn transition(self, message: RebaseModelIfRevision<Model>) -> Self::Output {
        if self.revision != message.expected {
            return (FormTransitionEffect::Unchanged, false);
        }
        let effect = self.transition(RebaseModel {
            value: message.value,
            validation: message.validation,
        });
        (effect, true)
    }
}

impl<Model, Context> Transition<ReplaceValidationContext<Context>>
    for &mut FormRuntime<Model, Context>
where
    Model: Clone + PartialEq + 'static,
    Context: Clone + 'static,
{
    type Output = FormTransitionEffect;

    fn transition(self, message: ReplaceValidationContext<Context>) -> Self::Output {
        self.validation_context = message.0;
        FormTransitionEffect::Publish(FormEvent::ValidationChanged {
            scope: ValidationScope::Form,
        })
    }
}

impl Transition<ReplaceSynchronousValidation> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: ReplaceSynchronousValidation) -> Self::Output {
        let scope = message.0.scope.clone();
        if self.replace_synchronous(message.0) {
            ValidationTransitionEffect::Changed(scope)
        } else {
            ValidationTransitionEffect::Unchanged
        }
    }
}

impl Transition<StartAsyncValidation> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: StartAsyncValidation) -> Self::Output {
        let scope = ValidationScope::Field(message.path.clone());
        self.set_async_task(message.path, message.source, message.attempt, message.task);
        ValidationTransitionEffect::Changed(scope)
    }
}

impl Transition<NextAsyncValidationAttempt> for &mut FormValidationRuntime {
    type Output = u64;

    fn transition(self, _message: NextAsyncValidationAttempt) -> Self::Output {
        self.next_async_generation()
    }
}

impl Transition<CompleteAsyncValidation> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: CompleteAsyncValidation) -> Self::Output {
        let scope = ValidationScope::Field(message.path.clone());
        if self.finish_async(
            &message.path,
            &message.source,
            message.attempt,
            message.issue,
        ) {
            ValidationTransitionEffect::Changed(scope)
        } else {
            ValidationTransitionEffect::Unchanged
        }
    }
}

impl Transition<CancelAsyncValidation> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: CancelAsyncValidation) -> Self::Output {
        let scope = ValidationScope::Field(message.path.clone());
        if self.cancel_async(&message.path, &message.source) {
            ValidationTransitionEffect::Changed(scope)
        } else {
            ValidationTransitionEffect::Unchanged
        }
    }
}

impl Transition<InvalidateValidationPath> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: InvalidateValidationPath) -> Self::Output {
        let before = self.report();
        self.invalidate_path(&message.0);
        if self.report() == before {
            ValidationTransitionEffect::Unchanged
        } else {
            ValidationTransitionEffect::Changed(ValidationScope::Field(message.0))
        }
    }
}

impl Transition<SetControlValidationIssue> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: SetControlValidationIssue) -> Self::Output {
        let path = message.issue.path.clone().unwrap_or_else(FieldPath::root);
        self.set_control_issue(
            message.id,
            ControlLifetime(Arc::downgrade(&message.lease)),
            message.issue,
        );
        ValidationTransitionEffect::Changed(ValidationScope::Field(path))
    }
}

impl Transition<ClearControlValidationIssue> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;

    fn transition(self, message: ClearControlValidationIssue) -> Self::Output {
        if self.clear_control_issue(message.id) {
            ValidationTransitionEffect::Changed(ValidationScope::Form)
        } else {
            ValidationTransitionEffect::Unchanged
        }
    }
}
