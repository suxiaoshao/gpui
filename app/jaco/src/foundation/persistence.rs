use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
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

pub(crate) fn atomic_copy_file(source: &Path, target: &Path) -> std::io::Result<u64> {
    let parent = target.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("path has no parent: {}", target.display()),
        )
    })?;
    let mut source = File::open(source)?;
    let mut staged = NamedTempFile::new_in(parent)?;
    let copied = io::copy(&mut source, staged.as_file_mut())?;
    // Release the source before replacing the target so copying a file onto
    // itself also works on Windows.
    drop(source);
    staged.as_file_mut().flush()?;
    staged.as_file().sync_all()?;
    persist_staged_copy(staged, target, parent)?;
    Ok(copied)
}

#[cfg(not(target_os = "windows"))]
fn persist_staged_copy(staged: NamedTempFile, path: &Path, parent: &Path) -> std::io::Result<()> {
    staged.persist(path).map_err(|error| error.error)?;
    if let Err(error) = sync_directory(parent) {
        tracing::warn!(
            error_kind = ?error.kind(),
            "attachment copy committed but directory durability sync failed"
        );
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn persist_staged_copy(staged: NamedTempFile, path: &Path, parent: &Path) -> std::io::Result<()> {
    persist_staged(staged, path, parent)
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
    use super::{atomic_copy_file, atomic_replace};
    use std::{collections::BTreeSet, fs, io::ErrorKind};

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

    #[test]
    fn atomic_copy_file_creates_the_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.bin");
        let target = directory.path().join("copy.bin");
        let contents = b"streamed source contents";
        fs::write(&source, contents).expect("write source");

        assert_eq!(
            atomic_copy_file(&source, &target).expect("create destination"),
            contents.len() as u64
        );
        assert_eq!(fs::read(target).expect("read destination"), contents);
    }

    #[test]
    fn atomic_copy_file_replaces_the_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.bin");
        let target = directory.path().join("copy.bin");
        fs::write(&source, b"new contents").expect("write source");
        fs::write(&target, b"old contents").expect("write destination");

        assert_eq!(
            atomic_copy_file(&source, &target).expect("replace destination"),
            12
        );
        assert_eq!(fs::read(target).expect("read destination"), b"new contents");
    }

    #[test]
    fn atomic_copy_file_supports_the_same_source_and_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("same.bin");
        let contents = b"same source and destination";
        fs::write(&path, contents).expect("write source");

        assert_eq!(
            atomic_copy_file(&path, &path).expect("copy onto source"),
            contents.len() as u64
        );
        assert_eq!(fs::read(path).expect("read destination"), contents);
    }

    #[test]
    fn atomic_copy_file_failure_preserves_destination_and_cleans_stage() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.bin");
        let target = directory.path().join("destination");
        fs::write(&source, b"contents that reach the staging file").expect("write source");
        fs::create_dir(&target).expect("create directory destination");
        let before = fs::read_dir(directory.path())
            .expect("list directory before copy")
            .map(|entry| entry.expect("read directory entry").file_name())
            .collect::<BTreeSet<_>>();

        atomic_copy_file(&source, &target).expect_err("directory destination must fail");

        let after = fs::read_dir(directory.path())
            .expect("list directory after copy")
            .map(|entry| entry.expect("read directory entry").file_name())
            .collect::<BTreeSet<_>>();
        assert_eq!(after, before, "failed copy must clean its staged file");
        assert!(target.is_dir(), "failed copy must preserve the destination");
    }

    #[test]
    fn atomic_copy_file_does_not_create_a_missing_parent() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.bin");
        let parent = directory.path().join("missing");
        let target = parent.join("copy.bin");
        fs::write(&source, b"contents").expect("write source");

        let error = atomic_copy_file(&source, &target).expect_err("missing parent must fail");

        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert!(!parent.exists(), "copy must not create the target parent");
    }

    #[test]
    fn atomic_copy_file_missing_source_preserves_existing_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("missing.bin");
        let target = directory.path().join("copy.bin");
        fs::write(&target, b"existing destination").expect("write destination");

        let error = atomic_copy_file(&source, &target).expect_err("missing source must fail");

        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(
            fs::read(target).expect("read destination"),
            b"existing destination"
        );
    }

    #[test]
    fn atomic_copy_file_streams_a_large_fixture_exactly() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("large-source.bin");
        let target = directory.path().join("large-copy.bin");
        let contents = (0..8 * 1024 * 1024)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&source, &contents).expect("write large source");

        assert_eq!(
            atomic_copy_file(&source, &target).expect("copy large source"),
            contents.len() as u64
        );
        assert_eq!(fs::read(target).expect("read large copy"), contents);
    }
}
