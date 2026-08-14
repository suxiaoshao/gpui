//! App-local, read-only PDF preview state.
//!
//! The response pane owns this runtime.  The parser and `RenderCache` never
//! leave the blocking worker in [`worker`]; this module only owns the current
//! rendered page and the owner-bound event bridge task.

mod worker;

use std::{error::Error, fmt, mem, sync::Arc};

use gpui::{RenderImage, Task};
use gpui_operation::Transition;

pub(crate) use self::worker::{PdfWorkerEvent, PdfWorkerHandle};
use super::PreviewToken;

pub(super) const MAX_PDF_PAGE_COUNT: usize = 10_000;
pub(super) const MAX_PDF_PAGE_DIMENSION: u32 = 4_096;
pub(super) const MAX_PDF_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Physical target size supplied by the Response body viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfViewport {
    width: u32,
    height: u32,
}

impl PdfViewport {
    /// Creates a bounded physical viewport.  The caller is responsible for
    /// multiplying the logical GPUI bounds by the current scale factor before
    /// constructing this value.
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub(crate) const fn width(self) -> u32 {
        self.width
    }

    pub(crate) const fn height(self) -> u32 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfProblemKind {
    Parse,
    Encrypted,
    Budget,
    Render,
    Internal,
}

/// A deliberately redacted, viewer-local PDF problem.
///
/// PDF parser errors frequently include document data.  The UI only receives
/// this stable kind and must map it to Fluent text; it must never surface a
/// parser error, source path, or PDF metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PdfProblem {
    kind: PdfProblemKind,
}

impl PdfProblem {
    pub(super) const fn parse() -> Self {
        Self {
            kind: PdfProblemKind::Parse,
        }
    }

    pub(super) const fn encrypted() -> Self {
        Self {
            kind: PdfProblemKind::Encrypted,
        }
    }

    pub(super) const fn budget() -> Self {
        Self {
            kind: PdfProblemKind::Budget,
        }
    }

    pub(super) const fn render() -> Self {
        Self {
            kind: PdfProblemKind::Render,
        }
    }

    pub(super) const fn internal() -> Self {
        Self {
            kind: PdfProblemKind::Internal,
        }
    }

    pub(crate) const fn kind(self) -> PdfProblemKind {
        self.kind
    }
}

impl fmt::Display for PdfProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            PdfProblemKind::Parse => "PDF preview could not parse the document",
            PdfProblemKind::Encrypted => "PDF preview does not support encrypted documents",
            PdfProblemKind::Budget => "PDF preview exceeded its resource budget",
            PdfProblemKind::Render => "PDF preview could not render the requested page",
            PdfProblemKind::Internal => "PDF preview encountered an internal error",
        })
    }
}

impl Error for PdfProblem {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PdfPhase {
    Idle,
    Loading,
    Ready,
    Rendering,
    Failed,
}

pub(crate) struct PdfPreview {
    runtime: PdfRuntime,
}

impl PdfPreview {
    pub(crate) const fn new() -> Self {
        Self {
            runtime: PdfRuntime::Idle,
        }
    }

    pub(crate) const fn is_loading(&self) -> bool {
        matches!(
            self.runtime,
            PdfRuntime::Reading(_) | PdfRuntime::Loading(_) | PdfRuntime::Rendering(_)
        )
    }

    pub(crate) fn current_page(&self) -> Option<usize> {
        self.runtime.current_page()
    }

    pub(crate) fn page_count(&self) -> Option<usize> {
        self.runtime.page_count()
    }

    pub(crate) fn image(&self) -> Option<&Arc<RenderImage>> {
        self.runtime.image()
    }

    pub(crate) const fn viewport(&self) -> Option<PdfViewport> {
        self.runtime.viewport()
    }

    pub(crate) const fn problem(&self) -> Option<PdfProblem> {
        self.runtime.problem()
    }

    pub(crate) fn begin_read(&mut self, token: PreviewToken) {
        (&mut self.runtime).transition(PdfMessage::BeginRead { token });
    }

    pub(crate) fn can_previous(&self) -> bool {
        self.current_page().is_some_and(|page| page > 0)
    }

    pub(crate) fn can_next(&self) -> bool {
        matches!(
            (self.current_page(), self.page_count()),
            (Some(page), Some(page_count)) if page.checked_add(1).is_some_and(|next| next < page_count)
        )
    }

    /// Installs the owner-bound bridge before waking the worker.  The caller
    /// must construct the bridge lazily and pass it here before GPUI polls it.
    pub(crate) fn load(
        &mut self,
        token: PreviewToken,
        worker: PdfWorkerHandle,
        task: Task<()>,
        viewport: PdfViewport,
    ) {
        (&mut self.runtime).transition(PdfMessage::Load {
            token,
            page_generation: 0,
            worker,
            task,
            viewport,
        });
    }

    pub(crate) fn previous(&mut self, viewport: PdfViewport) {
        let Some(page) = self.current_page().and_then(|page| page.checked_sub(1)) else {
            return;
        };
        self.render_page(page, viewport);
    }

    pub(crate) fn next(&mut self, viewport: PdfViewport) {
        let Some(page) = self.current_page().and_then(|page| page.checked_add(1)) else {
            return;
        };
        self.render_page(page, viewport);
    }

    pub(crate) fn render_page(&mut self, page: usize, viewport: PdfViewport) {
        let Some(page_generation) = self.runtime.next_page_generation() else {
            return;
        };
        (&mut self.runtime).transition(PdfMessage::RenderPage {
            page,
            page_generation,
            viewport,
        });
    }

    pub(crate) fn rerender(&mut self, viewport: PdfViewport) {
        let Some(page) = self.current_page() else {
            return;
        };
        self.render_page(page, viewport);
    }

    /// Routes an event only after the ResponsePane has verified the outer
    /// response/mode/generation token.  [`PdfRuntime`] repeats the token check
    /// against its active state before accepting the message.
    pub(crate) fn handle_event(&mut self, event: PdfWorkerEvent) {
        (&mut self.runtime).transition(event.into_message());
    }

    pub(crate) fn fail_before_load(&mut self, token: PreviewToken, problem: PdfProblem) {
        (&mut self.runtime).transition(PdfMessage::LoadFailed { token, problem });
    }

    pub(crate) fn fail_if_active(&mut self, token: PreviewToken, problem: PdfProblem) {
        (&mut self.runtime).transition(PdfMessage::WorkerClosed { token, problem });
    }

    pub(crate) fn stop(&mut self) {
        (&mut self.runtime).transition(PdfMessage::Stop);
    }
}

impl Default for PdfPreview {
    fn default() -> Self {
        Self::new()
    }
}

enum PdfRuntime {
    Idle,
    Reading(PdfReading),
    Loading(PdfLoading),
    Ready(PdfReady),
    Rendering(PdfRendering),
    Failed(PdfFailed),
}

struct PdfReading {
    token: PreviewToken,
}

struct PdfLoading {
    token: PreviewToken,
    page_generation: u64,
    // Drop the completion route before stopping the worker.
    task: Task<()>,
    worker: PdfWorkerHandle,
    viewport: PdfViewport,
}

struct PdfReady {
    token: PreviewToken,
    page_generation: u64,
    task: Task<()>,
    worker: PdfWorkerHandle,
    page_count: usize,
    page: usize,
    image: Arc<RenderImage>,
    viewport: PdfViewport,
}

struct PdfRendering {
    token: PreviewToken,
    page_generation: u64,
    task: Task<()>,
    worker: PdfWorkerHandle,
    page_count: usize,
    page: usize,
    viewport: PdfViewport,
}

struct PdfFailed {
    _token: PreviewToken,
    problem: PdfProblem,
}

enum PdfMessage {
    BeginRead {
        token: PreviewToken,
    },
    Load {
        token: PreviewToken,
        page_generation: u64,
        worker: PdfWorkerHandle,
        task: Task<()>,
        viewport: PdfViewport,
    },
    LoadFailed {
        token: PreviewToken,
        problem: PdfProblem,
    },
    Loaded {
        token: PreviewToken,
        page_generation: u64,
        page_count: usize,
        page: usize,
        image: Arc<RenderImage>,
    },
    RenderPage {
        page: usize,
        page_generation: u64,
        viewport: PdfViewport,
    },
    Rendered {
        token: PreviewToken,
        page_generation: u64,
        page: usize,
        image: Arc<RenderImage>,
    },
    Failed {
        token: PreviewToken,
        page_generation: u64,
        problem: PdfProblem,
    },
    WorkerClosed {
        token: PreviewToken,
        problem: PdfProblem,
    },
    Stop,
}

impl PdfRuntime {
    const fn phase(&self) -> PdfPhase {
        match self {
            Self::Idle => PdfPhase::Idle,
            Self::Reading(_) | Self::Loading(_) => PdfPhase::Loading,
            Self::Ready(_) => PdfPhase::Ready,
            Self::Rendering(_) => PdfPhase::Rendering,
            Self::Failed(_) => PdfPhase::Failed,
        }
    }

    fn current_page(&self) -> Option<usize> {
        match self {
            Self::Ready(state) => Some(state.page),
            Self::Rendering(state) => Some(state.page),
            Self::Idle | Self::Reading(_) | Self::Loading(_) | Self::Failed(_) => None,
        }
    }

    fn page_count(&self) -> Option<usize> {
        match self {
            Self::Ready(state) => Some(state.page_count),
            Self::Rendering(state) => Some(state.page_count),
            Self::Idle | Self::Reading(_) | Self::Loading(_) | Self::Failed(_) => None,
        }
    }

    fn image(&self) -> Option<&Arc<RenderImage>> {
        match self {
            Self::Ready(state) => Some(&state.image),
            Self::Idle
            | Self::Reading(_)
            | Self::Loading(_)
            | Self::Rendering(_)
            | Self::Failed(_) => None,
        }
    }

    const fn problem(&self) -> Option<PdfProblem> {
        match self {
            Self::Failed(state) => Some(state.problem),
            Self::Idle
            | Self::Reading(_)
            | Self::Loading(_)
            | Self::Ready(_)
            | Self::Rendering(_) => None,
        }
    }

    const fn viewport(&self) -> Option<PdfViewport> {
        match self {
            Self::Loading(state) => Some(state.viewport),
            Self::Ready(state) => Some(state.viewport),
            Self::Rendering(state) => Some(state.viewport),
            Self::Idle | Self::Reading(_) | Self::Failed(_) => None,
        }
    }

    fn next_page_generation(&self) -> Option<u64> {
        match self {
            Self::Ready(state) => state.page_generation.checked_add(1),
            Self::Rendering(state) => state.page_generation.checked_add(1),
            Self::Idle | Self::Reading(_) | Self::Loading(_) | Self::Failed(_) => None,
        }
    }

    fn accepts(token: &PreviewToken, expected: &PreviewToken) -> bool {
        token.matches(expected)
    }

    fn fail_after_load(&mut self, state: PdfLoading, problem: PdfProblem) {
        *self = Self::Failed(PdfFailed {
            _token: state.token.clone(),
            problem,
        });
        drop(state);
    }

    fn fail_after_render(&mut self, state: PdfRendering, problem: PdfProblem) {
        *self = Self::Failed(PdfFailed {
            _token: state.token.clone(),
            problem,
        });
        drop(state);
    }
}

impl Transition<PdfMessage> for &mut PdfRuntime {
    type Output = ();

    fn transition(self, message: PdfMessage) {
        let current = mem::replace(self, PdfRuntime::Idle);
        match (current, message) {
            (PdfRuntime::Idle | PdfRuntime::Failed(_), PdfMessage::BeginRead { token }) => {
                *self = PdfRuntime::Reading(PdfReading { token });
            }
            (
                PdfRuntime::Reading(reading),
                PdfMessage::Load {
                    token,
                    page_generation,
                    worker,
                    task,
                    viewport,
                },
            ) if PdfRuntime::accepts(&token, &reading.token) => {
                *self = PdfRuntime::Loading(PdfLoading {
                    token: reading.token,
                    page_generation,
                    worker,
                    task,
                    viewport,
                });
                request_initial_page(self, viewport);
            }
            (PdfRuntime::Reading(reading), PdfMessage::LoadFailed { token, problem })
                if PdfRuntime::accepts(&token, &reading.token) =>
            {
                *self = PdfRuntime::Failed(PdfFailed {
                    _token: reading.token,
                    problem,
                });
            }
            (
                PdfRuntime::Loading(state),
                PdfMessage::Loaded {
                    token,
                    page_generation,
                    page_count,
                    page,
                    image,
                },
            ) if PdfRuntime::accepts(&token, &state.token)
                && page_generation == state.page_generation
                && page_count > 0
                && page_count <= MAX_PDF_PAGE_COUNT
                && page < page_count =>
            {
                *self = PdfRuntime::Ready(PdfReady {
                    token: state.token,
                    page_generation: state.page_generation,
                    worker: state.worker,
                    task: state.task,
                    page_count,
                    page,
                    image,
                    viewport: state.viewport,
                });
            }
            (
                PdfRuntime::Loading(state),
                PdfMessage::Failed {
                    token,
                    page_generation,
                    problem,
                },
            ) if PdfRuntime::accepts(&token, &state.token)
                && page_generation == state.page_generation =>
            {
                self.fail_after_load(state, problem);
            }
            (
                PdfRuntime::Ready(state),
                PdfMessage::RenderPage {
                    page,
                    page_generation,
                    viewport,
                },
            ) if page < state.page_count && page_generation > state.page_generation => {
                let PdfReady {
                    token,
                    task,
                    worker,
                    page_count,
                    image,
                    ..
                } = state;
                // Release the previous 64 MiB-budgeted frame before waking
                // the worker for the next one, preserving the single-frame
                // peak-memory contract.
                drop(image);
                *self = PdfRuntime::Rendering(PdfRendering {
                    token,
                    page_generation,
                    worker,
                    task,
                    page_count,
                    page,
                    viewport,
                });
                let PdfRuntime::Rendering(state) = self else {
                    unreachable!("PDF rendering state was installed immediately above");
                };
                if let Err(problem) = state.worker.render(
                    state.token.clone(),
                    state.page_generation,
                    state.page,
                    viewport,
                ) {
                    let failed = mem::replace(self, PdfRuntime::Idle);
                    let PdfRuntime::Rendering(state) = failed else {
                        unreachable!("PDF rendering state was replaced immediately above");
                    };
                    self.fail_after_render(state, problem);
                }
            }
            (
                PdfRuntime::Rendering(mut state),
                PdfMessage::RenderPage {
                    page,
                    page_generation,
                    viewport,
                },
            ) if page < state.page_count && page_generation > state.page_generation => {
                state.page_generation = page_generation;
                state.page = page;
                state.viewport = viewport;
                let requested = state.worker.render(
                    state.token.clone(),
                    state.page_generation,
                    state.page,
                    viewport,
                );
                match requested {
                    Ok(()) => *self = PdfRuntime::Rendering(state),
                    Err(problem) => self.fail_after_render(state, problem),
                }
            }
            (
                PdfRuntime::Rendering(state),
                PdfMessage::Rendered {
                    token,
                    page_generation,
                    page,
                    image,
                },
            ) if PdfRuntime::accepts(&token, &state.token)
                && page_generation == state.page_generation
                && page == state.page =>
            {
                *self = PdfRuntime::Ready(PdfReady {
                    token: state.token,
                    page_generation: state.page_generation,
                    worker: state.worker,
                    task: state.task,
                    page_count: state.page_count,
                    page: state.page,
                    image,
                    viewport: state.viewport,
                });
            }
            (
                PdfRuntime::Rendering(state),
                PdfMessage::Failed {
                    token,
                    page_generation,
                    problem,
                },
            ) if PdfRuntime::accepts(&token, &state.token)
                && page_generation == state.page_generation =>
            {
                self.fail_after_render(state, problem);
            }
            (PdfRuntime::Loading(state), PdfMessage::WorkerClosed { token, problem })
                if PdfRuntime::accepts(&token, &state.token) =>
            {
                self.fail_after_load(state, problem);
            }
            (PdfRuntime::Rendering(state), PdfMessage::WorkerClosed { token, problem })
                if PdfRuntime::accepts(&token, &state.token) =>
            {
                self.fail_after_render(state, problem);
            }
            (current, PdfMessage::Stop) => {
                *self = PdfRuntime::Idle;
                drop(current);
            }
            (current, message) => {
                let phase = current.phase();
                *self = current;
                tracing::debug!(
                    operation = "http-response-pdf",
                    ?phase,
                    message = pdf_message_kind(&message),
                    "ignored PDF preview message"
                );
                drop(message);
            }
        }
    }
}

fn pdf_message_kind(message: &PdfMessage) -> &'static str {
    match message {
        PdfMessage::BeginRead { .. } => "BeginRead",
        PdfMessage::Load { .. } => "Load",
        PdfMessage::LoadFailed { .. } => "LoadFailed",
        PdfMessage::Loaded { .. } => "Loaded",
        PdfMessage::RenderPage { .. } => "RenderPage",
        PdfMessage::Rendered { .. } => "Rendered",
        PdfMessage::Failed { .. } => "Failed",
        PdfMessage::WorkerClosed { .. } => "WorkerClosed",
        PdfMessage::Stop => "Stop",
    }
}

fn request_initial_page(runtime: &mut PdfRuntime, viewport: PdfViewport) {
    let PdfRuntime::Loading(state) = runtime else {
        unreachable!("PDF load state must be installed before requesting the initial page");
    };
    if let Err(problem) = state
        .worker
        .load(state.token.clone(), state.page_generation, viewport)
    {
        let failed = mem::replace(runtime, PdfRuntime::Idle);
        let PdfRuntime::Loading(state) = failed else {
            unreachable!("PDF load state was replaced immediately above");
        };
        runtime.fail_after_load(state, problem);
    }
}

#[cfg(test)]
mod tests;
