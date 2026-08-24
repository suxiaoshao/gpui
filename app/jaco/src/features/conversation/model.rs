use std::fmt;

use gpui::{Context, EventEmitter, Task};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use jaco_conversation::{ConversationError, ConversationService};
use jaco_core::{Conversation, ConversationChanges, ConversationEffect, ConversationId};

use crate::database::session::SessionDatabaseExecutor;

pub(crate) type ConversationOperation =
    refresh::Operation<Option<Conversation>, ConversationProblem, Task<()>>;

#[derive(Debug)]
pub(crate) struct ConversationProblem(ConversationError);

impl fmt::Display for ConversationProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ConversationProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<ConversationError> for ConversationProblem {
    fn from(error: ConversationError) -> Self {
        Self(error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ConversationModelEvent {
    Reloaded,
    Changed(Vec<ConversationEffect>),
}

pub(crate) struct ConversationModel {
    id: ConversationId,
    executor: SessionDatabaseExecutor,
    operation: ConversationOperation,
}

impl EventEmitter<ConversationModelEvent> for ConversationModel {}

impl ConversationModel {
    pub(crate) fn new(id: ConversationId, executor: SessionDatabaseExecutor) -> Self {
        Self {
            id,
            executor,
            operation: ConversationOperation::new(),
        }
    }

    #[cfg(test)]
    fn new_ready_for_test(conversation: Conversation, executor: SessionDatabaseExecutor) -> Self {
        let id = conversation.summary.id.clone();
        let mut operation = ConversationOperation::new();
        operation.transition(Load(Task::ready(())));
        operation.transition(Complete(Ok(Some(conversation))));
        Self {
            id,
            executor,
            operation,
        }
    }

    pub(crate) fn id(&self) -> &ConversationId {
        &self.id
    }

    pub(crate) fn operation(&self) -> &ConversationOperation {
        &self.operation
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.operation.is_running() {
            return;
        }
        let executor = self.executor.clone();
        let id = self.id.clone();
        let task = cx.spawn(async move |model, cx| {
            let result = executor
                .execute(move |repository| {
                    ConversationService::new(repository)
                        .load(&id)
                        .map_err(|error| match error {
                            ConversationError::Database(error) => error,
                        })
                })
                .await
                .map_err(|error| ConversationProblem(ConversationError::Database(error)));
            let _ = model.update(cx, |model, cx| {
                if model.operation.is_running() {
                    let reloaded = result.is_ok();
                    model.operation.transition(Complete(result));
                    if reloaded {
                        cx.emit(ConversationModelEvent::Reloaded);
                    }
                    cx.notify();
                }
            });
        });
        match &mut self.operation {
            ConversationOperation::Idle(_) => self.operation.transition(Load(task)),
            ConversationOperation::Ready(_) | ConversationOperation::Degraded(_) => {
                self.operation.transition(Refresh(task))
            }
            ConversationOperation::Unavailable(_) => self.operation.transition(Retry(task)),
            ConversationOperation::Loading(_)
            | ConversationOperation::Refreshing(_)
            | ConversationOperation::Retrying(_)
            | ConversationOperation::RefreshingDegraded(_) => return,
        }
        cx.notify();
    }

    pub(crate) fn apply_changes(&mut self, changes: ConversationChanges, cx: &mut Context<Self>) {
        if changes.0.is_empty() {
            return;
        }

        if matches!(self.operation, ConversationOperation::Refreshing(_)) {
            self.operation.transition(Cancel);
        }

        if matches!(
            &self.operation,
            ConversationOperation::Ready(ready) if ready.data().is_some()
        ) {
            let ConversationOperation::Ready(ready) = &mut self.operation else {
                unreachable!("ready operation was checked before applying changes")
            };
            let effects = ready.transition(changes);
            cx.emit(ConversationModelEvent::Changed(effects));
            cx.notify();
            return;
        }

        match self.operation {
            ConversationOperation::Idle(_) => {}
            ConversationOperation::Loading(_)
            | ConversationOperation::Retrying(_)
            | ConversationOperation::RefreshingDegraded(_) => {
                self.operation.transition(Cancel);
                self.refresh(cx);
            }
            ConversationOperation::Ready(_)
            | ConversationOperation::Unavailable(_)
            | ConversationOperation::Degraded(_) => {
                self.refresh(cx);
            }
            ConversationOperation::Refreshing(_) => {
                unreachable!("ready and refreshing states were handled before invalidation")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use gpui::{AppContext, Entity, Subscription, TestAppContext};
    use jaco_core::{
        ContentPart, ConversationEntry, ConversationEntryKind, ConversationEntryPayload,
        ConversationEntryStatus, ConversationMetadata, ConversationSettingsSnapshot,
        ConversationStatus, EntryChangeKind, ProjectKind, ProjectMetadata, ProjectSummary,
        ToolApprovalMode, ToolApprovalPolicy, ToolPolicySnapshot, TranscriptRole,
    };
    use time::OffsetDateTime;

    use super::*;

    struct EventRecorder {
        events: Arc<Mutex<Vec<ConversationModelEvent>>>,
        _subscription: Subscription,
    }

    impl EventRecorder {
        fn new(model: Entity<ConversationModel>, cx: &mut Context<Self>) -> Self {
            let events = Arc::new(Mutex::new(Vec::new()));
            let recorded = events.clone();
            let subscription = cx.subscribe(
                &model,
                move |_recorder, _model, event: &ConversationModelEvent, _cx| {
                    recorded.lock().unwrap().push(event.clone());
                },
            );
            Self {
                events,
                _subscription: subscription,
            }
        }
    }

    #[gpui::test]
    fn shared_model_publishes_one_precise_event_to_every_consumer(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            jaco_db::FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3"))
                .unwrap();
        let executor = SessionDatabaseExecutor::for_test(store);
        let model = cx.update(|cx| {
            cx.new(|_| ConversationModel::new_ready_for_test(conversation(), executor))
        });
        let first = cx.update(|cx| cx.new(|cx| EventRecorder::new(model.clone(), cx)));
        let second = cx.update(|cx| cx.new(|cx| EventRecorder::new(model.clone(), cx)));
        let updated = entry("streaming output");

        cx.update(|cx| {
            model.update(cx, |model, cx| {
                model.apply_changes(
                    ConversationChanges(vec![jaco_core::ConversationChange::EntryUpdated {
                        entry: Box::new(updated.clone()),
                        kind: EntryChangeKind::TextAppended,
                    }]),
                    cx,
                );
            });
        });

        let expected = ConversationModelEvent::Changed(vec![ConversationEffect::EntryChanged {
            entry_id: "entry-1".to_string(),
            kind: EntryChangeKind::TextAppended,
        }]);
        cx.update(|cx| {
            assert_eq!(
                first.read(cx).events.lock().unwrap().as_slice(),
                std::slice::from_ref(&expected)
            );
            assert_eq!(
                second.read(cx).events.lock().unwrap().as_slice(),
                &[expected]
            );
            let entry = model
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .unwrap()
                .entries
                .first()
                .unwrap();
            assert_eq!(entry, &updated);
        });
    }

    #[gpui::test]
    fn committed_change_cancels_refresh_before_applying_to_ready_data(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            jaco_db::FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3"))
                .unwrap();
        let executor = SessionDatabaseExecutor::for_test(store);
        let model = cx.update(|cx| {
            cx.new(|_| ConversationModel::new_ready_for_test(conversation(), executor))
        });
        let updated = entry("newer committed output");

        cx.update(|cx| {
            model.update(cx, |model, cx| {
                model.refresh(cx);
                assert!(matches!(
                    model.operation(),
                    ConversationOperation::Refreshing(_)
                ));
                model.apply_changes(
                    ConversationChanges(vec![jaco_core::ConversationChange::EntryUpdated {
                        entry: Box::new(updated.clone()),
                        kind: EntryChangeKind::TextAppended,
                    }]),
                    cx,
                );
                assert!(matches!(model.operation(), ConversationOperation::Ready(_)));
            });
        });

        cx.update(|cx| {
            let entry = model
                .read(cx)
                .operation()
                .data()
                .and_then(Option::as_ref)
                .unwrap()
                .entries
                .first()
                .unwrap();
            assert_eq!(entry, &updated);
        });
    }

    #[gpui::test]
    fn committed_change_restarts_loading_instead_of_merging_partial_data(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            jaco_db::FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3"))
                .unwrap();
        let executor = SessionDatabaseExecutor::for_test(store);
        let model = cx.update(|cx| {
            cx.new(|_| ConversationModel::new("conversation-1".to_string(), executor))
        });

        cx.update(|cx| {
            model.update(cx, |model, cx| {
                model.refresh(cx);
                model.apply_changes(
                    ConversationChanges(vec![jaco_core::ConversationChange::EntryUpdated {
                        entry: Box::new(entry("committed while loading")),
                        kind: EntryChangeKind::Replaced,
                    }]),
                    cx,
                );
                assert!(matches!(
                    model.operation(),
                    ConversationOperation::Loading(_)
                ));
                assert!(model.operation().data().is_none());
            });
        });
    }

    #[gpui::test]
    fn committed_change_keeps_degraded_data_stale_and_restarts_refresh(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            jaco_db::FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3"))
                .unwrap();
        let executor = SessionDatabaseExecutor::for_test(store);
        let model = cx.update(|cx| {
            cx.new(|_| ConversationModel::new_ready_for_test(conversation(), executor))
        });

        cx.update(|cx| {
            model.update(cx, |model, cx| {
                model.operation.transition(Refresh(Task::ready(())));
                model.operation.transition(Complete(Err(ConversationProblem(
                    ConversationError::Database(jaco_db::DbError::Invariant(
                        "test refresh failure".to_string(),
                    )),
                ))));
                assert!(matches!(
                    model.operation(),
                    ConversationOperation::Degraded(_)
                ));

                model.apply_changes(
                    ConversationChanges(vec![jaco_core::ConversationChange::EntryUpdated {
                        entry: Box::new(entry("committed while degraded")),
                        kind: EntryChangeKind::Replaced,
                    }]),
                    cx,
                );

                assert!(matches!(
                    model.operation(),
                    ConversationOperation::RefreshingDegraded(_)
                ));
                let current = model
                    .operation()
                    .data()
                    .and_then(Option::as_ref)
                    .unwrap()
                    .entries
                    .first()
                    .unwrap();
                assert_eq!(current, &entry("streaming"));
            });
        });
    }

    fn conversation() -> Conversation {
        Conversation {
            summary: jaco_core::ConversationSummary {
                id: "conversation-1".to_string(),
                project_id: "project-1".to_string(),
                title: "Conversation".to_string(),
                status: ConversationStatus::Active,
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                last_entry_seq: 1,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: ConversationSettingsSnapshot {
                    prompt: None,
                    provider_id: None,
                    model_id: None,
                    model_capabilities: None,
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::Never,
                        enabled_sources: Vec::new(),
                        max_steps: 1,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                recency_at: OffsetDateTime::UNIX_EPOCH,
                archived_at: None,
                deleted_at: None,
            },
            project: ProjectSummary {
                id: "project-1".to_string(),
                path: "/tmp/project".to_string(),
                display_name: "Project".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: None,
                    last_active_conversation_id: None,
                },
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                last_opened_at: None,
            },
            entries: vec![entry("streaming")],
            attachments: Vec::new(),
            runs: Vec::new(),
            provider_steps: Vec::new(),
            tool_invocations: Vec::new(),
            agent_message_request_usages: Vec::new(),
            latest_context_request_usage: None,
        }
    }

    fn entry(text: &str) -> ConversationEntry {
        ConversationEntry {
            id: "entry-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            seq: 1,
            kind: ConversationEntryKind::Message,
            status: ConversationEntryStatus::Running,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                content: vec![ContentPart::Text {
                    text: text.to_string(),
                }],
            },
            search_text: text.to_string(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
