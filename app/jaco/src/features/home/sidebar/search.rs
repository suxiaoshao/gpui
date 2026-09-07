use super::super::workspace::{HomeWorkspace, SidebarSearchLoad, SidebarSearchResult};
use crate::foundation::{I18n, assets::IconName};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::Button,
    command::{Command, CommandItem, CommandState},
    h_flex,
    label::Label,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};

const SEARCH_RESULT_LIMIT: usize = 50;

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
    workspace: Entity<HomeWorkspace>,
    command: Entity<CommandState>,
    query: String,
    operation: refresh::Operation<Vec<SidebarSearchResult>, jaco_db::DbError, Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ConversationSearchView {
    fn new(workspace: Entity<HomeWorkspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let command = cx.new(|cx| CommandState::new(window, cx));
        let workspace_subscription = cx.observe_in(&workspace, window, |view, _, window, cx| {
            if view.query.is_empty() && !view.operation.is_running() {
                view.reload(window, cx);
            }
        });
        let view = Self {
            workspace,
            command,
            query: String::new(),
            operation: refresh::Operation::new(),
            _subscriptions: vec![workspace_subscription],
        };
        let entity = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = entity.update(cx, |view, cx| view.reload(window, cx));
        });
        view
    }

    fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command.focus_handle(cx).focus(window, cx);
    }

    fn on_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        let query = query.trim();
        if self.query == query {
            return;
        }
        self.query = query.to_owned();
        if self.operation.is_running() {
            self.operation.transition(Cancel);
        }
        self.reload(window, cx);
    }

    fn sync_loading(&self, window: &mut Window, cx: &mut Context<Self>) {
        let running = self.operation.is_running();
        self.command
            .update(cx, |state, cx| state.set_loading(running, window, cx));
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
}

impl Render for ConversationSearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let no_results = i18n.t("sidebar-search-no-results");
        let no_project = i18n.t("sidebar-section-no-project-conversations");
        let placeholder = i18n.t("sidebar-search-placeholder");
        let error = self.operation.problem().map(ToString::to_string);
        let running = self.operation.is_running();
        let results = self.operation.data().cloned().unwrap_or_default();
        // Resolve against the same snapshot that supplies this Command model,
        // even if another query completes before its deferred confirmation runs.
        let ids = results
            .iter()
            .map(|result| result.conversation.id.clone())
            .collect::<Vec<_>>();
        let items = results
            .into_iter()
            .map(|result| {
                let project = result
                    .project
                    .as_ref()
                    .map(|project| project.display_name.clone())
                    .unwrap_or_else(|| no_project.clone().into());
                CommandItem::new()
                    .label(result.conversation.title.clone())
                    .icon(IconName::MessageSquare)
                    .child(move |_, cx| {
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(
                                Label::new(result.conversation.title.clone())
                                    .text_sm()
                                    .truncate(),
                            )
                            .child(
                                Label::new(project.clone())
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate(),
                            )
                    })
            })
            .collect::<Vec<_>>();
        let workspace = self.workspace.clone();
        let command = Command::new(&self.command)
            .filterable(false)
            .placeholder(placeholder)
            .items(items)
            .max_h(px(400.))
            .on_query(cx.listener(|view, query: &str, window, cx| view.on_query(query, window, cx)))
            .on_confirm(move |index, window, cx| {
                if let Some(id) = ids.get(index.row) {
                    workspace.update(cx, |workspace, cx| {
                        workspace.open_conversation(id.clone(), cx)
                    });
                    window.close_dialog(cx);
                }
            })
            .on_cancel(|window, cx| window.close_dialog(cx))
            .empty(move |_, _, cx| {
                Label::new(no_results.clone())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
            });
        v_flex()
            .w_full()
            .overflow_hidden()
            .child(command)
            .when_some(error, |this, error| {
                this.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
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
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.reload(window, cx)),
                                ),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationSearchView, Refresh, Task};
    use crate::{
        database,
        features::{conversation, home::workspace},
        state,
    };
    use gpui_kit::{AppContext as _, TestAppContext, VisualTestContext};
    use gpui_operation::Transition as _;
    use jaco_core::{
        ConversationMetadata, ConversationSettingsSnapshot, ProjectKind, ProjectMetadata,
    };
    use jaco_db::{NewConversation, NewProject};

    #[gpui_kit::test]
    fn project_only_search_survives_query_replacement_and_confirms_the_business_id(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let expected = cx.update(|cx| {
            gpui_kit::init(cx);
            database::install_for_test(cx, directory.path());
            crate::foundation::i18n::init(cx);
            state::hotkey::set_test_hotkey_state(cx);
            let id = database::with_ready_repository(cx, |repository| {
                let project = repository.insert_project(NewProject {
                    path: directory
                        .path()
                        .join("project")
                        .to_string_lossy()
                        .into_owned(),
                    display_name: "project needle".into(),
                    kind: ProjectKind::Normal,
                    pinned: false,
                    removed: false,
                    metadata: ProjectMetadata {
                        scratch_reason: None,
                        git_root: None,
                        last_active_conversation_id: None,
                    },
                })?;
                Ok(repository
                    .insert_conversation(NewConversation {
                        project_id: project.id,
                        title: "Unrelated title".into(),
                        pinned: false,
                        prompt_id: None,
                        default_provider_id: None,
                        default_model_id: None,
                        metadata: ConversationMetadata {
                            summary: None,
                            tags: vec![],
                        },
                        settings_snapshot: ConversationSettingsSnapshot {
                            prompt: None,
                            provider_id: None,
                            model_id: None,
                            model_capabilities: None,
                            tool_policy: conversation::default_tool_policy(),
                        },
                    })?
                    .id)
            })
            .unwrap();
            state::providers::init(cx);
            state::projects::init(cx);
            conversation::resources::init(cx);
            id
        });
        cx.run_until_parked();
        let workspace = cx.update(workspace::create);
        let (window, view) = cx.update(|cx| {
            let mut view = None;
            let window = cx
                .open_window(Default::default(), |window, cx| {
                    let search =
                        cx.new(|cx| ConversationSearchView::new(workspace.clone(), window, cx));
                    view = Some(search.clone());
                    cx.new(|cx| gpui_kit::component::Root::new(search, window, cx))
                })
                .unwrap();
            (window, view.unwrap())
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        let command = view.read_with(&cx, |view, _| view.command.clone());
        cx.update(|window, cx| {
            command.update(cx, |state, cx| state.set_query("missing", window, cx))
        });
        cx.run_until_parked();
        assert_eq!(command.read_with(&cx, |state, _| state.matched_count()), 0);
        cx.update(|window, cx| {
            command.update(cx, |state, cx| state.set_query("obsolete", window, cx));
            command.update(cx, |state, cx| state.set_query("needle", window, cx));
        });
        cx.run_until_parked();
        view.read_with(&cx, |view, cx| {
            assert_eq!(view.query, "needle");
            assert_eq!(view.operation.data().unwrap()[0].conversation.id, expected);
            assert_eq!(view.command.read(cx).matched_count(), 1);
            assert!(!view.command.read(cx).is_loading());
        });
        // A transient failure keeps the recovery path usable for the same query.
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.operation.transition(Refresh(Task::ready(())));
                view.complete_load(Err(jaco_db::DbError::Invariant(
                    "transient fixture error".into(),
                )));
                view.reload(window, cx);
            })
        });
        cx.run_until_parked();
        assert!(view.read_with(&cx, |view, _| view.operation.problem().is_none()));
        cx.update(|window, cx| view.update(cx, |view, cx| view.focus_search_input(window, cx)));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        workspace.read_with(&cx, |workspace, _| {
            assert_eq!(
                workspace.route(),
                &workspace::HomeRoute::Conversation(expected)
            );
        });
    }
}
