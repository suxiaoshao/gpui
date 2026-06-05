use std::rc::Rc;

use crate::{
    foundation::{I18n, assets::IconName},
    state::{self, workspace::SidebarSearchResult},
};
use ai_chat_core::ConversationId;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Selectable, Sizable, WindowExt, h_flex,
    input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
    label::Label,
    list::{List, ListDelegate, ListState},
    v_flex,
};

const CONTEXT: &str = "ai_chat2_conversation_search";
const SEARCH_ITEM_HEIGHT: f32 = 52.;
const SEARCH_RESULT_LIMIT: usize = 50;

type OnConfirm = Rc<dyn Fn(ConversationId, &mut Window, &mut App) + 'static>;

pub(crate) fn open_conversation_search_dialog(window: &mut Window, cx: &mut App) {
    let title = cx.global::<I18n>().t("sidebar-search-title");
    let view = cx.new(|cx| ConversationSearchView::new(window, cx));
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
                this.hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            })
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .on_click(move |_, window, cx| {
                on_confirm(conversation_id.clone(), window, cx);
            })
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().border.opacity(0.35))
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
            (self.on_confirm)(result.conversation.id.clone(), window, cx);
        }
    }
}

pub(crate) struct ConversationSearchView {
    search_input: Entity<InputState>,
    results: Entity<ListState<ConversationSearchDelegate>>,
    _search_input_subscription: Subscription,
}

impl ConversationSearchView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("sidebar-search-placeholder"))
        });
        let _search_input_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let results = Self::build_list("", window, cx);

        Self {
            search_input,
            results,
            _search_input_subscription,
        }
    }

    fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input
            .update(cx, |search_input, cx| search_input.focus(window, cx));
    }

    fn build_list(
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ListState<ConversationSearchDelegate>> {
        let no_project_label = cx
            .global::<I18n>()
            .t("sidebar-section-no-project-conversations")
            .into();
        let items = state::workspace::workspace(cx)
            .read(cx)
            .search_conversations(query, SEARCH_RESULT_LIMIT, cx)
            .unwrap_or_default();
        let on_confirm: OnConfirm = Rc::new(|conversation_id, window, cx| {
            state::workspace::workspace(cx).update(cx, |workspace, cx| {
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
        self.results = Self::build_list(&self.current_query(cx), window, cx);
        cx.notify();
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
            .map(|this| {
                if count == 0 {
                    this.child(
                        v_flex().flex_1().items_center().justify_center().child(
                            Label::new(no_results)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                    )
                } else {
                    this.child(List::new(&self.results).large().flex_1())
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
