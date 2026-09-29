//! Bounded local diagnostics: current log plus three 5 MiB archives.
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 5 * 1024 * 1024;
const ARCHIVES: usize = 3;
pub(crate) struct LogWriter {
    directory: PathBuf,
    file: Option<File>,
    bytes: u64,
}
impl LogWriter {
    pub fn open(directory: PathBuf) -> io::Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let file = open_file(&directory)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            directory,
            file: Some(file),
            bytes,
        })
    }
    fn rotate(&mut self) -> io::Result<()> {
        // Close before renaming, including on Windows.
        self.file.take();
        let result = (|| {
            let oldest = self.directory.join(format!("gupi.log.{ARCHIVES}"));
            match std::fs::remove_file(oldest) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            for index in (0..ARCHIVES).rev() {
                let name = if index == 0 {
                    "gupi.log".to_owned()
                } else {
                    format!("gupi.log.{index}")
                };
                match std::fs::rename(
                    self.directory.join(name),
                    self.directory.join(format!("gupi.log.{}", index + 1)),
                ) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        })();
        let file = open_file(&self.directory)?;
        self.bytes = file.metadata()?.len();
        self.file = Some(file);
        result
    }
}
fn open_file(directory: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("gupi.log"))
}
impl Write for LogWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.bytes > 0 && self.bytes.saturating_add(buffer.len() as u64) > MAX_BYTES {
            self.rotate()?;
        }
        let written = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file unavailable"))?
            .write(buffer)?;
        self.bytes += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file unavailable"))?
            .flush()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotates_existing_log_and_keeps_only_three_archives() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("gupi.log"), b"old").unwrap();
        let mut log = LogWriter::open(directory.path().to_owned()).unwrap();
        for index in 0..5 {
            log.bytes = MAX_BYTES;
            writeln!(log, "{index}").unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(directory.path().join("gupi.log")).unwrap(),
            "4\n"
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("gupi.log.3")).unwrap(),
            "1\n"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 4);
    }
}
