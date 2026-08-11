use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use tempfile::{NamedTempFile, TempPath};
use tokio::io::AsyncWriteExt as _;

use super::{ResponseReadLease, ResponseReadProblem};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResponseSaveProblemKind {
    InvalidTarget,
    CreateStaging,
    ReadResponse,
    WriteStaging,
    Persist,
}

pub(crate) struct ResponseSaveProblem {
    kind: ResponseSaveProblemKind,
}

impl ResponseSaveProblem {
    pub(crate) const fn kind(&self) -> ResponseSaveProblemKind {
        self.kind
    }

    const fn new(kind: ResponseSaveProblemKind) -> Self {
        Self { kind }
    }

    pub(crate) const fn prompt_failed() -> Self {
        Self::new(ResponseSaveProblemKind::InvalidTarget)
    }

    pub(crate) const fn task_failed() -> Self {
        Self::new(ResponseSaveProblemKind::Persist)
    }
}

impl fmt::Debug for ResponseSaveProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseSaveProblem")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for ResponseSaveProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "response save failed ({:?})", self.kind)
    }
}

impl Error for ResponseSaveProblem {}

pub(crate) async fn save_response(
    lease: ResponseReadLease,
    target: PathBuf,
) -> Result<(), ResponseSaveProblem> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| ResponseSaveProblem::new(ResponseSaveProblemKind::InvalidTarget))?
        .to_owned();
    let (file, staging) = create_staging(parent).await?;
    let mut file = tokio::fs::File::from_std(file);

    let copied = lease
        .copy_all_to(&mut file)
        .await
        .map_err(map_read_problem)?;
    if copied != lease.len() {
        return Err(ResponseSaveProblem::new(
            ResponseSaveProblemKind::ReadResponse,
        ));
    }
    file.flush()
        .await
        .map_err(|_| ResponseSaveProblem::new(ResponseSaveProblemKind::WriteStaging))?;
    file.shutdown()
        .await
        .map_err(|_| ResponseSaveProblem::new(ResponseSaveProblemKind::WriteStaging))?;
    drop(file);

    persist_staging(staging, target).await
}

async fn create_staging(parent: PathBuf) -> Result<(std::fs::File, TempPath), ResponseSaveProblem> {
    tokio::task::spawn_blocking(move || {
        NamedTempFile::new_in(parent)
            .map(NamedTempFile::into_parts)
            .map_err(|_| ResponseSaveProblem::new(ResponseSaveProblemKind::CreateStaging))
    })
    .await
    .map_err(|_| ResponseSaveProblem::new(ResponseSaveProblemKind::CreateStaging))?
}

async fn persist_staging(staging: TempPath, target: PathBuf) -> Result<(), ResponseSaveProblem> {
    tokio::task::spawn_blocking(move || {
        staging
            .persist(target)
            .map(|_| ())
            .map_err(|_| ResponseSaveProblem::new(ResponseSaveProblemKind::Persist))
    })
    .await
    .map_err(|_| ResponseSaveProblem::new(ResponseSaveProblemKind::Persist))?
}

const fn map_read_problem(_: ResponseReadProblem) -> ResponseSaveProblem {
    ResponseSaveProblem::new(ResponseSaveProblemKind::ReadResponse)
}

pub(crate) fn suggested_response_name(head_content_type: Option<&str>) -> &'static str {
    match head_content_type {
        Some(content_type) if content_type.contains("json") => "response.json",
        Some(content_type) if content_type.contains("xml") => "response.xml",
        Some(content_type) if content_type.starts_with("text/html") => "response.html",
        Some(content_type) if content_type.starts_with("text/") => "response.txt",
        _ => "response.bin",
    }
}

pub(crate) fn initial_save_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_owned())
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

    fn memory_response(bytes: &'static [u8]) -> Arc<ResponseData> {
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
                body: StoredBody::Memory(Bytes::from_static(bytes)),
                body_decoding: BodyDecoding::Identity,
                sizes: ResponseSizes {
                    declared_encoded_bytes: Some(bytes.len() as u64),
                    received_encoded_bytes: bytes.len() as u64,
                    stored_body_bytes: bytes.len() as u64,
                },
            },
        ))
    }

    #[tokio::test]
    async fn save_replaces_the_confirmed_target_with_exact_response_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("response.bin");
        tokio::fs::write(&target, b"old").await.unwrap();
        let response = memory_response(b"complete response");

        save_response(response.read_lease(), target.clone())
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(target).await.unwrap(), b"complete response");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn save_copies_a_temp_file_response_with_its_exact_length() {
        let source = tempfile::NamedTempFile::new().unwrap();
        let source_path = source.path().to_owned();
        tokio::fs::write(&source_path, b"spilled response")
            .await
            .unwrap();
        let body = StoredBody::TempFile {
            path: source.into_temp_path(),
            len: 16,
        };
        let response = Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/large").unwrap(),
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
                    declared_encoded_bytes: Some(16),
                    received_encoded_bytes: 16,
                    stored_body_bytes: 16,
                },
            },
        ));
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("response.bin");

        save_response(response.read_lease(), target.clone())
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(target).await.unwrap(), b"spilled response");
        assert!(source_path.exists());
        drop(response);
        assert!(!source_path.exists());
    }

    #[tokio::test]
    async fn invalid_target_does_not_create_a_partial_output() {
        let response = memory_response(b"body");
        let error = save_response(response.read_lease(), PathBuf::from("response.bin"))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ResponseSaveProblemKind::InvalidTarget);
    }

    #[test]
    fn suggested_name_uses_only_the_media_type_family() {
        assert_eq!(
            suggested_response_name(Some("application/problem+json")),
            "response.json"
        );
        assert_eq!(
            suggested_response_name(Some("text/plain; charset=utf-8")),
            "response.txt"
        );
        assert_eq!(suggested_response_name(None), "response.bin");
    }
}
