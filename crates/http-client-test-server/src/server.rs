use std::{
    convert::Infallible,
    fmt,
    io::{self, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Empty, Full, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response, StatusCode,
    body::Incoming,
    header::{ALLOW, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::{rt::TokioIo, server::graceful::GracefulShutdown};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::JoinSet,
    time::timeout,
};
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};

use crate::{
    AbortSpec, RespondSpec, SpecUrlError,
    contract::{ControlError, encode_spec},
};

const ACTIVE_CONNECTION_LIMIT: usize = 64;
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(2);

pub(crate) type ServerBody = UnsyncBoxBody<Bytes, WireError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerErrorKind {
    Bind,
    Accept,
    TaskPanicked,
    Shutdown,
}

pub struct ServerError {
    kind: ServerErrorKind,
}

impl ServerError {
    pub fn kind(&self) -> ServerErrorKind {
        self.kind
    }

    const fn new(kind: ServerErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Debug for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "test server lifecycle failed ({:?})", self.kind)
    }
}

impl std::error::Error for ServerError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WireError {
    BeforeHead,
    BodyInterrupted,
    Cancelled,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("intentional test connection interruption")
    }
}

impl std::error::Error for WireError {}

pub(crate) fn empty_body() -> ServerBody {
    Empty::<Bytes>::new()
        .map_err(|never: Infallible| match never {})
        .boxed_unsync()
}

pub(crate) fn full_body(bytes: Bytes) -> ServerBody {
    Full::new(bytes)
        .map_err(|never: Infallible| match never {})
        .boxed_unsync()
}

pub(crate) fn empty_response(
    status: StatusCode,
    allow: Option<&'static str>,
) -> Response<ServerBody> {
    let mut response = Response::new(empty_body());
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
    if let Some(allow) = allow {
        response
            .headers_mut()
            .insert(ALLOW, HeaderValue::from_static(allow));
    }
    response
}

pub(crate) fn control_error_response(error: ControlError) -> Response<ServerBody> {
    let bytes = Bytes::from(format!("{{\"code\":\"{}\"}}", error.code.as_str()));
    let length = HeaderValue::from_str(&bytes.len().to_string()).expect("decimal length is valid");
    let mut response = Response::new(full_body(bytes));
    *response.status_mut() = if error.too_large {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response.headers_mut().insert(CONTENT_LENGTH, length);
    response
}

pub struct TestServer {
    base_url: String,
    cancellation: CancellationToken,
    owner: Option<AbortOnDropHandle<Result<(), ServerError>>>,
}

impl TestServer {
    pub async fn spawn() -> Result<Self, ServerError> {
        Self::spawn_on_port(0).await
    }

    async fn spawn_on_port(port: u16) -> Result<Self, ServerError> {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let listener = TcpListener::bind(address)
            .await
            .map_err(|_| ServerError::new(ServerErrorKind::Bind))?;
        let actual_address = listener
            .local_addr()
            .map_err(|_| ServerError::new(ServerErrorKind::Bind))?;
        let cancellation = CancellationToken::new();
        let owner_cancellation = cancellation.clone();
        let owner = tokio::spawn(async move { accept_owner(listener, owner_cancellation).await });
        Ok(Self {
            base_url: format!("http://127.0.0.1:{}", actual_address.port()),
            cancellation,
            owner: Some(AbortOnDropHandle::new(owner)),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn respond_url(&self, spec: &RespondSpec) -> Result<String, SpecUrlError> {
        Ok(format!(
            "{}/v1/respond?spec={}",
            self.base_url,
            encode_spec(spec)?
        ))
    }

    pub fn abort_url(&self, spec: &AbortSpec) -> Result<String, SpecUrlError> {
        Ok(format!(
            "{}/v1/abort?spec={}",
            self.base_url,
            encode_spec(spec)?
        ))
    }

    pub async fn shutdown(mut self) -> Result<(), ServerError> {
        self.cancellation.cancel();
        let owner = self
            .owner
            .take()
            .expect("test server owner is present")
            .detach();
        owner
            .await
            .map_err(|_| ServerError::new(ServerErrorKind::TaskPanicked))?
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(owner) = self.owner.take() {
            owner.abort();
        }
    }
}

pub(crate) async fn run_cli(port: u16) -> Result<(), ServerError> {
    let server = TestServer::spawn_on_port(port).await?;
    {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        writeln!(stdout, "HTTP_CLIENT_TEST_SERVER={}", server.base_url())
            .map_err(|_| ServerError::new(ServerErrorKind::Shutdown))?;
        stdout
            .flush()
            .map_err(|_| ServerError::new(ServerErrorKind::Shutdown))?;
    }
    tokio::signal::ctrl_c()
        .await
        .map_err(|_| ServerError::new(ServerErrorKind::Shutdown))?;
    server.shutdown().await
}

async fn accept_owner(
    listener: TcpListener,
    cancellation: CancellationToken,
) -> Result<(), ServerError> {
    let graceful = GracefulShutdown::new();
    let semaphore = std::sync::Arc::new(Semaphore::new(ACTIVE_CONNECTION_LIMIT));
    let mut connections = JoinSet::new();
    let mut terminal_error = None;

    loop {
        tokio::select! {
            _ = cancellation.cancelled() => break,
            joined = connections.join_next(), if !connections.is_empty() => {
                if joined.is_some_and(|result| result.is_err_and(|error| error.is_panic())) {
                    terminal_error.get_or_insert(ServerErrorKind::TaskPanicked);
                    break;
                }
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _peer)) => {
                        let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                            drop(stream);
                            continue;
                        };
                        let watcher = graceful.watcher();
                        let connection_cancellation = cancellation.clone();
                        connections.spawn(async move {
                            let _permit = permit;
                            serve_connection(stream, watcher, connection_cancellation).await;
                        });
                    }
                    Err(_) => {
                        terminal_error.get_or_insert(ServerErrorKind::Accept);
                        break;
                    }
                }
            }
        }
    }

    let graceful_drain = async {
        graceful.shutdown().await;
        while let Some(result) = connections.join_next().await {
            if result.is_err_and(|error| error.is_panic()) {
                terminal_error.get_or_insert(ServerErrorKind::TaskPanicked);
            }
        }
    };

    if timeout(SHUTDOWN_DEADLINE, graceful_drain).await.is_err() {
        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if result.is_err_and(|error| error.is_panic()) {
                terminal_error.get_or_insert(ServerErrorKind::TaskPanicked);
            }
        }
    }

    if let Some(kind) = terminal_error {
        Err(ServerError::new(kind))
    } else {
        Ok(())
    }
}

async fn serve_connection(
    stream: TcpStream,
    watcher: hyper_util::server::graceful::Watcher,
    cancellation: CancellationToken,
) {
    let io = TokioIo::new(stream);
    let service = service_fn(move |request| route(request, cancellation.clone()));
    let mut builder = http1::Builder::new();
    builder.keep_alive(false).half_close(true);
    let connection = builder.serve_connection(io, service);
    let _expected_connection_result = watcher.watch(connection).await;
}

async fn route(
    request: Request<Incoming>,
    cancellation: CancellationToken,
) -> Result<Response<ServerBody>, WireError> {
    match request.uri().path() {
        "/healthz" => {
            if request.method() != hyper::Method::GET {
                return Ok(empty_response(StatusCode::METHOD_NOT_ALLOWED, Some("GET")));
            }
            if request.uri().query().is_some() {
                return Ok(control_error_response(ControlError::invalid(
                    crate::contract::ControlCode::InvalidRequest,
                )));
            }
            let bytes = Bytes::from_static(b"ok\n");
            let mut response = Response::new(full_body(bytes));
            response.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            response
                .headers_mut()
                .insert(CONTENT_LENGTH, HeaderValue::from_static("3"));
            Ok(response)
        }
        "/v1/respond" => crate::respond::handle(request, cancellation).await,
        "/v1/abort" => crate::abort::handle(request, cancellation).await,
        "/v1/echo" => Ok(crate::echo::handle(request).await),
        _ => Ok(empty_response(StatusCode::NOT_FOUND, None)),
    }
}
