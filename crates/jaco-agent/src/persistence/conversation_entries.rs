use super::{PersistenceContext, lock, mutex_clone, mutex_replace};
use crate::{AgentRuntimeEvent, AgentStep, Result};
use jaco_core::*;
use jaco_db::{
    ConversationEntryRecord, NewConversationEntry, ToolInvocationApproval, ToolInvocationRecord,
    UpdateToolInvocationStatus,
};

impl PersistenceContext {
    pub(crate) fn record_persisted_entries(&self, entries: &[ConversationEntryRecord]) {
        for entry in entries {
            self.add_input_item_id(entry.id.clone());
            self.push_step(AgentStep::ConversationEntry(entry.id.clone()));
            self.emit_runtime(AgentRuntimeEvent::ConversationEntryAppended {
                conversation_id: self.conversation_id.clone(),
                item_id: entry.id.clone(),
            });
        }
    }

    pub(super) fn append_entries_and_update_tool_invocation_full(
        &self,
        entries: Vec<NewConversationEntry>,
        invocation: &ToolInvocationRecord,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> Result<(Vec<ConversationEntryRecord>, ToolInvocationRecord)> {
        let (entries, invocation) = self
            .repo
            .append_conversation_entries_and_update_tool_invocation_full(
                entries,
                &invocation.id,
                update,
                approval,
            )?;
        self.record_persisted_entries(&entries);
        self.emit_runtime(AgentRuntimeEvent::ToolInvocationChanged {
            agent_run_id: invocation.agent_run_id.clone(),
            tool_invocation_id: invocation.id.clone(),
        });
        Ok((entries, invocation))
    }

    pub(super) fn append_item(
        &self,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let item = self.repo.append_conversation_entry(NewConversationEntry {
            conversation_id: self.conversation_id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: None,
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationEntryAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
    }

    pub(crate) fn append_running_item(
        &self,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let item = self.repo.append_conversation_entry(NewConversationEntry {
            conversation_id: self.conversation_id.clone(),
            status: ConversationEntryStatus::Running,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: None,
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationEntryAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
    }

    pub(crate) fn update_item_payload(
        &self,
        item_id: &str,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let item = self
            .repo
            .update_conversation_entry_payload(item_id, status, payload)?;
        self.emit_runtime(AgentRuntimeEvent::ConversationEntryUpdated {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
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

    pub(super) fn append_tool_item(
        &self,
        tool_invocation_id: ToolInvocationId,
        payload: ConversationEntryPayload,
    ) -> Result<ConversationEntryRecord> {
        let item = self.repo.append_conversation_entry(NewConversationEntry {
            conversation_id: self.conversation_id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: Some(tool_invocation_id),
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationEntry(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationEntryAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
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
}
