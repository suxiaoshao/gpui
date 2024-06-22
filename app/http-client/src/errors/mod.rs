#[derive(thiserror::Error, Debug)]
pub enum HttpClientError {
    #[error("url parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
}

pub type HttpClientResult<T> = Result<T, HttpClientError>;
