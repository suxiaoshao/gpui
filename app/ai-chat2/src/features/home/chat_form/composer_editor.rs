mod blink_cursor;
mod buffer;
mod element;
mod history;
mod snapshot;
mod token;

#[allow(unused_imports)]
pub(crate) use snapshot::{ComposerAttachment, ComposerSendPolicy, ComposerSnapshot};

use std::{collections::BTreeMap, ops::Range, path::Path};

use ai_chat_agent::SkillCatalog;
use gpui::{
    App, AppContext as _, ClipboardItem, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    SharedString, Styled as _, Subscription, UTF16Selection, Window, actions,
};

use self::{
    blink_cursor::BlinkCursor,
    buffer::{
        Selection, byte_range_to_utf16_range, byte_to_utf16, clamp_offset, line_start,
        next_char_boundary, next_word_end, offset_for_line_column, previous_char_boundary,
        previous_word_start, utf16_range_to_byte_range,
    },
    element::ComposerEditorElement,
    history::{EditorHistory, EditorState},
    snapshot::build_snapshot,
    token::{ComposerSkill, ComposerToken, parse_skill_tokens, skills_from_catalog},
};

const KEY_CONTEXT: &str = "AiChat2ComposerEditor";

actions!(
    ai_chat2_composer_editor,
    [
        ComposerBackspace,
        ComposerDelete,
        ComposerDeletePreviousWord,
        ComposerDeleteNextWord,
        ComposerDeleteToLineStart,
        ComposerDeleteToLineEnd,
        ComposerMoveLeft,
        ComposerMoveRight,
        ComposerMoveUp,
        ComposerMoveDown,
        ComposerMovePreviousWord,
        ComposerMoveNextWord,
        ComposerMoveLineStart,
        ComposerMoveLineEnd,
        ComposerMoveStart,
        ComposerMoveEnd,
        ComposerSelectLeft,
        ComposerSelectRight,
        ComposerSelectUp,
        ComposerSelectDown,
        ComposerSelectPreviousWord,
        ComposerSelectNextWord,
        ComposerSelectLineStart,
        ComposerSelectLineEnd,
        ComposerSelectStart,
        ComposerSelectEnd,
        ComposerSelectAll,
        ComposerNewline,
        ComposerSubmit,
        ComposerUndo,
        ComposerRedo,
        ComposerCopy,
        ComposerCut,
        ComposerPaste,
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", ComposerBackspace, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", ComposerDelete, Some(KEY_CONTEXT)),
        KeyBinding::new("left", ComposerMoveLeft, Some(KEY_CONTEXT)),
        KeyBinding::new("right", ComposerMoveRight, Some(KEY_CONTEXT)),
        KeyBinding::new("up", ComposerMoveUp, Some(KEY_CONTEXT)),
        KeyBinding::new("down", ComposerMoveDown, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-left", ComposerSelectLeft, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-right", ComposerSelectRight, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-up", ComposerSelectUp, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-down", ComposerSelectDown, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-enter", ComposerNewline, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", ComposerSubmit, Some(KEY_CONTEXT)),
    ]);

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-a", ComposerSelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-z", ComposerUndo, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-shift-z", ComposerRedo, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-c", ComposerCopy, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-x", ComposerCut, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-v", ComposerPaste, Some(KEY_CONTEXT)),
        KeyBinding::new("alt-left", ComposerMovePreviousWord, Some(KEY_CONTEXT)),
        KeyBinding::new("alt-right", ComposerMoveNextWord, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "alt-shift-left",
            ComposerSelectPreviousWord,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new("alt-shift-right", ComposerSelectNextWord, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-left", ComposerMoveLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-right", ComposerMoveLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-shift-left", ComposerSelectLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-shift-right", ComposerSelectLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-up", ComposerMoveStart, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-down", ComposerMoveEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-shift-up", ComposerSelectStart, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-shift-down", ComposerSelectEnd, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "alt-backspace",
            ComposerDeletePreviousWord,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new("alt-delete", ComposerDeleteNextWord, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "cmd-backspace",
            ComposerDeleteToLineStart,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new("cmd-delete", ComposerDeleteToLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("home", ComposerMoveLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("end", ComposerMoveLineEnd, Some(KEY_CONTEXT)),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-a", ComposerSelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-z", ComposerUndo, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-y", ComposerRedo, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-shift-z", ComposerRedo, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-c", ComposerCopy, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-x", ComposerCut, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-v", ComposerPaste, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-left", ComposerMovePreviousWord, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-right", ComposerMoveNextWord, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "ctrl-shift-left",
            ComposerSelectPreviousWord,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(
            "ctrl-shift-right",
            ComposerSelectNextWord,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new("home", ComposerMoveLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("end", ComposerMoveLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-home", ComposerSelectLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-end", ComposerSelectLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-home", ComposerMoveStart, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-end", ComposerMoveEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-shift-home", ComposerSelectStart, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-shift-end", ComposerSelectEnd, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "ctrl-backspace",
            ComposerDeletePreviousWord,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new("ctrl-delete", ComposerDeleteNextWord, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Debug)]
pub(crate) enum ComposerEditorEvent {
    Changed,
    SubmitRequested(ComposerSnapshot),
}

impl EventEmitter<ComposerEditorEvent> for ComposerEditor {}

pub(crate) struct ComposerEditor {
    text: String,
    placeholder: SharedString,
    selection: Selection,
    marked_range: Option<Range<usize>>,
    tokens: Vec<ComposerToken>,
    skills: BTreeMap<String, ComposerSkill>,
    next_token_id: u64,
    history: EditorHistory,
    composition_base: Option<EditorState>,
    preferred_column: Option<usize>,
    selecting: bool,
    focus_handle: FocusHandle,
    blink_cursor: gpui::Entity<BlinkCursor>,
    last_layout: Option<element::LayoutCache>,
    _subscriptions: Vec<Subscription>,
}

impl ComposerEditor {
    pub(crate) const MAX_VISIBLE_LINES: usize = 8;

    pub(crate) fn new(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let skills = SkillCatalog::scan(None)
            .map(|catalog| skills_from_catalog(&catalog))
            .unwrap_or_default();
        let focus_handle = cx.focus_handle();
        let blink_cursor = cx.new(|_| BlinkCursor::new());
        let _subscriptions = vec![
            cx.observe(&blink_cursor, |_, _, cx| cx.notify()),
            cx.observe_window_activation(window, |editor, window, cx| {
                editor.sync_blink_cursor(window, cx);
            }),
            cx.on_focus(&focus_handle, window, |editor, window, cx| {
                editor.sync_blink_cursor(window, cx);
                cx.notify();
            }),
            cx.on_blur(&focus_handle, window, |editor, window, cx| {
                editor.sync_blink_cursor(window, cx);
                cx.notify();
            }),
        ];

        Self {
            text: String::new(),
            placeholder: placeholder.into(),
            selection: Selection::default(),
            marked_range: None,
            tokens: Vec::new(),
            skills,
            next_token_id: 1,
            history: EditorHistory::default(),
            composition_base: None,
            preferred_column: None,
            selecting: false,
            focus_handle,
            blink_cursor,
            last_layout: None,
            _subscriptions,
        }
    }

    pub(crate) fn snapshot(&self) -> ComposerSnapshot {
        build_snapshot(&self.text, &self.tokens)
    }

    pub(crate) fn can_submit(&self) -> bool {
        !self.snapshot().is_empty()
    }

    pub(crate) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
    }

    #[allow(dead_code)]
    pub(crate) fn refresh_skill_catalog(
        &mut self,
        project_root: Option<&Path>,
        cx: &mut Context<Self>,
    ) {
        self.skills = SkillCatalog::scan(project_root)
            .map(|catalog| skills_from_catalog(&catalog))
            .unwrap_or_default();
        self.refresh_tokens();
        cx.notify();
        cx.emit(ComposerEditorEvent::Changed);
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn placeholder(&self) -> &SharedString {
        &self.placeholder
    }

    fn selection(&self) -> &Selection {
        &self.selection
    }

    pub(super) fn marked_range(&self) -> &Option<Range<usize>> {
        &self.marked_range
    }

    fn tokens(&self) -> &[ComposerToken] {
        &self.tokens
    }

    pub(super) fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub(super) fn show_cursor(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle.is_focused(window)
            && window.is_window_active()
            && self.blink_cursor.read(cx).visible()
    }

    pub(super) fn display_line_count(&self) -> usize {
        buffer::line_ranges(&self.text).len()
    }

    fn current_state(&self) -> EditorState {
        EditorState {
            text: self.text.clone(),
            selection: self.selection.clone(),
            marked_range: self.marked_range.clone(),
            tokens: self.tokens.clone(),
        }
    }

    fn restore_state(&mut self, state: EditorState, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        self.text = state.text;
        self.selection = state.selection;
        self.marked_range = state.marked_range;
        self.tokens = state.tokens;
        self.preferred_column = None;
        cx.notify();
        cx.emit(ComposerEditorEvent::Changed);
    }

    fn record_before_change(&mut self) {
        self.history.record_before(self.current_state());
        self.composition_base = None;
    }

    fn sync_blink_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = window.is_window_active() && self.focus_handle.is_focused(window);
        self.blink_cursor.update(cx, |cursor, cx| {
            if active {
                cursor.start(cx);
            } else {
                cursor.stop(cx);
            }
        });
    }

    fn pause_cursor_blink(&mut self, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| cursor.pause(cx));
    }

    fn refresh_tokens(&mut self) {
        self.tokens = parse_skill_tokens(&self.text, &self.skills, &mut self.next_token_id);
    }

    fn replace_range(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        record_history: bool,
        cx: &mut Context<Self>,
    ) {
        self.pause_cursor_blink(cx);
        if record_history {
            self.record_before_change();
        }

        let range = buffer::normalize_range(&self.text, range);
        self.text.replace_range(range.clone(), new_text);
        let cursor = range.start + new_text.len();
        self.selection.collapse(clamp_offset(&self.text, cursor));
        self.marked_range = None;
        self.preferred_column = None;
        self.refresh_tokens();
        cx.notify();
        cx.emit(ComposerEditorEvent::Changed);
    }

    fn replace_selection(&mut self, new_text: &str, record_history: bool, cx: &mut Context<Self>) {
        self.replace_range(self.selection.range(), new_text, record_history, cx);
    }

    fn cursor(&self) -> usize {
        self.selection.head
    }

    fn move_to(&mut self, offset: usize, selecting: bool, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        let offset = clamp_offset(&self.text, offset);
        self.selection.move_head(offset, selecting);
        self.preferred_column = None;
        self.marked_range = None;
        cx.notify();
    }

    fn move_vertically(&mut self, delta: isize, selecting: bool, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        let (line_ix, column) = buffer::line_column(&self.text, self.cursor());
        let preferred_column = self.preferred_column.unwrap_or(column);
        let line_count = buffer::line_ranges(&self.text).len();
        let next_line_ix = if delta < 0 {
            line_ix.saturating_sub(delta.unsigned_abs())
        } else {
            (line_ix + delta as usize).min(line_count.saturating_sub(1))
        };
        let offset = offset_for_line_column(&self.text, next_line_ix, preferred_column);
        self.selection.move_head(offset, selecting);
        self.preferred_column = Some(preferred_column);
        self.marked_range = None;
        cx.notify();
    }

    fn selection_or_previous_char(&self) -> Option<Range<usize>> {
        if !self.selection.is_empty() {
            return Some(self.selection.range());
        }
        let cursor = self.cursor();
        (cursor > 0).then(|| previous_char_boundary(&self.text, cursor)..cursor)
    }

    fn selection_or_next_char(&self) -> Option<Range<usize>> {
        if !self.selection.is_empty() {
            return Some(self.selection.range());
        }
        let cursor = self.cursor();
        (cursor < self.text.len()).then(|| cursor..next_char_boundary(&self.text, cursor))
    }

    fn delete_range_or_bell(
        &mut self,
        range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(range) = range
            && range.start != range.end
        {
            self.replace_range(range, "", true, cx);
            return;
        }
        window.play_system_bell();
    }

    fn select_word_at(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        let offset = clamp_offset(&self.text, offset);
        if self.text.is_empty() {
            self.selection.collapse(0);
            cx.notify();
            return;
        }
        let start = previous_word_start(&self.text, offset);
        let end = next_word_end(&self.text, offset);
        self.selection = Selection {
            anchor: start,
            head: end,
        };
        cx.notify();
    }

    fn select_line_at(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        let offset = clamp_offset(&self.text, offset);
        self.selection = Selection {
            anchor: line_start(&self.text, offset),
            head: buffer::line_end(&self.text, offset),
        };
        cx.notify();
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = &self.last_layout else {
            return self.text.len();
        };
        if layout.lines.is_empty() {
            return 0;
        }

        let local_y = position.y - layout.bounds.top();
        let mut line_ix = (local_y / layout.line_height).floor() as usize;
        line_ix = line_ix.min(layout.lines.len().saturating_sub(1));
        let line = &layout.lines[line_ix];
        let local_x = position.x - layout.bounds.left();
        let relative = line.line.closest_index_for_x(local_x);
        clamp_offset(
            &self.text,
            line.range.start + relative.min(line.range.len()),
        )
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus(window, cx);
        self.selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        match event.click_count {
            2 => self.select_word_at(offset, cx),
            3.. => self.select_line_at(offset, cx),
            _ if event.modifiers.shift => self.move_to(offset, true, cx),
            _ => self.move_to(offset, false, cx),
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.move_to(self.index_for_mouse_position(event.position), true, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn on_backspace(&mut self, _: &ComposerBackspace, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_range_or_bell(self.selection_or_previous_char(), window, cx);
    }

    fn on_delete(&mut self, _: &ComposerDelete, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_range_or_bell(self.selection_or_next_char(), window, cx);
    }

    fn on_delete_previous_word(
        &mut self,
        _: &ComposerDeletePreviousWord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if self.selection.is_empty() {
            previous_word_start(&self.text, self.cursor())..self.cursor()
        } else {
            self.selection.range()
        };
        self.delete_range_or_bell(Some(range), window, cx);
    }

    fn on_delete_next_word(
        &mut self,
        _: &ComposerDeleteNextWord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if self.selection.is_empty() {
            self.cursor()..next_word_end(&self.text, self.cursor())
        } else {
            self.selection.range()
        };
        self.delete_range_or_bell(Some(range), window, cx);
    }

    fn on_delete_to_line_start(
        &mut self,
        _: &ComposerDeleteToLineStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if self.selection.is_empty() {
            line_start(&self.text, self.cursor())..self.cursor()
        } else {
            self.selection.range()
        };
        self.delete_range_or_bell(Some(range), window, cx);
    }

    fn on_delete_to_line_end(
        &mut self,
        _: &ComposerDeleteToLineEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if self.selection.is_empty() {
            self.cursor()..buffer::line_end(&self.text, self.cursor())
        } else {
            self.selection.range()
        };
        self.delete_range_or_bell(Some(range), window, cx);
    }

    fn on_move_left(&mut self, _: &ComposerMoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selection.is_empty() {
            previous_char_boundary(&self.text, self.cursor())
        } else {
            self.selection.range().start
        };
        self.move_to(offset, false, cx);
    }

    fn on_move_right(&mut self, _: &ComposerMoveRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selection.is_empty() {
            next_char_boundary(&self.text, self.cursor())
        } else {
            self.selection.range().end
        };
        self.move_to(offset, false, cx);
    }

    fn on_move_up(&mut self, _: &ComposerMoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, false, cx);
    }

    fn on_move_down(&mut self, _: &ComposerMoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, false, cx);
    }

    fn on_move_previous_word(
        &mut self,
        _: &ComposerMovePreviousWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(previous_word_start(&self.text, self.cursor()), false, cx);
    }

    fn on_move_next_word(
        &mut self,
        _: &ComposerMoveNextWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(next_word_end(&self.text, self.cursor()), false, cx);
    }

    fn on_move_line_start(
        &mut self,
        _: &ComposerMoveLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(line_start(&self.text, self.cursor()), false, cx);
    }

    fn on_move_line_end(
        &mut self,
        _: &ComposerMoveLineEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(buffer::line_end(&self.text, self.cursor()), false, cx);
    }

    fn on_move_start(&mut self, _: &ComposerMoveStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, false, cx);
    }

    fn on_move_end(&mut self, _: &ComposerMoveEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), false, cx);
    }

    fn on_select_left(&mut self, _: &ComposerSelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(previous_char_boundary(&self.text, self.cursor()), true, cx);
    }

    fn on_select_right(&mut self, _: &ComposerSelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(next_char_boundary(&self.text, self.cursor()), true, cx);
    }

    fn on_select_up(&mut self, _: &ComposerSelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, true, cx);
    }

    fn on_select_down(&mut self, _: &ComposerSelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, true, cx);
    }

    fn on_select_previous_word(
        &mut self,
        _: &ComposerSelectPreviousWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(previous_word_start(&self.text, self.cursor()), true, cx);
    }

    fn on_select_next_word(
        &mut self,
        _: &ComposerSelectNextWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(next_word_end(&self.text, self.cursor()), true, cx);
    }

    fn on_select_line_start(
        &mut self,
        _: &ComposerSelectLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(line_start(&self.text, self.cursor()), true, cx);
    }

    fn on_select_line_end(
        &mut self,
        _: &ComposerSelectLineEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(buffer::line_end(&self.text, self.cursor()), true, cx);
    }

    fn on_select_start(&mut self, _: &ComposerSelectStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, true, cx);
    }

    fn on_select_end(&mut self, _: &ComposerSelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), true, cx);
    }

    fn on_select_all(&mut self, _: &ComposerSelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        self.selection = Selection {
            anchor: 0,
            head: self.text.len(),
        };
        self.preferred_column = None;
        cx.notify();
    }

    fn on_newline(&mut self, _: &ComposerNewline, _: &mut Window, cx: &mut Context<Self>) {
        self.replace_selection("\n", true, cx);
    }

    fn on_submit(&mut self, _: &ComposerSubmit, _: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.snapshot();
        if !snapshot.is_empty() {
            cx.emit(ComposerEditorEvent::SubmitRequested(snapshot));
        }
    }

    fn on_undo(&mut self, _: &ComposerUndo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.history.undo(self.current_state()) {
            self.restore_state(state, cx);
        }
    }

    fn on_redo(&mut self, _: &ComposerRedo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.history.redo(self.current_state()) {
            self.restore_state(state, cx);
        }
    }

    fn on_copy(&mut self, _: &ComposerCopy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text[self.selection.range()].to_string(),
            ));
        }
    }

    fn on_cut(&mut self, _: &ComposerCut, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.text[self.selection.range()].to_string(),
        ));
        self.delete_range_or_bell(Some(self.selection.range()), window, cx);
    }

    fn on_paste(&mut self, _: &ComposerPaste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, text.as_ref(), window, cx);
        }
    }
}

impl EntityInputHandler for ComposerEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = utf16_range_to_byte_range(&self.text, range_utf16);
        adjusted_range.replace(byte_range_to_utf16_range(&self.text, range.clone()));
        Some(self.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: byte_range_to_utf16_range(&self.text, self.selection.range()),
            reversed: self.selection.reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| byte_range_to_utf16_range(&self.text, range.clone()))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.take().is_some() {
            self.pause_cursor_blink(cx);
            if let Some(base) = self.composition_base.take() {
                self.history.record_before(base);
                self.history.clear_redo();
            }
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|range| utf16_range_to_byte_range(&self.text, range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.range());

        if let Some(base) = self.composition_base.take() {
            self.replace_range(range, text, false, cx);
            self.history.record_before(base);
            self.history.clear_redo();
        } else {
            self.replace_range(range, text, true, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pause_cursor_blink(cx);
        if self.composition_base.is_none() {
            self.composition_base = Some(self.current_state());
        }

        let range = range_utf16
            .map(|range| {
                if let Some(marked_range) = &self.marked_range {
                    let relative_start = utf16_range_to_byte_range(
                        &self.text[marked_range.clone()],
                        range.start..range.start,
                    )
                    .start;
                    let relative_end = utf16_range_to_byte_range(
                        &self.text[marked_range.clone()],
                        range.end..range.end,
                    )
                    .start;
                    marked_range.start + relative_start..marked_range.start + relative_end
                } else {
                    utf16_range_to_byte_range(&self.text, range)
                }
            })
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.range());

        let range = buffer::normalize_range(&self.text, range);
        self.text.replace_range(range.clone(), new_text);
        let marked = range.start..range.start + new_text.len();
        self.marked_range = (!new_text.is_empty()).then_some(marked.clone());
        let selected = new_selected_range_utf16
            .map(|selection| {
                let start = buffer::utf16_to_byte(new_text, selection.start);
                let end = buffer::utf16_to_byte(new_text, selection.end);
                range.start + start..range.start + end
            })
            .unwrap_or_else(|| marked.end..marked.end);
        self.selection = Selection {
            anchor: clamp_offset(&self.text, selected.start),
            head: clamp_offset(&self.text, selected.end),
        };
        self.refresh_tokens();
        cx.notify();
        cx.emit(ComposerEditorEvent::Changed);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: gpui::Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui::Bounds<Pixels>> {
        let byte = buffer::utf16_to_byte(&self.text, range_utf16.start);
        let layout = self.last_layout.as_ref()?;
        let line = layout
            .lines
            .iter()
            .find(|line| byte >= line.range.start && byte <= line.range.end)
            .or_else(|| layout.lines.last())?;
        let x = line.line.x_for_index(byte.saturating_sub(line.range.start));
        Some(gpui::Bounds::new(
            gpui::point(element_bounds.left() + x, element_bounds.top() + line.y),
            gpui::size(gpui::px(1.), layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(byte_to_utf16(
            &self.text,
            self.index_for_mouse_position(point),
        ))
    }
}

impl Focusable for ComposerEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ComposerEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::div()
            .id("ai-chat2-composer-editor")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::on_backspace))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_delete_previous_word))
            .on_action(cx.listener(Self::on_delete_next_word))
            .on_action(cx.listener(Self::on_delete_to_line_start))
            .on_action(cx.listener(Self::on_delete_to_line_end))
            .on_action(cx.listener(Self::on_move_left))
            .on_action(cx.listener(Self::on_move_right))
            .on_action(cx.listener(Self::on_move_up))
            .on_action(cx.listener(Self::on_move_down))
            .on_action(cx.listener(Self::on_move_previous_word))
            .on_action(cx.listener(Self::on_move_next_word))
            .on_action(cx.listener(Self::on_move_line_start))
            .on_action(cx.listener(Self::on_move_line_end))
            .on_action(cx.listener(Self::on_move_start))
            .on_action(cx.listener(Self::on_move_end))
            .on_action(cx.listener(Self::on_select_left))
            .on_action(cx.listener(Self::on_select_right))
            .on_action(cx.listener(Self::on_select_up))
            .on_action(cx.listener(Self::on_select_down))
            .on_action(cx.listener(Self::on_select_previous_word))
            .on_action(cx.listener(Self::on_select_next_word))
            .on_action(cx.listener(Self::on_select_line_start))
            .on_action(cx.listener(Self::on_select_line_end))
            .on_action(cx.listener(Self::on_select_start))
            .on_action(cx.listener(Self::on_select_end))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_newline))
            .on_action(cx.listener(Self::on_submit))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_redo))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_paste))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .w_full()
            .min_w_0()
            .text_base()
            .child(ComposerEditorElement::new(cx.entity()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ai_chat_core::SkillSourceKind;
    use gpui::{AppContext as _, TestAppContext, VisualTestContext};

    use super::*;

    fn editor_with_skills(
        names: &[&str],
        window: &mut Window,
        cx: &mut Context<ComposerEditor>,
    ) -> ComposerEditor {
        let mut editor = ComposerEditor::new("placeholder", window, cx);
        editor.skills = names
            .iter()
            .map(|name| {
                (
                    (*name).to_string(),
                    ComposerSkill {
                        name: (*name).to_string(),
                        source_kind: SkillSourceKind::User,
                        skill_file_path: format!("/skills/{name}/SKILL.md"),
                        directory_path: format!("/skills/{name}"),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        editor
    }

    fn init_test_app(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
    }

    #[gpui::test]
    fn snapshot_contains_text_and_skill_requests(cx: &mut TestAppContext) {
        init_test_app(cx);

        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&["rust"], window, cx);
                        editor.replace_range(0..0, "use $rust", true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        let snapshot = editor.read_with(&cx, |editor, _| editor.snapshot());
        assert_eq!(snapshot.text, "use $rust");
        assert_eq!(snapshot.content_parts.len(), 1);
        assert_eq!(
            snapshot.skill_requests,
            vec![ai_chat_agent::SkillActivationRequest::new("rust")]
        );
        assert_eq!(snapshot.token_ranges[0].range, 4..9);
    }

    #[gpui::test]
    fn undo_redo_restores_text_selection_and_tokens(cx: &mut TestAppContext) {
        init_test_app(cx);

        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&["rust"], window, cx);
                        editor.replace_range(0..0, "$rust", true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.on_undo(&ComposerUndo, window, cx);
                assert_eq!(editor.text, "");
                assert!(editor.tokens.is_empty());
                editor.on_redo(&ComposerRedo, window, cx);
                assert_eq!(editor.text, "$rust");
                assert_eq!(editor.tokens.len(), 1);
            });
        });
    }

    #[gpui::test]
    fn ime_marked_text_replaces_as_single_undo_group(cx: &mut TestAppContext) {
        init_test_app(cx);

        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| editor_with_skills(&[], window, cx))
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
                editor.replace_and_mark_text_in_range(Some(0..2), "你", Some(1..1), window, cx);
                assert_eq!(editor.marked_text_range(window, cx), Some(0..1));
                editor.unmark_text(window, cx);
                editor.on_undo(&ComposerUndo, window, cx);
                assert_eq!(editor.text, "");
            });
        });
    }

    #[test]
    fn token_is_removed_when_text_no_longer_matches_catalog() {
        let mut next_id = 1;
        let mut skills = BTreeMap::new();
        skills.insert(
            "rust".to_string(),
            ComposerSkill {
                name: "rust".to_string(),
                source_kind: SkillSourceKind::User,
                skill_file_path: "/skills/rust/SKILL.md".to_string(),
                directory_path: "/skills/rust".to_string(),
            },
        );

        assert_eq!(parse_skill_tokens("$rust", &skills, &mut next_id).len(), 1);
        assert_eq!(parse_skill_tokens("$ru", &skills, &mut next_id).len(), 0);
    }
}
