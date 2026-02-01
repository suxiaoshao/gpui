use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum AiChatError {
    #[error("数据库错误:{}",.0)]
    Sqlite(#[from] diesel::result::Error),
    #[error("数据库连接错误:{}",.0)]
    Connection(#[from] diesel::ConnectionError),
    #[error("数据库连接池错误:{}",.0)]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("数据库连接池获取链接错误:{}",.0)]
    GetConnection(#[from] diesel::r2d2::Error),
    #[error("log file not found")]
    LogFileNotFound,
    #[error("获取不了历史记录数据库路径")]
    DbPath,
    #[error("文件系统错误:{}",.0)]
    Fs(#[from] std::io::Error),
    #[error("无效的模式:{}",.0)]
    InvalidMode(String),
    #[error("无效的角色:{}",.0)]
    InvalidRole(String),
    #[error("无效的消息状态:{}",.0)]
    InvalidMessageStatus(String),
    #[error("serde_json错误:{}",.0)]
    SerdeJson(#[from] serde_json::Error),
    #[error("eventsource错误:{}",.0)]
    EventSource(#[from] reqwest_eventsource::CannotCloneRequestError),
    #[error("api key未设置")]
    ApiKeyNotSet,
    #[error("请求头构造错误:{}",.0)]
    HeaderParse(#[from] reqwest::header::InvalidHeaderValue),
    #[error("请求错误:{}",.0)]
    Request(#[from] reqwest::Error),
    #[error("conversation path exists:{}",.0)]
    ConversationPathExists(String),
    #[error("folder path exists:{}",.0)]
    FolderPathExists(String),
    #[error("这个 template 的 conversation 还存在,不能删除")]
    TemplateHasConversation,
    #[error("Extension runtime error")]
    ExtensionRuntimeError,
    #[error("adapter {} settings not found",.0)]
    AdapterSettingsNotFound(String),
    #[error("adapter {} not found",.0)]
    AdapterNotFound(String),
    #[error("Wasmtime engine creation failed")]
    WasmtimeEngineCreationFailed,
    #[error("Wasmtime component creation failed")]
    WasmtimeComponentCreationFailed(PathBuf),
    #[error("Wasmtime error")]
    WasmtimeError,
    #[error("Extension {} not found",.0)]
    ExtensionNotFound(String),
    #[error("Extension {} error",.0)]
    ExtensionError(String),
    #[error("toml解析错误:{}",.0)]
    TomlParse(#[from] toml::de::Error),
    #[error("toml序列化错误:{}",.0)]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("GlobalHotKeyManager creation failed: {}", .0)]
    GlobalHotKeyManagerCreationFailed(#[from] global_hotkey::Error),
    #[error("HotKey creation failed: {}", .0)]
    HotKeyCreationFailed(#[from] global_hotkey::hotkey::HotKeyParseError),
    #[error("gpui error")]
    GpuiError,
}

pub type AiChatResult<T> = Result<T, AiChatError>;
