//! A disposable window view over the application-owned temporary sessions.
mod actions_panel;
use super::home::{
    HomeView,
    actions::{Kind, Run},
    navigation,
};
use crate::{
    app::{menus, temporary},
    foundation::{assets::IconName, i18n::t},
    state::conversation::{Activity, ConversationState},
};
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Icon, IndexPath, Root, Selectable, Sizable, WindowExt as _,
        button::{Button, ButtonVariants},
        h_flex,
        input::{self, Input, InputEvent, InputState, MoveDown, MoveUp},
        kbd::Kbd,
        list::{List, ListDelegate, ListState},
        menu::ContextMenuExt,
        popover::Popover,
        resizable::{h_resizable, resizable_panel},
        v_flex,
    },
    *,
};

use gpui_kit::prelude::FluentBuilder as _;

actions!(gupi_temporary, [FocusSearch, ToggleInputFocus]);
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-f", FocusSearch, Some("GupiTemporary")),
        KeyBinding::new("tab", ToggleInputFocus, Some("GupiTemporary")),
        KeyBinding::new("tab", ToggleInputFocus, Some("GupiTemporary > Input")),
    ]);
}

pub(crate) struct TemporaryView {
    state: Entity<ConversationState>,
    home: Entity<HomeView>,
    search: Entity<InputState>,
    list: Entity<ListState<Sessions>>,
    focus: FocusHandle,
    panel: Option<Entity<actions_panel::ActionsPanel>>,
    // A revision-keyed availability bit, never a second copy of the transcript.
    answer_available: Option<(String, u64, bool)>,
    _subscriptions: Vec<Subscription>,
}
impl TemporaryView {
    pub(crate) fn new(
        state: Entity<ConversationState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let home = cx.new(|cx| HomeView::with_state(state.clone(), window, cx));
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(cx, "temporary-search-placeholder"))
        });
        let list = cx.new(|cx| {
            ListState::new(
                Sessions {
                    rows: vec![],
                    selected: None,
                    state: state.clone(),
                    home: home.downgrade(),
                },
                window,
                cx,
            )
        });
        let composer = home.read(cx).input.clone();
        let subscriptions = vec![
            cx.observe(&composer, |_, _, cx| cx.notify()),
            cx.observe_in(&state, window, |this, _, window, cx| {
                this.refresh(window, cx)
            }),
            cx.subscribe_in(&search, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh(window, cx);
                } else if let InputEvent::PressEnter {
                    secondary,
                    shift: false,
                } = event
                {
                    if this.search.update(cx, |input, cx| {
                        input.marked_text_range(window, cx).is_some()
                    }) {
                        return;
                    }
                    if !secondary && crate::state::keybindings::uses_enter(Kind::PasteAnswer, cx) {
                        this.run(&Run(Kind::PasteAnswer), window, cx);
                    }
                }
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() {
                    this.focus_search(window, cx);
                } else {
                    this.panel = None;
                    temporary::on_deactivate(window, cx);
                }
            }),
        ];
        let mut this = Self {
            state,
            home,
            search,
            list,
            focus: cx.focus_handle(),
            panel: None,
            answer_available: None,
            _subscriptions: subscriptions,
        };
        this.refresh(window, cx);
        this
    }
    pub(crate) fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let state = self.state.read(cx);
        if let Some((key, session)) = state
            .selected
            .as_ref()
            .and_then(|key| state.sessions.get(key).map(|s| (key, s)))
        {
            if session.busy() || session.interrupted || !session.pending_ui.is_empty() {
                self.answer_available = None;
            } else if self
                .answer_available
                .as_ref()
                .is_none_or(|(k, revision, _)| k != key || *revision != session.content_revision)
            {
                self.answer_available = Some((
                    key.clone(),
                    session.content_revision,
                    session.completed_answer().is_some(),
                ));
            }
        } else {
            self.answer_available = None;
        }
        let rows: Vec<_> = state
            .infos()
            .into_iter()
            .filter_map(|(key, info)| {
                let title = navigation::display_title(&info, cx);
                if !matches_query(&title, &query) {
                    return None;
                }
                let activity = state
                    .sessions
                    .get(&key)
                    .map(|s| s.activity())
                    .unwrap_or(Activity::Idle);
                Some(Row {
                    key,
                    title,
                    activity,
                })
            })
            .collect();
        let selected = rows
            .iter()
            .position(|r| Some(&r.key) == state.selected.as_ref())
            .map(|row| IndexPath::default().row(row));
        self.list.update(cx, |list, cx| {
            list.delegate_mut().rows = rows;
            list.set_selected_index(selected, window, cx);
            cx.notify();
        });
        if let Some(panel) = &self.panel {
            panel.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }
    fn close_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(panel) = self.panel.take()
            && let Some(focus) = panel.read(cx).original_focus.clone()
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn has_answer(&self) -> bool {
        self.answer_available
            .as_ref()
            .is_some_and(|(_, _, available)| *available)
    }
    fn run(&mut self, action: &Run, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            cx.propagate();
            return;
        }
        if action.0 == Kind::TemporaryActions {
            if self.panel.is_some() {
                self.close_actions(window, cx);
            } else {
                let owner = cx.weak_entity();
                let target = self.state.read(cx).selected.clone();
                self.panel =
                    Some(cx.new(|cx| actions_panel::ActionsPanel::new(owner, target, window, cx)));
            }
            cx.notify();
            return;
        }
        if self.panel.is_some() {
            if matches!(
                action.0,
                Kind::PasteAnswer
                    | Kind::CopyTemporaryAnswer
                    | Kind::RevealWorkspace
                    | Kind::HideTemporary
                    | Kind::TrashTemporary
                    | Kind::New
                    | Kind::Stop
                    | Kind::FocusInput
                    | Kind::Model
            ) {
                self.close_actions(window, cx);
            } else {
                return;
            }
        }
        match action.0 {
            Kind::TemporarySession(number) => {
                let rows = &self.list.read(cx).delegate().rows;
                if let Some(key) = session_index(number, rows.len())
                    .and_then(|ix| rows.get(ix))
                    .map(|r| r.key.clone())
                {
                    self.state.update(cx, |s, cx| s.open(&key, cx));
                }
            }
            Kind::PasteAnswer | Kind::CopyTemporaryAnswer => {
                // A filtered-out conversation is not the selected search result.
                if self.search.focus_handle(cx).is_focused(window)
                    && self.list.read(cx).selected_index().is_none()
                {
                    return;
                }
                if let Some(text) = self
                    .state
                    .read(cx)
                    .current()
                    .and_then(|s| s.completed_answer())
                {
                    if action.0 == Kind::CopyTemporaryAnswer {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    } else {
                        temporary::paste_answer(text, window, cx);
                    }
                }
            }
            Kind::RevealWorkspace => {
                if let Some(path) = self
                    .state
                    .read(cx)
                    .selected
                    .as_ref()
                    .and_then(|key| self.state.read(cx).temporary_workspace(key))
                {
                    cx.reveal_path(&path);
                }
            }
            Kind::HideTemporary => temporary::hide(window, cx),
            Kind::TrashTemporary => self.home.update(cx, |home, cx| {
                home.run_action(&Run(Kind::Delete), window, cx)
            }),
            Kind::New => {
                self.search
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.home
                    .update(cx, |home, cx| home.run_action(action, window, cx));
            }
            _ => self
                .home
                .update(cx, |home, cx| home.run_action(action, window, cx)),
        }
    }
    fn render_primary(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let home = self.home.read(cx);
        let sending = home.input.focus_handle(cx).is_focused(window)
            && self
                .state
                .read(cx)
                .current()
                .is_some_and(|s| !s.composer_empty());
        let enabled = if sending {
            home.command_input_allowed(cx)
        } else {
            self.has_answer()
                && (!self.search.focus_handle(cx).is_focused(window)
                    || self.list.read(cx).selected_index().is_some())
        };
        Button::new("temporary-primary")
            .small()
            .ghost()
            .label(t(
                cx,
                if sending {
                    "temporary-send"
                } else {
                    "temporary-paste-answer"
                },
            ))
            .children(if sending {
                Some(Kbd::new(Keystroke::parse("enter").unwrap()))
            } else {
                crate::features::command_palette::binding(Kind::PasteAnswer, window)
            })
            .disabled(!enabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                if sending {
                    this.home
                        .update(cx, |home, cx| home.submit_or_paste(false, window, cx));
                } else {
                    this.run(&Run(Kind::PasteAnswer), window, cx);
                }
            }))
    }
    fn render_actions(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let panel = self.panel.clone();
        let focus = panel.as_ref().map(|panel| panel.read(cx).focus_handle(cx));
        Popover::new("temporary-actions")
            .anchor(Anchor::BottomRight)
            .p_0()
            .open(panel.is_some())
            .when_some(focus, |popover, focus| popover.track_focus(&focus))
            .trigger(
                Button::new("temporary-actions-trigger")
                    .small()
                    .ghost()
                    .label(t(cx, "temporary-actions"))
                    .children(crate::features::command_palette::binding(
                        Kind::TemporaryActions,
                        window,
                    )),
            )
            .on_open_change(move |open, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if *open && this.panel.is_none() {
                        let owner = cx.weak_entity();
                        let target = this.state.read(cx).selected.clone();
                        this.panel =
                            Some(cx.new(|cx| {
                                actions_panel::ActionsPanel::new(owner, target, window, cx)
                            }));
                    } else if !open {
                        this.close_actions(window, cx);
                    }
                    cx.notify();
                });
            })
            .when_some(panel, |popover, panel| popover.child(panel))
    }
    fn move_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        let list = self.list.read(cx);
        let Some(index) = next_index(
            list.selected_index().map(|i| i.row),
            list.delegate().rows.len(),
            delta,
        ) else {
            return;
        };
        let key = list.delegate().rows[index].key.clone();
        self.state.update(cx, |s, cx| s.open(&key, cx));
        self.list.update(cx, |list, cx| {
            let ix = IndexPath::default().row(index);
            list.set_selected_index(Some(ix), window, cx);
            list.scroll_to_item(ix, ScrollStrategy::Top, window, cx);
            cx.notify();
        });
    }
    fn toggle_focus(&mut self, _: &ToggleInputFocus, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) || self.panel.is_some() {
            cx.propagate();
            return;
        }
        // Extension forms and picker controls keep their normal tab navigation.
        if self.search.focus_handle(cx).is_focused(window) {
            self.home
                .update(cx, |home, cx| home.focus_composer(window, cx));
        } else if self.home.read(cx).input.focus_handle(cx).is_focused(window) {
            self.focus_search(window, cx);
        } else {
            cx.propagate();
        }
    }
}
impl Render for TemporaryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&t(cx, "temporary-title"));
        v_flex()
            .key_context("Gupi GupiTemporary GupiApplication")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .capture_action(cx.listener(|this, action: &Run, window, cx| {
                if action.0 == Kind::New && !window.has_active_dialog(cx) {
                    this.run(action, window, cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(Self::run))
            .on_action(
                cx.listener(|this, _: &menus::ShowCommandPalette, window, cx| {
                    this.home.update(cx, |home, cx| {
                        home.run_action(&Run(Kind::Palette), window, cx)
                    });
                }),
            )
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                if window.has_active_dialog(cx) {
                    cx.propagate();
                } else {
                    this.focus_search(window, cx);
                }
            }))
            .on_action(cx.listener(Self::toggle_focus))
            .on_action(
                cx.listener(|this, _: &MoveUp, window, cx| this.move_selection(-1, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &MoveDown, window, cx| this.move_selection(1, window, cx)),
            )
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .prefix(Icon::new(IconName::Search))
                            .suffix(
                                Kbd::binding_for_action(
                                    &FocusSearch,
                                    Some("GupiTemporary"),
                                    window,
                                )
                                .map(IntoElement::into_any_element)
                                .unwrap_or_else(|| div().into_any_element()),
                            )
                            .cleanable(true)
                            .p_0(),
                    ),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    h_resizable("gupi-temporary-layout")
                        .child(
                            resizable_panel()
                                .size(px(280.))
                                .size_range(px(220.)..px(420.))
                                .child(
                                    v_flex().size_full().bg(cx.theme().background).child(
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .p_2()
                                            .child(List::new(&self.list).large()),
                                    ),
                                ),
                        )
                        .child(
                            resizable_panel()
                                .child(div().size_full().min_w_0().child(self.home.clone())),
                        ),
                ),
            )
            .child(
                h_flex()
                    .px_3()
                    .py_1()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(cx, "temporary-title")),
                    )
                    .child(self.render_primary(window, cx))
                    .child(self.render_actions(window, cx)),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
#[derive(Clone)]
struct Row {
    key: String,
    title: String,
    activity: Activity,
}
struct Sessions {
    rows: Vec<Row>,
    selected: Option<IndexPath>,
    state: Entity<ConversationState>,
    home: WeakEntity<HomeView>,
}
#[derive(IntoElement)]
struct SessionItem {
    row: Row,
    selected: bool,
    state: Entity<ConversationState>,
    home: WeakEntity<HomeView>,
    shortcut: Option<u8>,
}
impl Selectable for SessionItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}
impl RenderOnce for SessionItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = self.row.key;
        h_flex()
            .id(format!("temporary-session-{key}"))
            .w_full()
            .min_w_0()
            .h_9()
            .px_3()
            .gap_2()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .when(self.selected, |row| {
                row.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .when(!self.selected, |row| {
                row.hover(|row| row.bg(cx.theme().accent))
            })
            .child(
                Icon::new(IconName::MessageCircle)
                    .size_4()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .truncate()
                    .child(self.row.title),
            )
            .child(navigation::activity_mark(self.row.activity, cx))
            .children(self.shortcut.and_then(|n| {
                crate::features::command_palette::binding(Kind::TemporarySession(n), window)
            }))
            .context_menu(move |menu, _, cx| {
                navigation::session_menu(
                    menu,
                    key.clone(),
                    self.state.clone(),
                    self.home.clone(),
                    cx,
                )
            })
    }
}
impl ListDelegate for Sessions {
    type Item = SessionItem;
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.rows.len()
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        Some(SessionItem {
            row: self.rows.get(ix.row)?.clone(),
            selected: false,
            state: self.state.clone(),
            home: self.home.clone(),
            shortcut: if ix.row < 8 {
                Some(ix.row as u8 + 1)
            } else if ix.row + 1 == self.rows.len() {
                Some(9)
            } else {
                None
            },
        })
    }
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }
    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        if let Some(row) = self.selected.and_then(|ix| self.rows.get(ix.row)) {
            let key = row.key.clone();
            let state = self.state.clone();
            window.defer(cx, move |_, cx| state.update(cx, |s, cx| s.open(&key, cx)));
        }
    }
    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .p_4()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(t(cx, "temporary-search-empty"))
    }
}
fn matches_query(title: &str, query: &str) -> bool {
    title.to_lowercase().contains(query)
}
fn next_index(current: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current {
        Some(i) => (i as isize + delta).clamp(0, count as isize - 1) as usize,
        None => {
            if delta < 0 {
                count - 1
            } else {
                0
            }
        }
    })
}
fn session_index(number: u8, count: usize) -> Option<usize> {
    match number {
        1..=8 if number as usize <= count => Some(number as usize - 1),
        9 => count.checked_sub(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{matches_query, next_index};
    #[test]
    fn search_and_navigation_handle_filtered_out_selection() {
        assert!(matches_query("Rust 临时会话", "rust"));
        assert!(!matches_query("Rust 临时会话", "workdir"));
        assert_eq!(next_index(None, 0, 1), None);
        assert_eq!(next_index(None, 3, 1), Some(0));
        assert_eq!(next_index(None, 3, -1), Some(2));
        assert_eq!(next_index(Some(0), 3, -1), Some(0));
        assert_eq!(next_index(Some(2), 3, 1), Some(2));
    }
}
