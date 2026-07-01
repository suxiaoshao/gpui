use gpui::{App, Entity, Window};

use crate::{
    AnyFormField, FieldChangeCause, FieldError, FieldMeta, FieldPath, FieldValidationReport,
    FormError, FormField, FormMeta, FormStore, FormValidationReport, NoComponentState,
    SubscriptionSet, ValidationTrigger, macro_support::GeneratedFormStore,
};

pub trait FormFragment {
    type Output;

    fn path(&self) -> &FieldPath;
    fn meta(&self) -> &FormMeta;
    fn validate(&mut self, trigger: ValidationTrigger) -> FormValidationReport;
    fn output(&self) -> Result<Self::Output, FormValidationReport>;
}

#[derive(Debug)]
pub struct FieldGroupStore<Value, Store>
where
    Value: Clone + PartialEq + 'static,
    Store: GeneratedFormStore<Value>,
{
    path: FieldPath,
    value: Value,
    default_value: Value,
    store: Entity<Store>,
    meta: FormMeta,
    field_meta: FieldMeta,
    required: bool,
    errors: Vec<FormError>,
    subscriptions: SubscriptionSet,
    revision: u64,
}

impl<Value, Store> FieldGroupStore<Value, Store>
where
    Value: Clone + PartialEq + 'static,
    Store: GeneratedFormStore<Value>,
{
    pub fn new(path: impl Into<FieldPath>, value: Value, store: Entity<Store>) -> Self {
        Self {
            path: path.into(),
            default_value: value.clone(),
            value,
            store,
            meta: FormMeta::default(),
            field_meta: FieldMeta::default(),
            required: false,
            errors: Vec::new(),
            subscriptions: SubscriptionSet::default(),
            revision: 0,
        }
    }

    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    pub fn set_path(&mut self, path: impl Into<FieldPath>) {
        self.path = path.into();
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn default_value(&self) -> &Value {
        &self.default_value
    }

    pub fn store(&self) -> Entity<Store> {
        self.store.clone()
    }

    pub fn meta(&self) -> &FormMeta {
        &self.meta
    }

    pub fn field_meta(&self) -> &FieldMeta {
        &self.field_meta
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn set_required(&mut self, required: bool) {
        self.required = required;
    }

    pub fn errors(&self) -> &[FormError] {
        &self.errors
    }

    pub fn set_errors(&mut self, errors: Vec<FormError>) {
        let is_valid = errors.iter().all(|error| !error.is_error());
        self.meta.is_valid = is_valid;
        self.field_meta.set_valid(is_valid);
        self.errors = errors;
    }

    pub fn subscriptions(&self) -> &SubscriptionSet {
        &self.subscriptions
    }

    pub fn subscriptions_mut(&mut self) -> &mut SubscriptionSet {
        &mut self.subscriptions
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn sync_from_child(&mut self, value: Value, meta: FormMeta) {
        let changed = self.value != value;
        self.value = value;
        self.meta = meta;
        self.field_meta.is_dirty = self.meta.is_dirty;
        self.field_meta.is_pristine = self.meta.is_pristine;
        self.field_meta.is_touched = self.meta.is_touched;
        self.field_meta.is_blurred = self.meta.is_blurred;
        self.field_meta.is_validating = self.meta.is_validating;
        self.field_meta.is_valid =
            self.meta.is_valid && self.errors.iter().all(|error| !error.is_error());
        self.field_meta.is_default_value = self.value == self.default_value;
        if changed {
            self.revision = self.revision.saturating_add(1);
        }
    }

    pub fn write_child_value(
        &mut self,
        value: Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.store.update(cx, |store, cx| {
            store.write_draft(value.clone(), cause, window, cx);
        });
        let meta = self.store.read(cx).meta().clone();
        self.sync_from_child(value, meta);
    }
}

impl<Value, Store> FormField for FieldGroupStore<Value, Store>
where
    Value: Clone + PartialEq + 'static,
    Store: GeneratedFormStore<Value> + FormStore<Output = Value>,
{
    type Value = Value;
    type ComponentState = NoComponentState;

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        let changed = self.value != value;
        self.value = value;
        if cause == FieldChangeCause::Reset {
            self.field_meta = FieldMeta::default();
            self.errors.clear();
        } else {
            if cause == FieldChangeCause::Blur {
                self.field_meta.mark_blurred();
            }
            if cause.marks_dirty() || changed {
                self.field_meta.mark_touched();
                self.field_meta.mark_dirty(self.value == self.default_value);
            }
        }
        if changed || cause == FieldChangeCause::NormalizeOnSubmit {
            self.revision = self.revision.saturating_add(1);
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut App) {
        let default_value = self.default_value.clone();
        self.store.update(cx, |store, cx| {
            store.reset(window, cx);
        });
        self.sync_from_child(default_value, FormMeta::default());
        self.field_meta = FieldMeta::default();
        self.errors.clear();
    }

    fn component_state(&self) -> Option<Entity<Self::ComponentState>> {
        None
    }

    fn meta(&self) -> &FieldMeta {
        &self.field_meta
    }

    fn is_required(&self) -> bool {
        self.required
    }

    fn errors(&self) -> &[FieldError] {
        &[]
    }

    fn visible_errors(&self, _form_meta: &FormMeta) -> Vec<&FieldError> {
        Vec::new()
    }

    fn set_errors(&mut self, _errors: Vec<FieldError>) {}

    fn clear_errors(&mut self) {
        self.errors.clear();
        self.field_meta.set_valid(true);
        self.meta.is_valid = true;
    }

    fn mark_touched(&mut self) {
        self.field_meta.mark_touched();
    }

    fn mark_blurred(&mut self) {
        self.field_meta.mark_blurred();
    }

    fn validate(&mut self, trigger: ValidationTrigger) -> FieldValidationReport {
        let _ = trigger;
        FieldValidationReport::default()
    }

    fn focus(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.store
            .update(cx, |store, cx| store.focus_first_error(window, cx))
    }
}

impl<Value, Store> AnyFormField for FieldGroupStore<Value, Store>
where
    Value: Clone + PartialEq + 'static,
    Store: GeneratedFormStore<Value> + FormStore<Output = Value>,
{
    fn meta(&self) -> &FieldMeta {
        &self.field_meta
    }

    fn is_required(&self) -> bool {
        self.required
    }

    fn errors(&self) -> &[FieldError] {
        &[]
    }

    fn visible_errors(&self, _form_meta: &FormMeta) -> Vec<&FieldError> {
        Vec::new()
    }

    fn set_errors(&mut self, _errors: Vec<FieldError>) {}

    fn clear_errors(&mut self) {
        self.errors.clear();
        self.field_meta.set_valid(true);
        self.meta.is_valid = true;
    }

    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.focus(window, cx)
    }
}
