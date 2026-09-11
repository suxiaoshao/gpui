use gpui_kit::Task;

#[derive(Default)]
pub(crate) enum SessionCommand {
    #[default]
    Idle,
    Renaming {
        _task: Task<()>,
    },
    Forking {
        _task: Task<()>,
        editor: Option<String>,
    },
}
impl SessionCommand {
    pub fn running(&self) -> bool {
        matches!(self, Self::Renaming { .. } | Self::Forking { .. })
    }
    pub fn finish(&mut self) -> Option<String> {
        match std::mem::replace(self, Self::Idle) {
            Self::Forking { editor, .. } => editor,
            Self::Idle | Self::Renaming { .. } => None,
        }
    }
}
