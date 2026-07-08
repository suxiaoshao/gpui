#[derive(Debug, thiserror::Error)]
pub(crate) enum JacoError {
    #[error("could not resolve jaco config directory")]
    ConfigDirUnavailable,
    #[error("config error: {0}")]
    Config(String),
    #[error("log file not found")]
    LogFileNotFound,
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("toml serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("database error: {0}")]
    Database(#[from] jaco_db::DbError),
    #[error("agent runtime error: {0}")]
    AgentRuntime(#[from] jaco_agent::AgentRuntimeError),
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

pub(crate) type JacoResult<T> = Result<T, JacoError>;
