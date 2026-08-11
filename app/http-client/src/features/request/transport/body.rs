use std::{fmt, io};

use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{Body, RequestBuilder, multipart};
use tokio_util::io::ReaderStream;

use crate::features::request::prepared::{BodyContentType, PreparedBody, PreparedMultipartPart};

/// Rebuilds the frozen request body for one HTTP hop.
///
/// Streaming bodies are deliberately opened again for every call. The error
/// never formats a source path or body value.
pub(super) async fn apply(
    mut builder: RequestBuilder,
    body: &PreparedBody,
    content_type: &BodyContentType,
) -> Result<RequestBuilder, ReplayError> {
    match body {
        PreparedBody::None => Ok(builder),
        PreparedBody::Text(bytes) | PreparedBody::UrlEncoded(bytes) => {
            builder = builder
                .header(CONTENT_LENGTH, bytes.len() as u64)
                .body(bytes.clone());
            Ok(apply_fixed_content_type(builder, content_type))
        }
        PreparedBody::Binary(path) => {
            let file = tokio::fs::File::open(path)
                .await
                .map_err(ReplayError::open)?;
            let len = file.metadata().await.map_err(ReplayError::metadata)?.len();
            Ok(builder
                .header(CONTENT_LENGTH, len)
                .body(Body::wrap_stream(ReaderStream::new(file))))
        }
        PreparedBody::Multipart(parts) => {
            let mut form = multipart::Form::new();
            for part in parts {
                match part {
                    PreparedMultipartPart::Text {
                        name,
                        value,
                        content_type,
                    } => {
                        let mut field = multipart::Part::text(value.clone());
                        if let Some(content_type) = content_type {
                            field = field
                                .mime_str(content_type.as_ref())
                                .map_err(ReplayError::multipart)?;
                        }
                        form = form.part(name.clone(), field);
                    }
                    PreparedMultipartPart::File {
                        name,
                        path,
                        file_name,
                        content_type,
                    } => {
                        let file = tokio::fs::File::open(path)
                            .await
                            .map_err(ReplayError::open)?;
                        let len = file.metadata().await.map_err(ReplayError::metadata)?.len();
                        let field = multipart::Part::stream_with_length(
                            Body::wrap_stream(ReaderStream::new(file)),
                            len,
                        )
                        .file_name(file_name.clone())
                        .mime_str(content_type.as_ref())
                        .map_err(ReplayError::multipart)?;
                        form = form.part(name.clone(), field);
                    }
                }
            }
            Ok(builder.multipart(form))
        }
    }
}

fn apply_fixed_content_type(
    builder: RequestBuilder,
    content_type: &BodyContentType,
) -> RequestBuilder {
    match content_type {
        BodyContentType::Fixed(value) => builder.header(CONTENT_TYPE, value.clone()),
        BodyContentType::None | BodyContentType::MultipartBoundary => builder,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayErrorKind {
    Open,
    Metadata,
    Multipart,
}

pub(super) struct ReplayError {
    kind: ReplayErrorKind,
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ReplayError {
    fn open(source: io::Error) -> Self {
        Self {
            kind: ReplayErrorKind::Open,
            source: Box::new(source),
        }
    }

    fn metadata(source: io::Error) -> Self {
        Self {
            kind: ReplayErrorKind::Metadata,
            source: Box::new(source),
        }
    }

    fn multipart(source: reqwest::Error) -> Self {
        Self {
            kind: ReplayErrorKind::Multipart,
            source: Box::new(source),
        }
    }
}

impl fmt::Debug for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("request body could not be replayed")
    }
}

impl std::error::Error for ReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
