use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("log directory unavailable")]
    LogDirectoryUnavailable,
    #[error("failed to initialize logging")]
    LogInitialization(#[source] io::Error),
}

pub(crate) type AppResult<T> = Result<T, AppError>;
