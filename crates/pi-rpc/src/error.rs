#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("Pi I/O failed: {0}")]
    Io(String),
    #[error("invalid Pi protocol: {0}")]
    Protocol(String),
    #[error("Pi rejected {command}: {message}")]
    Rejected { command: String, message: String },
    #[error("Pi connection closed")]
    Closed,
    #[error("Pi startup did not become ready before its deadline")]
    StartupTimeout,
    #[error("Pi exited before startup became ready")]
    NotReady,
    #[error("Pi {0} capacity exceeded")]
    Capacity(&'static str),
    #[error("invalid client options: {0}")]
    Options(&'static str),
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Protocol(error.to_string())
    }
}
