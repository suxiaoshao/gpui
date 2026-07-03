use std::borrow::Cow;

use crate::{ErrorParamValue, FieldPath, ValidationIssue, ValidationSource, ValidationTrigger};

pub const REQUIRED_ERROR_CODE: &str = "required";
pub const REQUIRED_ERROR_MESSAGE_KEY: &str = "gpui-form-error-required";

pub trait RequiredValue {
    fn is_empty_value(&self) -> bool;
}

impl RequiredValue for String {
    fn is_empty_value(&self) -> bool {
        self.trim().is_empty()
    }
}

impl RequiredValue for str {
    fn is_empty_value(&self) -> bool {
        self.trim().is_empty()
    }
}

impl<T> RequiredValue for Option<T> {
    fn is_empty_value(&self) -> bool {
        self.is_none()
    }
}

impl<T> RequiredValue for Vec<T> {
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
}

impl RequiredValue for bool {
    fn is_empty_value(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequiredRule {
    pub path: FieldPath,
    pub label_key: Option<Cow<'static, str>>,
    pub message_key: Cow<'static, str>,
}

impl RequiredRule {
    pub fn new(path: FieldPath) -> Self {
        Self {
            path,
            label_key: None,
            message_key: Cow::Borrowed(REQUIRED_ERROR_MESSAGE_KEY),
        }
    }

    pub fn with_label_key(mut self, label_key: impl Into<Cow<'static, str>>) -> Self {
        self.label_key = Some(label_key.into());
        self
    }

    pub fn validate_value<T>(
        &self,
        value: &T,
        trigger: ValidationTrigger,
    ) -> Option<ValidationIssue>
    where
        T: RequiredValue + ?Sized,
    {
        value
            .is_empty_value()
            .then(|| ValidationIssue::required(self.path.clone(), self.label_key.clone(), trigger))
    }
}

pub(crate) fn required_issue(
    path: FieldPath,
    label_key: Option<Cow<'static, str>>,
    trigger: ValidationTrigger,
) -> ValidationIssue {
    let field = label_key.unwrap_or_else(|| Cow::Owned(path.to_string()));

    ValidationIssue::field(
        path,
        trigger,
        ValidationSource::Internal,
        REQUIRED_ERROR_CODE,
        REQUIRED_ERROR_MESSAGE_KEY,
    )
    .with_param("field", ErrorParamValue::String(field))
}
