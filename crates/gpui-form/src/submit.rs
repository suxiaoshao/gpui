use crate::{error::ValidationReport, transform::TransformReport};

#[derive(Clone, Debug, PartialEq)]
pub enum SubmitError {
    Validation(ValidationReport),
    ValidationPending,
    Transform(TransformReport),
}
