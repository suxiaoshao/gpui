use super::Message;
use crate::{
    database::Content,
    errors::{AiChatError, AiChatResult},
    config::AiChatConfig,
};
use futures::{future::BoxFuture, stream::BoxStream};
use gpui_component::setting::SettingGroup;

mod openai;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "tag", content = "value", rename_all = "camelCase")]
pub(crate) enum InputType {
    Text {
        max_length: Option<usize>,
        min_length: Option<usize>,
    },
    Float {
        max: Option<f64>,
        min: Option<f64>,
        step: Option<f64>,
        default: Option<f64>,
    },
    Boolean {
        default: Option<bool>,
    },
    Integer {
        max: Option<i64>,
        min: Option<i64>,
        step: Option<i64>,
        default: Option<i64>,
    },
    Select(Vec<String>),
    Array {
        #[serde(rename = "inputType")]
        input_type: Box<InputType>,
        name: &'static str,
        description: &'static str,
    },
    ArrayObject(Vec<InputItem>),
    Object(Vec<InputItem>),
    Optional(Box<InputType>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct InputItem {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    #[serde(rename = "inputType")]
    input_type: InputType,
}

impl InputItem {
    pub(crate) fn new(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        input_type: InputType,
    ) -> Self {
        Self {
            id,
            name,
            description,
            input_type,
        }
    }

    pub(crate) fn id(&self) -> &'static str {
        self.id
    }

    pub(crate) fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn description(&self) -> &'static str {
        self.description
    }

    pub(crate) fn input_type(&self) -> &InputType {
        &self.input_type
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ChatFormLayout {
    pub(crate) inline_field_ids: Vec<&'static str>,
    pub(crate) popover_groups: Vec<ChatFormGroup>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChatFormGroup {
    pub(crate) title_key: Option<&'static str>,
    pub(crate) description_key: Option<&'static str>,
    pub(crate) field_ids: Vec<&'static str>,
}

impl ChatFormGroup {
    pub(crate) fn new(
        title_key: Option<&'static str>,
        description_key: Option<&'static str>,
        field_ids: Vec<&'static str>,
    ) -> Self {
        Self {
            title_key,
            description_key,
            field_ids,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderModelCapability {
    Streaming,
    NonStreaming,
}

impl ProviderModelCapability {
    pub(crate) fn stream_flag(self) -> bool {
        matches!(self, Self::Streaming)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderModel {
    pub(crate) provider_name: String,
    pub(crate) id: String,
    pub(crate) capability: ProviderModelCapability,
}

impl ProviderModel {
    pub(crate) fn new(
        provider_name: impl Into<String>,
        id: impl Into<String>,
        capability: ProviderModelCapability,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            id: id.into(),
            capability,
        }
    }
}

pub(crate) trait Provider: Sync {
    fn name(&self) -> &'static str;
    fn is_configured(&self, settings: &serde_json::Value) -> bool;
    fn default_template_for_model(&self, model: &ProviderModel) -> AiChatResult<serde_json::Value>;
    fn get_template_inputs(&self) -> Vec<InputItem>;
    fn request_body(
        &self,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> AiChatResult<serde_json::Value>;
    fn fetch_by_request_body<'a>(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
        request_body: &'a serde_json::Value,
    ) -> BoxStream<'a, AiChatResult<FetchUpdate>>;
    fn list_models(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
    ) -> BoxFuture<'static, AiChatResult<Vec<ProviderModel>>>;
    fn chat_form_layout(&self) -> ChatFormLayout {
        ChatFormLayout {
            inline_field_ids: Vec::new(),
            popover_groups: Vec::new(),
        }
    }
    fn setting_group(&self) -> SettingGroup;
}

pub(crate) use openai::{OpenAIProvider, OpenAISettings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchUpdate {
    TextDelta(String),
    Complete(Content),
}

const PROVIDERS: [&dyn Provider; 1] = [&OpenAIProvider];

pub(crate) fn provider_names() -> Vec<&'static str> {
    PROVIDERS.iter().map(|provider| provider.name()).collect()
}

pub(crate) fn provider_setting_groups() -> Vec<SettingGroup> {
    PROVIDERS
        .iter()
        .map(|provider| provider.setting_group())
        .collect()
}

pub(crate) fn provider_by_name(name: &str) -> AiChatResult<&'static dyn Provider> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.name() == name)
        .ok_or_else(|| AiChatError::ProviderNotFound(name.to_string()))
}

pub(crate) fn template_inputs_by_provider(provider: &str) -> AiChatResult<Vec<InputItem>> {
    Ok(provider_by_name(provider)?.get_template_inputs())
}

pub(crate) fn chat_form_layout_by_provider(provider: &str) -> AiChatResult<ChatFormLayout> {
    Ok(provider_by_name(provider)?.chat_form_layout())
}

fn provider_settings_json(
    config: &AiChatConfig,
    provider: &'static dyn Provider,
) -> Option<(toml::Value, serde_json::Value)> {
    let settings = config.get_provider_settings(provider.name())?.clone();
    let settings_json = serde_json::to_value(&settings).ok()?;
    Some((settings, settings_json))
}

pub(crate) fn provider_is_configured(
    config: &AiChatConfig,
    provider_name: &str,
) -> AiChatResult<bool> {
    let provider = provider_by_name(provider_name)?;
    Ok(provider_settings_json(config, provider)
        .is_some_and(|(_, settings)| provider.is_configured(&settings)))
}

pub(crate) async fn available_models(config: AiChatConfig) -> AiChatResult<Vec<ProviderModel>> {
    let mut models = Vec::new();
    for provider in PROVIDERS {
        let Some((settings, settings_json)) = provider_settings_json(&config, provider) else {
            continue;
        };
        if !provider.is_configured(&settings_json) {
            continue;
        }
        models.extend(provider.list_models(config.clone(), settings).await?);
    }
    models.sort_by(|left, right| {
        left.provider_name
            .cmp(&right.provider_name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(models)
}
