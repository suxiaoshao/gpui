use thiserror::Error;

pub type Result<T> = std::result::Result<T, XtaskError>;

#[derive(Debug, Error)]
pub enum XtaskError {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("toml parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("plist error: {0}")]
    Plist(#[from] plist::Error),
    #[error("system time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error("failed to execute `{command}`: {source}")]
    CommandExecute {
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("command failed ({status}): {command}")]
    CommandFailed { command: String, status: String },
}

impl XtaskError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
