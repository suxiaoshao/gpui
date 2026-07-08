use time::{error::Format, error::Parse};

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database path is not valid UTF-8")]
    InvalidDatabasePath,
    #[error("database pool failed: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database connection failed: {0}")]
    Connection(#[from] diesel::r2d2::Error),
    #[error("database query failed: {0}")]
    Diesel(#[from] diesel::result::Error),
    #[error("database connection setup failed: {0}")]
    ConnectionSetup(#[from] diesel::ConnectionError),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization failed: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("timestamp formatting failed: {0}")]
    TimeFormat(#[from] Format),
    #[error("timestamp parsing failed: {0}")]
    TimeParse(#[from] Parse),
    #[error(
        "database schema version {database_version} is newer than supported version {supported_version}"
    )]
    UnsupportedSchemaVersion {
        database_version: i32,
        supported_version: i32,
    },
    #[error("database invariant failed: {0}")]
    Invariant(String),
}
