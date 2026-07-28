use super::{PersistenceContext, lock, mutex_clone, mutex_replace};
use crate::{AgentRuntimeEvent, AgentStep, Result};
use jaco_core::*;
use jaco_db::{
    ConversationEntryRecord, NewConversationEntry, ToolInvocationRecord, UpdateToolInvocationStatus,
};

impl PersistenceContext {
    pub(crate) fn record_persisted_entries(&self, entries: &[ConversationEntryRecord]) {
        for entry in entries {
            self.add_input_item_id(entry.id.clone());
            self.push_step(AgentStep::ConversationEntry(entry.id.clone()));
        }
    }

    pub(super) async fn append_entries_and_update_tool_invocation_full(
        &self,
        entries: Vec<NewConversationEntry>,
        invocation: &ToolInvocationRecord,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> Result<(Vec<ConversationEntryRecord>, ToolInvocationRecord)> {
        let commit = self
            .persistence
            .append_entries_and_update_tool_invocation(
                entries,
                invocation.id.clone(),
                update,
                approval,
            )
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            commit
                .value
                .0
                .iter()
                .cloned()
                .map(|entry| jaco_core::ConversationChange::EntryAppended {
                    entry: Box::new(entry),
                })
                .chain(std::iter::once(
                    jaco_core::ConversationChange::ToolInvocationChanged {
                        invocation: Box::new(commit.value.1.clone()),
                    },
                ))
                .collect(),
        );
        let (entries, invocation) = commit.value;
        self.record_persisted_entries(&entries);
        Ok((entries, invocation))
    }

    pub(super) async fn append_item(
        &self,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .append_conversation_entry(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                tool_invocation_id: None,
                provider_item_id: None,
                payload,
            })
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.clone()),
            }],
        );
        let item = commit.value;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        Ok(item)
    }

    pub(crate) async fn append_running_item(
        &self,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .append_conversation_entry(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Running,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                tool_invocation_id: None,
                provider_item_id: None,
                payload,
            })
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.clone()),
            }],
        );
        let item = commit.value;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        Ok(item)
    }

    pub(crate) async fn update_item_payload(
        &self,
        item_id: &str,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .update_conversation_entry_payload(item_id.to_string(), status, payload)
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryUpdated {
                entry: Box::new(commit.value.clone()),
                kind: EntryChangeKind::TextAppended,
            }],
        );
        Ok(commit.value)
    }

    pub(crate) fn set_final_entry_id(&self, item_id: Option<ConversationEntryId>) {
        mutex_replace(&self.final_entry_id, item_id);
    }

    pub(crate) fn current_provider_step_id(&self) -> Option<ProviderStepId> {
        mutex_clone(&self.last_provider_step_id)
    }

    pub(crate) fn push_current_provider_step_event(&self, event: ProviderStepEvent) {
        let Some(provider_step_id) = self.current_provider_step_id() else {
            return;
        };
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id,
            event,
        });
    }

    pub(super) async fn append_tool_item(
        &self,
        tool_invocation_id: ToolInvocationId,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let commit = self
            .persistence
            .append_conversation_entry(NewConversationEntry {
                conversation_id: self.conversation_id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: Some(self.agent_run_id.clone()),
                provider_step_id: mutex_clone(&self.last_provider_step_id),
                tool_invocation_id: Some(tool_invocation_id),
                provider_item_id: None,
                payload,
            })
            .await?;
        self.emit_conversation_commit_with_changes(
            &commit,
            vec![jaco_core::ConversationChange::EntryAppended {
                entry: Box::new(commit.value.clone()),
            }],
        );
        let item = commit.value;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        Ok(item)
    }

    pub(super) fn add_input_item_id(&self, item_id: ConversationEntryId) {
        let mut guard = lock(&self.input_item_ids);
        guard.push(item_id);
    }

    pub(super) fn push_event(&self, event: AgentRunEvent) {
        lock(&self.events).push(event);
    }

    pub(crate) fn push_step(&self, step: AgentStep) {
        lock(&self.steps).push(step);
    }

    pub(super) fn emit_runtime(&self, event: AgentRuntimeEvent) {
        if let Some(observer) = &self.observer {
            observer.emit(event);
        }
    }

    pub(super) fn emit_conversation_commit_with_changes<T>(
        &self,
        commit: &jaco_db::ConversationCommit<T>,
        changes: Vec<jaco_core::ConversationChange>,
    ) {
        self.emit_runtime(AgentRuntimeEvent::ConversationCommitted {
            conversation: Box::new(commit.conversation.clone()),
            changes,
        });
    }

    pub(super) fn emit_tool_entries_commit(
        &self,
        commit: &jaco_db::ConversationCommit<(Vec<ConversationEntryRecord>, ToolInvocationRecord)>,
    ) {
        self.emit_conversation_commit_with_changes(
            commit,
            commit
                .value
                .0
                .iter()
                .cloned()
                .map(|entry| jaco_core::ConversationChange::EntryAppended {
                    entry: Box::new(entry),
                })
                .chain(std::iter::once(
                    jaco_core::ConversationChange::ToolInvocationChanged {
                        invocation: Box::new(commit.value.1.clone()),
                    },
                ))
                .collect(),
        );
    }

    pub(super) fn emit_tool_entry_commit(
        &self,
        commit: &jaco_db::ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>,
    ) {
        self.emit_conversation_commit_with_changes(
            commit,
            vec![
                jaco_core::ConversationChange::EntryAppended {
                    entry: Box::new(commit.value.0.clone()),
                },
                jaco_core::ConversationChange::ToolInvocationChanged {
                    invocation: Box::new(commit.value.1.clone()),
                },
            ],
        );
    }
}
