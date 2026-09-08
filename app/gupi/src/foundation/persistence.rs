use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Failure {
    #[error("file changed externally")]
    Conflict,
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("commit needs reconciliation: {0}")]
    NeedsReconcile(io::Error),
}

pub(crate) fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// A backup is retained even if a later commit fails.
pub(crate) fn backup(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let mut file = tempfile::Builder::new()
        .prefix("config-backup-")
        .suffix(".toml")
        .tempfile_in(
            path.parent()
                .ok_or_else(|| io::Error::other("missing parent"))?,
        )?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    let (_, backup) = file.keep().map_err(|e| e.error)?;
    Ok(backup)
}

pub(crate) fn replace(path: &Path, expected: Option<&[u8]>, bytes: &[u8]) -> Result<(), Failure> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    fs::create_dir_all(parent)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    lock.try_lock().map_err(io::Error::from)?;
    if read(path)?.as_deref() != expected {
        return Err(Failure::Conflict);
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if read(path)?.as_deref() != expected {
        return Err(Failure::Conflict);
    }
    temp.persist(path).map_err(|e| Failure::Io(e.error))?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(Failure::NeedsReconcile)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflict_preserves_external_version_and_backup_is_independent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        replace(&path, None, b"first").unwrap();
        fs::write(&path, b"external").unwrap();
        assert!(matches!(
            replace(&path, Some(b"first"), b"draft"),
            Err(Failure::Conflict)
        ));
        let copy = backup(&path, b"external").unwrap();
        replace(&path, Some(b"external"), b"draft").unwrap();
        assert_eq!(fs::read(copy).unwrap(), b"external");
        assert_eq!(fs::read(path).unwrap(), b"draft");
    }
}
