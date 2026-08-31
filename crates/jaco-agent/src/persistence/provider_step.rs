use super::{PersistenceContext, mutex_clone, mutex_replace, provider_usage};
use crate::{AgentRuntimeError, AgentRuntimeEvent, AgentStep, Result};
use jaco_core::*;
use jaco_db::{
    CompleteProviderStep, NewProviderStep, ProviderStepRecord, UpdateProviderStepStatus,
};
use rig::completion::{AssistantContent, CompletionRequest, CompletionResponse, Usage};
use serde::Serialize;

pub(crate) fn safe_generated_response_body(response: &CompletionResponse) -> ProviderRawPayload {
    let image_count = response
        .choice
        .iter()
        .filter(|content| matches!(content, AssistantContent::Image(_)))
        .count();
    let mut value = response.raw.clone();
    let mut redacted = 0_usize;
    if let Some(choices) = value
        .get_mut("choices")
        .and_then(serde_json::Value::as_array_mut)
    {
        for choice in choices {
            let Some(images) = choice
                .get_mut("message")
                .and_then(|message| message.get_mut("images"))
                .and_then(serde_json::Value::as_array_mut)
            else {
                continue;
            };
            for image in images {
                let Some(locator) = image
                    .get_mut("image_url")
                    .and_then(|image_url| image_url.get_mut("url"))
                else {
                    continue;
                };
                if locator.is_string() {
                    *locator = serde_json::Value::String("[redacted-generated-image]".to_string());
                    redacted += 1;
                }
            }
        }
    }
    if image_count == 0 || redacted != image_count {
        value = serde_json::json!({
            "providerResponse": "redacted_generated_images",
            "imageCount": image_count,
        });
    }
    ProviderRawPayload {
        provider_kind: "rig".to_string(),
        value,
    }
}

impl PersistenceContext {
    pub(super) async fn insert_provider_step(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderStepRecord> {
        self.insert_provider_step_with_context(
            request,
            ProviderTransportSnapshot::ProviderDefault,
            ProviderRequestContextSnapshot::FullHistory,
            None,
        )
        .await
    }

    pub(crate) async fn insert_provider_step_with_context(
        &self,
        request: &CompletionRequest,
        transport: ProviderTransportSnapshot,
        context_mode: ProviderRequestContextSnapshot,
        previous_response_id: Option<String>,
    ) -> Result<ProviderStepRecord> {
        let seq = self
            .persistence
            .next_provider_step_seq(self.agent_run_id.clone())
            .await?;
        let input_item_ids = mutex_clone(&self.input_item_ids);
        let step = self
            .persistence
            .insert_provider_step(NewProviderStep {
                agent_run_id: self.agent_run_id.clone(),
                seq,
                status: ProviderStepStatus::Running,
                request_snapshot: ProviderStepRequestSnapshot {
                    provider_id: self.provider_id.clone(),
                    model_id: self.model_id.clone(),
                    input_item_ids,
                    snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
                    transport,
                    context_mode,
                    previous_response_id,
                    request_body: ProviderRawPayload {
                        provider_kind: "rig".to_string(),
                        value: serde_json::to_value(request)?,
                    },
                },
                response_snapshot: None,
                state_snapshot: None,
                settings_snapshot: self.settings_snapshot.clone(),
                error: None,
            })
            .await?;
        mutex_replace(&self.last_provider_step_id, Some(step.id.clone()));
        self.push_event(AgentRunEvent::ProviderStepStarted {
            provider_step_id: step.id.clone(),
        });
        self.push_step(AgentStep::ProviderStep(step.id.clone()));
        self.emit_runtime(AgentRuntimeEvent::ConversationTimelineChanged {
            conversation_id: self.conversation_id.clone(),
            changes: vec![jaco_core::ConversationChange::ProviderStepChanged {
                step: Box::new(step.clone()),
            }],
        });
        Ok(step)
    }

    pub(super) async fn finish_provider_step(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse,
    ) -> Result<()> {
        self.finish_provider_step_with_options(provider_step_id, response, None, None)
            .await
    }

    pub(super) async fn finish_provider_step_with_response_body(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse,
        response_body: ProviderRawPayload,
    ) -> Result<()> {
        self.finish_provider_step_with_options(
            provider_step_id,
            response,
            None,
            Some(response_body),
        )
        .await
    }

    pub(crate) async fn finish_openai_provider_step(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse,
    ) -> Result<()> {
        let raw: rig::providers::openai::responses_api::CompletionResponse =
            serde_json::from_value(response.raw.clone()).map_err(|error| {
                crate::AgentRuntimeError::Invariant(format!(
                    "OpenAI GPT-5.6 websocket response raw payload was invalid: {error}"
                ))
            })?;
        let reasoning_context = raw.reasoning_context.clone().ok_or_else(|| {
            crate::AgentRuntimeError::Invariant(
                "OpenAI GPT-5.6 websocket response omitted reasoning context".to_string(),
            )
        })?;
        let continuation = ProviderContinuationSnapshot::openai_responses(
            raw.id.clone(),
            reasoning_context,
            time::OffsetDateTime::now_utc(),
        )
        .map_err(|error| crate::AgentRuntimeError::Invariant(error.to_string()))?;
        self.finish_provider_step_with_options(provider_step_id, response, Some(continuation), None)
            .await
    }

    async fn finish_provider_step_with_options(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse,
        continuation: Option<ProviderContinuationSnapshot>,
        response_body_override: Option<ProviderRawPayload>,
    ) -> Result<()> {
        let output_item_ids = response
            .choice
            .iter()
            .filter_map(|content| match content {
                AssistantContent::Reasoning(reasoning) => reasoning.id.clone(),
                _ => None,
            })
            .collect::<Vec<_>>();
        let response_body = response_body_override.unwrap_or_else(|| ProviderRawPayload {
            provider_kind: "rig".to_string(),
            value: response.raw.clone(),
        });
        let state_snapshot = ProviderRunStateSnapshot {
            provider_id: self.provider_id.clone(),
            provider_run_id: response.message_id.clone(),
            output_item_ids,
        };
        let usage = provider_usage(response.usage);
        let cost_amount = self
            .estimate_provider_step_cost(provider_step_id, &usage)
            .await?;
        let provider_outputs = self.take_provider_outputs(provider_step_id);
        let response_snapshot = ProviderStepResponseSnapshot {
            provider_run_id: response.message_id.clone(),
            output_item_ids: state_snapshot.output_item_ids.clone(),
            response_body: Some(response_body),
            provider_outputs: provider_outputs.clone(),
        };
        let completed = match self
            .persistence
            .complete_provider_step_with_usage(
                provider_step_id.to_string(),
                CompleteProviderStep {
                    response_snapshot,
                    state_snapshot: state_snapshot.clone(),
                    continuation,
                    usage: usage.clone(),
                    cost_amount,
                },
            )
            .await
        {
            Ok(completed) => completed,
            Err(error) => {
                self.restore_provider_outputs(provider_step_id, provider_outputs);
                return Err(error.into());
            }
        };
        self.clear_current_provider_step(provider_step_id);
        let step = completed.step;
        self.emit_runtime(AgentRuntimeEvent::ConversationTimelineChanged {
            conversation_id: self.conversation_id.clone(),
            changes: vec![jaco_core::ConversationChange::ProviderStepChanged {
                step: Box::new(step.clone()),
            }],
        });
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

    pub(crate) async fn finish_current_streaming_provider_step<M>(
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
            .await?;
        mutex_replace(&self.last_provider_step_id, None);
        Ok(())
    }

    pub(crate) async fn fail_current_provider_step(&self, error: RunErrorPayload) -> Result<()> {
        let Some(provider_step_id) = mutex_clone(&self.last_provider_step_id) else {
            return Ok(());
        };
        self.fail_provider_step(&provider_step_id, error).await
    }

    pub(crate) async fn cancel_current_provider_step(&self, error: RunErrorPayload) -> Result<()> {
        let Some(provider_step_id) = mutex_clone(&self.last_provider_step_id) else {
            return Ok(());
        };
        self.cancel_provider_step(&provider_step_id, error).await
    }

    pub(crate) async fn finish_streaming_provider_step<M>(
        &self,
        provider_step_id: &str,
        response: Option<&M>,
        usage: Usage,
    ) -> Result<()>
    where
        M: Serialize,
    {
        let raw_response = response.map(serde_json::to_value).transpose()?;
        let openai_continuation = openai_streaming_continuation(
            &self.settings_snapshot.provider_settings.provider_kind,
            &self.model_id,
            raw_response.as_ref(),
        )?;
        let provider_run_id = openai_continuation
            .as_ref()
            .map(|continuation| continuation.response_id.clone());
        let state_snapshot = ProviderRunStateSnapshot {
            provider_id: self.provider_id.clone(),
            provider_run_id: provider_run_id.clone(),
            output_item_ids: Vec::new(),
        };
        let usage = provider_usage(usage);
        let cost_amount = self
            .estimate_provider_step_cost(provider_step_id, &usage)
            .await?;
        let provider_outputs = self.take_provider_outputs(provider_step_id);
        let response_snapshot = ProviderStepResponseSnapshot {
            provider_run_id: provider_run_id.clone(),
            output_item_ids: Vec::new(),
            response_body: raw_response.map(|value| ProviderRawPayload {
                provider_kind: "rig".to_string(),
                value,
            }),
            provider_outputs: provider_outputs.clone(),
        };
        let completed = match self
            .persistence
            .complete_provider_step_with_usage(
                provider_step_id.to_string(),
                CompleteProviderStep {
                    response_snapshot,
                    state_snapshot: state_snapshot.clone(),
                    continuation: openai_continuation,
                    usage: usage.clone(),
                    cost_amount,
                },
            )
            .await
        {
            Ok(completed) => completed,
            Err(error) => {
                self.restore_provider_outputs(provider_step_id, provider_outputs);
                return Err(error.into());
            }
        };
        self.clear_current_provider_step(provider_step_id);
        let step = completed.step;
        self.emit_runtime(AgentRuntimeEvent::ConversationTimelineChanged {
            conversation_id: self.conversation_id.clone(),
            changes: vec![jaco_core::ConversationChange::ProviderStepChanged {
                step: Box::new(step.clone()),
            }],
        });
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

    async fn estimate_provider_step_cost(
        &self,
        provider_step_id: &str,
        usage: &ProviderUsageSnapshot,
    ) -> Result<Option<UsdNanoAmount>> {
        let step = self
            .persistence
            .provider_step(provider_step_id.to_string())
            .await?
            .ok_or_else(|| {
                AgentRuntimeError::Invariant(format!(
                    "provider step `{provider_step_id}` disappeared before completion"
                ))
            })?;
        Ok(step.pricing_snapshot.as_ref().and_then(|pricing| {
            estimate_request_cost(
                &step.settings_snapshot.provider_settings.provider_kind,
                usage,
                pricing,
            )
        }))
    }

    pub(crate) async fn fail_provider_step(
        &self,
        provider_step_id: &str,
        error: RunErrorPayload,
    ) -> Result<()> {
        let step = self
            .terminalize_provider_step(provider_step_id, ProviderStepStatus::Failed, &error)
            .await?;
        if step.status != ProviderStepStatus::Failed {
            return Ok(());
        }
        self.emit_runtime(AgentRuntimeEvent::ConversationTimelineChanged {
            conversation_id: self.conversation_id.clone(),
            changes: vec![jaco_core::ConversationChange::ProviderStepChanged {
                step: Box::new(step.clone()),
            }],
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Failed { error },
        });
        Ok(())
    }

    pub(super) async fn cancel_provider_step(
        &self,
        provider_step_id: &str,
        error: RunErrorPayload,
    ) -> Result<()> {
        let step = self
            .terminalize_provider_step(provider_step_id, ProviderStepStatus::Canceled, &error)
            .await?;
        if step.status != ProviderStepStatus::Canceled {
            return Ok(());
        }
        self.emit_runtime(AgentRuntimeEvent::ConversationTimelineChanged {
            conversation_id: self.conversation_id.clone(),
            changes: vec![jaco_core::ConversationChange::ProviderStepChanged {
                step: Box::new(step.clone()),
            }],
        });
        self.push_event(AgentRunEvent::ProviderStepEvent {
            provider_step_id: provider_step_id.to_string(),
            event: ProviderStepEvent::Failed { error },
        });
        Ok(())
    }

    async fn terminalize_provider_step(
        &self,
        provider_step_id: &str,
        status: ProviderStepStatus,
        error: &RunErrorPayload,
    ) -> Result<ProviderStepRecord> {
        let provider_outputs = self.take_provider_outputs(provider_step_id);
        let response_snapshot = failure_response_snapshot(error, provider_outputs);
        let step = match self
            .persistence
            .update_provider_step_status(
                provider_step_id.to_string(),
                UpdateProviderStepStatus {
                    status,
                    response_snapshot: response_snapshot.clone(),
                    state_snapshot: None,
                    error: Some(error.clone()),
                },
            )
            .await
        {
            Ok(step) => step,
            Err(persistence_error) => {
                self.restore_provider_outputs(
                    provider_step_id,
                    response_snapshot
                        .map(|snapshot| snapshot.provider_outputs)
                        .unwrap_or_default(),
                );
                return Err(persistence_error.into());
            }
        };
        self.clear_current_provider_step(provider_step_id);
        Ok(step)
    }

    pub(crate) fn record_provider_output(&self, output: serde_json::Value) -> Result<()> {
        let provider_step_id = mutex_clone(&self.last_provider_step_id).ok_or_else(|| {
            AgentRuntimeError::Invariant(
                "streaming provider output has no active provider step".to_string(),
            )
        })?;
        let payload = ProviderRawPayload {
            provider_kind: self
                .settings_snapshot
                .provider_settings
                .provider_kind
                .clone(),
            value: output,
        };
        super::lock(&self.provider_outputs)
            .entry(provider_step_id)
            .or_default()
            .push(payload);
        Ok(())
    }

    fn take_provider_outputs(&self, provider_step_id: &str) -> Vec<ProviderRawPayload> {
        super::lock(&self.provider_outputs)
            .remove(provider_step_id)
            .unwrap_or_default()
    }

    fn restore_provider_outputs(
        &self,
        provider_step_id: &str,
        mut provider_outputs: Vec<ProviderRawPayload>,
    ) {
        if provider_outputs.is_empty() {
            return;
        }
        let mut outputs_by_step = super::lock(&self.provider_outputs);
        if let Some(mut later_outputs) = outputs_by_step.remove(provider_step_id) {
            provider_outputs.append(&mut later_outputs);
        }
        outputs_by_step.insert(provider_step_id.to_string(), provider_outputs);
    }

    fn clear_current_provider_step(&self, provider_step_id: &str) {
        let mut current = super::lock(&self.last_provider_step_id);
        if current.as_deref() == Some(provider_step_id) {
            *current = None;
        }
    }
}

fn failure_response_snapshot(
    error: &RunErrorPayload,
    provider_outputs: Vec<ProviderRawPayload>,
) -> Option<ProviderStepResponseSnapshot> {
    if error.raw.is_none() && provider_outputs.is_empty() {
        return None;
    }
    let provider_run_id = error
        .raw
        .as_ref()
        .and_then(|raw| raw.value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    Some(ProviderStepResponseSnapshot {
        provider_run_id,
        output_item_ids: Vec::new(),
        response_body: error.raw.clone(),
        provider_outputs,
    })
}

fn openai_streaming_continuation(
    provider_kind: &str,
    model_id: &str,
    raw_response: Option<&serde_json::Value>,
) -> Result<Option<ProviderContinuationSnapshot>> {
    if provider_kind != "openai" || !model_id.to_ascii_lowercase().starts_with("gpt-5.6") {
        return Ok(None);
    }
    let Some(raw_response) = raw_response else {
        return Ok(None);
    };
    let response_id = raw_response
        .get("reasoning_metadata")
        .and_then(serde_json::Value::as_object)
        .and_then(|metadata| metadata.get("__jaco_response_id"))
        .and_then(serde_json::Value::as_str);
    let reasoning_context = raw_response
        .get("reasoning_context")
        .and_then(serde_json::Value::as_str);
    match (response_id, reasoning_context) {
        (Some(response_id), Some(reasoning_context)) => {
            ProviderContinuationSnapshot::openai_responses(
                response_id.to_string(),
                reasoning_context.to_string(),
                time::OffsetDateTime::now_utc(),
            )
            .map(Some)
            .map_err(|error| crate::AgentRuntimeError::Invariant(error.to_string()))
        }
        _ => Err(crate::AgentRuntimeError::Invariant(
            "OpenAI GPT-5.6 websocket response omitted response ID or reasoning context"
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod generated_response_tests {
    use super::*;
    use rig::{
        completion::Usage,
        message::{AdditionalParams, DocumentSourceKind, Image, ImageMediaType},
    };

    fn image() -> AssistantContent {
        AssistantContent::Image(Image {
            data: DocumentSourceKind::Base64("sensitive-base64".to_string()),
            media_type: Some(ImageMediaType::PNG),
            detail: None,
            additional_params: AdditionalParams::from_entries([(
                "openrouter",
                serde_json::json!({
                    "response_only": true,
                    "source": "assistant.images",
                }),
            )]),
        })
    }

    #[test]
    fn generated_response_body_redacts_every_proven_image_slot() {
        let response = CompletionResponse::new(vec![image()], Usage::new(), "openrouter").with_raw(
            serde_json::json!({
                "id": "response-secret",
                "choices": [{
                    "message": {
                        "images": [{
                            "image_url": { "url": "https://secret.example/image.png?token=x" }
                        }]
                    }
                }]
            }),
        );

        let body = safe_generated_response_body(&response);
        let serialized = serde_json::to_string(&body.value).unwrap();
        assert!(serialized.contains("[redacted-generated-image]"));
        assert!(!serialized.contains("secret.example"));
        assert!(!serialized.contains("token=x"));
        assert!(!serialized.contains("sensitive-base64"));
    }

    #[test]
    fn generated_response_body_falls_back_when_slot_mapping_is_unproven() {
        let response = CompletionResponse::new(vec![image()], Usage::new(), "openrouter").with_raw(
            serde_json::json!({
                "id": "must-not-survive",
                "choices": [{ "message": { "images": [] } }]
            }),
        );

        assert_eq!(
            safe_generated_response_body(&response).value,
            serde_json::json!({
                "providerResponse": "redacted_generated_images",
                "imageCount": 1,
            })
        );
    }
}
