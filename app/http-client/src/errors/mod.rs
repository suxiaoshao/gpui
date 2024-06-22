#[derive(thiserror::Error, Debug)]
pub enum HttpClientError {
    #[error("url parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("gpui runtime")]
    GpuiRuntime,
}

pub type HttpClientResult<T> = Result<T, HttpClientError>;
