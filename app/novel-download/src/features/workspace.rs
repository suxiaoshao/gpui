mod form;
mod runtime;

use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use async_compat::Compat;
use fluent_bundle::FluentArgs;
use futures::{
    Stream,
    channel::mpsc::{UnboundedReceiver, unbounded},
    channel::oneshot,
    future::{Abortable, Aborted},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    form::field as component_form_field,
    h_flex,
    input::{Input, InputState},
    label::Label,
    link::Link,
    progress::Progress,
    v_flex,
};
use gpui_form::Form;
use gpui_form_gpui_component::FormInput;
use gpui_operation::Transition;
use tracing::{Level, event};

use crate::{
    crawler::{
        DownloadBackend, DownloadEngine, DownloadEngineEvent, DownloadReceipt,
        PreparedDownloadRequest, StagingTracker,
    },
    errors::{CleanupProblem, DownloadFailure, DownloadProblem, OutputProblem, RangeProblemKind},
    foundation::{I18n, i18n::validation_message},
};

use self::{
    form::{DownloadRequest, DownloadRequestValidator},
    runtime::{DownloadEffect, DownloadMessage, DownloadRuntime, DownloadStatus},
};

enum WorkerCompletion {
    Complete(Result<DownloadReceipt, DownloadFailure>),
    Cancelled(Option<CleanupProblem>),
}

enum DriverMessage {
    Progress(DownloadEngineEvent),
    Terminal(WorkerCompletion),
}

pub struct WorkspaceView {
    form: Entity<Form<DownloadRequest>>,
    source_input: Entity<InputState>,
    _source_control: FormInput,
    runtime: DownloadRuntime,
    backend: Arc<dyn DownloadBackend>,
    focus_handle: FocusHandle,
    _form_observer: Subscription,
}

impl WorkspaceView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with_backend(Arc::new(DownloadEngine::system_downloads()), window, cx)
    }

    fn new_with_backend(
        backend: Arc<dyn DownloadBackend>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = cx.global::<I18n>().t("download-source-placeholder");
        let form = cx.new(|_| {
            Form::new(DownloadRequest::default()).with_validator(DownloadRequestValidator)
        });
        let source_control = FormInput::new(
            &form,
            DownloadRequest::SOURCE,
            move |window, cx| InputState::new(window, cx).placeholder(placeholder),
            window,
            cx,
        );
        let source_input = (*source_control).clone();
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Self {
            form,
            source_input,
            _source_control: source_control,
            runtime: DownloadRuntime::default(),
            backend,
            focus_handle: cx.focus_handle(),
            _form_observer: form_observer,
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.runtime.is_active() {
            event!(Level::DEBUG, "ignored download start while a run is active");
            return;
        }

        let Ok(prepared) = self.form.update(cx, |form, cx| form.prepare(cx)) else {
            return;
        };
        let request = match PreparedDownloadRequest::try_from(prepared) {
            Ok(request) => request,
            Err(error) => {
                event!(Level::ERROR, error = %error, "validated download source failed preflight");
                return;
            }
        };

        let (events_tx, events_rx) = unbounded();
        let staging = StagingTracker::default();
        let (abort, registration) = futures::future::AbortHandle::new_pair();
        let backend = self.backend.clone();
        let worker_request = request.clone();
        let worker_staging = staging.clone();
        let (start_tx, start_rx) = oneshot::channel();
        let worker = cx.background_spawn(async move {
            if start_rx.await.is_err() {
                return WorkerCompletion::Cancelled(None);
            }
            let download = backend.run(worker_request, events_tx, worker_staging);
            match Compat::new(Abortable::new(download, registration)).await {
                Ok(result) => WorkerCompletion::Complete(result),
                Err(Aborted) => WorkerCompletion::Cancelled(staging.take_cleanup_problem()),
            }
        });
        let task = cx.spawn(async move |owner, cx| {
            let mut events = events_rx;
            let mut worker = Box::pin(worker);
            loop {
                let message = next_driver_message(&mut events, &mut worker).await;
                let terminal = matches!(message, DriverMessage::Terminal(_));
                let update = owner.update(cx, |this, cx| match message {
                    DriverMessage::Progress(event) => {
                        this.route(DownloadMessage::Progress(event), cx);
                    }
                    DriverMessage::Terminal(WorkerCompletion::Complete(result)) => {
                        match &result {
                            Ok(receipt) => event!(
                                Level::INFO,
                                path = %receipt.final_path().display(),
                                items_written = receipt.items_written(),
                                "download committed"
                            ),
                            Err(failure) => event!(
                                Level::ERROR,
                                problem = ?failure.problem(),
                                cleanup_problem = ?failure.cleanup_problem(),
                                "download failed"
                            ),
                        }
                        this.route(DownloadMessage::Complete(result), cx);
                    }
                    DriverMessage::Terminal(WorkerCompletion::Cancelled(cleanup_problem)) => {
                        if let Some(cleanup_problem) = &cleanup_problem {
                            event!(
                                Level::ERROR,
                                path = %cleanup_problem.path().display(),
                                error = ?cleanup_problem,
                                "download cancelled but staging cleanup failed"
                            );
                        } else {
                            event!(Level::INFO, "download cancelled");
                        }
                        this.route(DownloadMessage::Cancelled(cleanup_problem), cx);
                    }
                });
                if update.is_err() || terminal {
                    break;
                }
            }
        });

        self.route(
            DownloadMessage::Start {
                snapshot: request,
                task,
                abort,
            },
            cx,
        );
        let _ = start_tx.send(());
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.route(DownloadMessage::Cancel, cx);
    }

    fn route(&mut self, message: DownloadMessage, cx: &mut Context<Self>) {
        let effect = (&mut self.runtime).transition(message);
        match effect {
            DownloadEffect::Ignored => return,
            DownloadEffect::None => {}
            DownloadEffect::Abort(abort) => abort.abort(),
        }
        cx.notify();
    }

    fn render_source_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let (label, help) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("download-field-source"),
                i18n.t("download-source-help"),
            )
        };
        let error = DownloadRequest::SOURCE
            .errors(&self.form, cx)
            .first()
            .map(|issue| validation_message(issue.message(), cx));

        component_form_field()
            .label(label)
            .description(help)
            .required(true)
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(Input::new(&self.source_input))
                    .when_some(error, |this, error| {
                        this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
                    }),
            )
            .into_any_element()
    }

    fn render_status(&self, cx: &mut Context<Self>) -> Div {
        match self.runtime.status() {
            DownloadStatus::Idle => {
                let i18n = cx.global::<I18n>();
                div().child(
                    Alert::info("download-idle", i18n.t("download-state-idle-description"))
                        .title(i18n.t("download-state-idle-title")),
                )
            }
            DownloadStatus::Running { snapshot, progress } => {
                self.render_running(snapshot, progress, false, cx)
            }
            DownloadStatus::Cancelling { snapshot, progress } => {
                self.render_running(snapshot, progress, true, cx)
            }
            DownloadStatus::Succeeded { snapshot, receipt } => {
                let i18n = cx.global::<I18n>();
                let mut path_args = FluentArgs::new();
                path_args.set("path", receipt.final_path().display().to_string());
                let mut novel_args = FluentArgs::new();
                novel_args.set("name", receipt.metadata().name().to_string());
                novel_args.set("author", receipt.metadata().author().to_string());
                let mut item_args = FluentArgs::new();
                item_args.set("count", receipt.items_written() as i64);
                v_flex()
                    .gap_3()
                    .child(snapshot_label(snapshot, i18n))
                    .child(Label::new(
                        i18n.t_with_args("download-progress-novel", &novel_args),
                    ))
                    .child(Label::new(
                        i18n.t_with_args("download-progress-items", &item_args),
                    ))
                    .child(
                        Alert::success(
                            "download-succeeded",
                            i18n.t_with_args("download-output-path", &path_args),
                        )
                        .title(i18n.t("download-state-succeeded")),
                    )
            }
            DownloadStatus::Failed {
                snapshot,
                progress,
                failure,
            } => {
                let i18n = cx.global::<I18n>();
                v_flex()
                    .gap_3()
                    .child(snapshot_label(snapshot, i18n))
                    .child(progress_summary(progress, i18n))
                    .child(
                        Alert::error("download-failed", problem_message(failure.problem(), i18n))
                            .title(i18n.t("download-state-failed")),
                    )
                    .when_some(failure.cleanup_problem(), |this, cleanup| {
                        this.child(cleanup_alert(cleanup, i18n))
                    })
            }
            DownloadStatus::Cancelled {
                snapshot,
                cleanup_problem,
            } => {
                let i18n = cx.global::<I18n>();
                v_flex()
                    .gap_3()
                    .child(snapshot_label(snapshot, i18n))
                    .child(Alert::info(
                        "download-cancelled",
                        i18n.t("download-state-cancelled"),
                    ))
                    .when_some(cleanup_problem, |this, cleanup| {
                        this.child(cleanup_alert(cleanup, i18n))
                    })
            }
        }
    }

    fn render_running(
        &self,
        snapshot: &PreparedDownloadRequest,
        progress: &crate::crawler::DownloadProgress,
        cancelling: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let i18n = cx.global::<I18n>();
        let state = if cancelling {
            i18n.t("download-state-cancelling")
        } else if progress.metadata().is_some() {
            i18n.t("download-state-downloading")
        } else {
            i18n.t("download-state-resolving")
        };
        let mut content = v_flex()
            .gap_3()
            .child(snapshot_label(snapshot, i18n))
            .child(Label::new(state).font_semibold())
            .child(Progress::new("download-progress").loading(true));

        if let Some(metadata) = progress.metadata() {
            let mut args = FluentArgs::new();
            args.set("name", metadata.name().to_string());
            args.set("author", metadata.author().to_string());
            content = content.child(Label::new(
                i18n.t_with_args("download-progress-novel", &args),
            ));
        }
        if progress.items_written() > 0 {
            let mut args = FluentArgs::new();
            args.set("count", progress.items_written() as i64);
            content = content.child(Label::new(
                i18n.t_with_args("download-progress-items", &args),
            ));
        }
        if let Some(url) = progress.current_url() {
            let mut args = FluentArgs::new();
            args.set("url", url.to_string());
            content = content.child(
                Link::new("download-current-source")
                    .child(i18n.t_with_args("download-progress-current", &args))
                    .href(url.to_string()),
            );
        }
        content
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (start_label, cancel_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("download-action-start"),
                i18n.t("download-action-cancel"),
            )
        };
        let running = matches!(self.runtime.status(), DownloadStatus::Running { .. });
        let cancelling = matches!(self.runtime.status(), DownloadStatus::Cancelling { .. });
        let active = running || cancelling;

        div()
            .track_focus(&self.focus_handle)
            .key_context("NovelDownload")
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .bg(cx.theme().tokens.background.background)
            .text_color(cx.theme().foreground)
            .child(self.render_source_field(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("download-start")
                            .primary()
                            .label(start_label)
                            .loading(running)
                            .disabled(active)
                            .on_click(cx.listener(|this, _, _, cx| this.start(cx))),
                    )
                    .child(
                        Button::new("download-cancel")
                            .danger()
                            .label(cancel_label)
                            .loading(cancelling)
                            .disabled(!running)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    ),
            )
            .child(self.render_status(cx))
    }
}

async fn next_driver_message(
    events: &mut UnboundedReceiver<DownloadEngineEvent>,
    worker: &mut Pin<Box<Task<WorkerCompletion>>>,
) -> DriverMessage {
    poll_fn(|cx| {
        if let Poll::Ready(Some(event)) = Pin::new(&mut *events).poll_next(cx) {
            return Poll::Ready(DriverMessage::Progress(event));
        }
        worker.as_mut().poll(cx).map(DriverMessage::Terminal)
    })
    .await
}

fn snapshot_label(snapshot: &PreparedDownloadRequest, i18n: &I18n) -> Label {
    let mut args = FluentArgs::new();
    args.set("source", snapshot.submitted_source().to_string());
    Label::new(i18n.t_with_args("download-snapshot-source", &args)).text_sm()
}

fn progress_summary(progress: &crate::crawler::DownloadProgress, i18n: &I18n) -> Div {
    let mut summary = v_flex().gap_2();
    if let Some(metadata) = progress.metadata() {
        let mut args = FluentArgs::new();
        args.set("name", metadata.name().to_string());
        args.set("author", metadata.author().to_string());
        summary = summary.child(Label::new(
            i18n.t_with_args("download-progress-novel", &args),
        ));
    }
    if progress.items_written() > 0 {
        let mut args = FluentArgs::new();
        args.set("count", progress.items_written() as i64);
        summary = summary.child(Label::new(
            i18n.t_with_args("download-progress-items", &args),
        ));
    }
    summary
}

fn cleanup_alert(cleanup: &CleanupProblem, i18n: &I18n) -> Alert {
    let mut args = FluentArgs::new();
    args.set("path", cleanup.path().display().to_string());
    Alert::warning(
        "download-cleanup-warning",
        i18n.t_with_args("download-error-cleanup", &args),
    )
}

fn problem_message(problem: &DownloadProblem, i18n: &I18n) -> String {
    match problem {
        DownloadProblem::Http(problem) => match problem.status() {
            Some(status) => {
                let mut args = FluentArgs::new();
                args.set("status", u64::from(status.as_u16()));
                i18n.t_with_args("download-error-http-status", &args)
            }
            None => i18n.t("download-error-network"),
        },
        DownloadProblem::Parse(_) => i18n.t("download-error-parse"),
        DownloadProblem::Range(problem) => match problem.kind() {
            RangeProblemKind::MissingChapter { .. } | RangeProblemKind::EmptyRange => {
                i18n.t("download-error-range-chapter")
            }
            RangeProblemKind::PageOutOfRange { page, .. } => {
                let mut args = FluentArgs::new();
                args.set("page", u64::from(*page));
                i18n.t_with_args("download-error-range-page", &args)
            }
        },
        DownloadProblem::Output(problem) => match problem {
            OutputProblem::DownloadDirectoryUnavailable => {
                i18n.t("download-error-download-directory")
            }
            OutputProblem::TargetExists { path } => {
                let mut args = FluentArgs::new();
                args.set("path", path.display().to_string());
                i18n.t_with_args("download-error-target-exists", &args)
            }
            OutputProblem::StagingExists { path } => {
                let mut args = FluentArgs::new();
                args.set("path", path.display().to_string());
                i18n.t_with_args("download-error-staging-exists", &args)
            }
            OutputProblem::InvalidFileName | OutputProblem::Io { .. } => {
                i18n.t("download-error-output")
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        path::PathBuf,
        pin::Pin,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context as TaskContext, Poll},
    };

    use gpui::{AppContext as _, TestAppContext, WindowHandle};
    use tempfile::tempdir;

    use super::{
        DownloadRequest, DownloadStatus, DriverMessage, WorkerCompletion, WorkspaceView,
        next_driver_message, problem_message,
    };
    use crate::{
        crawler::source::ZgzlRange,
        crawler::{
            DownloadBackend, DownloadEngineEvent, DownloadFuture, DownloadReceipt, NovelMetadata,
            PreparedDownloadRequest, StagedOutput, StagingTracker,
        },
        errors::{
            DownloadFailure, DownloadProblem, OutputProblem, ParseProblem, ParseStage,
            RangeProblem, RangeProblemKind,
        },
        foundation::{I18n, i18n::init_i18n},
    };

    struct PendingDownload {
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
    }

    impl Future for PendingDownload {
        type Output = Result<DownloadReceipt, DownloadFailure>;

        fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
            self.started.store(true, Ordering::SeqCst);
            Poll::Pending
        }
    }

    impl Drop for PendingDownload {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    struct PendingBackend {
        requests: Arc<Mutex<Vec<PreparedDownloadRequest>>>,
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
    }

    impl DownloadBackend for PendingBackend {
        fn run(
            &self,
            request: PreparedDownloadRequest,
            _events: futures::channel::mpsc::UnboundedSender<DownloadEngineEvent>,
            _staging: StagingTracker,
        ) -> DownloadFuture {
            self.requests.lock().unwrap().push(request);
            Box::pin(PendingDownload {
                started: self.started.clone(),
                dropped: self.dropped.clone(),
            })
        }
    }

    struct PendingOutputDownload {
        _output: StagedOutput,
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
    }

    impl Future for PendingOutputDownload {
        type Output = Result<DownloadReceipt, DownloadFailure>;

        fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
            self.started.store(true, Ordering::SeqCst);
            Poll::Pending
        }
    }

    impl Drop for PendingOutputDownload {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    struct PendingOutputBackend {
        root: PathBuf,
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
    }

    impl DownloadBackend for PendingOutputBackend {
        fn run(
            &self,
            _request: PreparedDownloadRequest,
            _events: futures::channel::mpsc::UnboundedSender<DownloadEngineEvent>,
            staging: StagingTracker,
        ) -> DownloadFuture {
            let metadata = NovelMetadata::new(
                "Novel".into(),
                "Author".into(),
                "novel".into(),
                vec!["chapter".into()],
            );
            let output = StagedOutput::create(&self.root, &metadata, staging).unwrap();
            Box::pin(PendingOutputDownload {
                _output: output,
                started: self.started.clone(),
                dropped: self.dropped.clone(),
            })
        }
    }

    struct PendingFixture {
        backend: Arc<PendingBackend>,
        requests: Arc<Mutex<Vec<PreparedDownloadRequest>>>,
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
    }

    fn pending_fixture() -> PendingFixture {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        PendingFixture {
            backend: Arc::new(PendingBackend {
                requests: requests.clone(),
                started: started.clone(),
                dropped: dropped.clone(),
            }),
            requests,
            started,
            dropped,
        }
    }

    fn open_workspace(
        backend: Arc<dyn DownloadBackend>,
        cx: &mut TestAppContext,
    ) -> WindowHandle<WorkspaceView> {
        cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| WorkspaceView::new_with_backend(backend, window, cx))
            })
            .expect("open workspace test window")
        })
    }

    #[test]
    fn foreground_driver_drains_queued_progress_before_the_terminal_result() {
        let (events_tx, mut events_rx) = futures::channel::mpsc::unbounded();
        events_tx
            .unbounded_send(DownloadEngineEvent::ContentWritten {
                url: "first".into(),
                items_written: 1,
            })
            .unwrap();
        events_tx
            .unbounded_send(DownloadEngineEvent::ContentWritten {
                url: "second".into(),
                items_written: 2,
            })
            .unwrap();
        let mut worker = Box::pin(gpui::Task::ready(WorkerCompletion::Complete(Ok(
            DownloadReceipt::fixture(2),
        ))));

        let first = futures::executor::block_on(next_driver_message(&mut events_rx, &mut worker));
        assert!(matches!(
            first,
            DriverMessage::Progress(DownloadEngineEvent::ContentWritten {
                ref url,
                items_written: 1,
            }) if url == "first"
        ));

        let second = futures::executor::block_on(next_driver_message(&mut events_rx, &mut worker));
        assert!(matches!(
            second,
            DriverMessage::Progress(DownloadEngineEvent::ContentWritten {
                ref url,
                items_written: 2,
            }) if url == "second"
        ));

        let terminal =
            futures::executor::block_on(next_driver_message(&mut events_rx, &mut worker));
        let DriverMessage::Terminal(WorkerCompletion::Complete(Ok(receipt))) = terminal else {
            panic!("the worker result must follow all queued progress");
        };
        assert_eq!(receipt.items_written(), 2);
    }

    #[gpui::test]
    fn running_snapshot_is_frozen_while_the_form_remains_editable(cx: &mut TestAppContext) {
        let fixture = pending_fixture();
        let window = open_workspace(fixture.backend, cx);
        cx.update(|cx| {
            window
                .update(cx, |view, _window, cx| {
                    DownloadRequest::SOURCE.set(&view.form, "first".into(), cx);
                    view.start(cx);
                    DownloadRequest::SOURCE.set(&view.form, "second".into(), cx);
                })
                .unwrap();
        });
        cx.run_until_parked();

        assert!(fixture.started.load(Ordering::SeqCst));
        assert_eq!(
            fixture.requests.lock().unwrap()[0].submitted_source(),
            "first"
        );
        cx.update(|cx| {
            window
                .update(cx, |view, _window, cx| {
                    let DownloadStatus::Running { snapshot, .. } = view.runtime.status() else {
                        panic!("download must still be running");
                    };
                    assert_eq!(snapshot.submitted_source(), "first");
                    assert_eq!(DownloadRequest::SOURCE.get(&view.form, cx), "second");
                })
                .unwrap();
        });
    }

    #[gpui::test]
    fn cancel_aborts_the_worker_before_exposing_cancelled(cx: &mut TestAppContext) {
        let fixture = pending_fixture();
        let window = open_workspace(fixture.backend, cx);
        cx.update(|cx| {
            window
                .update(cx, |view, _window, cx| {
                    DownloadRequest::SOURCE.set(&view.form, "novel".into(), cx);
                    view.start(cx);
                })
                .unwrap();
        });
        cx.run_until_parked();
        cx.update(|cx| {
            window
                .update(cx, |view, _window, cx| view.cancel(cx))
                .unwrap();
        });
        cx.run_until_parked();

        assert!(fixture.dropped.load(Ordering::SeqCst));
        cx.update(|cx| {
            window
                .update(cx, |view, _window, _cx| {
                    assert!(matches!(
                        view.runtime.status(),
                        DownloadStatus::Cancelled { .. }
                    ));
                })
                .unwrap();
        });
    }

    #[gpui::test]
    fn removing_the_window_cancels_the_owned_task_tree(cx: &mut TestAppContext) {
        let directory = tempdir().unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let backend = Arc::new(PendingOutputBackend {
            root: directory.path().to_path_buf(),
            started: started.clone(),
            dropped: dropped.clone(),
        });
        let part_path = directory.path().join("NovelbyAuthor.txt.part");
        let window = open_workspace(backend, cx);
        cx.update(|cx| {
            window
                .update(cx, |view, _window, cx| {
                    DownloadRequest::SOURCE.set(&view.form, "novel".into(), cx);
                    view.start(cx);
                })
                .unwrap();
        });
        cx.run_until_parked();
        assert!(started.load(Ordering::SeqCst));
        assert!(part_path.exists());

        cx.update(|cx| {
            window
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
        });
        cx.run_until_parked();
        assert!(dropped.load(Ordering::SeqCst));
        assert!(!part_path.exists());
    }

    #[gpui::test]
    fn terminal_problem_mapping_is_localized_without_response_content(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.set_global(I18n::for_locale_tag("en-US"));
            let i18n = cx.global::<I18n>();
            let range = DownloadProblem::Range(RangeProblem::new(
                ZgzlRange::Page {
                    novel_id: "novel".into(),
                    chapter_id: "chapter".into(),
                    page: std::num::NonZeroU32::new(9).unwrap(),
                },
                RangeProblemKind::PageOutOfRange {
                    page: 9,
                    page_count: 3,
                },
            ));
            assert_eq!(
                problem_message(&range, i18n),
                "The requested page 9 was not found."
            );

            let output = DownloadProblem::Output(OutputProblem::TargetExists {
                path: PathBuf::from("/tmp/existing.txt"),
            });
            assert!(problem_message(&output, i18n).contains("/tmp/existing.txt"));

            let parse = DownloadProblem::Parse(ParseProblem::new(
                reqwest::Url::parse("https://m.zgzl.net/info_novel/").unwrap(),
                ParseStage::NovelMetadata,
            ));
            let secret_body = "<html>secret novel body</html>";
            assert!(!format!("{parse:?}").contains(secret_body));
            assert_eq!(
                problem_message(&parse, i18n),
                "The downloaded novel data could not be parsed."
            );
        });
    }
}
