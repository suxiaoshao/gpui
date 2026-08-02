pub(crate) mod transition;

use gpui::{App, Context, EventEmitter};
use gpui_operation::Transition as _;

use crate::{
    schema::path::FieldPath,
    submit::transform::SubmitTransform,
    submit::{PreparedSubmit, SubmitError},
    validation::report::{ValidationIssue, ValidationReport},
    validation::trigger::ValidationTrigger,
    validation::{
        FormValidationRuntime, StructuralValidate, ValidationAdapter, ValidationContextValue,
        ValidationScope, ValidationSnapshot,
    },
};

use self::transition::{
    FormTransitionEffect, RebaseModel, RebaseModelIfRevision, ReplaceModel,
    ReplaceSynchronousValidation, ReplaceValidationContext, ResetModel, ValidationTransitionEffect,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormRevision(u64);

impl FormRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) fn next(self) -> Self {
        Self(self.0.checked_add(1).expect("form revision overflow"))
    }
}

#[doc(hidden)]
pub struct FormRuntime<Model, ValidationContext> {
    value: Model,
    baseline: Model,
    revision: FormRevision,
    validation_context: ValidationContext,
    validation: FormValidationRuntime,
}

impl<Model, ValidationContext> FormRuntime<Model, ValidationContext>
where
    Model: Clone + PartialEq + 'static,
    ValidationContext: ValidationContextValue,
{
    #[doc(hidden)]
    pub fn new(value: Model, validation_context: ValidationContext) -> Self {
        Self {
            baseline: value.clone(),
            value,
            revision: FormRevision::INITIAL,
            validation_context,
            validation: FormValidationRuntime::default(),
        }
    }

    #[doc(hidden)]
    pub fn value(&self) -> &Model {
        &self.value
    }

    #[doc(hidden)]
    pub fn baseline(&self) -> &Model {
        &self.baseline
    }

    #[doc(hidden)]
    pub fn revision(&self) -> FormRevision {
        self.revision
    }

    #[doc(hidden)]
    pub fn validation_context(&self) -> &ValidationContext {
        &self.validation_context
    }

    pub(crate) fn validation(&self) -> &FormValidationRuntime {
        &self.validation
    }

    pub(crate) fn validation_mut(&mut self) -> &mut FormValidationRuntime {
        &mut self.validation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormEvent {
    ValueChanged {
        path: FieldPath,
        revision: FormRevision,
    },
    ModelReplaced {
        revision: FormRevision,
    },
    ValidationChanged {
        scope: ValidationScope,
    },
}

pub trait FormState: EventEmitter<FormEvent> + Sized + 'static {
    type Model: Clone + PartialEq + StructuralValidate + crate::schema::FormModelSchema + 'static;
    type ValidationContext: ValidationContextValue;
    type ValidationAdapter: ValidationAdapter<Self::Model, Context = Self::ValidationContext>;
    type SubmitTransform: SubmitTransform<Self::Model>;

    fn from_value(value: Self::Model, cx: &mut Context<Self>) -> Self
    where
        Self::ValidationContext: Default,
    {
        Self::from_value_with_validation_context(value, Default::default(), cx)
    }

    fn from_value_with_validation_context(
        value: Self::Model,
        validation_context: Self::ValidationContext,
        cx: &mut Context<Self>,
    ) -> Self;

    #[doc(hidden)]
    fn __runtime(&self) -> &FormRuntime<Self::Model, Self::ValidationContext>;

    #[doc(hidden)]
    fn __runtime_mut(&mut self) -> &mut FormRuntime<Self::Model, Self::ValidationContext>;

    #[doc(hidden)]
    fn __validation_snapshot(
        &self,
        snapshot: &Self::Model,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        cx: &App,
    ) -> ValidationSnapshot;

    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.value().clone();
        let validation = self.__validation_snapshot(&snapshot, trigger, scope, cx);
        let effect = self
            .__runtime_mut()
            .validation_mut()
            .transition(ReplaceSynchronousValidation(validation));
        apply_validation_effect(effect, cx);
    }

    // Keep the transform output tied directly to the selected static policy.
    #[allow(clippy::type_complexity)]
    fn prepare_submit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<
        PreparedSubmit<<Self::SubmitTransform as SubmitTransform<Self::Model>>::Output>,
        SubmitError,
    > {
        let snapshot = self.value().clone();
        let revision = self.revision();
        let validation = self.__validation_snapshot(
            &snapshot,
            ValidationTrigger::Submit,
            ValidationScope::Form,
            cx,
        );
        let effect = self
            .__runtime_mut()
            .validation_mut()
            .transition(ReplaceSynchronousValidation(validation));
        apply_validation_effect(effect, cx);

        let report = self.validation_report();
        if !report.is_valid() {
            return Err(SubmitError::Validation(report));
        }
        if self.is_validating() {
            return Err(SubmitError::ValidationPending);
        }

        Ok(PreparedSubmit {
            revision,
            output: Self::SubmitTransform::transform(&snapshot),
        })
    }

    fn value(&self) -> &Self::Model {
        self.__runtime().value()
    }

    fn baseline(&self) -> &Self::Model {
        self.__runtime().baseline()
    }

    fn revision(&self) -> FormRevision {
        self.__runtime().revision()
    }

    fn validation_context(&self) -> &Self::ValidationContext {
        self.__runtime().validation_context()
    }

    fn set_validation_context(&mut self, next: Self::ValidationContext, cx: &mut Context<Self>) {
        let effect = self
            .__runtime_mut()
            .transition(ReplaceValidationContext(next));
        apply_form_effect(effect, cx);
    }

    fn replace(&mut self, value: Self::Model, cx: &mut Context<Self>) {
        let validation =
            self.__validation_snapshot(&value, ValidationTrigger::Mount, ValidationScope::Form, cx);
        let effect = self
            .__runtime_mut()
            .transition(ReplaceModel { value, validation });
        apply_form_effect(effect, cx);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        let value = self.baseline().clone();
        let validation =
            self.__validation_snapshot(&value, ValidationTrigger::Mount, ValidationScope::Form, cx);
        let effect = self.__runtime_mut().transition(ResetModel { validation });
        apply_form_effect(effect, cx);
    }

    fn rebase(&mut self, value: Self::Model, cx: &mut Context<Self>) {
        let validation =
            self.__validation_snapshot(&value, ValidationTrigger::Mount, ValidationScope::Form, cx);
        let effect = self
            .__runtime_mut()
            .transition(RebaseModel { value, validation });
        apply_form_effect(effect, cx);
    }

    fn rebase_if_revision(
        &mut self,
        expected: FormRevision,
        value: Self::Model,
        cx: &mut Context<Self>,
    ) -> bool {
        let validation =
            self.__validation_snapshot(&value, ValidationTrigger::Mount, ValidationScope::Form, cx);
        let (effect, rebased) = self.__runtime_mut().transition(RebaseModelIfRevision {
            expected,
            value,
            validation,
        });
        apply_form_effect(effect, cx);
        rebased
    }

    fn is_dirty(&self) -> bool {
        self.value() != self.baseline()
    }

    fn validation_report(&self) -> ValidationReport {
        self.__runtime().validation().report()
    }

    fn is_valid(&self) -> bool {
        self.validation_report().is_valid()
    }

    fn is_validating(&self) -> bool {
        self.__runtime().validation().is_validating()
    }

    fn is_validating_at(&self, path: &FieldPath) -> bool {
        self.__runtime().validation().is_validating_at(path)
    }

    fn errors_at(&self, path: &FieldPath) -> Vec<ValidationIssue> {
        self.validation_report().errors_at(path).cloned().collect()
    }

    fn first_error_path(&self) -> Option<FieldPath> {
        self.validation_report().first_error_path().cloned()
    }
}

pub(super) fn apply_form_effect<Form: FormState>(
    effect: FormTransitionEffect,
    cx: &mut Context<Form>,
) {
    match effect {
        FormTransitionEffect::Unchanged => {}
        FormTransitionEffect::Notify => cx.notify(),
        FormTransitionEffect::Publish(event) => {
            cx.emit(event);
            cx.notify();
        }
    }
}

pub(super) fn apply_validation_effect<Form: FormState>(
    effect: ValidationTransitionEffect,
    cx: &mut Context<Form>,
) {
    if let ValidationTransitionEffect::Changed(scope) = effect {
        cx.emit(FormEvent::ValidationChanged { scope });
        cx.notify();
    }
}
