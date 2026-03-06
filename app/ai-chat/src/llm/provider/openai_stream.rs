use super::{
    Adapter, InputItem, OpenAIAdapter, description_items_default,
    openai::{OpenAIConversationTemplate, get_openai_template_inputs},
};
use crate::llm::{ChatRequest, Message, OpenAIResponseStreamEvent};
use crate::{
    config::AiChatConfig,
    database::ConversationTemplate,
    errors::{AiChatError, AiChatResult},
    i18n::t_static,
};
use futures::{StreamExt, stream::BoxStream};
use gpui::*;
use gpui_component::description_list::DescriptionItem;
use gpui_component::setting::{SettingField, SettingGroup, SettingItem};
use reqwest::Client;
use reqwest_eventsource::{Error as EventSourceError, Event, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use toml::Value;
use tracing::{Level, event};

fn default_url() -> String {
    "https://api.openai.com/v1/responses".to_string()
}

fn default_models() -> HashSet<String> {
    let mut models = HashSet::new();
    models.insert("gpt-4o".into());
    models.insert("gpt-4o-mini".into());
    models.insert("gpt-4.1".into());
    models.insert("gpt-4.1-mini".into());
    models.insert("gpt-4.1-nano".into());
    models.insert("gpt-5".into());
    models.insert("gpt-5-mini".into());
    models.insert("gpt-5-nano".into());
    models.insert("gpt-5.2".into());
    models.insert("gpt-5.2-pro".into());
    models
}

#[derive(Deserialize, Serialize)]
pub(crate) struct OpenAIStreamSettings {
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(rename = "httpProxy")]
    pub http_proxy: Option<String>,
    #[serde(default = "default_models")]
    pub models: HashSet<String>,
}

impl Default for OpenAIStreamSettings {
    fn default() -> Self {
        Self {
            api_key: Default::default(),
            url: Default::default(),
            http_proxy: Default::default(),
            models: default_models(),
        }
    }
}

pub(crate) struct OpenAIStreamAdapter;

fn is_normal_stream_end(error: &EventSourceError) -> bool {
    matches!(error, EventSourceError::StreamEnded)
}

impl OpenAIStreamAdapter {
    fn get_body(
        template: &'_ OpenAIConversationTemplate,
        history_messages: Vec<Message>,
    ) -> ChatRequest<'_> {
        ChatRequest {
            input: history_messages,
            model: template.model.as_str(),
            stream: true,
            temperature: template.temperature,
            top_p: template.top_p,
            max_output_tokens: template.max_completion_tokens,
            presence_penalty: template.presence_penalty,
            frequency_penalty: template.frequency_penalty,
        }
    }
    fn get_reqwest_client(
        config: &AiChatConfig,
        settings: &OpenAIStreamSettings,
    ) -> AiChatResult<Client> {
        let api_key = settings
            .api_key
            .as_deref()
            .ok_or(AiChatError::ApiKeyNotSet)?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append("Authorization", format!("Bearer {api_key}").parse()?);
        let mut client = reqwest::ClientBuilder::new().default_headers(headers);
        match settings.http_proxy.as_deref().or(config.get_http_proxy()) {
            None => {}
            Some(proxy) => {
                client = client.proxy(reqwest::Proxy::all(proxy)?);
            }
        }
        let client = client.build()?;
        Ok(client)
    }
}

impl Adapter for OpenAIStreamAdapter {
    fn name(&self) -> &'static str {
        "OpenAI Stream"
    }

    fn get_setting_inputs(&self) -> Vec<InputItem> {
        OpenAIAdapter.get_setting_inputs()
    }

    fn get_template_inputs(&self, settings: serde_json::Value) -> AiChatResult<Vec<InputItem>> {
        let settings: OpenAIStreamSettings = serde_json::from_value(settings)?;
        get_openai_template_inputs(&settings.models)
    }

    fn request_body(
        &self,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> AiChatResult<serde_json::Value> {
        let template = OpenAIConversationTemplate::deserialize(template)?;
        Ok(serde_json::to_value(Self::get_body(
            &template,
            history_messages,
        ))?)
    }

    fn fetch_by_request_body<'a>(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
        request_body: &'a serde_json::Value,
    ) -> BoxStream<'a, AiChatResult<String>> {
        async_stream::try_stream! {
            let settings = settings.try_into()?;
            let client = Self::get_reqwest_client(&config, &settings)?;
            let mut es = client
                .post(settings.url.as_str())
                .json(&request_body)
                .eventsource()?;
            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {},
                    Ok(Event::Message(message)) => {
                        let message = message.data;
                        if message == "[DONE]" {
                            es.close();
                            break;
                        } else if let Some(content) = parse_response_stream_event(&message)? {
                            yield content;
                        }
                    }
                    Err(err) => {
                        es.close();
                        if is_normal_stream_end(&err) {
                            break;
                        }
                        Err::<(), AiChatError>(err.into())?;
                    }
                }
            }
        }
        .boxed()
    }

    fn setting_group(&self) -> gpui_component::setting::SettingGroup {
        fn get_openai_setting(cx: &App) -> OpenAIStreamSettings {
            let config = cx.global::<AiChatConfig>();
            config
                .get_adapter_settings(OpenAIStreamAdapter.name())
                .and_then(|x| x.clone().try_into::<OpenAIStreamSettings>().ok())
                .unwrap_or_default()
        }
        SettingGroup::new()
            .title(t_static("settings-openai-stream-title"))
            .item(SettingItem::new(
                t_static("field-api-key"),
                SettingField::input(
                    |cx| {
                        let openai_setting = get_openai_setting(cx);
                        openai_setting.api_key.map(|x| x.into()).unwrap_or_default()
                    },
                    |value, cx| {
                        let mut open_settings = get_openai_setting(cx);
                        open_settings.api_key = if value.is_empty() {
                            None
                        } else {
                            Some(value.into())
                        };
                        let config = cx.global_mut::<AiChatConfig>();
                        match Value::try_from(open_settings) {
                            Ok(settings) => {
                                config.set_adapter_settings(OpenAIStreamAdapter.name(), settings)
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Failed to convert OpenAI settings: {}", err);
                            }
                        }
                        if let Err(err) = config.save() {
                            event!(Level::ERROR, "Failed to save OpenAI settings: {}", err);
                        };
                    },
                ),
            ))
            .item(SettingItem::new(
                t_static("field-api-url"),
                SettingField::input(
                    |cx| {
                        let openai_setting = get_openai_setting(cx);
                        openai_setting.url.into()
                    },
                    |value, cx| {
                        let mut open_settings = get_openai_setting(cx);
                        open_settings.url = value.into();
                        let config = cx.global_mut::<AiChatConfig>();
                        match Value::try_from(open_settings) {
                            Ok(settings) => {
                                config.set_adapter_settings(OpenAIStreamAdapter.name(), settings)
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Failed to convert OpenAI settings: {}", err);
                            }
                        }
                        if let Err(err) = config.save() {
                            event!(Level::ERROR, "Failed to save OpenAI settings: {}", err);
                        };
                    },
                ),
            ))
            .item(SettingItem::new(
                t_static("field-http-proxy"),
                SettingField::input(
                    |cx| {
                        let openai_setting = get_openai_setting(cx);
                        openai_setting
                            .http_proxy
                            .map(|x| x.into())
                            .unwrap_or_default()
                    },
                    |value, cx| {
                        let mut open_settings = get_openai_setting(cx);
                        open_settings.http_proxy = if value.is_empty() {
                            None
                        } else {
                            Some(value.into())
                        };
                        let config = cx.global_mut::<AiChatConfig>();
                        match Value::try_from(open_settings) {
                            Ok(settings) => {
                                config.set_adapter_settings(OpenAIStreamAdapter.name(), settings)
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Failed to convert OpenAI settings: {}", err);
                            }
                        }
                        if let Err(err) = config.save() {
                            event!(Level::ERROR, "Failed to save OpenAI settings: {}", err);
                        };
                    },
                ),
            ))
    }

    fn description_items(&self, template: &ConversationTemplate) -> Vec<DescriptionItem> {
        let Ok(settings) =
            serde_json::from_value::<OpenAIConversationTemplate>(template.template.clone())
        else {
            return description_items_default(template);
        };

        vec![
            DescriptionItem::new(t_static("field-model")).value(settings.model),
            DescriptionItem::new(t_static("field-temperature"))
                .value(settings.temperature.to_string()),
            DescriptionItem::new(t_static("field-top-p")).value(settings.top_p.to_string()),
            DescriptionItem::new(t_static("field-n")).value(settings.n.to_string()),
            DescriptionItem::new(t_static("field-max-completion-tokens")).value(
                settings
                    .max_completion_tokens
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            DescriptionItem::new(t_static("field-presence-penalty"))
                .value(settings.presence_penalty.to_string()),
            DescriptionItem::new(t_static("field-frequency-penalty"))
                .value(settings.frequency_penalty.to_string()),
        ]
    }
}

fn parse_response_stream_event(message: &str) -> AiChatResult<Option<String>> {
    let event = serde_json::from_str::<OpenAIResponseStreamEvent>(message)?;
    match event {
        OpenAIResponseStreamEvent::ResponseOutputTextDelta { delta } => Ok(Some(delta)),
        OpenAIResponseStreamEvent::Error { message, .. } => Err(AiChatError::StreamError(message)),
        OpenAIResponseStreamEvent::ResponseFailed { response } => {
            Err(AiChatError::StreamError(response.error.message))
        }
        OpenAIResponseStreamEvent::Other => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_normal_stream_end, parse_response_stream_event};
    use crate::errors::AiChatError;
    use reqwest_eventsource::Error as EventSourceError;

    #[test]
    fn parse_stream_delta_event() -> anyhow::Result<()> {
        let event = r#"{"type":"response.output_text.delta","delta":"hello"}"#;
        assert_eq!(
            parse_response_stream_event(event)?,
            Some("hello".to_string())
        );
        Ok(())
    }

    #[test]
    fn ignore_non_delta_event() -> anyhow::Result<()> {
        let event = r#"{"type":"response.completed"}"#;
        assert_eq!(parse_response_stream_event(event)?, None);
        Ok(())
    }

    #[test]
    fn parse_error_stream_event() -> anyhow::Result<()> {
        let event = r#"{"type":"error","message":"quota exceeded"}"#;
        let error = parse_response_stream_event(event).unwrap_err();
        assert!(matches!(error, AiChatError::StreamError(ref msg) if msg == "quota exceeded"));
        Ok(())
    }

    #[test]
    fn parse_failed_response_event() -> anyhow::Result<()> {
        let event =
            r#"{"type":"response.failed","response":{"error":{"message":"request failed"}}}"#;
        let error = parse_response_stream_event(event).unwrap_err();
        assert!(matches!(error, AiChatError::StreamError(ref msg) if msg == "request failed"));
        Ok(())
    }

    #[test]
    fn stream_ended_is_treated_as_normal_exit() {
        assert!(is_normal_stream_end(&EventSourceError::StreamEnded));
    }
}
