#[derive(Debug, thiserror::Error)]
pub(crate) enum AiChat2Error {
    #[error("could not resolve ai-chat2 config directory")]
    ConfigDirUnavailable,
    #[error("log file not found")]
    LogFileNotFound,
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("toml serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("database error: {0}")]
    Database(#[from] ai_chat_db::DbError),
    #[error("global hotkey error: {0}")]
    GlobalHotkey(#[from] global_hotkey::Error),
    #[error("hotkey parse error: {0}")]
    HotkeyParse(#[from] global_hotkey::hotkey::HotKeyParseError),
    #[error("hotkey runtime unavailable: {0}")]
    HotkeyUnavailable(String),
    #[error("window error: {0}")]
    Window(String),
    #[error("attachment error: {0}")]
    Attachment(String),
}

pub(crate) type AiChat2Result<T> = Result<T, AiChat2Error>;
