#[derive(thiserror::Error, Debug)]
pub enum FeiwenError {
    #[error("url parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
}

pub type FeiwenResult<T> = Result<T, FeiwenError>;
