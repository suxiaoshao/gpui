use super::{buffer::Selection, token::ComposerToken};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EditorState {
    pub(super) text: String,
    pub(super) selection: Selection,
    pub(super) marked_range: Option<std::ops::Range<usize>>,
    pub(super) tokens: Vec<ComposerToken>,
}

#[derive(Debug)]
pub(super) struct EditorHistory {
    undo: Vec<EditorState>,
    redo: Vec<EditorState>,
    max_entries: usize,
}

impl Default for EditorHistory {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_entries: 200,
        }
    }
}

impl EditorHistory {
    pub(super) fn record_before(&mut self, state: EditorState) {
        if self.undo.last() == Some(&state) {
            return;
        }

        self.undo.push(state);
        if self.undo.len() > self.max_entries {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub(super) fn undo(&mut self, current: EditorState) -> Option<EditorState> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        Some(previous)
    }

    pub(super) fn redo(&mut self, current: EditorState) -> Option<EditorState> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    pub(super) fn clear_redo(&mut self) {
        self.redo.clear();
    }
}
