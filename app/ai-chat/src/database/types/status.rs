use std::{fmt::Display, str::FromStr};

use crate::errors::AiChatError;

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum Status {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "hidden")]
    Hidden,
    #[serde(rename = "loading")]
    Loading,
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "error")]
    Error,
}

impl FromStr for Status {
    type Err = AiChatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(Status::Normal),
            "hidden" => Ok(Status::Hidden),
            "loading" => Ok(Status::Loading),
            "thinking" => Ok(Status::Thinking),
            "paused" => Ok(Status::Paused),
            "error" => Ok(Status::Error),
            _ => Err(AiChatError::InvalidMessageStatus(s.to_owned())),
        }
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Normal => f.write_str("normal"),
            Status::Hidden => f.write_str("hidden"),
            Status::Loading => f.write_str("loading"),
            Status::Thinking => f.write_str("thinking"),
            Status::Paused => f.write_str("paused"),
            Status::Error => f.write_str("error"),
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_mode() -> anyhow::Result<()> {
        use super::Status;
        assert_eq!("normal".parse::<Status>()?, Status::Normal);
        assert_eq!("hidden".parse::<Status>()?, Status::Hidden);
        assert_eq!("loading".parse::<Status>()?, Status::Loading);
        assert_eq!("thinking".parse::<Status>()?, Status::Thinking);
        assert_eq!("paused".parse::<Status>()?, Status::Paused);
        assert_eq!("error".parse::<Status>()?, Status::Error);
        assert!("invalid".parse::<Status>().is_err());
        Ok(())
    }
}
