pub(crate) mod transform;

use crate::{form::FormRevision, validation::report::ValidationReport};

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSubmit<Output> {
    pub revision: FormRevision,
    pub output: Output,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SubmitError {
    Validation(ValidationReport),
    ValidationPending,
}
