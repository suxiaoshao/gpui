use super::{Adapter, Message, adapter_by_name};
use crate::{
    config::AiChatConfig,
    database::{Content, Role},
    errors::{AiChatError, AiChatResult},
    extensions::ExtensionRunner,
};
use futures::pin_mut;

pub trait FetchRunner {
    fn get_adapter(&self) -> &str;
    fn get_template(&self) -> &serde_json::Value;
    fn get_config(&self) -> &AiChatConfig;
    fn get_history(&self) -> Vec<Message>;
    fn adapter(&self) -> AiChatResult<&'static dyn Adapter> {
        adapter_by_name(self.get_adapter())
    }
    fn request_body(&self) -> AiChatResult<serde_json::Value> {
        self.adapter()?
            .request_body(self.get_template(), self.get_history())
    }
    fn request_body_with_message(
        &self,
        role: Role,
        send_content: &str,
    ) -> AiChatResult<serde_json::Value> {
        let mut history = self.get_history();
        history.push(Message::new(role, send_content.to_string()));
        self.adapter()?.request_body(self.get_template(), history)
    }
    async fn get_new_user_content(
        send_content: String,
        extension: Option<ExtensionRunner>,
    ) -> AiChatResult<Content> {
        if let Some(ExtensionRunner {
            extension,
            mut store,
            config,
        }) = extension
        {
            let chat_request = crate::extensions::ChatRequest {
                message: send_content.clone(),
            };
            let extension_api = extension.chatgpt_extension_extension_api();
            let data = extension_api
                .call_on_request(&mut store, &chat_request)
                .await
                .map_err(|_| AiChatError::ExtensionRuntimeError)?
                .map_err(|_| AiChatError::ExtensionRuntimeError)?;
            return Ok(Content::Extension {
                source: send_content,
                extension_name: config.name,
                content: data.message,
            });
        }
        Ok(Content::Text(send_content))
    }
    fn fetch(&self) -> impl futures::Stream<Item = AiChatResult<String>> {
        async_stream::try_stream! {
            let adapter = self.adapter()?;
            let config = self.get_config().clone();
            let settings = config
                .get_adapter_settings(adapter.name())
                .ok_or(AiChatError::AdapterSettingsNotFound(adapter.name().to_string()))?;
            let settings = settings.clone();
            let template = self.get_template().clone();
            let history = self.get_history();
            let stream = adapter.fetch(config, settings, template, history);
            pin_mut!(stream);
            for await item in stream {
                yield item?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FetchRunner;
    use crate::database::Content;
    use crate::extensions::ExtensionRunner;
    use futures::executor::block_on;
    use std::sync::OnceLock;

    struct DummyRunner;

    impl FetchRunner for DummyRunner {
        fn get_adapter(&self) -> &str {
            "noop"
        }

        fn get_template(&self) -> &serde_json::Value {
            static TEMPLATE: OnceLock<serde_json::Value> = OnceLock::new();
            TEMPLATE.get_or_init(|| serde_json::Value::Null)
        }

        fn get_config(&self) -> &crate::config::AiChatConfig {
            static CONFIG: OnceLock<crate::config::AiChatConfig> = OnceLock::new();
            CONFIG.get_or_init(crate::config::AiChatConfig::default)
        }

        fn get_history(&self) -> Vec<crate::llm::Message> {
            Vec::new()
        }
    }

    #[test]
    fn get_new_user_content_without_extension_returns_text() {
        let content = block_on(DummyRunner::get_new_user_content(
            "hello".to_string(),
            None::<ExtensionRunner>,
        ))
        .expect("content");
        assert_eq!(content, Content::Text("hello".to_string()));
    }
}
