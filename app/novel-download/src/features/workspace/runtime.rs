use futures::future::AbortHandle;
use gpui::Task;
use gpui_operation::Transition;

use crate::{
    crawler::{DownloadEngineEvent, DownloadProgress, DownloadReceipt, PreparedDownloadRequest},
    errors::{CleanupProblem, DownloadFailure},
};

#[derive(Default)]
pub(super) enum DownloadRuntime {
    #[default]
    Idle,
    Running {
        snapshot: PreparedDownloadRequest,
        progress: DownloadProgress,
        task: Task<()>,
        abort: AbortHandle,
    },
    Cancelling {
        snapshot: PreparedDownloadRequest,
        progress: DownloadProgress,
        _task: Task<()>,
    },
    Succeeded {
        snapshot: PreparedDownloadRequest,
        receipt: DownloadReceipt,
    },
    Failed {
        snapshot: PreparedDownloadRequest,
        progress: DownloadProgress,
        failure: DownloadFailure,
    },
    Cancelled {
        snapshot: PreparedDownloadRequest,
        cleanup_problem: Option<CleanupProblem>,
    },
}

pub(super) enum DownloadMessage {
    Start {
        snapshot: PreparedDownloadRequest,
        task: Task<()>,
        abort: AbortHandle,
    },
    Progress(DownloadEngineEvent),
    Cancel,
    Complete(Result<DownloadReceipt, DownloadFailure>),
    Cancelled(Option<CleanupProblem>),
}

pub(super) enum DownloadEffect {
    Ignored,
    None,
    Abort(AbortHandle),
}

pub(super) enum DownloadStatus<'a> {
    Idle,
    Running {
        snapshot: &'a PreparedDownloadRequest,
        progress: &'a DownloadProgress,
    },
    Cancelling {
        snapshot: &'a PreparedDownloadRequest,
        progress: &'a DownloadProgress,
    },
    Succeeded {
        snapshot: &'a PreparedDownloadRequest,
        receipt: &'a DownloadReceipt,
    },
    Failed {
        snapshot: &'a PreparedDownloadRequest,
        progress: &'a DownloadProgress,
        failure: &'a DownloadFailure,
    },
    Cancelled {
        snapshot: &'a PreparedDownloadRequest,
        cleanup_problem: Option<&'a CleanupProblem>,
    },
}

impl DownloadRuntime {
    pub(super) fn is_active(&self) -> bool {
        matches!(
            Self::status(self),
            DownloadStatus::Running { .. } | DownloadStatus::Cancelling { .. }
        )
    }

    pub(super) fn status(&self) -> DownloadStatus<'_> {
        match self {
            Self::Idle => DownloadStatus::Idle,
            Self::Running {
                snapshot, progress, ..
            } => DownloadStatus::Running { snapshot, progress },
            Self::Cancelling {
                snapshot, progress, ..
            } => DownloadStatus::Cancelling { snapshot, progress },
            Self::Succeeded { snapshot, receipt } => {
                DownloadStatus::Succeeded { snapshot, receipt }
            }
            Self::Failed {
                snapshot,
                progress,
                failure,
            } => DownloadStatus::Failed {
                snapshot,
                progress,
                failure,
            },
            Self::Cancelled {
                snapshot,
                cleanup_problem,
            } => DownloadStatus::Cancelled {
                snapshot,
                cleanup_problem: cleanup_problem.as_ref(),
            },
        }
    }
}

impl Transition<DownloadMessage> for &mut DownloadRuntime {
    type Output = DownloadEffect;

    fn transition(self, message: DownloadMessage) -> Self::Output {
        match message {
            DownloadMessage::Start {
                snapshot,
                task,
                abort,
            } if !self.is_active() => {
                *self = DownloadRuntime::Running {
                    snapshot,
                    progress: DownloadProgress::default(),
                    task,
                    abort,
                };
                DownloadEffect::None
            }
            DownloadMessage::Progress(event) => {
                if let DownloadRuntime::Running { progress, .. } = self {
                    progress.apply(event);
                    DownloadEffect::None
                } else {
                    tracing::debug!("ignored download progress outside Running");
                    DownloadEffect::Ignored
                }
            }
            DownloadMessage::Cancel => {
                let current = std::mem::take(self);
                match current {
                    DownloadRuntime::Running {
                        snapshot,
                        progress,
                        task,
                        abort,
                    } => {
                        *self = DownloadRuntime::Cancelling {
                            snapshot,
                            progress,
                            _task: task,
                        };
                        DownloadEffect::Abort(abort)
                    }
                    current => {
                        *self = current;
                        tracing::debug!("ignored download cancel outside Running");
                        DownloadEffect::Ignored
                    }
                }
            }
            DownloadMessage::Complete(result) if self.is_active() => {
                let current = std::mem::take(self);
                let (snapshot, progress) = match current {
                    DownloadRuntime::Running {
                        snapshot, progress, ..
                    }
                    | DownloadRuntime::Cancelling {
                        snapshot, progress, ..
                    } => (snapshot, progress),
                    _ => unreachable!("active runtime must own a run snapshot"),
                };
                *self = match result {
                    Ok(receipt) => DownloadRuntime::Succeeded { snapshot, receipt },
                    Err(failure) => DownloadRuntime::Failed {
                        snapshot,
                        progress,
                        failure,
                    },
                };
                DownloadEffect::None
            }
            DownloadMessage::Cancelled(cleanup_problem)
                if matches!(self, DownloadRuntime::Cancelling { .. }) =>
            {
                let DownloadRuntime::Cancelling { snapshot, .. } = std::mem::take(self) else {
                    unreachable!();
                };
                *self = DownloadRuntime::Cancelled {
                    snapshot,
                    cleanup_problem,
                };
                DownloadEffect::None
            }
            DownloadMessage::Start { .. }
            | DownloadMessage::Complete(_)
            | DownloadMessage::Cancelled(_) => {
                tracing::debug!("ignored illegal download transition");
                DownloadEffect::Ignored
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::*;
    use crate::{
        crawler::{DownloadReceipt, DownloadSource, source::ZgzlRange},
        errors::OutputProblem,
    };

    fn snapshot(source: &str) -> PreparedDownloadRequest {
        PreparedDownloadRequest::new(
            source.into(),
            DownloadSource::Zgzl(ZgzlRange::Page {
                novel_id: "novel".into(),
                chapter_id: "chapter".into(),
                page: NonZeroU32::new(2).unwrap(),
            }),
        )
    }

    fn start(runtime: &mut DownloadRuntime, source: &str) {
        let (abort, _) = AbortHandle::new_pair();
        runtime.transition(DownloadMessage::Start {
            snapshot: snapshot(source),
            task: Task::ready(()),
            abort,
        });
    }

    fn assert_ignored(runtime: &mut DownloadRuntime, message: DownloadMessage) {
        assert!(matches!(
            runtime.transition(message),
            DownloadEffect::Ignored
        ));
    }

    #[test]
    fn duplicate_start_and_illegal_messages_are_discarded() {
        let mut runtime = DownloadRuntime::default();
        start(&mut runtime, "first");
        start(&mut runtime, "second");
        runtime.transition(DownloadMessage::Cancelled(None));

        let DownloadStatus::Running { snapshot, progress } = runtime.status() else {
            panic!("the first run must remain authoritative");
        };
        assert_eq!(snapshot.submitted_source(), "first");
        assert_eq!(progress.items_written(), 0);
    }

    #[test]
    fn illegal_messages_are_ignored_in_idle_and_cancelling() {
        let mut runtime = DownloadRuntime::default();
        assert_ignored(
            &mut runtime,
            DownloadMessage::Progress(DownloadEngineEvent::ContentWritten {
                url: "ignored".into(),
                items_written: 1,
            }),
        );
        assert_ignored(&mut runtime, DownloadMessage::Cancel);
        assert_ignored(
            &mut runtime,
            DownloadMessage::Complete(Ok(DownloadReceipt::fixture(1))),
        );
        assert_ignored(&mut runtime, DownloadMessage::Cancelled(None));
        assert!(matches!(runtime.status(), DownloadStatus::Idle));

        start(&mut runtime, "first");
        runtime.transition(DownloadMessage::Cancel);
        start(&mut runtime, "second");
        assert_ignored(
            &mut runtime,
            DownloadMessage::Progress(DownloadEngineEvent::ContentWritten {
                url: "ignored".into(),
                items_written: 1,
            }),
        );
        assert_ignored(&mut runtime, DownloadMessage::Cancel);

        let DownloadStatus::Cancelling { snapshot, progress } = runtime.status() else {
            panic!("illegal messages must not replace a cancelling download");
        };
        assert_eq!(snapshot.submitted_source(), "first");
        assert_eq!(progress.items_written(), 0);
    }

    #[test]
    fn cancel_keeps_the_task_until_cancelled_completion() {
        let mut runtime = DownloadRuntime::default();
        start(&mut runtime, "first");

        assert!(matches!(
            runtime.transition(DownloadMessage::Cancel),
            DownloadEffect::Abort(_)
        ));
        assert!(runtime.is_active());
        assert!(matches!(
            runtime.status(),
            DownloadStatus::Cancelling { .. }
        ));
        assert!(matches!(
            runtime.transition(DownloadMessage::Cancel),
            DownloadEffect::Ignored
        ));

        runtime.transition(DownloadMessage::Cancelled(None));
        assert!(!runtime.is_active());
        assert!(matches!(runtime.status(), DownloadStatus::Cancelled { .. }));
    }

    #[test]
    fn progress_only_changes_a_running_download() {
        let mut runtime = DownloadRuntime::default();
        runtime.transition(DownloadMessage::Progress(
            DownloadEngineEvent::ContentWritten {
                url: "ignored".into(),
                items_written: 1,
            },
        ));
        assert!(matches!(runtime.status(), DownloadStatus::Idle));

        start(&mut runtime, "first");
        runtime.transition(DownloadMessage::Progress(
            DownloadEngineEvent::ContentWritten {
                url: "current".into(),
                items_written: 1,
            },
        ));
        let DownloadStatus::Running { progress, .. } = runtime.status() else {
            panic!("run must remain active");
        };
        assert_eq!(progress.current_url(), Some("current"));
    }

    #[test]
    fn a_committed_completion_wins_the_cancel_race() {
        let mut runtime = DownloadRuntime::default();
        start(&mut runtime, "first");
        runtime.transition(DownloadMessage::Cancel);
        runtime.transition(DownloadMessage::Complete(Ok(DownloadReceipt::fixture(2))));

        let DownloadStatus::Succeeded { snapshot, receipt } = runtime.status() else {
            panic!("a committed result must win after cancellation was requested");
        };
        assert_eq!(snapshot.submitted_source(), "first");
        assert_eq!(receipt.items_written(), 2);
    }

    #[test]
    fn a_late_completion_cannot_replace_cancelled() {
        let mut runtime = DownloadRuntime::default();
        start(&mut runtime, "first");
        runtime.transition(DownloadMessage::Cancel);
        runtime.transition(DownloadMessage::Cancelled(None));
        assert!(matches!(
            runtime.transition(DownloadMessage::Complete(Ok(DownloadReceipt::fixture(1)))),
            DownloadEffect::Ignored
        ));
        assert!(matches!(runtime.status(), DownloadStatus::Cancelled { .. }));
    }

    #[test]
    fn terminal_downloads_discard_non_start_messages() {
        let mut succeeded = DownloadRuntime::default();
        start(&mut succeeded, "succeeded");
        succeeded.transition(DownloadMessage::Complete(Ok(DownloadReceipt::fixture(2))));
        assert_ignored(&mut succeeded, DownloadMessage::Cancel);
        assert_ignored(
            &mut succeeded,
            DownloadMessage::Progress(DownloadEngineEvent::ContentWritten {
                url: "ignored".into(),
                items_written: 3,
            }),
        );
        assert_ignored(
            &mut succeeded,
            DownloadMessage::Complete(Ok(DownloadReceipt::fixture(3))),
        );
        assert_ignored(&mut succeeded, DownloadMessage::Cancelled(None));
        let DownloadStatus::Succeeded { snapshot, receipt } = succeeded.status() else {
            panic!("succeeded download must remain terminal");
        };
        assert_eq!(snapshot.submitted_source(), "succeeded");
        assert_eq!(receipt.items_written(), 2);

        let mut failed = DownloadRuntime::default();
        start(&mut failed, "failed");
        failed.transition(DownloadMessage::Complete(Err(DownloadFailure::new(
            OutputProblem::InvalidFileName,
        ))));
        assert_ignored(&mut failed, DownloadMessage::Cancel);
        assert_ignored(
            &mut failed,
            DownloadMessage::Progress(DownloadEngineEvent::ContentWritten {
                url: "ignored".into(),
                items_written: 1,
            }),
        );
        assert_ignored(
            &mut failed,
            DownloadMessage::Complete(Ok(DownloadReceipt::fixture(1))),
        );
        assert_ignored(&mut failed, DownloadMessage::Cancelled(None));
        let DownloadStatus::Failed {
            snapshot, progress, ..
        } = failed.status()
        else {
            panic!("failed download must remain terminal");
        };
        assert_eq!(snapshot.submitted_source(), "failed");
        assert_eq!(progress.items_written(), 0);

        let mut cancelled = DownloadRuntime::default();
        start(&mut cancelled, "cancelled");
        cancelled.transition(DownloadMessage::Cancel);
        cancelled.transition(DownloadMessage::Cancelled(None));
        assert_ignored(&mut cancelled, DownloadMessage::Cancel);
        assert_ignored(
            &mut cancelled,
            DownloadMessage::Progress(DownloadEngineEvent::ContentWritten {
                url: "ignored".into(),
                items_written: 1,
            }),
        );
        assert_ignored(
            &mut cancelled,
            DownloadMessage::Complete(Ok(DownloadReceipt::fixture(1))),
        );
        assert_ignored(&mut cancelled, DownloadMessage::Cancelled(None));
        let DownloadStatus::Cancelled { snapshot, .. } = cancelled.status() else {
            panic!("cancelled download must remain terminal");
        };
        assert_eq!(snapshot.submitted_source(), "cancelled");
    }

    #[test]
    fn failure_preserves_progress_from_the_frozen_run() {
        let mut runtime = DownloadRuntime::default();
        start(&mut runtime, "first");
        runtime.transition(DownloadMessage::Progress(
            DownloadEngineEvent::ContentWritten {
                url: "current".into(),
                items_written: 3,
            },
        ));
        runtime.transition(DownloadMessage::Complete(Err(DownloadFailure::new(
            OutputProblem::InvalidFileName,
        ))));

        let DownloadStatus::Failed { progress, .. } = runtime.status() else {
            panic!("failure must become terminal");
        };
        assert_eq!(progress.items_written(), 3);
        assert_eq!(progress.current_url(), Some("current"));
    }
}
