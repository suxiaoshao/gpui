//! Local read resources and core synchronization. No Pi process ownership lives here.
use gpui_kit::Task;
use gpui_operation::Transition;

pub(crate) enum ReadState<T> {
    Idle,
    Loading {
        id: u64,
        _task: Task<()>,
        previous: Option<T>,
    },
    Ready(T),
    Failed {
        previous: Option<T>,
        error: String,
    },
}
pub(super) enum ReadMessage<T> {
    Start { id: u64, task: Task<()> },
    Finish { id: u64, result: Result<T, String> },
    Cancel,
}
impl<T> ReadState<T> {
    pub fn reset(&mut self) {
        drop(std::mem::replace(self, Self::Idle));
    }
    pub fn data(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            Self::Loading { previous, .. } | Self::Failed { previous, .. } => previous.as_ref(),
            Self::Idle => None,
        }
    }
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }
    pub fn running(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }
    pub fn accepts(&self, request: u64) -> bool {
        matches!(self, Self::Loading { id, .. } if *id == request)
    }
}
impl<T> Transition<ReadMessage<T>> for &mut ReadState<T> {
    type Output = bool;
    fn transition(self, message: ReadMessage<T>) -> bool {
        match message {
            ReadMessage::Start { id, task } => {
                if self.running() {
                    return false;
                }
                let previous = match std::mem::replace(self, ReadState::Idle) {
                    ReadState::Ready(data) => Some(data),
                    ReadState::Failed { previous, .. } => previous,
                    _ => None,
                };
                *self = ReadState::Loading {
                    id,
                    _task: task,
                    previous,
                };
            }
            ReadMessage::Finish { id, result } => {
                if !self.accepts(id) {
                    return false;
                }
                let ReadState::Loading {
                    _task: task,
                    previous,
                    ..
                } = std::mem::replace(self, ReadState::Idle)
                else {
                    unreachable!()
                };
                *self = match result {
                    Ok(data) => ReadState::Ready(data),
                    Err(error) => ReadState::Failed { previous, error },
                };
                drop(task);
            }
            ReadMessage::Cancel => {
                if !self.running() {
                    return false;
                }
                let ReadState::Loading {
                    _task: task,
                    previous,
                    ..
                } = std::mem::replace(self, ReadState::Idle)
                else {
                    unreachable!()
                };
                *self = previous.map(ReadState::Ready).unwrap_or(ReadState::Idle);
                drop(task);
            }
        }
        true
    }
}

/// Core state/history remain the session's event-merged data. This enum owns
/// only the in-flight read and its scoped failure, not a duplicate snapshot.
pub(crate) enum CoreRead {
    Idle,
    CheckingFile { _task: Task<()> },
    Reading { _task: Task<()>, again: bool },
    Failed(String),
}
impl CoreRead {
    pub fn running(&self) -> bool {
        matches!(self, Self::CheckingFile { .. } | Self::Reading { .. })
    }
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
    pub fn queue(&mut self) {
        if let Self::Reading { again, .. } = self {
            *again = true;
        }
    }
    pub fn finish(&mut self, error: Option<String>) -> bool {
        let again = matches!(self, Self::Reading { again: true, .. }) && error.is_none();
        let old = std::mem::replace(self, error.map(Self::Failed).unwrap_or(Self::Idle));
        drop(old);
        again
    }
}

pub(crate) enum ModelChange {
    Idle,
    Applying {
        task: Task<()>,
        refresh_core: bool,
    },
    Reconciling {
        task: Task<()>,
        refresh_core: bool,
    },
    /// The command failed, but the actual settings were read back successfully.
    Failed(String),
    /// The actual settings could not be read back; sending remains blocked.
    Unconfirmed(String),
}
impl ModelChange {
    pub fn running(&self) -> bool {
        matches!(self, Self::Applying { .. } | Self::Reconciling { .. })
    }
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) | Self::Unconfirmed(error) => Some(error),
            _ => None,
        }
    }
    pub fn unconfirmed(&self) -> bool {
        matches!(self, Self::Unconfirmed(_))
    }
    pub fn reconciling(&mut self) {
        if matches!(self, Self::Applying { .. }) {
            let Self::Applying { task, refresh_core } = std::mem::replace(self, Self::Idle) else {
                unreachable!()
            };
            *self = Self::Reconciling { task, refresh_core };
        }
    }
    pub fn queue_core_refresh(&mut self) {
        if let Self::Applying { refresh_core, .. } | Self::Reconciling { refresh_core, .. } = self {
            *refresh_core = true;
        }
    }
    /// Ok carries a possible command error after successful readback; Err is an
    /// unconfirmed result. A queued core refresh requires confirmed settings.
    pub fn finish(&mut self, result: Result<Option<String>, String>) -> bool {
        let settled = match result {
            Ok(None) => Self::Idle,
            Ok(Some(error)) => Self::Failed(error),
            Err(error) => Self::Unconfirmed(error),
        };
        let old = std::mem::replace(self, settled);
        // Keep the task alive through both the command and canonical readback.
        match old {
            Self::Applying { task, refresh_core } | Self::Reconciling { task, refresh_core } => {
                drop(task);
                refresh_core && !self.unconfirmed()
            }
            _ => false,
        }
    }
}
