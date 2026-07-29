use std::rc::Rc;

use super::super::workspace::{HomeWorkspace, SidebarSearchLoad, SidebarSearchResult};
use crate::foundation::{I18n, assets::IconName};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IndexPath, Selectable, Sizable, WindowExt,
    button::Button,
    h_flex,
    input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
    label::Label,
    list::{List, ListDelegate, ListState},
    v_flex,
};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use jaco_core::ConversationId;

const CONTEXT: &str = "jaco_conversation_search";
const SEARCH_ITEM_HEIGHT: f32 = 52.;
const SEARCH_RESULT_LIMIT: usize = 50;

type OnConfirm = Rc<dyn Fn(ConversationId, &mut Window, &mut App) + 'static>;

pub(crate) fn open_conversation_search_dialog(
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let title = cx.global::<I18n>().t("sidebar-search-title");
    let view = cx.new(|cx| ConversationSearchView::new(workspace, window, cx));
    let view_to_focus = view.clone();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .w(px(560.))
            .p_0()
            .close_button(false)
            .child(view.clone())
    });
    window.defer(cx, move |window, cx| {
        view_to_focus.update(cx, |view, cx| view.focus_search_input(window, cx));
    });
}

#[derive(IntoElement, Clone)]
struct ConversationSearchItem {
    result: Rc<SidebarSearchResult>,
    project_label: SharedString,
    is_selected: bool,
    on_confirm: OnConfirm,
}

impl ConversationSearchItem {
    fn new(
        result: Rc<SidebarSearchResult>,
        project_label: SharedString,
        on_confirm: OnConfirm,
    ) -> Self {
        Self {
            result,
            project_label,
            is_selected: false,
            on_confirm,
        }
    }
}

impl Selectable for ConversationSearchItem {
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

impl RenderOnce for ConversationSearchItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let result = self.result;
        let on_confirm = self.on_confirm;
        let conversation_id = result.conversation.id.clone();

        h_flex()
            .id(format!("conversation-search-result-{conversation_id}"))
            .w_full()
            .h(px(SEARCH_ITEM_HEIGHT))
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .cursor_pointer()
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().tokens.accent.background.opacity(0.45)))
            })
            .when(self.is_selected, |this| {
                this.bg(cx.theme().tokens.accent.background)
            })
            .on_click(move |_, window, cx| {
                let on_confirm = on_confirm.clone();
                let conversation_id = conversation_id.clone();
                window.defer(cx, move |window, cx| {
                    on_confirm(conversation_id, window, cx);
                });
            })
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().tokens.border.background.opacity(0.35))
                    .child(
                        Icon::new(IconName::MessageSquare)
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        Label::new(result.conversation.title.clone())
                            .text_sm()
                            .truncate(),
                    )
                    .child(
                        Label::new(self.project_label)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
    }
}

struct ConversationSearchDelegate {
    ix: Option<IndexPath>,
    items: Vec<Rc<SidebarSearchResult>>,
    no_project_label: SharedString,
    on_confirm: OnConfirm,
}

impl ConversationSearchDelegate {
    fn new(
        items: Vec<SidebarSearchResult>,
        no_project_label: SharedString,
        on_confirm: OnConfirm,
    ) -> Self {
        Self {
            ix: None,
            items: items.into_iter().map(Rc::new).collect(),
            no_project_label,
            on_confirm,
        }
    }
}

impl ListDelegate for ConversationSearchDelegate {
    type Item = ConversationSearchItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.items.get(ix.row).cloned().map(|result| {
            let project_label = result
                .project
                .as_ref()
                .map(|project| project.display_name.clone())
                .unwrap_or_else(|| self.no_project_label.clone());
            ConversationSearchItem::new(result, project_label, self.on_confirm.clone())
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.ix = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        if let Some(ix) = self.ix
            && let Some(result) = self.items.get(ix.row)
        {
            let on_confirm = self.on_confirm.clone();
            let conversation_id = result.conversation.id.clone();
            // `confirm` runs while `ListState` is locked. The callback updates
            // the workspace and closes the dialog, so dispatch it after the
            // list update releases its entity borrow.
            window.defer(cx, move |window, cx| {
                on_confirm(conversation_id, window, cx);
            });
        }
    }
}

pub(crate) struct ConversationSearchView {
    workspace: Entity<HomeWorkspace>,
    search_input: Entity<InputState>,
    results: Entity<ListState<ConversationSearchDelegate>>,
    query: String,
    operation: refresh::Operation<Vec<SidebarSearchResult>, jaco_db::DbError, Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ConversationSearchView {
    fn new(workspace: Entity<HomeWorkspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("sidebar-search-placeholder"))
        });
        let search_input_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let workspace_subscription = cx.observe_in(&workspace, window, |view, _, window, cx| {
            if view.query.is_empty() && !view.operation.is_running() {
                view.reload(window, cx);
            }
        });
        let results = Self::build_list(Vec::new(), workspace.clone(), window, cx);
        let view = Self {
            workspace,
            search_input,
            results,
            query: String::new(),
            operation: refresh::Operation::new(),
            _subscriptions: vec![search_input_subscription, workspace_subscription],
        };
        let entity = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = entity.update(cx, |view, cx| view.reload(window, cx));
        });
        view
    }

    fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input
            .update(cx, |search_input, cx| search_input.focus(window, cx));
    }

    fn build_list(
        items: Vec<SidebarSearchResult>,
        workspace: Entity<HomeWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ListState<ConversationSearchDelegate>> {
        let no_project_label = cx
            .global::<I18n>()
            .t("sidebar-section-no-project-conversations")
            .into();
        let on_confirm: OnConfirm = Rc::new(move |conversation_id, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.open_conversation(conversation_id.clone(), cx);
            });
            window.close_dialog(cx);
        });

        cx.new(move |cx| {
            let mut state = ListState::new(
                ConversationSearchDelegate::new(items, no_project_label, on_confirm),
                window,
                cx,
            );
            select_first_if_any(&mut state, window, cx);
            state
        })
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        self.query = self.current_query(cx);
        if self.operation.is_running() {
            self.operation.transition(Cancel);
        }
        self.reload(window, cx);
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.query.clone();
        let search = self.workspace.update(cx, |workspace, cx| {
            workspace.search_conversations(query.clone(), SEARCH_RESULT_LIMIT, cx)
        });
        let view = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = search.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.query != query || !view.operation.is_running() {
                    return;
                }
                view.complete_load(result);
                let items = view.operation.data().cloned().unwrap_or_default();
                view.results = Self::build_list(items, view.workspace.clone(), window, cx);
                cx.notify();
            });
        });
        match &self.operation {
            refresh::Operation::Idle(_) => self.operation.transition(Load(task)),
            refresh::Operation::Ready(_) | refresh::Operation::Degraded(_) => {
                self.operation.transition(Refresh(task))
            }
            refresh::Operation::Unavailable(_) => self.operation.transition(Retry(task)),
            refresh::Operation::Loading(_)
            | refresh::Operation::Refreshing(_)
            | refresh::Operation::Retrying(_)
            | refresh::Operation::RefreshingDegraded(_) => {}
        }
        cx.notify();
    }

    fn complete_load(&mut self, result: jaco_db::Result<SidebarSearchLoad>) {
        match result {
            Ok(load) => {
                self.operation.transition(Complete(Ok(load.results)));
                if let Some(problem) = load.stale_problem {
                    self.operation.transition(Refresh(Task::ready(())));
                    self.operation
                        .transition(Complete(Err(jaco_db::DbError::Invariant(problem))));
                }
            }
            Err(error) => self.operation.transition(Complete(Err(error))),
        }
    }

    fn on_search_move_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }
        self.move_selection(-1, window, cx);
        cx.stop_propagation();
    }

    fn on_search_move_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }
        self.move_selection(1, window, cx);
        cx.stop_propagation();
    }

    fn move_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        self.results.update(cx, |state, cx| {
            move_selected(state, delta, window, cx);
        });
        cx.notify();
    }

    fn on_search_enter(&mut self, enter: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }
        self.results.update(cx, |state, cx| {
            confirm_selected(state, enter.secondary, window, cx);
        });
        cx.stop_propagation();
    }
}

impl Render for ConversationSearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let no_results = cx.global::<I18n>().t("sidebar-search-no-results");
        let error = self.operation.problem().map(ToString::to_string);
        let running = self.operation.is_running();
        let count = item_count(self.results.read(cx), cx);

        v_flex()
            .key_context(CONTEXT)
            .w_full()
            .h(px(480.))
            .overflow_hidden()
            .on_action(cx.listener(Self::on_search_move_up))
            .on_action(cx.listener(Self::on_search_move_down))
            .on_action(cx.listener(Self::on_search_enter))
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search_input)
                            .w_full()
                            .appearance(false)
                            .p_0()
                            .bordered(false)
                            .focus_bordered(false)
                            .prefix(
                                Icon::new(IconName::Search).text_color(cx.theme().muted_foreground),
                            )
                            .cleanable(true),
                    ),
            )
            .when_some(error.clone(), |this, error| {
                this.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            Label::new(error)
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .flex_1(),
                        )
                        .child(
                            Button::new("conversation-search-retry")
                                .label(cx.global::<I18n>().t("resource-status-refresh"))
                                .xsmall()
                                .loading(running)
                                .disabled(running)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.reload(window, cx);
                                })),
                        ),
                )
            })
            .map(|this| {
                if count > 0 {
                    this.child(List::new(&self.results).large().flex_1())
                } else if running {
                    this.child(
                        v_flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(gpui_component::spinner::Spinner::new())
                            .child(
                                Label::new(cx.global::<I18n>().t("resource-status-loading"))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                } else if error.is_none() {
                    this.child(
                        v_flex().flex_1().items_center().justify_center().child(
                            Label::new(no_results)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                    )
                } else {
                    this
                }
            })
    }
}

fn select_first_if_any<D>(
    state: &mut ListState<D>,
    window: &mut Window,
    cx: &mut Context<ListState<D>>,
) where
    D: ListDelegate + 'static,
{
    let first = IndexPath::default();
    let has_items = state.delegate().items_count(0, cx) > 0;
    state.set_selected_index(has_items.then_some(first), window, cx);
    if has_items {
        state.scroll_to_item(first, ScrollStrategy::Top, window, cx);
    }
}

fn move_selected<D>(
    state: &mut ListState<D>,
    delta: isize,
    window: &mut Window,
    cx: &mut Context<ListState<D>>,
) where
    D: ListDelegate + 'static,
{
    let count = state.delegate().items_count(0, cx);
    if count == 0 {
        state.set_selected_index(None, window, cx);
        return;
    }

    let current = state.selected_index().map(|ix| ix.row).unwrap_or(0);
    let next = if delta < 0 {
        if current == 0 { count - 1 } else { current - 1 }
    } else if current + 1 >= count {
        0
    } else {
        current + 1
    };
    let next_ix = IndexPath::default().row(next);
    state.set_selected_index(Some(next_ix), window, cx);
    state.scroll_to_item(next_ix, ScrollStrategy::Top, window, cx);
}

fn confirm_selected<D>(
    state: &mut ListState<D>,
    secondary: bool,
    window: &mut Window,
    cx: &mut Context<ListState<D>>,
) where
    D: ListDelegate + 'static,
{
    let selected = state.selected_index();
    state
        .delegate_mut()
        .set_selected_index(selected, window, cx);
    state.delegate_mut().confirm(secondary, window, cx);
}

fn item_count<D>(state: &ListState<D>, cx: &App) -> usize
where
    D: ListDelegate + 'static,
{
    state.delegate().items_count(0, cx)
}

#[cfg(test)]
mod tests {
    use super::super::super::workspace::{SidebarConversationNode, SidebarSearchResult};
    use super::ConversationSearchDelegate;
    use gpui::{
        App, AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window, div,
    };
    use gpui_component::{
        IndexPath,
        list::{ListDelegate, ListState},
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    struct TestRoot;

    impl Render for TestRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn test_result() -> SidebarSearchResult {
        SidebarSearchResult {
            conversation: SidebarConversationNode {
                id: "conversation".to_string(),
                project_id: String::new(),
                title: "Conversation".into(),
                updated_at: 0,
                pinned: false,
            },
            project: None,
        }
    }

    #[gpui::test]
    fn confirm_callback_runs_after_list_update_finishes(cx: &mut TestAppContext) {
        let list_slot = Rc::new(RefCell::new(
            None::<Entity<ListState<ConversationSearchDelegate>>>,
        ));
        let callback_ran = Rc::new(Cell::new(false));
        let on_confirm = Rc::new({
            let list_slot = list_slot.clone();
            let callback_ran = callback_ran.clone();
            move |_conversation_id, _window: &mut Window, cx: &mut App| {
                let list = list_slot
                    .borrow()
                    .as_ref()
                    .cloned()
                    .expect("conversation search list should be initialized");
                list.update(cx, |_, _| callback_ran.set(true));
            }
        });

        let (_, cx) = cx.add_window_view(|window, cx| {
            let list = cx.new(|cx| {
                let mut list = ListState::new(
                    ConversationSearchDelegate::new(
                        vec![test_result()],
                        "No project".into(),
                        on_confirm,
                    ),
                    window,
                    cx,
                );
                list.delegate_mut()
                    .set_selected_index(Some(IndexPath::default()), window, cx);
                list
            });
            *list_slot.borrow_mut() = Some(list);
            TestRoot
        });
        let list = list_slot
            .borrow()
            .as_ref()
            .cloned()
            .expect("conversation search list should be initialized");

        cx.update(|window, cx| {
            list.update(cx, |list, cx| {
                list.delegate_mut().confirm(false, window, cx);
            });
        });
        cx.run_until_parked();

        assert!(callback_ran.get());
    }
}
