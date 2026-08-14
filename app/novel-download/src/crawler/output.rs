use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tempfile::{NamedTempFile, TempPath};

use super::{ContentItem, NovelMetadata};
use crate::errors::{CleanupProblem, DownloadFailure, OutputOperation, OutputProblem};

const MAX_COMPONENT_BYTES: usize = 96;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutputPaths {
    final_path: PathBuf,
    part_path: PathBuf,
}

impl OutputPaths {
    #[cfg(test)]
    pub(crate) fn final_path(&self) -> &Path {
        &self.final_path
    }

    #[cfg(test)]
    pub(crate) fn part_path(&self) -> &Path {
        &self.part_path
    }

    fn from_metadata(root: &Path, metadata: &NovelMetadata) -> Result<Self, OutputProblem> {
        Self::from_components(root, metadata.name(), metadata.author())
    }

    fn from_components(root: &Path, name: &str, author: &str) -> Result<Self, OutputProblem> {
        if !root.is_absolute() {
            return Err(OutputProblem::Io {
                operation: OutputOperation::Create,
                path: root.to_path_buf(),
                source: io::Error::new(io::ErrorKind::InvalidInput, "output root must be absolute"),
            });
        }
        let name = safe_filename_component(name).ok_or(OutputProblem::InvalidFileName)?;
        let author = safe_filename_component(author).ok_or(OutputProblem::InvalidFileName)?;
        let final_path = root.join(format!("{name}by{author}.txt"));
        let mut part_name = OsString::from(
            final_path
                .file_name()
                .expect("a path joined with a filename always has a filename"),
        );
        part_name.push(".part");
        let part_path = final_path.with_file_name(part_name);

        Ok(Self {
            final_path,
            part_path,
        })
    }
}

/// Carries an owned staging file's cleanup failure across an aborted worker.
///
/// The tracker deliberately does not retain a path that it could remove later:
/// only the `TempPath` owner may unlink the staging file, so a replacement at
/// the same path can never be mistaken for this run's file.
#[derive(Clone, Debug, Default)]
pub(crate) struct StagingTracker {
    cleanup_problem: Arc<Mutex<Option<CleanupProblem>>>,
}

impl StagingTracker {
    fn begin(&self) {
        self.with_cleanup_problem(|problem| *problem = None);
    }

    fn clear(&self) {
        self.with_cleanup_problem(|problem| *problem = None);
    }

    fn record_cleanup_problem(&self, problem: CleanupProblem) {
        self.with_cleanup_problem(|slot| *slot = Some(problem));
    }

    pub(crate) fn take_cleanup_problem(&self) -> Option<CleanupProblem> {
        self.with_cleanup_problem(Option::take)
    }

    fn with_cleanup_problem<T>(&self, f: impl FnOnce(&mut Option<CleanupProblem>) -> T) -> T {
        let mut problem = match self.cleanup_problem.lock() {
            Ok(problem) => problem,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut problem)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutputCommit {
    final_path: PathBuf,
    items_written: usize,
}

impl OutputCommit {
    pub(crate) fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub(crate) fn items_written(&self) -> usize {
        self.items_written
    }
}

pub(crate) struct StagedOutput {
    paths: OutputPaths,
    file: Option<NamedTempFile<File>>,
    items_written: usize,
    tracker: StagingTracker,
}

impl StagedOutput {
    pub(crate) fn create(
        root: &Path,
        metadata: &NovelMetadata,
        tracker: StagingTracker,
    ) -> Result<Self, OutputProblem> {
        let paths = OutputPaths::from_metadata(root, metadata)?;
        Self::create_at(paths, tracker)
    }

    pub(crate) fn write_item(&mut self, item: &ContentItem) -> Result<(), OutputProblem> {
        let file = self
            .file
            .as_mut()
            .expect("staged output remains open until commit or abort");
        file.as_file_mut()
            .write_all(item.content().as_bytes())
            .map_err(|source| self.io_problem(OutputOperation::Write, source))?;
        self.items_written += 1;
        Ok(())
    }

    pub(crate) fn commit(mut self) -> Result<OutputCommit, DownloadFailure> {
        let flush_result = self
            .file
            .as_mut()
            .expect("staged output remains open until commit or abort")
            .as_file_mut()
            .flush();
        if let Err(source) = flush_result {
            let problem = self.io_problem(OutputOperation::Flush, source);
            return Err(self.with_abort(problem));
        }

        let sync_result = self
            .file
            .as_mut()
            .expect("staged output remains open until commit or abort")
            .as_file_mut()
            .sync_all();
        if let Err(source) = sync_result {
            let problem = self.io_problem(OutputOperation::Sync, source);
            return Err(self.with_abort(problem));
        }

        let staged_file = self
            .file
            .take()
            .expect("staged output remains open until commit or abort");
        let (file, part_path) = staged_file.into_parts();
        drop(file);

        match part_path.persist_noclobber(&self.paths.final_path) {
            Ok(()) => {
                self.tracker.clear();
                Ok(OutputCommit {
                    final_path: self.paths.final_path.clone(),
                    items_written: self.items_written,
                })
            }
            Err(error) => {
                let tempfile::PathPersistError { error, path } = error;
                let problem = if error.kind() == io::ErrorKind::AlreadyExists {
                    OutputProblem::TargetExists {
                        path: self.paths.final_path.clone(),
                    }
                } else {
                    self.io_problem(OutputOperation::Promote, error)
                };
                let cleanup_problem = close_part(path, &self.tracker);
                Err(match cleanup_problem {
                    Some(cleanup_problem) => {
                        DownloadFailure::with_cleanup(problem, cleanup_problem)
                    }
                    None => DownloadFailure::new(problem),
                })
            }
        }
    }

    pub(crate) fn abort(mut self) -> Result<(), CleanupProblem> {
        let Some(file) = self.file.take() else {
            return self.tracker.take_cleanup_problem().map_or(Ok(()), Err);
        };

        close_part(file.into_temp_path(), &self.tracker).map_or(Ok(()), Err)
    }

    fn create_at(paths: OutputPaths, tracker: StagingTracker) -> Result<Self, OutputProblem> {
        ensure_absent(&paths.final_path, true)?;
        ensure_absent(&paths.part_path, false)?;

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.part_path)
            .map_err(|source| match source.kind() {
                io::ErrorKind::AlreadyExists => OutputProblem::StagingExists {
                    path: paths.part_path.clone(),
                },
                _ => OutputProblem::Io {
                    operation: OutputOperation::Create,
                    path: paths.part_path.clone(),
                    source,
                },
            })?;

        let temp_path = TempPath::try_from_path(&paths.part_path)
            .expect("OutputPaths only contains a non-empty absolute staging path");
        tracker.begin();

        Ok(Self {
            paths,
            file: Some(NamedTempFile::from_parts(file, temp_path)),
            items_written: 0,
            tracker,
        })
    }

    fn with_abort(self, problem: OutputProblem) -> DownloadFailure {
        match self.abort() {
            Ok(()) => DownloadFailure::new(problem),
            Err(cleanup_problem) => DownloadFailure::with_cleanup(problem, cleanup_problem),
        }
    }

    fn io_problem(&self, operation: OutputOperation, source: io::Error) -> OutputProblem {
        OutputProblem::Io {
            operation,
            path: if operation == OutputOperation::Promote {
                self.paths.final_path.clone()
            } else {
                self.paths.part_path.clone()
            },
            source,
        }
    }
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        let Some(file) = self.file.take() else {
            return;
        };
        let part_path = file.into_temp_path();
        let path = part_path.to_path_buf();
        match part_path.close() {
            Ok(()) => self.tracker.clear(),
            Err(source) if source.kind() == io::ErrorKind::NotFound => self.tracker.clear(),
            Err(source) => self
                .tracker
                .record_cleanup_problem(CleanupProblem::new(path, source)),
        }
    }
}

fn ensure_absent(path: &Path, final_path: bool) -> Result<(), OutputProblem> {
    match fs::symlink_metadata(path) {
        Ok(_) if final_path => Err(OutputProblem::TargetExists {
            path: path.to_path_buf(),
        }),
        Ok(_) => Err(OutputProblem::StagingExists {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(OutputProblem::Io {
            operation: OutputOperation::Create,
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn close_part(part_path: TempPath, tracker: &StagingTracker) -> Option<CleanupProblem> {
    let path = part_path.to_path_buf();
    match part_path.close() {
        Ok(()) => {
            tracker.clear();
            None
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            tracker.clear();
            None
        }
        Err(source) => {
            tracker.clear();
            Some(CleanupProblem::new(path, source))
        }
    }
}

fn safe_filename_component(value: &str) -> Option<String> {
    let mut component = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    component.truncate(component.trim_end_matches([' ', '.']).len());

    if component.is_empty() || matches!(component.as_str(), "." | "..") {
        return None;
    }

    if is_windows_reserved_name(&component) {
        component.push('_');
    }
    truncate_utf8(&mut component, MAX_COMPONENT_BYTES);

    if component.is_empty() || matches!(component.as_str(), "." | "..") {
        None
    } else {
        Some(component)
    }
}

fn is_windows_reserved_name(component: &str) -> bool {
    let base = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || base
            .strip_prefix("COM")
            .or_else(|| base.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn truncate_utf8(value: &mut String, maximum_bytes: usize) {
    while value.len() > maximum_bytes {
        value.pop();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn safe_filename_components_are_portable_and_bounded() {
        assert_eq!(
            safe_filename_component(" title: chapter? "),
            Some(" title_ chapter_".into())
        );
        assert_eq!(safe_filename_component("CON"), Some("CON_".into()));
        assert_eq!(safe_filename_component("..."), None);
        assert_eq!(safe_filename_component("\0"), Some("_".into()));

        let component = safe_filename_component(&"界".repeat(100)).unwrap();
        assert!(component.len() <= MAX_COMPONENT_BYTES);
        assert!(component.is_char_boundary(component.len()));
    }

    #[test]
    fn commit_promotes_only_after_writing_and_removes_part() {
        let directory = tempdir().unwrap();
        let paths = OutputPaths::from_components(directory.path(), "Novel", "Author").unwrap();
        let tracker = StagingTracker::default();
        let mut output = StagedOutput::create_at(paths.clone(), tracker).unwrap();
        output
            .write_item(&ContentItem::new("test".into(), "chapter".into()))
            .unwrap();

        let commit = output.commit().unwrap();
        assert_eq!(commit.final_path(), paths.final_path());
        assert_eq!(commit.items_written(), 1);
        assert_eq!(fs::read_to_string(paths.final_path()).unwrap(), "chapter");
        assert!(!paths.part_path().exists());
    }

    #[test]
    fn existing_final_or_part_is_never_modified() {
        let directory = tempdir().unwrap();
        let paths = OutputPaths::from_components(directory.path(), "Novel", "Author").unwrap();
        fs::write(paths.final_path(), "existing final").unwrap();
        let result = StagedOutput::create_at(paths.clone(), StagingTracker::default());
        assert!(matches!(result, Err(OutputProblem::TargetExists { .. })));
        assert_eq!(
            fs::read_to_string(paths.final_path()).unwrap(),
            "existing final"
        );

        fs::remove_file(paths.final_path()).unwrap();
        fs::write(paths.part_path(), "existing part").unwrap();
        let result = StagedOutput::create_at(paths.clone(), StagingTracker::default());
        assert!(matches!(result, Err(OutputProblem::StagingExists { .. })));
        assert_eq!(
            fs::read_to_string(paths.part_path()).unwrap(),
            "existing part"
        );
    }

    #[test]
    fn abort_removes_only_the_owned_part() {
        let directory = tempdir().unwrap();
        let paths = OutputPaths::from_components(directory.path(), "Novel", "Author").unwrap();
        let tracker = StagingTracker::default();
        let mut output = StagedOutput::create_at(paths.clone(), tracker.clone()).unwrap();
        output
            .write_item(&ContentItem::new("test".into(), "chapter".into()))
            .unwrap();
        output.abort().unwrap();

        assert!(!paths.part_path().exists());
        assert!(!paths.final_path().exists());
        assert!(tracker.take_cleanup_problem().is_none());
    }

    #[test]
    fn staging_cleanup_failure_is_preserved_without_creating_a_final_file() {
        let directory = tempdir().unwrap();
        let paths = OutputPaths::from_components(directory.path(), "Novel", "Author").unwrap();
        fs::create_dir(paths.part_path()).unwrap();
        let tracker = StagingTracker::default();

        let cleanup = close_part(
            TempPath::try_from_path(paths.part_path()).unwrap(),
            &tracker,
        )
        .expect("a staging directory cannot be removed as a file");

        assert_eq!(cleanup.path(), paths.part_path());
        assert!(!paths.final_path().exists());
        assert!(paths.part_path().is_dir());
        assert!(tracker.take_cleanup_problem().is_none());
    }

    #[test]
    fn promotion_race_never_overwrites_the_final_file() {
        let directory = tempdir().unwrap();
        let paths = OutputPaths::from_components(directory.path(), "Novel", "Author").unwrap();
        let tracker = StagingTracker::default();
        let mut output = StagedOutput::create_at(paths.clone(), tracker).unwrap();
        output
            .write_item(&ContentItem::new("test".into(), "ours".into()))
            .unwrap();
        fs::write(paths.final_path(), "other process").unwrap();

        let error = output.commit().unwrap_err();
        assert!(matches!(
            error.problem(),
            crate::errors::DownloadProblem::Output(OutputProblem::TargetExists { .. })
        ));
        assert_eq!(
            fs::read_to_string(paths.final_path()).unwrap(),
            "other process"
        );
        assert!(!paths.part_path().exists());
    }
}
