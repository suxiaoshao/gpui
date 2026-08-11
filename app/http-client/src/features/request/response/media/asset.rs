use std::{error::Error, fmt};

use tempfile::{NamedTempFile, TempPath};
use tokio::io::AsyncWriteExt as _;
use url::Url;

use super::super::{ResponseReadLease, ResponseReadProblem};

/// A session-private exact copy of a complete response body.
///
/// The original [`ResponseData`](super::super::ResponseData) remains the sole
/// authority. This asset only gives a media driver a disposable file with an
/// unambiguous lifetime. It is intentionally non-cloneable and never exposes a
/// filesystem path outside this response-local module.
pub(crate) struct ResponseAssetLease {
    _path: TempPath,
    uri: Url,
    len: u64,
}

impl ResponseAssetLease {
    pub(crate) fn uri(&self) -> &Url {
        &self.uri
    }

    #[cfg(test)]
    pub(crate) const fn len(&self) -> u64 {
        self.len
    }
}

impl fmt::Debug for ResponseAssetLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseAssetLease")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResponseAssetProblemKind {
    ReadResponse,
    CreateTemporaryAsset,
    WriteTemporaryAsset,
    InvalidTemporaryAssetUri,
}

pub(crate) struct ResponseAssetProblem {
    kind: ResponseAssetProblemKind,
}

impl ResponseAssetProblem {
    pub(crate) const fn kind(&self) -> ResponseAssetProblemKind {
        self.kind
    }

    const fn new(kind: ResponseAssetProblemKind) -> Self {
        Self { kind }
    }
}

impl fmt::Debug for ResponseAssetProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseAssetProblem")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for ResponseAssetProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            ResponseAssetProblemKind::ReadResponse => "response body could not be copied",
            ResponseAssetProblemKind::CreateTemporaryAsset => {
                "media temporary asset could not be created"
            }
            ResponseAssetProblemKind::WriteTemporaryAsset => {
                "media temporary asset could not be written"
            }
            ResponseAssetProblemKind::InvalidTemporaryAssetUri => {
                "media temporary asset could not be addressed"
            }
        })
    }
}

impl Error for ResponseAssetProblem {}

impl ResponseReadLease {
    /// Materializes an exact, session-owned response copy for a media driver.
    ///
    /// This path is deliberately identical for in-memory and spilled response
    /// storage. In particular, a media driver never receives the collector's
    /// private spill file path.
    pub(crate) async fn materialize_media_asset(
        &self,
    ) -> Result<ResponseAssetLease, ResponseAssetProblem> {
        let expected_len = self.len();
        let (file, path) = create_temporary_asset().await?;
        let mut file = tokio::fs::File::from_std(file);

        let copied = self
            .copy_all_to(&mut file)
            .await
            .map_err(map_read_problem)?;
        if copied != expected_len {
            return Err(ResponseAssetProblem::new(
                ResponseAssetProblemKind::ReadResponse,
            ));
        }
        file.flush().await.map_err(|_| {
            ResponseAssetProblem::new(ResponseAssetProblemKind::WriteTemporaryAsset)
        })?;
        file.shutdown().await.map_err(|_| {
            ResponseAssetProblem::new(ResponseAssetProblemKind::WriteTemporaryAsset)
        })?;
        drop(file);

        let uri = Url::from_file_path(path.as_ref() as &std::path::Path).map_err(|_| {
            ResponseAssetProblem::new(ResponseAssetProblemKind::InvalidTemporaryAssetUri)
        })?;
        Ok(ResponseAssetLease {
            _path: path,
            uri,
            len: copied,
        })
    }
}

async fn create_temporary_asset() -> Result<(std::fs::File, TempPath), ResponseAssetProblem> {
    tokio::task::spawn_blocking(|| {
        NamedTempFile::new()
            .map(NamedTempFile::into_parts)
            .map_err(|_| ResponseAssetProblem::new(ResponseAssetProblemKind::CreateTemporaryAsset))
    })
    .await
    .map_err(|_| ResponseAssetProblem::new(ResponseAssetProblemKind::CreateTemporaryAsset))?
}

const fn map_read_problem(_: ResponseReadProblem) -> ResponseAssetProblem {
    ResponseAssetProblem::new(ResponseAssetProblemKind::ReadResponse)
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use bytes::Bytes;
    use http::{HeaderMap, StatusCode, Version};
    use url::Url;

    use super::*;
    use crate::features::request::response::{
        BodyDecoding, CompletedBody, ResponseData, ResponseHead, ResponseSizes, ResponseTiming,
        StoredBody,
    };

    fn response(body: StoredBody) -> Arc<ResponseData> {
        let len = body.len();
        Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/private?token=value").unwrap(),
                HeaderMap::new(),
            ),
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

    #[tokio::test]
    async fn materialized_asset_is_exact_and_redacts_its_path() {
        let source = response(StoredBody::Memory(Bytes::from_static(b"media-body")));
        let asset = source.read_lease().materialize_media_asset().await.unwrap();
        let asset_path = asset.uri().to_file_path().unwrap();

        assert_eq!(asset.len(), 10);
        assert_eq!(tokio::fs::read(&asset_path).await.unwrap(), b"media-body");
        assert!(!format!("{asset:?}").contains(&asset_path.display().to_string()));

        drop(asset);
        assert!(!asset_path.exists());
    }

    #[tokio::test]
    async fn spilled_body_is_copied_to_a_distinct_session_asset() {
        let source_file = NamedTempFile::new().unwrap();
        let source_path = source_file.path().to_owned();
        tokio::fs::write(&source_path, b"spilled-media")
            .await
            .unwrap();
        let source = response(StoredBody::TempFile {
            path: source_file.into_temp_path(),
            len: 13,
        });

        let asset = source.read_lease().materialize_media_asset().await.unwrap();
        let asset_path = asset.uri().to_file_path().unwrap();
        assert_ne!(asset_path, source_path);
        assert_eq!(
            tokio::fs::read(&asset_path).await.unwrap(),
            b"spilled-media"
        );
        assert!(source_path.exists());

        drop(asset);
        assert!(!asset_path.exists());
        assert!(source_path.exists());
    }
}
