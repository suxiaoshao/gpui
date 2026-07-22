use super::IntegerValue;
use crate::IntegerInputPolicyError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegerInputPolicy<N> {
    pub(crate) min: Option<N>,
    pub(crate) max: Option<N>,
    pub(crate) step: N,
}

impl<N> Default for IntegerInputPolicy<N>
where
    N: IntegerValue,
{
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            step: N::ONE,
        }
    }
}

impl<N> IntegerInputPolicy<N>
where
    N: IntegerValue,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min(mut self, min: N) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: N) -> Self {
        self.max = Some(max);
        self
    }

    pub fn step(mut self, step: N) -> Self {
        self.step = step;
        self
    }

    pub fn validate(self) -> Result<Self, IntegerInputPolicyError> {
        if self.step <= N::ZERO {
            return Err(IntegerInputPolicyError::NonPositiveStep);
        }
        if self.min.zip(self.max).is_some_and(|(min, max)| min > max) {
            return Err(IntegerInputPolicyError::ReversedRange);
        }
        Ok(self)
    }

    pub fn contains(self, value: N) -> bool {
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }

    pub fn minimum(self) -> Option<N> {
        self.min
    }
    pub fn maximum(self) -> Option<N> {
        self.max
    }
    pub fn step_value(self) -> N {
        self.step
    }
}
