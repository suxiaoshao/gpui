use super::{
    ExtSettingControl, ExtSettingItem, ExtSettingOption, FetchUpdate, Provider, ProviderModel,
    ProviderModelCapability, ProviderSettingsFieldKind, ProviderSettingsFieldSpec,
    ProviderSettingsSpec, normalized_or_default, optional_setting_value,
};
use crate::{
    database::{Content, UrlCitation},
    errors::{AiChatError, AiChatResult},
    llm::Message,
    state::AiChatConfig,
};
use eventsource_stream::Eventsource;
use futures::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use gpui::App;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::digit1,
    combinator::{all_consuming, map, opt},
    multi::separated_list1,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use toml::Value;
use tracing::{Level, event};

pub(crate) struct OpenAIProvider;

const API_KEY_FIELD_KEY: &str = "apiKey";
const BASE_URL_FIELD_KEY: &str = "baseUrl";
const HTTP_PROXY_FIELD_KEY: &str = "httpProxy";

const OPENAI_SETTINGS_FIELDS: &[ProviderSettingsFieldSpec] = &[
    ProviderSettingsFieldSpec {
        key: API_KEY_FIELD_KEY,
        label_key: "field-api-key",
        kind: ProviderSettingsFieldKind::SecretText,
        search_keywords: "openai api key secret token credential",
    },
    ProviderSettingsFieldSpec {
        key: BASE_URL_FIELD_KEY,
        label_key: "field-base-url",
        kind: ProviderSettingsFieldKind::Text,
        search_keywords: "openai base url endpoint api",
    },
    ProviderSettingsFieldSpec {
        key: HTTP_PROXY_FIELD_KEY,
        label_key: "field-http-proxy",
        kind: ProviderSettingsFieldKind::Text,
        search_keywords: "openai http proxy network",
    },
];

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

fn openai_settings_from_config(config: &AiChatConfig) -> OpenAISettings {
    config
        .get_provider_settings(OpenAIProvider.name())
        .and_then(|x| x.clone().try_into::<OpenAISettings>().ok())
        .map(OpenAISettings::normalized)
        .unwrap_or_default()
}

fn openai_settings(cx: &App) -> OpenAISettings {
    openai_settings_from_config(cx.global::<AiChatConfig>())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HostedTool {
    #[serde(rename = "type")]
    tool_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReasoningConfig {
    effort: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    input: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<HostedTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAIResponseStreamEvent {
    #[serde(rename = "response.output_item.added")]
    ResponseOutputItemAdded { item: OpenAIOutputItemEvent },
    #[serde(rename = "response.reasoning_summary_text.delta")]
    ResponseReasoningSummaryTextDelta { delta: String },
    #[serde(rename = "response.output_text.delta")]
    ResponseOutputTextDelta { delta: String },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "response.failed")]
    ResponseFailed { response: OpenAIResponse },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    error: OpenAIResponseError,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIOutputItemEvent {
    #[serde(rename = "type")]
    item_type: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct OpenAIRequestTemplate {
    model: String,
    stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<HostedTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningConfig>,
}

impl Default for OpenAIRequestTemplate {
    fn default() -> Self {
        Self {
            model: "gpt-4o".to_string(),
            stream: true,
            tools: None,
            reasoning: None,
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
    #[serde(default)]
    summary: Vec<ReasoningSummaryPart>,
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

#[derive(Debug, Deserialize)]
struct ReasoningSummaryPart {
    #[serde(rename = "type")]
    part_type: String,
    text: Option<String>,
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
        let mut content = Content::default();
        let mut citations = Vec::new();
        for item in self.output {
            if item.item_type.as_deref() == Some("reasoning") {
                let summary = item
                    .summary
                    .into_iter()
                    .filter(|part| part.part_type == "summary_text")
                    .filter_map(|part| part.text)
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if !summary.trim().is_empty() {
                    append_reasoning_summary_block(&mut content, &summary);
                }
                continue;
            }
            if item.item_type.as_deref() == Some("web_search_call") {
                continue;
            }
            for part in item.content.unwrap_or_default() {
                if part.content_type != "output_text" {
                    continue;
                }
                if let Some(part_text) = part.text {
                    content.text.push_str(&part_text);
                }
                citations.extend(
                    part.annotations
                        .into_iter()
                        .filter_map(OutputAnnotation::into_citation),
                );
            }
        }
        content.citations = citations;
        content
    }
}

fn append_reasoning_summary_block(content: &mut Content, summary: &str) {
    let summary = summary.trim();
    if summary.is_empty() {
        return;
    }
    if let Some(existing) = content.reasoning_summary.as_mut() {
        if !existing.trim().is_empty() {
            existing.push_str("\n\n");
        }
        existing.push_str(summary);
    } else {
        content.reasoning_summary = Some(summary.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenAIModelFamily {
    Gpt,
    ChatGpt,
    OSeries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReasoningProfile {
    default_effort: &'static str,
    options: &'static [&'static str],
}

const REASONING_EFFORT_KEY: &str = "reasoning.effort";
const REASONING_SUMMARY_AUTO: &str = "auto";
const REASONING_NONE: &str = "none";
const REASONING_MINIMAL: &str = "minimal";
const REASONING_LOW: &str = "low";
const REASONING_MEDIUM: &str = "medium";
const REASONING_HIGH: &str = "high";
const REASONING_XHIGH: &str = "xhigh";
const O_SERIES_REASONING_OPTIONS: &[&str] = &[REASONING_LOW, REASONING_MEDIUM, REASONING_HIGH];
const GPT_5_REASONING_OPTIONS: &[&str] = &[
    REASONING_MINIMAL,
    REASONING_LOW,
    REASONING_MEDIUM,
    REASONING_HIGH,
];
const GPT_5_1_REASONING_OPTIONS: &[&str] = &[
    REASONING_NONE,
    REASONING_LOW,
    REASONING_MEDIUM,
    REASONING_HIGH,
];
const GPT_5_2_PLUS_REASONING_OPTIONS: &[&str] = &[
    REASONING_NONE,
    REASONING_LOW,
    REASONING_MEDIUM,
    REASONING_HIGH,
    REASONING_XHIGH,
];
const GPT_5_PRO_REASONING_OPTIONS: &[&str] = &[REASONING_HIGH];
const GPT_5_2_PLUS_PRO_REASONING_OPTIONS: &[&str] =
    &[REASONING_MEDIUM, REASONING_HIGH, REASONING_XHIGH];

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
        OpenAIResponseStreamEvent::ResponseOutputItemAdded { item }
            if item.item_type == "reasoning" =>
        {
            Ok(Some(FetchUpdate::ThinkingStarted))
        }
        OpenAIResponseStreamEvent::ResponseOutputItemAdded { .. } => Ok(None),
        OpenAIResponseStreamEvent::ResponseReasoningSummaryTextDelta { delta } => {
            Ok(Some(FetchUpdate::ReasoningSummaryDelta(delta)))
        }
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

impl OpenAIProvider {
    fn reasoning_effort_label_key(effort: &str) -> &'static str {
        match effort {
            REASONING_NONE => "reasoning-effort-none",
            REASONING_MINIMAL => "reasoning-effort-minimal",
            REASONING_LOW => "reasoning-effort-low",
            REASONING_MEDIUM => "reasoning-effort-medium",
            REASONING_HIGH => "reasoning-effort-high",
            REASONING_XHIGH => "reasoning-effort-xhigh",
            _ => "field-reasoning-effort",
        }
    }

    fn gpt5_minor_version(parsed: &ParsedOpenAIModel<'_>) -> Option<u32> {
        if parsed.family != OpenAIModelFamily::Gpt {
            return None;
        }
        let version = *parsed.segments.get(1)?;
        if version == "5" {
            return Some(0);
        }
        version.strip_prefix("5.")?.parse::<u32>().ok()
    }

    fn reasoning_profile(model: &str) -> Option<ReasoningProfile> {
        let parsed = all_consuming(parse_openai_model).parse(model).ok()?.1;
        if parsed.family == OpenAIModelFamily::OSeries {
            return Some(ReasoningProfile {
                default_effort: REASONING_MEDIUM,
                options: O_SERIES_REASONING_OPTIONS,
            });
        }
        let minor = Self::gpt5_minor_version(&parsed)?;
        let is_pro = parsed
            .segments
            .last()
            .is_some_and(|segment| *segment == "pro");
        if is_pro {
            return Some(if minor >= 2 {
                ReasoningProfile {
                    default_effort: REASONING_MEDIUM,
                    options: GPT_5_2_PLUS_PRO_REASONING_OPTIONS,
                }
            } else {
                ReasoningProfile {
                    default_effort: REASONING_HIGH,
                    options: GPT_5_PRO_REASONING_OPTIONS,
                }
            });
        }
        Some(if minor >= 2 {
            ReasoningProfile {
                default_effort: REASONING_NONE,
                options: GPT_5_2_PLUS_REASONING_OPTIONS,
            }
        } else if minor == 1 {
            ReasoningProfile {
                default_effort: REASONING_NONE,
                options: GPT_5_1_REASONING_OPTIONS,
            }
        } else {
            ReasoningProfile {
                default_effort: REASONING_MEDIUM,
                options: GPT_5_REASONING_OPTIONS,
            }
        })
    }

    fn sanitize_reasoning(
        model: &str,
        reasoning: Option<ReasoningConfig>,
    ) -> Option<ReasoningConfig> {
        let profile = Self::reasoning_profile(model)?;
        let mut reasoning = reasoning.unwrap_or_else(|| ReasoningConfig {
            effort: profile.default_effort.to_string(),
            summary: None,
        });
        profile
            .options
            .iter()
            .any(|option| *option == reasoning.effort)
            .then(|| {
                reasoning.summary = Some(REASONING_SUMMARY_AUTO.to_string());
                reasoning
            })
    }

    fn reasoning_effort_from_template(template: &serde_json::Value) -> Option<&str> {
        template.get("reasoning")?.get("effort")?.as_str()
    }

    fn remove_reasoning(template: &mut serde_json::Value) {
        let Some(template) = template.as_object_mut() else {
            return;
        };
        template.remove("reasoning");
    }

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

    fn web_search_tool() -> HostedTool {
        HostedTool {
            tool_type: "web_search".to_string(),
        }
    }

    fn default_tools(model: &str) -> Option<Vec<HostedTool>> {
        Self::supports_web_search(model).then(|| vec![Self::web_search_tool()])
    }

    fn sanitize_tools(model: &str, tools: Vec<HostedTool>) -> Option<Vec<HostedTool>> {
        let tools = tools
            .into_iter()
            .filter(|tool| tool.tool_type != "web_search" || Self::supports_web_search(model))
            .collect::<Vec<_>>();
        (!tools.is_empty()).then_some(tools)
    }

    fn get_body(
        template: &OpenAIRequestTemplate,
        history_messages: Vec<Message>,
    ) -> ChatRequest<'_> {
        ChatRequest {
            input: history_messages,
            model: template.model.as_str(),
            stream: template.stream,
            tools: template
                .tools
                .clone()
                .and_then(|tools| Self::sanitize_tools(&template.model, tools)),
            reasoning: Self::sanitize_reasoning(&template.model, template.reasoning.clone()),
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
            tools: Self::default_tools(&model.id),
            reasoning: None,
        })?)
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
                let response = client
                    .post(responses_url(&settings.base_url))
                    .json(&request_body)
                    .send()
                    .await?;
                let response = response.error_for_status()?;
                let mut es = response.bytes_stream().eventsource();
                while let Some(event) = es.next().await {
                    match event {
                        Ok(message) => {
                            let message = message.data;
                            if message == "[DONE]" {
                                break;
                            } else if let Some(content) = parse_response_stream_event(&message)? {
                                yield content;
                            }
                        }
                        Err(err) => {
                            Err::<(), AiChatError>(AiChatError::StreamError(err.to_string()))?;
                        }
                    }
                }
            } else {
                let response = client
                    .post(responses_url(&settings.base_url))
                    .json(&request_body)
                    .send()
                    .await?;
                let response = response
                    .json::<ResponsesCreateResponse>()
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
                .await?
                .json::<ModelListResponse>()
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

    fn settings_spec(&self) -> ProviderSettingsSpec {
        ProviderSettingsSpec {
            provider_name: self.name(),
            title_key: "settings-openai-title",
            fields: OPENAI_SETTINGS_FIELDS,
        }
    }

    fn read_settings_field(&self, key: &str, config: &AiChatConfig) -> Option<String> {
        let settings = openai_settings_from_config(config);
        match key {
            API_KEY_FIELD_KEY => Some(settings.api_key.unwrap_or_default()),
            BASE_URL_FIELD_KEY => Some(settings.base_url),
            HTTP_PROXY_FIELD_KEY => Some(settings.http_proxy.unwrap_or_default()),
            _ => None,
        }
    }

    fn write_settings_field(&self, key: &str, value: String, cx: &mut App) -> AiChatResult<()> {
        let mut settings = openai_settings(cx);
        match key {
            API_KEY_FIELD_KEY => {
                settings.api_key = optional_setting_value(value);
            }
            BASE_URL_FIELD_KEY => {
                settings.base_url =
                    normalized_or_default(&value, default_base_url, normalize_base_url);
            }
            HTTP_PROXY_FIELD_KEY => {
                settings.http_proxy = optional_setting_value(value);
            }
            _ => {
                return Err(AiChatError::StreamError(format!(
                    "unsupported OpenAI settings field: {key}"
                )));
            }
        }
        save_openai_settings(settings, cx);
        crate::state::chat::reload_models(cx);
        Ok(())
    }

    fn ext_settings(
        &self,
        model: &ProviderModel,
        template: &serde_json::Value,
    ) -> AiChatResult<Vec<ExtSettingItem>> {
        let Some(profile) = Self::reasoning_profile(&model.id) else {
            return Ok(Vec::new());
        };
        let value = Self::reasoning_effort_from_template(template)
            .filter(|effort| profile.options.contains(effort))
            .unwrap_or(profile.default_effort)
            .to_string();
        Ok(vec![ExtSettingItem {
            key: REASONING_EFFORT_KEY,
            label_key: "field-reasoning-effort",
            tooltip: None,
            control: ExtSettingControl::Select {
                value,
                options: profile
                    .options
                    .iter()
                    .copied()
                    .map(|effort| ExtSettingOption {
                        value: effort,
                        label_key: Self::reasoning_effort_label_key(effort),
                    })
                    .collect(),
            },
        }])
    }

    fn apply_ext_setting(
        &self,
        model: &ProviderModel,
        template: &mut serde_json::Value,
        setting: &ExtSettingItem,
    ) -> AiChatResult<()> {
        if setting.key != REASONING_EFFORT_KEY {
            return Err(AiChatError::StreamError(format!(
                "unsupported OpenAI setting: {}",
                setting.key
            )));
        }
        let ExtSettingControl::Select { value, .. } = &setting.control else {
            return Err(AiChatError::StreamError(
                "reasoning.effort must use select control".to_string(),
            ));
        };
        let Some(profile) = Self::reasoning_profile(&model.id) else {
            Self::remove_reasoning(template);
            return Ok(());
        };
        if !profile.options.contains(&value.as_str()) {
            return Err(AiChatError::StreamError(format!(
                "unsupported reasoning.effort '{value}' for model '{}'",
                model.id
            )));
        }
        let Some(template_object) = template.as_object_mut() else {
            return Err(AiChatError::StreamError(
                "request template must be a JSON object".to_string(),
            ));
        };
        if value == profile.default_effort {
            template_object.remove("reasoning");
            return Ok(());
        }
        template_object.insert(
            "reasoning".to_string(),
            serde_json::to_value(ReasoningConfig {
                effort: value.to_string(),
                summary: Some(REASONING_SUMMARY_AUTO.to_string()),
            })?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
