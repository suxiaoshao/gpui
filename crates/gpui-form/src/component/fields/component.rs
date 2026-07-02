use std::marker::PhantomData;

use gpui::{App, Entity, Window};

use crate::{
    AnyFormField, FieldChangeCause, FieldCore, FieldError, FieldMeta, FieldPath,
    FieldValidationReport, FormComponentBinding, FormComponentEvent, FormField, FormMeta,
    ValidationTrigger,
};

pub enum FieldDraftSync<Value> {
    Parsed {
        value: Value,
        draft_changed: bool,
    },
    ParseError {
        error: FieldError,
        draft_changed: bool,
    },
}

impl<Value> FieldDraftSync<Value> {
    pub fn is_parsed(&self) -> bool {
        matches!(self, Self::Parsed { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentFieldEventKind {
    Changed,
    Focused,
    Blurred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentFieldEventOutcome {
    Changed {
        parsed: bool,
        cause: FieldChangeCause,
    },
    Focused,
    Blurred {
        parsed: bool,
    },
    Ignored,
}

impl ComponentFieldEventOutcome {
    pub fn validation_trigger(self) -> Option<ValidationTrigger> {
        match self {
            Self::Changed { parsed: true, .. } => Some(ValidationTrigger::Change),
            Self::Blurred { parsed: true } => Some(ValidationTrigger::Blur),
            _ => None,
        }
    }

    pub fn field_event_kind(self) -> Option<ComponentFieldEventKind> {
        match self {
            Self::Changed { .. } => Some(ComponentFieldEventKind::Changed),
            Self::Focused => Some(ComponentFieldEventKind::Focused),
            Self::Blurred { .. } => Some(ComponentFieldEventKind::Blurred),
            Self::Ignored => None,
        }
    }
}

pub struct ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    core: FieldCore<Value>,
    state: Entity<Binding::State>,
    default_draft: Binding::Draft,
    draft: Binding::Draft,
    parse_error: Option<FieldError>,
    writeback_depth: usize,
    _binding: PhantomData<fn() -> Binding>,
}

impl<Value, Binding> ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    pub fn new(value: Value, state: Entity<Binding::State>) -> Self {
        let draft = Binding::draft_from_value(&value);
        Self {
            core: FieldCore::new(value),
            state,
            default_draft: draft.clone(),
            draft,
            parse_error: None,
            writeback_depth: 0,
            _binding: PhantomData,
        }
    }

    pub fn state(&self) -> Entity<Binding::State> {
        self.state.clone()
    }

    pub fn core(&self) -> &FieldCore<Value> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<Value> {
        &mut self.core
    }

    pub fn draft(&self) -> &Binding::Draft {
        &self.draft
    }

    pub fn default_draft(&self) -> &Binding::Draft {
        &self.default_draft
    }

    pub fn read_component_draft(&self, cx: &App) -> Binding::Draft {
        Binding::read_draft(&self.state, cx)
    }

    pub fn read_component_value(
        &self,
        path: FieldPath,
        cx: &App,
    ) -> Result<Value, Box<FieldError>> {
        let draft = self.read_component_draft(cx);
        Binding::parse_draft(&draft, path, ValidationTrigger::Change, cx)
    }

    pub fn sync_from_state(
        &mut self,
        path: FieldPath,
        trigger: ValidationTrigger,
        cause: FieldChangeCause,
        cx: &App,
    ) -> FieldDraftSync<Value> {
        let draft = Binding::read_draft(&self.state, cx);
        self.sync_draft(draft, path, trigger, cause, cx)
    }

    pub fn prepare_submit(&mut self, path: FieldPath, cx: &App) -> Result<Value, Box<FieldError>> {
        match self.sync_from_state(
            path,
            ValidationTrigger::Submit,
            FieldChangeCause::External,
            cx,
        ) {
            FieldDraftSync::Parsed { value, .. } => Ok(value),
            FieldDraftSync::ParseError { error, .. } => Err(Box::new(error)),
        }
    }

    pub fn sync_draft(
        &mut self,
        draft: Binding::Draft,
        path: FieldPath,
        trigger: ValidationTrigger,
        cause: FieldChangeCause,
        cx: &App,
    ) -> FieldDraftSync<Value> {
        let draft_changed = self.draft != draft;
        if draft_changed {
            self.draft = draft;
        }
        match Binding::parse_draft(&self.draft, path, trigger, cx) {
            Ok(value) => {
                self.core.set_value_with_default_state(
                    value.clone(),
                    cause,
                    self.draft == self.default_draft,
                    draft_changed,
                );
                self.set_parse_error(None);
                FieldDraftSync::Parsed {
                    value,
                    draft_changed,
                }
            }
            Err(error) => {
                let error = *error;
                self.set_parse_error(Some(error.clone()));
                self.core.refresh_meta_from_default_state(
                    self.draft == self.default_draft,
                    cause,
                    draft_changed,
                );
                FieldDraftSync::ParseError {
                    error,
                    draft_changed,
                }
            }
        }
    }

    pub fn apply_component_event(
        &mut self,
        path: FieldPath,
        event: FormComponentEvent,
        cx: &App,
    ) -> ComponentFieldEventOutcome {
        if self.writeback_depth > 0 {
            return ComponentFieldEventOutcome::Ignored;
        }

        match event {
            FormComponentEvent::Change(cause) => {
                let sync = self.sync_from_state(path, ValidationTrigger::Change, cause, cx);
                ComponentFieldEventOutcome::Changed {
                    parsed: sync.is_parsed(),
                    cause,
                }
            }
            FormComponentEvent::Focus => {
                self.core.meta_mut().mark_touched();
                ComponentFieldEventOutcome::Focused
            }
            FormComponentEvent::Blur => {
                let sync =
                    self.sync_from_state(path, ValidationTrigger::Blur, FieldChangeCause::Blur, cx);
                ComponentFieldEventOutcome::Blurred {
                    parsed: sync.is_parsed(),
                }
            }
        }
    }

    pub fn write_component_value(
        &mut self,
        value: &Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.set_typed_value(value.clone(), cause);
        self.writeback_depth = self.writeback_depth.saturating_add(1);
        Binding::write_value(&self.state, value, cause, window, cx);
        self.writeback_depth = self.writeback_depth.saturating_sub(1);
    }

    pub fn set_required(&mut self, required: bool, window: &mut Window, cx: &mut App) {
        self.core.set_required(required);
        Binding::set_required(&self.state, required, window, cx);
    }

    fn set_typed_value(&mut self, value: Value, cause: FieldChangeCause) {
        let draft = Binding::draft_from_value(&value);
        let draft_changed = self.draft != draft;
        self.draft = draft;
        self.core.set_value_with_default_state(
            value,
            cause,
            self.draft == self.default_draft,
            draft_changed,
        );
        self.set_parse_error(None);
    }

    fn set_parse_error(&mut self, error: Option<FieldError>) {
        let previous_parse_error = self.parse_error.take();
        self.parse_error = error;
        let mut errors = self
            .core
            .errors()
            .iter()
            .filter(|error| {
                if let Some(previous_parse_error) = &previous_parse_error {
                    *error != previous_parse_error
                } else {
                    true
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(error) = &self.parse_error
            && !errors.iter().any(|existing| existing == error)
        {
            errors.push(error.clone());
        }
        self.core.set_errors(errors);
    }
}

impl<Value, Binding> FormField for ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
{
    type Value = Value;
    type ComponentState = Binding::State;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.set_typed_value(value, cause);
    }

    fn reset(&mut self, window: &mut Window, cx: &mut App) {
        self.core.reset();
        let value = self.core.value().clone();
        self.draft = self.default_draft.clone();
        self.parse_error = None;
        self.writeback_depth = self.writeback_depth.saturating_add(1);
        Binding::write_value(&self.state, &value, FieldChangeCause::Reset, window, cx);
        self.writeback_depth = self.writeback_depth.saturating_sub(1);
    }

    fn component_state(&self) -> Option<Entity<Self::ComponentState>> {
        Some(self.state.clone())
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
        self.parse_error = None;
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

    fn focus(&mut self, window: &mut Window, cx: &mut App) -> bool {
        Binding::focus(&self.state, window, cx)
    }
}

impl<Value, Binding> AnyFormField for ComponentFieldStore<Value, Binding>
where
    Value: Clone + PartialEq + 'static,
    Binding: FormComponentBinding<Value>,
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
        self.parse_error = None;
    }

    fn focus_any(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.focus(window, cx)
    }
}
