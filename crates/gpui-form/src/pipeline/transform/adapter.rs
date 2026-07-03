use std::marker::PhantomData;

use crate::{FormError, FormValidationReport};

#[derive(Clone, Debug, Default)]
pub struct TransformContext {
    pub submitted: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformReport {
    errors: Vec<FormError>,
}

impl TransformReport {
    pub fn new(errors: Vec<FormError>) -> Self {
        Self { errors }
    }

    pub fn errors(&self) -> &[FormError] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<FormError> {
        self.errors
    }

    pub fn into_form_report(self) -> FormValidationReport {
        FormValidationReport::new(Vec::new(), self.errors)
    }
}

pub trait SubmitTransform<Draft, Output>: 'static {
    fn preview(&self, draft: &Draft, context: &TransformContext)
    -> Result<Output, TransformReport>;

    fn transform_on_submit(
        &self,
        draft: &Draft,
        context: &TransformContext,
    ) -> Result<Output, TransformReport>;
}

#[derive(Clone, Debug, Default)]
pub struct IdentityTransform<T> {
    _marker: PhantomData<T>,
}

impl<T> IdentityTransform<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> SubmitTransform<T, T> for IdentityTransform<T>
where
    T: Clone + 'static,
{
    fn preview(&self, draft: &T, _context: &TransformContext) -> Result<T, TransformReport> {
        Ok(draft.clone())
    }

    fn transform_on_submit(
        &self,
        draft: &T,
        _context: &TransformContext,
    ) -> Result<T, TransformReport> {
        Ok(draft.clone())
    }
}
