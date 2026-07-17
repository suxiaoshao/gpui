use std::{marker::PhantomData, str::FromStr};

use gpui::Entity;
use gpui_component::input::{InputState, NumberInput, StepAction};
use gpui_form::{FieldCodec, FieldCodecError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberInputKind {
    SignedInteger,
    UnsignedInteger,
    Float,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberInputPolicy {
    pub kind: NumberInputKind,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub component_step: Option<f64>,
}

impl NumberInputPolicy {
    pub const fn signed_integer(
        min: Option<f64>,
        max: Option<f64>,
        component_step: Option<f64>,
    ) -> Self {
        Self {
            kind: NumberInputKind::SignedInteger,
            min,
            max,
            component_step,
        }
    }

    pub const fn unsigned_integer(max: Option<f64>, component_step: Option<f64>) -> Self {
        Self {
            kind: NumberInputKind::UnsignedInteger,
            min: Some(0.),
            max,
            component_step,
        }
    }

    pub const fn float() -> Self {
        Self {
            kind: NumberInputKind::Float,
            min: None,
            max: None,
            component_step: Some(1.),
        }
    }

    pub fn allows_input(self, text: &str) -> bool {
        match self.kind {
            NumberInputKind::SignedInteger => allows_signed_integer(text),
            NumberInputKind::UnsignedInteger => allows_unsigned_integer(text),
            NumberInputKind::Float => allows_float(text),
        }
    }
}

fn allows_unsigned_integer(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit())
}

fn allows_signed_integer(text: &str) -> bool {
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    digits.is_empty() || digits.chars().all(|ch| ch.is_ascii_digit())
}

fn allows_float(text: &str) -> bool {
    let body = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    if body.is_empty() {
        return true;
    }

    let mut dot_seen = false;
    for ch in body.chars() {
        match ch {
            '0'..='9' => {}
            '.' if !dot_seen => dot_seen = true,
            _ => return false,
        }
    }
    true
}

pub trait NumberFieldValue: Clone + PartialEq + ToString + FromStr + 'static {
    fn input_policy() -> NumberInputPolicy;

    fn step_draft(_draft: &str, _action: StepAction) -> Option<String> {
        None
    }
}

/// Codec for numeric controls whose UI draft is edited as text.
/// Range, step and presentation policy stay in the component configuration;
/// this type only owns text-to-domain parsing for the form field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NumberCodec<N>(PhantomData<fn() -> N>);

impl<N> FieldCodec<N> for NumberCodec<N>
where
    N: NumberFieldValue,
{
    type Draft = String;

    fn draft_from_value(value: &N) -> Self::Draft {
        value.to_string()
    }

    fn parse(draft: &Self::Draft) -> Result<N, FieldCodecError> {
        draft
            .parse::<N>()
            .map_err(|_| FieldCodecError::new("parse", "gpui-form-error-number-parse"))
    }
}

macro_rules! impl_signed_integer_number {
    ($ty:ty, safe_step, $min:expr, $max:expr) => {
        impl NumberFieldValue for $ty {
            fn input_policy() -> NumberInputPolicy {
                NumberInputPolicy::signed_integer(Some($min as f64), Some($max as f64), Some(1.))
            }
        }
    };
    ($ty:ty, binding_step) => {
        impl NumberFieldValue for $ty {
            fn input_policy() -> NumberInputPolicy {
                NumberInputPolicy::signed_integer(None, None, None)
            }

            fn step_draft(draft: &str, action: StepAction) -> Option<String> {
                let current = draft.trim().parse::<$ty>().unwrap_or(0);
                let next = match action {
                    StepAction::Increment => current.checked_add(1),
                    StepAction::Decrement => current.checked_sub(1),
                }?;
                Some(next.to_string())
            }
        }
    };
}

macro_rules! impl_unsigned_integer_number {
    ($ty:ty, safe_step, $max:expr) => {
        impl NumberFieldValue for $ty {
            fn input_policy() -> NumberInputPolicy {
                NumberInputPolicy::unsigned_integer(Some($max as f64), Some(1.))
            }
        }
    };
    ($ty:ty, binding_step) => {
        impl NumberFieldValue for $ty {
            fn input_policy() -> NumberInputPolicy {
                NumberInputPolicy::unsigned_integer(None, None)
            }

            fn step_draft(draft: &str, action: StepAction) -> Option<String> {
                let current = draft.trim().parse::<$ty>().unwrap_or(0);
                let next = match action {
                    StepAction::Increment => current.checked_add(1),
                    StepAction::Decrement => current.checked_sub(1),
                }?;
                Some(next.to_string())
            }
        }
    };
}

impl_signed_integer_number!(i8, safe_step, i8::MIN, i8::MAX);
impl_signed_integer_number!(i16, safe_step, i16::MIN, i16::MAX);
impl_signed_integer_number!(i32, safe_step, i32::MIN, i32::MAX);
impl_signed_integer_number!(i64, binding_step);
impl_signed_integer_number!(isize, binding_step);

impl_unsigned_integer_number!(u8, safe_step, u8::MAX);
impl_unsigned_integer_number!(u16, safe_step, u16::MAX);
impl_unsigned_integer_number!(u32, safe_step, u32::MAX);
impl_unsigned_integer_number!(u64, binding_step);
impl_unsigned_integer_number!(usize, binding_step);

impl NumberFieldValue for f32 {
    fn input_policy() -> NumberInputPolicy {
        NumberInputPolicy::float()
    }
}

impl NumberFieldValue for f64 {
    fn input_policy() -> NumberInputPolicy {
        NumberInputPolicy::float()
    }
}

pub fn number_input<N>(state: &Entity<InputState>) -> NumberInput
where
    N: NumberFieldValue,
{
    let _ = PhantomData::<fn() -> N>;
    NumberInput::new(state)
}

#[cfg(test)]
mod tests {
    use super::{NumberFieldValue as _, NumberInputKind, StepAction};

    #[test]
    fn number_policy_distinguishes_integer_and_float_input_shapes() {
        let signed = i32::input_policy();
        assert_eq!(signed.kind, NumberInputKind::SignedInteger);
        assert!(signed.allows_input("-"));
        assert!(signed.allows_input("-12"));
        assert!(!signed.allows_input("12.0"));

        let unsigned = u32::input_policy();
        assert_eq!(unsigned.kind, NumberInputKind::UnsignedInteger);
        assert!(unsigned.allows_input("12"));
        assert!(!unsigned.allows_input("-1"));
        assert!(!unsigned.allows_input("+1"));
        assert!(!unsigned.allows_input("12.0"));

        let float = f64::input_policy();
        assert_eq!(float.kind, NumberInputKind::Float);
        assert!(float.allows_input("-"));
        assert!(float.allows_input("."));
        assert!(float.allows_input("-0.5"));
        assert!(!float.allows_input("1.2.3"));
    }

    #[test]
    fn number_policy_uses_binding_step_for_large_integer_types() {
        assert_eq!(i32::input_policy().component_step, Some(1.));
        assert_eq!(u32::input_policy().component_step, Some(1.));
        assert_eq!(i64::input_policy().component_step, None);
        assert_eq!(u64::input_policy().component_step, None);
        assert_eq!(f64::input_policy().component_step, Some(1.));
    }

    #[test]
    fn large_integer_step_uses_checked_rust_arithmetic() {
        assert_eq!(
            i64::step_draft("9223372036854775807", StepAction::Increment),
            None
        );
        assert_eq!(
            i64::step_draft("9223372036854775807", StepAction::Decrement),
            Some("9223372036854775806".to_string())
        );
        assert_eq!(u64::step_draft("0", StepAction::Decrement), None);
        assert_eq!(
            u64::step_draft("", StepAction::Increment),
            Some("1".to_string())
        );
    }
}
