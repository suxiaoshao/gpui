//! The catalog lifecycle owns its task. Progress is a message, never a second state machine.
use crate::foundation::session_catalog::{Catalog, ScanProgress};
use gpui_operation::Transition;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) struct ScanWork<T> {
    pub id: u64,
    _task: T,
    pub cancel: Arc<AtomicBool>,
    again: bool,
}
impl<T> ScanWork<T> {
    pub fn new(id: u64, task: T, cancel: Arc<AtomicBool>) -> Self {
        Self {
            id,
            _task: task,
            cancel,
            again: false,
        }
    }
}
impl<T> Drop for ScanWork<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // The task drops after cancellation becomes visible to blocking readers.
    }
}

#[derive(Default)]
pub(crate) enum CatalogState<T> {
    #[default]
    Idle,
    Discovering {
        work: ScanWork<T>,
        previous: Option<Catalog>,
        files: usize,
    },
    Reading {
        work: ScanWork<T>,
        previous: Option<Catalog>,
        completed: usize,
        total: usize,
    },
    Ready(Catalog),
    Failed {
        previous: Option<Catalog>,
        error: String,
    },
}
pub(crate) enum Message<T> {
    Start(ScanWork<T>),
    Progress {
        id: u64,
        value: ScanProgress,
    },
    Finish {
        id: u64,
        result: Result<Catalog, String>,
    },
    QueueRefresh,
    Cancel,
    RemoveSession(PathBuf),
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Update {
    Ignored,
    Changed,
    Finished { rescan: bool },
}
impl<T> CatalogState<T> {
    pub fn data(&self) -> Option<&Catalog> {
        match self {
            Self::Ready(data) => Some(data),
            Self::Discovering { previous, .. }
            | Self::Reading { previous, .. }
            | Self::Failed { previous, .. } => previous.as_ref(),
            Self::Idle => None,
        }
    }
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }
    pub fn progress(&self) -> Option<ScanProgress> {
        match self {
            Self::Discovering { files, .. } => Some(ScanProgress::Discovering { files: *files }),
            Self::Reading {
                completed, total, ..
            } => Some(ScanProgress::Reading {
                completed: *completed,
                total: *total,
            }),
            _ => None,
        }
    }
    pub fn running(&self) -> bool {
        self.work().is_some()
    }
    fn work(&self) -> Option<&ScanWork<T>> {
        match self {
            Self::Discovering { work, .. } | Self::Reading { work, .. } => Some(work),
            _ => None,
        }
    }
}
impl<T> Transition<Message<T>> for &mut CatalogState<T> {
    type Output = Update;
    fn transition(self, message: Message<T>) -> Update {
        match message {
            Message::RemoveSession(path) => {
                // Discard any scan that may have read the file before deletion.
                self.transition(Message::Cancel);
                match self {
                    CatalogState::Ready(data)
                    | CatalogState::Failed {
                        previous: Some(data),
                        ..
                    } => {
                        data.sessions.retain(|info| info.path != path);
                    }
                    _ => {}
                }
            }
            Message::Start(work) => {
                if self.running() {
                    return Update::Ignored;
                }
                let previous = match std::mem::replace(self, CatalogState::Idle) {
                    CatalogState::Ready(data) => Some(data),
                    CatalogState::Failed { previous, .. } => previous,
                    _ => None,
                };
                *self = CatalogState::Discovering {
                    work,
                    previous,
                    files: 0,
                };
            }
            Message::Progress { id, value } => {
                if self.work().is_none_or(|work| work.id != id) {
                    return Update::Ignored;
                }
                match (self, value) {
                    (
                        CatalogState::Discovering { files, .. },
                        ScanProgress::Discovering { files: count },
                    ) if count >= *files => *files = count,
                    (
                        state @ CatalogState::Discovering { .. },
                        ScanProgress::Reading {
                            completed: 0,
                            total,
                        },
                    ) => {
                        let CatalogState::Discovering { work, previous, .. } =
                            std::mem::replace(state, CatalogState::Idle)
                        else {
                            unreachable!()
                        };
                        *state = CatalogState::Reading {
                            work,
                            previous,
                            completed: 0,
                            total,
                        };
                    }
                    (
                        CatalogState::Reading {
                            completed, total, ..
                        },
                        ScanProgress::Reading {
                            completed: count,
                            total: count_total,
                        },
                    ) if count_total == *total && count >= *completed && count <= *total => {
                        *completed = count
                    }
                    _ => return Update::Ignored,
                }
            }
            Message::Finish { id, result } => {
                if self.work().is_none_or(|work| work.id != id) {
                    return Update::Ignored;
                }
                if result.is_ok()
                    && !matches!(self, CatalogState::Reading { completed, total, .. } if completed == total)
                {
                    return Update::Ignored;
                }
                let (work, previous) = match std::mem::replace(self, CatalogState::Idle) {
                    CatalogState::Discovering { work, previous, .. }
                    | CatalogState::Reading { work, previous, .. } => (work, previous),
                    _ => unreachable!(),
                };
                let rescan = result.is_ok() && work.again;
                *self = match result {
                    Ok(data) => CatalogState::Ready(data),
                    Err(error) => CatalogState::Failed { previous, error },
                };
                drop(work);
                return Update::Finished { rescan };
            }
            Message::QueueRefresh => match self {
                CatalogState::Discovering { work, .. } | CatalogState::Reading { work, .. } => {
                    work.again = true
                }
                _ => return Update::Ignored,
            },
            Message::Cancel => {
                if !self.running() {
                    return Update::Ignored;
                }
                let (work, previous) = match std::mem::replace(self, CatalogState::Idle) {
                    CatalogState::Discovering { work, previous, .. }
                    | CatalogState::Reading { work, previous, .. } => (work, previous),
                    _ => unreachable!(),
                };
                *self = previous
                    .map(CatalogState::Ready)
                    .unwrap_or(CatalogState::Idle);
                drop(work);
            }
        }
        Update::Changed
    }
}

#[cfg(test)]
mod tests;
