use gpui::Task;
use gpui_operation::Transition;
use tracing::{Level, event};

use crate::{
    fetch::{FetchErrorKind, FetchPageError},
    foundation::I18n,
};

use super::form::FetchRequest;

const MAX_PAGE_LOGS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FetchPageLogStatus {
    Running,
    Success,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FetchPageLog {
    pub(super) page: u32,
    pub(super) status: FetchPageLogStatus,
    pub(super) inserted: Option<usize>,
    pub(super) elapsed_ms: Option<u128>,
    pub(super) message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FetchProgress {
    pub(super) start_page: u32,
    pub(super) end_page: u32,
    pub(super) current_page: u32,
    pub(super) last_success_page: Option<u32>,
    pub(super) total: i64,
}

impl FetchProgress {
    pub(super) fn completed_pages(&self) -> u32 {
        self.last_success_page
            .filter(|page| *page >= self.start_page)
            .map(|page| page - self.start_page + 1)
            .unwrap_or(0)
    }

    pub(super) fn page_count(&self) -> u32 {
        page_count(self.start_page, self.end_page)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FetchFailure {
    pub(super) progress: FetchProgress,
    pub(super) page: u32,
    pub(super) kind: FetchErrorKind,
    pub(super) message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum FetchStatus {
    #[default]
    Idle,
    Running(FetchProgress),
    Interrupted(FetchProgress),
    Failed(FetchFailure),
    Success(FetchProgress),
}

#[derive(Default)]
pub(crate) enum FetchRun {
    #[default]
    Idle,
    Running {
        snapshot: FetchRequest,
        progress: FetchProgress,
        logs: Vec<FetchPageLog>,
        task: Task<()>,
    },
    Interrupted {
        snapshot: FetchRequest,
        progress: FetchProgress,
        logs: Vec<FetchPageLog>,
    },
    Failed {
        snapshot: FetchRequest,
        failure: FetchFailure,
        logs: Vec<FetchPageLog>,
    },
    Succeeded {
        snapshot: FetchRequest,
        progress: FetchProgress,
        logs: Vec<FetchPageLog>,
    },
}

impl FetchRun {
    pub(crate) fn is_running(&self) -> bool {
        if let Self::Running { task, .. } = self {
            let _ = task.is_ready();
            true
        } else {
            false
        }
    }

    pub(crate) fn has_visible_summary(&self) -> bool {
        !matches!(self, Self::Idle | Self::Succeeded { .. })
    }

    pub(crate) fn summary_text(&self, i18n: &I18n) -> Option<String> {
        match self {
            Self::Idle => None,
            Self::Running { progress, .. } => Some(format!(
                "{} {} / {} · {} {} · {} {}",
                i18n.t("fetch-state-running-title"),
                progress.current_page,
                progress.end_page,
                i18n.t("fetch-stat-completed-pages"),
                progress.completed_pages(),
                i18n.t("fetch-stat-total"),
                progress.total
            )),
            Self::Interrupted { progress, .. } => Some(format!(
                "{} · {} {} · {} {}",
                i18n.t("fetch-state-interrupted-title"),
                i18n.t("fetch-stat-next-page"),
                resume_page_after_interrupt(
                    progress.last_success_page,
                    progress.start_page,
                    progress.end_page
                )
                .unwrap_or(progress.end_page),
                i18n.t("fetch-stat-completed-pages"),
                progress.completed_pages()
            )),
            Self::Failed { failure, .. } => Some(format!(
                "{} · {} {} · {}",
                i18n.t("fetch-state-failed-title"),
                i18n.t("fetch-stat-failed-page"),
                failure.page,
                failure.message
            )),
            Self::Succeeded { progress, .. } => Some(format!(
                "{} · {} {} · {} {}",
                i18n.t("fetch-state-success"),
                i18n.t("fetch-stat-completed-pages"),
                progress.completed_pages(),
                i18n.t("fetch-stat-total"),
                progress.total
            )),
        }
    }

    pub(crate) fn titlebar_summary(&self, i18n: &I18n) -> String {
        self.summary_text(i18n)
            .unwrap_or_else(|| i18n.t("fetch-state-idle-title"))
    }

    fn begin_run(&mut self, request: FetchRequest, total: i64, clear_logs: bool, task: Task<()>) {
        let run_start_page = request.start_page;
        let previous = std::mem::take(self);
        let (snapshot, logs, last_success_page) = if clear_logs {
            (request.clone(), Vec::new(), None)
        } else {
            match previous {
                Self::Interrupted {
                    snapshot,
                    progress,
                    logs,
                }
                | Self::Succeeded {
                    snapshot,
                    progress,
                    logs,
                } => (snapshot, logs, progress.last_success_page),
                Self::Failed {
                    snapshot,
                    failure,
                    logs,
                } => (snapshot, logs, failure.progress.last_success_page),
                Self::Idle => (request.clone(), Vec::new(), None),
                Self::Running { .. } => unreachable!("running reentry is rejected by Transition"),
            }
        };
        event!(
            Level::INFO,
            start_page = snapshot.start_page,
            end_page = snapshot.end_page,
            run_start_page,
            ?last_success_page,
            total,
            clear_logs,
            "fetch run state started"
        );
        *self = Self::Running {
            progress: FetchProgress {
                start_page: snapshot.start_page,
                end_page: snapshot.end_page,
                current_page: run_start_page,
                last_success_page,
                total,
            },
            snapshot,
            logs,
            task,
        };
    }

    fn interrupt(&mut self) {
        event!(Level::INFO, "interrupting fetch run");
        if let Self::Running {
            snapshot,
            progress,
            logs,
            ..
        } = std::mem::take(self)
        {
            event!(
                Level::INFO,
                current_page = progress.current_page,
                last_success_page = ?progress.last_success_page,
                total = progress.total,
                "fetch run interrupted"
            );
            *self = Self::Interrupted {
                snapshot,
                progress,
                logs,
            };
        }
    }

    fn mark_page_started(&mut self, page: u32) {
        event!(Level::INFO, page, "fetch page started");
        if let Self::Running { progress, .. } = self {
            progress.current_page = page;
        }
        self.upsert_log(FetchPageLog {
            page,
            status: FetchPageLogStatus::Running,
            inserted: None,
            elapsed_ms: None,
            message: "fetching".to_string(),
        });
    }

    fn mark_page_succeeded(&mut self, page: u32, inserted: usize, total: i64, elapsed_ms: u128) {
        event!(
            Level::INFO,
            page,
            inserted,
            total,
            elapsed_ms,
            "fetch page succeeded"
        );
        if let Self::Running { progress, .. } = self {
            progress.current_page = page;
            progress.last_success_page = Some(page);
            progress.total = total;
        }
        self.upsert_log(FetchPageLog {
            page,
            status: FetchPageLogStatus::Success,
            inserted: Some(inserted),
            elapsed_ms: Some(elapsed_ms),
            message: "success".to_string(),
        });
    }

    fn mark_failed(&mut self, error: FetchPageError, elapsed_ms: Option<u128>) {
        event!(
            Level::ERROR,
            page = error.page,
            kind = %error.kind,
            message = %error.message,
            ?elapsed_ms,
            "fetch run failed"
        );
        let previous = std::mem::take(self);
        let (snapshot, progress, mut logs) = match previous {
            Self::Running {
                snapshot,
                progress,
                logs,
                ..
            }
            | Self::Interrupted {
                snapshot,
                progress,
                logs,
            }
            | Self::Succeeded {
                snapshot,
                progress,
                logs,
            } => (snapshot, progress, logs),
            Self::Failed {
                snapshot,
                failure,
                logs,
            } => (snapshot, failure.progress, logs),
            Self::Idle => return,
        };
        upsert_page_log(
            &mut logs,
            FetchPageLog {
                page: error.page,
                status: FetchPageLogStatus::Failed,
                inserted: None,
                elapsed_ms,
                message: error.message.clone(),
            },
        );
        *self = Self::Failed {
            snapshot,
            failure: FetchFailure {
                progress,
                page: error.page,
                kind: error.kind,
                message: error.message,
            },
            logs,
        };
    }

    fn mark_succeeded(&mut self) {
        if let Self::Running {
            snapshot,
            progress,
            logs,
            ..
        } = std::mem::take(self)
        {
            event!(
                Level::INFO,
                start_page = progress.start_page,
                end_page = progress.end_page,
                completed_pages = progress.completed_pages(),
                total = progress.total,
                "fetch run succeeded"
            );
            *self = Self::Succeeded {
                snapshot,
                progress,
                logs,
            };
        }
    }

    fn upsert_log(&mut self, log: FetchPageLog) {
        if let Self::Running { logs, .. }
        | Self::Interrupted { logs, .. }
        | Self::Failed { logs, .. }
        | Self::Succeeded { logs, .. } = self
        {
            upsert_page_log(logs, log);
        }
    }

    pub(super) fn last_success_page(&self) -> Option<u32> {
        match self {
            Self::Running { progress, .. }
            | Self::Interrupted { progress, .. }
            | Self::Succeeded { progress, .. } => progress.last_success_page,
            Self::Failed { failure, .. } => failure.progress.last_success_page,
            Self::Idle => None,
        }
    }

    pub(super) fn failed_page(&self) -> Option<u32> {
        match self {
            Self::Failed { failure, .. } => Some(failure.page),
            _ => None,
        }
    }

    pub(super) fn snapshot(&self) -> Option<&FetchRequest> {
        match self {
            Self::Idle => None,
            Self::Running { snapshot, .. }
            | Self::Interrupted { snapshot, .. }
            | Self::Failed { snapshot, .. }
            | Self::Succeeded { snapshot, .. } => Some(snapshot),
        }
    }

    pub(super) fn logs(&self) -> &[FetchPageLog] {
        match self {
            Self::Idle => &[],
            Self::Running { logs, .. }
            | Self::Interrupted { logs, .. }
            | Self::Failed { logs, .. }
            | Self::Succeeded { logs, .. } => logs,
        }
    }

    pub(super) fn status(&self) -> FetchStatus {
        match self {
            Self::Idle => FetchStatus::Idle,
            Self::Running { progress, .. } => FetchStatus::Running(*progress),
            Self::Interrupted { progress, .. } => FetchStatus::Interrupted(*progress),
            Self::Failed { failure, .. } => FetchStatus::Failed(failure.clone()),
            Self::Succeeded { progress, .. } => FetchStatus::Success(*progress),
        }
    }

    fn reject(&mut self, request: Option<FetchRequest>, error: FetchPageError) {
        if let Some(request) = request {
            let progress = FetchProgress {
                start_page: request.start_page,
                end_page: request.end_page,
                current_page: error.page,
                last_success_page: None,
                total: 0,
            };
            let mut logs = Vec::new();
            upsert_page_log(
                &mut logs,
                FetchPageLog {
                    page: error.page,
                    status: FetchPageLogStatus::Failed,
                    inserted: None,
                    elapsed_ms: None,
                    message: error.message.clone(),
                },
            );
            *self = Self::Failed {
                snapshot: request,
                failure: FetchFailure {
                    progress,
                    page: error.page,
                    kind: error.kind,
                    message: error.message,
                },
                logs,
            };
            return;
        }
        self.mark_failed(error, None);
    }
}

fn upsert_page_log(logs: &mut Vec<FetchPageLog>, log: FetchPageLog) {
    if let Some(existing) = logs.iter_mut().find(|existing| existing.page == log.page) {
        *existing = log;
    } else {
        logs.push(log);
    }
    if logs.len() > MAX_PAGE_LOGS {
        let overflow = logs.len() - MAX_PAGE_LOGS;
        logs.drain(0..overflow);
    }
}

pub(super) enum FetchMessage {
    Start {
        request: FetchRequest,
        clear_logs: bool,
        task: Task<()>,
    },
    Rejected {
        request: Option<FetchRequest>,
        error: FetchPageError,
    },
    Interrupt,
    PageStarted(u32),
    PageSucceeded {
        page: u32,
        inserted: usize,
        total: i64,
        elapsed_ms: u128,
    },
    Failed {
        error: FetchPageError,
        elapsed_ms: Option<u128>,
    },
    Succeeded,
}

impl Transition<FetchMessage> for &mut FetchRun {
    type Output = ();

    fn transition(self, message: FetchMessage) {
        match message {
            FetchMessage::Start {
                request,
                clear_logs,
                task,
            } if !self.is_running() => {
                self.begin_run(request, 0, clear_logs, task);
            }
            FetchMessage::Interrupt if self.is_running() => self.interrupt(),
            FetchMessage::Rejected { request, error } if !self.is_running() => {
                self.reject(request, error);
            }
            FetchMessage::PageStarted(page) if self.is_running() => self.mark_page_started(page),
            FetchMessage::PageSucceeded {
                page,
                inserted,
                total,
                elapsed_ms,
            } if self.is_running() => self.mark_page_succeeded(page, inserted, total, elapsed_ms),
            FetchMessage::Failed { error, elapsed_ms } if self.is_running() => {
                self.mark_failed(error, elapsed_ms)
            }
            FetchMessage::Succeeded if self.is_running() => self.mark_succeeded(),
            _ => tracing::debug!("ignored fetch transition"),
        }
    }
}

pub(super) fn page_count(start_page: u32, end_page: u32) -> u32 {
    end_page.saturating_sub(start_page).saturating_add(1)
}

pub(super) fn resume_page_after_interrupt(
    last_success_page: Option<u32>,
    start_page: u32,
    end_page: u32,
) -> Option<u32> {
    let next_page = last_success_page
        .map(|page| page.saturating_add(1))
        .unwrap_or(start_page);
    (next_page <= end_page).then_some(next_page.max(start_page))
}

pub(super) fn retry_page_after_failure(
    failed_page: Option<u32>,
    start_page: u32,
    end_page: u32,
) -> Option<u32> {
    failed_page
        .filter(|page| *page >= start_page && *page <= end_page)
        .or(Some(start_page).filter(|page| *page <= end_page))
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, Task, TestAppContext};
    use gpui_operation::Transition;
    use gpui_store::Store;

    use super::*;
    use crate::foundation::i18n::I18n;

    fn request(url: &str) -> FetchRequest {
        FetchRequest {
            url: url.to_owned(),
            start_page: 1,
            end_page: 10,
            cookie: String::new(),
        }
    }

    #[test]
    fn resumes_and_retries_use_the_frozen_range() {
        assert_eq!(resume_page_after_interrupt(Some(3), 1, 10), Some(4));
        assert_eq!(resume_page_after_interrupt(Some(10), 1, 10), None);
        assert_eq!(retry_page_after_failure(Some(6), 1, 10), Some(6));
        assert_eq!(retry_page_after_failure(Some(20), 1, 10), Some(1));
    }

    #[test]
    fn second_start_is_discarded_while_running() {
        let mut run = FetchRun::default();
        let first = request("first");
        run.transition(FetchMessage::Start {
            request: first.clone(),
            clear_logs: true,
            task: Task::ready(()),
        });
        run.transition(FetchMessage::Start {
            request: request("second"),
            clear_logs: true,
            task: Task::ready(()),
        });
        assert_eq!(run.snapshot(), Some(&first));
    }

    #[test]
    fn titlebar_summary_does_not_expose_cookie() {
        let i18n = I18n::chinese_for_test();
        let mut run = FetchRun::default();
        run.transition(FetchMessage::Start {
            request: FetchRequest {
                cookie: "secret-cookie".to_owned(),
                ..request("test")
            },
            clear_logs: true,
            task: Task::ready(()),
        });
        assert!(!run.titlebar_summary(&i18n).contains("secret-cookie"));
    }

    struct FetchOwner {
        run: Store<FetchRun>,
    }

    #[gpui::test]
    async fn dropping_non_global_owner_cancels_task_without_a_store_cycle(cx: &mut TestAppContext) {
        let owner = cx.new(|cx| FetchOwner {
            run: Store::new(cx, FetchRun::default()),
        });
        let weak_owner = owner.downgrade();
        owner.update(cx, |owner, cx| {
            let task = cx.spawn(async move |_owner, _cx| {
                std::future::pending::<()>().await;
            });
            owner.run.update(cx, |run| {
                run.transition(FetchMessage::Start {
                    request: request("test"),
                    clear_logs: true,
                    task,
                });
            });
        });

        drop(owner);
        cx.run_until_parked();

        assert!(weak_owner.upgrade().is_none());
    }
}
