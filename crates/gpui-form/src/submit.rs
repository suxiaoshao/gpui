pub(crate) mod transform;

use crate::{submit::transform::TransformReport, validation::report::ValidationReport};

#[derive(Clone, Debug, PartialEq)]
pub enum SubmitError {
    Validation(ValidationReport),
    ValidationPending,
    Transform(TransformReport),
}
