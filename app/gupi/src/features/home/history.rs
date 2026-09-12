pub(super) mod canvas;
mod graph;
mod presentation;
use super::*;
use crate::state::history::{HistoryGraph, HistoryRow};
use gpui_kit::component::{
    Icon, IndexPath,
    button::{Toggle, ToggleGroup, ToggleVariants},
    list::{List, ListDelegate},
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
    tag::Tag,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder;
pub(super) struct HistoryDelegate {
    pub rows: Rc<Vec<HistoryRow>>,
    pub graph: Rc<HistoryGraph>,
    pub lane_offset: usize,
    pub preview_id: Option<String>,
    wheel_remainder: f32,
    pub session: String,
    pub selected_id: Option<String>,
    pub forkable: HashSet<String>,
    pub can_fork: bool,
    pub owner: WeakEntity<HomeView>,
}
impl HistoryDelegate {
    pub fn new(owner: WeakEntity<HomeView>) -> Self {
        Self {
            rows: Rc::new(vec![]),
            graph: Rc::default(),
            lane_offset: 0,
            preview_id: None,
            wheel_remainder: 0.,
            session: String::new(),
            selected_id: None,
            forkable: HashSet::new(),
            can_fork: false,
            owner,
        }
    }
    fn reveal_selected(&mut self) {
        if let Some(lane) = self
            .selected_id
            .as_ref()
            .and_then(|id| self.rows.iter().position(|row| &row.id == id))
            .and_then(|ix| self.graph.nodes.get(ix))
            .copied()
        {
            if lane < self.lane_offset {
                self.lane_offset = lane;
            }
            if lane >= self.lane_offset + graph::VISIBLE_LANES {
                self.lane_offset = lane + 1 - graph::VISIBLE_LANES;
            }
        }
        self.lane_offset = self
            .lane_offset
            .min(self.graph.lanes.len().saturating_sub(graph::VISIBLE_LANES));
    }
    pub(super) fn pan(&mut self, delta: isize) {
        self.lane_offset = self
            .lane_offset
            .saturating_add_signed(delta)
            .min(self.graph.lanes.len().saturating_sub(graph::VISIBLE_LANES));
    }
}
impl ListDelegate for HistoryDelegate {
    type Item = presentation::HistoryListItem;
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.rows.len()
    }
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected_id = ix.and_then(|i| self.rows.get(i.row)).map(|r| r.id.clone());
        self.reveal_selected();
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let row = self.rows.get(ix.row)?.clone();
        let id = row.id.clone();
        let owner = self.owner.clone();
        let session = self.session.clone();
        let forkable = self.can_fork && self.forkable.contains(&id);
        let mut content = h_flex().w_full().min_w_0().h(px(graph::ROW_HEIGHT));
        let history_graph = self.graph.clone();
        let start = self.lane_offset;
        let current = row.current;
        let preview = self.preview_id.as_deref() == Some(id.as_str());
        content = content.child(
            div()
                .id(format!("history-graph-{}", id))
                .w(px(graph::column_width(history_graph.lanes.len())))
                .h(px(graph::ROW_HEIGHT))
                .flex_shrink_0()
                .overflow_hidden()
                .on_scroll_wheel(cx.listener(|list, event: &ScrollWheelEvent, window, cx| {
                    let delta = event.delta.pixel_delta(window.line_height());
                    let horizontal = if event.modifiers.shift {
                        delta.x + delta.y
                    } else {
                        delta.x
                    };
                    if horizontal.abs() > delta.y.abs() || event.modifiers.shift {
                        let delegate = list.delegate_mut();
                        delegate.wheel_remainder -= f32::from(horizontal);
                        let steps = (delegate.wheel_remainder / graph::LANE_WIDTH).trunc() as isize;
                        delegate.wheel_remainder -= steps as f32 * graph::LANE_WIDTH;
                        delegate.pan(steps);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(
                    canvas(
                        move |_, _, _| (),
                        move |bounds, (), window, cx| {
                            graph::paint_row(
                                &history_graph,
                                ix.row,
                                start,
                                current,
                                preview,
                                bounds,
                                window,
                                cx,
                            );
                        },
                    )
                    .size_full(),
                ),
        );
        let (icon, color, role_label) = presentation::role(&row, cx);
        let title = if row.title.is_empty() {
            role_label.clone()
        } else {
            row.title.clone()
        };
        let timestamp =
            presentation::tooltip(&row, self.preview_id.as_deref() == Some(id.as_str()), cx);
        let accessible_title = format!("{role_label}: {title}");
        let mut message = h_flex()
            .pl_2()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap_2()
            .child(Icon::new(icon).size_4().flex_none().text_color(color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .text_color(if presentation::is_process(row.kind) {
                        cx.theme().muted_foreground
                    } else {
                        cx.theme().foreground
                    })
                    .truncate()
                    .child(title),
            );
        if let Some(label) = &row.label {
            message = message.child(
                Tag::secondary()
                    .small()
                    .max_w(px(80.))
                    .child(div().truncate().child(label.clone())),
            );
        }
        content = content.child(message);
        Some(presentation::HistoryListItem::new(
            id.clone(),
            div()
                .id(format!("history-menu-{id}"))
                .aria_label(accessible_title)
                .tooltip(move |window, cx| Tooltip::new(timestamp.clone()).build(window, cx))
                .w_full()
                .h(px(graph::ROW_HEIGHT))
                .pr_2()
                .child(content)
                .context_menu(move |menu, _, cx| {
                    let owner = owner.clone();
                    let key = session.clone();
                    let id = id.clone();
                    menu.item(
                        PopupMenuItem::new(t(cx, "conversation-fork"))
                            .disabled(!forkable)
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.state.update(cx, |s, cx| s.fork(&key, id.clone(), cx))
                                });
                            }),
                    )
                }),
        ))
    }
}
impl HomeView {
    fn refresh_history_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.shown_key.as_ref().and_then(|key| self.views.get(key)) {
            view.history_canvas
                .update(cx, |canvas, cx| canvas.clear_pointer(cx));
        }
        self.sync(true, window, cx);
        if self.history_view == HistoryView::List {
            self.history_list
                .update(cx, |list, cx| list.scroll_to_selected_item(window, cx));
        }
        cx.notify();
    }
    pub(super) fn render_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let toolbar = h_flex()
            .px_2()
            .py_2()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(t(cx, "conversation-history")),
            )
            .child(
                ToggleGroup::new("history-display-mode")
                    .segmented()
                    .outline()
                    .small()
                    .child(
                        Toggle::new("history-list")
                            .icon(Icon::new(IconName::List).size_4())
                            .checked(self.history_view == HistoryView::List)
                            .when(self.history_view == HistoryView::List, |toggle| {
                                toggle
                                    .bg(cx.theme().primary.opacity(0.12))
                                    .text_color(cx.theme().primary)
                            })
                            .tooltip(t(cx, "history-view-list")),
                    )
                    .child(
                        Toggle::new("history-tree")
                            .icon(Icon::new(IconName::GitBranch).size_4())
                            .checked(self.history_view == HistoryView::Tree)
                            .when(self.history_view == HistoryView::Tree, |toggle| {
                                toggle
                                    .bg(cx.theme().primary.opacity(0.12))
                                    .text_color(cx.theme().primary)
                            })
                            .tooltip(t(cx, "history-view-tree")),
                    )
                    .on_click(cx.listener(|this, states: &Vec<bool>, window, cx| {
                        let next = [HistoryView::List, HistoryView::Tree]
                            .into_iter()
                            .enumerate()
                            .find(|(i, view)| {
                                *view != this.history_view && states.get(*i) == Some(&true)
                            })
                            .map(|(_, view)| view)
                            .unwrap_or(this.history_view);
                        if next != this.history_view {
                            this.history_view = next;
                            this.refresh_history_options(window, cx);
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("close-history")
                    .ghost()
                    .small()
                    .icon(IconName::X)
                    .tooltip(t(cx, "conversation-close-history"))
                    .accessibility_label(t(cx, "conversation-close-history"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(view) =
                            this.shown_key.as_ref().and_then(|key| this.views.get(key))
                        {
                            view.history_canvas
                                .update(cx, |canvas, cx| canvas.clear_pointer(cx));
                        }
                        this.show_history = false;
                        cx.notify();
                    })),
            );
        let detail = self.history_detail;
        let detail_label = t(
            cx,
            match detail {
                HistoryDetail::Brief => "history-level-brief",
                HistoryDetail::Detailed => "history-level-detailed",
                HistoryDetail::All => "history-level-all",
            },
        );
        let owner = cx.weak_entity();
        let detail_picker = h_flex()
            .flex_none()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(cx, "history-content")),
            )
            .child(
                Button::new("history-detail")
                    .ghost()
                    .small()
                    .label(detail_label.clone())
                    .dropdown_caret(true)
                    .tooltip(t(
                        cx,
                        match detail {
                            HistoryDetail::Brief => "conversation-history-brief",
                            HistoryDetail::Detailed => "conversation-history-detailed",
                            HistoryDetail::All => "history-level-all-description",
                        },
                    ))
                    .accessibility_label(format!("{}: {}", t(cx, "history-content"), detail_label))
                    .dropdown_menu(move |mut menu, _, cx| {
                        for (level, key) in [
                            (HistoryDetail::Brief, "history-level-brief"),
                            (HistoryDetail::Detailed, "history-level-detailed"),
                            (HistoryDetail::All, "history-level-all"),
                        ] {
                            let owner = owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(t(cx, key))
                                    .checked(detail == level)
                                    .on_click(move |_, window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            this.history_detail = level;
                                            this.refresh_history_options(window, cx);
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            );
        let mut options = h_flex()
            .w_full()
            .flex_wrap()
            .px_2()
            .pb_2()
            .gap_3()
            .child(detail_picker);
        if self.history_view == HistoryView::List {
            let owner = cx.weak_entity();
            let scope = self.history_scope;
            let label = t(
                cx,
                if scope == HistoryScope::All {
                    "history-scope-all"
                } else {
                    "history-scope-branch"
                },
            );
            options = options.child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(cx, "history-range")),
                    )
                    .child(
                        Button::new("history-scope")
                            .ghost()
                            .small()
                            .label(label.clone())
                            .dropdown_caret(true)
                            .tooltip(label.clone())
                            .accessibility_label(format!("{}: {}", t(cx, "history-range"), label))
                            .dropdown_menu(move |mut menu, _, cx| {
                                for (value, label) in [
                                    (HistoryScope::All, "history-scope-all"),
                                    (HistoryScope::Branch, "history-scope-branch"),
                                ] {
                                    let owner = owner.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(t(cx, label))
                                            .checked(scope == value)
                                            .on_click(move |_, window, cx| {
                                                let _ = owner.update(cx, |this, cx| {
                                                    this.history_scope = value;
                                                    this.refresh_history_options(window, cx);
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    ),
            );
        }
        let mut panel = v_flex().size_full().child(toolbar).child(options);
        if let Some(session) = self.state.read(cx).current() {
            use crate::state::conversation::content::BodyState;
            match session.body_state() {
                BodyState::New => {
                    return panel
                        .child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(t(cx, "history-canvas-empty")),
                        )
                        .into_any_element();
                }
                BodyState::Loading(stage) => {
                    return panel.child(content::skeleton(stage, cx)).into_any_element();
                }
                BodyState::Failed(error) => {
                    return panel
                        .child(self.render_content_error("retry-history-panel", error, cx))
                        .into_any_element();
                }
                BodyState::Ready => {}
                BodyState::Refreshing(stage) => panel = panel.child(content::refreshing(stage, cx)),
                BodyState::RefreshFailed(error) => {
                    panel = panel.child(self.render_content_error("retry-history-panel", error, cx))
                }
            }
        }
        if let Some(source) = self
            .state
            .read(cx)
            .current()
            .and_then(|s| s.info.parent_session.clone())
        {
            panel = panel.child(
                Button::new("fork-source")
                    .ghost()
                    .small()
                    .label(t(cx, "conversation-source"))
                    .tooltip(source.clone())
                    .on_click(move |_, _, cx| cx.reveal_path(std::path::Path::new(&source))),
            );
        }
        if let Some(session) = self.state.read(cx).current() {
            if let Some(error) = session.fork_messages.error() {
                let owner = self.state.clone();
                let key = self.shown_key.clone().unwrap_or_default();
                panel = panel.child(
                    Button::new("fork-options-error")
                        .ghost()
                        .small()
                        .icon(IconName::CircleAlert)
                        .label(t(cx, "conversation-fork-options-retry"))
                        .tooltip(error.to_owned())
                        .on_click(move |_, _, cx| {
                            owner.update(cx, |state, cx| state.refresh_fork_messages(&key, cx))
                        }),
                );
            } else if session.fork_messages.running() {
                panel = panel.child(
                    h_flex()
                        .px_2()
                        .gap_2()
                        .child(gpui_kit::component::spinner::Spinner::new().small())
                        .child(
                            div()
                                .text_xs()
                                .child(t(cx, "conversation-fork-options-loading")),
                        ),
                );
            }
        }
        if self.history_view == HistoryView::Tree {
            if let Some(view) = self.shown_key.as_ref().and_then(|key| self.views.get(key)) {
                return panel
                    .child(div().flex_1().min_h_0().child(view.history_canvas.clone()))
                    .into_any_element();
            }
            return panel
                .child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, "history-canvas-empty")),
                )
                .into_any_element();
        }
        let delegate = self.history_list.read(cx).delegate();
        let lanes = delegate.graph.lanes.len();
        let offset = delegate.lane_offset;
        if lanes > graph::VISIBLE_LANES {
            panel = panel.child(
                h_flex()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("graph-left")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowLeft)
                            .disabled(offset == 0)
                            .tooltip(t(cx, "conversation-graph-left"))
                            .accessibility_label(t(cx, "conversation-graph-left"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.history_list.update(cx, |list, cx| {
                                    list.delegate_mut().pan(-4);
                                    cx.notify();
                                });
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{}–{} / {}",
                                offset + 1,
                                (offset + graph::VISIBLE_LANES).min(lanes),
                                lanes
                            )),
                    )
                    .child(
                        Button::new("graph-right")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowRight)
                            .disabled(offset + graph::VISIBLE_LANES >= lanes)
                            .tooltip(t(cx, "conversation-graph-right"))
                            .accessibility_label(t(cx, "conversation-graph-right"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.history_list.update(cx, |list, cx| {
                                    list.delegate_mut().pan(4);
                                    cx.notify();
                                });
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("graph-reveal")
                            .ghost()
                            .xsmall()
                            .label(t(cx, "conversation-graph-reveal"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.history_list.update(cx, |list, cx| {
                                    list.delegate_mut().reveal_selected();
                                    list.scroll_to_selected_item(window, cx);
                                    cx.notify();
                                });
                            })),
                    ),
            );
        }
        panel = panel.child(
            h_flex()
                .h_7()
                .border_b_1()
                .border_color(cx.theme().border)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(
                    div()
                        .w(px(graph::column_width(lanes)))
                        .flex_shrink_0()
                        .pl_2()
                        .child(t(cx, "conversation-graph-column")),
                )
                .child(div().pl_2().child(t(cx, "conversation-graph-message"))),
        );
        panel
            .child(
                List::new(&self.history_list)
                    .p_0()
                    .py(px(12.))
                    .flex_1()
                    .min_h_0(),
            )
            .into_any_element()
    }
}
