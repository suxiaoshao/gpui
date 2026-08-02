use std::marker::PhantomData;

pub trait SubmitTransform<Model>: 'static {
    type Output: 'static;

    fn transform(model: &Model) -> Self::Output;
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

    fn transform(model: &T) -> Self::Output {
        model.clone()
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

    fn transform(model: &T) -> Self::Output {
        let mut output = model.clone();
        validify::Modify::modify(&mut output);
        output
    }
}
