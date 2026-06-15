use super::{PersistenceContext, mutex_clone, mutex_replace, provider_usage};
use crate::{AgentRuntimeEvent, AgentStep, Result};
use ai_chat_core::*;
use ai_chat_db::{NewProviderStep, NewUsageEvent, ProviderStepRecord, UpdateProviderStepStatus};
use rig_core::completion::{AssistantContent, CompletionRequest, CompletionResponse, Usage};
use serde::Serialize;

impl PersistenceContext {
    pub(super) fn insert_provider_step(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderStepRecord> {
        let seq = self.repo.next_provider_step_seq(&self.agent_run_id)?;
        let input_item_ids = mutex_clone(&self.input_item_ids);
        let step = self.repo.insert_provider_step(NewProviderStep {
            agent_run_id: self.agent_run_id.clone(),
            seq,
            status: ProviderStepStatus::Running,
            request_snapshot: ProviderStepRequestSnapshot {
                provider_id: self.provider_id.clone(),
                model_id: self.model_id.clone(),
                input_item_ids,
                snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
                request_body: ProviderRawPayload {
                    provider_kind: "rig".to_string(),
                    value: serde_json::to_value(request)?,
                },
            },
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: self.settings_snapshot.clone(),
            error: None,
        })?;
        mutex_replace(&self.last_provider_step_id, Some(step.id.clone()));
        self.push_event(AgentRunEvent::ProviderStepStarted {
            provider_step_id: step.id.clone(),
        });
        self.push_step(AgentStep::ProviderStep(step.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ProviderStepChanged {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: step.id.clone(),
        });
        Ok(step)
    }

    pub(super) fn finish_provider_step<M>(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse<M>,
    ) -> Result<()>
    where
        M: Serialize,
    {
        let output_item_ids = response
            .choice
            .iter()
            .filter_map(|content| match content {
                AssistantContent::Reasoning(reasoning) => reasoning.id.clone(),
                _ => None,
            })
            .collect::<Vec<_>>();
        let response_snapshot = ProviderStepResponseSnapshot {
            provider_run_id: response.message_id.clone(),
            output_item_ids: output_item_ids.clone(),
            response_body: Some(ProviderRawPayload {
                provider_kind: "rig".to_string(),
                value: serde_json::to_value(&response.raw_response)?,
            }),
        };
        let state_snapshot = ProviderRunStateSnapshot {
            provider_id: self.provider_id.clone(),
            provider_run_id: response.message_id.clone(),
            output_item_ids,
            continuation: response
                .message_id
                .as_ref()
                .map(|message_id| ProviderRawPayload {
                    provider_kind: "rig".to_string(),
                    value: serde_json::json!({ "messageId": message_id }),
                }),
        };
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(response_snapshot),
                state_snapshot: Some(state_snapshot.clone()),
                error: None,
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::ProviderStepChanged {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: provider_step_id.to_string(),
        });
        let usage = provider_usage(response.usage);
        self.repo.insert_usage_event(NewUsageEvent {
            provider_step_id: provider_step_id.to_string(),
            date_key: time::OffsetDateTime::now_utc().date().to_string(),
            usage: usage.clone(),
        })?;
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::UsageUpdated { usage },
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Completed {
                state: Some(state_snapshot),
            },
        });
        Ok(())
    }

    pub(crate) fn finish_current_streaming_provider_step<M>(
        &self,
        response: Option<&M>,
        usage: Usage,
    ) -> Result<()>
    where
        M: Serialize,
    {
        let Some(provider_step_id) = mutex_clone(&self.last_provider_step_id) else {
            return Ok(());
        };
        self.finish_streaming_provider_step(&provider_step_id, response, usage)
    }

    pub(crate) fn fail_current_provider_step(&self, error: RunErrorPayload) -> Result<()> {
        let Some(provider_step_id) = mutex_clone(&self.last_provider_step_id) else {
            return Ok(());
        };
        self.fail_provider_step(&provider_step_id, error)
    }

    pub(crate) fn cancel_current_provider_step(&self, error: RunErrorPayload) -> Result<()> {
        let Some(provider_step_id) = mutex_clone(&self.last_provider_step_id) else {
            return Ok(());
        };
        self.cancel_provider_step(&provider_step_id, error)
    }

    pub(super) fn complete_streaming_provider_step(&self, provider_step_id: &str) -> Result<()> {
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(ProviderStepResponseSnapshot {
                    provider_run_id: None,
                    output_item_ids: Vec::new(),
                    response_body: None,
                }),
                state_snapshot: None,
                error: None,
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::ProviderStepChanged {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: provider_step_id.to_string(),
        });
        Ok(())
    }

    pub(super) fn finish_streaming_provider_step<M>(
        &self,
        provider_step_id: &str,
        response: Option<&M>,
        usage: Usage,
    ) -> Result<()>
    where
        M: Serialize,
    {
        let response_snapshot = ProviderStepResponseSnapshot {
            provider_run_id: None,
            output_item_ids: Vec::new(),
            response_body: response
                .map(|response| {
                    serde_json::to_value(response).map(|value| ProviderRawPayload {
                        provider_kind: "rig".to_string(),
                        value,
                    })
                })
                .transpose()?,
        };
        let state_snapshot = ProviderRunStateSnapshot {
            provider_id: self.provider_id.clone(),
            provider_run_id: None,
            output_item_ids: Vec::new(),
            continuation: None,
        };
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: Some(response_snapshot),
                state_snapshot: Some(state_snapshot.clone()),
                error: None,
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::ProviderStepChanged {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: provider_step_id.to_string(),
        });
        let usage = provider_usage(usage);
        self.repo.insert_usage_event(NewUsageEvent {
            provider_step_id: provider_step_id.to_string(),
            date_key: time::OffsetDateTime::now_utc().date().to_string(),
            usage: usage.clone(),
        })?;
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::UsageUpdated { usage },
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Completed {
                state: Some(state_snapshot),
            },
        });
        Ok(())
    }

    pub(super) fn fail_provider_step(
        &self,
        provider_step_id: &str,
        error: RunErrorPayload,
    ) -> Result<()> {
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Failed,
                response_snapshot: None,
                state_snapshot: None,
                error: Some(error.clone()),
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::ProviderStepChanged {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: provider_step_id.to_string(),
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Failed { error },
        });
        Ok(())
    }

    pub(super) fn cancel_provider_step(
        &self,
        provider_step_id: &str,
        error: RunErrorPayload,
    ) -> Result<()> {
        self.repo.update_provider_step_status(
            provider_step_id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Canceled,
                response_snapshot: None,
                state_snapshot: None,
                error: Some(error.clone()),
            },
        )?;
        self.emit_runtime(AgentRuntimeEvent::ProviderStepChanged {
            agent_run_id: self.agent_run_id.clone(),
            provider_step_id: provider_step_id.to_string(),
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Failed { error },
        });
        Ok(())
    }
}
