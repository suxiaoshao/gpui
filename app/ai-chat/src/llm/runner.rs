use super::{Provider, provider_by_name};
use crate::{
    errors::{AiChatError, AiChatResult},
    llm::{ProviderRunEvent, ProviderRunRequest},
    state::AiChatConfig,
};
use futures::pin_mut;

pub(crate) trait ProviderRunRunner {
    fn get_provider(&self) -> &str;
    fn get_config(&self) -> &AiChatConfig;
    fn run_request(&self) -> &ProviderRunRequest;
    fn request_body(&self) -> &serde_json::Value {
        &self.run_request().request_body
    }
    fn provider(&self) -> AiChatResult<&'static dyn Provider> {
        provider_by_name(self.get_provider())
    }
    fn run(&self) -> impl futures::Stream<Item = AiChatResult<ProviderRunEvent>> {
        async_stream::try_stream! {
            let provider = self.provider()?;
            let config = self.get_config().clone();
            let settings = config
                .get_provider_settings(provider.name())
                .ok_or(AiChatError::ProviderSettingsNotFound(provider.name().to_string()))?;
            let settings = settings.clone();
            let stream = provider.run(config, settings, self.run_request());
            pin_mut!(stream);
            for await item in stream {
                yield item?;
            }
        }
    }
}
