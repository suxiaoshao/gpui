use std::rc::Rc;

use super::super::workspace::{HomeWorkspace, SidebarSearchLoad, SidebarSearchResult};
use crate::foundation::{I18n, assets::IconName};
use fluent_bundle::FluentArgs;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable, WindowExt,
    button::Button,
    command::{Command, CommandItem, CommandState},
    h_flex,
    label::Label,
    v_flex,
};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use jaco_core::ConversationId;

const SEARCH_RESULT_LIMIT: usize = 50;

type SearchConversations =
    Rc<dyn Fn(String, usize, &mut App) -> Task<jaco_db::Result<SidebarSearchLoad>> + 'static>;
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

pub(crate) struct ConversationSearchView {
    search: SearchConversations,
    on_confirm: OnConfirm,
    command: Entity<CommandState>,
    results: Vec<Rc<SidebarSearchResult>>,
    query: String,
    operation: refresh::Operation<Vec<SidebarSearchResult>, jaco_db::DbError, Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ConversationSearchView {
    fn new(workspace: Entity<HomeWorkspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace_subscription = cx.observe_in(&workspace, window, |view, _, window, cx| {
            if view.query.is_empty() && !view.operation.is_running() {
                view.reload(window, cx);
            }
        });
        let search_workspace = workspace.clone();
        let search: SearchConversations = Rc::new(move |query, limit, cx| {
            search_workspace.update(cx, |workspace, cx| {
                workspace.search_conversations(query, limit, cx)
            })
        });
        let on_confirm: OnConfirm = Rc::new(move |conversation_id, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.open_conversation(conversation_id, cx);
            });
            window.close_dialog(cx);
        });

        Self::new_with_owners(search, on_confirm, vec![workspace_subscription], window, cx)
    }

    fn new_with_owners(
        search: SearchConversations,
        on_confirm: OnConfirm,
        subscriptions: Vec<Subscription>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let command = cx.new(|cx| CommandState::new(window, cx));
        let view = Self {
            search,
            on_confirm,
            command,
            results: Vec::new(),
            query: String::new(),
            operation: refresh::Operation::new(),
            _subscriptions: subscriptions,
        };
        let entity = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = entity.update(cx, |view, cx| view.reload(window, cx));
        });
        view
    }

    fn focus_search_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.command
            .update(cx, |command, cx| command.focus(window, cx));
    }

    fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.query = normalize_query(query);
        if self.operation.is_running() {
            self.operation.transition(Cancel);
        }
        self.reload(window, cx);
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.query.clone();
        let search = (self.search)(query.clone(), SEARCH_RESULT_LIMIT, cx);
        let view = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            let result = search.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.query != query || !view.operation.is_running() {
                    return;
                }
                view.complete_load(result);
                view.results = view
                    .operation
                    .data()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Rc::new)
                    .collect();
                view.sync_loading(window, cx);
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
        self.sync_loading(window, cx);
        cx.notify();
    }

    fn sync_loading(&self, window: &mut Window, cx: &mut Context<Self>) {
        let loading = self.operation.is_running();
        self.command
            .update(cx, |command, cx| command.set_loading(loading, window, cx));
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

    fn confirm(&mut self, index: IndexPath, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conversation_id) = conversation_id_at(&self.results, index).cloned() else {
            return;
        };
        (self.on_confirm)(conversation_id, window, cx);
    }
}

impl Render for ConversationSearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let no_project_label: SharedString =
            i18n.t("sidebar-section-no-project-conversations").into();
        let no_results: SharedString = i18n.t("sidebar-search-no-results").into();
        let placeholder: SharedString = i18n.t("sidebar-search-placeholder").into();
        let error = self.operation.problem().map(ToString::to_string);
        let running = self.operation.is_running();
        let has_error = error.is_some();
        let announcement = search_announcement(i18n, running, error.as_deref(), self.results.len());
        let items = self
            .results
            .iter()
            .cloned()
            .map(|result| command_item(result, no_project_label.clone()))
            .collect::<Vec<_>>();
        let query_owner = cx.entity().downgrade();
        let confirm_owner = cx.entity().downgrade();

        let mut command = Command::new(&self.command)
            .items(items)
            .filterable(false)
            .placeholder(placeholder)
            .bordered(false)
            .max_h(px(400.))
            .w_full()
            .on_query(move |query, window, cx| {
                let _ = query_owner.update(cx, |view, cx| view.set_query(query, window, cx));
            })
            .on_confirm(move |index, window, cx| {
                let _ = confirm_owner.update(cx, |view, cx| view.confirm(index, window, cx));
            })
            .empty(move |_, _, cx| {
                v_flex()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .py_8()
                    .when(!has_error, |this| {
                        this.child(
                            Label::new(no_results.clone())
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
            });

        if let Some(error) = error {
            let retry_owner = cx.entity().downgrade();
            command = command.header(move |_, _, cx| {
                let retry_owner = retry_owner.clone();
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Label::new(error.clone())
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
                            .on_click(move |_, window, cx| {
                                let _ = retry_owner.update(cx, |view, cx| view.reload(window, cx));
                            }),
                    )
            });
        }

        v_flex()
            .w_full()
            .h(px(480.))
            .overflow_hidden()
            .child(
                div()
                    .id("conversation-search-status")
                    .role(Role::Status)
                    .a11y_synthetic_children(move |builder| {
                        configure_search_status(builder.parent_node(), &announcement);
                    })
                    .absolute()
                    .size_0()
                    .overflow_hidden(),
            )
            .child(command)
    }
}

fn command_item(result: Rc<SidebarSearchResult>, no_project_label: SharedString) -> CommandItem {
    let label = result.conversation.title.clone();
    let project_label = result
        .project
        .as_ref()
        .map(|project| project.display_name.clone())
        .unwrap_or(no_project_label);

    CommandItem::new().label(label).child(move |_, cx| {
        h_flex()
            .w_full()
            .h(px(40.))
            .items_center()
            .gap_3()
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
                        Label::new(project_label.clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
    })
}

fn normalize_query(query: &str) -> String {
    query.trim().to_string()
}

fn search_announcement(
    i18n: &I18n,
    running: bool,
    error: Option<&str>,
    result_count: usize,
) -> String {
    if running {
        return i18n.t("sidebar-search-announcement-loading");
    }

    if let Some(error) = error {
        let mut args = FluentArgs::new();
        args.set("error", error);
        return i18n.t_with_args("sidebar-search-announcement-error", &args);
    }

    if result_count == 0 {
        return i18n.t("sidebar-search-announcement-empty");
    }

    let mut args = FluentArgs::new();
    args.set("count", result_count);
    i18n.t_with_args("sidebar-search-announcement-results", &args)
}

fn configure_search_status(node: &mut gpui::accesskit::Node, announcement: &str) {
    node.set_label(announcement);
    node.set_live(gpui::accesskit::Live::Polite);
    node.set_live_atomic();
}

fn conversation_id_at(
    results: &[Rc<SidebarSearchResult>],
    index: IndexPath,
) -> Option<&ConversationId> {
    if index.section != 0 {
        return None;
    }

    results.get(index.row).map(|result| &result.conversation.id)
}

#[cfg(test)]
mod tests {
    use super::super::super::workspace::{
        SidebarConversationNode, SidebarSearchLoad, SidebarSearchResult,
    };
    use super::{
        ConversationSearchView, OnConfirm, SEARCH_RESULT_LIMIT, SearchConversations,
        configure_search_status, conversation_id_at, normalize_query, search_announcement,
    };
    use crate::foundation::I18n;
    use gpui::{TestAppContext, accesskit};
    use gpui_component::IndexPath;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};
    use tokio::sync::oneshot;

    fn test_result(id: &str) -> Rc<SidebarSearchResult> {
        Rc::new(SidebarSearchResult {
            conversation: SidebarConversationNode {
                id: id.to_string(),
                project_id: String::new(),
                title: "Conversation".into(),
                updated_at: 0,
                pinned: false,
            },
            project: None,
        })
    }

    #[test]
    fn command_index_maps_to_the_current_external_result_identity() {
        let results = vec![test_result("first"), test_result("second")];

        assert_eq!(
            conversation_id_at(&results, IndexPath::default().row(1)).map(String::as_str),
            Some("second")
        );
        assert_eq!(
            conversation_id_at(&results, IndexPath::default().row(2)),
            None
        );
        assert_eq!(
            conversation_id_at(&results, IndexPath::default().section(1)),
            None
        );
    }

    #[test]
    fn database_query_keeps_the_existing_trimmed_contract() {
        assert_eq!(normalize_query("  message body  "), "message body");
    }

    #[test]
    fn announcement_maps_owner_loading_error_empty_and_result_states() {
        let english = I18n::for_locale_tag("en-US");
        assert_eq!(
            search_announcement(&english, true, Some("ignored"), 3),
            "Searching conversations"
        );
        assert_eq!(
            search_announcement(&english, false, Some("offline"), 3),
            "Search failed: offline"
        );
        assert_eq!(
            search_announcement(&english, false, None, 0),
            "No matching conversations"
        );
        assert_eq!(
            search_announcement(&english, false, None, 3),
            "3 conversations found"
        );

        let chinese = I18n::for_locale_tag("zh-CN");
        assert_eq!(search_announcement(&chinese, true, None, 0), "正在搜索对话");
        assert_eq!(
            search_announcement(&chinese, false, Some("离线"), 2),
            "搜索失败：离线"
        );
        assert_eq!(
            search_announcement(&chinese, false, None, 0),
            "没有匹配的对话"
        );
        assert_eq!(
            search_announcement(&chinese, false, None, 2),
            "找到 2 个对话"
        );
    }

    #[test]
    fn search_status_node_is_a_polite_atomic_live_region() {
        let mut node = accesskit::Node::new(accesskit::Role::Status);

        configure_search_status(&mut node, "3 conversations found");

        assert_eq!(node.role(), accesskit::Role::Status);
        assert_eq!(node.label(), Some("3 conversations found"));
        assert_eq!(node.live(), Some(accesskit::Live::Polite));
        assert!(node.is_live_atomic());
    }

    #[gpui::test]
    fn delayed_external_search_keeps_the_latest_body_match_and_confirm_identity(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::foundation::init_i18n(cx);
        });
        let (old_tx, old_rx) = oneshot::channel();
        let (latest_tx, latest_rx) = oneshot::channel();
        let pending = Rc::new(RefCell::new(VecDeque::from([old_rx, latest_rx])));
        let queries = Rc::new(RefCell::new(Vec::new()));
        let search: SearchConversations = Rc::new({
            let pending = pending.clone();
            let queries = queries.clone();
            move |query, limit, cx| {
                assert_eq!(limit, SEARCH_RESULT_LIMIT);
                queries.borrow_mut().push(query);
                let receiver = pending
                    .borrow_mut()
                    .pop_front()
                    .expect("test search must have a delayed response");
                cx.spawn(async move |_| {
                    receiver
                        .await
                        .expect("test search response sender must stay alive")
                })
            }
        });
        let confirmed = Rc::new(RefCell::new(None));
        let on_confirm: OnConfirm = Rc::new({
            let confirmed = confirmed.clone();
            move |conversation_id, _, _| *confirmed.borrow_mut() = Some(conversation_id)
        });
        let (view, cx) = cx.add_window_view(move |window, cx| {
            ConversationSearchView::new_with_owners(search, on_confirm, Vec::new(), window, cx)
        });

        cx.run_until_parked();
        cx.update(|window, cx| _ = window.draw(cx));
        let command = cx.update(|_, cx| view.read(cx).command.clone());
        cx.update(|window, cx| {
            command.update(cx, |command, cx| {
                command.set_query("body-only-hit", window, cx)
            });
        });
        cx.run_until_parked();
        assert_eq!(
            queries.borrow().as_slice(),
            &[String::new(), "body-only-hit".to_string()]
        );

        assert!(
            latest_tx
                .send(Ok(test_load("latest", "Title does not contain the query")))
                .is_ok(),
            "latest search receiver must stay alive"
        );
        cx.run_until_parked();
        cx.update(|window, cx| _ = window.draw(cx));
        cx.update(|_, cx| {
            assert_eq!(view.read(cx).results[0].conversation.id, "latest");
            assert_eq!(command.read(cx).matched_count(), 1);
            assert_eq!(
                command.read(cx).selected_index(),
                Some(IndexPath::default())
            );
        });

        assert!(
            old_tx.send(Ok(test_load("old", "Old result"))).is_err(),
            "superseded search receiver must be cancelled"
        );
        cx.run_until_parked();
        cx.update(|window, cx| _ = window.draw(cx));
        cx.update(|_, cx| {
            assert_eq!(view.read(cx).results[0].conversation.id, "latest");
        });

        cx.update(|window, cx| {
            command.update(cx, |command, cx| command.focus(window, cx));
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(confirmed.borrow().as_deref(), Some("latest"));
    }

    fn test_load(id: &str, title: &str) -> SidebarSearchLoad {
        SidebarSearchLoad {
            results: vec![SidebarSearchResult {
                conversation: SidebarConversationNode {
                    id: id.to_string(),
                    project_id: String::new(),
                    title: title.into(),
                    updated_at: 0,
                    pinned: false,
                },
                project: None,
            }],
            stale_problem: None,
        }
    }
}
