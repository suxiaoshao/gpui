use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub(crate) fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Atomically replace application-owned state. Callers serialize writes.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
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
