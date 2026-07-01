use gpui::{App, Entity, Window};

use crate::{
    ErrorVisibility, FieldChangeCause, FieldError, FieldMeta, FieldValidationReport, FormMeta,
    SubscriptionSet, ValidationTrigger,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationTriggers {
    pub on_mount: bool,
    pub on_change: bool,
    pub on_blur: bool,
    pub on_submit: bool,
    pub on_dynamic: bool,
}

impl Default for ValidationTriggers {
    fn default() -> Self {
        Self {
            on_mount: false,
            on_change: false,
            on_blur: false,
            on_submit: true,
            on_dynamic: false,
        }
    }
}

impl ValidationTriggers {
    pub fn contains(&self, trigger: ValidationTrigger) -> bool {
        match trigger {
            ValidationTrigger::Mount => self.on_mount,
            ValidationTrigger::Change => self.on_change,
            ValidationTrigger::Blur => self.on_blur,
            ValidationTrigger::Submit => self.on_submit,
            ValidationTrigger::Dynamic => self.on_dynamic,
        }
    }
}

pub trait FormField {
    type Value: Clone + PartialEq + 'static;
    type ComponentState: 'static;

    fn value(&self) -> &Self::Value;
    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause);
    fn reset(&mut self, window: &mut Window, cx: &mut App);
    fn component_state(&self) -> Option<Entity<Self::ComponentState>>;
    fn meta(&self) -> &FieldMeta;
    fn is_required(&self) -> bool;
    fn errors(&self) -> &[FieldError];
    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError>;
    fn set_errors(&mut self, errors: Vec<FieldError>);
    fn clear_errors(&mut self);
    fn mark_touched(&mut self);
    fn mark_blurred(&mut self);
    fn validate(&mut self, trigger: ValidationTrigger) -> FieldValidationReport;
    fn focus(&mut self, window: &mut Window, cx: &mut App) -> bool;
}

pub trait AnyFormField {
    fn meta(&self) -> &FieldMeta;
    fn is_required(&self) -> bool;
    fn errors(&self) -> &[FieldError];
    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError>;
    fn set_errors(&mut self, errors: Vec<FieldError>);
    fn clear_errors(&mut self);
    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool;
}

#[derive(Debug)]
pub struct FieldCore<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
    default_value: T,
    meta: FieldMeta,
    required: bool,
    errors: Vec<FieldError>,
    visibility: ErrorVisibility,
    validation_triggers: ValidationTriggers,
    subscriptions: SubscriptionSet,
    revision: u64,
}

impl<T> FieldCore<T>
where
    T: Clone + PartialEq + 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            default_value: value.clone(),
            value,
            meta: FieldMeta::default(),
            required: false,
            errors: Vec::new(),
            visibility: ErrorVisibility::default(),
            validation_triggers: ValidationTriggers::default(),
            subscriptions: SubscriptionSet::default(),
            revision: 0,
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn default_value(&self) -> &T {
        &self.default_value
    }

    pub fn meta(&self) -> &FieldMeta {
        &self.meta
    }

    pub fn meta_mut(&mut self) -> &mut FieldMeta {
        &mut self.meta
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn set_required(&mut self, required: bool) {
        self.required = required;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn visibility(&self) -> ErrorVisibility {
        self.visibility
    }

    pub fn set_visibility(&mut self, visibility: ErrorVisibility) {
        self.visibility = visibility;
    }

    pub fn validation_triggers(&self) -> &ValidationTriggers {
        &self.validation_triggers
    }

    pub fn set_validation_triggers(&mut self, triggers: ValidationTriggers) {
        self.validation_triggers = triggers;
    }

    pub fn subscriptions(&self) -> &SubscriptionSet {
        &self.subscriptions
    }

    pub fn subscriptions_mut(&mut self) -> &mut SubscriptionSet {
        &mut self.subscriptions
    }

    pub fn set_value(&mut self, value: T, cause: FieldChangeCause) {
        let changed = self.value != value;
        self.value = value;

        if cause == FieldChangeCause::Reset {
            self.meta = FieldMeta::default();
            self.errors.clear();
        } else {
            if cause == FieldChangeCause::Blur {
                self.meta.mark_blurred();
            }
            if cause.marks_dirty() || changed {
                self.meta.mark_touched();
                self.meta.mark_dirty(self.value == self.default_value);
            }
        }

        if changed || cause == FieldChangeCause::NormalizeOnSubmit {
            self.revision = self.revision.saturating_add(1);
        }
    }

    pub fn reset(&mut self) {
        self.value = self.default_value.clone();
        self.meta = FieldMeta::default();
        self.errors.clear();
        self.revision = self.revision.saturating_add(1);
    }

    pub fn reset_to(&mut self, value: T) {
        self.default_value = value.clone();
        self.value = value;
        self.meta = FieldMeta::default();
        self.errors.clear();
        self.revision = self.revision.saturating_add(1);
    }

    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    pub fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        if self.visibility.is_visible(&self.meta, form_meta) {
            self.errors.iter().collect()
        } else {
            Vec::new()
        }
    }

    pub fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.errors = errors;
    }

    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }
}

#[derive(Debug)]
pub struct NoComponentState;

#[derive(Debug)]
pub struct ValueFieldStore<T>
where
    T: Clone + PartialEq + 'static,
{
    core: FieldCore<T>,
}

impl<T> ValueFieldStore<T>
where
    T: Clone + PartialEq + 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            core: FieldCore::new(value),
        }
    }

    pub fn core(&self) -> &FieldCore<T> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<T> {
        &mut self.core
    }
}

impl<T> FormField for ValueFieldStore<T>
where
    T: Clone + PartialEq + 'static,
{
    type Value = T;
    type ComponentState = NoComponentState;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.core.set_value(value, cause);
    }

    fn reset(&mut self, _window: &mut Window, _cx: &mut App) {
        self.core.reset();
    }

    fn component_state(&self) -> Option<Entity<Self::ComponentState>> {
        None
    }

    fn meta(&self) -> &FieldMeta {
        self.core.meta()
    }

    fn is_required(&self) -> bool {
        self.core.is_required()
    }

    fn errors(&self) -> &[FieldError] {
        self.core.errors()
    }

    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        self.core.visible_errors(form_meta)
    }

    fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.core.set_errors(errors);
    }

    fn clear_errors(&mut self) {
        self.core.clear_errors();
    }

    fn mark_touched(&mut self) {
        self.core.meta_mut().mark_touched();
    }

    fn mark_blurred(&mut self) {
        self.core.meta_mut().mark_blurred();
    }

    fn validate(&mut self, _trigger: ValidationTrigger) -> FieldValidationReport {
        FieldValidationReport::new(self.core.errors().to_vec())
    }

    fn focus(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

impl<T> AnyFormField for ValueFieldStore<T>
where
    T: Clone + PartialEq + 'static,
{
    fn meta(&self) -> &FieldMeta {
        self.core.meta()
    }

    fn is_required(&self) -> bool {
        self.core.is_required()
    }

    fn errors(&self) -> &[FieldError] {
        self.core.errors()
    }

    fn visible_errors(&self, form_meta: &FormMeta) -> Vec<&FieldError> {
        self.core.visible_errors(form_meta)
    }

    fn set_errors(&mut self, errors: Vec<FieldError>) {
        self.core.set_errors(errors);
    }

    fn clear_errors(&mut self) {
        self.core.clear_errors();
    }

    fn focus_any(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}
