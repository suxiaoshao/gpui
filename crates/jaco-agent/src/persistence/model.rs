use super::{PersistenceContext, completion_request_error, run_error};
use crate::AgentRuntimeError;
use rig::{
    completion::{CompletionModel, CompletionRequest, CompletionResponse},
    streaming::StreamingCompletionResponse,
};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct PersistingCompletionModel<M>
where
    M: CompletionModel,
{
    inner: M,
    context: Option<PersistenceContext>,
    openai_attempts: Option<crate::providers::openai::OpenAiAttemptCoordinator>,
}

impl<M> PersistingCompletionModel<M>
where
    M: CompletionModel,
{
    pub(crate) fn new(inner: M, context: PersistenceContext) -> Self {
        Self {
            inner,
            context: Some(context),
            openai_attempts: None,
        }
    }

    pub(crate) fn new_with_openai_attempts(
        inner: M,
        context: PersistenceContext,
        attempts: crate::providers::openai::OpenAiAttemptCoordinator,
    ) -> Self {
        Self {
            inner,
            context: Some(context),
            openai_attempts: Some(attempts),
        }
    }
}

impl<M> CompletionModel for PersistingCompletionModel<M>
where
    M: CompletionModel,
    M::Response: Serialize + DeserializeOwned,
    M::StreamingResponse:
        Clone + Unpin + Send + Sync + Serialize + DeserializeOwned + rig::completion::GetTokenUsage,
{
    type Response = M::Response;
    type StreamingResponse = M::StreamingResponse;
    type Client = M::Client;

    fn make(client: &Self::Client, model: impl Into<String>) -> Self {
        Self {
            inner: M::make(client, model),
            context: None,
            openai_attempts: None,
        }
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, rig::completion::CompletionError>
    {
        let Some(context) = self.context.clone() else {
            return self.inner.completion(request).await;
        };
        if let Some(attempts) = self.openai_attempts.as_ref() {
            attempts.bind(context.clone()).await;
            let response = tokio::select! {
                biased;
                _ = context.cancellation_token.cancelled() => {
                    let payload = run_error("canceled", "runtime canceled", false, None);
                    context
                        .cancel_current_provider_step(payload)
                        .await
                        .map_err(completion_request_error)?;
                    return Err(completion_request_error(AgentRuntimeError::Canceled));
                }
                response = self.inner.completion(request) => response,
            };
            if let Err(error) = &response {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_current_provider_step(payload).await;
            }
            return response;
        }
        let provider_step = context
            .insert_provider_step(&request)
            .await
            .map_err(completion_request_error)?;

        let response = tokio::select! {
            biased;
            _ = context.cancellation_token.cancelled() => {
                let payload = run_error("canceled", "runtime canceled", false, None);
                context
                    .cancel_provider_step(&provider_step.id, payload)
                    .await
                    .map_err(completion_request_error)?;
                return Err(completion_request_error(AgentRuntimeError::Canceled));
            }
            response = self.inner.completion(request) => response,
        };
        if context.cancellation_token.is_cancelled() {
            let payload = run_error("canceled", "runtime canceled", false, None);
            context
                .cancel_provider_step(&provider_step.id, payload)
                .await
                .map_err(completion_request_error)?;
            return Err(completion_request_error(AgentRuntimeError::Canceled));
        }

        match response {
            Ok(response) => {
                context
                    .finish_provider_step(&provider_step.id, &response)
                    .await
                    .map_err(completion_request_error)?;
                Ok(response)
            }
            Err(error) => {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_provider_step(&provider_step.id, payload).await;
                Err(error)
            }
        }
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<
        StreamingCompletionResponse<Self::StreamingResponse>,
        rig::completion::CompletionError,
    > {
        let Some(context) = self.context.clone() else {
            return self.inner.stream(request).await;
        };
        if let Some(attempts) = self.openai_attempts.as_ref() {
            attempts.bind(context.clone()).await;
            return tokio::select! {
                biased;
                _ = context.cancellation_token.cancelled() => {
                    let payload = run_error("canceled", "runtime canceled", false, None);
                    context
                        .cancel_current_provider_step(payload)
                        .await
                        .map_err(completion_request_error)?;
                    Err(completion_request_error(AgentRuntimeError::Canceled))
                }
                response = self.inner.stream(request) => {
                    if let Err(error) = &response {
                        let payload = run_error("provider_error", error.to_string(), true, None);
                        let _ = context.fail_current_provider_step(payload).await;
                    }
                    response
                },
            };
        }
        let provider_step = context
            .insert_provider_step(&request)
            .await
            .map_err(completion_request_error)?;
        let response = tokio::select! {
            biased;
            _ = context.cancellation_token.cancelled() => {
                let payload = run_error("canceled", "runtime canceled", false, None);
                context
                    .cancel_provider_step(&provider_step.id, payload)
                    .await
                    .map_err(completion_request_error)?;
                return Err(completion_request_error(AgentRuntimeError::Canceled));
            }
            response = self.inner.stream(request) => response,
        };
        if context.cancellation_token.is_cancelled() {
            let payload = run_error("canceled", "runtime canceled", false, None);
            context
                .cancel_provider_step(&provider_step.id, payload)
                .await
                .map_err(completion_request_error)?;
            return Err(completion_request_error(AgentRuntimeError::Canceled));
        }
        match response {
            Ok(response) => Ok(response),
            Err(error) => {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_provider_step(&provider_step.id, payload).await;
                Err(error)
            }
        }
    }
}
