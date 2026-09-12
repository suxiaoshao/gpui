mod composer;
mod content;
mod history;
mod messages;
mod navigation;
mod panes;
mod pickers;
mod titlebar;
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
    preview: Option<String>,
    process_open: HashMap<String, bool>,
    scroller: Entity<MessageScrollerState>,
    rows: Rc<Vec<messages::ChatRow>>,
    content_revision: u64,
}
pub(crate) struct HomeView {
    pub state: Entity<ConversationState>,
    input: Entity<TextareaState>,
    extension_input: Entity<TextareaState>,
    shown_key: Option<String>,
    shown_request: Option<(String, String)>,
    views: BTreeMap<String, SessionView>,
    history_list: Entity<ListState<history::HistoryDelegate>>,
    history_view: HistoryView,
    history_detail: HistoryDetail,
    history_scope: HistoryScope,
    show_sidebar: bool,
    show_history: bool,
    projects_with_more: HashSet<PathBuf>,
    open_projects: HashSet<PathBuf>,
    layout_save: Option<Task<()>>,
    pane_layout: panes::PaneLayout,
    pane_drag: Option<panes::Drag>,
    _subscriptions: Vec<Subscription>,
}
impl HomeView {
    pub fn new(command: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new(
                "alt-enter",
                Enter {
                    secondary: true,
                    shift: false,
                },
                Some("GupiComposer"),
            ),
            KeyBinding::new(
                "super-enter",
                Enter {
                    secondary: false,
                    shift: true,
                },
                Some("GupiComposer"),
            ),
        ]);
        let state = cx.new(|cx| ConversationState::new(command, cx));
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
        let subscriptions = vec![
            cx.observe(&history_list, |_, _, cx| cx.notify()),
            cx.observe_in(&state, window, |this, _, window, cx| {
                this.sync(false, window, cx)
            }),
            cx.subscribe_in(
                &state,
                window,
                |_, _, event: &ConversationEvent, window, cx| match event {
                    ConversationEvent::Notify { message, error } => {
                        use gpui_kit::component::notification::Notification;
                        window.push_notification(
                            if *error {
                                Notification::error(message.clone())
                            } else {
                                Notification::info(message.clone())
                            },
                            cx,
                        );
                    }
                },
            ),
            cx.subscribe_in(&input, window, |this, input, event, _, cx| {
                let Some(key) = this.shown_key.clone() else {
                    return;
                };
                if this.state.read(cx).selected.as_ref() != Some(&key) {
                    return;
                }
                match event {
                    InputEvent::Change => {
                        let value = input.read(cx).value().to_string();
                        this.state.update(cx, |s, cx| s.set_draft(&key, value, cx));
                    }
                    InputEvent::PressEnter {
                        secondary,
                        shift: false,
                    } => {
                        let preview = this.views.get(&key).and_then(|v| v.preview.as_deref());
                        let allowed = this.state.read(cx).current().is_some_and(|s| {
                            preview.is_none_or(|id| s.history().on_current_path(id))
                        });
                        if allowed {
                            this.state.update(cx, |s, cx| {
                                s.send(
                                    &key,
                                    if *secondary {
                                        StreamingBehavior::FollowUp
                                    } else {
                                        StreamingBehavior::Steer
                                    },
                                    cx,
                                )
                            });
                        }
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
        state.update(cx, |state, cx| state.load(cx));
        let mut view = Self {
            state,
            input,
            extension_input,
            shown_key: None,
            shown_request: None,
            views: BTreeMap::new(),
            history_list,
            history_view: Default::default(),
            history_detail: Default::default(),
            history_scope: Default::default(),
            show_sidebar: true,
            show_history: false,
            projects_with_more: HashSet::new(),
            open_projects: HashSet::new(),
            layout_save: None,
            pane_layout: panes::PaneLayout::default(),
            pane_drag: None,
            _subscriptions: subscriptions,
        };
        view.sync(false, window, cx);
        view
    }
    fn sync(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.state.read(cx).selected.clone();
        let changed = self.shown_key != key;
        if changed && let Some(view) = self.shown_key.as_ref().and_then(|key| self.views.get(key)) {
            view.history_canvas
                .update(cx, |canvas, cx| canvas.clear_pointer(cx));
            view.model_picker
                .update(cx, |picker, cx| picker.close(window, cx));
        }
        self.shown_key = key.clone();
        let Some(key) = key else {
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
                    preview: None,
                    process_open: HashMap::new(),
                    scroller,
                    rows: Rc::new(vec![]),
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
        let draft = session.draft.clone();
        let request = session.pending_ui.front().map(|p| {
            (
                p.request.id.clone(),
                p.text.clone(),
                matches!(p.request.method, pi_rpc::protocol::UiMethod::Editor { .. }),
            )
        });
        let view = self.views.get_mut(&key).unwrap();
        if changed || force || view.content_revision != session.content_revision {
            if view
                .preview
                .as_ref()
                .is_some_and(|id| session.history().entry(id).is_none())
            {
                view.preview = None;
            }
            let rows = messages::project(session, view.preview.as_deref());
            let history_rows = session.history().list_rows(
                self.history_detail,
                self.history_scope,
                view.preview.as_deref(),
            );
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
            let forkable: HashSet<_> = session
                .fork_options()
                .iter()
                .map(|m| m.entry_id.clone())
                .collect();
            let can_fork = !session.settings_busy() && !session.model_change.unconfirmed();
            view.content_revision = session.content_revision;
            view.history_canvas.update(cx, |canvas, cx| {
                canvas.sync(canvas_rows, canvas_preview, forkable.clone(), can_fork, cx)
            });
            let old = view.rows.clone();
            view.rows = Rc::new(rows);
            let new = view.rows.clone();
            view.scroller.update(cx, |state, cx| {
                let prefix = old
                    .iter()
                    .zip(new.iter())
                    .take_while(|(a, b)| a.id == b.id)
                    .count();
                if prefix < old.len() || new.len() != old.len() {
                    let _ = state.splice(prefix..old.len(), new.len() - prefix, cx);
                }
                state.remeasure(cx);
            });
            self.history_list.update(cx, |list, cx| {
                let old_selected = list.delegate().selected_id.clone();
                let old_lane_offset = list.delegate().lane_offset;
                let delegate = list.delegate_mut();
                delegate.graph = Rc::new(crate::state::history::HistoryGraph::new(&history_rows));
                delegate.preview_id = history_preview;
                if changed {
                    delegate.lane_offset = 0;
                }
                delegate.rows = Rc::new(history_rows);
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
                s.remeasure(cx);
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
                        this.state.update(cx, |s, cx| s.new_draft(Some(path), cx));
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
        let mut columns = h_flex().size_full().child(self.render_sidebar(cx));
        let mut center_panel = div().relative().flex_1().min_w_0().h_full().child(center);
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
        v_flex().size_full().child(titlebar).child(shell)
    }
}
