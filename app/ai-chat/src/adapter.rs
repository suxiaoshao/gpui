use crate::{
    config::AiChatConfig,
    database::{ConversationTemplate, ConversationTemplatePrompt, Role},
    errors::{AiChatError, AiChatResult},
    fetch::Message,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, h_flex, label::Label, scroll::ScrollableElement, tag::Tag, v_flex,
};

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
    fn render_template_detail(&self, template: &ConversationTemplate, cx: &App) -> AnyElement {
        render_template_detail_default(template, cx)
    }
}

use gpui_component::setting::SettingGroup;
pub(crate) use openai::{OpenAIAdapter, OpenAIConversationTemplate, OpenAISettings};
pub(crate) use openai_stream::{OpenAIStreamAdapter, OpenAIStreamSettings};

pub(crate) fn render_template_detail_by_adapter(
    template: &ConversationTemplate,
    cx: &App,
) -> AiChatResult<AnyElement> {
    match template.adapter.as_str() {
        OpenAIAdapter::NAME => Ok(OpenAIAdapter.render_template_detail(template, cx)),
        OpenAIStreamAdapter::NAME => Ok(OpenAIStreamAdapter.render_template_detail(template, cx)),
        _ => Err(AiChatError::AdapterNotFound(template.adapter.clone())),
    }
}

pub(crate) fn render_template_detail_default(template: &ConversationTemplate, cx: &App) -> AnyElement {
    v_flex()
        .size_full()
        .gap_3()
        .p_4()
        .overflow_y_scrollbar()
        .child(Label::new("Base Information").text_lg())
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Label::new("Name").text_sm())
                .child(Label::new(&template.name).text_sm()),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Label::new("Icon").text_sm())
                .child(Label::new(&template.icon).text_sm()),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Label::new("Mode").text_sm())
                .child(
                    match template.mode {
                        crate::database::Mode::Contextual => Tag::primary(),
                        crate::database::Mode::Single => Tag::info(),
                        crate::database::Mode::AssistantOnly => Tag::success(),
                    }
                    .outline()
                    .child(template.mode.to_string()),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Label::new("Adapter").text_sm())
                .child(Label::new(&template.adapter).text_sm()),
        )
        .map(|this| match template.description.as_ref() {
            Some(description) => this.child(
                v_flex()
                    .gap_1()
                    .child(Label::new("Description").text_sm())
                    .child(Label::new(description).text_sm()),
            ),
            None => this,
        })
        .child(Label::new("Prompts").text_lg())
        .children(
            template
                .prompts
                .iter()
                .map(render_prompt)
                .collect::<Vec<AnyElement>>(),
        )
        .child(Label::new("Template JSON").text_lg())
        .child(
            div()
                .w_full()
                .p_3()
                .rounded_md()
                .bg(cx.theme().secondary)
                .child(
                    Label::new(serde_json::to_string_pretty(&template.template).unwrap_or_default())
                        .text_xs(),
                ),
        )
        .into_any_element()
}

fn render_prompt(prompt: &ConversationTemplatePrompt) -> AnyElement {
    let role = match prompt.role {
        Role::User => "User",
        Role::Assistant => "Assistant",
        Role::Developer => "Developer",
    };
    v_flex()
        .w_full()
        .gap_1()
        .p_3()
        .rounded_md()
        .border_1()
        .child(Label::new(role).text_sm())
        .child(Label::new(&prompt.prompt).text_sm())
        .into_any_element()
}
