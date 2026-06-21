use super::{PersistenceContext, completion_request_error, run_error};
use rig_core::{
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
}

impl<M> PersistingCompletionModel<M>
where
    M: CompletionModel,
{
    pub(crate) fn new(inner: M, context: PersistenceContext) -> Self {
        Self {
            inner,
            context: Some(context),
        }
    }
}

impl<M> CompletionModel for PersistingCompletionModel<M>
where
    M: CompletionModel,
    M::Response: Serialize + DeserializeOwned,
    M::StreamingResponse: Clone
        + Unpin
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + rig_core::completion::GetTokenUsage,
{
    type Response = M::Response;
    type StreamingResponse = M::StreamingResponse;
    type Client = M::Client;

    fn make(client: &Self::Client, model: impl Into<String>) -> Self {
        Self {
            inner: M::make(client, model),
            context: None,
        }
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<
        CompletionResponse<Self::Response>,
        rig_core::completion::CompletionError,
    > {
        let Some(context) = self.context.clone() else {
            return self.inner.completion(request).await;
        };
        let provider_step = context
            .insert_provider_step(&request)
            .map_err(completion_request_error)?;

        let response = self.inner.completion(request).await;
        match response {
            Ok(response) => {
                context
                    .finish_provider_step(&provider_step.id, &response)
                    .map_err(completion_request_error)?;
                Ok(response)
            }
            Err(error) => {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_provider_step(&provider_step.id, payload);
                Err(error)
            }
        }
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<
        StreamingCompletionResponse<Self::StreamingResponse>,
        rig_core::completion::CompletionError,
    > {
        let Some(context) = self.context.clone() else {
            return self.inner.stream(request).await;
        };
        let provider_step = context
            .insert_provider_step(&request)
            .map_err(completion_request_error)?;
        let response = self.inner.stream(request).await;
        match response {
            Ok(response) => Ok(response),
            Err(error) => {
                let payload = run_error("provider_error", error.to_string(), true, None);
                let _ = context.fail_provider_step(&provider_step.id, payload);
                Err(error)
            }
        }
    }
}
