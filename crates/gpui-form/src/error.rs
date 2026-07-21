use std::{borrow::Cow, collections::BTreeMap};

use crate::{path::FieldPath, trigger::ValidationTrigger};

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

impl From<Cow<'static, str>> for ErrorParamValue {
    fn from(value: Cow<'static, str>) -> Self {
        Self::String(value)
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

#[derive(Clone, Debug, PartialEq)]
pub enum ValidationMessage {
    Key {
        key: Cow<'static, str>,
        params: ErrorParams,
    },
    Localized(Cow<'static, str>),
}

impl ValidationMessage {
    pub fn key(key: impl Into<Cow<'static, str>>) -> Self {
        Self::Key {
            key: key.into(),
            params: ErrorParams::default(),
        }
    }

    pub fn localized(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Localized(message.into())
    }

    pub fn with_param(
        mut self,
        key: impl Into<Cow<'static, str>>,
        value: impl Into<ErrorParamValue>,
    ) -> Self {
        if let Self::Key { params, .. } = &mut self {
            params.insert(key.into(), value.into());
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValidationSource {
    Required,
    Garde,
    App(Cow<'static, str>),
    Async(Cow<'static, str>),
    Control,
    Internal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationIssue {
    pub path: Option<FieldPath>,
    pub source: ValidationSource,
    pub trigger: ValidationTrigger,
    pub code: Cow<'static, str>,
    pub message: ValidationMessage,
}

impl ValidationIssue {
    pub fn field(
        path: FieldPath,
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
    ) -> Self {
        Self {
            path: Some(path),
            source,
            trigger,
            code: code.into(),
            message,
        }
    }

    pub fn form(
        trigger: ValidationTrigger,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
    ) -> Self {
        Self {
            path: None,
            source,
            trigger,
            code: code.into(),
            message,
        }
    }

    pub fn with_param(
        mut self,
        key: impl Into<Cow<'static, str>>,
        value: impl Into<ErrorParamValue>,
    ) -> Self {
        self.message = self.message.with_param(key, value);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationReport {
    issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }

    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn errors_at(&self, path: &FieldPath) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(move |issue| issue.path.as_ref() == Some(path))
    }

    pub fn first_error_path(&self) -> Option<&FieldPath> {
        self.issues.iter().find_map(|issue| issue.path.as_ref())
    }
}
