#[cfg(not(feature = "garde-adapter"))]
use std::marker::PhantomData;

use crate::{FieldPath, FormItemId, ValidationTrigger};

use super::report::ValidationAdapterReport;

#[derive(Clone, Debug, Default)]
pub struct ValidationContext {
    pub submitted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationScope {
    Form,
    Field(FieldPath),
    Group(FieldPath),
    ArrayItem { path: FieldPath, id: FormItemId },
}

pub trait ValidationAdapter<Draft>: 'static {
    fn validate(
        &self,
        draft: &Draft,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: &ValidationContext,
    ) -> ValidationAdapterReport;
}

#[derive(Clone, Debug, Default)]
pub struct NoopValidationAdapter;

impl<Draft: 'static> ValidationAdapter<Draft> for NoopValidationAdapter {
    fn validate(
        &self,
        _draft: &Draft,
        _trigger: ValidationTrigger,
        _scope: ValidationScope,
        _context: &ValidationContext,
    ) -> ValidationAdapterReport {
        ValidationAdapterReport::default()
    }
}

#[cfg(not(feature = "garde-adapter"))]
#[derive(Clone, Debug, Default)]
pub struct GardeAdapter<T> {
    _marker: PhantomData<T>,
}

#[cfg(not(feature = "garde-adapter"))]
impl<T> GardeAdapter<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}
