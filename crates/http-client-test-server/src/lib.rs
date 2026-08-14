//! Deterministic loopback HTTP service for HTTP Client integration scenarios.

mod abort;
mod contract;
mod echo;
mod respond;
mod server;

pub use contract::{
    AbortSpec, ContentEncoding, HeaderSpec, RespondSpec, ResponseBodySpec, ResponseFraming,
    SpecUrlError,
};
pub use server::{ServerError, ServerErrorKind, TestServer};

/// Runs the standalone loopback server until Ctrl-C is received.
///
/// This entry point is shared with the binary and is not intended as consumer API.
#[doc(hidden)]
pub async fn run_cli(port: u16) -> Result<(), ServerError> {
    server::run_cli(port).await
}
