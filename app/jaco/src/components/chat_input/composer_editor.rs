mod blink_cursor;
mod buffer;
mod completion;
mod element;
mod history;
mod skill_detail;
mod snapshot;
mod token;

#[cfg(test)]
pub(crate) use snapshot::ComposerSendPolicy;
pub(crate) use snapshot::ComposerSnapshot;

use crate::{
    foundation::I18n,
    state::{attachments::clipboard_item_has_attachments, skills::GlobalSkillEntry},
};

use std::{collections::BTreeMap, ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, CursorStyle, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Render, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    UTF16Selection, Window, actions, div, point, px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    list::{List, ListState},
    scroll::ScrollableElement as _,
};

use self::{
    blink_cursor::BlinkCursor,
    buffer::{
        Selection, byte_range_to_utf16_range, byte_to_utf16, clamp_offset, line_start,
        next_grapheme_boundary, next_word_end, offset_for_line_column, previous_grapheme_boundary,
        previous_word_start, utf16_range_to_byte_range, word_range_at,
    },
    completion::{
        SkillCompletionDelegate, SkillCompletionRow, SkillCompletionTrigger, skill_completion_rows,
        skill_completion_trigger,
    },
    element::ComposerEditorElement,
    history::{EditorHistory, EditorState},
    skill_detail::open_skill_detail_dialog,
    snapshot::build_snapshot,
    token::{
        ComposerSkill, ComposerToken, expand_range_to_token_boundaries, nearest_token_boundary,
        parse_skill_tokens, skills_from_entries, token_after_offset, token_at_offset,
        token_before_offset,
    },
};

pub(super) const KEY_CONTEXT: &str = "JacoComposerEditor";

actions!(
    jaco_composer_editor,
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
        ComposerCancelCompletion,
        ComposerConfirmCompletion,
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
        KeyBinding::new("escape", ComposerCancelCompletion, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", ComposerConfirmCompletion, Some(KEY_CONTEXT)),
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
    PasteAttachmentRequested(ClipboardItem),
    SubmitRequested(ComposerSnapshot),
}

impl EventEmitter<ComposerEditorEvent> for ComposerEditor {}

pub(crate) struct ComposerEditor {
    disabled: bool,
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
    preferred_x: Option<Pixels>,
    selecting: bool,
    scroll_cursor_into_view: bool,
    focus_handle: FocusHandle,
    blink_cursor: gpui::Entity<BlinkCursor>,
    scroll_handle: ScrollHandle,
    last_layout: Option<element::LayoutCache>,
    completion_list: gpui::Entity<ListState<SkillCompletionDelegate>>,
    completion_trigger: Option<SkillCompletionTrigger>,
    completion_needs_selection_sync: bool,
    _subscriptions: Vec<Subscription>,
}

impl ComposerEditor {
    pub(crate) const MIN_VISIBLE_LINES: usize = 2;
    pub(crate) const MAX_VISIBLE_LINES: usize = 8;

    pub(crate) fn new(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let blink_cursor = cx.new(|_| BlinkCursor::new());
        let state = cx.entity().downgrade();
        let completion_empty_label = if cx.has_global::<I18n>() {
            cx.global::<I18n>().t("chat-form-skill-completion-empty")
        } else {
            "No matching skills".to_string()
        };
        let completion_confirm = Rc::new({
            let state = state.clone();
            move |row: SkillCompletionRow, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |editor, cx| {
                    editor.confirm_skill_completion(row, window, cx);
                });
            }
        });
        let completion_cancel = Rc::new({
            let state = state.clone();
            move |_window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |editor, cx| {
                    editor.close_skill_completion(cx);
                });
            }
        });
        let completion_list = cx.new(|cx| {
            ListState::new(
                SkillCompletionDelegate::new(
                    Vec::new(),
                    completion_empty_label.into(),
                    completion_confirm,
                    completion_cancel,
                ),
                window,
                cx,
            )
        });
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
                editor.close_skill_completion(cx);
                cx.notify();
            }),
        ];

        Self {
            disabled: false,
            text: String::new(),
            placeholder: placeholder.into(),
            selection: Selection::default(),
            marked_range: None,
            tokens: Vec::new(),
            skills: BTreeMap::new(),
            next_token_id: 1,
            history: EditorHistory::default(),
            composition_base: None,
            preferred_column: None,
            preferred_x: None,
            selecting: false,
            scroll_cursor_into_view: false,
            focus_handle,
            blink_cursor,
            scroll_handle: ScrollHandle::new(),
            last_layout: None,
            completion_list,
            completion_trigger: None,
            completion_needs_selection_sync: false,
            _subscriptions,
        }
    }

    pub(crate) fn snapshot(&self) -> ComposerSnapshot {
        build_snapshot(&self.text, &self.tokens)
    }

    pub(crate) fn can_submit(&self) -> bool {
        !self.snapshot().is_empty()
    }

    pub(crate) fn clear(&mut self, cx: &mut Context<Self>) {
        if self.text.is_empty() && self.tokens.is_empty() {
            return;
        }
        self.record_before_change();
        self.restore_state(
            EditorState {
                text: String::new(),
                selection: Selection::default(),
                marked_range: None,
                tokens: Vec::new(),
            },
            cx,
        );
    }

    pub(crate) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.focus_handle.focus(window, cx);
    }

    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        if disabled {
            self.close_skill_completion(cx);
        }
        cx.notify();
    }

    pub(crate) fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub(crate) fn set_skill_entries(
        &mut self,
        entries: &[GlobalSkillEntry],
        cx: &mut Context<Self>,
    ) {
        self.skills = skills_from_entries(entries);
        self.refresh_tokens();
        if cx.has_global::<I18n>() {
            let rows = skill_completion_rows(entries, cx.global::<I18n>());
            self.completion_list.update(cx, |list, cx| {
                list.delegate_mut().set_rows(rows);
                cx.notify();
            });
        }
        self.completion_needs_selection_sync = true;
        self.refresh_skill_completion(cx);
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

    pub(super) fn display_visual_line_count(&self) -> usize {
        self.last_layout
            .as_ref()
            .map(|layout| layout.visual_line_count)
            .unwrap_or_else(|| self.display_line_count())
            .max(1)
    }

    pub(super) fn content_line_count(&self) -> usize {
        self.display_visual_line_count()
            .max(Self::MIN_VISIBLE_LINES)
    }

    pub(super) fn scroll_offset(&self) -> Point<Pixels> {
        self.scroll_handle.offset()
    }

    fn visible_line_count(&self) -> usize {
        self.display_visual_line_count()
            .clamp(Self::MIN_VISIBLE_LINES, Self::MAX_VISIBLE_LINES)
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
        self.preferred_x = None;
        self.last_layout = None;
        self.refresh_skill_completion(cx);
        self.request_cursor_visible(cx);
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

    fn invalidate_layout(&mut self) {
        self.last_layout = None;
    }

    fn set_layout(&mut self, layout: element::LayoutCache, cx: &mut Context<Self>) {
        let old_visible_lines = self.visible_line_count();
        let old_visual_lines = self.display_visual_line_count();
        let new_visual_lines = layout.visual_line_count;
        let should_ensure_cursor_visible = self.scroll_cursor_into_view
            || (Self::layout_overflows(&layout) && old_visual_lines != new_visual_lines);
        self.last_layout = Some(layout);
        if should_ensure_cursor_visible {
            self.scroll_cursor_into_view = true;
            cx.notify();
        }
        if old_visible_lines != self.visible_line_count()
            || old_visual_lines != self.display_visual_line_count()
        {
            cx.notify();
        }
    }

    fn clamp_scroll_offset(&mut self, cx: &mut Context<Self>) {
        let Some(layout) = &self.last_layout else {
            return;
        };
        let viewport = layout.viewport_bounds;
        if viewport.size.height <= px(0.) {
            return;
        }

        if !Self::layout_overflows(layout) {
            self.scroll_cursor_into_view = false;
            let old_offset = self.scroll_handle.offset();
            let new_offset = point(px(0.), px(0.));
            if old_offset != point(px(0.), px(0.)) {
                self.scroll_handle.set_offset(new_offset);
                cx.notify();
            }
            return;
        }

        let min_y = (viewport.size.height - layout.content_height()).min(px(0.));
        let old_offset = self.scroll_handle.offset();
        let new_offset = point(px(0.), old_offset.y.max(min_y).min(px(0.)));
        if new_offset != old_offset {
            self.scroll_handle.set_offset(new_offset);
            cx.notify();
        }
    }

    fn request_cursor_visible(&mut self, cx: &mut Context<Self>) {
        self.scroll_cursor_into_view = true;
        self.ensure_cursor_visible(cx);
    }

    fn ensure_cursor_visible(&mut self, cx: &mut Context<Self>) {
        let Some(layout) = &self.last_layout else {
            return;
        };
        if !Self::layout_overflows(layout) {
            self.scroll_cursor_into_view = false;
            let old_offset = self.scroll_handle.offset();
            let new_offset = point(px(0.), px(0.));
            if old_offset != new_offset {
                self.scroll_handle.set_offset(new_offset);
                cx.notify();
            }
            return;
        }
        let Some(cursor) = layout.bounds_for_offset(&self.text, self.cursor()) else {
            return;
        };
        let viewport = layout.viewport_bounds;
        if viewport.size.height <= px(0.) {
            return;
        }
        let expected_max_y = (layout.content_height() - viewport.size.height).max(px(0.));
        let scroll_range_ready = self.scroll_handle.max_offset().y + px(1.) >= expected_max_y;

        let old_offset = self.scroll_handle.offset();
        let mut next_offset = old_offset;
        if cursor.top() < viewport.top() {
            next_offset.y += viewport.top() - cursor.top();
        } else if cursor.bottom() > viewport.bottom() {
            next_offset.y -= cursor.bottom() - viewport.bottom();
        }

        let min_y = (viewport.size.height - layout.content_height()).min(px(0.));
        next_offset = point(px(0.), next_offset.y.max(min_y).min(px(0.)));
        let offset_changed = next_offset != old_offset;
        if offset_changed {
            self.scroll_handle.set_offset(next_offset);
            cx.notify();
        }
        let cursor_visible = cursor.top() >= viewport.top() - px(1.)
            && cursor.bottom() <= viewport.bottom() + px(1.);
        if scroll_range_ready && !offset_changed && cursor_visible {
            self.scroll_cursor_into_view = false;
        }
    }

    fn apply_scroll_before_render(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.last_layout.is_some() {
            self.clamp_scroll_offset(cx);
            if self.scroll_cursor_into_view {
                self.ensure_cursor_visible(cx);
            }
            return;
        }

        if !self.scroll_cursor_into_view {
            return;
        }

        let visual_lines = self.display_line_count();
        let visible_lines = visual_lines.clamp(Self::MIN_VISIBLE_LINES, Self::MAX_VISIBLE_LINES);
        let viewport_height = window.line_height() * visible_lines as f32;
        let content_height = window.line_height() * visual_lines.max(1) as f32;
        let old_offset = self.scroll_handle.offset();
        let next_offset = if visual_lines > Self::MAX_VISIBLE_LINES {
            point(px(0.), (viewport_height - content_height).min(px(0.)))
        } else {
            point(px(0.), px(0.))
        };

        if next_offset != old_offset {
            self.scroll_handle.set_offset(next_offset);
            cx.notify();
        }
    }

    fn layout_overflows(layout: &element::LayoutCache) -> bool {
        layout.visual_line_count > Self::MAX_VISIBLE_LINES
    }

    fn refresh_tokens(&mut self) {
        self.tokens = parse_skill_tokens(&self.text, &self.skills, &mut self.next_token_id);
    }

    pub(crate) fn skill_completion_open(&self) -> bool {
        self.completion_trigger.is_some()
    }

    fn close_skill_completion(&mut self, cx: &mut Context<Self>) {
        if self.completion_trigger.take().is_none() && !self.completion_needs_selection_sync {
            return;
        }

        self.completion_needs_selection_sync = false;
        cx.notify();
    }

    fn refresh_skill_completion(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            self.close_skill_completion(cx);
            return;
        }

        let trigger = self
            .selection
            .is_empty()
            .then(|| {
                skill_completion_trigger(&self.text, self.cursor(), self.marked_range.as_ref())
            })
            .flatten();
        if self.completion_trigger == trigger {
            return;
        }

        let query = trigger
            .as_ref()
            .map(|trigger| trigger.query.clone())
            .unwrap_or_default();
        self.completion_trigger = trigger;
        self.completion_needs_selection_sync = self.completion_trigger.is_some();
        self.completion_list.update(cx, |list, cx| {
            list.delegate_mut().set_query(query);
            cx.notify();
        });
        cx.notify();
    }

    fn sync_completion_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.completion_needs_selection_sync {
            return;
        }

        let open = self.skill_completion_open();
        self.completion_list.update(cx, |list, cx| {
            let selected = open.then(|| list.delegate().first_index()).flatten();
            list.set_selected_index(selected, window, cx);
            if let Some(ix) = selected {
                list.scroll_to_item(ix, gpui::ScrollStrategy::Top, window, cx);
            }
        });
        self.completion_needs_selection_sync = false;
    }

    fn move_completion_selection(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_completion_selection(window, cx);
        self.completion_list.update(cx, |list, cx| {
            let count = list.delegate().item_count();
            if count == 0 {
                list.set_selected_index(None, window, cx);
                return;
            }

            let current = list.selected_index().map(|ix| ix.row).unwrap_or(0);
            let next = if delta < 0 {
                if current == 0 { count - 1 } else { current - 1 }
            } else if current + 1 >= count {
                0
            } else {
                current + 1
            };
            let ix = gpui_component::IndexPath::default().row(next);
            list.set_selected_index(Some(ix), window, cx);
            list.scroll_to_item(ix, gpui::ScrollStrategy::Top, window, cx);
        });
        cx.notify();
    }

    fn confirm_completion_if_open(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.disabled || !self.skill_completion_open() {
            return false;
        }

        self.sync_completion_selection(window, cx);
        let row = self
            .completion_list
            .read_with(cx, |list, _| list.delegate().selected_row());
        if let Some(row) = row {
            self.confirm_skill_completion(row, window, cx);
            true
        } else {
            false
        }
    }

    fn confirm_skill_completion(
        &mut self,
        row: SkillCompletionRow,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(trigger) = self.completion_trigger.clone() else {
            return;
        };
        let mut replacement = format!("${}", row.skill.name);
        if self.text[trigger.range.end..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_whitespace())
        {
            replacement.push(' ');
        }
        self.replace_range(trigger.range, &replacement, true, cx);
        self.close_skill_completion(cx);
    }

    pub(crate) fn render_skill_completion_panel(
        &mut self,
        max_height: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_completion_selection(window, cx);

        div()
            .id("jaco-skill-completion")
            .w_full()
            .max_h(max_height)
            .occlude()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.popover.background)
            .shadow_lg()
            .child(
                List::new(&self.completion_list)
                    .with_size(gpui_component::Size::Small)
                    .scrollbar_visible(true)
                    .max_h(max_height)
                    .p_1(),
            )
            .into_any_element()
    }

    fn expand_edit_range(&self, range: Range<usize>) -> Range<usize> {
        let range = buffer::normalize_range(&self.text, range);
        expand_range_to_token_boundaries(&self.tokens, range)
    }

    fn replace_range(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        record_history: bool,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.pause_cursor_blink(cx);
        if record_history {
            self.record_before_change();
        }

        let range = self.expand_edit_range(range);
        self.text.replace_range(range.clone(), new_text);
        let cursor = range.start + new_text.len();
        self.selection.collapse(clamp_offset(&self.text, cursor));
        self.marked_range = None;
        self.preferred_column = None;
        self.preferred_x = None;
        self.invalidate_layout();
        self.refresh_tokens();
        self.refresh_skill_completion(cx);
        self.request_cursor_visible(cx);
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
        let offset = buffer::clamp_grapheme_offset(&self.text, offset);
        let offset = if selecting {
            offset
        } else {
            nearest_token_boundary(&self.tokens, offset).unwrap_or(offset)
        };
        self.selection.move_head(offset, selecting);
        self.preferred_column = None;
        self.preferred_x = None;
        self.marked_range = None;
        self.refresh_skill_completion(cx);
        self.request_cursor_visible(cx);
        cx.notify();
    }

    fn move_vertically(&mut self, delta: isize, selecting: bool, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        if let Some(layout) = &self.last_layout
            && let Some((offset, preferred_x)) =
                layout.offset_for_vertical_move(&self.text, self.cursor(), delta, self.preferred_x)
        {
            let offset = buffer::clamp_grapheme_offset(&self.text, offset);
            let offset = if selecting {
                offset
            } else {
                nearest_token_boundary(&self.tokens, offset).unwrap_or(offset)
            };
            self.selection.move_head(offset, selecting);
            self.preferred_column = None;
            self.preferred_x = Some(preferred_x);
            self.marked_range = None;
            self.refresh_skill_completion(cx);
            self.request_cursor_visible(cx);
            cx.notify();
            return;
        }

        let (line_ix, column) = buffer::line_column(&self.text, self.cursor());
        let preferred_column = self.preferred_column.unwrap_or(column);
        let line_count = buffer::line_ranges(&self.text).len();
        let next_line_ix = if delta < 0 {
            line_ix.saturating_sub(delta.unsigned_abs())
        } else {
            (line_ix + delta as usize).min(line_count.saturating_sub(1))
        };
        let offset = offset_for_line_column(&self.text, next_line_ix, preferred_column);
        let offset = buffer::clamp_grapheme_offset(&self.text, offset);
        let offset = if selecting {
            offset
        } else {
            nearest_token_boundary(&self.tokens, offset).unwrap_or(offset)
        };
        self.selection.move_head(offset, selecting);
        self.preferred_column = Some(preferred_column);
        self.preferred_x = None;
        self.marked_range = None;
        self.refresh_skill_completion(cx);
        self.request_cursor_visible(cx);
        cx.notify();
    }

    fn selection_or_previous_char(&self) -> Option<Range<usize>> {
        if !self.selection.is_empty() {
            return Some(self.selection.range());
        }
        let cursor = self.cursor();
        if let Some(token) = token_before_offset(&self.tokens, cursor) {
            return Some(token.range.clone());
        }
        if let Some(token) = token_at_offset(&self.tokens, cursor) {
            return Some(token.range.clone());
        }
        (cursor > 0).then(|| previous_grapheme_boundary(&self.text, cursor)..cursor)
    }

    fn selection_or_next_char(&self) -> Option<Range<usize>> {
        if !self.selection.is_empty() {
            return Some(self.selection.range());
        }
        let cursor = self.cursor();
        if let Some(token) = token_after_offset(&self.tokens, cursor) {
            return Some(token.range.clone());
        }
        if let Some(token) = token_at_offset(&self.tokens, cursor) {
            return Some(token.range.clone());
        }
        (cursor < self.text.len()).then(|| cursor..next_grapheme_boundary(&self.text, cursor))
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
            self.replace_range(self.expand_edit_range(range), "", true, cx);
            return;
        }
        window.play_system_bell();
    }

    fn select_word_at(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        let offset = buffer::clamp_grapheme_offset(&self.text, offset);
        if self.text.is_empty() {
            self.selection.collapse(0);
            self.preferred_column = None;
            self.preferred_x = None;
            cx.notify();
            return;
        }
        if let Some(range) = word_range_at(&self.text, offset) {
            self.selection = Selection {
                anchor: range.start,
                head: range.end,
            };
        } else {
            self.selection.collapse(offset);
        }
        self.preferred_column = None;
        self.preferred_x = None;
        self.close_skill_completion(cx);
        self.request_cursor_visible(cx);
        cx.notify();
    }

    fn select_line_at(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.pause_cursor_blink(cx);
        let offset = clamp_offset(&self.text, offset);
        self.selection = Selection {
            anchor: line_start(&self.text, offset),
            head: buffer::line_end(&self.text, offset),
        };
        self.preferred_column = None;
        self.preferred_x = None;
        self.close_skill_completion(cx);
        self.request_cursor_visible(cx);
        cx.notify();
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = &self.last_layout else {
            return self.text.len();
        };
        buffer::clamp_grapheme_offset(&self.text, layout.offset_for_position(&self.text, position))
    }

    fn token_for_mouse_position(&self, position: Point<Pixels>) -> Option<ComposerToken> {
        let layout = self.last_layout.as_ref()?;
        let hit = layout.token_hit_for_position(&self.text, position)?;
        self.tokens
            .iter()
            .find(|token| token.range == hit.range)
            .cloned()
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.focus(window, cx);
        if event.click_count == 1
            && !event.modifiers.shift
            && let Some(token) = self.token_for_mouse_position(event.position)
        {
            self.selecting = false;
            open_skill_detail_dialog(token.skill, window, cx);
            return;
        }

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
        if self.disabled {
            return;
        }
        if self.selecting {
            self.move_to(self.index_for_mouse_position(event.position), true, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.selecting = false;
    }

    fn on_backspace(&mut self, _: &ComposerBackspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.delete_range_or_bell(self.selection_or_previous_char(), window, cx);
    }

    fn on_delete(&mut self, _: &ComposerDelete, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.delete_range_or_bell(self.selection_or_next_char(), window, cx);
    }

    fn on_delete_previous_word(
        &mut self,
        _: &ComposerDeletePreviousWord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
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
        if self.disabled {
            return;
        }
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
        if self.disabled {
            return;
        }
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
        if self.disabled {
            return;
        }
        let range = if self.selection.is_empty() {
            self.cursor()..buffer::line_end(&self.text, self.cursor())
        } else {
            self.selection.range()
        };
        self.delete_range_or_bell(Some(range), window, cx);
    }

    fn on_move_left(&mut self, _: &ComposerMoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selection.is_empty() {
            previous_grapheme_boundary(&self.text, self.cursor())
        } else {
            self.selection.range().start
        };
        self.move_to(offset, false, cx);
    }

    fn on_move_right(&mut self, _: &ComposerMoveRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selection.is_empty() {
            next_grapheme_boundary(&self.text, self.cursor())
        } else {
            self.selection.range().end
        };
        self.move_to(offset, false, cx);
    }

    fn on_move_up(&mut self, _: &ComposerMoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.skill_completion_open() {
            self.move_completion_selection(-1, window, cx);
            return;
        }
        self.move_vertically(-1, false, cx);
    }

    fn on_move_down(&mut self, _: &ComposerMoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.skill_completion_open() {
            self.move_completion_selection(1, window, cx);
            return;
        }
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
        self.move_to(
            previous_grapheme_boundary(&self.text, self.cursor()),
            true,
            cx,
        );
    }

    fn on_select_right(&mut self, _: &ComposerSelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(next_grapheme_boundary(&self.text, self.cursor()), true, cx);
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
        self.preferred_x = None;
        self.request_cursor_visible(cx);
        cx.notify();
    }

    fn on_newline(&mut self, _: &ComposerNewline, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.replace_selection("\n", true, cx);
    }

    fn on_submit(&mut self, _: &ComposerSubmit, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if self.confirm_completion_if_open(window, cx) {
            return;
        }

        let snapshot = self.snapshot();
        cx.emit(ComposerEditorEvent::SubmitRequested(snapshot));
    }

    fn on_cancel_completion(
        &mut self,
        _: &ComposerCancelCompletion,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.skill_completion_open() {
            self.close_skill_completion(cx);
        } else {
            cx.propagate();
        }
    }

    fn on_confirm_completion(
        &mut self,
        _: &ComposerConfirmCompletion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.confirm_completion_if_open(window, cx) {
            cx.propagate();
        }
    }

    fn on_undo(&mut self, _: &ComposerUndo, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if let Some(state) = self.history.undo(self.current_state()) {
            self.restore_state(state, cx);
            self.refresh_skill_completion(cx);
        }
    }

    fn on_redo(&mut self, _: &ComposerRedo, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if let Some(state) = self.history.redo(self.current_state()) {
            self.restore_state(state, cx);
            self.refresh_skill_completion(cx);
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
        if self.disabled {
            return;
        }
        if self.selection.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.text[self.selection.range()].to_string(),
        ));
        self.delete_range_or_bell(Some(self.selection.range()), window, cx);
    }

    fn on_paste(&mut self, _: &ComposerPaste, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if let Some(item) = cx.read_from_clipboard() {
            if clipboard_item_has_attachments(&item) {
                cx.emit(ComposerEditorEvent::PasteAttachmentRequested(item));
                return;
            }

            if let Some(text) = item.text() {
                self.replace_text_in_range(None, text.as_ref(), window, cx);
            }
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
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
        self.sync_completion_selection(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
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

        let range = self.expand_edit_range(range);
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
        self.preferred_column = None;
        self.preferred_x = None;
        // macOS queries IME candidate bounds immediately after marked-text updates.
        // Keep the previous layout available until paint replaces it with a fresh one.
        self.refresh_tokens();
        self.refresh_skill_completion(cx);
        self.request_cursor_visible(cx);
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
        let mut bounds = layout.bounds_for_offset(&self.text, byte)?;
        let layout_delta = element_bounds.origin - layout.bounds.origin;
        bounds.origin += layout_delta;
        Some(bounds)
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_scroll_before_render(window, cx);
        let height = window.line_height() * self.visible_line_count() as f32;
        let scroll_handle = self.scroll_handle.clone();

        gpui::div()
            .id("jaco-composer-editor")
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
            .on_action(cx.listener(Self::on_cancel_completion))
            .on_action(cx.listener(Self::on_confirm_completion))
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
            .h(height)
            .min_w_0()
            .relative()
            .text_base()
            .child(
                gpui::div()
                    .id("jaco-composer-scroll-area")
                    .size_full()
                    .track_scroll(&scroll_handle)
                    .overflow_y_scroll()
                    .child(ComposerEditorElement::new(cx.entity())),
            )
            .vertical_scrollbar(&scroll_handle)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use crate::state::skills::GlobalSkillEntry;
    use gpui::{AppContext as _, TestAppContext, VisualTestContext, px, size};
    use jaco_core::SkillSourceKind;

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
                        description: Some("Rust skill".to_string()),
                        source_kind: SkillSourceKind::User,
                        skill_file_path: format!("/skills/{name}/SKILL.md"),
                        directory_path: format!("/skills/{name}"),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        editor
    }

    fn skill_entry(name: &str) -> GlobalSkillEntry {
        GlobalSkillEntry {
            name: name.to_string(),
            description: Some("Rust skill".to_string()),
            source_kind: SkillSourceKind::User,
            skill_file_path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
            directory_path: PathBuf::from(format!("/skills/{name}")),
            search_text: format!("{name} Rust skill /skills/{name}/SKILL.md"),
        }
    }

    fn init_test_app(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::foundation::i18n::init(cx);
        });
    }

    fn resize_editor(cx: &mut VisualTestContext, width: f32, height: f32) {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
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
            vec![jaco_agent::SkillActivationRequest::new("rust")]
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
    fn confirming_skill_completion_appends_space(cx: &mut TestAppContext) {
        init_test_app(cx);

        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = ComposerEditor::new("placeholder", window, cx);
                        editor.set_skill_entries(&[skill_entry("rust")], cx);
                        editor.replace_range(0..0, "$ru", true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                assert!(editor.skill_completion_open());
                editor.on_confirm_completion(&ComposerConfirmCompletion, window, cx);
                assert_eq!(editor.text, "$rust ");
                assert_eq!(editor.cursor(), "$rust ".len());
                assert_eq!(editor.tokens.len(), 1);
                assert_eq!(editor.tokens[0].range, 0.."$rust".len());
            });
        });
    }

    #[gpui::test]
    fn confirming_skill_completion_does_not_duplicate_existing_space(cx: &mut TestAppContext) {
        init_test_app(cx);

        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = ComposerEditor::new("placeholder", window, cx);
                        editor.set_skill_entries(&[skill_entry("rust")], cx);
                        editor.replace_range(0..0, "$ru next", true, cx);
                        editor.selection.collapse("$ru".len());
                        editor.refresh_skill_completion(cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                assert!(editor.skill_completion_open());
                editor.on_confirm_completion(&ComposerConfirmCompletion, window, cx);
                assert_eq!(editor.text, "$rust next");
                assert_eq!(editor.cursor(), "$rust".len());
                assert_eq!(editor.tokens.len(), 1);
                assert_eq!(editor.tokens[0].range, 0.."$rust".len());
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

    #[gpui::test]
    fn ime_marked_text_keeps_candidate_bounds_available(cx: &mut TestAppContext) {
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

        resize_editor(&mut cx, 320., 500.);
        let element_bounds =
            editor.read_with(&cx, |editor, _| editor.last_layout.as_ref().unwrap().bounds);

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);

                let bounds = editor
                    .bounds_for_range(2..2, element_bounds, window, cx)
                    .expect("IME candidate bounds should stay available during composition");

                assert!(bounds.left() >= element_bounds.left());
                assert!(bounds.top() >= element_bounds.top());
            });
        });
    }

    #[gpui::test]
    fn grapheme_actions_do_not_split_clusters(cx: &mut TestAppContext) {
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
                let coder = "👩🏽‍💻";
                editor.replace_range(0..0, coder, true, cx);
                assert_eq!(editor.cursor(), coder.len());

                editor.on_select_left(&ComposerSelectLeft, window, cx);
                assert_eq!(editor.selection.range(), 0..coder.len());
                editor.on_backspace(&ComposerBackspace, window, cx);
                assert_eq!(editor.text, "");

                editor.replace_range(0..0, coder, true, cx);
                editor.selection.collapse(0);
                editor.on_delete(&ComposerDelete, window, cx);
                assert_eq!(editor.text, "");

                let acute = "e\u{301}";
                editor.replace_range(0..0, acute, true, cx);
                editor.on_move_left(&ComposerMoveLeft, window, cx);
                assert_eq!(editor.cursor(), 0);
                editor.on_move_right(&ComposerMoveRight, window, cx);
                assert_eq!(editor.cursor(), acute.len());
            });
        });
    }

    #[gpui::test]
    fn word_actions_select_skill_token_and_unicode_words(cx: &mut TestAppContext) {
        init_test_app(cx);

        let text = "ask $rust-skill 中文";
        let skill_start = text.find('$').unwrap();
        let skill_end = skill_start + "$rust-skill".len();
        let chinese_start = text.find("中文").unwrap();
        let chinese_end = chinese_start + "中文".len();
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&["rust-skill"], window, cx);
                        editor.replace_range(0..0, text, true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                assert_eq!(editor.tokens.len(), 1);

                editor.select_word_at(skill_start + 1, cx);
                assert_eq!(editor.selection.range(), skill_start..skill_end);
                assert_eq!(
                    editor.snapshot().skill_requests,
                    vec![jaco_agent::SkillActivationRequest::new("rust-skill")]
                );

                editor.select_word_at("ask".len(), cx);
                assert!(editor.selection.is_empty());
                assert_eq!(editor.cursor(), "ask".len());

                editor.selection.collapse(skill_end);
                editor.on_move_previous_word(&ComposerMovePreviousWord, window, cx);
                assert_eq!(editor.cursor(), skill_start);
                editor.on_move_next_word(&ComposerMoveNextWord, window, cx);
                assert_eq!(editor.cursor(), skill_end);

                editor.selection.collapse(chinese_end);
                editor.on_delete_previous_word(&ComposerDeletePreviousWord, window, cx);
                assert_eq!(editor.text, "ask $rust-skill ");
                assert_eq!(editor.tokens.len(), 1);
                assert_eq!(editor.tokens[0].range, skill_start..skill_end);

                editor.selection.collapse(skill_start);
                editor.on_delete_next_word(&ComposerDeleteNextWord, window, cx);
                assert_eq!(editor.text, "ask  ");
                assert!(editor.tokens.is_empty());
            });
        });
    }

    #[gpui::test]
    fn skill_token_edits_are_atomic(cx: &mut TestAppContext) {
        init_test_app(cx);

        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&["rust"], window, cx);
                        editor.replace_range(0..0, "$rust next", true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.selection.collapse("$rust".len());
                editor.on_backspace(&ComposerBackspace, window, cx);
                assert_eq!(editor.text, " next");
                assert!(editor.tokens.is_empty());

                editor.clear(cx);
                editor.replace_range(0..0, "$rust next", true, cx);
                editor.selection.collapse(0);
                editor.on_delete(&ComposerDelete, window, cx);
                assert_eq!(editor.text, " next");
                assert!(editor.tokens.is_empty());

                editor.clear(cx);
                editor.replace_range(0..0, "$rust next", true, cx);
                editor.selection.collapse(2);
                editor.replace_text_in_range(None, "go", window, cx);
                assert_eq!(editor.text, "go next");
                assert!(editor.tokens.is_empty());
            });
        });
    }

    #[gpui::test]
    fn soft_wrap_layout_maps_points_to_utf8_offsets(cx: &mut TestAppContext) {
        init_test_app(cx);

        let text = "hello 中文🙂 hello 中文🙂 hello 中文🙂".to_string();
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&[], window, cx);
                        editor.replace_range(0..0, &text, true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        resize_editor(&mut cx, 80., 400.);

        editor.read_with(&cx, |editor, _| {
            let layout = editor.last_layout.as_ref().unwrap();
            assert!(
                layout.visual_line_count > editor.display_line_count(),
                "visual={}, hard={}, bounds={:?}, line_width={:?}",
                layout.visual_line_count,
                editor.display_line_count(),
                layout.bounds,
                layout
                    .lines
                    .first()
                    .and_then(|line| line.first_visual_width())
            );
            assert_eq!(
                layout.offset_for_position(&editor.text, layout.bounds.origin),
                0
            );
            let end_bounds = layout
                .bounds_for_offset(&editor.text, editor.text.len())
                .unwrap();
            assert_eq!(
                layout.offset_for_position(&editor.text, end_bounds.origin),
                editor.text.len()
            );
        });
    }

    #[gpui::test]
    fn soft_wrapped_composer_scrolls_cursor_into_view(cx: &mut TestAppContext) {
        init_test_app(cx);

        let text = "hello 中文🙂 ".repeat(48);
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&[], window, cx);
                        editor.replace_range(0..0, &text, true, cx);
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let editor = window.root(&mut cx).unwrap();

        resize_editor(&mut cx, 96., 500.);
        resize_editor(&mut cx, 96., 500.);
        resize_editor(&mut cx, 96., 500.);

        editor.read_with(&cx, |editor, _| {
            let layout = editor.last_layout.as_ref().unwrap();
            let viewport = editor.scroll_handle.bounds();
            let cursor = layout
                .bounds_for_offset(&editor.text, editor.cursor())
                .unwrap();

            assert!(
                layout.visual_line_count > ComposerEditor::MAX_VISIBLE_LINES,
                "visual={}, bounds={:?}, line_width={:?}",
                layout.visual_line_count,
                layout.bounds,
                layout.lines.first().and_then(|line| line.first_visual_width())
            );
            assert_eq!(
                editor.visible_line_count(),
                ComposerEditor::MAX_VISIBLE_LINES
            );
            assert!(editor.scroll_handle.max_offset().y > px(0.));
            assert!(
                editor.scroll_handle.offset().y < px(0.),
                "offset={:?}, max_offset={:?}, cursor={cursor:?}, viewport={viewport:?}, layout_viewport={:?}, scroll_pending={}",
                editor.scroll_handle.offset(),
                editor.scroll_handle.max_offset(),
                layout.viewport_bounds,
                editor.scroll_cursor_into_view,
            );
            assert!(
                cursor.top() >= viewport.top() - px(1.),
                "cursor={cursor:?}, viewport={viewport:?}, offset={:?}",
                editor.scroll_handle.offset()
            );
            assert!(
                cursor.bottom() <= viewport.bottom() + px(1.),
                "cursor={cursor:?}, viewport={viewport:?}, offset={:?}",
                editor.scroll_handle.offset()
            );
        });
    }

    #[gpui::test]
    fn repeated_newlines_do_not_scroll_before_overflow(cx: &mut TestAppContext) {
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

        resize_editor(&mut cx, 320., 500.);

        for expected_lines in 2..=ComposerEditor::MAX_VISIBLE_LINES {
            cx.update(|_, cx| {
                editor.update(cx, |editor, cx| {
                    let cursor = editor.cursor();
                    editor.replace_range(cursor..cursor, "\n", true, cx);
                });
            });
            resize_editor(&mut cx, 320., 500.);

            editor.read_with(&cx, |editor, _| {
                assert_eq!(editor.display_visual_line_count(), expected_lines);
                assert_eq!(editor.visible_line_count(), expected_lines);
                assert_eq!(editor.scroll_handle.offset().y, px(0.));
            });
        }

        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                let cursor = editor.cursor();
                editor.replace_range(cursor..cursor, "\n", true, cx);
            });
        });
        resize_editor(&mut cx, 320., 500.);
        resize_editor(&mut cx, 320., 500.);

        editor.read_with(&cx, |editor, _| {
            let layout = editor.last_layout.as_ref().unwrap();
            let cursor = layout
                .bounds_for_offset(&editor.text, editor.cursor())
                .unwrap();
            let viewport = layout.viewport_bounds;

            assert_eq!(
                editor.visible_line_count(),
                ComposerEditor::MAX_VISIBLE_LINES
            );
            assert!(editor.scroll_handle.offset().y < px(0.));
            assert!(cursor.bottom() <= viewport.bottom() + px(1.));
        });
    }

    #[gpui::test]
    fn soft_wrap_selection_paint_does_not_panic(cx: &mut TestAppContext) {
        init_test_app(cx);

        let text = "hello 中文🙂 ".repeat(12);
        let window = cx
            .update(|cx| {
                cx.open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut editor = editor_with_skills(&[], window, cx);
                        editor.replace_range(0..0, &text, true, cx);
                        editor.selection = Selection {
                            anchor: 0,
                            head: editor.text.len(),
                        };
                        editor
                    })
                })
            })
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        resize_editor(&mut cx, 96., 400.);
    }

    #[test]
    fn token_is_removed_when_text_no_longer_matches_catalog() {
        let mut next_id = 1;
        let mut skills = BTreeMap::new();
        skills.insert(
            "rust".to_string(),
            ComposerSkill {
                name: "rust".to_string(),
                description: Some("Rust skill".to_string()),
                source_kind: SkillSourceKind::User,
                skill_file_path: "/skills/rust/SKILL.md".to_string(),
                directory_path: "/skills/rust".to_string(),
            },
        );

        assert_eq!(parse_skill_tokens("$rust", &skills, &mut next_id).len(), 1);
        assert_eq!(parse_skill_tokens("$ru", &skills, &mut next_id).len(), 0);
    }
}
