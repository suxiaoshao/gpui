use crate::{
    config::AiChatConfig,
    database::ConversationTemplate,
    errors::{AiChatError, AiChatResult},
    fetch::Message,
};
use gpui_component::description_list::DescriptionItem;

mod openai;
mod openai_stream;

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
    fn new(
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

    pub(crate) fn input_type(&self) -> &InputType {
        &self.input_type
    }
}

pub trait Adapter {
    const NAME: &'static str;
    fn get_setting_inputs(&self) -> Vec<InputItem>;
    fn get_template_inputs(&self, settings: &serde_json::Value) -> AiChatResult<Vec<InputItem>>;
    fn fetch(
        &self,
        config: &AiChatConfig,
        settings: &toml::Value,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> impl futures::Stream<Item = AiChatResult<String>>;
    fn setting_group(&self) -> SettingGroup;
    fn description_items(&self, template: &ConversationTemplate) -> Vec<DescriptionItem> {
        description_items_default(template)
    }
}

use gpui_component::setting::SettingGroup;
pub(crate) use openai::{OpenAIAdapter, OpenAIConversationTemplate, OpenAISettings};
pub(crate) use openai_stream::{OpenAIStreamAdapter, OpenAIStreamSettings};

pub(crate) fn adapter_names() -> Vec<&'static str> {
    vec![OpenAIAdapter::NAME, OpenAIStreamAdapter::NAME]
}

pub(crate) fn template_inputs_by_adapter(
    adapter: &str,
    config: &AiChatConfig,
) -> AiChatResult<Vec<InputItem>> {
    let settings = config
        .get_adapter_settings(adapter)
        .ok_or_else(|| AiChatError::AdapterSettingsNotFound(adapter.to_string()))?
        .clone();
    let settings = serde_json::to_value(settings)?;
    match adapter {
        OpenAIAdapter::NAME => OpenAIAdapter.get_template_inputs(&settings),
        OpenAIStreamAdapter::NAME => OpenAIStreamAdapter.get_template_inputs(&settings),
        _ => Err(AiChatError::AdapterNotFound(adapter.to_string())),
    }
}

pub(crate) fn description_items_by_adapter(
    template: &ConversationTemplate,
) -> AiChatResult<Vec<DescriptionItem>> {
    match template.adapter.as_str() {
        OpenAIAdapter::NAME => Ok(OpenAIAdapter.description_items(template)),
        OpenAIStreamAdapter::NAME => Ok(OpenAIStreamAdapter.description_items(template)),
        _ => Err(AiChatError::AdapterNotFound(template.adapter.clone())),
    }
}

pub(crate) fn description_items_default(template: &ConversationTemplate) -> Vec<DescriptionItem> {
    match template.template.as_object() {
        Some(map) if !map.is_empty() => map
            .iter()
            .map(|(key, value)| {
                DescriptionItem::new(key.clone()).value(format_template_value(value))
            })
            .collect(),
        _ => vec![DescriptionItem::new("Raw").value(format_template_value(&template.template))],
    }
}

fn format_template_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(num) => num.to_string(),
        serde_json::Value::Bool(boolean) => boolean.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_default()
        }
    }
}
