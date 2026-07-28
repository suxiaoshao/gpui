use std::{collections::HashMap, fmt};

use gpui::{App, AppContext, Context, Entity, Task, WeakEntity};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use jaco_conversation::{ConversationError, ConversationService};
use jaco_core::{
    ConversationChange, ConversationChanges, ConversationId, ConversationStatus,
    ConversationSummary,
};

use crate::database::session::SessionDatabaseExecutor;

use super::model::ConversationModel;

pub(crate) type ConversationCatalogOperation =
    refresh::Operation<Vec<ConversationSummary>, ConversationCatalogProblem, Task<()>>;

#[derive(Debug)]
pub(crate) struct ConversationCatalogProblem(ConversationError);

impl fmt::Display for ConversationCatalogProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ConversationCatalogProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub(crate) enum ConversationCatalogMessage {
    Upsert(ConversationSummary),
    Remove(ConversationId),
}

impl Transition<ConversationCatalogMessage> for &mut Vec<ConversationSummary> {
    type Output = ();

    fn transition(self, message: ConversationCatalogMessage) {
        match message {
            ConversationCatalogMessage::Upsert(summary) => {
                if summary.status != ConversationStatus::Active {
                    self.retain(|current| current.id != summary.id);
                } else if let Some(current) =
                    self.iter_mut().find(|current| current.id == summary.id)
                {
                    *current = summary;
                } else {
                    self.push(summary);
                }
            }
            ConversationCatalogMessage::Remove(id) => {
                self.retain(|current| current.id != id);
            }
        }
        sort_catalog(self);
    }
}

pub(crate) struct ConversationCatalogModel {
    executor: SessionDatabaseExecutor,
    operation: ConversationCatalogOperation,
}

impl ConversationCatalogModel {
    fn new(executor: SessionDatabaseExecutor) -> Self {
        Self {
            executor,
            operation: ConversationCatalogOperation::new(),
        }
    }

    pub(crate) fn operation(&self) -> &ConversationCatalogOperation {
        &self.operation
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.operation.is_running() {
            return;
        }
        let executor = self.executor.clone();
        let task = cx.spawn(async move |catalog, cx| {
            let result = executor
                .execute(|repository| {
                    ConversationService::new(repository)
                        .load_catalog()
                        .map_err(|error| match error {
                            ConversationError::Database(error) => error,
                        })
                })
                .await
                .map_err(|error| ConversationCatalogProblem(ConversationError::Database(error)));
            let _ = catalog.update(cx, |catalog, cx| {
                if catalog.operation.is_running() {
                    catalog.operation.transition(Complete(result));
                    cx.notify();
                }
            });
        });
        match &mut self.operation {
            ConversationCatalogOperation::Idle(_) => self.operation.transition(Load(task)),
            ConversationCatalogOperation::Ready(_) | ConversationCatalogOperation::Degraded(_) => {
                self.operation.transition(Refresh(task))
            }
            ConversationCatalogOperation::Unavailable(_) => self.operation.transition(Retry(task)),
            ConversationCatalogOperation::Loading(_)
            | ConversationCatalogOperation::Refreshing(_)
            | ConversationCatalogOperation::Retrying(_)
            | ConversationCatalogOperation::RefreshingDegraded(_) => return,
        }
        cx.notify();
    }

    fn transition(&mut self, message: ConversationCatalogMessage, cx: &mut Context<Self>) {
        if matches!(self.operation, ConversationCatalogOperation::Refreshing(_)) {
            self.operation.transition(Cancel);
        }

        if let ConversationCatalogOperation::Ready(ready) = &mut self.operation {
            ready.transition(message);
            cx.notify();
            return;
        }

        if self.operation.is_running() {
            self.operation.transition(Cancel);
        }
        drop(message);
        self.refresh(cx);
    }
}

pub(crate) struct ConversationRegistry {
    executor: SessionDatabaseExecutor,
    catalog: Entity<ConversationCatalogModel>,
    conversations: HashMap<ConversationId, WeakEntity<ConversationModel>>,
    active_conversations: HashMap<ConversationId, Entity<ConversationModel>>,
}

impl ConversationRegistry {
    pub(crate) fn new(executor: SessionDatabaseExecutor, cx: &mut Context<Self>) -> Self {
        let catalog = cx.new(|_| ConversationCatalogModel::new(executor.clone()));
        Self {
            executor,
            catalog,
            conversations: HashMap::new(),
            active_conversations: HashMap::new(),
        }
    }

    pub(crate) fn catalog(&self) -> Entity<ConversationCatalogModel> {
        self.catalog.clone()
    }

    pub(crate) fn conversation(
        &mut self,
        id: ConversationId,
        cx: &mut Context<Self>,
    ) -> Entity<ConversationModel> {
        if let Some(model) = self.conversations.get(&id).and_then(WeakEntity::upgrade) {
            return model;
        }
        let model_id = id.clone();
        let model = cx.new(|_| ConversationModel::new(model_id, self.executor.clone()));
        self.conversations.insert(id, model.downgrade());
        model.update(cx, |model, cx| model.refresh(cx));
        model
    }

    pub(crate) fn retain_active(
        &mut self,
        id: ConversationId,
        cx: &mut Context<Self>,
    ) -> Entity<ConversationModel> {
        let model = self.conversation(id.clone(), cx);
        self.active_conversations.insert(id, model.clone());
        model
    }

    pub(crate) fn release_active(&mut self, id: &ConversationId) {
        self.active_conversations.remove(id);
    }

    pub(crate) fn publish_summary(&mut self, summary: ConversationSummary, cx: &mut Context<Self>) {
        let id = summary.id.clone();
        self.catalog.update(cx, |catalog, cx| {
            catalog.transition(ConversationCatalogMessage::Upsert(summary.clone()), cx);
        });
        if let Some(model) = self.conversations.get(&id).and_then(WeakEntity::upgrade) {
            model.update(cx, |model, cx| {
                model.apply_changes(
                    ConversationChanges(vec![ConversationChange::SummaryChanged {
                        summary: Box::new(summary),
                    }]),
                    cx,
                );
            });
        }
    }

    pub(crate) fn publish_removed(&mut self, id: ConversationId, cx: &mut Context<Self>) {
        self.catalog.update(cx, |catalog, cx| {
            catalog.transition(ConversationCatalogMessage::Remove(id.clone()), cx);
        });
        if let Some(model) = self.conversations.get(&id).and_then(WeakEntity::upgrade) {
            model.update(cx, |model, cx| {
                model.apply_changes(ConversationChanges(vec![ConversationChange::Deleted]), cx);
            });
        }
    }

    pub(crate) fn publish_changes(
        &mut self,
        id: ConversationId,
        summary: Option<ConversationSummary>,
        mut changes: Vec<ConversationChange>,
        cx: &mut Context<Self>,
    ) {
        if let Some(summary) = summary {
            self.catalog.update(cx, |catalog, cx| {
                catalog.transition(ConversationCatalogMessage::Upsert(summary.clone()), cx);
            });
            changes.insert(
                0,
                ConversationChange::SummaryChanged {
                    summary: Box::new(summary),
                },
            );
        }
        let Some(model) = self.conversations.get(&id).and_then(WeakEntity::upgrade) else {
            return;
        };
        model.update(cx, |model, cx| {
            model.apply_changes(ConversationChanges(changes), cx);
        });
    }

    pub(crate) fn refresh_conversation(&mut self, id: &ConversationId, cx: &mut Context<Self>) {
        if let Some(model) = self.conversations.get(id).and_then(WeakEntity::upgrade) {
            model.update(cx, |model, cx| model.refresh(cx));
        }
    }
}

fn sort_catalog(catalog: &mut [ConversationSummary]) {
    catalog.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub(crate) fn publish_summary(summary: ConversationSummary, cx: &mut impl AppContext) {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return;
    };
    registry.update(cx, |registry, cx| registry.publish_summary(summary, cx));
}

pub(crate) fn publish_removed(id: ConversationId, cx: &mut impl AppContext) {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return;
    };
    registry.update(cx, |registry, cx| registry.publish_removed(id, cx));
}

pub(crate) fn publish_changes(
    id: ConversationId,
    summary: Option<ConversationSummary>,
    changes: Vec<jaco_core::ConversationChange>,
    cx: &mut impl AppContext,
) {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return;
    };
    registry.update(cx, |registry, cx| {
        registry.publish_changes(id, summary, changes, cx);
    });
}

pub(crate) fn refresh_conversation(id: &ConversationId, cx: &mut impl AppContext) {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return;
    };
    registry.update(cx, |registry, cx| registry.refresh_conversation(id, cx));
}

pub(crate) fn is_catalog_ready(cx: &App) -> bool {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return false;
    };
    let catalog = registry.read(cx).catalog();
    matches!(
        catalog.read(cx).operation(),
        ConversationCatalogOperation::Ready(_)
    )
}

pub(crate) fn retain_active(id: ConversationId, cx: &mut impl AppContext) {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return;
    };
    registry.update(cx, |registry, cx| {
        registry.retain_active(id, cx);
    });
}

pub(crate) fn release_active(id: &ConversationId, cx: &mut impl AppContext) {
    let Some(registry) = crate::app::session::ready_conversations(cx) else {
        return;
    };
    registry.update(cx, |registry, _cx| registry.release_active(id));
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use jaco_core::{
        ConversationMetadata, ConversationSettingsSnapshot, ToolApprovalMode, ToolApprovalPolicy,
        ToolPolicySnapshot,
    };
    use time::OffsetDateTime;

    use super::*;

    #[gpui::test]
    fn registry_reuses_a_live_conversation_model(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            jaco_db::FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3"))
                .unwrap();
        let executor = SessionDatabaseExecutor::for_test(store);

        cx.update(|cx| {
            let registry = cx.new(|cx| ConversationRegistry::new(executor, cx));
            let first = registry.update(cx, |registry, cx| {
                registry.conversation("conversation-1".to_string(), cx)
            });
            let second = registry.update(cx, |registry, cx| {
                registry.conversation("conversation-1".to_string(), cx)
            });

            assert_eq!(first, second);
        });
    }

    #[gpui::test]
    fn committed_summary_cancels_catalog_refresh_and_updates_retained_data(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            jaco_db::FreshStore::open_or_create_initial(directory.path().join("jaco.sqlite3"))
                .unwrap();
        let executor = SessionDatabaseExecutor::for_test(store);

        cx.update(|cx| {
            let catalog = cx.new(|_| {
                let mut operation = ConversationCatalogOperation::new();
                operation.transition(gpui_operation::Settle(Ok(vec![summary("Before")])));
                ConversationCatalogModel {
                    executor,
                    operation,
                }
            });

            catalog.update(cx, |catalog, cx| {
                catalog.operation.transition(Refresh(Task::ready(())));
                catalog.transition(ConversationCatalogMessage::Upsert(summary("After")), cx);

                let ConversationCatalogOperation::Ready(ready) = &catalog.operation else {
                    panic!("committed summary must restore an exact Ready catalog");
                };
                assert_eq!(ready.data()[0].title, "After");
            });
        });
    }

    fn summary(title: &str) -> ConversationSummary {
        ConversationSummary {
            id: "conversation-1".to_string(),
            project_id: "project-1".to_string(),
            title: title.to_string(),
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
            archived_at: None,
            deleted_at: None,
        }
    }
}
