use std::{error::Error, fmt, sync::Arc, time::Duration};

use bytes::Bytes;
use http::{HeaderMap, StatusCode, Version};
use tempfile::TempPath;
use tokio::io::{AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use url::Url;

pub(crate) const MEMORY_SPILL_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) const INLINE_PREVIEW_BYTES: u64 = 2 * 1024 * 1024;
pub(crate) const CAPTURE_LIMIT_BYTES: u64 = 50 * 1024 * 1024;

pub(crate) struct ResponseData {
    head: ResponseHead,
    timing: ResponseTiming,
    sizes: ResponseSizes,
    body_decoding: BodyDecoding,
    body: StoredBody,
}

impl ResponseData {
    pub(crate) fn new(
        head: ResponseHead,
        timing: ResponseTiming,
        completed: CompletedBody,
    ) -> Self {
        debug_assert_eq!(completed.sizes.stored_body_bytes, completed.body.len());
        Self {
            head,
            timing,
            sizes: completed.sizes,
            body_decoding: completed.body_decoding,
            body: completed.body,
        }
    }

    pub(crate) const fn head(&self) -> &ResponseHead {
        &self.head
    }

    pub(crate) const fn timing(&self) -> &ResponseTiming {
        &self.timing
    }

    pub(crate) const fn sizes(&self) -> &ResponseSizes {
        &self.sizes
    }

    pub(crate) const fn body_decoding(&self) -> BodyDecoding {
        self.body_decoding
    }

    pub(crate) fn progress(&self) -> ResponseProgress {
        ResponseProgress {
            declared_encoded_bytes: self.sizes.declared_encoded_bytes,
            received_encoded_bytes: self.sizes.received_encoded_bytes,
            stored_body_bytes: self.sizes.stored_body_bytes,
            storage: self.body.active_storage(),
        }
    }

    pub(crate) fn read_lease(self: &Arc<Self>) -> ResponseReadLease {
        ResponseReadLease(Arc::clone(self))
    }
}

impl fmt::Debug for ResponseData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseData")
            .field("head", &self.head)
            .field("timing", &self.timing)
            .field("sizes", &self.sizes)
            .field("body_decoding", &self.body_decoding)
            .field("body", &self.body)
            .finish()
    }
}

pub(crate) struct ResponseHead {
    pub(crate) status: StatusCode,
    pub(crate) version: Version,
    pub(crate) final_url: Url,
    pub(crate) headers: HeaderMap,
}

impl ResponseHead {
    #[cfg(test)]
    pub(crate) fn new(
        status: StatusCode,
        version: Version,
        final_url: Url,
        headers: HeaderMap,
    ) -> Self {
        Self {
            status,
            version,
            final_url,
            headers,
        }
    }
}

impl fmt::Debug for ResponseHead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseHead")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("final_url", &"<redacted>")
            .field("header_count", &self.headers.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResponseTiming {
    pub(crate) head_after: Duration,
    pub(crate) completed_after: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResponseSizes {
    pub(crate) declared_encoded_bytes: Option<u64>,
    pub(crate) received_encoded_bytes: u64,
    pub(crate) stored_body_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BodyDecoding {
    Identity,
    Decoded,
    Unsupported,
}

pub(crate) enum StoredBody {
    Empty,
    Memory(Bytes),
    TempFile { path: TempPath, len: u64 },
}

impl StoredBody {
    pub(crate) fn len(&self) -> u64 {
        match self {
            Self::Empty => 0,
            Self::Memory(bytes) => bytes.len() as u64,
            Self::TempFile { len, .. } => *len,
        }
    }

    fn active_storage(&self) -> ActiveBodyStorage {
        match self {
            Self::Empty | Self::Memory(_) => ActiveBodyStorage::Memory,
            Self::TempFile { .. } => ActiveBodyStorage::TempFile,
        }
    }

    pub(super) fn temp_file(path: TempPath, len: u64) -> Self {
        debug_assert!(len > 0);
        Self::TempFile { path, len }
    }
}

impl fmt::Debug for StoredBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("StoredBody::Empty"),
            Self::Memory(bytes) => formatter
                .debug_struct("StoredBody::Memory")
                .field("len", &bytes.len())
                .finish(),
            Self::TempFile { len, .. } => formatter
                .debug_struct("StoredBody::TempFile")
                .field("path", &"<redacted>")
                .field("len", len)
                .finish(),
        }
    }
}

pub(crate) struct CompletedBody {
    pub(crate) body: StoredBody,
    pub(crate) body_decoding: BodyDecoding,
    pub(crate) sizes: ResponseSizes,
}

impl fmt::Debug for CompletedBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedBody")
            .field("body", &self.body)
            .field("body_decoding", &self.body_decoding)
            .field("sizes", &self.sizes)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActiveBodyStorage {
    Memory,
    TempFile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResponseProgress {
    pub(crate) declared_encoded_bytes: Option<u64>,
    pub(crate) received_encoded_bytes: u64,
    pub(crate) stored_body_bytes: u64,
    pub(crate) storage: ActiveBodyStorage,
}

impl ResponseProgress {
    pub(crate) const fn initial(declared_encoded_bytes: Option<u64>) -> Self {
        Self {
            declared_encoded_bytes,
            received_encoded_bytes: 0,
            stored_body_bytes: 0,
            storage: ActiveBodyStorage::Memory,
        }
    }

    pub(crate) fn is_monotonic_from(&self, previous: &Self) -> bool {
        self.declared_encoded_bytes == previous.declared_encoded_bytes
            && self.received_encoded_bytes >= previous.received_encoded_bytes
            && self.stored_body_bytes >= previous.stored_body_bytes
            && !matches!(
                (previous.storage, self.storage),
                (ActiveBodyStorage::TempFile, ActiveBodyStorage::Memory)
            )
    }
}

pub(crate) struct ResponseReadLease(Arc<ResponseData>);

impl ResponseReadLease {
    pub(crate) fn len(&self) -> u64 {
        self.0.body.len()
    }

    pub(crate) async fn read_prefix(
        &self,
        limit: usize,
    ) -> Result<PrefixBytes, ResponseReadProblem> {
        let expected = self.len().min(limit as u64) as usize;
        let complete = self.len() <= limit as u64;
        let bytes = match &self.0.body {
            StoredBody::Empty => Bytes::new(),
            StoredBody::Memory(bytes) => bytes.slice(..expected),
            StoredBody::TempFile { path, .. } => {
                if expected == 0 {
                    Bytes::new()
                } else {
                    let mut file = tokio::fs::File::open(path.as_ref() as &std::path::Path)
                        .await
                        .map_err(|_| ResponseReadProblem::Open)?;
                    let mut buffer = vec![0; expected];
                    file.read_exact(&mut buffer)
                        .await
                        .map_err(|error| match error.kind() {
                            std::io::ErrorKind::UnexpectedEof => {
                                ResponseReadProblem::LengthMismatch
                            }
                            _ => ResponseReadProblem::Read,
                        })?;
                    Bytes::from(buffer)
                }
            }
        };
        Ok(PrefixBytes { bytes, complete })
    }

    pub(crate) async fn read_all_bounded(&self, limit: u64) -> Result<Bytes, ResponseReadProblem> {
        let len = self.len();
        if len > limit || len > usize::MAX as u64 {
            return Err(ResponseReadProblem::LimitExceeded);
        }
        match &self.0.body {
            StoredBody::Empty => Ok(Bytes::new()),
            StoredBody::Memory(bytes) if bytes.len() as u64 == len => Ok(bytes.clone()),
            StoredBody::Memory(_) => Err(ResponseReadProblem::LengthMismatch),
            StoredBody::TempFile { .. } => {
                let mut bytes = Vec::with_capacity(len as usize);
                let copied = self.copy_all_to(&mut bytes).await?;
                if copied != len || bytes.len() as u64 != len {
                    return Err(ResponseReadProblem::LengthMismatch);
                }
                Ok(Bytes::from(bytes))
            }
        }
    }

    pub(crate) async fn copy_all_to<W>(&self, writer: &mut W) -> Result<u64, ResponseReadProblem>
    where
        W: AsyncWrite + Unpin,
    {
        match &self.0.body {
            StoredBody::Empty => Ok(0),
            StoredBody::Memory(bytes) => {
                writer
                    .write_all(bytes)
                    .await
                    .map_err(|_| ResponseReadProblem::Read)?;
                Ok(bytes.len() as u64)
            }
            StoredBody::TempFile { path, len } => {
                let mut file = tokio::fs::File::open(path.as_ref() as &std::path::Path)
                    .await
                    .map_err(|_| ResponseReadProblem::Open)?;
                let mut remaining = *len;
                let mut copied = 0_u64;
                let mut buffer = vec![0; 64 * 1024];

                while remaining > 0 {
                    let requested = remaining.min(buffer.len() as u64) as usize;
                    let read = file
                        .read(&mut buffer[..requested])
                        .await
                        .map_err(|_| ResponseReadProblem::Read)?;
                    if read == 0 {
                        return Err(ResponseReadProblem::LengthMismatch);
                    }
                    writer
                        .write_all(&buffer[..read])
                        .await
                        .map_err(|_| ResponseReadProblem::Read)?;
                    let read = read as u64;
                    copied += read;
                    remaining -= read;
                }

                let mut extra = [0_u8; 1];
                if file
                    .read(&mut extra)
                    .await
                    .map_err(|_| ResponseReadProblem::Read)?
                    != 0
                {
                    return Err(ResponseReadProblem::LengthMismatch);
                }
                Ok(copied)
            }
        }
    }
}

impl fmt::Debug for ResponseReadLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseReadLease")
            .field("len", &self.len())
            .finish()
    }
}

pub(crate) struct PrefixBytes {
    pub(crate) bytes: Bytes,
    pub(crate) complete: bool,
}

impl fmt::Debug for PrefixBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrefixBytes")
            .field("len", &self.bytes.len())
            .field("complete", &self.complete)
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ResponseReadProblem {
    Open,
    Read,
    LengthMismatch,
    LimitExceeded,
}

impl fmt::Display for ResponseReadProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Open => "response body could not be opened",
            Self::Read => "response body could not be read",
            Self::LengthMismatch => "response body length changed unexpectedly",
            Self::LimitExceeded => "response body exceeds the requested read limit",
        })
    }
}

impl fmt::Debug for ResponseReadProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Error for ResponseReadProblem {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::NamedTempFile;
    use tokio::io::AsyncWriteExt as _;

    use super::*;

    fn head() -> ResponseHead {
        let mut headers = HeaderMap::new();
        headers.insert("x-secret", "header-secret".parse().unwrap());
        ResponseHead::new(
            StatusCode::OK,
            Version::HTTP_11,
            Url::parse("https://example.test/?token=secret").unwrap(),
            headers,
        )
    }

    fn response(body: StoredBody) -> Arc<ResponseData> {
        let len = body.len();
        Arc::new(ResponseData::new(
            head(),
            ResponseTiming {
                head_after: Duration::from_millis(1),
                completed_after: Duration::from_millis(2),
            },
            CompletedBody {
                body,
                body_decoding: BodyDecoding::Identity,
                sizes: ResponseSizes {
                    declared_encoded_bytes: Some(len),
                    received_encoded_bytes: len,
                    stored_body_bytes: len,
                },
            },
        ))
    }

    #[test]
    fn diagnostics_do_not_expose_response_values_or_paths() {
        let response = response(StoredBody::Memory(Bytes::from_static(b"body-secret")));
        let diagnostic = format!("{response:?} {:?}", response.read_lease());
        assert!(!diagnostic.contains("token=secret"));
        assert!(!diagnostic.contains("header-secret"));
        assert!(!diagnostic.contains("body-secret"));
    }

    #[tokio::test]
    async fn memory_prefix_and_copy_use_exact_body_length() {
        let response = response(StoredBody::Memory(Bytes::from_static(b"abcdef")));
        let lease = response.read_lease();

        let prefix = lease.read_prefix(3).await.unwrap();
        assert_eq!(&prefix.bytes[..], b"abc");
        assert!(!prefix.complete);

        let mut copied = Vec::new();
        assert_eq!(lease.copy_all_to(&mut copied).await.unwrap(), 6);
        assert_eq!(copied, b"abcdef");
    }

    #[tokio::test]
    async fn lease_keeps_temp_path_alive_after_response_owner_is_released() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"temporary body").unwrap();
        let path = file.path().to_path_buf();
        let (_file, temp_path) = file.into_parts();
        let response = response(StoredBody::temp_file(temp_path, 14));
        let lease = response.read_lease();

        drop(response);
        assert!(path.exists());
        let prefix = lease.read_prefix(usize::MAX).await.unwrap();
        assert_eq!(&prefix.bytes[..], b"temporary body");
        assert!(prefix.complete);

        drop(lease);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn temp_file_length_mismatch_is_rejected() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"short").unwrap();
        let (_file, temp_path) = file.into_parts();
        let response = response(StoredBody::temp_file(temp_path, 8));
        let lease = response.read_lease();

        assert_eq!(
            lease.read_prefix(8).await.unwrap_err(),
            ResponseReadProblem::LengthMismatch
        );
        let mut target = Vec::new();
        assert_eq!(
            lease.copy_all_to(&mut target).await.unwrap_err(),
            ResponseReadProblem::LengthMismatch
        );
    }

    #[tokio::test]
    async fn temp_file_growth_is_rejected_by_full_copy() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"body-plus-extra").unwrap();
        let (_file, temp_path) = file.into_parts();
        let response = response(StoredBody::temp_file(temp_path, 4));
        let lease = response.read_lease();
        let mut target = tokio::io::sink();

        assert_eq!(
            lease.copy_all_to(&mut target).await.unwrap_err(),
            ResponseReadProblem::LengthMismatch
        );
        assert_eq!(
            lease.read_all_bounded(4).await.unwrap_err(),
            ResponseReadProblem::LengthMismatch
        );
        target.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn bounded_full_read_rejects_before_allocating_and_returns_exact_bytes() {
        let response = response(StoredBody::Memory(Bytes::from_static(b"abcdef")));
        let lease = response.read_lease();

        assert_eq!(
            lease.read_all_bounded(5).await.unwrap_err(),
            ResponseReadProblem::LimitExceeded
        );
        assert_eq!(&lease.read_all_bounded(6).await.unwrap()[..], b"abcdef");
    }
}
