use std::{borrow::Cow, collections::BTreeMap};

use crate::{FieldPath, ValidationTrigger};

pub type ErrorParams = BTreeMap<Cow<'static, str>, ErrorParamValue>;

#[derive(Clone, Debug, PartialEq)]
pub enum ErrorParamValue {
    String(Cow<'static, str>),
    Integer(i64),
    Unsigned(u64),
    Float(f64),
    Bool(bool),
}

impl From<&'static str> for ErrorParamValue {
    fn from(value: &'static str) -> Self {
        Self::String(Cow::Borrowed(value))
    }
}

impl From<String> for ErrorParamValue {
    fn from(value: String) -> Self {
        Self::String(Cow::Owned(value))
    }
}

impl From<bool> for ErrorParamValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for ErrorParamValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<u64> for ErrorParamValue {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

impl From<f64> for ErrorParamValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

impl ValidationSeverity {
    pub fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValidationSource {
    Garde,
    Validify,
    App(Cow<'static, str>),
    Internal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldError {
    pub path: FieldPath,
    pub trigger: ValidationTrigger,
    pub severity: ValidationSeverity,
    pub source: ValidationSource,
    pub code: Cow<'static, str>,
    pub message_key: Cow<'static, str>,
    pub params: ErrorParams,
}

impl FieldError {
    pub fn new(
        path: FieldPath,
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message_key: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            path,
            trigger,
            severity: ValidationSeverity::Error,
            source,
            code: code.into(),
            message_key: message_key.into(),
            params: ErrorParams::default(),
        }
    }

    pub fn new_for_field(
        field: &'static str,
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message_key: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(
            FieldPath::from_static(field),
            trigger,
            source,
            code,
            message_key,
        )
    }

    pub fn with_param(
        mut self,
        key: impl Into<Cow<'static, str>>,
        value: impl Into<ErrorParamValue>,
    ) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity.is_error()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormError {
    pub path: Option<FieldPath>,
    pub trigger: ValidationTrigger,
    pub severity: ValidationSeverity,
    pub source: ValidationSource,
    pub code: Cow<'static, str>,
    pub message_key: Cow<'static, str>,
    pub params: ErrorParams,
}

impl FormError {
    pub fn new(
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message_key: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            path: None,
            trigger,
            severity: ValidationSeverity::Error,
            source,
            code: code.into(),
            message_key: message_key.into(),
            params: ErrorParams::default(),
        }
    }

    pub fn with_path(mut self, path: FieldPath) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_param(
        mut self,
        key: impl Into<Cow<'static, str>>,
        value: impl Into<ErrorParamValue>,
    ) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity.is_error()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldValidationReport {
    errors: Vec<FieldError>,
}

impl FieldValidationReport {
    pub fn new(errors: Vec<FieldError>) -> Self {
        Self { errors }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.iter().all(|error| !error.is_error())
    }

    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<FieldError> {
        self.errors
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormValidationReport {
    field_errors: Vec<FieldError>,
    form_errors: Vec<FormError>,
}

impl FormValidationReport {
    pub fn new(field_errors: Vec<FieldError>, form_errors: Vec<FormError>) -> Self {
        Self {
            field_errors,
            form_errors,
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_valid(&self) -> bool {
        self.field_errors.iter().all(|error| !error.is_error())
            && self.form_errors.iter().all(|error| !error.is_error())
    }

    pub fn field_errors(&self) -> &[FieldError] {
        &self.field_errors
    }

    pub fn form_errors(&self) -> &[FormError] {
        &self.form_errors
    }

    pub fn push_field_error(&mut self, error: FieldError) {
        self.field_errors.push(error);
    }

    pub fn push_form_error(&mut self, error: FormError) {
        self.form_errors.push(error);
    }

    pub fn merge(&mut self, other: Self) {
        self.field_errors.extend(other.field_errors);
        self.form_errors.extend(other.form_errors);
    }

    pub fn first_field_error(&self) -> Option<&FieldError> {
        self.field_errors.iter().find(|error| error.is_error())
    }

    pub fn into_result(self) -> Result<(), Self> {
        if self.is_valid() { Ok(()) } else { Err(self) }
    }

    pub fn strip_field_prefix(&self, prefix: &FieldPath) -> Self {
        let field_errors = self
            .field_errors
            .iter()
            .filter_map(|error| {
                let mut error = error.clone();
                error.path = error.path.strip_prefix(prefix)?;
                Some(error)
            })
            .collect();

        Self {
            field_errors,
            form_errors: Vec::new(),
        }
    }
}
