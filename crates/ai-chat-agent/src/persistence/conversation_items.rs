use super::{PersistenceContext, lock, mutex_clone, mutex_replace};
use crate::{AgentRuntimeEvent, AgentStep, Result};
use ai_chat_core::*;
use ai_chat_db::{ConversationItemRecord, NewConversationItem};

impl PersistenceContext {
    pub(super) fn append_item(
        &self,
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: self.conversation_id.clone(),
            status: ConversationItemStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: None,
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationItem(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationItemAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
    }

    pub(crate) fn append_running_item(
        &self,
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: self.conversation_id.clone(),
            status: ConversationItemStatus::Running,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: None,
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationItem(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationItemAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
    }

    pub(crate) fn update_item_payload(
        &self,
        item_id: &str,
        status: ConversationItemStatus,
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let item = self
            .repo
            .update_conversation_item_payload(item_id, status, payload)?;
        self.emit_runtime(AgentRuntimeEvent::ConversationItemUpdated {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
    }

    pub(crate) fn set_final_item_id(&self, item_id: Option<ConversationItemId>) {
        mutex_replace(&self.final_item_id, item_id);
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
        payload: ConversationItemPayload,
    ) -> Result<ConversationItemRecord> {
        let item = self.repo.append_conversation_item(NewConversationItem {
            conversation_id: self.conversation_id.clone(),
            status: ConversationItemStatus::Completed,
            agent_run_id: Some(self.agent_run_id.clone()),
            provider_step_id: mutex_clone(&self.last_provider_step_id),
            tool_invocation_id: Some(tool_invocation_id),
            provider_item_id: None,
            payload,
        })?;
        self.add_input_item_id(item.id.clone());
        self.push_step(AgentStep::ConversationItem(item.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationItemAppended {
            conversation_id: self.conversation_id.clone(),
            item_id: item.id.clone(),
        });
        Ok(item)
    }

    pub(super) fn add_input_item_id(&self, item_id: ConversationItemId) {
        let mut guard = lock(&self.input_item_ids);
        guard.push(item_id);
    }

    pub(super) fn push_event(&self, event: AgentRunEvent) {
        lock(&self.events).push(event);
    }

    pub(super) fn push_step(&self, step: AgentStep) {
        lock(&self.steps).push(step);
    }

    pub(super) fn emit_runtime(&self, event: AgentRuntimeEvent) {
        if let Some(observer) = &self.observer {
            observer.emit(event);
        }
    }
}
