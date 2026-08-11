use std::mem;

use bytes::Bytes;
use tempfile::{NamedTempFile, TempPath};
use tokio::io::AsyncWriteExt as _;

use super::super::runtime::{BodySizeDimension, RequestProblem};
use super::data::{ActiveBodyStorage, CAPTURE_LIMIT_BYTES, MEMORY_SPILL_BYTES, StoredBody};

pub(crate) struct BodyCollector {
    storage: CollectorStorage,
    len: u64,
}

enum CollectorStorage {
    Memory(Vec<u8>),
    TempFile {
        file: tokio::fs::File,
        path: TempPath,
    },
    Finishing,
}

impl BodyCollector {
    pub(crate) const fn new() -> Self {
        Self {
            storage: CollectorStorage::Memory(Vec::new()),
            len: 0,
        }
    }

    pub(crate) const fn len(&self) -> u64 {
        self.len
    }

    pub(crate) fn storage(&self) -> ActiveBodyStorage {
        match &self.storage {
            CollectorStorage::Memory(_) => ActiveBodyStorage::Memory,
            CollectorStorage::TempFile { .. } | CollectorStorage::Finishing => {
                ActiveBodyStorage::TempFile
            }
        }
    }

    pub(crate) async fn write(&mut self, bytes: &[u8]) -> Result<(), RequestProblem> {
        if bytes.is_empty() {
            return Ok(());
        }

        let next_len = checked_stored_len(self.len, bytes.len())?;
        if matches!(self.storage, CollectorStorage::Memory(_)) && next_len > MEMORY_SPILL_BYTES {
            self.spill_to_temp_file().await?;
        }

        match &mut self.storage {
            CollectorStorage::Memory(buffer) => buffer.extend_from_slice(bytes),
            CollectorStorage::TempFile { file, .. } => file
                .write_all(bytes)
                .await
                .map_err(RequestProblem::temporary_storage)?,
            CollectorStorage::Finishing => {
                return Err(RequestProblem::internal());
            }
        }
        self.len = next_len;
        Ok(())
    }

    pub(crate) async fn finish(mut self) -> Result<StoredBody, RequestProblem> {
        let storage = mem::replace(&mut self.storage, CollectorStorage::Finishing);
        match storage {
            CollectorStorage::Memory(buffer) if buffer.is_empty() => Ok(StoredBody::Empty),
            CollectorStorage::Memory(buffer) => {
                debug_assert_eq!(buffer.len() as u64, self.len);
                Ok(StoredBody::Memory(Bytes::from(buffer)))
            }
            CollectorStorage::TempFile { mut file, path } => {
                file.flush()
                    .await
                    .map_err(RequestProblem::temporary_storage)?;
                file.shutdown()
                    .await
                    .map_err(RequestProblem::temporary_storage)?;
                drop(file);
                Ok(StoredBody::temp_file(path, self.len))
            }
            CollectorStorage::Finishing => Err(RequestProblem::internal()),
        }
    }

    async fn spill_to_temp_file(&mut self) -> Result<(), RequestProblem> {
        let (file, path) =
            tokio::task::spawn_blocking(|| NamedTempFile::new().map(NamedTempFile::into_parts))
                .await
                .map_err(RequestProblem::temporary_storage)?
                .map_err(RequestProblem::temporary_storage)?;
        let mut file = tokio::fs::File::from_std(file);

        let existing = match mem::replace(&mut self.storage, CollectorStorage::Finishing) {
            CollectorStorage::Memory(existing) => existing,
            storage => {
                self.storage = storage;
                return Err(RequestProblem::internal());
            }
        };
        if let Err(error) = file.write_all(&existing).await {
            return Err(RequestProblem::temporary_storage(error));
        }
        self.storage = CollectorStorage::TempFile { file, path };
        Ok(())
    }
}

impl Default for BodyCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn checked_stored_len(current: u64, incoming: usize) -> Result<u64, RequestProblem> {
    let observed = current.saturating_add(incoming as u64);
    if observed > CAPTURE_LIMIT_BYTES {
        Err(RequestProblem::too_large(
            BodySizeDimension::Stored,
            CAPTURE_LIMIT_BYTES,
            observed,
        ))
    } else {
        Ok(observed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::request::runtime::RequestProblemKind;

    #[tokio::test]
    async fn empty_and_exact_spill_threshold_stay_in_memory() {
        assert!(matches!(
            BodyCollector::new().finish().await.unwrap(),
            StoredBody::Empty
        ));

        let mut collector = BodyCollector::new();
        collector
            .write(&vec![0x5a; MEMORY_SPILL_BYTES as usize])
            .await
            .unwrap();
        assert_eq!(collector.storage(), ActiveBodyStorage::Memory);
        assert!(matches!(
            collector.finish().await.unwrap(),
            StoredBody::Memory(bytes) if bytes.len() as u64 == MEMORY_SPILL_BYTES
        ));
    }

    #[tokio::test]
    async fn crossing_spill_threshold_moves_existing_and_new_bytes_once() {
        let mut collector = BodyCollector::new();
        let prefix = vec![0x41; MEMORY_SPILL_BYTES as usize];
        collector.write(&prefix).await.unwrap();
        collector.write(b"B").await.unwrap();
        assert_eq!(collector.storage(), ActiveBodyStorage::TempFile);

        let body = collector.finish().await.unwrap();
        let StoredBody::TempFile { path, len } = body else {
            panic!("collector did not spill")
        };
        assert_eq!(len, MEMORY_SPILL_BYTES + 1);
        let bytes = tokio::fs::read(path.as_ref() as &std::path::Path)
            .await
            .unwrap();
        assert_eq!(&bytes[..prefix.len()], &prefix);
        assert_eq!(bytes.last(), Some(&b'B'));
    }

    #[tokio::test]
    async fn dropping_spilled_collector_removes_partial_file() {
        let mut collector = BodyCollector::new();
        collector
            .write(&vec![0_u8; MEMORY_SPILL_BYTES as usize + 1])
            .await
            .unwrap();
        let path = match &collector.storage {
            CollectorStorage::TempFile { path, .. } => path.to_path_buf(),
            _ => panic!("collector did not spill"),
        };
        assert!(path.exists());

        drop(collector);
        assert!(!path.exists());
    }

    #[test]
    fn stored_limit_is_checked_before_any_write() {
        assert_eq!(
            checked_stored_len(CAPTURE_LIMIT_BYTES - 1, 1).unwrap(),
            CAPTURE_LIMIT_BYTES
        );
        let problem = checked_stored_len(CAPTURE_LIMIT_BYTES, 1).unwrap_err();
        assert_eq!(
            problem.kind(),
            RequestProblemKind::BodyTooLarge {
                dimension: BodySizeDimension::Stored,
                limit: CAPTURE_LIMIT_BYTES,
                observed: CAPTURE_LIMIT_BYTES + 1,
            }
        );
    }
}
