use super::OpenAiReasoningPolicy;
use crate::{AgentPersistence, persistence::PersistenceContext};
use async_stream::try_stream;
use jaco_core::{
    ConversationId, ProviderId, ProviderModelId, ProviderRequestContextSnapshot, ProviderStepId,
};
use rig::{
    OneOrMany,
    completion::{
        CompletionError, CompletionModel, CompletionRequest, CompletionResponse, GetTokenUsage,
    },
    message::ReasoningContent,
    providers::openai::{
        self,
        responses_api::{
            CompletionResponse as OpenAiCompletionResponse, Output, ReasoningSummary,
            ResponseStatus, ResponsesUsage,
            streaming::{
                ItemChunk, ItemChunkKind, ResponseChunkKind,
                StreamingCompletionResponse as OpenAiStreamingResponse,
            },
            websocket::{ResponsesWebSocketEvent, ResponsesWebSocketSession},
        },
    },
    streaming::{
        RawStreamingChoice, RawStreamingToolCall, StreamingCompletionResponse, ToolCallDeltaContent,
    },
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OwnedMutexGuard};

const MAX_SESSION_AGE: Duration = Duration::from_secs(55 * 60);
const EVENT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OpenAiSessionKey {
    conversation_id: ConversationId,
    provider_id: ProviderId,
    model_id: ProviderModelId,
    connection_fingerprint: String,
}

impl OpenAiSessionKey {
    pub(crate) fn new(
        conversation_id: ConversationId,
        provider_id: ProviderId,
        model_id: ProviderModelId,
        base_url: &str,
        api_key: &str,
    ) -> Self {
        let mut hash = Sha256::new();
        hash.update(base_url.trim_end_matches('/').as_bytes());
        hash.update([0]);
        hash.update(api_key.as_bytes());
        Self {
            conversation_id,
            provider_id,
            model_id,
            connection_fingerprint: hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        }
    }
}

struct OpenAiConversationSession {
    key: Option<OpenAiSessionKey>,
    session: Option<ResponsesWebSocketSession>,
    opened_at: Option<Instant>,
}

#[derive(Clone, Default)]
pub struct OpenAiResponsesSessionPool {
    conversations: Arc<Mutex<HashMap<ConversationId, Arc<Mutex<OpenAiConversationSession>>>>>,
}

impl OpenAiResponsesSessionPool {
    pub fn new() -> Self {
        Self::default()
    }

    async fn acquire(
        &self,
        key: &OpenAiSessionKey,
        client: &openai::Client,
        model: &str,
    ) -> std::result::Result<OwnedMutexGuard<OpenAiConversationSession>, CompletionError> {
        let mut guard = self.lock_conversation(&key.conversation_id).await;
        let replace_current = guard.key.as_ref().is_some_and(|current| current != key)
            || guard
                .opened_at
                .is_some_and(|opened_at| opened_at.elapsed() >= MAX_SESSION_AGE);
        if replace_current {
            Self::close_session(&mut guard).await;
        }
        if guard.session.is_none() {
            guard.session = Some(
                client
                    .responses_websocket_builder(model)
                    .event_timeout(EVENT_TIMEOUT)
                    .connect()
                    .await?,
            );
            guard.key = Some(key.clone());
            guard.opened_at = Some(Instant::now());
        }
        Ok(guard)
    }

    async fn lock_conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> OwnedMutexGuard<OpenAiConversationSession> {
        self.conversation_slot(conversation_id)
            .await
            .lock_owned()
            .await
    }

    async fn conversation_slot(
        &self,
        conversation_id: &ConversationId,
    ) -> Arc<Mutex<OpenAiConversationSession>> {
        let mut conversations = self.conversations.lock().await;
        conversations
            .entry(conversation_id.clone())
            .or_insert_with(|| {
                Arc::new(Mutex::new(OpenAiConversationSession {
                    key: None,
                    session: None,
                    opened_at: None,
                }))
            })
            .clone()
    }

    #[cfg(test)]
    pub(crate) async fn seed_conversation_for_test(&self, conversation_id: &ConversationId) {
        self.conversation_slot(conversation_id).await;
    }

    #[cfg(test)]
    pub(crate) async fn contains_conversation_for_test(
        &self,
        conversation_id: &ConversationId,
    ) -> bool {
        self.conversations
            .lock()
            .await
            .contains_key(conversation_id)
    }

    async fn evict_unusable_session(
        &self,
        key: &OpenAiSessionKey,
        guard: &mut OwnedMutexGuard<OpenAiConversationSession>,
    ) {
        let current_slot = OwnedMutexGuard::mutex(guard).clone();
        let session = guard.session.take();
        guard.key = None;
        guard.opened_at = None;

        let mut conversations = self.conversations.lock().await;
        if conversations
            .get(&key.conversation_id)
            .is_some_and(|slot| Arc::ptr_eq(slot, &current_slot))
        {
            conversations.remove(&key.conversation_id);
        }
        drop(conversations);

        if let Some(mut session) = session {
            let _ = session.close().await;
        }
    }

    async fn close_session(slot: &mut OpenAiConversationSession) {
        let session = slot.session.take();
        slot.key = None;
        slot.opened_at = None;
        if let Some(mut session) = session {
            let _ = session.close().await;
        }
    }

    pub async fn close_conversation(&self, conversation_id: &ConversationId) {
        let slot = {
            let mut conversations = self.conversations.lock().await;
            conversations.remove(conversation_id)
        };
        if let Some(slot) = slot {
            let mut slot = slot.lock().await;
            Self::close_session(&mut slot).await;
        }
    }

    pub async fn close_all(&self) {
        let slots = {
            let mut conversations = self.conversations.lock().await;
            std::mem::take(&mut *conversations)
                .into_values()
                .collect::<Vec<_>>()
        };
        for slot in slots {
            let mut slot = slot.lock().await;
            Self::close_session(&mut slot).await;
        }
    }
}

#[derive(Clone)]
pub(crate) struct OpenAiWebSocketModelClient {
    pub(crate) client: openai::Client,
    pub(crate) pool: OpenAiResponsesSessionPool,
    pub(crate) key: OpenAiSessionKey,
    pub(crate) reasoning: OpenAiReasoningPolicy,
    pub(crate) previous_response_id: Option<String>,
    pub(crate) previous_source_step_id: Option<ProviderStepId>,
    pub(crate) persistence: Arc<dyn AgentPersistence>,
    pub(crate) attempts: OpenAiAttemptCoordinator,
}

#[derive(Clone, Default)]
pub(crate) struct OpenAiAttemptCoordinator {
    context: Arc<Mutex<Option<PersistenceContext>>>,
}

impl OpenAiAttemptCoordinator {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn bind(&self, context: PersistenceContext) {
        *self.context.lock().await = Some(context);
    }

    async fn context(&self) -> std::result::Result<PersistenceContext, CompletionError> {
        self.context.lock().await.clone().ok_or_else(|| {
            CompletionError::ProviderError(
                "OpenAI websocket persistence context is unavailable".to_string(),
            )
        })
    }

    async fn begin(
        &self,
        request: &CompletionRequest,
        context_mode: ProviderRequestContextSnapshot,
        previous_response_id: Option<String>,
    ) -> std::result::Result<ProviderStepId, CompletionError> {
        self.context()
            .await?
            .insert_provider_step_with_context(
                request,
                jaco_core::ProviderTransportSnapshot::WebSocket,
                context_mode,
                previous_response_id,
            )
            .await
            .map(|step| step.id)
            .map_err(completion_request_error)
    }

    async fn complete_blocking(
        &self,
        provider_step_id: &str,
        response: &CompletionResponse<OpenAiCompletionResponse>,
    ) -> std::result::Result<(), CompletionError> {
        self.context()
            .await?
            .finish_openai_provider_step(provider_step_id, response)
            .await
            .map_err(completion_request_error)
    }

    async fn complete_streaming(
        &self,
        response: &OpenAiStreamingResponse,
    ) -> std::result::Result<(), CompletionError> {
        self.context()
            .await?
            .finish_current_streaming_provider_step(Some(response), response.token_usage())
            .await
            .map_err(completion_request_error)
    }

    async fn fail(&self, provider_step_id: &str, error: jaco_core::RunErrorPayload) {
        if let Ok(context) = self.context().await {
            let _ = context.fail_provider_step(provider_step_id, error).await;
        }
    }
}

fn completion_request_error(error: crate::AgentRuntimeError) -> CompletionError {
    CompletionError::RequestError(Box::new(error))
}

#[derive(Clone, Debug)]
struct OpenAiRunContinuationState {
    source_step_id: Option<ProviderStepId>,
    source_response_id: Option<String>,
    documents_sent_in_run: bool,
    full_history_fallback_used: bool,
}

impl OpenAiRunContinuationState {
    fn new(source_step_id: Option<ProviderStepId>, source_response_id: Option<String>) -> Self {
        Self {
            source_step_id,
            source_response_id,
            documents_sent_in_run: false,
            full_history_fallback_used: false,
        }
    }
}

#[derive(Clone)]
pub(crate) struct OpenAiWebSocketCompletionModel {
    binding: OpenAiWebSocketModelClient,
    model: String,
    state: Arc<Mutex<OpenAiRunContinuationState>>,
}

impl CompletionModel for OpenAiWebSocketCompletionModel {
    type Response = OpenAiCompletionResponse;
    type StreamingResponse = OpenAiStreamingResponse;
    type Client = OpenAiWebSocketModelClient;

    fn make(client: &Self::Client, model: impl Into<String>) -> Self {
        Self {
            binding: client.clone(),
            model: model.into(),
            state: Arc::new(Mutex::new(OpenAiRunContinuationState::new(
                client.previous_source_step_id.clone(),
                client.previous_response_id.clone(),
            ))),
        }
    }

    fn composes_native_output_with_tools(&self) -> bool {
        true
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        let mut guard = self
            .binding
            .pool
            .acquire(&self.binding.key, &self.binding.client, &self.model)
            .await?;
        let session = guard.session.as_mut().ok_or_else(|| {
            CompletionError::ProviderError("OpenAI websocket session is unavailable".to_string())
        })?;
        let original_request = request.clone();
        let mut fallback = false;
        let mut fallback_available = !self.state.lock().await.full_history_fallback_used;
        loop {
            let prepared = self
                .prepare_request(
                    original_request.clone(),
                    session.previous_response_id(),
                    fallback,
                )
                .await?;
            if prepared.context != ProviderRequestContextSnapshot::PreviousResponse {
                session.clear_previous_response_id();
            }
            let provider_step_id = self
                .binding
                .attempts
                .begin(
                    &prepared.request,
                    prepared.context,
                    prepared.previous_response_id.clone(),
                )
                .await?;
            match session.completion(prepared.request).await {
                Ok(response) => {
                    if let Err(error) = self
                        .binding
                        .attempts
                        .complete_blocking(&provider_step_id, &response)
                        .await
                    {
                        session.clear_previous_response_id();
                        self.binding
                            .attempts
                            .fail(
                                &provider_step_id,
                                local_commit_error(
                                    &error,
                                    serde_json::to_value(&response.raw_response).ok(),
                                ),
                            )
                            .await;
                        return Err(error);
                    }
                    self.mark_completed(
                        provider_step_id,
                        response.raw_response.id.clone(),
                        prepared.included_documents,
                    )
                    .await;
                    return Ok(response);
                }
                Err(error)
                    if prepared.context == ProviderRequestContextSnapshot::PreviousResponse
                        && fallback_available
                        && previous_response_rejected(&error) =>
                {
                    let rejection = continuation_rejection_error(
                        error.to_string(),
                        error.provider_response_json().ok().flatten(),
                    );
                    self.binding
                        .attempts
                        .fail(&provider_step_id, rejection.clone())
                        .await;
                    self.invalidate_source_continuation(rejection).await;
                    session.clear_previous_response_id();
                    self.mark_fallback_used().await;
                    fallback_available = false;
                    fallback = true;
                }
                Err(error) => {
                    self.binding
                        .attempts
                        .fail(
                            &provider_step_id,
                            provider_attempt_error(
                                error.to_string(),
                                error.provider_response_json().ok().flatten(),
                            ),
                        )
                        .await;
                    return Err(error);
                }
            }
        }
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        let binding = self.binding.clone();
        let model = self.model.clone();
        let this = self.clone();
        let stream = try_stream! {
            let mut guard = binding.pool.acquire(&binding.key, &binding.client, &model).await?;
            let original_request = request.clone();
            let mut fallback = false;
            let mut fallback_available = !this.state.lock().await.full_history_fallback_used;
            'attempt: loop {
                let session_previous_response_id = guard
                    .session
                    .as_ref()
                    .ok_or_else(session_unavailable)?
                    .previous_response_id()
                    .map(str::to_owned);
                let prepared = this
                    .prepare_request(
                        original_request.clone(),
                        session_previous_response_id.as_deref(),
                        fallback,
                    )
                    .await?;
                if prepared.context != ProviderRequestContextSnapshot::PreviousResponse {
                    guard
                        .session
                        .as_mut()
                        .ok_or_else(session_unavailable)?
                        .clear_previous_response_id();
                }
                let provider_step_id = binding
                    .attempts
                    .begin(
                        &prepared.request,
                        prepared.context,
                        prepared.previous_response_id.clone(),
                    )
                    .await?;
                let mut item_decoder = OpenAiWebSocketItemDecoder::default();
                if let Err(error) = guard
                    .session
                    .as_mut()
                    .ok_or_else(session_unavailable)?
                    .send(prepared.request)
                    .await
                {
                    binding
                        .attempts
                        .fail(&provider_step_id, provider_attempt_error(error.to_string(), None))
                        .await;
                    binding
                        .pool
                        .evict_unusable_session(&binding.key, &mut guard)
                        .await;
                    Err(error)?;
                }
                loop {
                    let event = match guard
                        .session
                        .as_mut()
                        .ok_or_else(session_unavailable)?
                        .next_event()
                        .await
                    {
                        Ok(event) => event,
                        Err(error) => {
                            binding
                                .attempts
                                .fail(
                                    &provider_step_id,
                                    provider_attempt_error(error.to_string(), None),
                                )
                                .await;
                            binding
                                .pool
                                .evict_unusable_session(&binding.key, &mut guard)
                                .await;
                            Err(error)?
                        }
                    };
                    let completed_response = match event {
                        ResponsesWebSocketEvent::Item(item) => {
                            for choice in item_decoder.decode(item) {
                                yield choice;
                            }
                            None
                        }
                        ResponsesWebSocketEvent::Response(chunk) => match chunk.kind {
                            ResponseChunkKind::ResponseCompleted => {
                                Some(chunk.response)
                            }
                            ResponseChunkKind::ResponseFailed
                            | ResponseChunkKind::ResponseIncomplete => {
                                let error = provider_terminal_error(&chunk.response);
                                binding
                                    .attempts
                                    .fail(
                                        &provider_step_id,
                                        provider_attempt_error(
                                            error.to_string(),
                                            serde_json::to_value(&chunk.response).ok(),
                                        ),
                                    )
                                    .await;
                                Err::<Option<OpenAiCompletionResponse>, CompletionError>(error)?
                            }
                            ResponseChunkKind::ResponseCreated
                            | ResponseChunkKind::ResponseInProgress => None,
                        },
                        ResponsesWebSocketEvent::Error(error)
                            if prepared.context
                                == ProviderRequestContextSnapshot::PreviousResponse
                                && fallback_available
                                && websocket_previous_response_rejected(&error) =>
                        {
                            let rejection = continuation_rejection_error(
                                error.to_string(),
                                serde_json::to_value(&error).ok(),
                            );
                            binding
                                .attempts
                                .fail(&provider_step_id, rejection.clone())
                                .await;
                            this.invalidate_source_continuation(rejection).await;
                            guard
                                .session
                                .as_mut()
                                .ok_or_else(session_unavailable)?
                                .clear_previous_response_id();
                            this.mark_fallback_used().await;
                            fallback_available = false;
                            fallback = true;
                            continue 'attempt;
                        }
                        ResponsesWebSocketEvent::Error(error) => {
                            binding
                                .attempts
                                .fail(
                                    &provider_step_id,
                                    provider_attempt_error(
                                        error.to_string(),
                                        serde_json::to_value(&error).ok(),
                                    ),
                                )
                                .await;
                            Err::<Option<OpenAiCompletionResponse>, CompletionError>(
                                CompletionError::ProviderError(error.to_string()),
                            )?
                        }
                        ResponsesWebSocketEvent::Done(done) => {
                            let raw = done.response;
                            let response = match completion_response_from_done(&raw) {
                                Ok(response) => response,
                                Err(error) => {
                                    guard
                                        .session
                                        .as_mut()
                                        .ok_or_else(session_unavailable)?
                                        .clear_previous_response_id();
                                    binding
                                        .attempts
                                        .fail(
                                            &provider_step_id,
                                            provider_attempt_error(
                                                error.to_string(),
                                                Some(raw),
                                            ),
                                        )
                                        .await;
                                    Err::<OpenAiCompletionResponse, CompletionError>(error)?
                                }
                            };
                            if response.status != ResponseStatus::Completed {
                                let error = provider_terminal_error(&response);
                                guard
                                    .session
                                    .as_mut()
                                    .ok_or_else(session_unavailable)?
                                    .clear_previous_response_id();
                                binding
                                    .attempts
                                    .fail(
                                        &provider_step_id,
                                        provider_attempt_error(
                                            error.to_string(),
                                            serde_json::to_value(&response).ok(),
                                        ),
                                    )
                                    .await;
                                Err(error)?;
                            }
                            Some(response)
                        }
                    };
                    if let Some(response) = completed_response {
                        let usage = response.usage.clone().unwrap_or_else(empty_usage);
                        let mut reasoning_metadata =
                            response.reasoning_metadata.clone().unwrap_or_default();
                        reasoning_metadata.insert(
                            "__jaco_response_id".to_string(),
                            serde_json::Value::String(response.id.clone()),
                        );
                        let final_response = OpenAiStreamingResponse {
                            usage,
                            reasoning_metadata: Some(reasoning_metadata),
                            reasoning_context: response.reasoning_context.clone(),
                        };
                        if let Err(error) =
                            binding.attempts.complete_streaming(&final_response).await
                        {
                            guard
                                .session
                                .as_mut()
                                .ok_or_else(session_unavailable)?
                                .clear_previous_response_id();
                            binding
                                .attempts
                                .fail(
                                    &provider_step_id,
                                    local_commit_error(
                                        &error,
                                        serde_json::to_value(&response).ok(),
                                    ),
                                )
                                .await;
                            Err(error)?;
                        }
                        this.mark_completed(
                            provider_step_id,
                            response.id,
                            prepared.included_documents,
                        )
                        .await;
                        yield RawStreamingChoice::FinalResponse(final_response);
                        break 'attempt;
                    }
                }
            }
        };
        Ok(StreamingCompletionResponse::stream(Box::pin(stream)))
    }
}

fn session_unavailable() -> CompletionError {
    CompletionError::ProviderError("OpenAI websocket session is unavailable".to_string())
}

struct PreparedOpenAiRequest {
    request: CompletionRequest,
    context: ProviderRequestContextSnapshot,
    previous_response_id: Option<String>,
    included_documents: bool,
}

impl OpenAiWebSocketCompletionModel {
    async fn prepare_request(
        &self,
        request: CompletionRequest,
        session_previous_response_id: Option<&str>,
        full_history_fallback: bool,
    ) -> std::result::Result<PreparedOpenAiRequest, CompletionError> {
        let state = self.state.lock().await.clone();
        prepare_openai_request(
            &self.binding.reasoning,
            &state,
            request,
            session_previous_response_id,
            full_history_fallback,
        )
    }

    async fn invalidate_source_continuation(&self, error: jaco_core::RunErrorPayload) {
        let source_step_id = {
            let mut state = self.state.lock().await;
            state.source_response_id = None;
            state.source_step_id.take()
        };
        if let Some(source_step_id) = source_step_id {
            let _ = self
                .binding
                .persistence
                .invalidate_provider_continuation(
                    source_step_id,
                    time::OffsetDateTime::now_utc(),
                    error,
                )
                .await;
        }
    }

    async fn mark_completed(
        &self,
        provider_step_id: ProviderStepId,
        response_id: String,
        included_documents: bool,
    ) {
        let mut state = self.state.lock().await;
        state.source_step_id = Some(provider_step_id);
        state.source_response_id = Some(response_id);
        if included_documents {
            state.documents_sent_in_run = true;
        }
    }

    async fn mark_fallback_used(&self) {
        self.state.lock().await.full_history_fallback_used = true;
    }
}

fn prepare_openai_request(
    reasoning: &OpenAiReasoningPolicy,
    state: &OpenAiRunContinuationState,
    mut request: CompletionRequest,
    session_previous_response_id: Option<&str>,
    full_history_fallback: bool,
) -> std::result::Result<PreparedOpenAiRequest, CompletionError> {
    let previous_response_id = state.source_response_id.clone();
    let context = if full_history_fallback {
        ProviderRequestContextSnapshot::FullHistoryFallback
    } else if previous_response_id.is_some() {
        ProviderRequestContextSnapshot::PreviousResponse
    } else {
        ProviderRequestContextSnapshot::FullHistory
    };
    let policy = reasoning.clone().for_request_context(context);
    let mut parameters = policy
        .merge_into_request_params(request.additional_params.take())
        .map_err(|error| CompletionError::RequestError(Box::new(error)))?;

    if !full_history_fallback && let Some(previous_response_id) = previous_response_id.as_ref() {
        request.chat_history = incremental_history(request.chat_history)?;
        if state.documents_sent_in_run {
            request.documents.clear();
        }
        if session_previous_response_id != Some(previous_response_id.as_str()) {
            parameters
                .as_object_mut()
                .expect("reasoning policy always returns an object")
                .insert(
                    "previous_response_id".to_string(),
                    serde_json::Value::String(previous_response_id.clone()),
                );
        }
    }
    let included_documents = !request.documents.is_empty();
    request.additional_params = Some(parameters);
    Ok(PreparedOpenAiRequest {
        request,
        context,
        previous_response_id: (!full_history_fallback)
            .then_some(previous_response_id)
            .flatten(),
        included_documents,
    })
}

fn incremental_history(
    history: OneOrMany<rig::completion::Message>,
) -> std::result::Result<OneOrMany<rig::completion::Message>, CompletionError> {
    let messages = history.into_iter().collect::<Vec<_>>();
    let mut selected = messages
        .iter()
        .take_while(|message| matches!(message, rig::completion::Message::System { .. }))
        .cloned()
        .collect::<Vec<_>>();
    if let Some(last) = messages
        .iter()
        .rev()
        .find(|message| !matches!(message, rig::completion::Message::System { .. }))
    {
        selected.push(last.clone());
    }
    OneOrMany::many(selected).map_err(|error| CompletionError::RequestError(Box::new(error)))
}

#[derive(Default)]
struct OpenAiWebSocketItemDecoder {
    tool_call_internal_ids: HashMap<String, String>,
}

impl OpenAiWebSocketItemDecoder {
    fn decode(&mut self, item: ItemChunk) -> Vec<RawStreamingChoice<OpenAiStreamingResponse>> {
        let item_id = item.item_id;
        match item.data {
            ItemChunkKind::OutputItemAdded(added) => match added.item {
                Output::FunctionCall(call) => {
                    let internal_call_id = self.tool_call_internal_id(&call.id);
                    vec![RawStreamingChoice::ToolCallDelta {
                        id: call.id,
                        internal_call_id,
                        content: ToolCallDeltaContent::Name(call.name),
                    }]
                }
                _ => Vec::new(),
            },
            ItemChunkKind::OutputTextDelta(delta) => {
                vec![RawStreamingChoice::Message(delta.delta)]
            }
            ItemChunkKind::RefusalDelta(delta) => {
                vec![RawStreamingChoice::Message(delta.delta)]
            }
            ItemChunkKind::ReasoningSummaryTextDelta(delta) => {
                vec![RawStreamingChoice::ReasoningDelta {
                    id: item_id,
                    reasoning: delta.delta,
                }]
            }
            ItemChunkKind::ReasoningTextDelta(delta) => {
                vec![RawStreamingChoice::ReasoningDelta {
                    id: item_id,
                    reasoning: delta.delta,
                }]
            }
            ItemChunkKind::FunctionCallArgsDelta(delta) => {
                let Some(item_id) = item_id else {
                    return Vec::new();
                };
                let internal_call_id = self.tool_call_internal_id(&item_id);
                vec![RawStreamingChoice::ToolCallDelta {
                    id: item_id,
                    internal_call_id,
                    content: ToolCallDeltaContent::Delta(delta.delta),
                }]
            }
            ItemChunkKind::OutputItemDone(done) => self.decode_completed_output(done.item),
            _ => Vec::new(),
        }
    }

    fn decode_completed_output(
        &mut self,
        output: Output,
    ) -> Vec<RawStreamingChoice<OpenAiStreamingResponse>> {
        match output {
            Output::FunctionCall(call) => {
                let internal_call_id = self.tool_call_internal_id(&call.id);
                vec![RawStreamingChoice::ToolCall(
                    RawStreamingToolCall::new(call.id, call.name, call.arguments)
                        .with_internal_call_id(internal_call_id)
                        .with_call_id(call.call_id),
                )]
            }
            Output::Message(message) => vec![RawStreamingChoice::MessageId(message.id)],
            Output::Unknown(value) => vec![RawStreamingChoice::Unknown(value)],
            Output::Reasoning {
                id,
                summary,
                content,
                encrypted_content,
                ..
            } => reasoning_choices_from_done_item(
                &id,
                &summary,
                &content,
                encrypted_content.as_deref(),
            ),
        }
    }

    fn tool_call_internal_id(&mut self, item_id: &str) -> String {
        self.tool_call_internal_ids
            .entry(item_id.to_string())
            .or_insert_with(rig::id::generate)
            .clone()
    }
}

fn reasoning_choices_from_done_item(
    id: &str,
    summary: &[ReasoningSummary],
    content: &[String],
    encrypted_content: Option<&str>,
) -> Vec<RawStreamingChoice<OpenAiStreamingResponse>> {
    let mut choices = summary
        .iter()
        .map(|summary| RawStreamingChoice::Reasoning {
            id: Some(id.to_string()),
            content: ReasoningContent::Summary(summary.text()),
        })
        .collect::<Vec<_>>();
    choices.extend(content.iter().map(|text| RawStreamingChoice::Reasoning {
        id: Some(id.to_string()),
        content: ReasoningContent::Text {
            text: text.clone(),
            signature: None,
        },
    }));
    if let Some(encrypted_content) = encrypted_content.filter(|content| !content.is_empty()) {
        choices.push(RawStreamingChoice::Reasoning {
            id: Some(id.to_string()),
            content: ReasoningContent::Encrypted(encrypted_content.to_string()),
        });
    }
    choices
}

fn completion_response_from_done(
    response: &serde_json::Value,
) -> std::result::Result<OpenAiCompletionResponse, CompletionError> {
    serde_json::from_value(response.clone()).map_err(|error| {
        let response_id = response
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(|id| format!(" (response_id={id})"))
            .unwrap_or_default();
        CompletionError::ProviderError(format!(
            "OpenAI websocket turn ended with response.done before a complete response body was available{response_id}: {error}"
        ))
    })
}

fn empty_usage() -> ResponsesUsage {
    ResponsesUsage {
        input_tokens: 0,
        input_tokens_details: None,
        output_tokens: 0,
        output_tokens_details: None,
        total_tokens: 0,
    }
}

fn provider_terminal_error(response: &OpenAiCompletionResponse) -> CompletionError {
    CompletionError::ProviderError(format!(
        "OpenAI websocket response {} ended with status {:?}",
        response.id, response.status
    ))
}

fn previous_response_rejected(error: &CompletionError) -> bool {
    error
        .provider_response_json()
        .ok()
        .flatten()
        .is_some_and(|body| {
            let error = body.get("error").unwrap_or(&body);
            error.get("code").and_then(serde_json::Value::as_str)
                == Some("previous_response_not_found")
                || error.get("param").and_then(serde_json::Value::as_str)
                    == Some("previous_response_id")
        })
}

fn websocket_previous_response_rejected(
    error: &rig::providers::openai::responses_api::websocket::ResponsesWebSocketErrorEvent,
) -> bool {
    error.error.code.as_deref() == Some("previous_response_not_found")
        || error
            .error
            .extra
            .get("param")
            .and_then(serde_json::Value::as_str)
            == Some("previous_response_id")
}

fn provider_attempt_error(
    message: String,
    raw: Option<serde_json::Value>,
) -> jaco_core::RunErrorPayload {
    jaco_core::RunErrorPayload {
        code: "provider_error".to_string(),
        message,
        retryable: true,
        provider: Some("openai".to_string()),
        raw: raw.map(|value| jaco_core::ProviderRawPayload {
            provider_kind: "openai".to_string(),
            value,
        }),
    }
}

fn local_commit_error(
    error: &CompletionError,
    raw: Option<serde_json::Value>,
) -> jaco_core::RunErrorPayload {
    jaco_core::RunErrorPayload {
        code: "local_provider_commit_failed".to_string(),
        message: error.to_string(),
        retryable: true,
        provider: Some("openai".to_string()),
        raw: raw.map(|value| jaco_core::ProviderRawPayload {
            provider_kind: "openai".to_string(),
            value,
        }),
    }
}

fn continuation_rejection_error(
    message: String,
    raw: Option<serde_json::Value>,
) -> jaco_core::RunErrorPayload {
    jaco_core::RunErrorPayload {
        code: "previous_response_id_rejected".to_string(),
        message,
        retryable: true,
        provider: Some("openai".to_string()),
        raw: raw.map(|value| jaco_core::ProviderRawPayload {
            provider_kind: "openai".to_string(),
            value,
        }),
    }
}

pub(crate) fn official_gpt_5_6_websocket(
    model_id: &str,
    configured_base_url: Option<&str>,
    stateful_response_continuation: bool,
) -> bool {
    let official_endpoint = configured_base_url.is_none_or(|base_url| {
        let normalized = base_url.trim().trim_end_matches('/');
        normalized == "https://api.openai.com" || normalized == "https://api.openai.com/v1"
    });
    official_endpoint
        && stateful_response_continuation
        && model_id.to_ascii_lowercase().starts_with("gpt-5.6")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::{
        completion::{Document, Message, ToolDefinition},
        providers::openai::responses_api::{
            ReasoningEffort, streaming::ItemChunk, websocket::ResponsesWebSocketDoneEvent,
        },
    };
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn websocket_selection_is_limited_to_official_gpt_5_6() {
        assert!(official_gpt_5_6_websocket("gpt-5.6", None, true));
        assert!(official_gpt_5_6_websocket(
            "GPT-5.6-codex",
            Some(" https://api.openai.com/v1/ "),
            true
        ));
        assert!(!official_gpt_5_6_websocket("gpt-5.5", None, true));
        assert!(!official_gpt_5_6_websocket("gpt-5.6", None, false));
        assert!(!official_gpt_5_6_websocket(
            "gpt-5.6",
            Some("https://proxy.example.com/v1"),
            true
        ));
    }

    #[test]
    fn incremental_history_keeps_leading_system_messages_and_latest_turn() {
        let history = OneOrMany::many(vec![
            Message::system("system"),
            Message::system("developer"),
            Message::user("old user"),
            Message::assistant("old assistant"),
            Message::user("latest user"),
        ])
        .unwrap();

        let selected = incremental_history(history)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(selected.len(), 3);
        assert!(matches!(&selected[0], Message::System { content } if content == "system"));
        assert!(matches!(&selected[1], Message::System { content } if content == "developer"));
        assert_eq!(selected[2], Message::user("latest user"));
    }

    #[test]
    fn websocket_error_detects_previous_response_rejection_by_code_or_param() {
        let by_code = serde_json::from_value(json!({
            "type": "error",
            "error": {
                "code": "previous_response_not_found",
                "message": "expired"
            }
        }))
        .unwrap();
        let by_param = serde_json::from_value(json!({
            "type": "error",
            "error": {
                "code": "invalid_request_error",
                "param": "previous_response_id"
            }
        }))
        .unwrap();

        assert!(websocket_previous_response_rejected(&by_code));
        assert!(websocket_previous_response_rejected(&by_param));
    }

    #[test]
    fn complete_done_payload_decodes_terminal_response() {
        let done = serde_json::from_value::<ResponsesWebSocketDoneEvent>(json!({
            "type": "response.done",
            "response": {
                "id": "resp_done",
                "object": "response",
                "created_at": 0,
                "status": "completed",
                "error": null,
                "incomplete_details": null,
                "instructions": null,
                "max_output_tokens": null,
                "model": "gpt-5.6",
                "usage": {
                    "input_tokens": 3,
                    "output_tokens": 5,
                    "total_tokens": 8
                },
                "output": [],
                "tools": []
            }
        }))
        .unwrap();

        let response = completion_response_from_done(&done.response).unwrap();

        assert_eq!(response.id, "resp_done");
        assert_eq!(response.status, ResponseStatus::Completed);
        assert_eq!(response.usage.unwrap().total_tokens, 8);
    }

    #[test]
    fn incomplete_done_payload_returns_terminal_error() {
        let done = serde_json::from_value::<ResponsesWebSocketDoneEvent>(json!({
            "type": "response.done",
            "response": {
                "id": "resp_incomplete_done",
                "status": "completed"
            }
        }))
        .unwrap();

        let error = completion_response_from_done(&done.response).unwrap_err();

        let message = error.to_string();
        assert!(message.contains("response.done"));
        assert!(message.contains("complete response body"));
        assert!(message.contains("resp_incomplete_done"));
    }

    #[test]
    fn refusal_delta_is_streamed_as_message_text() {
        let item = serde_json::from_value::<ItemChunk>(json!({
            "type": "response.refusal.delta",
            "item_id": "msg_refusal",
            "output_index": 0,
            "content_index": 0,
            "sequence_number": 1,
            "delta": "I can’t help with that request."
        }))
        .unwrap();

        let choices = OpenAiWebSocketItemDecoder::default().decode(item);

        assert!(matches!(
            choices.as_slice(),
            [RawStreamingChoice::Message(text)] if text == "I can’t help with that request."
        ));
    }

    #[test]
    fn function_call_sequence_uses_one_internal_id_and_emits_name_first() {
        let added = serde_json::from_value::<ItemChunk>(json!({
            "type": "response.output_item.added",
            "item_id": "fc_weather",
            "output_index": 0,
            "sequence_number": 1,
            "item": {
                "type": "function_call",
                "id": "fc_weather",
                "arguments": "{}",
                "call_id": "call_weather",
                "name": "get_weather",
                "status": "in_progress"
            }
        }))
        .unwrap();
        let arguments = serde_json::from_value::<ItemChunk>(json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_weather",
            "output_index": 0,
            "content_index": 0,
            "sequence_number": 2,
            "delta": "{\"city\":\"Paris\"}"
        }))
        .unwrap();
        let done = serde_json::from_value::<ItemChunk>(json!({
            "type": "response.output_item.done",
            "item_id": "fc_weather",
            "output_index": 0,
            "sequence_number": 3,
            "item": {
                "type": "function_call",
                "id": "fc_weather",
                "arguments": "{\"city\":\"Paris\"}",
                "call_id": "call_weather",
                "name": "get_weather",
                "status": "completed"
            }
        }))
        .unwrap();

        let mut decoder = OpenAiWebSocketItemDecoder::default();
        let name = decoder.decode(added);
        let arguments = decoder.decode(arguments);
        let completed = decoder.decode(done);

        let name_internal_id = match name.as_slice() {
            [
                RawStreamingChoice::ToolCallDelta {
                    id,
                    internal_call_id,
                    content: ToolCallDeltaContent::Name(name),
                },
            ] => {
                assert_eq!(id, "fc_weather");
                assert_eq!(name, "get_weather");
                internal_call_id.clone()
            }
            choices => panic!("expected tool name delta, got {choices:?}"),
        };
        let arguments_internal_id = match arguments.as_slice() {
            [
                RawStreamingChoice::ToolCallDelta {
                    id,
                    internal_call_id,
                    content: ToolCallDeltaContent::Delta(arguments),
                },
            ] => {
                assert_eq!(id, "fc_weather");
                assert_eq!(arguments, "{\"city\":\"Paris\"}");
                internal_call_id.clone()
            }
            choices => panic!("expected tool arguments delta, got {choices:?}"),
        };
        let completed_call = match completed.as_slice() {
            [RawStreamingChoice::ToolCall(call)] => call,
            choices => panic!("expected completed tool call, got {choices:?}"),
        };

        assert_eq!(name_internal_id, arguments_internal_id);
        assert_eq!(completed_call.internal_call_id, name_internal_id);
        assert_eq!(completed_call.id, "fc_weather");
        assert_eq!(completed_call.call_id.as_deref(), Some("call_weather"));
        assert_eq!(completed_call.name, "get_weather");
        assert_eq!(completed_call.arguments, json!({"city": "Paris"}));
    }

    #[test]
    fn completed_reasoning_item_preserves_canonical_blocks() {
        let item = serde_json::from_value::<ItemChunk>(json!({
            "type": "response.output_item.done",
            "item_id": "rs_canonical",
            "output_index": 0,
            "sequence_number": 1,
            "item": {
                "type": "reasoning",
                "id": "rs_canonical",
                "summary": [
                    {
                        "type": "summary_text",
                        "text": "Checked the available evidence."
                    }
                ],
                "content": [
                    {
                        "type": "reasoning_text",
                        "text": "Canonical final reasoning."
                    }
                ],
                "encrypted_content": "encrypted-reasoning",
                "status": "completed"
            }
        }))
        .unwrap();

        let choices = OpenAiWebSocketItemDecoder::default().decode(item);

        assert!(matches!(
            choices.first(),
            Some(RawStreamingChoice::Reasoning {
                id: Some(id),
                content: ReasoningContent::Summary(summary),
            }) if id == "rs_canonical" && summary == "Checked the available evidence."
        ));
        assert!(matches!(
            choices.get(1),
            Some(RawStreamingChoice::Reasoning {
                id: Some(id),
                content: ReasoningContent::Text {
                    text,
                    signature: None,
                },
            }) if id == "rs_canonical" && text == "Canonical final reasoning."
        ));
        assert!(matches!(
            choices.get(2),
            Some(RawStreamingChoice::Reasoning {
                id: Some(id),
                content: ReasoningContent::Encrypted(content),
            }) if id == "rs_canonical" && content == "encrypted-reasoning"
        ));
        assert_eq!(choices.len(), 3);
    }

    #[tokio::test]
    async fn evicting_unusable_session_replaces_the_pooled_slot() {
        let pool = OpenAiResponsesSessionPool::new();
        let key = OpenAiSessionKey::new(
            "conversation".to_string(),
            "provider".to_string(),
            "model".to_string(),
            "https://api.openai.com/v1",
            "test-key",
        );
        let first_slot = pool.conversation_slot(&key.conversation_id).await;
        let mut guard = first_slot.clone().lock_owned().await;
        guard.key = Some(key.clone());
        guard.opened_at = Some(Instant::now());

        pool.evict_unusable_session(&key, &mut guard).await;

        assert!(guard.key.is_none());
        assert!(guard.session.is_none());
        assert!(guard.opened_at.is_none());
        drop(guard);
        let replacement_slot = pool.conversation_slot(&key.conversation_id).await;
        assert!(!Arc::ptr_eq(&first_slot, &replacement_slot));
    }

    #[tokio::test]
    async fn busy_conversation_does_not_block_another_conversation() {
        let pool = OpenAiResponsesSessionPool::new();
        let first_conversation = "conversation-a".to_string();
        let second_conversation = "conversation-b".to_string();
        let _first_guard = pool.lock_conversation(&first_conversation).await;

        let second_guard = tokio::time::timeout(
            Duration::from_millis(100),
            pool.lock_conversation(&second_conversation),
        )
        .await
        .expect("an active conversation must not block another conversation");

        assert!(second_guard.session.is_none());
    }

    #[test]
    fn continuation_request_shaping_preserves_current_fields_and_full_fallback() {
        let reasoning = OpenAiReasoningPolicy {
            effort: Some(ReasoningEffort::High),
            mode: None,
            context: None,
            store: true,
        };
        let request = CompletionRequest {
            model: None,
            preamble: None,
            chat_history: OneOrMany::many(vec![
                Message::system("system"),
                Message::user("old user"),
                Message::assistant("old assistant"),
                Message::user("latest user"),
            ])
            .unwrap(),
            documents: vec![Document {
                id: "document".to_string(),
                text: "content".to_string(),
                additional_props: HashMap::new(),
            }],
            tools: vec![ToolDefinition {
                name: "local_tool".to_string(),
                description: "tool".to_string(),
                parameters: json!({"type": "object"}),
            }],
            temperature: Some(0.2),
            max_tokens: Some(1024),
            tool_choice: None,
            additional_params: Some(json!({
                "tools": [{"type": "web_search"}],
                "parallel_tool_calls": true
            })),
            output_schema: None,
            record_telemetry_content: false,
        };
        let state = OpenAiRunContinuationState {
            source_step_id: Some("step-1".to_string()),
            source_response_id: Some("resp-1".to_string()),
            documents_sent_in_run: false,
            full_history_fallback_used: false,
        };

        let incremental =
            prepare_openai_request(&reasoning, &state, request.clone(), None, false).unwrap();
        assert_eq!(
            incremental.context,
            ProviderRequestContextSnapshot::PreviousResponse
        );
        assert_eq!(incremental.previous_response_id.as_deref(), Some("resp-1"));
        assert_eq!(incremental.request.chat_history.len(), 2);
        assert_eq!(incremental.request.documents.len(), 1);
        assert_eq!(incremental.request.tools.len(), 1);
        let params = incremental.request.additional_params.unwrap();
        assert_eq!(params["previous_response_id"], "resp-1");
        assert_eq!(params["reasoning"]["context"], "all_turns");
        assert_eq!(params["tools"][0]["type"], "web_search");
        assert_eq!(params["parallel_tool_calls"], true);

        let mut sent_state = state.clone();
        sent_state.documents_sent_in_run = true;
        let cached = prepare_openai_request(
            &reasoning,
            &sent_state,
            request.clone(),
            Some("resp-1"),
            false,
        )
        .unwrap();
        assert!(cached.request.documents.is_empty());
        assert!(cached.request.additional_params.unwrap()["previous_response_id"].is_null());

        let fallback =
            prepare_openai_request(&reasoning, &sent_state, request, Some("resp-1"), true).unwrap();
        assert_eq!(
            fallback.context,
            ProviderRequestContextSnapshot::FullHistoryFallback
        );
        assert!(fallback.previous_response_id.is_none());
        assert_eq!(fallback.request.chat_history.len(), 4);
        assert_eq!(fallback.request.documents.len(), 1);
        assert_eq!(
            fallback.request.additional_params.unwrap()["reasoning"]["context"],
            "current_turn"
        );
    }
}
