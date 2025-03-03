use thiserror::Error;

#[derive(Error, Debug)]
pub enum NovelError {
    #[error("网络错误")]
    NetworkError(reqwest::Error),
    #[error("解析错误")]
    ParseError,
}

impl From<reqwest::Error> for NovelError {
    fn from(e: reqwest::Error) -> Self {
        NovelError::NetworkError(e)
    }
}

impl From<nom::Err<nom::error::Error<&str>>> for NovelError {
    fn from(_: nom::Err<nom::error::Error<&str>>) -> Self {
        NovelError::ParseError
    }
}

pub(crate) type NovelResult<T> = Result<T, NovelError>;
