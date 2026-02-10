use std::{fmt::Display, str::FromStr};

use crate::errors::{AiChatError};
use gpui::SharedString;
use gpui_component::select::SelectItem;

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum Mode {
    #[serde(rename = "contextual")]
    Contextual,
    #[serde(rename = "single")]
    Single,
    #[serde(rename = "assistant-only")]
    AssistantOnly,
}

impl FromStr for Mode {
    type Err = AiChatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "contextual" => Ok(Mode::Contextual),
            "single" => Ok(Mode::Single),
            "assistant-only" => Ok(Mode::AssistantOnly),
            _ => Err(AiChatError::InvalidMode(s.to_owned())),
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::AssistantOnly => f.write_str("assistant-only"),
            Mode::Contextual => f.write_str("contextual"),
            Mode::Single => f.write_str("single"),
        }
    }
}

impl SelectItem for Mode {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.to_string().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_mode() -> anyhow::Result<()> {
        use super::Mode;
        assert_eq!("contextual".parse::<Mode>()?, Mode::Contextual);
        assert_eq!("single".parse::<Mode>()?, Mode::Single);
        assert_eq!("assistant-only".parse::<Mode>()?, Mode::AssistantOnly);
        assert!("invalid".parse::<Mode>().is_err());
        Ok(())
    }
}
