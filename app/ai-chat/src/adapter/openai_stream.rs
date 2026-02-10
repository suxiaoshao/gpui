use super::{
    Adapter, InputItem, OpenAIAdapter, description_items_default,
    openai::{OpenAIConversationTemplate, get_openai_template_inputs},
};
use crate::{
    config::AiChatConfig,
    database::ConversationTemplate,
    errors::{AiChatError, AiChatResult},
    fetch::{ChatRequest, Message, OpenAIStreamResponse},
};
use futures::StreamExt;
use gpui::*;
use gpui_component::description_list::DescriptionItem;
use gpui_component::setting::{SettingField, SettingGroup, SettingItem};
use reqwest::Client;
use reqwest_eventsource::{Event, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use toml::Value;
use tracing::{Level, event};

fn default_url() -> String {
    "https://api.openai.com/v1/chat/completions".to_string()
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

impl OpenAIStreamAdapter {
    fn get_body(
        template: &'_ OpenAIConversationTemplate,
        history_messages: Vec<Message>,
    ) -> ChatRequest<'_> {
        ChatRequest {
            messages: history_messages,
            model: template.model.as_str(),
            stream: true,
            temperature: template.temperature,
            top_p: template.top_p,
            n: template.n,
            max_completion_tokens: template.max_completion_tokens,
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
    const NAME: &'static str = "OpenAI Stream";

    fn get_setting_inputs(&self) -> Vec<InputItem> {
        OpenAIAdapter.get_setting_inputs()
    }

    fn get_template_inputs(&self, settings: &serde_json::Value) -> AiChatResult<Vec<InputItem>> {
        let settings: OpenAIStreamSettings = serde_json::from_value(settings.clone())?;
        get_openai_template_inputs(&settings.models)
    }

    fn fetch(
        &self,
        config: &AiChatConfig,
        settings: &toml::Value,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> impl futures::Stream<Item = AiChatResult<String>> {
        async_stream::try_stream! {
            let template = serde_json::from_value(template.clone())?;
            let settings = settings.clone().try_into()?;
            let body = Self::get_body(&template, history_messages);
            let client = Self::get_reqwest_client(config, &settings)?;
            let mut es = client.post(settings.url.as_str()).json(&body).eventsource()?;
            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {},
                    Ok(Event::Message(message)) => {
                        let message = message.data;
                        if message == "[DONE]" {
                            es.close();
                        } else {
                            let response= serde_json::from_str::<OpenAIStreamResponse>(&message)?;
                            let content = response
                                .choices
                                .into_iter()
                                .filter_map(|choice| choice.delta.content)
                                .collect::<String>();
                            yield content
                        }
                    }
                    Err(_err) => {
                        es.close();
                    }
                }
            }
        }
    }

    fn setting_group(&self) -> gpui_component::setting::SettingGroup {
        fn get_openai_setting(cx: &App) -> OpenAIStreamSettings {
            let config = cx.global::<AiChatConfig>();
            config
                .get_adapter_settings(OpenAIStreamAdapter::NAME)
                .and_then(|x| x.clone().try_into::<OpenAIStreamSettings>().ok())
                .unwrap_or_default()
        }
        SettingGroup::new()
            .title("OpenAI Stream")
            .item(SettingItem::new(
                "Api Key",
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
                                config.set_adapter_settings(OpenAIStreamAdapter::NAME, settings)
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
                "Api Url",
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
                                config.set_adapter_settings(OpenAIStreamAdapter::NAME, settings)
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
                "Http Proxy",
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
                                config.set_adapter_settings(OpenAIStreamAdapter::NAME, settings)
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
            DescriptionItem::new("Model").value(settings.model),
            DescriptionItem::new("Temperature").value(settings.temperature.to_string()),
            DescriptionItem::new("Top P").value(settings.top_p.to_string()),
            DescriptionItem::new("N").value(settings.n.to_string()),
            DescriptionItem::new("Max Completion Tokens").value(
                settings
                    .max_completion_tokens
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            DescriptionItem::new("Presence Penalty").value(settings.presence_penalty.to_string()),
            DescriptionItem::new("Frequency Penalty").value(settings.frequency_penalty.to_string()),
        ]
    }
}
