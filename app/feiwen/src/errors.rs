#[derive(thiserror::Error, Debug)]
pub(crate) enum FeiwenError {
    #[error("log file not found")]
    LogFileNotFound,
}
pub(crate) type FeiwenResult<T> = Result<T, FeiwenError>;
