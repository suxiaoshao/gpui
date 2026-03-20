use super::{Provider, provider_by_name};
use crate::{
    errors::{AiChatError, AiChatResult},
    llm::FetchUpdate,
    state::AiChatConfig,
};
use futures::pin_mut;

pub trait FetchRunner {
    fn get_provider(&self) -> &str;
    fn get_config(&self) -> &AiChatConfig;
    fn request_body(&self) -> &serde_json::Value;
    fn provider(&self) -> AiChatResult<&'static dyn Provider> {
        provider_by_name(self.get_provider())
    }
    fn fetch(&self) -> impl futures::Stream<Item = AiChatResult<FetchUpdate>> {
        async_stream::try_stream! {
            let provider = self.provider()?;
            let config = self.get_config().clone();
            let settings = config
                .get_provider_settings(provider.name())
                .ok_or(AiChatError::ProviderSettingsNotFound(provider.name().to_string()))?;
            let settings = settings.clone();
            let stream = provider.fetch_by_request_body(config, settings, self.request_body());
            pin_mut!(stream);
            for await item in stream {
                yield item?;
            }
        }
    }
}
