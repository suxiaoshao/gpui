#[derive(thiserror::Error, Debug)]
pub enum HttpClientError {
    #[error("log file not found")]
    LogFileNotFound,
}

pub type HttpClientResult<T> = Result<T, HttpClientError>;
