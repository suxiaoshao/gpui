use super::{
    ChatFormGroup, ChatFormLayout, FetchUpdate, InputItem, InputType, Provider, ProviderModel,
    ProviderModelCapability,
};
use crate::{
    database::{Content, UrlCitation},
    errors::{AiChatError, AiChatResult},
    config::AiChatConfig,
    i18n::t_static,
    llm::{ChatRequest, HostedTool, Message, OpenAIResponseStreamEvent},
};
use async_compat::CompatExt;
use futures::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use gpui::*;
use gpui_component::{
    Sizable,
    input::{Input, InputEvent, InputState},
    setting::{SettingField, SettingGroup, SettingItem},
};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::digit1,
    combinator::{all_consuming, map, opt},
    multi::separated_list1,
};
use reqwest::Client;
use reqwest_eventsource::{Error as EventSourceError, Event, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use toml::Value;
use tracing::{Level, event};

pub(crate) struct OpenAIProvider;

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

pub(crate) fn normalize_base_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return default_base_url();
    }
    if let Some(base) = trimmed.strip_suffix("/responses") {
        return base.to_string();
    }
    if let Some(base) = trimmed.strip_suffix("/models") {
        return base.to_string();
    }
    trimmed
}

fn responses_url(base_url: &str) -> String {
    format!("{}/responses", normalize_base_url(base_url))
}

fn models_url(base_url: &str) -> String {
    format!("{}/models", normalize_base_url(base_url))
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct OpenAISettings {
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
    #[serde(rename = "baseUrl", alias = "url", default = "default_base_url")]
    pub base_url: String,
    #[serde(rename = "httpProxy")]
    pub http_proxy: Option<String>,
}

impl Default for OpenAISettings {
    fn default() -> Self {
        Self {
            api_key: Default::default(),
            base_url: default_base_url(),
            http_proxy: Default::default(),
        }
    }
}

impl OpenAISettings {
    pub(crate) fn normalized(mut self) -> Self {
        self.base_url = normalize_base_url(&self.base_url);
        self
    }
}

fn openai_settings(cx: &App) -> OpenAISettings {
    let config = cx.global::<AiChatConfig>();
    config
        .get_provider_settings(OpenAIProvider.name())
        .and_then(|x| x.clone().try_into::<OpenAISettings>().ok())
        .map(OpenAISettings::normalized)
        .unwrap_or_default()
}

fn save_openai_settings(settings: OpenAISettings, cx: &mut App) {
    let config = cx.global_mut::<AiChatConfig>();
    match Value::try_from(settings.normalized()) {
        Ok(settings) => config.set_provider_settings(OpenAIProvider.name(), settings),
        Err(err) => {
            event!(Level::ERROR, "Failed to convert OpenAI settings: {}", err);
            return;
        }
    }
    if let Err(err) = config.save() {
        event!(Level::ERROR, "Failed to save OpenAI settings: {}", err);
    }
}

struct BaseUrlFieldState {
    input: Entity<InputState>,
    last_value: String,
    _subscription: Subscription,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct OpenAIRequestTemplate {
    pub(crate) model: String,
    pub(crate) stream: bool,
    #[serde(default)]
    pub(crate) web_search: bool,
    pub(crate) temperature: f64,
    pub(crate) top_p: f64,
    pub(crate) n: u32,
    pub(crate) max_completion_tokens: Option<u32>,
    pub(crate) presence_penalty: f64,
    pub(crate) frequency_penalty: f64,
}

impl Default for OpenAIRequestTemplate {
    fn default() -> Self {
        Self {
            model: "gpt-4o".to_string(),
            stream: true,
            web_search: false,
            temperature: 1.0,
            top_p: 1.0,
            n: 1,
            max_completion_tokens: None,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ModelListResponse {
    data: Vec<ModelListItem>,
}

#[derive(Debug, Deserialize)]
struct ModelListItem {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ResponsesCreateResponse {
    output: Vec<OutputItem>,
}

#[derive(Debug, Deserialize)]
struct OutputItem {
    #[serde(rename = "type")]
    item_type: Option<String>,
    content: Option<Vec<OutputContent>>,
}

#[derive(Debug, Deserialize)]
struct OutputContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    #[serde(default)]
    annotations: Vec<OutputAnnotation>,
}

#[derive(Debug, Deserialize)]
struct OutputAnnotation {
    #[serde(rename = "type")]
    annotation_type: String,
    title: Option<String>,
    url: Option<String>,
    start_index: Option<usize>,
    end_index: Option<usize>,
}

impl OutputAnnotation {
    fn into_citation(self) -> Option<UrlCitation> {
        if self.annotation_type != "url_citation" {
            return None;
        }
        Some(UrlCitation {
            title: self.title,
            url: self.url?,
            start_index: self.start_index,
            end_index: self.end_index,
        })
    }
}

impl ResponsesCreateResponse {
    fn into_content(self) -> Content {
        let mut text = String::new();
        let mut citations = Vec::new();
        for item in self.output {
            if item.item_type.as_deref() == Some("web_search_call") {
                continue;
            }
            for part in item.content.unwrap_or_default() {
                if part.content_type != "output_text" {
                    continue;
                }
                if let Some(part_text) = part.text {
                    text.push_str(&part_text);
                }
                citations.extend(part.annotations.into_iter().filter_map(OutputAnnotation::into_citation));
            }
        }
        if citations.is_empty() {
            Content::Text(text)
        } else {
            Content::WebSearch { text, citations }
        }
    }
}

fn is_normal_stream_end(error: &EventSourceError) -> bool {
    matches!(error, EventSourceError::StreamEnded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenAIModelFamily {
    Gpt,
    ChatGpt,
    OSeries,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedOpenAIModel<'a> {
    family: OpenAIModelFamily,
    has_date_suffix: bool,
    segments: Vec<&'a str>,
}

fn is_model_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_')
}

fn parse_model_segments(input: &str) -> IResult<&str, Vec<&str>> {
    separated_list1(tag("-"), take_while1(is_model_char)).parse(input)
}

fn parse_oseries_segment(input: &str) -> IResult<&str, &str> {
    map((tag("o"), digit1), |_| input).parse(input)
}

fn detect_family(first: &str) -> Option<OpenAIModelFamily> {
    match all_consuming(alt((
        map(tag("gpt"), |_| OpenAIModelFamily::Gpt),
        map(tag("chatgpt"), |_| OpenAIModelFamily::ChatGpt),
        map(parse_oseries_segment, |_| OpenAIModelFamily::OSeries),
    )))
    .parse(first)
    {
        Ok((_, family)) => Some(family),
        Err(_) => None,
    }
}

fn parse_openai_model(input: &str) -> IResult<&str, ParsedOpenAIModel<'_>> {
    let (input, _) = opt(tag("ft:")).parse(input)?;
    let (input, segments) = parse_model_segments(input)?;
    let family = detect_family(segments.first().copied().unwrap_or_default()).ok_or_else(|| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag))
    })?;
    let has_date_suffix = matches!(
        segments.as_slice(),
        [.., year, month, day]
            if year.len() == 4
                && month.len() == 2
                && day.len() == 2
                && year.chars().all(|ch| ch.is_ascii_digit())
                && month.chars().all(|ch| ch.is_ascii_digit())
                && day.chars().all(|ch| ch.is_ascii_digit())
    );
    Ok((
        input,
        ParsedOpenAIModel {
            family,
            has_date_suffix,
            segments,
        },
    ))
}

fn classify_model(id: &str) -> Option<ProviderModelCapability> {
    if id.strip_prefix("ft:").unwrap_or(id).starts_with("fp") {
        return None;
    }
    let parsed = all_consuming(parse_openai_model).parse(id).ok()?.1;
    let has_short_numeric_suffix = parsed
        .segments
        .last()
        .is_some_and(|segment| segment.len() == 4 && segment.chars().all(|ch| ch.is_ascii_digit()));
    let has_preview_suffix = parsed
        .segments
        .last()
        .is_some_and(|segment| *segment == "preview");
    if parsed.has_date_suffix || has_short_numeric_suffix || has_preview_suffix {
        return None;
    }
    Some(match parsed.family {
        OpenAIModelFamily::OSeries => ProviderModelCapability::NonStreaming,
        OpenAIModelFamily::Gpt | OpenAIModelFamily::ChatGpt => ProviderModelCapability::Streaming,
    })
}

fn parse_response_stream_event(message: &str) -> AiChatResult<Option<FetchUpdate>> {
    let event = serde_json::from_str::<OpenAIResponseStreamEvent>(message)?;
    match event {
        OpenAIResponseStreamEvent::ResponseOutputTextDelta { delta } => {
            Ok(Some(FetchUpdate::TextDelta(delta)))
        }
        OpenAIResponseStreamEvent::ResponseCompleted { response } => {
            let response = serde_json::from_value::<ResponsesCreateResponse>(response)?;
            Ok(Some(FetchUpdate::Complete(response.into_content())))
        }
        OpenAIResponseStreamEvent::Error { message, .. } => Err(AiChatError::StreamError(message)),
        OpenAIResponseStreamEvent::ResponseFailed { response } => {
            Err(AiChatError::StreamError(response.error.message))
        }
        OpenAIResponseStreamEvent::Other => Ok(None),
    }
}

fn request_inputs() -> Vec<InputItem> {
    vec![
        InputItem::new(
            "web_search",
            "Web Search",
            "Enable OpenAI web search for supported models only.",
            InputType::Boolean {
                default: Some(false),
            },
        ),
        InputItem::new(
            "temperature",
            "Temperature",
            "Temperature",
            InputType::Float {
                min: Some(0.0),
                max: Some(2.0),
                step: Some(0.1),
                default: Some(1.0),
            },
        ),
        InputItem::new(
            "top_p",
            "Top P",
            "Top P",
            InputType::Float {
                min: Some(0.0),
                max: Some(1.0),
                step: Some(0.1),
                default: Some(1.0),
            },
        ),
        InputItem::new(
            "n",
            "N",
            "N",
            InputType::Integer {
                max: None,
                min: Some(1),
                step: Some(1),
                default: Some(1),
            },
        ),
        InputItem::new(
            "max_completion_tokens",
            "Max Completion Tokens",
            "Max Completion Tokens",
            InputType::Optional(Box::new(InputType::Integer {
                max: None,
                min: Some(1),
                step: Some(1),
                default: None,
            })),
        ),
        InputItem::new(
            "presence_penalty",
            "Presence Penalty",
            "Presence Penalty",
            InputType::Float {
                max: Some(2.0),
                min: Some(-2.0),
                step: Some(0.1),
                default: Some(0.0),
            },
        ),
        InputItem::new(
            "frequency_penalty",
            "Frequency Penalty",
            "Frequency Penalty",
            InputType::Float {
                max: Some(2.0),
                min: Some(-2.0),
                step: Some(0.1),
                default: Some(0.0),
            },
        ),
    ]
}

impl OpenAIProvider {
    fn supports_web_search(model: &str) -> bool {
        let model = model.strip_prefix("ft:").unwrap_or(model);
        if model == "gpt-4o-search-preview" || model == "gpt-4o-mini-search-preview" {
            return true;
        }
        if model == "gpt-4.1" || model == "gpt-4.1-mini" {
            return true;
        }
        model.starts_with("gpt-5")
    }

    fn get_body(
        template: &OpenAIRequestTemplate,
        history_messages: Vec<Message>,
    ) -> ChatRequest<'_> {
        let tools = (template.web_search && Self::supports_web_search(&template.model)).then(|| {
            vec![HostedTool {
                tool_type: "web_search",
            }]
        });
        ChatRequest {
            input: history_messages,
            model: template.model.as_str(),
            stream: template.stream,
            temperature: template.temperature,
            top_p: template.top_p,
            max_output_tokens: template.max_completion_tokens,
            presence_penalty: template.presence_penalty,
            frequency_penalty: template.frequency_penalty,
            tools,
        }
    }

    fn get_reqwest_client(
        config: &AiChatConfig,
        settings: &OpenAISettings,
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
            Some(proxy) => client = client.proxy(reqwest::Proxy::all(proxy)?),
        }
        Ok(client.build()?)
    }
}

impl Provider for OpenAIProvider {
    fn name(&self) -> &'static str {
        "OpenAI"
    }

    fn is_configured(&self, settings: &serde_json::Value) -> bool {
        settings
            .get("apiKey")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|api_key| !api_key.trim().is_empty())
    }

    fn default_template_for_model(&self, model: &ProviderModel) -> AiChatResult<serde_json::Value> {
        Ok(serde_json::to_value(OpenAIRequestTemplate {
            model: model.id.clone(),
            stream: model.capability.stream_flag(),
            web_search: Self::supports_web_search(&model.id),
            ..OpenAIRequestTemplate::default()
        })?)
    }

    fn get_template_inputs(&self) -> Vec<InputItem> {
        request_inputs()
    }

    fn request_body(
        &self,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> AiChatResult<serde_json::Value> {
        let template = OpenAIRequestTemplate::deserialize(template)?;
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
    ) -> BoxStream<'a, AiChatResult<FetchUpdate>> {
        async_stream::try_stream! {
            let settings: OpenAISettings = settings.try_into()?;
            let settings = settings.normalized();
            let client = Self::get_reqwest_client(&config, &settings)?;
            let stream = request_body
                .get("stream")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if stream {
                let mut es = client
                    .post(responses_url(&settings.base_url))
                    .json(&request_body)
                    .eventsource()?;
                while let Some(event) = es.next().await {
                    match event {
                        Ok(Event::Open) => {}
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
            } else {
                let response = client
                    .post(responses_url(&settings.base_url))
                    .json(&request_body)
                    .send()
                    .compat()
                    .await?;
                let response = response
                    .json::<ResponsesCreateResponse>()
                    .compat()
                    .await?;
                yield FetchUpdate::Complete(response.into_content());
            }
        }
        .boxed()
    }

    fn list_models(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
    ) -> BoxFuture<'static, AiChatResult<Vec<ProviderModel>>> {
        async move {
            let settings: OpenAISettings = settings.try_into()?;
            let settings = settings.normalized();
            let client = Self::get_reqwest_client(&config, &settings)?;
            let response = client
                .get(models_url(&settings.base_url))
                .send()
                .compat()
                .await?
                .json::<ModelListResponse>()
                .compat()
                .await?;
            let mut models = response
                .data
                .into_iter()
                .filter_map(|model| {
                    let capability = classify_model(&model.id)?;
                    ProviderModel::new(OpenAIProvider.name(), model.id.clone(), capability).into()
                })
                .collect::<Vec<_>>();
            models.sort_by(|left, right| left.id.cmp(&right.id));
            Ok(models)
        }
        .boxed()
    }

    fn chat_form_layout(&self) -> ChatFormLayout {
        ChatFormLayout {
            inline_field_ids: Vec::new(),
            popover_groups: vec![ChatFormGroup::new(
                None,
                None,
                vec![
                    "web_search",
                    "temperature",
                    "top_p",
                    "n",
                    "max_completion_tokens",
                    "presence_penalty",
                    "frequency_penalty",
                ],
            )],
        }
    }

    fn setting_group(&self) -> gpui_component::setting::SettingGroup {
        SettingGroup::new()
            .title(t_static("settings-openai-title"))
            .item(SettingItem::new(
                t_static("field-api-key"),
                SettingField::input(
                    |cx| {
                        let openai_setting = openai_settings(cx);
                        openai_setting.api_key.map(Into::into).unwrap_or_default()
                    },
                    |value, cx| {
                        let mut open_settings = openai_settings(cx);
                        open_settings.api_key = if value.is_empty() {
                            None
                        } else {
                            Some(value.into())
                        };
                        save_openai_settings(open_settings, cx);
                    },
                ),
            ))
            .item(SettingItem::new(
                t_static("field-base-url"),
                SettingField::render(
                    |options, window, cx| {
                        let initial_value = openai_settings(cx).base_url;
                        let state = window
                            .use_keyed_state("openai-base-url-field", cx, |window, cx| {
                                let input = cx.new(|cx| {
                                    InputState::new(window, cx)
                                        .default_value(initial_value.clone())
                                });
                                let _subscription = cx.subscribe_in(&input, window, {
                                    move |state: &mut BaseUrlFieldState,
                                          input,
                                          event: &InputEvent,
                                          window,
                                          cx| {
                                        if !matches!(event, InputEvent::Change) {
                                            return;
                                        }

                                        let current_value = input.read(cx).value().to_string();
                                        let next_value = if current_value.trim().is_empty() {
                                            default_base_url()
                                        } else {
                                            normalize_base_url(&current_value)
                                        };

                                        if next_value == state.last_value {
                                            return;
                                        }

                                        if current_value != next_value {
                                            input.update(cx, |input, cx| {
                                                input.set_value(next_value.clone(), window, cx);
                                            });
                                        }

                                        let mut settings = openai_settings(cx);
                                        settings.base_url = next_value.clone();
                                        save_openai_settings(settings, cx);
                                        state.last_value = next_value;
                                    }
                                });

                                BaseUrlFieldState {
                                    input,
                                    last_value: initial_value,
                                    _subscription,
                                }
                            })
                            .read(cx);

                        Input::new(&state.input)
                            .with_size(options.size)
                            .w(px(256.))
                    },
                ),
            ))
            .item(SettingItem::new(
                t_static("field-http-proxy"),
                SettingField::input(
                    |cx| {
                        openai_settings(cx)
                            .http_proxy
                            .map(Into::into)
                            .unwrap_or_default()
                    },
                    |value, cx| {
                        let mut open_settings = openai_settings(cx);
                        open_settings.http_proxy = if value.is_empty() {
                            None
                        } else {
                            Some(value.into())
                        };
                        save_openai_settings(open_settings, cx);
                    },
                ),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OpenAIProvider, OpenAIRequestTemplate, Provider, ProviderModel, ProviderModelCapability,
        ResponsesCreateResponse, classify_model, normalize_base_url, parse_response_stream_event,
    };
    use crate::{database::Content, llm::FetchUpdate};
    use serde_json::json;

    #[test]
    fn normalize_base_url_strips_terminal_api_paths() {
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/responses"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/models"),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn normalize_base_url_defaults_when_empty() {
        assert_eq!(normalize_base_url(""), "https://api.openai.com/v1");
        assert_eq!(normalize_base_url("   "), "https://api.openai.com/v1");
    }

    #[test]
    fn openai_provider_requires_non_empty_api_key() {
        assert!(!OpenAIProvider.is_configured(&json!({})));
        assert!(!OpenAIProvider.is_configured(&json!({ "apiKey": "   " })));
        assert!(OpenAIProvider.is_configured(&json!({ "apiKey": "sk-test" })));
    }

    #[test]
    fn default_template_uses_model_stream_capability() -> anyhow::Result<()> {
        let model = ProviderModel::new("OpenAI", "gpt-5", ProviderModelCapability::Streaming);
        let template = serde_json::from_value::<OpenAIRequestTemplate>(
            OpenAIProvider.default_template_for_model(&model)?,
        )?;
        assert_eq!(template.model, "gpt-5");
        assert!(template.stream);
        assert!(template.web_search);
        Ok(())
    }

    #[test]
    fn unsupported_models_default_web_search_to_false() -> anyhow::Result<()> {
        let model = ProviderModel::new("OpenAI", "gpt-4o", ProviderModelCapability::Streaming);
        let template = serde_json::from_value::<OpenAIRequestTemplate>(
            OpenAIProvider.default_template_for_model(&model)?,
        )?;
        assert!(!template.web_search);
        Ok(())
    }

    #[test]
    fn chat_form_layout_uses_only_popover_fields() {
        let layout = OpenAIProvider.chat_form_layout();
        assert!(layout.inline_field_ids.is_empty());
        assert_eq!(layout.popover_groups.len(), 1);
    }

    #[test]
    fn classify_model_marks_o_series_as_non_streaming() {
        assert_eq!(
            classify_model("o3-mini"),
            Some(ProviderModelCapability::NonStreaming)
        );
        assert_eq!(
            classify_model("o4-mini"),
            Some(ProviderModelCapability::NonStreaming)
        );
    }

    #[test]
    fn classify_model_marks_gpt_series_as_streaming() {
        assert_eq!(
            classify_model("gpt-5"),
            Some(ProviderModelCapability::Streaming)
        );
        assert_eq!(
            classify_model("chatgpt-4o-latest"),
            Some(ProviderModelCapability::Streaming)
        );
    }

    #[test]
    fn classify_model_filters_dated_and_fp_models() {
        assert_eq!(classify_model("gpt-4o-2024-11-20"), None);
        assert_eq!(classify_model("gpt-3.5-0125"), None);
        assert_eq!(classify_model("gpt-4o-realtime-preview"), None);
        assert_eq!(classify_model("fp-model"), None);
    }

    #[test]
    fn request_body_includes_web_search_tool_for_supported_models() -> anyhow::Result<()> {
        let request_body = OpenAIProvider.request_body(
            &json!({
                "model": "gpt-5",
                "stream": false,
                "web_search": true,
                "temperature": 1.0,
                "top_p": 1.0,
                "n": 1,
                "max_completion_tokens": null,
                "presence_penalty": 0.0,
                "frequency_penalty": 0.0
            }),
            vec![],
        )?;
        assert_eq!(request_body["tools"][0]["type"], "web_search");
        Ok(())
    }

    #[test]
    fn request_body_omits_web_search_for_unsupported_models() -> anyhow::Result<()> {
        let request_body = OpenAIProvider.request_body(
            &json!({
                "model": "gpt-4o",
                "stream": false,
                "web_search": true,
                "temperature": 1.0,
                "top_p": 1.0,
                "n": 1,
                "max_completion_tokens": null,
                "presence_penalty": 0.0,
                "frequency_penalty": 0.0
            }),
            vec![],
        )?;
        assert!(request_body.get("tools").is_none());
        Ok(())
    }

    #[test]
    fn responses_create_response_extracts_web_search_citations() {
        let response = serde_json::from_value::<ResponsesCreateResponse>(json!({
            "output": [
                { "type": "web_search_call" },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "hello",
                            "annotations": [
                                {
                                    "type": "url_citation",
                                    "title": "Example",
                                    "url": "https://example.com",
                                    "start_index": 0,
                                    "end_index": 5
                                }
                            ]
                        }
                    ]
                }
            ]
        }))
        .expect("response");
        assert_eq!(
            response.into_content(),
            Content::WebSearch {
                text: "hello".to_string(),
                citations: vec![crate::database::UrlCitation {
                    title: Some("Example".to_string()),
                    url: "https://example.com".to_string(),
                    start_index: Some(0),
                    end_index: Some(5),
                }]
            }
        );
    }

    #[test]
    fn response_completed_event_yields_complete_update() -> anyhow::Result<()> {
        let update = parse_response_stream_event(
            r#"{"type":"response.completed","response":{"output":[{"type":"message","content":[{"type":"output_text","text":"done","annotations":[{"type":"url_citation","title":"Example","url":"https://example.com","start_index":0,"end_index":4}]}]}]}}"#,
        )?;
        assert_eq!(
            update,
            Some(FetchUpdate::Complete(Content::WebSearch {
                text: "done".to_string(),
                citations: vec![crate::database::UrlCitation {
                    title: Some("Example".to_string()),
                    url: "https://example.com".to_string(),
                    start_index: Some(0),
                    end_index: Some(4),
                }]
            }))
        );
        Ok(())
    }
}
