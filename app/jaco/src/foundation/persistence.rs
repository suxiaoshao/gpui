use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

#[cfg(target_os = "windows")]
use windows::{
    Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_TEMPORARY, MOVEFILE_REPLACE_EXISTING,
        MOVEFILE_WRITE_THROUGH, MoveFileExW, SetFileAttributesW,
    },
    core::PCWSTR,
};

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

    let mut staged = NamedTempFile::new_in(parent)?;
    staged.write_all(contents)?;
    staged.flush()?;
    staged.as_file().sync_all()?;
    match expected {
        None => persist_staged_noclobber(staged, path, parent)?,
        Some(expected) => {
            compare_current(path, Some(expected))?;
            persist_staged(staged, path, parent)?;
        }
    }

    let committed = fs::read(path)?;
    if committed != contents {
        return Err(std::io::Error::other(format!(
            "verified bytes differ after replacing {}",
            path.display()
        )));
    }
    Ok(committed)
}

fn persist_staged_noclobber(
    staged: NamedTempFile,
    path: &Path,
    parent: &Path,
) -> std::io::Result<()> {
    staged
        .persist_noclobber(path)
        .map_err(|error| error.error)?;
    sync_directory(parent)
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

#[cfg(not(target_os = "windows"))]
fn persist_staged(staged: NamedTempFile, path: &Path, parent: &Path) -> std::io::Result<()> {
    staged.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)
}

#[cfg(target_os = "windows")]
fn persist_staged(staged: NamedTempFile, path: &Path, _parent: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;

    let staged_path = staged
        .path()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    // NamedTempFile marks the staged file as temporary on Windows. Persisting
    // it must first restore normal file semantics, just as tempfile::persist
    // does, before the write-through replacement makes the rename durable.
    unsafe {
        SetFileAttributesW(PCWSTR(staged_path.as_ptr()), FILE_ATTRIBUTE_NORMAL)
            .map_err(std::io::Error::from)?;
        if let Err(error) = MoveFileExW(
            PCWSTR(staged_path.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        ) {
            let _ = SetFileAttributesW(PCWSTR(staged_path.as_ptr()), FILE_ATTRIBUTE_TEMPORARY);
            return Err(std::io::Error::from(error));
        }
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(target_os = "windows")]
pub(crate) fn sync_directory(_path: &Path) -> std::io::Result<()> {
    // Windows cannot open a directory with std::fs::File. Atomic replacement
    // uses MOVEFILE_WRITE_THROUGH above, while newly created files have already
    // been flushed and synced through their own file handle.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::atomic_replace;
    use std::io::ErrorKind;

    #[test]
    fn atomic_replace_creates_and_replaces_the_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");

        assert_eq!(
            atomic_replace(&path, None, b"first").expect("create destination"),
            b"first"
        );
        assert_eq!(
            atomic_replace(&path, Some(b"first"), b"second").expect("replace destination"),
            b"second"
        );
        assert_eq!(std::fs::read(path).expect("read destination"), b"second");
    }

    #[test]
    fn atomic_create_never_overwrites_an_external_winner() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        std::fs::write(&path, b"external").expect("write external winner");

        let error = atomic_replace(&path, None, b"default")
            .expect_err("create-if-absent must reject an existing destination");

        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(path).expect("read external winner"),
            b"external"
        );
    }
}
