pub(crate) use http_client_core::{FailedAttempt, RequestProblem, ResponseReceipt};
use std::fmt;
use std::{
    mem,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::Task;
use gpui_operation::Transition;

use http_client_core::{
    CompletedBody, ResponseData, ResponseHead, ResponseProgress, ResponseTiming,
};

pub(crate) type RequestTask = Task<()>;

pub(crate) enum RequestRuntime {
    Idle,
    Sending {
        task: RequestTask,
        started_at: Instant,
    },
    Receiving {
        task: RequestTask,
        receipt: ResponseReceipt,
    },
    Ready {
        response: Arc<ResponseData>,
    },
    Failed {
        attempt: FailedAttempt,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestPhase {
    Idle,
    Sending,
    Receiving,
    Ready,
    Failed,
}

pub(crate) enum HttpRunMessage {
    Start {
        task: RequestTask,
        started_at: Instant,
    },
    HeadReceived {
        head: ResponseHead,
        head_after: Duration,
        progress: ResponseProgress,
    },
    BodyProgress(ResponseProgress),
    Finished {
        result: Result<CompletedBody, RequestProblem>,
        finished_after: Duration,
    },
    Cancel,
    Clear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HttpRunEffect {
    Ignored,
    Started,
    HeadAccepted,
    Progressed,
    Ready,
    Failed,
    Cancelled,
    Cleared,
}

impl RequestRuntime {
    pub(crate) fn presentation(&self) -> http_client_core::ResponseState {
        use http_client_core::ResponseState;
        match self {
            Self::Idle => ResponseState::Idle,
            Self::Sending { .. } => ResponseState::Sending,
            Self::Receiving { receipt, .. } => ResponseState::Receiving {
                receipt: receipt.clone(),
            },
            Self::Ready { response } => ResponseState::Ready {
                response: response.clone(),
            },
            Self::Failed { attempt } => ResponseState::Failed {
                attempt: attempt.clone(),
            },
        }
    }

    pub(crate) const fn new() -> Self {
        Self::Idle
    }

    pub(crate) const fn phase(&self) -> RequestPhase {
        match self {
            Self::Idle => RequestPhase::Idle,
            Self::Sending { .. } => RequestPhase::Sending,
            Self::Receiving { .. } => RequestPhase::Receiving,
            Self::Ready { .. } => RequestPhase::Ready,
            Self::Failed { .. } => RequestPhase::Failed,
        }
    }

    pub(crate) const fn is_running(&self) -> bool {
        matches!(self, Self::Sending { .. } | Self::Receiving { .. })
    }

    #[cfg(test)]
    pub(crate) fn response(&self) -> Option<&Arc<ResponseData>> {
        match self {
            Self::Ready { response } => Some(response),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn receipt(&self) -> Option<&ResponseReceipt> {
        match self {
            Self::Receiving { receipt, .. } => Some(receipt),
            Self::Failed {
                attempt:
                    FailedAttempt {
                        receipt: Some(receipt),
                        ..
                    },
            } => Some(receipt),
            _ => None,
        }
    }
}

impl Default for RequestRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RequestRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("RequestRuntime");
        debug.field("phase", &self.phase());
        if let Self::Sending { started_at, .. } = self {
            debug.field("elapsed", &started_at.elapsed());
        }
        debug.finish_non_exhaustive()
    }
}

impl Transition<HttpRunMessage> for &mut RequestRuntime {
    type Output = HttpRunEffect;

    fn transition(self, message: HttpRunMessage) -> Self::Output {
        let current = mem::replace(self, RequestRuntime::Idle);

        match (current, message) {
            (
                RequestRuntime::Idle | RequestRuntime::Ready { .. } | RequestRuntime::Failed { .. },
                HttpRunMessage::Start { task, started_at },
            ) => {
                *self = RequestRuntime::Sending { task, started_at };
                HttpRunEffect::Started
            }
            (
                RequestRuntime::Sending {
                    task,
                    started_at: _,
                },
                HttpRunMessage::HeadReceived {
                    head,
                    head_after,
                    progress,
                },
            ) => {
                *self = RequestRuntime::Receiving {
                    task,
                    receipt: ResponseReceipt::new(head, progress, head_after),
                };
                HttpRunEffect::HeadAccepted
            }
            (
                RequestRuntime::Sending { task, .. },
                HttpRunMessage::Finished {
                    result: Err(problem),
                    finished_after,
                },
            ) => {
                *self = RequestRuntime::Failed {
                    attempt: FailedAttempt::new(problem, None, finished_after),
                };
                drop(task);
                HttpRunEffect::Failed
            }
            (
                RequestRuntime::Receiving { task, mut receipt },
                HttpRunMessage::BodyProgress(progress),
            ) if progress.is_monotonic_from(&receipt.progress) => {
                receipt.progress = progress;
                *self = RequestRuntime::Receiving { task, receipt };
                HttpRunEffect::Progressed
            }
            (
                RequestRuntime::Receiving { task, receipt },
                HttpRunMessage::Finished {
                    result: Ok(body),
                    finished_after,
                },
            ) => {
                let response = Arc::new(ResponseData::new(
                    receipt.head,
                    ResponseTiming::new(receipt.head_after, finished_after),
                    body,
                ));
                *self = RequestRuntime::Ready { response };
                drop(task);
                HttpRunEffect::Ready
            }
            (
                RequestRuntime::Receiving { task, receipt },
                HttpRunMessage::Finished {
                    result: Err(problem),
                    finished_after,
                },
            ) => {
                *self = RequestRuntime::Failed {
                    attempt: FailedAttempt::new(problem, Some(receipt), finished_after),
                };
                drop(task);
                HttpRunEffect::Failed
            }
            (
                RequestRuntime::Sending { task, .. } | RequestRuntime::Receiving { task, .. },
                HttpRunMessage::Cancel,
            ) => {
                *self = RequestRuntime::Idle;
                drop(task);
                HttpRunEffect::Cancelled
            }
            (
                RequestRuntime::Ready { .. } | RequestRuntime::Failed { .. },
                HttpRunMessage::Clear,
            ) => {
                *self = RequestRuntime::Idle;
                HttpRunEffect::Cleared
            }
            (current, message) => {
                *self = current;
                trace_ignored(self.phase(), message_kind(&message));
                drop(message);
                HttpRunEffect::Ignored
            }
        }
    }
}

fn message_kind(message: &HttpRunMessage) -> &'static str {
    match message {
        HttpRunMessage::Start { .. } => "Start",
        HttpRunMessage::HeadReceived { .. } => "HeadReceived",
        HttpRunMessage::BodyProgress(_) => "BodyProgress",
        HttpRunMessage::Finished { .. } => "Finished",
        HttpRunMessage::Cancel => "Cancel",
        HttpRunMessage::Clear => "Clear",
    }
}

fn trace_ignored(phase: RequestPhase, message: &'static str) {
    tracing::debug!(
        operation = "http-request",
        ?phase,
        message,
        "ignored request runtime message"
    );
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui_kit::{App, TestAppContext};

    use super::*;
    use http_client_core::{ActiveBodyStorage, BodyDecoding, ResponseSizes, StoredBody};

    fn task(cx: &mut App, dropped: Rc<Cell<bool>>) -> RequestTask {
        struct DropProbe(Rc<Cell<bool>>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let probe = DropProbe(dropped);
        cx.spawn(async move |_| {
            let _probe = probe;
            std::future::pending::<()>().await;
        })
    }

    fn progress(encoded: u64, stored: u64) -> ResponseProgress {
        ResponseProgress::new(None, encoded, stored, ActiveBodyStorage::Memory)
    }

    fn head() -> ResponseHead {
        ResponseHead::new(
            http::StatusCode::OK,
            http::Version::HTTP_11,
            url::Url::parse("https://example.test/secret?token=value").unwrap(),
            http::HeaderMap::new(),
        )
    }

    #[gpui_kit::test]
    fn cancel_installs_idle_before_dropping_the_owned_task(cx: &mut TestAppContext) {
        let dropped = Rc::new(Cell::new(false));
        let owned = cx.update(|cx| task(cx, dropped.clone()));
        let mut runtime = RequestRuntime::new();
        runtime.transition(HttpRunMessage::Start {
            task: owned,
            started_at: Instant::now(),
        });
        assert_eq!(runtime.phase(), RequestPhase::Sending);

        runtime.transition(HttpRunMessage::Cancel);
        assert_eq!(runtime.phase(), RequestPhase::Idle);
        cx.run_until_parked();
        assert!(dropped.get());
    }

    #[gpui_kit::test]
    fn head_progress_and_completion_form_one_ready_response(cx: &mut TestAppContext) {
        let dropped = Rc::new(Cell::new(false));
        let owned = cx.update(|cx| task(cx, dropped));
        let mut runtime = RequestRuntime::new();
        runtime.transition(HttpRunMessage::Start {
            task: owned,
            started_at: Instant::now(),
        });
        runtime.transition(HttpRunMessage::HeadReceived {
            head: head(),
            head_after: Duration::from_millis(2),
            progress: progress(0, 0),
        });
        runtime.transition(HttpRunMessage::BodyProgress(progress(4, 4)));
        runtime.transition(HttpRunMessage::Finished {
            result: Ok(CompletedBody::new(
                StoredBody::Memory(bytes::Bytes::from_static(b"body")),
                BodyDecoding::Identity,
                ResponseSizes::new(Some(4), 4, 4),
            )),
            finished_after: Duration::from_millis(4),
        });

        assert_eq!(runtime.phase(), RequestPhase::Ready);
        let response = runtime.response().unwrap();
        assert_eq!(response.sizes().stored_body_bytes, 4);
        assert_eq!(response.timing().head_after, Duration::from_millis(2));
    }

    #[gpui_kit::test]
    fn illegal_and_regressing_messages_preserve_the_exact_running_state(cx: &mut TestAppContext) {
        let dropped = Rc::new(Cell::new(false));
        let owned = cx.update(|cx| task(cx, dropped.clone()));
        let mut runtime = RequestRuntime::new();
        runtime.transition(HttpRunMessage::Start {
            task: owned,
            started_at: Instant::now(),
        });
        runtime.transition(HttpRunMessage::HeadReceived {
            head: head(),
            head_after: Duration::from_millis(2),
            progress: progress(4, 4),
        });
        runtime.transition(HttpRunMessage::BodyProgress(progress(3, 3)));

        assert_eq!(runtime.phase(), RequestPhase::Receiving);
        assert_eq!(
            runtime.receipt().unwrap().progress.received_encoded_bytes,
            4
        );
        assert!(!dropped.get());
    }

    #[gpui_kit::test]
    fn illegal_clear_success_without_head_and_duplicate_head_are_ignored(cx: &mut TestAppContext) {
        let dropped = Rc::new(Cell::new(false));
        let owned = cx.update(|cx| task(cx, dropped.clone()));
        let mut runtime = RequestRuntime::new();
        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::Start {
                task: owned,
                started_at: Instant::now(),
            }),
            HttpRunEffect::Started
        );
        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::Clear),
            HttpRunEffect::Ignored
        );
        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::Finished {
                result: Ok(CompletedBody::new(
                    StoredBody::Empty,
                    BodyDecoding::Identity,
                    ResponseSizes::new(Some(0), 0, 0)
                )),
                finished_after: Duration::from_millis(1),
            }),
            HttpRunEffect::Ignored
        );
        assert_eq!(runtime.phase(), RequestPhase::Sending);
        assert!(!dropped.get());

        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::HeadReceived {
                head: head(),
                head_after: Duration::from_millis(2),
                progress: progress(0, 0),
            }),
            HttpRunEffect::HeadAccepted
        );
        let mut duplicate = head();
        duplicate.status = http::StatusCode::CREATED;
        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::HeadReceived {
                head: duplicate,
                head_after: Duration::from_millis(3),
                progress: progress(0, 0),
            }),
            HttpRunEffect::Ignored
        );
        assert_eq!(runtime.receipt().unwrap().head.status, http::StatusCode::OK);
        assert!(!dropped.get());
    }

    #[test]
    fn stable_state_cancel_and_idle_clear_are_noops() {
        let mut runtime = RequestRuntime::new();
        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::Cancel),
            HttpRunEffect::Ignored
        );
        assert_eq!(
            (&mut runtime).transition(HttpRunMessage::Clear),
            HttpRunEffect::Ignored
        );
        assert_eq!(runtime.phase(), RequestPhase::Idle);
    }

    #[test]
    fn problem_diagnostics_redact_sources_and_sensitive_values() {
        let problem = RequestProblem::transport(std::io::Error::other(
            "https://secret.test/?token=value header-secret",
        ));
        let diagnostic = format!("{problem:?} {problem}");
        assert!(!diagnostic.contains("secret.test"));
        assert!(!diagnostic.contains("header-secret"));
        assert_eq!(
            problem.kind(),
            http_client_core::RequestProblemKind::Transport
        );
    }
}
