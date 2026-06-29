use std::marker::PhantomData;

#[cfg(feature = "validify-transform")]
use super::{SubmitTransform, TransformContext, TransformReport};

#[derive(Clone, Debug, Default)]
pub struct ValidifyTransform<T> {
    _marker: PhantomData<T>,
}

impl<T> ValidifyTransform<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "validify-transform")]
impl<T> SubmitTransform<T, T> for ValidifyTransform<T>
where
    T: Clone + validify::Modify + 'static,
{
    fn preview(&self, draft: &T, _context: &TransformContext) -> Result<T, TransformReport> {
        let mut preview = draft.clone();
        preview.modify();
        Ok(preview)
    }

    fn transform_on_submit(
        &self,
        draft: &T,
        _context: &TransformContext,
    ) -> Result<T, TransformReport> {
        let mut output = draft.clone();
        output.modify();
        Ok(output)
    }
}
