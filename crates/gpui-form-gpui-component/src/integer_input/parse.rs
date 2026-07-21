use super::{IntegerInputError, IntegerInputPolicy, IntegerValue};

pub(crate) fn parse_integer<N>(
    text: &str,
    policy: IntegerInputPolicy<N>,
) -> Result<N, IntegerInputError<N>>
where
    N: IntegerValue,
{
    let text = text.trim();
    if text.is_empty() || text == "+" || text == "-" {
        return Err(IntegerInputError::Incomplete);
    }
    if !valid_shape(text) {
        return Err(IntegerInputError::InvalidSyntax);
    }
    let value = N::from_str(text).map_err(|_| IntegerInputError::Overflow)?;
    if !policy.contains(value) {
        return Err(IntegerInputError::OutOfRange {
            min: policy.min,
            max: policy.max,
        });
    }
    Ok(value)
}

fn valid_shape(text: &str) -> bool {
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}
