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
    Forking {
        _task: Task<()>,
    },
}
impl SessionCommand {
    pub fn running(&self) -> bool {
        !matches!(self, Self::Idle)
    }
    pub fn finish(&mut self) {
        *self = Self::Idle;
    }
}
