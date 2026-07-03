use super::{
    ExtSettingControl, ExtSettingItem, ExtSettingOption, ModelCapabilities,
    OpenAIModelCapabilities, Provider, ProviderModel, ProviderSettingsFieldKind,
    ProviderSettingsFieldSpec, ProviderSettingsSpec, ReasoningCapability, ReasoningEffort,
    normalized_or_default, optional_setting_value,
};
use crate::{
    database::{Content, Role, UrlCitation},
    errors::{AiChatError, AiChatResult},
    llm::{
        LlmContentPart, LlmHostedToolCall, LlmInputItem, LlmMcpApprovalRequest, LlmOutputItem,
        LlmToolCall, ProviderRunEvent, ProviderRunRequest, ProviderRunState, ProviderUsage,
    },
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
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct HostedTool {
    #[serde(rename = "type")]
    tool_type: String,
    #[serde(default, flatten)]
    extra: Map<String, JsonValue>,
}

impl HostedTool {
    fn new(tool_type: impl Into<String>) -> Self {
        Self {
            tool_type: tool_type.into(),
            extra: Map::new(),
        }
    }
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
    input: Vec<OpenAIInputItem>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<HostedTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAIInputItem {
    Message(OpenAIInputMessage),
    ToolResult(OpenAIToolResultInput),
    ItemReference(OpenAIItemReferenceInput),
}

#[derive(Debug, Serialize)]
struct OpenAIInputMessage {
    role: &'static str,
    content: Vec<OpenAIInputContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum OpenAIInputContentPart {
    #[serde(rename = "input_text")]
    Text { text: String },
    #[serde(rename = "input_image")]
    Image {
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        detail: &'static str,
    },
    #[serde(rename = "input_file")]
    File { file_id: String },
}

#[derive(Debug, Serialize)]
struct OpenAIToolResultInput {
    #[serde(rename = "type")]
    item_type: &'static str,
    call_id: String,
    output: String,
}

#[derive(Debug, Serialize)]
struct OpenAIItemReferenceInput {
    #[serde(rename = "type")]
    item_type: &'static str,
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAIResponseStreamEvent {
    #[serde(rename = "response.created")]
    ResponseCreated { response: OpenAIResponseSnapshot },
    #[serde(rename = "response.in_progress")]
    ResponseInProgress { response: OpenAIResponseSnapshot },
    #[serde(rename = "response.output_item.added")]
    ResponseOutputItemAdded { item: OpenAIOutputItem },
    #[serde(rename = "response.output_item.done")]
    ResponseOutputItemDone { item: OpenAIOutputItem },
    #[serde(rename = "response.reasoning_summary_text.delta")]
    ResponseReasoningSummaryTextDelta { delta: String },
    #[serde(rename = "response.output_text.delta")]
    ResponseOutputTextDelta { delta: String },
    #[serde(rename = "response.function_call_arguments.done")]
    ResponseFunctionCallArgumentsDone {
        item_id: String,
        name: String,
        arguments: String,
    },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: ResponsesCreateResponse },
    #[serde(rename = "response.incomplete")]
    ResponseIncomplete { response: OpenAIResponse },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "response.failed")]
    ResponseFailed { response: OpenAIResponse },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: Option<String>,
    error: Option<OpenAIResponseError>,
    incomplete_details: Option<OpenAIIncompleteDetails>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIIncompleteDetails {
    reason: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseSnapshot {
    id: Option<String>,
    #[serde(default)]
    output: Vec<OpenAIOutputItem>,
    usage: Option<OpenAIUsage>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct OpenAIRequestTemplate {
    model: String,
    stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    include: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<HostedTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_choice: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
}

impl Default for OpenAIRequestTemplate {
    fn default() -> Self {
        Self {
            model: "gpt-4o".to_string(),
            stream: true,
            include: None,
            tools: None,
            reasoning: None,
            text: None,
            tool_choice: None,
            parallel_tool_calls: None,
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
    id: Option<String>,
    output: Vec<OpenAIOutputItem>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIOutputItem {
    id: Option<String>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    status: Option<String>,
    role: Option<String>,
    content: Option<Vec<OutputContent>>,
    #[serde(default)]
    summary: Vec<ReasoningSummaryPart>,
    call_id: Option<String>,
    name: Option<String>,
    arguments: Option<JsonValue>,
    server_label: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
struct OpenAIUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

#[derive(Default)]
struct OpenAIResponseStreamState {
    call_ids_by_item_id: HashMap<String, String>,
    pending_tool_calls_by_item_id: HashMap<String, PendingOpenAIToolCall>,
}

struct PendingOpenAIToolCall {
    name: String,
    arguments: serde_json::Value,
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

impl OpenAIOutputItem {
    fn output_item_id(&self) -> Option<String> {
        self.id.clone().or_else(|| self.call_id.clone())
    }

    fn call_id(&self) -> Option<String> {
        self.call_id.clone().or_else(|| self.id.clone())
    }

    fn explicit_call_id(&self) -> Option<&str> {
        self.call_id.as_deref()
    }

    fn is_function_call(&self) -> bool {
        matches!(
            self.item_type.as_deref(),
            Some("function_call" | "custom_tool_call")
        )
    }

    fn output_item(&self) -> Option<LlmOutputItem> {
        let item_type = self.item_type.as_deref()?;
        match item_type {
            "message" => Some(LlmOutputItem::Message {
                role: openai_output_role(self.role.as_deref()),
                content: self.output_content_parts(),
            }),
            "reasoning" => Some(LlmOutputItem::Reasoning {
                summary: self.reasoning_summary(),
            }),
            "function_call" | "custom_tool_call" => Some(LlmOutputItem::ToolCall(
                self.tool_call(item_type.to_string()),
            )),
            "mcp_approval_request" => Some(LlmOutputItem::McpApproval(self.mcp_approval_request())),
            "web_search_call"
            | "file_search_call"
            | "image_generation_call"
            | "code_interpreter_call"
            | "computer_call"
            | "computer_call_output"
            | "mcp_call"
            | "mcp_list_tools" => Some(LlmOutputItem::HostedToolCall(LlmHostedToolCall {
                call_id: self.output_item_id().unwrap_or_default(),
                tool_type: item_type.to_string(),
                status: self.status.clone(),
            })),
            _ => None,
        }
    }

    fn output_content_parts(&self) -> Vec<LlmContentPart> {
        self.content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|part| part.text_content_part())
            .collect()
    }

    fn reasoning_summary(&self) -> Option<String> {
        let summary = self
            .summary
            .iter()
            .filter(|part| part.part_type == "summary_text")
            .filter_map(|part| part.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n\n");
        (!summary.trim().is_empty()).then_some(summary)
    }

    fn tool_call(&self, fallback_name: String) -> LlmToolCall {
        LlmToolCall {
            call_id: self.call_id().unwrap_or_default(),
            name: self.name.clone().unwrap_or(fallback_name),
            arguments: parse_openai_arguments(self.arguments.as_ref()),
        }
    }

    fn mcp_approval_request(&self) -> LlmMcpApprovalRequest {
        LlmMcpApprovalRequest {
            request_id: self.call_id().unwrap_or_default(),
            server_label: self.server_label.clone().unwrap_or_default(),
            tool_name: self.name.clone().unwrap_or_default(),
            arguments: parse_openai_arguments(self.arguments.as_ref()),
        }
    }
}

impl OpenAIResponseStreamState {
    fn record_output_item(&mut self, item: &OpenAIOutputItem) -> Vec<ProviderRunEvent> {
        if !item.is_function_call() {
            return Vec::new();
        }
        let (Some(item_id), Some(call_id)) = (item.id.as_deref(), item.explicit_call_id()) else {
            return Vec::new();
        };

        self.call_ids_by_item_id
            .insert(item_id.to_string(), call_id.to_string());
        self.pending_tool_calls_by_item_id
            .remove(item_id)
            .map(|pending| {
                vec![ProviderRunEvent::ToolCallRequested(LlmToolCall {
                    call_id: call_id.to_string(),
                    name: pending.name,
                    arguments: pending.arguments,
                })]
            })
            .unwrap_or_default()
    }

    fn tool_call_requested(
        &mut self,
        item_id: String,
        name: String,
        arguments: String,
    ) -> Vec<ProviderRunEvent> {
        let arguments = parse_openai_argument_string(&arguments);
        if let Some(call_id) = self.call_ids_by_item_id.get(&item_id) {
            return vec![ProviderRunEvent::ToolCallRequested(LlmToolCall {
                call_id: call_id.clone(),
                name,
                arguments,
            })];
        }

        self.pending_tool_calls_by_item_id
            .insert(item_id, PendingOpenAIToolCall { name, arguments });
        Vec::new()
    }
}

impl OutputContent {
    fn text_content_part(&self) -> Option<LlmContentPart> {
        match self.content_type.as_str() {
            "output_text" | "refusal" => self.text.clone().map(LlmContentPart::Text),
            _ => None,
        }
    }
}

fn parse_openai_arguments(arguments: Option<&JsonValue>) -> JsonValue {
    match arguments {
        Some(JsonValue::String(arguments)) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| JsonValue::String(arguments.clone()))
        }
        Some(arguments) => arguments.clone(),
        None => serde_json::json!({}),
    }
}

fn parse_openai_argument_string(arguments: &str) -> JsonValue {
    serde_json::from_str(arguments).unwrap_or_else(|_| JsonValue::String(arguments.to_string()))
}

fn openai_output_role(role: Option<&str>) -> Role {
    match role {
        Some("developer") | Some("system") => Role::Developer,
        Some("user") => Role::User,
        Some("assistant") | None => Role::Assistant,
        Some(_) => Role::Assistant,
    }
}

fn continuation_metadata(request_body: &serde_json::Value) -> serde_json::Value {
    request_body
        .get("previous_response_id")
        .cloned()
        .map(|previous_response_id| serde_json::json!({ "previous_response_id": previous_response_id }))
        .unwrap_or(serde_json::Value::Null)
}

impl OpenAIUsage {
    fn provider_usage(&self) -> ProviderUsage {
        ProviderUsage::new(self.input_tokens, self.output_tokens, self.total_tokens)
    }
}

impl ResponsesCreateResponse {
    fn output_item_ids(&self) -> Vec<String> {
        self.output
            .iter()
            .filter_map(OpenAIOutputItem::output_item_id)
            .collect()
    }

    fn output_items(&self) -> Vec<LlmOutputItem> {
        self.output
            .iter()
            .filter_map(OpenAIOutputItem::output_item)
            .collect()
    }

    fn usage(&self) -> Option<ProviderUsage> {
        self.usage.as_ref().map(OpenAIUsage::provider_usage)
    }

    fn into_content(self) -> Content {
        let mut content = Content::default();
        let mut citations = Vec::new();
        for item in self.output {
            if item.item_type.as_deref() == Some("reasoning") {
                if let Some(summary) = item.reasoning_summary() {
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
    default_effort: ReasoningEffort,
    options: &'static [ReasoningEffort],
}

const REASONING_EFFORT_KEY: &str = "reasoning.effort";
const REASONING_SUMMARY_AUTO: &str = "auto";
#[cfg(test)]
const REASONING_NONE: &str = "none";
#[cfg(test)]
const REASONING_LOW: &str = "low";
#[cfg(test)]
const REASONING_MEDIUM: &str = "medium";
#[cfg(test)]
const REASONING_HIGH: &str = "high";
#[cfg(test)]
const REASONING_XHIGH: &str = "xhigh";
const O_SERIES_REASONING_OPTIONS: &[ReasoningEffort] = &[
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
];
const GPT_5_REASONING_OPTIONS: &[ReasoningEffort] = &[
    ReasoningEffort::Minimal,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
];
const GPT_5_1_REASONING_OPTIONS: &[ReasoningEffort] = &[
    ReasoningEffort::None,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
];
const GPT_5_2_PLUS_REASONING_OPTIONS: &[ReasoningEffort] = &[
    ReasoningEffort::None,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::XHigh,
];
const GPT_5_PRO_REASONING_OPTIONS: &[ReasoningEffort] = &[ReasoningEffort::High];
const GPT_5_2_PLUS_PRO_REASONING_OPTIONS: &[ReasoningEffort] = &[
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::XHigh,
];

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

fn classify_model(id: &str) -> Option<ModelCapabilities> {
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
    let mut capabilities = match parsed.family {
        OpenAIModelFamily::OSeries => ModelCapabilities::text_non_streaming(),
        OpenAIModelFamily::Gpt | OpenAIModelFamily::ChatGpt => ModelCapabilities::text_streaming(),
    };
    capabilities.reasoning =
        OpenAIProvider::reasoning_profile(id).map(|profile| ReasoningCapability {
            default_effort: profile.default_effort,
            efforts: profile.options.to_vec(),
            summaries: true,
        });
    capabilities.structured_output = true;
    let reasoning_summaries = capabilities.supports_reasoning();
    capabilities = capabilities.with_openai_extension(OpenAIModelCapabilities {
        responses_api: true,
        reasoning_summaries,
        hosted_web_search: OpenAIProvider::supports_web_search(id),
        stateful_response_continuation: true,
    });
    Some(capabilities)
}

#[cfg(test)]
fn parse_response_stream_event(message: &str) -> AiChatResult<Option<ProviderRunEvent>> {
    Ok(parse_response_stream_events(message)?.into_iter().next())
}

#[cfg(test)]
fn parse_response_stream_events(message: &str) -> AiChatResult<Vec<ProviderRunEvent>> {
    let mut state = OpenAIResponseStreamState::default();
    parse_response_stream_events_with_state(message, &mut state)
}

fn parse_response_stream_events_with_state(
    message: &str,
    state: &mut OpenAIResponseStreamState,
) -> AiChatResult<Vec<ProviderRunEvent>> {
    let event = serde_json::from_str::<OpenAIResponseStreamEvent>(message)?;
    match event {
        OpenAIResponseStreamEvent::ResponseCreated { response }
        | OpenAIResponseStreamEvent::ResponseInProgress { response } => {
            let _ = response.id;
            let _ = response.output;
            Ok(response
                .usage
                .map(|usage| vec![ProviderRunEvent::UsageUpdated(usage.provider_usage())])
                .unwrap_or_default())
        }
        OpenAIResponseStreamEvent::ResponseOutputItemAdded { item }
            if item.item_type.as_deref() == Some("reasoning") =>
        {
            let mut events = state.record_output_item(&item);
            events.push(ProviderRunEvent::ThinkingStarted);
            if let Some(item) = item.output_item() {
                events.push(ProviderRunEvent::OutputItemAdded(item));
            }
            Ok(events)
        }
        OpenAIResponseStreamEvent::ResponseOutputItemAdded { item } => {
            let mut events = state.record_output_item(&item);
            if let Some(item) = item.output_item() {
                events.push(ProviderRunEvent::OutputItemAdded(item));
            }
            Ok(events)
        }
        OpenAIResponseStreamEvent::ResponseOutputItemDone { item } => {
            let mut events = state.record_output_item(&item);
            if let Some(item) = item.output_item() {
                events.push(ProviderRunEvent::OutputItemDone(item));
            }
            Ok(events)
        }
        OpenAIResponseStreamEvent::ResponseReasoningSummaryTextDelta { delta } => {
            Ok(vec![ProviderRunEvent::ReasoningSummaryDelta(delta)])
        }
        OpenAIResponseStreamEvent::ResponseOutputTextDelta { delta } => {
            Ok(vec![ProviderRunEvent::TextDelta(delta)])
        }
        OpenAIResponseStreamEvent::ResponseFunctionCallArgumentsDone {
            item_id,
            name,
            arguments,
        } => Ok(state.tool_call_requested(item_id, name, arguments)),
        OpenAIResponseStreamEvent::ResponseCompleted { response } => {
            let state = ProviderRunState::new(
                OpenAIProvider.name(),
                response.id.clone(),
                response.output_item_ids(),
                serde_json::Value::Null,
            );
            let usage = response.usage();
            Ok(vec![ProviderRunEvent::Completed {
                content: response.into_content(),
                state: Some(state),
                usage,
            }])
        }
        OpenAIResponseStreamEvent::Error { message, .. } => Err(AiChatError::StreamError(message)),
        OpenAIResponseStreamEvent::ResponseFailed { response } => {
            let message = response
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| "OpenAI response failed".to_string());
            Err(AiChatError::StreamError(message))
        }
        OpenAIResponseStreamEvent::ResponseIncomplete { response } => {
            let message = response
                .incomplete_details
                .map(|details| format!("OpenAI response incomplete: {}", details.reason))
                .or_else(|| response.error.map(|error| error.message))
                .unwrap_or_else(|| "OpenAI response incomplete".to_string());
            let _ = response.id;
            Ok(vec![ProviderRunEvent::Failed { message }])
        }
        OpenAIResponseStreamEvent::Other => Ok(Vec::new()),
    }
}

impl OpenAIProvider {
    fn reasoning_effort_label_key(effort: ReasoningEffort) -> &'static str {
        match effort {
            ReasoningEffort::None => "reasoning-effort-none",
            ReasoningEffort::Minimal => "reasoning-effort-minimal",
            ReasoningEffort::Low => "reasoning-effort-low",
            ReasoningEffort::Medium => "reasoning-effort-medium",
            ReasoningEffort::High => "reasoning-effort-high",
            ReasoningEffort::XHigh => "reasoning-effort-xhigh",
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
                default_effort: ReasoningEffort::Medium,
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
                    default_effort: ReasoningEffort::Medium,
                    options: GPT_5_2_PLUS_PRO_REASONING_OPTIONS,
                }
            } else {
                ReasoningProfile {
                    default_effort: ReasoningEffort::High,
                    options: GPT_5_PRO_REASONING_OPTIONS,
                }
            });
        }
        Some(if minor >= 2 {
            ReasoningProfile {
                default_effort: ReasoningEffort::None,
                options: GPT_5_2_PLUS_REASONING_OPTIONS,
            }
        } else if minor == 1 {
            ReasoningProfile {
                default_effort: ReasoningEffort::None,
                options: GPT_5_1_REASONING_OPTIONS,
            }
        } else {
            ReasoningProfile {
                default_effort: ReasoningEffort::Medium,
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
            effort: profile.default_effort.as_str().to_string(),
            summary: None,
        });
        ReasoningEffort::from_str(&reasoning.effort)
            .filter(|effort| profile.options.contains(effort))
            .map(|_| {
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
        HostedTool::new("web_search")
    }

    fn default_tools_for_model(model: &ProviderModel) -> Option<Vec<HostedTool>> {
        model
            .capabilities
            .supports_hosted_web_search()
            .then(|| vec![Self::web_search_tool()])
    }

    fn sanitize_tools(model: &str, tools: Vec<HostedTool>) -> Option<Vec<HostedTool>> {
        let tools = tools
            .into_iter()
            .filter(|tool| tool.tool_type != "web_search" || Self::supports_web_search(model))
            .collect::<Vec<_>>();
        (!tools.is_empty()).then_some(tools)
    }

    fn get_body<'a>(
        template: &'a OpenAIRequestTemplate,
        input_items: Vec<LlmInputItem>,
        previous_state: Option<&ProviderRunState>,
    ) -> AiChatResult<ChatRequest<'a>> {
        Ok(ChatRequest {
            input: input_items
                .into_iter()
                .map(Self::to_openai_input_item)
                .collect::<AiChatResult<_>>()?,
            model: template.model.as_str(),
            stream: template.stream,
            previous_response_id: previous_state
                .filter(|state| state.provider_name == OpenAIProvider.name())
                .and_then(|state| state.run_id.clone()),
            include: template.include.clone(),
            tools: template
                .tools
                .clone()
                .and_then(|tools| Self::sanitize_tools(&template.model, tools)),
            reasoning: Self::sanitize_reasoning(&template.model, template.reasoning.clone()),
            text: template.text.clone(),
            tool_choice: template.tool_choice.clone(),
            parallel_tool_calls: template.parallel_tool_calls,
        })
    }

    fn to_openai_input_item(item: LlmInputItem) -> AiChatResult<OpenAIInputItem> {
        Ok(match item {
            LlmInputItem::System { content } => OpenAIInputItem::Message(OpenAIInputMessage {
                role: "system",
                content: Self::to_openai_content_parts(content)?,
            }),
            LlmInputItem::Developer { content } => OpenAIInputItem::Message(OpenAIInputMessage {
                role: "developer",
                content: Self::to_openai_content_parts(content)?,
            }),
            LlmInputItem::User { content } => OpenAIInputItem::Message(OpenAIInputMessage {
                role: "user",
                content: Self::to_openai_content_parts(content)?,
            }),
            LlmInputItem::Assistant { content } => OpenAIInputItem::Message(OpenAIInputMessage {
                role: "assistant",
                content: Self::to_openai_content_parts(content)?,
            }),
            LlmInputItem::ToolResult(result) => {
                OpenAIInputItem::ToolResult(OpenAIToolResultInput {
                    item_type: "function_call_output",
                    call_id: result.call_id,
                    output: Self::tool_result_output(result.content)?,
                })
            }
            LlmInputItem::ItemReference { item_id } => {
                OpenAIInputItem::ItemReference(OpenAIItemReferenceInput {
                    item_type: "item_reference",
                    id: item_id,
                })
            }
        })
    }

    fn to_openai_content_parts(
        content: Vec<LlmContentPart>,
    ) -> AiChatResult<Vec<OpenAIInputContentPart>> {
        if content.is_empty() {
            return Err(Self::unsupported_input("empty content"));
        }
        content
            .into_iter()
            .map(Self::to_openai_content_part)
            .collect()
    }

    fn to_openai_content_part(part: LlmContentPart) -> AiChatResult<OpenAIInputContentPart> {
        Ok(match part {
            LlmContentPart::Text(text) => OpenAIInputContentPart::Text { text },
            LlmContentPart::ImageRef(attachment) if Self::is_image_url(&attachment.id) => {
                OpenAIInputContentPart::Image {
                    image_url: Some(attachment.id),
                    file_id: None,
                    detail: "auto",
                }
            }
            LlmContentPart::ImageRef(attachment) => OpenAIInputContentPart::Image {
                image_url: None,
                file_id: Some(attachment.id),
                detail: "auto",
            },
            LlmContentPart::FileRef(attachment) => OpenAIInputContentPart::File {
                file_id: attachment.id,
            },
            LlmContentPart::AudioRef(_) => return Err(Self::unsupported_input("audio content")),
            LlmContentPart::AttachmentRef(_) => {
                return Err(Self::unsupported_input("generic attachment content"));
            }
        })
    }

    fn tool_result_output(content: Vec<LlmContentPart>) -> AiChatResult<String> {
        if content.is_empty() {
            return Err(Self::unsupported_input("empty tool result"));
        }
        let mut output = String::new();
        for part in content {
            match part {
                LlmContentPart::Text(text) => output.push_str(&text),
                LlmContentPart::ImageRef(_) => {
                    return Err(Self::unsupported_input("image tool result"));
                }
                LlmContentPart::FileRef(_) => {
                    return Err(Self::unsupported_input("file tool result"));
                }
                LlmContentPart::AudioRef(_) => {
                    return Err(Self::unsupported_input("audio tool result"));
                }
                LlmContentPart::AttachmentRef(_) => {
                    return Err(Self::unsupported_input("generic attachment tool result"));
                }
            }
        }
        Ok(output)
    }

    fn is_image_url(id: &str) -> bool {
        id.starts_with("http://") || id.starts_with("https://") || id.starts_with("data:")
    }

    fn unsupported_input(kind: &str) -> AiChatError {
        AiChatError::StreamError(format!("unsupported OpenAI Responses input item: {kind}"))
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
            stream: model.capabilities.supports_streaming(),
            include: None,
            tools: Self::default_tools_for_model(model),
            reasoning: None,
            text: None,
            tool_choice: None,
            parallel_tool_calls: None,
        })?)
    }

    fn build_run_request(
        &self,
        template: &serde_json::Value,
        input_items: Vec<LlmInputItem>,
    ) -> AiChatResult<ProviderRunRequest> {
        self.build_run_request_with_state(template, input_items, None)
    }

    fn build_run_request_with_state(
        &self,
        template: &serde_json::Value,
        input_items: Vec<LlmInputItem>,
        state: Option<ProviderRunState>,
    ) -> AiChatResult<ProviderRunRequest> {
        let template = OpenAIRequestTemplate::deserialize(template)?;
        let request_body = serde_json::to_value(Self::get_body(
            &template,
            input_items.clone(),
            state.as_ref(),
        )?)?;
        Ok(ProviderRunRequest {
            provider_name: self.name().to_string(),
            request_body,
            input_items,
            state,
        })
    }

    fn run<'a>(
        &'a self,
        config: AiChatConfig,
        settings: toml::Value,
        request: &'a ProviderRunRequest,
    ) -> BoxStream<'a, AiChatResult<ProviderRunEvent>> {
        async_stream::try_stream! {
            let settings: OpenAISettings = settings.try_into()?;
            let settings = settings.normalized();
            let client = Self::get_reqwest_client(&config, &settings)?;
            let stream = request.request_body
                .get("stream")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if stream {
                let response = client
                    .post(responses_url(&settings.base_url))
                    .json(&request.request_body)
                    .send()
                    .await?;
                let response = response.error_for_status()?;
                let mut es = response.bytes_stream().eventsource();
                let mut stream_state = OpenAIResponseStreamState::default();
                while let Some(event) = es.next().await {
                    match event {
                        Ok(message) => {
                            let message = message.data;
                            if message == "[DONE]" {
                                break;
                            } else {
                                for mut event in parse_response_stream_events_with_state(
                                    &message,
                                    &mut stream_state,
                                )? {
                                    if let ProviderRunEvent::Completed { state: Some(state), .. } = &mut event {
                                        state.request_body = request.request_body.clone();
                                        state.continuation_metadata =
                                            continuation_metadata(&request.request_body);
                                    }
                                    yield event;
                                }
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
                    .json(&request.request_body)
                    .send()
                    .await?;
                let response = response
                    .json::<ResponsesCreateResponse>()
                    .await?;
                let output_items = response.output_items();
                let state = ProviderRunState::new(
                    self.name(),
                    response.id.clone(),
                    response.output_item_ids(),
                    request.request_body.clone(),
                );
                let mut state = state;
                state.continuation_metadata = continuation_metadata(&request.request_body);
                let usage = response.usage();
                for item in output_items {
                    yield ProviderRunEvent::OutputItemDone(item);
                }
                yield ProviderRunEvent::Completed {
                    content: response.into_content(),
                    state: Some(state),
                    usage,
                };
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
                    let capabilities = classify_model(&model.id)?;
                    ProviderModel::new(OpenAIProvider.name(), model.id.clone(), capabilities).into()
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
        crate::state::chat::reload_models_debounced(cx);
        Ok(())
    }

    fn ext_settings(
        &self,
        model: &ProviderModel,
        template: &serde_json::Value,
    ) -> AiChatResult<Vec<ExtSettingItem>> {
        let Some(reasoning) = model.capabilities.reasoning.as_ref() else {
            return Ok(Vec::new());
        };
        let value = Self::reasoning_effort_from_template(template)
            .and_then(ReasoningEffort::from_str)
            .filter(|effort| reasoning.supports_effort(*effort))
            .unwrap_or(reasoning.default_effort)
            .as_str()
            .to_string();
        Ok(vec![ExtSettingItem {
            key: REASONING_EFFORT_KEY,
            label_key: "field-reasoning-effort",
            tooltip: None,
            control: ExtSettingControl::Select {
                value,
                options: reasoning
                    .efforts
                    .iter()
                    .copied()
                    .map(|effort| ExtSettingOption {
                        value: effort.as_str(),
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
        let Some(reasoning) = model.capabilities.reasoning.as_ref() else {
            Self::remove_reasoning(template);
            return Ok(());
        };
        let Some(effort) = ReasoningEffort::from_str(value) else {
            return Err(AiChatError::StreamError(format!(
                "unsupported reasoning.effort '{value}' for model '{}'",
                model.id
            )));
        };
        if !reasoning.supports_effort(effort) {
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
        if effort == reasoning.default_effort {
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
