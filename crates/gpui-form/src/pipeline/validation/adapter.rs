#[cfg(not(feature = "garde-adapter"))]
use std::marker::PhantomData;

use gpui::App;

use crate::{FieldPath, FormItemId, ValidationTrigger};

use super::report::ValidationAdapterReport;

#[derive(Clone, Debug, Default)]
pub struct NoValidationContext;

pub trait ValidationContextValue: Clone + 'static {}

impl<T> ValidationContextValue for T where T: Clone + 'static {}

#[derive(Clone, Copy, Debug)]
pub struct ValidationContext<'a, C = NoValidationContext>
where
    C: ValidationContextValue,
{
    pub submitted: bool,
    pub external: &'a C,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationScope {
    Form,
    Field(FieldPath),
    Group(FieldPath),
    ArrayItem { path: FieldPath, id: FormItemId },
}

pub trait ValidationAdapter<Draft>: 'static {
    type Context: ValidationContextValue;

    fn validate(
        &self,
        draft: &Draft,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport;
}

#[derive(Clone, Debug, Default)]
pub struct NoopValidationAdapter;

impl<Draft: 'static> ValidationAdapter<Draft> for NoopValidationAdapter {
    type Context = NoValidationContext;

    fn validate(
        &self,
        _draft: &Draft,
        _trigger: ValidationTrigger,
        _scope: ValidationScope,
        _context: ValidationContext<'_, Self::Context>,
        _cx: &App,
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
