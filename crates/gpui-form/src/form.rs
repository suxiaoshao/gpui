use gpui::{Context, EventEmitter};

use crate::{
    error::{ValidationIssue, ValidationReport},
    path::FieldPath,
    schema::{FormFieldId, FormModelSchema},
    submit::SubmitError,
    transform::SubmitTransform,
    trigger::ValidationTrigger,
    validation::{
        FormValidationRuntime, StructuralValidate, ValidationAdapter, ValidationContextValue,
        ValidationScope,
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormRevision(u64);

impl FormRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Self {
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
    #[doc(hidden)]
    pub fn validation(&self) -> &FormValidationRuntime {
        &self.validation
    }
    #[doc(hidden)]
    pub fn validation_mut(&mut self) -> &mut FormValidationRuntime {
        &mut self.validation
    }
    #[doc(hidden)]
    pub fn set_validation_context(&mut self, value: ValidationContext) {
        self.validation_context = value;
    }

    #[doc(hidden)]
    pub fn commit_field_value(&mut self, value: Model) -> Option<FormRevision> {
        if self.value == value {
            return None;
        }
        self.value = value;
        self.revision = self.revision.next();
        Some(self.revision)
    }

    fn replace_value(&mut self, value: Model) -> FormRevision {
        self.value = value;
        self.revision = self.revision.next();
        self.validation.clear_for_model_replacement();
        self.revision
    }

    fn reset_value(&mut self) -> FormRevision {
        self.replace_value(self.baseline.clone())
    }

    fn rebase_value(&mut self, value: Model) -> FormRevision {
        self.value = value.clone();
        self.baseline = value;
        self.revision = self.revision.next();
        self.validation.clear_for_model_replacement();
        self.revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormEvent<Field> {
    FieldChanged {
        field: Field,
        path: FieldPath,
        revision: FormRevision,
    },
    ModelReplaced {
        revision: FormRevision,
    },
    RuntimeChanged,
}

pub trait FormStore: EventEmitter<FormEvent<Self::Field>> + Sized + 'static {
    type Model: Clone + PartialEq + StructuralValidate + FormModelSchema + 'static;
    type Output: 'static;
    type Field: FormFieldId;
    type ValidationContext: ValidationContextValue;
    type ValidationAdapter: ValidationAdapter<Self::Model, Context = Self::ValidationContext>;
    type SubmitTransform: SubmitTransform<Self::Model, Output = Self::Output>;

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
    fn __validate_snapshot(
        &mut self,
        snapshot: &Self::Model,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        cx: &mut Context<Self>,
    );

    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.value().clone();
        self.__validate_snapshot(&snapshot, trigger, scope, cx);
        cx.emit(FormEvent::RuntimeChanged);
        cx.notify();
    }

    fn prepare_submit(&mut self, cx: &mut Context<Self>) -> Result<Self::Output, SubmitError> {
        let snapshot = self.value().clone();
        self.__validate_snapshot(
            &snapshot,
            ValidationTrigger::Submit,
            ValidationScope::Form,
            cx,
        );
        let report = self.validation_report();
        let is_validating = self.is_validating();
        cx.emit(FormEvent::RuntimeChanged);
        cx.notify();

        if !report.is_valid() {
            return Err(SubmitError::Validation(report));
        }
        if is_validating {
            return Err(SubmitError::ValidationPending);
        }
        Self::SubmitTransform::default()
            .transform(&snapshot)
            .map_err(SubmitError::Transform)
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
        self.__runtime_mut().set_validation_context(next);
        cx.emit(FormEvent::RuntimeChanged);
        cx.notify();
    }

    fn replace(&mut self, value: Self::Model, cx: &mut Context<Self>) {
        let revision = self.__runtime_mut().replace_value(value);
        cx.emit(FormEvent::ModelReplaced { revision });
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        let revision = self.__runtime_mut().reset_value();
        cx.emit(FormEvent::ModelReplaced { revision });
        cx.notify();
    }

    fn rebase(&mut self, value: Self::Model, cx: &mut Context<Self>) {
        let revision = self.__runtime_mut().rebase_value(value);
        cx.emit(FormEvent::ModelReplaced { revision });
        cx.notify();
    }

    fn rebase_if_revision(
        &mut self,
        expected: FormRevision,
        value: Self::Model,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.revision() != expected {
            return false;
        }
        self.rebase(value, cx);
        true
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
