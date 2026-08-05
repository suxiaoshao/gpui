use std::fmt;

use gpui_form::ResolveError;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormIntegerInputBuildError {
    Resolve(ResolveError),
    Policy(IntegerInputPolicyError),
}

impl fmt::Display for FormIntegerInputBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve(error) => error.fmt(f),
            Self::Policy(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FormIntegerInputBuildError {}

impl From<ResolveError> for FormIntegerInputBuildError {
    fn from(error: ResolveError) -> Self {
        Self::Resolve(error)
    }
}

impl From<IntegerInputPolicyError> for FormIntegerInputBuildError {
    fn from(error: IntegerInputPolicyError) -> Self {
        Self::Policy(error)
    }
}
