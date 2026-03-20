use std::{fmt::Display, str::FromStr};

use crate::errors::AiChatError;

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum ShortcutInputSource {
    #[serde(rename = "selection_or_clipboard")]
    SelectionOrClipboard,
    #[serde(rename = "screenshot")]
    Screenshot,
}

impl FromStr for ShortcutInputSource {
    type Err = AiChatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "selection_or_clipboard" => Ok(Self::SelectionOrClipboard),
            "screenshot" => Ok(Self::Screenshot),
            _ => Err(AiChatError::InvalidShortcutInputSource(s.to_owned())),
        }
    }
}

impl Display for ShortcutInputSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelectionOrClipboard => f.write_str("selection_or_clipboard"),
            Self::Screenshot => f.write_str("screenshot"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::ShortcutInputSource;

    #[test]
    fn test_shortcut_input_source() -> anyhow::Result<()> {
        assert_eq!(
            "selection_or_clipboard".parse::<ShortcutInputSource>()?,
            ShortcutInputSource::SelectionOrClipboard
        );
        assert_eq!(
            "screenshot".parse::<ShortcutInputSource>()?,
            ShortcutInputSource::Screenshot
        );
        assert!("invalid".parse::<ShortcutInputSource>().is_err());
        Ok(())
    }
}
