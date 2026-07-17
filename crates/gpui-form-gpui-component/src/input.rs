use gpui_form::{FieldCodec, FieldCodecError};

/// Codec for controls whose empty text represents an absent optional value.
/// The UI remains a plain text input; the form owns the conversion policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OptionalTextCodec;

impl FieldCodec<Option<String>> for OptionalTextCodec {
    type Draft = String;

    fn draft_from_value(value: &Option<String>) -> Self::Draft {
        value.clone().unwrap_or_default()
    }

    fn parse(draft: &Self::Draft) -> Result<Option<String>, FieldCodecError> {
        Ok((!draft.is_empty()).then(|| draft.clone()))
    }
}

/// A small conversion helper for custom text-backed fields.
pub trait TextFieldValue: Clone + PartialEq + 'static {
    fn to_text(&self) -> String;
    fn from_text(text: String) -> Self;
}

impl TextFieldValue for String {
    fn to_text(&self) -> String {
        self.clone()
    }

    fn from_text(text: String) -> Self {
        text
    }
}

impl TextFieldValue for Option<String> {
    fn to_text(&self) -> String {
        self.clone().unwrap_or_default()
    }

    fn from_text(text: String) -> Self {
        (!text.is_empty()).then_some(text)
    }
}
