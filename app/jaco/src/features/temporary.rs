pub(crate) mod list;
pub(crate) mod new_conversation;

use std::collections::HashMap;
use std::rc::Rc;

use crate::{
    app::{menus, temporary_window},
    components::{
        chat_input::{COMPOSER_EDITOR_KEY_CONTEXT, ChatInputSubmit},
        conversation_detail::ConversationDetailPage,
    },
    foundation::{self, I18n, assets::IconName},
    state,
    state::temporary::TemporaryConversationNode,
};
use gpui::{actions, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Root, Sizable, WindowExt as _, h_flex,
    input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
    label::Label,
    list::{List, ListState},
    notification::{Notification, NotificationType},
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
use jaco_core::ConversationId;
use new_conversation::{TemporaryNewConversationPane, TemporaryNewConversationPaneEvent};

use self::list::TemporaryConversationListDelegate;

pub(crate) const KEY_CONTEXT: &str = "JacoTemporaryWindow";
const TEMPORARY_LEFT_PANEL_WIDTH: f32 = 280.;
const TEMPORARY_LEFT_PANEL_MIN_WIDTH: f32 = 220.;
const TEMPORARY_LEFT_PANEL_MAX_WIDTH: f32 = 420.;

actions!(
    jaco_temporary,
    [
        OpenTemporaryNewConversation,
        ToggleTemporaryInputFocus,
        FocusTemporarySearch
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            "secondary-n",
            OpenTemporaryNewConversation,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new("tab", ToggleTemporaryInputFocus, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "tab",
            ToggleTemporaryInputFocus,
            Some(COMPOSER_EDITOR_KEY_CONTEXT),
        ),
        KeyBinding::new("secondary-f", FocusTemporarySearch, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TemporaryWindowRoute {
    NewConversation,
    Conversation(ConversationId),
}

pub(crate) struct TemporaryWindow {
    focus_handle: FocusHandle,
    search_input: Entity<InputState>,
    list: Entity<ListState<TemporaryConversationListDelegate>>,
    query: String,
    route: TemporaryWindowRoute,
    conversations: Vec<TemporaryConversationNode>,
    selected_index: Option<usize>,
    new_conversation: Entity<TemporaryNewConversationPane>,
    conversation_pages: HashMap<ConversationId, Entity<ConversationDetailPage>>,
    runtime: Entity<state::conversation_runtime::ConversationRuntimeStore>,
    last_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl TemporaryWindow {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        state::theme::apply_current_theme(window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("temporary-search-placeholder"))
        });
        let (conversations, last_error) = load_no_project_conversations("", cx);
        let selected_index = (!conversations.is_empty()).then_some(0);
        let route = selected_index
            .and_then(|index| conversations.get(index))
            .map(|conversation| TemporaryWindowRoute::Conversation(conversation.id.clone()))
            .unwrap_or(TemporaryWindowRoute::NewConversation);
        let list = Self::build_list(
            conversations.clone(),
            "",
            last_error.as_deref(),
            selected_index,
            window,
            cx,
        );
        let new_conversation = cx.new(|cx| TemporaryNewConversationPane::new(window, cx));
        let runtime = state::conversation_runtime::runtime(cx);
        let config_store = state::config::store(cx);
        let search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let new_conversation_subscription = cx.subscribe_in(
            &new_conversation,
            window,
            |view, _pane, event: &TemporaryNewConversationPaneEvent, window, cx| match event {
                TemporaryNewConversationPaneEvent::SendRequested(submit) => {
                    view.submit_new_conversation((**submit).clone(), window, cx);
                }
            },
        );

        Self {
            focus_handle,
            search_input,
            list,
            query: String::new(),
            route,
            conversations,
            selected_index,
            new_conversation,
            conversation_pages: HashMap::new(),
            runtime,
            last_error,
            _subscriptions: vec![
                search_subscription,
                new_conversation_subscription,
                cx.observe_window_activation(window, |this, window, cx| {
                    if window.is_window_active() {
                        this.focus_search_input(window, cx);
                    } else {
                        temporary_window::request_hide_for_window_activation(window, cx);
                    }
                }),
                cx.observe_window_appearance(window, |_state, window, cx| {
                    state::theme::apply_current_theme(window, cx);
                    cx.refresh_windows();
                }),
                cx.observe_global_in::<state::theme::SystemAccentThemeState>(
                    window,
                    |_state, window, cx| {
                        state::theme::apply_current_theme(window, cx);
                        cx.refresh_windows();
                    },
                ),
                config_store.observe_select_in(
                    cx,
                    window,
                    |config| {
                        (
                            config.app_settings.language,
                            config.app_settings.theme.clone(),
                        )
                    },
                    |this, _settings, window, cx| {
                        foundation::init_i18n(cx);
                        menus::sync_app_menus(cx);
                        state::theme::apply_current_theme(window, cx);
                        this.search_input.update(cx, |input, cx| {
                            input.set_placeholder(
                                cx.global::<I18n>().t("temporary-search-placeholder"),
                                window,
                                cx,
                            );
                        });
                        cx.refresh_windows();
                    },
                ),
            ],
        }
    }

    pub(crate) fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input
            .update(cx, |search_input, cx| search_input.focus(window, cx));
    }

    fn build_list(
        conversations: Vec<TemporaryConversationNode>,
        query: &str,
        last_error: Option<&str>,
        selected_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ListState<TemporaryConversationListDelegate>> {
        let state = cx.entity().downgrade();
        let on_select = Rc::new(move |index: usize, window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |view, cx| {
                view.select_conversation_index(index, window, cx);
            });
        });
        let empty_label = cx.global::<I18n>().t("temporary-empty-conversations");
        let no_results_label = cx.global::<I18n>().t("temporary-no-results");
        let error_label = last_error.map(|error| {
            format!(
                "{}: {}",
                cx.global::<I18n>().t("temporary-load-failed"),
                error
            )
            .into()
        });
        let has_query = !query.trim().is_empty();

        cx.new(move |cx| {
            let mut list = ListState::new(
                TemporaryConversationListDelegate::new(
                    conversations,
                    has_query,
                    empty_label.into(),
                    no_results_label.into(),
                    error_label,
                    on_select,
                ),
                window,
                cx,
            );
            if let Some(index) = selected_index {
                let ix = IndexPath::default().row(index);
                list.set_selected_index(Some(ix), window, cx);
                list.scroll_to_item(ix, ScrollStrategy::Top, window, cx);
            }
            list
        })
    }

    fn rebuild_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.list = Self::build_list(
            self.conversations.clone(),
            &self.query,
            self.last_error.as_deref(),
            self.selected_index,
            window,
            cx,
        );
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
        self.reload_conversations(ReloadSelection::FirstMatch, window, cx);
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn reload_conversations(
        &mut self,
        selection: ReloadSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match state::temporary::load_no_project_conversations(&self.query, cx) {
            Ok(snapshot) => {
                self.conversations = snapshot.conversations;
                self.last_error = None;
                self.apply_reload_selection(selection);
            }
            Err(err) => {
                self.last_error = Some(err.to_string());
                self.selected_index = None;
            }
        }
        self.rebuild_list(window, cx);
        cx.notify();
    }

    fn apply_reload_selection(&mut self, selection: ReloadSelection) {
        match selection {
            ReloadSelection::FirstMatch => {
                self.selected_index = (!self.conversations.is_empty()).then_some(0);
            }
            ReloadSelection::Conversation(conversation_id) => {
                self.selected_index = self
                    .conversations
                    .iter()
                    .position(|conversation| conversation.id == conversation_id);
            }
        }

        if let Some(index) = self.selected_index
            && let Some(conversation) = self.conversations.get(index)
        {
            self.route = TemporaryWindowRoute::Conversation(conversation.id.clone());
        } else if !matches!(self.route, TemporaryWindowRoute::Conversation(_)) {
            self.route = TemporaryWindowRoute::NewConversation;
        }
    }

    fn select_conversation_index(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.conversations.len() {
            return;
        }
        self.selected_index = Some(index);
        self.route = TemporaryWindowRoute::Conversation(self.conversations[index].id.clone());
        self.sync_list_selection(window, cx);
        cx.notify();
    }

    fn sync_list_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self
            .selected_index
            .map(|index| IndexPath::default().row(index));
        self.list.update(cx, |list, cx| {
            list.set_selected_index(selected, window, cx);
            if let Some(ix) = selected {
                list.scroll_to_item(ix, ScrollStrategy::Top, window, cx);
            }
        });
    }

    fn move_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(next) =
            selection_after_delta(self.selected_index, self.conversations.len(), delta)
        else {
            self.selected_index = None;
            self.sync_list_selection(window, cx);
            cx.notify();
            return;
        };
        self.select_conversation_index(next, window, cx);
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

    fn on_search_enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }
        if let Some(index) = self.selected_index {
            self.select_conversation_index(index, window, cx);
        }
        cx.stop_propagation();
    }

    fn open_new_conversation(
        &mut self,
        _: &OpenTemporaryNewConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.route = TemporaryWindowRoute::NewConversation;
        self.new_conversation
            .update(cx, |pane, cx| pane.focus_primary(window, cx));
        cx.notify();
        cx.stop_propagation();
    }

    fn toggle_input_focus(
        &mut self,
        _: &ToggleTemporaryInputFocus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match tab_focus_target(self.search_input.focus_handle(cx).is_focused(window)) {
            TemporaryTabTarget::RouteComposer => self.focus_route_composer(window, cx),
            TemporaryTabTarget::Search => self.focus_search_input(window, cx),
        }
        cx.stop_propagation();
    }

    fn focus_search(
        &mut self,
        _: &FocusTemporarySearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_search_input(window, cx);
        cx.stop_propagation();
    }

    fn focus_route_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.route.clone() {
            TemporaryWindowRoute::NewConversation => {
                self.new_conversation
                    .update(cx, |pane, cx| pane.focus_primary(window, cx));
            }
            TemporaryWindowRoute::Conversation(conversation_id) => {
                let page = self.conversation_page(conversation_id, window, cx);
                page.update(cx, |page, cx| page.focus_primary(window, cx));
            }
        }
    }

    fn submit_new_conversation(
        &mut self,
        submit: ChatInputSubmit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let request = state::conversations::CreateConversationRequest {
            project_id: None,
            content_parts: submit.composer.content_parts.clone(),
            attachments: submit.composer.attachments.clone(),
            title_seed: submit.composer.text.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
            approval_mode: submit.approval_mode,
            prompt_id: None,
            prompt_snapshot: None,
            trigger_kind: jaco_core::AgentRunTriggerKind::User,
        };

        match state::conversations::create_conversation(request, cx) {
            Ok(created) => {
                let conversation_id = created.record.conversation.id.clone();
                self.new_conversation.update(cx, |pane, cx| {
                    pane.clear_after_submit(window, cx);
                });
                self.query.clear();
                self.search_input.update(cx, |input, cx| {
                    if !input.value().is_empty() {
                        input.set_value("", window, cx);
                    }
                });
                self.reload_conversations(
                    ReloadSelection::Conversation(conversation_id.clone()),
                    window,
                    cx,
                );
                self.route = TemporaryWindowRoute::Conversation(conversation_id.clone());
                let _ = self.conversation_page(conversation_id.clone(), window, cx);
                state::workspace::workspace(cx).update(cx, |workspace, cx| {
                    workspace.reload_sidebar(cx);
                });
                self.runtime.update(cx, |runtime, cx| {
                    runtime.start_run(created.run_request, window, cx);
                });
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("temporary-submit-failed");
                push_temporary_notification(
                    window,
                    cx,
                    title,
                    err.to_string(),
                    NotificationType::Error,
                );
            }
        }
    }

    pub(crate) fn open_created_conversation(
        &mut self,
        created: state::conversations::CreatedConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let conversation_id = created.record.conversation.id.clone();
        self.query.clear();
        self.search_input.update(cx, |input, cx| {
            if !input.value().is_empty() {
                input.set_value("", window, cx);
            }
        });
        self.reload_conversations(
            ReloadSelection::Conversation(conversation_id.clone()),
            window,
            cx,
        );
        self.route = TemporaryWindowRoute::Conversation(conversation_id.clone());
        let _ = self.conversation_page(conversation_id.clone(), window, cx);
        state::workspace::workspace(cx).update(cx, |workspace, cx| {
            workspace.reload_sidebar(cx);
        });
        self.runtime.update(cx, |runtime, cx| {
            runtime.start_run(created.run_request, window, cx)
        })
    }

    fn conversation_page(
        &mut self,
        conversation_id: ConversationId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ConversationDetailPage> {
        self.conversation_pages
            .entry(conversation_id.clone())
            .or_insert_with(|| {
                // Keep search focus while the route is materialized after an
                // arrow-key selection.
                cx.new(|cx| ConversationDetailPage::new_without_focus(conversation_id, window, cx))
            })
            .clone()
    }

    fn minimize(&mut self, _: &menus::Minimize, window: &mut Window, _: &mut Context<Self>) {
        window.minimize_window();
    }

    fn zoom(&mut self, _: &menus::Zoom, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }

    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Input::new(&self.search_input)
            .w_full()
            .appearance(false)
            .p_0()
            .bordered(false)
            .focus_bordered(false)
            .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
            .cleanable(true)
    }

    fn render_left_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let last_error = self.last_error.clone();

        v_flex()
            .id("temporary-conversation-list-panel")
            .size_full()
            .min_w_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .when_some(last_error, |this, error| {
                this.child(
                    h_flex()
                        .items_start()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .text_color(cx.theme().danger)
                        .child(Icon::new(IconName::CircleAlert).size_4().flex_none())
                        .child(
                            Label::new(format!(
                                "{}: {}",
                                cx.global::<I18n>().t("temporary-load-failed"),
                                error
                            ))
                            .text_xs(),
                        ),
                )
            })
            .child(List::new(&self.list).large().flex_1())
    }

    fn render_right_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.route.clone() {
            TemporaryWindowRoute::NewConversation => {
                self.new_conversation.clone().into_any_element()
            }
            TemporaryWindowRoute::Conversation(conversation_id) => self
                .conversation_page(conversation_id, window, cx)
                .into_any_element(),
        }
    }
}

impl Focusable for TemporaryWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TemporaryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = cx.global::<I18n>().t("temporary-window-title");
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        window.set_window_title(&title);

        v_flex()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::minimize))
            .on_action(cx.listener(Self::zoom))
            .on_action(cx.listener(Self::on_search_move_up))
            .on_action(cx.listener(Self::on_search_move_down))
            .on_action(cx.listener(Self::on_search_enter))
            .on_action(cx.listener(Self::open_new_conversation))
            .on_action(cx.listener(Self::toggle_input_focus))
            .on_action(cx.listener(Self::focus_search))
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(self.render_search(cx)),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    h_resizable("jaco-temporary-layout")
                        .child(
                            resizable_panel()
                                .size(px(TEMPORARY_LEFT_PANEL_WIDTH))
                                .size_range(
                                    px(TEMPORARY_LEFT_PANEL_MIN_WIDTH)
                                        ..px(TEMPORARY_LEFT_PANEL_MAX_WIDTH),
                                )
                                .child(self.render_left_panel(cx)),
                        )
                        .child(
                            resizable_panel().child(
                                div()
                                    .size_full()
                                    .min_w_0()
                                    .child(self.render_right_panel(window, cx)),
                            ),
                        ),
                ),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ReloadSelection {
    FirstMatch,
    Conversation(ConversationId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemporaryTabTarget {
    RouteComposer,
    Search,
}

fn tab_focus_target(search_focused: bool) -> TemporaryTabTarget {
    if search_focused {
        TemporaryTabTarget::RouteComposer
    } else {
        TemporaryTabTarget::Search
    }
}

fn load_no_project_conversations(
    query: &str,
    cx: &App,
) -> (Vec<TemporaryConversationNode>, Option<String>) {
    match state::temporary::load_no_project_conversations(query, cx) {
        Ok(snapshot) => (snapshot.conversations, None),
        Err(err) => (Vec::new(), Some(err.to_string())),
    }
}

fn selection_after_delta(current: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }

    let current = current.unwrap_or(0).min(count - 1);
    Some(if delta < 0 {
        if current == 0 { count - 1 } else { current - 1 }
    } else if current + 1 >= count {
        0
    } else {
        current + 1
    })
}

fn push_temporary_notification(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    notification_type: NotificationType,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(message.into())
            .with_type(notification_type),
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::{TemporaryTabTarget, selection_after_delta, tab_focus_target};

    #[test]
    fn temporary_selection_wraps_up_and_down() {
        assert_eq!(selection_after_delta(Some(0), 3, -1), Some(2));
        assert_eq!(selection_after_delta(Some(2), 3, 1), Some(0));
        assert_eq!(selection_after_delta(Some(1), 3, -1), Some(0));
        assert_eq!(selection_after_delta(Some(1), 3, 1), Some(2));
    }

    #[test]
    fn temporary_selection_handles_empty_list() {
        assert_eq!(selection_after_delta(None, 0, 1), None);
    }

    #[test]
    fn tab_toggles_between_search_and_route_composer() {
        assert_eq!(tab_focus_target(true), TemporaryTabTarget::RouteComposer);
        assert_eq!(tab_focus_target(false), TemporaryTabTarget::Search);
    }
}
