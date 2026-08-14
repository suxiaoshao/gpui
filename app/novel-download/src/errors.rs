use std::{io, path::PathBuf};

use thiserror::Error;

use crate::crawler::source::ZgzlRange;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("log directory unavailable")]
    LogDirectoryUnavailable,
    #[error("failed to initialize logging")]
    LogInitialization(#[source] io::Error),
}

pub(crate) type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(crate) enum DownloadInputError {
    #[error("download source is empty")]
    Empty,
    #[error("download source is unsupported")]
    Unsupported,
}

#[derive(Debug, Error)]
pub(crate) enum DownloadProblem {
    #[error(transparent)]
    Http(#[from] HttpProblem),
    #[error(transparent)]
    Parse(#[from] ParseProblem),
    #[error(transparent)]
    Range(#[from] RangeProblem),
    #[error(transparent)]
    Output(#[from] OutputProblem),
}

#[derive(Debug)]
pub(crate) struct DownloadFailure {
    problem: Box<DownloadProblem>,
    cleanup_problem: Option<CleanupProblem>,
}

impl DownloadFailure {
    pub(crate) fn new(problem: impl Into<DownloadProblem>) -> Self {
        Self {
            problem: Box::new(problem.into()),
            cleanup_problem: None,
        }
    }

    pub(crate) fn with_cleanup(
        problem: impl Into<DownloadProblem>,
        cleanup_problem: CleanupProblem,
    ) -> Self {
        Self {
            problem: Box::new(problem.into()),
            cleanup_problem: Some(cleanup_problem),
        }
    }

    pub(crate) fn problem(&self) -> &DownloadProblem {
        &self.problem
    }

    pub(crate) fn cleanup_problem(&self) -> Option<&CleanupProblem> {
        self.cleanup_problem.as_ref()
    }
}

impl From<DownloadProblem> for DownloadFailure {
    fn from(problem: DownloadProblem) -> Self {
        Self::new(problem)
    }
}

impl From<HttpProblem> for DownloadFailure {
    fn from(problem: HttpProblem) -> Self {
        Self::new(problem)
    }
}

impl From<ParseProblem> for DownloadFailure {
    fn from(problem: ParseProblem) -> Self {
        Self::new(problem)
    }
}

impl From<RangeProblem> for DownloadFailure {
    fn from(problem: RangeProblem) -> Self {
        Self::new(problem)
    }
}

impl From<OutputProblem> for DownloadFailure {
    fn from(problem: OutputProblem) -> Self {
        Self::new(problem)
    }
}

#[derive(Debug, Error)]
#[error("HTTP request failed after {attempts} attempt(s): {url}")]
pub(crate) struct HttpProblem {
    url: reqwest::Url,
    attempts: u8,
    #[source]
    source: reqwest::Error,
}

impl HttpProblem {
    pub(crate) fn new(url: reqwest::Url, attempts: u8, source: reqwest::Error) -> Self {
        Self {
            url,
            attempts,
            source,
        }
    }

    pub(crate) fn status(&self) -> Option<reqwest::StatusCode> {
        self.source.status()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParseStage {
    NovelMetadata,
    ChapterList,
    ChapterMetadata,
    ChapterContent,
    PageContent,
}

#[derive(Debug, Error)]
#[error("failed to parse {stage:?}: {url}")]
pub(crate) struct ParseProblem {
    url: reqwest::Url,
    stage: ParseStage,
}

impl ParseProblem {
    pub(crate) fn new(url: reqwest::Url, stage: ParseStage) -> Self {
        Self { url, stage }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RangeProblemKind {
    MissingChapter { chapter_id: String },
    PageOutOfRange { page: u32, page_count: u32 },
    EmptyRange,
}

#[derive(Debug, Error)]
#[error("requested range is unavailable")]
pub(crate) struct RangeProblem {
    requested: ZgzlRange,
    kind: RangeProblemKind,
}

impl RangeProblem {
    pub(crate) fn new(requested: ZgzlRange, kind: RangeProblemKind) -> Self {
        Self { requested, kind }
    }

    pub(crate) fn kind(&self) -> &RangeProblemKind {
        &self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputOperation {
    Create,
    Write,
    Flush,
    Sync,
    Promote,
}

#[derive(Debug, Error)]
pub(crate) enum OutputProblem {
    #[error("system Downloads directory is unavailable")]
    DownloadDirectoryUnavailable,
    #[error("novel metadata cannot form a safe output filename")]
    InvalidFileName,
    #[error("target already exists: {path:?}")]
    TargetExists { path: PathBuf },
    #[error("staging file already exists: {path:?}")]
    StagingExists { path: PathBuf },
    #[error("output {operation:?} failed for {path:?}")]
    Io {
        operation: OutputOperation,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Error)]
#[error("failed to clean staging file {path:?}")]
pub(crate) struct CleanupProblem {
    path: PathBuf,
    #[source]
    source: io::Error,
}

impl CleanupProblem {
    pub(crate) fn new(path: PathBuf, source: io::Error) -> Self {
        Self { path, source }
    }

    pub(crate) fn path(&self) -> &PathBuf {
        &self.path
    }
}
