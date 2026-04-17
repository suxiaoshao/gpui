use crate::{
    assets::IconName,
    i18n::I18n,
    state::{ChatData, ConversationSearchResult, WorkspaceStore},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Selectable, Sizable, WindowExt, h_flex,
    input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
    label::Label,
    list::{List, ListDelegate, ListState},
    v_flex,
};
use std::{ops::Deref, rc::Rc};

use super::HOME_CONTEXT;

actions!(conversation_search, [OpenConversationSearch]);

const CONTEXT: &str = "conversation_search_view";
const SEARCH_ITEM_HEIGHT: f32 = 48.;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        if cfg!(target_os = "macos") {
            "cmd-f"
        } else {
            "ctrl-f"
        },
        OpenConversationSearch,
        Some(HOME_CONTEXT),
    )]);
}

pub(crate) fn open_conversation_search_dialog(window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| ConversationSearchView::new(window, cx));
    let view_to_focus = view.clone();
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
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
    result: Rc<ConversationSearchResult>,
    root_label: SharedString,
    is_selected: bool,
}

impl ConversationSearchItem {
    fn new(result: Rc<ConversationSearchResult>, root_label: SharedString) -> Self {
        Self {
            result,
            root_label,
            is_selected: false,
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
        let path_label = result.path_label(&self.root_label);
        h_flex()
            .id(("conversation-search-result", result.id as usize))
            .w_full()
            .gap_3()
            .h(px(SEARCH_ITEM_HEIGHT))
            .items_center()
            .px_3()
            .py_2()
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().accent.alpha(0.7)))
            })
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().border.opacity(0.35))
                    .child(Label::new(result.icon.clone()).text_base()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_none()
                                    .max_w(relative(0.45))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .truncate()
                                    .child(Label::new(result.title.clone()).text_sm()),
                            )
                            .when_some(result.info.as_ref(), |this, info| {
                                this.child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .truncate()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(Label::new(info.clone()).text_xs()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(Label::new(path_label).text_xs()),
                    ),
            )
    }
}

type OnConfirm = Rc<dyn Fn(i32, &mut Window, &mut App) + 'static>;

struct ConversationSearchDelegate {
    ix: Option<IndexPath>,
    items: Vec<Rc<ConversationSearchResult>>,
    root_label: SharedString,
    on_confirm: OnConfirm,
}

impl ConversationSearchDelegate {
    fn new(
        items: Vec<ConversationSearchResult>,
        root_label: SharedString,
        on_confirm: OnConfirm,
    ) -> Self {
        Self {
            ix: None,
            items: items.into_iter().map(Rc::new).collect(),
            root_label,
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
        self.items
            .get(ix.row)
            .cloned()
            .map(|result| ConversationSearchItem::new(result, self.root_label.clone()))
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
            (self.on_confirm)(result.id, window, cx);
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
                .placeholder(cx.global::<I18n>().t("field-search-conversation"))
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
        let root_label = cx.global::<I18n>().t("sidebar-root").into();
        let items = cx
            .global::<ChatData>()
            .read(cx)
            .as_ref()
            .ok()
            .map(|data| data.search_conversations(query))
            .unwrap_or_default();
        let on_confirm: OnConfirm = Rc::new(|conversation_id, window, cx| {
            cx.global::<WorkspaceStore>()
                .deref()
                .clone()
                .update(cx, |workspace, cx| {
                    workspace.add_conversation_tab(conversation_id, window, cx);
                });
            window.close_dialog(cx);
        });
        cx.new(move |cx| {
            let mut state = ListState::new(
                ConversationSearchDelegate::new(items, root_label, on_confirm),
                window,
                cx,
            );
            let has_items = state.delegate().items_count(0, cx) > 0;
            state.set_selected_index(has_items.then_some(IndexPath::default()), window, cx);
            state.scroll_to_item(IndexPath::default(), ScrollStrategy::Top, window, cx);
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
            let count = state.delegate().items.len();
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
        });
        cx.notify();
    }

    fn on_search_enter(&mut self, enter: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }

        self.results.update(cx, |state, cx| {
            let selected = state.selected_index();
            state
                .delegate_mut()
                .set_selected_index(selected, window, cx);
            state.delegate_mut().confirm(enter.secondary, window, cx);
        });
        cx.stop_propagation();
    }
}

impl Render for ConversationSearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let no_results = cx.global::<I18n>().t("conversation-search-no-results");
        let count = self.results.read(cx).delegate().items.len();
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
                        v_flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .child(Label::new(no_results).text_sm()),
                    )
                } else {
                    this.child(List::new(&self.results).large().flex_1())
                }
            })
    }
}
