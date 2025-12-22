#[derive(thiserror::Error, Debug)]
pub enum HttpClientError {
    #[error("url parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("log file not found")]
    LogFileNotFound,
}

pub type HttpClientResult<T> = Result<T, HttpClientError>;
