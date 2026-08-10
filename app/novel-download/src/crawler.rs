mod http;
mod output;
pub(crate) mod source;

use std::{future::Future, path::PathBuf, pin::Pin};

use futures::{StreamExt, channel::mpsc::UnboundedSender};

use crate::errors::{
    DownloadFailure, DownloadProblem, OutputProblem, RangeProblem, RangeProblemKind,
};

use self::{http::HttpClient, source::zgzl::ZgzlNovel};
pub(crate) use self::{
    output::{StagedOutput, StagingTracker},
    source::{DownloadSource, PreparedDownloadRequest},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NovelMetadata {
    name: String,
    author: String,
    novel_id: String,
    chapter_ids: Vec<String>,
}

impl NovelMetadata {
    pub(crate) fn new(
        name: String,
        author: String,
        novel_id: String,
        chapter_ids: Vec<String>,
    ) -> Self {
        Self {
            name,
            author,
            novel_id,
            chapter_ids,
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn author(&self) -> &str {
        &self.author
    }

    pub(crate) fn novel_id(&self) -> &str {
        &self.novel_id
    }

    pub(crate) fn chapter_ids(&self) -> &[String] {
        &self.chapter_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContentItem {
    url: String,
    content: String,
}

impl ContentItem {
    pub(crate) fn new(url: String, content: String) -> Self {
        Self { url, content }
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn content(&self) -> &str {
        &self.content
    }
}

pub(crate) type DownloadFuture =
    Pin<Box<dyn Future<Output = Result<DownloadReceipt, DownloadFailure>> + Send + 'static>>;

pub(crate) trait DownloadBackend: Send + Sync + 'static {
    fn run(
        &self,
        request: PreparedDownloadRequest,
        events: UnboundedSender<DownloadEngineEvent>,
        staging: StagingTracker,
    ) -> DownloadFuture;
}

#[derive(Clone, Debug)]
pub(crate) struct DownloadEngine {
    output_root: OutputRoot,
}

#[derive(Clone, Debug)]
enum OutputRoot {
    SystemDownloads,
    #[cfg(test)]
    Fixed(PathBuf),
}

impl DownloadEngine {
    pub(crate) fn system_downloads() -> Self {
        Self {
            output_root: OutputRoot::SystemDownloads,
        }
    }

    #[cfg(test)]
    pub(crate) fn fixed(root: PathBuf) -> Self {
        Self {
            output_root: OutputRoot::Fixed(root),
        }
    }

    fn output_root(&self) -> Result<PathBuf, OutputProblem> {
        match &self.output_root {
            OutputRoot::SystemDownloads => {
                dirs_next::download_dir().ok_or(OutputProblem::DownloadDirectoryUnavailable)
            }
            #[cfg(test)]
            OutputRoot::Fixed(root) => Ok(root.clone()),
        }
    }

    async fn run_inner(
        self,
        request: PreparedDownloadRequest,
        events: UnboundedSender<DownloadEngineEvent>,
        staging: StagingTracker,
    ) -> Result<DownloadReceipt, DownloadFailure> {
        let http = HttpClient::new().map_err(DownloadFailure::from)?;
        let novel = match request.source() {
            DownloadSource::Zgzl(range) => ZgzlNovel::fetch_metadata(range, &http).await?,
        };
        let metadata = novel.metadata().clone();
        let _ = events.unbounded_send(DownloadEngineEvent::MetadataResolved(metadata.clone()));
        let range: source::zgzl::ResolvedRange =
            novel.resolve_range(request.source().range(), &http).await?;
        let root = self.output_root().map_err(DownloadFailure::from)?;
        let mut output = StagedOutput::create(&root, &metadata, staging)?;
        let mut stream = Box::pin(novel.content_stream(range, http));
        let mut items_written = 0usize;

        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(item) => item,
                Err(problem) => return Err(abort_with_problem(output, problem)),
            };
            if let Err(problem) = output.write_item(&item) {
                return Err(abort_with_problem(output, problem.into()));
            }
            items_written += 1;
            let _ = events.unbounded_send(DownloadEngineEvent::ContentWritten {
                url: item.url().to_string(),
                items_written,
            });
        }

        if items_written == 0 {
            return Err(abort_with_problem(
                output,
                RangeProblem::new(
                    request.source().range().clone(),
                    RangeProblemKind::EmptyRange,
                )
                .into(),
            ));
        }

        let committed = output.commit()?;
        debug_assert_eq!(committed.items_written(), items_written);
        Ok(DownloadReceipt {
            metadata,
            final_path: committed.final_path().to_path_buf(),
            items_written,
        })
    }
}

impl DownloadBackend for DownloadEngine {
    fn run(
        &self,
        request: PreparedDownloadRequest,
        events: UnboundedSender<DownloadEngineEvent>,
        staging: StagingTracker,
    ) -> DownloadFuture {
        Box::pin(self.clone().run_inner(request, events, staging))
    }
}

fn abort_with_problem(output: StagedOutput, problem: DownloadProblem) -> DownloadFailure {
    match output.abort() {
        Ok(()) => DownloadFailure::new(problem),
        Err(cleanup) => DownloadFailure::with_cleanup(problem, cleanup),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DownloadEngineEvent {
    MetadataResolved(NovelMetadata),
    ContentWritten { url: String, items_written: usize },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DownloadProgress {
    metadata: Option<NovelMetadata>,
    items_written: usize,
    current_url: Option<String>,
}

impl DownloadProgress {
    pub(crate) fn apply(&mut self, event: DownloadEngineEvent) {
        match event {
            DownloadEngineEvent::MetadataResolved(metadata) => {
                self.metadata = Some(metadata);
            }
            DownloadEngineEvent::ContentWritten { url, items_written } => {
                self.items_written = items_written;
                self.current_url = Some(url);
            }
        }
    }

    pub(crate) fn metadata(&self) -> Option<&NovelMetadata> {
        self.metadata.as_ref()
    }

    pub(crate) fn items_written(&self) -> usize {
        self.items_written
    }

    pub(crate) fn current_url(&self) -> Option<&str> {
        self.current_url.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DownloadReceipt {
    metadata: NovelMetadata,
    final_path: PathBuf,
    items_written: usize,
}

impl DownloadReceipt {
    #[cfg(test)]
    pub(crate) fn fixture(items_written: usize) -> Self {
        Self {
            metadata: NovelMetadata::new(
                "Novel".into(),
                "Author".into(),
                "novel".into(),
                vec!["chapter".into()],
            ),
            final_path: PathBuf::from("NovelbyAuthor.txt"),
            items_written,
        }
    }

    pub(crate) fn metadata(&self) -> &NovelMetadata {
        &self.metadata
    }

    pub(crate) fn final_path(&self) -> &PathBuf {
        &self.final_path
    }

    pub(crate) fn items_written(&self) -> usize {
        self.items_written
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn fixed_engine_root_is_reserved_for_isolated_tests() {
        let directory = tempdir().unwrap();
        let engine = DownloadEngine::fixed(directory.path().to_path_buf());
        assert_eq!(engine.output_root().unwrap(), directory.path());
    }
}
