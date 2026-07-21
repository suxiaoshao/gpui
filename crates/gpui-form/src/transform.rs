use std::marker::PhantomData;

use crate::error::ValidationIssue;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformReport {
    issues: Vec<ValidationIssue>,
}

impl TransformReport {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }
}

pub trait SubmitTransform<Model>: Default + 'static {
    type Output: 'static;

    fn transform(&self, model: &Model) -> Result<Self::Output, TransformReport>;
}

#[derive(Clone, Debug)]
pub struct IdentityTransform<T> {
    marker: PhantomData<fn() -> T>,
}

impl<T> Default for IdentityTransform<T> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<T> SubmitTransform<T> for IdentityTransform<T>
where
    T: Clone + 'static,
{
    type Output = T;

    fn transform(&self, model: &T) -> Result<Self::Output, TransformReport> {
        Ok(model.clone())
    }
}

#[cfg(feature = "validify-transform")]
#[derive(Clone, Debug)]
pub struct ValidifyTransform<T> {
    marker: PhantomData<fn() -> T>,
}

#[cfg(feature = "validify-transform")]
impl<T> Default for ValidifyTransform<T> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

#[cfg(feature = "validify-transform")]
impl<T> SubmitTransform<T> for ValidifyTransform<T>
where
    T: Clone + validify::Modify + 'static,
{
    type Output = T;

    fn transform(&self, model: &T) -> Result<Self::Output, TransformReport> {
        let mut output = model.clone();
        validify::Modify::modify(&mut output);
        Ok(output)
    }
}
