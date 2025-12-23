#[derive(thiserror::Error, Debug)]
pub enum AiChatError {
    #[error("log file not found")]
    LogFileNotFound,
}

pub type AiChatResult<T> = Result<T, AiChatError>;
