use std::{error::Error, fmt, sync::Arc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectProblemKind {
    InvalidLocation,
    Loop,
    HopLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodySizeDimension {
    Encoded,
    Stored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestProblemKind {
    Transport,
    Timeout,
    Redirect(RedirectProblemKind),
    RequestBodyRead,
    ResponseBodyRead,
    ResponseBodyDecode,
    TemporaryStorage,
    BodyTooLarge {
        dimension: BodySizeDimension,
        limit: u64,
        observed: u64,
    },
    Internal,
}

#[derive(Clone)]
pub struct RequestProblem {
    kind: RequestProblemKind,
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl RequestProblem {
    pub const fn kind(&self) -> RequestProblemKind {
        self.kind
    }

    pub fn transport(source: impl Error + Send + Sync + 'static) -> Self {
        Self::with_source(RequestProblemKind::Transport, source)
    }

    pub const fn timeout() -> Self {
        Self::without_source(RequestProblemKind::Timeout)
    }

    pub const fn redirect(kind: RedirectProblemKind) -> Self {
        Self::without_source(RequestProblemKind::Redirect(kind))
    }

    pub fn request_body_read(source: impl Error + Send + Sync + 'static) -> Self {
        Self::with_source(RequestProblemKind::RequestBodyRead, source)
    }

    pub fn response_body_read(source: impl Error + Send + Sync + 'static) -> Self {
        Self::with_source(RequestProblemKind::ResponseBodyRead, source)
    }

    pub fn response_body_decode(source: impl Error + Send + Sync + 'static) -> Self {
        Self::with_source(RequestProblemKind::ResponseBodyDecode, source)
    }

    pub fn temporary_storage(source: impl Error + Send + Sync + 'static) -> Self {
        Self::with_source(RequestProblemKind::TemporaryStorage, source)
    }

    pub const fn too_large(dimension: BodySizeDimension, limit: u64, observed: u64) -> Self {
        Self::without_source(RequestProblemKind::BodyTooLarge {
            dimension,
            limit,
            observed,
        })
    }

    pub const fn internal() -> Self {
        Self::without_source(RequestProblemKind::Internal)
    }

    const fn without_source(kind: RequestProblemKind) -> Self {
        Self { kind, source: None }
    }

    fn with_source(kind: RequestProblemKind, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind,
            source: Some(Arc::new(source)),
        }
    }
}

impl fmt::Display for RequestProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "HTTP request failed ({:?})", self.kind)
    }
}

impl fmt::Debug for RequestProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestProblem")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Error for RequestProblem {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
