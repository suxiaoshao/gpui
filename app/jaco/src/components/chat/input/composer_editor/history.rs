use super::{buffer::Selection, token::ComposerToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EditorState {
    pub(super) text: String,
    pub(super) selection: Selection,
    pub(super) marked_range: Option<std::ops::Range<usize>>,
    pub(super) tokens: Vec<ComposerToken>,
}

#[derive(Clone, Debug)]
struct Edit {
    before: EditorState,
    after: EditorState,
}

#[derive(Debug)]
pub(super) struct EditorHistory {
    transactions: gpui_kit::component::history::UndoHistory<Edit>,
}

impl Default for EditorHistory {
    fn default() -> Self {
        Self {
            transactions: gpui_kit::component::history::UndoHistory::new().max_undos(200),
        }
    }
}

impl EditorHistory {
    pub(super) fn record(&mut self, before: EditorState, after: EditorState) {
        if before != after {
            self.transactions.push(Edit { before, after });
        }
    }

    pub(super) fn undo(&mut self) -> Option<EditorState> {
        self.transactions
            .undo()?
            .last()
            .map(|edit| edit.before.clone())
    }

    pub(super) fn redo(&mut self) -> Option<EditorState> {
        self.transactions
            .redo()?
            .last()
            .map(|edit| edit.after.clone())
    }
}
