use std::fmt;

use gpui_form::typed::FormFieldError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerInputPolicyError {
    NonPositiveStep,
    ReversedRange,
}

impl fmt::Display for IntegerInputPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveStep => f.write_str("integer input step must be positive"),
            Self::ReversedRange => f.write_str("integer input minimum exceeds its maximum"),
        }
    }
}

impl std::error::Error for IntegerInputPolicyError {}

#[derive(Debug)]
pub enum FormControlError {
    Field(FormFieldError),
    IntegerPolicy(IntegerInputPolicyError),
}

impl fmt::Display for FormControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(error) => error.fmt(f),
            Self::IntegerPolicy(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FormControlError {}

impl From<FormFieldError> for FormControlError {
    fn from(error: FormFieldError) -> Self {
        Self::Field(error)
    }
}

impl From<IntegerInputPolicyError> for FormControlError {
    fn from(error: IntegerInputPolicyError) -> Self {
        Self::IntegerPolicy(error)
    }
}
