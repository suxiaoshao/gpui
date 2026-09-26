pub(crate) mod actions;
mod attachments;
mod composer;
mod content;
mod history;
mod image_preview;
mod messages;
pub(crate) mod navigation;
pub(crate) mod palette;
mod panes;
pub(crate) mod pickers;
mod progress;
mod session_info;
mod slash;
mod titlebar;
mod welcome;
use crate::{
    foundation::{assets::IconName, i18n::t},
    state::{
        conversation::{ConversationEvent, ConversationState},
        history::{HistoryDetail, HistoryScope, HistoryView},
        layout,
    },
};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, WindowExt as _,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Enter, InputEvent, TextareaState},
    list::{ListEvent, ListState},
    message_scroller::MessageScrollerState,
    v_flex,
};
use gpui_kit::*;
use pi_rpc::protocol::StreamingBehavior;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
};

struct SessionView {
    history_canvas: Entity<history::canvas::HistoryCanvas>,
    history_projection: Option<(u64, HistoryDetail)>,
    model_picker: Entity<pickers::Picker>,
    clock: Entity<progress::ProcessClock>,
    list_projection: Option<(u64, HistoryDetail, HistoryScope, Option<String>)>,
    preview: Option<String>,
    process_open: HashMap<String, bool>,
    queue_open: bool,
    notices_open: bool,
    scroller: Entity<MessageScrollerState>,
    rows: Rc<Vec<messages::ChatRow>>,
    row_positions: Rc<std::cell::RefCell<HashMap<String, usize>>>,
    content_revision: u64,
}
pub(crate) struct HomeView {
    pub state: Entity<ConversationState>,
    pub(crate) input: Entity<TextareaState>,
    extension_input: Entity<TextareaState>,
    shown_key: Option<String>,
    notification_visible: bool,
    shown_request: Option<(String, String)>,
    views: BTreeMap<String, SessionView>,
    history_list: Entity<ListState<history::HistoryDelegate>>,
    history_view: HistoryView,
    history_detail: HistoryDetail,
    history_scope: HistoryScope,
    show_sidebar: bool,
    show_history: bool,
    navigation: navigation::Navigation,
    projects_with_more: HashSet<PathBuf>,
    open_projects: HashSet<PathBuf>,
    layout_save: Option<Task<()>>,
    progress: Entity<progress::RetryView>,
    pane_layout: panes::PaneLayout,
    pane_drag: Option<panes::Drag>,
    palette: Option<Entity<palette::Palette>>,
    image_preview: Entity<image_preview::PreviewHost>,
    slash: slash::Completion,
    pub(crate) command_panel: Option<Entity<super::command_palette::CommandPalette>>,
    focus_handle: FocusHandle,
    extension_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl HomeView {
    fn sync_messages(&mut self, key: &str, force: bool, cx: &mut Context<Self>) {
        let Some(session) = self.state.read(cx).sessions.get(key) else {
            return;
        };
        let Some(view) = self.views.get_mut(key) else {
            return;
        };
        if force || view.content_revision != session.content_revision {
            if view
                .preview
                .as_ref()
                .is_some_and(|id| session.history().entry(id).is_none())
            {
                view.preview = None;
            }
            let rows = messages::project(session, view.preview.as_deref());
            let old = view.rows.clone();
            view.rows = Rc::new(rows);
            let new = view.rows.clone();
            *view.row_positions.borrow_mut() = new
                .iter()
                .enumerate()
                .map(|(i, row)| (row.id.clone(), i))
                .collect();
            view.content_revision = session.content_revision;
            view.scroller.update(cx, |state, cx| {
                let prefix = old
                    .iter()
                    .zip(new.iter())
                    .take_while(|(a, b)| a.id == b.id)
                    .count();
                if prefix < old.len() || new.len() != old.len() {
                    let _ = state.splice(prefix..old.len(), new.len() - prefix, cx);
                }
                for (index, row) in new.iter().enumerate() {
                    if old.get(index) != Some(row) || row.active() {
                        state.remeasure_items(index..index + 1, cx);
                    }
                }
            });
        }
    }
    pub(crate) fn has_image_preview(&self, cx: &App) -> bool {
        self.image_preview.read(cx).is_open()
    }
    pub(crate) fn submit_or_paste(
        &mut self,
        secondary: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        if window.has_active_dialog(cx) || self.has_image_preview(cx) {
            return;
        }
        let Some(key) = self.state.read(cx).selected.clone() else {
            return;
        };
        let state = self.state.read(cx);
        if state.temporary && state.current().is_some_and(|s| s.composer_empty()) {
            if !secondary
                && crate::state::keybindings::uses_enter(actions::Kind::PasteAnswer, cx)
                && let Some(text) = state.current().and_then(|s| s.completed_answer())
            {
                crate::app::temporary::paste_answer(text, window, cx);
            }
            return;
        }
        let preview = self.views.get(&key).and_then(|v| v.preview.as_deref());
        let allowed = self.state.read(cx).current().is_some_and(|s| {
            self.state.read(cx).can_submit(&key, cx)
                && preview.is_none_or(|id| s.history().on_current_path(id))
        });
        if allowed {
            self.state.update(cx, |s, cx| {
                s.send(
                    &key,
                    if secondary {
                        StreamingBehavior::FollowUp
                    } else {
                        StreamingBehavior::Steer
                    },
                    cx,
                )
            });
        }
    }
    pub fn new(command: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| ConversationState::new(command, cx));
        Self::with_state(state, window, cx)
    }
    pub(crate) fn with_state(
        state: Entity<ConversationState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        actions::init(cx);
        for context in ["GupiComposer > Input", "GupiPalette > Input"] {
            cx.bind_keys([
                KeyBinding::new(
                    "alt-enter",
                    Enter {
                        secondary: true,
                        shift: false,
                    },
                    Some(context),
                ),
                KeyBinding::new(
                    "super-enter",
                    Enter {
                        secondary: false,
                        shift: true,
                    },
                    Some(context),
                ),
            ]);
        }
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 8)
                .submit_on_enter(true)
        });
        let extension_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 10)
                .submit_on_enter(true)
        });
        let owner = cx.entity().downgrade();
        let history_list = cx.new(|cx| {
            ListState::new(history::HistoryDelegate::new(owner), window, cx).searchable(false)
        });
        crate::app::notifications::present(&state, window, true, cx);
        let subscriptions = vec![
            cx.observe_window_activation(window, |this, window, cx| {
                crate::app::notifications::present(
                    &this.state,
                    window,
                    this.notification_visible,
                    cx,
                );
            }),
            cx.observe(&input, |_, _, cx| cx.notify()),
            cx.observe(&history_list, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &state,
                window,
                |this, _, event: &ConversationEvent, window, cx| match event {
                    ConversationEvent::Changed(changes) => {
                        if changes.catalog {
                            this.sync_navigation(None, cx);
                        } else {
                            for (key, navigation) in &changes.sessions {
                                if *navigation {
                                    this.sync_navigation(Some(key), cx);
                                }
                            }
                        }
                        if changes.selection {
                            this.sync_navigation_selection(cx);
                        }
                        if changes.catalog || changes.affects(this.state.read(cx).selected.as_ref())
                        {
                            this.sync(false, window, cx);
                        } else if let Some(key) = this.state.read(cx).selected.clone()
                            && changes.bodies.contains(&key)
                        {
                            this.sync_messages(&key, false, cx);
                        }
                        for key in changes.sessions.keys() {
                            if !this.state.read(cx).sessions.contains_key(key) {
                                this.views.remove(key);
                            }
                        }
                        if changes.progress {
                            cx.notify();
                        }
                    }
                    ConversationEvent::Notify { .. } | ConversationEvent::Attention(_) => {}
                },
            ),
            cx.subscribe_in(&input, window, |this, input, event, window, cx| {
                let Some(key) = this.shown_key.clone() else {
                    return;
                };
                if this.state.read(cx).selected.as_ref() != Some(&key) {
                    return;
                }
                match event {
                    InputEvent::Change => {
                        if this.state.read(cx).sessions[&key].submitting() {
                            this.sync(false, window, cx);
                            return;
                        }
                        let value = input.read(cx).value().to_string();
                        this.state
                            .update(cx, |s, cx| s.set_draft(&key, value.clone(), cx));
                        this.open_slash_if_needed(&value, window, cx);
                    }
                    InputEvent::PressEnter {
                        secondary,
                        shift: false,
                    } => {
                        this.submit_or_paste(*secondary, window, cx);
                    }
                    _ => {}
                }
            }),
            cx.subscribe_in(&extension_input, window, |this, input, event, _, cx| {
                let Some((key, id)) = this.shown_request.clone() else {
                    return;
                };
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    this.state.update(cx, |state, _| {
                        if let Some(p) = state
                            .sessions
                            .get_mut(&key)
                            .and_then(|s| s.pending_ui.front_mut())
                            .filter(|p| p.request.id == id)
                        {
                            p.text = text;
                        }
                    });
                }
                if matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                    this.submit_extension(cx);
                }
            }),
            cx.subscribe_in(&history_list, window, |this, list, event, window, cx| {
                if let ListEvent::Confirm(ix) = event
                    && let Some(row) = list.read(cx).delegate().rows.get(ix.row)
                {
                    this.preview_node(row.id.clone(), window, cx);
                }
            }),
        ];
        if state.read(cx).selected.is_none() {
            state.update(cx, |state, cx| state.load(cx));
        }
        let mut view = Self {
            state,
            input,
            extension_input,
            shown_key: None,
            notification_visible: true,
            shown_request: None,
            views: BTreeMap::new(),
            history_list,
            history_view: Default::default(),
            history_detail: Default::default(),
            history_scope: Default::default(),
            show_sidebar: true,
            show_history: false,
            navigation: Default::default(),
            projects_with_more: HashSet::new(),
            open_projects: HashSet::new(),
            layout_save: None,
            progress: cx.new(progress::RetryView::new),
            pane_layout: panes::PaneLayout::default(),
            pane_drag: None,
            palette: None,
            image_preview: cx.new(image_preview::PreviewHost::new),
            slash: Default::default(),
            command_panel: None,
            focus_handle: cx.focus_handle(),
            extension_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        view.sync_navigation(None, cx);
        view.sync(false, window, cx);
        view.focus_composer(window, cx);
        view
    }
    pub fn set_notification_visible(
        &mut self,
        visible: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.notification_visible != visible {
            self.notification_visible = visible;
            crate::app::notifications::present(&self.state, window, visible, cx);
        }
    }
    fn sync(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.state.read(cx).selected.clone();
        let changed = self.shown_key != key;
        if changed {
            self.slash = Default::default();
        }
        if changed && let Some(view) = self.shown_key.as_ref().and_then(|key| self.views.get(key)) {
            view.clock.update(cx, |clock, cx| clock.sync(None, cx));
            view.history_canvas
                .update(cx, |canvas, cx| canvas.clear_pointer(cx));
            view.model_picker
                .update(cx, |picker, cx| picker.close(window, cx));
        }
        self.shown_key = key.clone();
        let Some(key) = key else {
            if changed {
                self.shown_request = None;
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.extension_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                let owner = cx.weak_entity();
                self.history_list.update(cx, |list, cx| {
                    *list.delegate_mut() = history::HistoryDelegate::new(owner);
                    list.set_selected_index(None, window, cx);
                    cx.notify();
                });
                self.focus_handle.focus(window, cx);
            }
            self.progress
                .update(cx, |progress, cx| progress.sync(None, None, cx));
            cx.notify();
            return;
        };
        if !self.views.contains_key(&key) {
            let history_canvas = cx.new(history::canvas::HistoryCanvas::new);
            self._subscriptions
                .push(cx.observe(&history_canvas, |_, _, cx| cx.notify()));
            let canvas_key = key.clone();
            self._subscriptions.push(cx.subscribe_in(
                &history_canvas,
                window,
                move |this, _, event: &history::canvas::CanvasEvent, window, cx| {
                    match event {
                        history::canvas::CanvasEvent::Preview(id)
                            if this.shown_key.as_ref() == Some(&canvas_key) =>
                        {
                            this.preview_node(id.clone(), window, cx)
                        }
                        history::canvas::CanvasEvent::Fork(id) => this
                            .state
                            .update(cx, |state, cx| state.fork(&canvas_key, id.clone(), cx)),
                        _ => {}
                    }
                },
            ));
            let scroller = cx.new(|cx| MessageScrollerState::new(0, cx));
            self._subscriptions
                .push(cx.observe(&scroller, |_, _, cx| cx.notify()));
            let picker_state = self.state.clone();
            let source_key = key.clone();
            let model_picker = cx.new(|cx| {
                pickers::Picker::new(
                    move |cx| {
                        picker_state
                            .read(cx)
                            .sessions
                            .get(&source_key)
                            .map(pickers::Projection::from_session)
                            .unwrap_or_default()
                    },
                    window,
                    cx,
                )
            });
            self._subscriptions
                .push(cx.observe(&model_picker, |_, _, cx| cx.notify()));
            let picker_key = key.clone();
            self._subscriptions.push(cx.subscribe_in(
                &model_picker,
                window,
                move |this, _, event: &pickers::PickerEvent, window, cx| {
                    let owner = this.state.clone();
                    let key = picker_key.clone();
                    let event = event.clone();
                    window.defer(cx, move |_, cx| {
                        owner.update(cx, |state, cx| {
                            if state.selected.as_ref() != Some(&key) {
                                return;
                            }
                            match event {
                                pickers::PickerEvent::Model(selected) => {
                                    let model = state
                                        .sessions
                                        .get(&key)
                                        .and_then(|s| {
                                            s.model_options().iter().find(|m| {
                                                m.provider == selected.provider
                                                    && m.id == selected.id
                                            })
                                        })
                                        .cloned();
                                    if let Some(model) = model {
                                        state.set_model(&key, model, cx);
                                    }
                                }
                                pickers::PickerEvent::Thinking(level) => {
                                    state.set_thinking(&key, level, cx)
                                }
                                pickers::PickerEvent::ResetModel
                                | pickers::PickerEvent::ResetThinking => {}
                                pickers::PickerEvent::Load => state.refresh_models(&key, cx),
                                pickers::PickerEvent::Refresh => {
                                    state.refresh_models(&key, cx);
                                }
                            }
                        })
                    });
                },
            ));
            self.views.insert(
                key.clone(),
                SessionView {
                    history_canvas,
                    history_projection: None,
                    model_picker,
                    clock: cx.new(progress::ProcessClock::new),
                    list_projection: None,
                    preview: None,
                    process_open: HashMap::new(),
                    queue_open: false,
                    notices_open: false,
                    scroller,
                    rows: Rc::new(vec![]),
                    row_positions: Rc::default(),
                    content_revision: u64::MAX,
                },
            );
        }
        let view = self.views.get_mut(&key).unwrap();
        view.model_picker
            .update(cx, |picker, cx| picker.sync_controls(window, cx));
        let Some(session) = self.state.read(cx).sessions.get(&key) else {
            return;
        };
        let retry = session.retry.clone();
        let summary_retry = session.summary_retry.clone();
        let started_at = session.run_started_at();
        self.progress
            .update(cx, |progress, cx| progress.sync(retry, summary_retry, cx));
        self.views[&key]
            .clock
            .update(cx, |clock, cx| clock.sync(started_at, cx));
        let Some(session) = self.state.read(cx).sessions.get(&key) else {
            return;
        };
        let draft = session.draft.clone();
        let request = session.pending_ui.front().map(|p| {
            (
                p.request.id.clone(),
                p.text.clone(),
                matches!(p.request.method, pi_rpc::protocol::UiMethod::Editor { .. }),
            )
        });
        self.sync_messages(&key, changed || force, cx);
        let view = self.views.get_mut(&key).unwrap();
        let Some(session) = self.state.read(cx).sessions.get(&key) else {
            return;
        };
        let history_key = (
            session.history().revision,
            self.history_detail,
            self.history_scope,
            view.preview.clone(),
        );
        let forkable: HashSet<_> = session
            .fork_options()
            .iter()
            .map(|m| m.entry_id.clone())
            .collect();
        let can_fork = !session.settings_busy() && !session.model_change.unconfirmed();
        let history_changed =
            changed || force || view.list_projection.as_ref() != Some(&history_key);
        let controls_changed = self.history_list.read(cx).delegate().forkable != forkable
            || self.history_list.read(cx).delegate().can_fork != can_fork;
        if history_changed || controls_changed {
            view.list_projection = Some(history_key);
            let history_rows = if history_changed {
                Rc::new(session.history().list_rows(
                    self.history_detail,
                    self.history_scope,
                    view.preview.as_deref(),
                ))
            } else {
                self.history_list.read(cx).delegate().rows.clone()
            };
            let target = view
                .preview
                .clone()
                .or_else(|| session.history().leaf.clone());
            let selected = self
                .history_list
                .read(cx)
                .delegate()
                .selected_id
                .clone()
                .filter(|_| !changed)
                .or(target)
                .and_then(|id| session.history().visible_ancestor(&id, self.history_detail))
                .filter(|id| history_rows.iter().any(|row| &row.id == id))
                .or_else(|| {
                    view.preview
                        .as_deref()
                        .and_then(|id| session.history().visible_ancestor(id, self.history_detail))
                        .filter(|id| history_rows.iter().any(|row| &row.id == id))
                })
                .or_else(|| {
                    history_rows
                        .iter()
                        .find(|row| row.current)
                        .or_else(|| history_rows.last())
                        .map(|row| row.id.clone())
                });
            let projection = (session.history().revision, self.history_detail);
            let canvas_rows = if view.history_projection != Some(projection) {
                let rows = session.history().tree_rows(self.history_detail);
                view.history_projection = Some(projection);
                Some((
                    rows,
                    session
                        .history()
                        .entries
                        .iter()
                        .map(|entry| entry.id.clone())
                        .collect(),
                ))
            } else {
                None
            };
            let canvas_preview = view
                .preview
                .as_deref()
                .and_then(|id| session.history().visible_ancestor(id, self.history_detail));
            let history_preview = view
                .preview
                .as_deref()
                .and_then(|id| session.history().visible_ancestor(id, self.history_detail));
            view.history_canvas.update(cx, |canvas, cx| {
                canvas.sync(canvas_rows, canvas_preview, forkable.clone(), can_fork, cx)
            });
            self.history_list.update(cx, |list, cx| {
                let old_selected = list.delegate().selected_id.clone();
                let old_lane_offset = list.delegate().lane_offset;
                let delegate = list.delegate_mut();
                if history_changed {
                    delegate.graph =
                        Rc::new(crate::state::history::HistoryGraph::new(&history_rows));
                }
                delegate.preview_id = history_preview;
                if changed {
                    delegate.lane_offset = 0;
                }
                delegate.rows = history_rows;
                delegate.session = key.clone();
                delegate.forkable = forkable;
                delegate.can_fork = can_fork;
                let ix = selected
                    .as_ref()
                    .and_then(|id| delegate.rows.iter().position(|r| &r.id == id))
                    .map(gpui_kit::component::IndexPath::new);
                list.set_selected_index(ix, window, cx);
                if changed {
                    list.scroll_to_selected_item(window, cx);
                }
                if !changed && selected == old_selected {
                    // Incoming content must not undo deliberate horizontal browsing.
                    let delegate = list.delegate_mut();
                    delegate.lane_offset = old_lane_offset;
                    delegate.pan(0);
                }
                cx.notify();
            });
        }
        if self.show_history {
            self.state
                .update(cx, |state, cx| state.read_visible_history(&key, cx));
        }
        if self.input.read(cx).value().as_ref() != draft {
            self.input
                .update(cx, |input, cx| input.set_value(draft, window, cx));
        }
        if let Some((id, text, editor)) = request {
            let request_key = (key, id);
            if self.shown_request.as_ref() != Some(&request_key) {
                self.shown_request = Some(request_key);
                self.extension_input.update(cx, |input, cx| {
                    input.set_value(text, window, cx);
                    input.set_submit_on_enter(!editor, cx);
                    input.focus(window, cx);
                });
            }
        } else {
            let restore = self.shown_request.take().is_some();
            if restore {
                self.input.update(cx, |input, cx| input.focus(window, cx));
            }
        }
        cx.notify();
    }
    fn preview_node(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.shown_key.clone() else {
            return;
        };
        if let Some(view) = self.views.get_mut(&key) {
            view.preview = Some(id.clone());
            view.history_canvas
                .update(cx, |canvas, cx| canvas.reveal(id.clone(), cx));
        }
        self.sync(true, window, cx);
        if let Some(view) = self.views.get_mut(&key)
            && let Some(index) = view.rows.iter().position(|row| row.entries.contains(&id))
        {
            view.rows[index].reveal(&id, &mut view.process_open);
            view.scroller.update(cx, |s, cx| {
                s.remeasure_items(index..index + 1, cx);
                s.scroll_to_item(index, cx);
            });
        }
    }
    fn return_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(key) = &self.shown_key
            && let Some(view) = self.views.get_mut(key)
        {
            view.preview = None;
            view.scroller.update(cx, |s, cx| s.scroll_to_end(cx));
        }
        self.sync(true, window, cx);
    }
    fn pick_directory(&mut self, cx: &mut Context<Self>) {
        let key = self.shown_key.clone();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(cx, "conversation-project").into()),
        });
        cx.spawn(async move |owner, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = owner.update(cx, |this, cx| {
                    if let Some(key) = key {
                        this.state.update(cx, |s, cx| s.set_cwd(&key, path, cx));
                    } else {
                        this.state
                            .update(cx, |s, cx| s.new_or_reuse(Some(path), cx));
                    }
                });
            }
        })
        .detach();
    }
    fn save_layout(
        &mut self,
        left: Option<f32>,
        right: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.read(cx).temporary {
            return;
        }
        let mut layout = layout::capture(window, cx.global::<layout::LayoutState>());
        if let Some(width) = left {
            layout.sidebar_width = width.clamp(160., 480.);
        }
        if let Some(width) = right {
            layout.history_width = width.clamp(240., 520.);
        }
        cx.set_global(layout.clone());
        let previous = self.layout_save.take();
        self.layout_save = Some(cx.spawn(async move |_, _| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = smol::unblock(move || {
                crate::foundation::paths::config_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| layout::save(&dir.join("state.toml"), &layout))
            })
            .await;
            if let Err(error) = result {
                tracing::warn!(%error,"panel layout save failed");
            }
        }));
    }
    pub fn flush(&mut self, cx: &mut Context<Self>) -> Task<()> {
        let save = self.state.update(cx, |s, cx| s.flush(cx));
        let layout = self.layout_save.take();
        cx.spawn(async move |_, _| {
            save.await;
            if let Some(layout) = layout {
                layout.await;
            }
        })
    }
}
impl Render for HomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let commands = crate::app::menus::CONVERSATION_COMMANDS.map(|kind| {
            !window.has_active_dialog(cx)
                && !self.has_image_preview(cx)
                && self.action_enabled(kind, cx)
        });
        crate::app::menus::conversation_commands(commands, window, cx);
        if self.state.read(cx).temporary {
            let empty =
                self.state.read(cx).current().is_some_and(|s| {
                    s.empty_conversation() && !s.busy() && s.pending_ui.is_empty()
                });
            let content = if empty {
                v_flex()
                    .size_full()
                    .justify_center()
                    .items_center()
                    .px_8()
                    .py_12()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(780.))
                            .child(self.render_composer(window, cx)),
                    )
            } else {
                v_flex()
                    .size_full()
                    .min_w_0()
                    .child(self.render_messages(window, cx))
                    .child(self.render_composer(window, cx))
            };
            return content
                .child(self.image_preview.clone())
                .key_context("Gupi")
                .track_focus(&self.focus_handle)
                .into_any_element();
        }
        if self.pane_layout.fit(
            f32::from(window.viewport_size().width),
            self.show_sidebar,
            self.show_history,
            cx.global::<layout::LayoutState>(),
        ) {
            // A window resize or panel toggle ends an in-progress drag without
            // persisting a width that was only imposed by the available space.
            if self.pane_drag.take().is_some() {
                cx.stop_active_drag(window);
            }
        }
        let titlebar = self.render_titlebar(window, cx);
        let center = v_flex()
            .size_full()
            .min_w_0()
            .child(self.render_messages(window, cx))
            .child(self.render_composer(window, cx));
        // Keep the component mounted: Offcanvas owns the closing animation and
        // removes its contents from the tab order after the transition finishes.
        let mut columns = h_flex().size_full().child(self.render_sidebar(window, cx));
        let mut center_panel = div()
            .id("conversation-center")
            .debug_selector(|| "conversation-center".into())
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(center);
        if self.show_sidebar {
            center_panel = center_panel.child(self.pane_handle(panes::Side::Left, cx));
        }
        columns = columns.child(center_panel);
        if self.show_history && !self.pane_layout.overlay {
            columns = columns.child(
                div()
                    .relative()
                    .w(px(self.pane_layout.right))
                    .h_full()
                    .flex_none()
                    .child(self.render_history(cx))
                    .child(self.pane_handle(panes::Side::Right, cx)),
            );
        }
        let mut shell = div().relative().flex_1().min_h_0().w_full().child(columns);
        if self.show_history && self.pane_layout.overlay {
            shell = shell.child(
                div()
                    .occlude()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(self.pane_layout.right))
                    .bg(cx.theme().background)
                    .shadow_md()
                    .child(self.render_history(cx))
                    .child(self.pane_handle(panes::Side::Right, cx)),
            );
        }
        shell = shell.child(panes::ResizeEvents(cx.weak_entity()));
        v_flex()
            .key_context("Gupi")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::run_action))
            .on_action(cx.listener(
                |this, _: &crate::app::menus::ShowCommandPalette, window, cx| {
                    this.run_action(&actions::Run(actions::Kind::Palette), window, cx)
                },
            ))
            .size_full()
            .relative()
            .child(titlebar)
            .children(crate::features::chrome::app_menu_bar(window, cx))
            .child(shell)
            .children(self.render_session_search(window, cx))
            .child(self.image_preview.clone())
            .into_any_element()
    }
}
