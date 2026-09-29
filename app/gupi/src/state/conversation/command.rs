use gpui_kit::Task;

#[derive(Default)]
pub(crate) enum SessionCommand {
    #[default]
    Idle,
    Renaming {
        _task: Task<()>,
    },
    Deleting {
        task: Task<()>,
    },
    Closing {
        _task: Task<()>,
    },
    Reconnecting {
        _task: Task<()>,
    },
    ReconnectUnconfirmed,
    Forking {
        _task: Task<()>,
    },
    Exporting {
        _task: Task<()>,
    },
    Compacting {
        _task: Task<()>,
    },
    ClearingQueue {
        _task: Task<()>,
    },
}
impl SessionCommand {
    pub fn clearing_queue(&self) -> bool {
        matches!(self, Self::ClearingQueue { .. })
    }
    pub fn compacting(&self) -> bool {
        matches!(self, Self::Compacting { .. })
    }
    pub fn exporting(&self) -> bool {
        matches!(self, Self::Exporting { .. })
    }
    pub fn reconnecting(&self) -> bool {
        matches!(self, Self::Reconnecting { .. })
    }
    pub fn running(&self) -> bool {
        !matches!(self, Self::Idle)
    }
    pub fn finish(&mut self) {
        *self = Self::Idle;
    }
}
