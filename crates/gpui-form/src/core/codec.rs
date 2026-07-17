use std::{borrow::Cow, marker::PhantomData};

use crate::{
    ErrorParams, FieldChangeCause, FieldCore, FieldError, FieldMeta, FieldPath,
    FieldValidationReport, FormField, FormMeta, ValidationTrigger,
};
use gpui::{App, Window};

/// Converts the editable draft representation of a field into its submitted
/// value. The codec is deliberately independent of any UI component or
/// catalog; those concerns belong to the caller/adapter layer.
pub trait FieldCodec<Value>: 'static
where
    Value: Clone + PartialEq + 'static,
{
    type Draft: Clone + PartialEq + 'static;

    fn draft_from_value(value: &Value) -> Self::Draft;

    fn parse(draft: &Self::Draft) -> Result<Value, FieldCodecError>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdentityCodec<Value>(PhantomData<fn() -> Value>);

impl<Value> FieldCodec<Value> for IdentityCodec<Value>
where
    Value: Clone + PartialEq + 'static,
{
    type Draft = Value;

    fn draft_from_value(value: &Value) -> Self::Draft {
        value.clone()
    }

    fn parse(draft: &Self::Draft) -> Result<Value, FieldCodecError> {
        Ok(draft.clone())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldCodecError {
    pub code: Cow<'static, str>,
    pub message_key: Cow<'static, str>,
    pub params: ErrorParams,
}

/// The result of applying a raw draft to a field.
///
/// A raw draft can change without producing a parsed value (for example while
/// a number input contains only `-`). The form keeps that raw draft visible to
/// adapters while retaining the last valid typed value for validation/submit.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftUpdate<Value> {
    draft_changed: bool,
    value: Option<Value>,
    value_changed: bool,
    parse_error: Option<FieldCodecError>,
}

impl<Value> DraftUpdate<Value> {
    pub fn draft_changed(&self) -> bool {
        self.draft_changed
    }

    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    pub fn value_changed(&self) -> bool {
        self.value_changed
    }

    pub fn is_valid(&self) -> bool {
        self.parse_error.is_none()
    }

    pub fn parse_error(&self) -> Option<&FieldCodecError> {
        self.parse_error.as_ref()
    }
}

impl FieldCodecError {
    pub fn new(
        code: impl Into<Cow<'static, str>>,
        message_key: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            code: code.into(),
            message_key: message_key.into(),
            params: ErrorParams::default(),
        }
    }

    pub fn with_param(
        mut self,
        key: impl Into<Cow<'static, str>>,
        value: impl Into<crate::ErrorParamValue>,
    ) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn into_field_error(self, path: FieldPath, trigger: ValidationTrigger) -> FieldError {
        FieldError {
            path,
            trigger,
            severity: crate::ValidationSeverity::Error,
            source: crate::ValidationSource::Internal,
            code: self.code,
            message_key: self.message_key,
            params: self.params,
        }
    }
}

/// A field store that keeps the user-editable draft separate from the
/// submitted/domain value.
#[derive(Debug)]
pub struct DraftFieldStore<Value, Codec = IdentityCodec<Value>>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    core: FieldCore<Value>,
    baseline: Codec::Draft,
    draft: Codec::Draft,
    parse_error: Option<FieldCodecError>,
}

impl<Value, Codec> DraftFieldStore<Value, Codec>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    pub fn draft(&self) -> &Codec::Draft {
        &self.draft
    }

    pub fn value(&self) -> &Value {
        self.core.value()
    }

    pub fn baseline(&self) -> &Codec::Draft {
        &self.baseline
    }

    pub fn parse_error(&self) -> Option<&FieldCodecError> {
        self.parse_error.as_ref()
    }

    pub fn core(&self) -> &FieldCore<Value> {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut FieldCore<Value> {
        &mut self.core
    }

    pub fn set_value(&mut self, value: Value, cause: FieldChangeCause) {
        <Self as FormField>::set_value(self, value, cause);
    }

    pub fn set_user_draft(&mut self, draft: Codec::Draft) -> DraftUpdate<Value> {
        self.apply_draft(draft, FieldChangeCause::UserInput)
    }

    pub fn set_draft(&mut self, draft: Codec::Draft, cause: FieldChangeCause) -> bool {
        self.apply_draft(draft, cause).draft_changed
    }

    fn apply_draft(&mut self, draft: Codec::Draft, cause: FieldChangeCause) -> DraftUpdate<Value> {
        let changed = self.draft != draft;
        let previous_value = self.core.value().clone();
        self.draft = draft;

        let (value, parse_error) = match Codec::parse(&self.draft) {
            Ok(value) => {
                self.parse_error = None;
                let is_default = self.draft == self.baseline;
                self.core
                    .set_value_with_default_state(value.clone(), cause, is_default, changed);
                (Some(value), None)
            }
            Err(error) => {
                self.parse_error = Some(error.clone());
                self.core.refresh_meta_from_default_state(
                    self.draft == self.baseline,
                    cause,
                    changed,
                );
                (None, Some(error))
            }
        };

        DraftUpdate {
            draft_changed: changed,
            value_changed: value.as_ref().is_some_and(|value| previous_value != *value),
            value,
            parse_error,
        }
    }

    pub fn reset_to(&mut self, value: Value) {
        self.core.reset_to(value.clone());
        self.baseline = Codec::draft_from_value(&value);
        self.draft = self.baseline.clone();
        self.parse_error = None;
    }

    pub fn replace_baseline(&mut self, value: Value) -> bool {
        let next_baseline = Codec::draft_from_value(&value);
        let changed = self.draft != next_baseline;
        self.reset_to(value);
        changed
    }

    pub fn prepare_submit(
        &mut self,
        path: FieldPath,
        trigger: ValidationTrigger,
    ) -> Result<Value, Box<FieldError>> {
        match Codec::parse(&self.draft) {
            Ok(value) => {
                self.parse_error = None;
                let normalized_draft = Codec::draft_from_value(&value);
                let changed = self.draft != normalized_draft;
                self.draft = normalized_draft;
                self.core.set_value_with_default_state(
                    value.clone(),
                    FieldChangeCause::NormalizeOnSubmit,
                    self.draft == self.baseline,
                    changed,
                );
                Ok(value)
            }
            Err(error) => {
                self.parse_error = Some(error.clone());
                Err(Box::new(error.into_field_error(path, trigger)))
            }
        }
    }

    pub fn prepare_submit_at(&mut self, path: FieldPath) -> Result<Value, Box<FieldError>> {
        self.prepare_submit(path, ValidationTrigger::Submit)
    }

    pub fn validation_report_at(
        &self,
        path: FieldPath,
        trigger: ValidationTrigger,
    ) -> FieldValidationReport {
        let mut errors = self.core.errors().to_vec();
        if let Some(error) = &self.parse_error {
            errors.push(error.clone().into_field_error(path, trigger));
        }
        FieldValidationReport::new(errors)
    }
}

impl<Value, Codec> DraftFieldStore<Value, Codec>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    pub fn new(value: Value) -> Self {
        let draft = Codec::draft_from_value(&value);
        Self {
            core: FieldCore::new(value),
            baseline: draft.clone(),
            draft,
            parse_error: None,
        }
    }
}

impl<Value, Codec> FormField for DraftFieldStore<Value, Codec>
where
    Value: Clone + PartialEq + 'static,
    Codec: FieldCodec<Value>,
{
    type Value = Value;

    fn value(&self) -> &Self::Value {
        self.core.value()
    }

    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause) {
        self.core.set_value(value.clone(), cause);
        self.draft = Codec::draft_from_value(&value);
        self.parse_error = None;
    }

    fn reset(&mut self, _window: &mut Window, _cx: &mut App) {
        let value = self.core.default_value().clone();
        self.reset_to(value);
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

    fn validate(&mut self, trigger: ValidationTrigger) -> FieldValidationReport {
        self.validation_report_at(FieldPath::root(), trigger)
    }

    fn focus(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct NumberCodec;

    impl FieldCodec<u32> for NumberCodec {
        type Draft = String;

        fn draft_from_value(value: &u32) -> Self::Draft {
            value.to_string()
        }

        fn parse(draft: &Self::Draft) -> Result<u32, FieldCodecError> {
            draft
                .parse()
                .map_err(|_| FieldCodecError::new("parse", "test-parse"))
        }
    }

    #[test]
    fn draft_store_keeps_invalid_raw_draft_until_submit() {
        let mut field = DraftFieldStore::<u32, NumberCodec>::new(10);
        assert_eq!(field.draft(), "10");

        field.set_draft("-".to_string(), FieldChangeCause::UserInput);
        assert_eq!(field.draft(), "-");
        assert_eq!(*field.value(), 10);
        assert!(field.parse_error().is_some());

        let error = field
            .prepare_submit_at(FieldPath::from_static("port"))
            .expect_err("invalid draft should fail submit");
        assert_eq!(error.path, FieldPath::from_static("port"));
        assert_eq!(error.code, "parse");
    }

    #[test]
    fn draft_store_normalizes_successful_parse() {
        let mut field = DraftFieldStore::<u32, NumberCodec>::new(10);
        field.set_draft("12".to_string(), FieldChangeCause::UserInput);
        assert_eq!(*field.value(), 12);

        let value = field
            .prepare_submit_at(FieldPath::from_static("port"))
            .expect("valid draft should submit");
        assert_eq!(value, 12);
        assert_eq!(field.draft(), "12");
        assert!(field.parse_error().is_none());
    }

    #[test]
    fn equivalent_value_with_different_raw_draft_stays_dirty() {
        let mut field = DraftFieldStore::<u32, NumberCodec>::new(1);
        field.set_draft("01".to_string(), FieldChangeCause::UserInput);

        assert_eq!(*field.value(), 1);
        assert_eq!(field.draft(), "01");
        assert!(field.meta().is_dirty);
    }

    #[test]
    fn user_draft_update_reports_invalid_raw_input_without_losing_typed_value() {
        let mut field = DraftFieldStore::<u32, NumberCodec>::new(10);
        let update = field.set_user_draft("-".to_string());

        assert!(update.draft_changed());
        assert!(!update.value_changed());
        assert!(!update.is_valid());
        assert_eq!(field.value(), &10);
    }

    #[test]
    fn replace_baseline_discards_draft_and_clears_meta() {
        let mut field = DraftFieldStore::<u32, NumberCodec>::new(10);
        field.set_draft("12".to_string(), FieldChangeCause::UserInput);
        assert!(field.meta().is_dirty);

        assert!(field.replace_baseline(20));

        assert_eq!(field.draft(), "20");
        assert_eq!(field.baseline(), "20");
        assert_eq!(field.value(), &20);
        assert!(field.meta().is_pristine());
        assert!(field.parse_error().is_none());
    }
}
