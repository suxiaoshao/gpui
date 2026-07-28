pub(crate) mod list;
pub(crate) mod new_conversation;
pub(crate) mod search;

use std::collections::HashMap;
use std::rc::Rc;

use self::search::TemporaryConversationNode;
use crate::{
    app::{menus, temporary_window},
    components::{
        chat::detail::ConversationDetailPage,
        chat::input::{COMPOSER_EDITOR_KEY_CONTEXT, ChatInputSubmit},
        resource::{CriticalResourceAction, CriticalResourceProblem, CriticalResourcesView},
    },
    database::DatabaseResource,
    features::conversation,
    foundation::{I18n, assets::IconName},
    state,
};
use gpui::{actions, prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IndexPath, Root, Sizable, WindowExt as _,
    button::Button,
    h_flex,
    input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
    label::Label,
    list::{List, ListState},
    notification::{Notification, NotificationType},
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition};
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
    search_operation: search::TemporarySearchOperation,
    selected_index: Option<usize>,
    new_conversation: Entity<TemporaryNewConversationPane>,
    conversation_pages: HashMap<ConversationId, Entity<ConversationDetailPage>>,
    runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
    _theme_binding: state::theme::WindowThemeBinding,
    _subscriptions: Vec<Subscription>,
}

impl TemporaryWindow {
    pub(crate) fn new(
        runtime: Entity<conversation::runtime::ConversationRuntimeStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("temporary-search-placeholder"))
        });
        let mut search_operation = search::TemporarySearchOperation::new();
        search_operation.transition(Load(Task::ready(())));
        search_operation.transition(Complete(
            search::empty_snapshot(cx).map_err(search::TemporarySearchProblem::from),
        ));
        let conversations = search_operation
            .data()
            .map(|snapshot| snapshot.conversations.clone())
            .unwrap_or_default();
        let search_error = search_operation.problem().map(ToString::to_string);
        let selected_index = (!conversations.is_empty()).then_some(0);
        let route = selected_index
            .and_then(|index| conversations.get(index))
            .map(|conversation| TemporaryWindowRoute::Conversation(conversation.id.clone()))
            .unwrap_or(TemporaryWindowRoute::NewConversation);
        let list = Self::build_list(
            conversations.clone(),
            "",
            search_error.as_deref(),
            selected_index,
            window,
            cx,
        );
        let new_conversation = cx.new(|cx| TemporaryNewConversationPane::new(window, cx));
        let database_store = crate::database::store(cx);
        let project_catalog = state::projects::catalog(cx);
        let conversation_catalog =
            crate::features::conversation::resources::ready_conversations(cx)
                .expect("temporary window requires a ready conversation session")
                .read(cx)
                .catalog();
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

        let project_subscription = project_catalog.observe_in(cx, window, |view, _, window, cx| {
            view.sync_new_conversation_capability(cx);
            if view.query.is_empty() {
                view.reload_conversations(ReloadSelection::FirstMatch, window, cx);
            }
        });
        let conversation_subscription =
            cx.observe_in(&conversation_catalog, window, |view, _, window, cx| {
                view.sync_new_conversation_capability(cx);
                if view.query.is_empty() {
                    view.reload_conversations(ReloadSelection::FirstMatch, window, cx);
                }
            });
        let runtime_subscription = cx.observe_in(&runtime, window, |view, _, _window, cx| {
            view.sync_new_conversation_capability(cx);
        });

        let mut view = Self {
            focus_handle,
            search_input,
            list,
            query: String::new(),
            route,
            search_operation,
            selected_index,
            new_conversation,
            conversation_pages: HashMap::new(),
            runtime,
            _subscriptions: vec![
                search_subscription,
                new_conversation_subscription,
                project_subscription,
                conversation_subscription,
                runtime_subscription,
                cx.observe_window_activation(window, |this, window, cx| {
                    if window.is_window_active() {
                        this.focus_search_input(window, cx);
                    } else {
                        temporary_window::request_hide_for_window_activation(window, cx);
                    }
                }),
                cx.observe_global_in::<I18n>(window, |this, window, cx| {
                    this.search_input.update(cx, |input, cx| {
                        input.set_placeholder(
                            cx.global::<I18n>().t("temporary-search-placeholder"),
                            window,
                            cx,
                        );
                    });
                    cx.refresh_windows();
                }),
                database_store.observe_in(cx, window, move |_view, _resource, window, cx| {
                    if !crate::database::is_ready(cx) {
                        window.remove_window();
                        crate::app::show_or_create_main_window(cx);
                    } else {
                        cx.notify();
                    }
                }),
            ],
            _theme_binding: state::theme::WindowThemeBinding::new(window, cx),
        };
        view.sync_new_conversation_capability(cx);
        view
    }

    fn sync_new_conversation_capability(&mut self, cx: &mut Context<Self>) {
        let project_problem = state::projects::catalog(cx).read(cx, |operation| {
            (!matches!(operation, state::projects::ProjectOperation::Ready(_))).then(|| {
                operation
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| cx.global::<I18n>().t("resource-status-loading"))
            })
        });
        let conversation_problem = crate::features::conversation::resources::ready_conversations(
            cx,
        )
        .and_then(|registry| {
            let catalog = registry.read(cx).catalog();
            let operation = catalog.read(cx).operation();
            (!matches!(
                operation,
                conversation::registry::ConversationCatalogOperation::Ready(_)
            ))
            .then(|| {
                operation
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| cx.global::<I18n>().t("resource-status-loading"))
            })
        });
        let runtime_problem = {
            let runtime = self.runtime.read(cx);
            let recovery = runtime.recovery();
            (!matches!(recovery, gpui_operation::refresh::Operation::Ready(_))).then(|| {
                recovery
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| cx.global::<I18n>().t("conversation-runtime-recovering"))
            })
        };
        let problem = project_problem
            .or(conversation_problem)
            .or(runtime_problem)
            .map(Into::into);
        self.new_conversation.update(cx, |pane, cx| {
            pane.set_submission_problem(problem, cx);
        });
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
        let conversations = self.conversations().to_vec();
        let error = self.search_operation.problem().map(ToString::to_string);
        self.list = Self::build_list(
            conversations,
            &self.query,
            error.as_deref(),
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
        if self.search_operation.is_running() {
            self.search_operation.transition(Cancel);
        }
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
        if self.search_operation.is_running() {
            return;
        }
        if self.query.is_empty() {
            let result = search::empty_snapshot(cx).map_err(search::TemporarySearchProblem::from);
            self.search_operation.transition(Refresh(Task::ready(())));
            self.search_operation.transition(Complete(result));
            self.apply_reload_selection(selection);
            self.rebuild_list(window, cx);
            cx.notify();
            return;
        }
        let query = self.query.clone();
        let search = search::search(query.clone(), cx);
        let page = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = search.await;
            let _ = page.update_in(cx, |page, window, cx| {
                if page.query != query || !page.search_operation.is_running() {
                    return;
                }
                page.search_operation.transition(Complete(result));
                if page.search_operation.data().is_some() {
                    page.apply_reload_selection(selection);
                } else {
                    page.selected_index = None;
                }
                page.rebuild_list(window, cx);
                cx.notify();
            });
        });
        match &self.search_operation {
            search::TemporarySearchOperation::Idle(_) => {
                self.search_operation.transition(Load(task))
            }
            search::TemporarySearchOperation::Ready(_)
            | search::TemporarySearchOperation::Degraded(_) => {
                self.search_operation.transition(Refresh(task))
            }
            search::TemporarySearchOperation::Unavailable(_) => {
                self.search_operation.transition(Retry(task))
            }
            search::TemporarySearchOperation::Loading(_)
            | search::TemporarySearchOperation::Refreshing(_)
            | search::TemporarySearchOperation::Retrying(_)
            | search::TemporarySearchOperation::RefreshingDegraded(_) => {}
        }
        self.rebuild_list(window, cx);
        cx.notify();
    }

    fn conversations(&self) -> &[TemporaryConversationNode] {
        self.search_operation
            .data()
            .map(|snapshot| snapshot.conversations.as_slice())
            .unwrap_or_default()
    }

    fn apply_reload_selection(&mut self, selection: ReloadSelection) {
        match selection {
            ReloadSelection::FirstMatch => {
                self.selected_index = (!self.conversations().is_empty()).then_some(0);
            }
            ReloadSelection::Conversation(conversation_id) => {
                self.selected_index = self
                    .conversations()
                    .iter()
                    .position(|conversation| conversation.id == conversation_id);
            }
        }

        if let Some(index) = self.selected_index
            && let Some(conversation) = self.conversations().get(index)
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
        if index >= self.conversations().len() {
            return;
        }
        self.selected_index = Some(index);
        self.route = TemporaryWindowRoute::Conversation(self.conversations()[index].id.clone());
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
            selection_after_delta(self.selected_index, self.conversations().len(), delta)
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
        let request = conversation::CreateConversationRequest {
            project_id: None,
            content_parts: submit.composer.content_parts.clone(),
            attachments: submit.attachments.clone(),
            title_seed: submit.composer.text.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
            approval_mode: submit.approval_mode,
            prompt_id: None,
            prompt_snapshot: None,
            trigger_kind: jaco_core::AgentRunTriggerKind::User,
        };

        let task = conversation::create_conversation(request, cx);
        let page = cx.entity().downgrade();
        let completion = window.spawn(cx, async move |cx| {
            let result = task.await;
            let _ = page.update_in(cx, |page, window, cx| match result {
                Ok(created) => {
                    let conversation_id = created.conversation_id.clone();
                    page.new_conversation.update(cx, |pane, cx| {
                        pane.clear_after_submit(window, cx);
                    });
                    if page.search_operation.is_running() {
                        page.search_operation.transition(Cancel);
                    }
                    page.query.clear();
                    page.search_input.update(cx, |input, cx| {
                        if !input.value().is_empty() {
                            input.set_value("", window, cx);
                        }
                    });
                    page.reload_conversations(
                        ReloadSelection::Conversation(conversation_id.clone()),
                        window,
                        cx,
                    );
                    page.route = TemporaryWindowRoute::Conversation(conversation_id.clone());
                    let _ = page.conversation_page(conversation_id.clone(), window, cx);
                    let start = page.runtime.update(cx, |runtime, cx| {
                        runtime.start_run(created.run_request, window, cx)
                    });
                    if let Err(error) = start {
                        let title = cx.global::<I18n>().t("conversation-run-failed");
                        push_temporary_notification(
                            window,
                            cx,
                            title,
                            error,
                            NotificationType::Error,
                        );
                    }
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
            });
        });
        crate::app::tasks::retain_window(window, completion, cx);
    }

    pub(crate) fn open_created_conversation(
        &mut self,
        created: conversation::CreatedConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let conversation_id = created.conversation_id.clone();
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
        let start = self.runtime.update(cx, |runtime, cx| {
            runtime.start_run(created.run_request, window, cx)
        });
        if let Err(error) = start {
            let title = cx.global::<I18n>().t("conversation-run-failed");
            push_temporary_notification(window, cx, title, error, NotificationType::Error);
            return false;
        }
        true
    }

    fn conversation_page(
        &mut self,
        conversation_id: ConversationId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ConversationDetailPage> {
        if let Some(page) = self.conversation_pages.get(&conversation_id) {
            return page.clone();
        }
        let registry = crate::features::conversation::resources::ready_conversations(cx)
            .expect("conversation page requires ready conversation resources");
        let conversation = registry.update(cx, |registry, cx| {
            registry.conversation(conversation_id.clone(), cx)
        });
        // Keep search focus while the route is materialized after an
        // arrow-key selection.
        let runtime = self.runtime.clone();
        let page = cx
            .new(|cx| ConversationDetailPage::new_without_focus(conversation, runtime, window, cx));
        self.conversation_pages
            .insert(conversation_id, page.clone());
        page
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
        let last_error = self.search_operation.problem().map(ToString::to_string);
        let running = self.search_operation.is_running();
        let view = cx.entity().downgrade();

        v_flex()
            .id("temporary-conversation-list-panel")
            .size_full()
            .min_w_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.sidebar.background)
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
                        )
                        .child(
                            Button::new("temporary-reload-conversations")
                                .label(cx.global::<I18n>().t("resource-status-refresh"))
                                .loading(running)
                                .disabled(running)
                                .on_click(move |_, window, cx| {
                                    let _ = view.update(cx, |view, cx| {
                                        view.reload_conversations(
                                            ReloadSelection::FirstMatch,
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        ),
                )
            })
            .when(
                running && self.search_operation.problem().is_none(),
                |this| {
                    this.child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(gpui_component::spinner::Spinner::new().small())
                            .child(
                                Label::new(cx.global::<I18n>().t("resource-status-loading"))
                                    .text_xs(),
                            ),
                    )
                },
            )
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

        let content = v_flex()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().tokens.background.background)
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
            .children(notification_layer);
        if crate::database::is_ready(cx) {
            content.into_any_element()
        } else {
            temporary_database_resource_view(cx).overlay(content, cx)
        }
    }
}

fn temporary_database_resource_view(cx: &App) -> CriticalResourcesView {
    let snapshot = crate::database::store(cx).read(cx, |resource| match resource {
        DatabaseResource::AwaitingConfig => None,
        DatabaseResource::Bound { operation, .. } => Some((
            operation.is_running(),
            operation.problem().map(ToString::to_string),
            operation
                .problem()
                .is_some_and(crate::database::DatabaseProblem::can_create_fresh),
        )),
    });
    let Some((running, message, can_create_fresh)) = snapshot else {
        return CriticalResourcesView::loading(cx.global::<I18n>().t("critical-database-loading"));
    };
    let mut actions = vec![CriticalResourceAction::RefreshDatabase];
    if can_create_fresh {
        actions.push(CriticalResourceAction::BackupAndCreateFreshDatabase);
    }
    CriticalResourcesView::problem(CriticalResourceProblem {
        id: "temporary-critical-database",
        title: cx
            .global::<I18n>()
            .t("critical-database-error-title")
            .into(),
        message: message
            .unwrap_or_else(|| cx.global::<I18n>().t("critical-read-only-description"))
            .into(),
        running,
        warning: true,
        actions,
    })
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
