use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

pub(crate) struct FileLock {
    file: File,
}

impl FileLock {
    pub(crate) fn acquire(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        file.try_lock()?;
        Ok(Self { file })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub(crate) fn atomic_replace(
    path: &Path,
    expected: Option<&[u8]>,
    contents: &[u8],
) -> std::io::Result<Vec<u8>> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })?;
    fs::create_dir_all(parent)?;

    compare_current(path, expected)?;
    let mut staged = NamedTempFile::new_in(parent)?;
    staged.write_all(contents)?;
    staged.flush()?;
    staged.as_file().sync_all()?;
    compare_current(path, expected)?;
    staged.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)?;

    let committed = fs::read(path)?;
    if committed != contents {
        return Err(std::io::Error::other(format!(
            "verified bytes differ after replacing {}",
            path.display()
        )));
    }
    Ok(committed)
}

pub(crate) fn copy_new_synced(source: &[u8], path: &Path) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })?;
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(source)?;
    file.flush()?;
    file.sync_all()?;
    sync_directory(parent)
}

pub(crate) fn next_available_path(parent: &Path, stem: &str, extension: &str) -> PathBuf {
    let first = parent.join(format!("{stem}.{extension}"));
    if !first.exists() {
        return first;
    }
    for suffix in 1_u64.. {
        let candidate = parent.join(format!("{stem}-{suffix}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn compare_current(path: &Path, expected: Option<&[u8]>) -> std::io::Result<()> {
    let current = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if current.as_deref() == expected {
        Ok(())
    } else {
        Err(std::io::Error::new(
            ErrorKind::AlreadyExists,
            format!("{} changed outside Jaco", path.display()),
        ))
    }
}

fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}
