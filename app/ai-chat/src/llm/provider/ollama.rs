use super::{
    ExtSettingControl, ExtSettingItem, ExtSettingOption, FetchUpdate, Provider, ProviderModel,
    ProviderModelCapability, ProviderSettingsFieldKind, ProviderSettingsFieldSpec,
    ProviderSettingsSpec,
};
use crate::{
    database::{Content, Role, UrlCitation},
    errors::{AiChatError, AiChatResult},
    llm::Message,
    state::AiChatConfig,
};
use futures::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use gpui::App;
use reqwest::Client;
use reqwest::StatusCode;
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use std::net::IpAddr;
use toml::Value;
use tracing::{Level, event};

pub(crate) struct OllamaProvider;

const BASE_URL_FIELD_KEY: &str = "baseUrl";

const OLLAMA_SETTINGS_FIELDS: &[ProviderSettingsFieldSpec] = &[ProviderSettingsFieldSpec {
    key: BASE_URL_FIELD_KEY,
    label_key: "field-base-url",
    kind: ProviderSettingsFieldKind::Text,
    search_keywords: "ollama base url endpoint local",
}];

const THINK_KEY: &str = "think";
const WEB_SEARCH_KEY: &str = "web_search";
const THINK_LOW: &str = "low";
const THINK_MEDIUM: &str = "medium";
const THINK_HIGH: &str = "high";
const THINKING_OPTIONS: &[&str] = &[THINK_LOW, THINK_MEDIUM, THINK_HIGH];
const WEB_SEARCH_TOOLTIP_KEY: &str = "tooltip-ollama-web-search-help";

fn default_base_url() -> String {
    "http://localhost:11434".to_string()
}

fn normalize_base_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return default_base_url();
    }
    let suffixes = [
        "/api/chat",
        "/api/tags",
        "/api/show",
        "/api/experimental/web_search",
        "/api/experimental/web_fetch",
    ];
    for suffix in suffixes {
        if let Some(base) = trimmed.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    trimmed.to_string()
}

fn tags_url(base_url: &str) -> String {
    format!("{}/api/tags", normalize_base_url(base_url))
}

fn show_url(base_url: &str) -> String {
    format!("{}/api/show", normalize_base_url(base_url))
}

fn chat_url(base_url: &str) -> String {
    format!("{}/api/chat", normalize_base_url(base_url))
}

fn web_search_url(base_url: &str) -> String {
    format!(
        "{}/api/experimental/web_search",
        normalize_base_url(base_url)
    )
}

fn web_fetch_url(base_url: &str) -> String {
    format!(
        "{}/api/experimental/web_fetch",
        normalize_base_url(base_url)
    )
}

fn should_bypass_proxy(base_url: &str) -> bool {
    let Ok(url) = Url::parse(&normalize_base_url(base_url)) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_matches(['[', ']']);
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn deserialize_null_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

fn format_ollama_error_message(operation: &str, status: StatusCode, body: &str) -> String {
    #[derive(Deserialize)]
    struct OllamaErrorResponse {
        #[serde(default)]
        error: String,
    }

    let status_text = status
        .canonical_reason()
        .map(|reason| format!("{} {}", status.as_u16(), reason))
        .unwrap_or_else(|| status.as_u16().to_string());
    let detail = serde_json::from_str::<OllamaErrorResponse>(body)
        .ok()
        .map(|response| response.error)
        .filter(|error| !error.trim().is_empty())
        .or_else(|| {
            let trimmed = body.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });

    match detail {
        Some(detail) => format!("Ollama {operation} failed ({status_text}): {detail}"),
        None => format!("Ollama {operation} failed ({status_text})"),
    }
}

async fn error_for_status_with_ollama_message(
    response: reqwest::Response,
    operation: &str,
) -> AiChatResult<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.unwrap_or_default();
    Err(AiChatError::StreamError(format_ollama_error_message(
        operation, status, &body,
    )))
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct OllamaSettings {
    #[serde(rename = "baseUrl", alias = "url", default = "default_base_url")]
    pub base_url: String,
}

impl Default for OllamaSettings {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
        }
    }
}

impl OllamaSettings {
    pub(crate) fn normalized(mut self) -> Self {
        self.base_url = normalize_base_url(&self.base_url);
        self
    }
}

fn ollama_settings_from_config(config: &AiChatConfig) -> OllamaSettings {
    config
        .get_provider_settings(OllamaProvider.name())
        .and_then(|x| x.clone().try_into::<OllamaSettings>().ok())
        .map(OllamaSettings::normalized)
        .unwrap_or_default()
}

fn ollama_settings(cx: &App) -> OllamaSettings {
    ollama_settings_from_config(cx.global::<AiChatConfig>())
}

fn save_ollama_settings(settings: OllamaSettings, cx: &mut App) {
    let config = cx.global_mut::<AiChatConfig>();
    match Value::try_from(settings.normalized()) {
        Ok(settings) => config.set_provider_settings(OllamaProvider.name(), settings),
        Err(err) => {
            event!(Level::ERROR, "Failed to convert Ollama settings: {}", err);
            return;
        }
    }
    if let Err(err) = config.save() {
        event!(Level::ERROR, "Failed to save Ollama settings: {}", err);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum OllamaThinkValue {
    Boolean(bool),
    Level(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OllamaRequestTemplate {
    model: String,
    stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    think: Option<OllamaThinkValue>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    web_search: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OllamaStoredRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    think: Option<OllamaThinkValue>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    web_search: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct OllamaChatMessage {
    role: String,
    content: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    thinking: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OllamaToolCall>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    tool_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    tool_call_id: String,
}

#[derive(Clone, Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<OllamaThinkValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaToolDefinition>>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct OllamaChatResponse {
    #[serde(default)]
    message: OllamaChatMessage,
    #[serde(default)]
    done: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct OllamaToolCall {
    #[serde(default)]
    id: String,
    function: OllamaToolCallFunction,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct OllamaToolCallFunction {
    #[serde(default)]
    index: i32,
    name: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
struct OllamaToolDefinition {
    #[serde(rename = "type")]
    tool_type: &'static str,
    function: OllamaFunctionDefinition,
}

#[derive(Clone, Debug, Serialize)]
struct OllamaFunctionDefinition {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagModel {
    name: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    #[serde(default)]
    details: OllamaModelDetails,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    capabilities: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    family: String,
    #[serde(default, deserialize_with = "deserialize_null_default_vec")]
    families: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WebSearchResponse {
    #[serde(default)]
    results: Vec<WebSearchResult>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WebSearchResult {
    title: String,
    url: String,
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct WebFetchResponse {
    #[serde(default)]
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    links: Vec<String>,
}

struct RoundResult {
    message: OllamaChatMessage,
}

impl OllamaProvider {
    fn metadata_capabilities(model: &ProviderModel) -> Vec<String> {
        model
            .metadata
            .get("capabilities")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .collect()
    }

    fn model_family(model: &ProviderModel) -> String {
        model
            .metadata
            .get("family")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn model_families(model: &ProviderModel) -> Vec<String> {
        model
            .metadata
            .get("families")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .collect()
    }

    fn supports_capability(model: &ProviderModel, capability: &str) -> bool {
        Self::metadata_capabilities(model)
            .iter()
            .any(|candidate| candidate == capability)
    }

    fn supports_thinking(model: &ProviderModel) -> bool {
        Self::supports_capability(model, "thinking")
    }

    fn supports_tools(model: &ProviderModel) -> bool {
        Self::supports_capability(model, "tools")
    }

    fn uses_thinking_levels(model: &ProviderModel) -> bool {
        let family = Self::model_family(model);
        if matches!(family.as_str(), "gptoss" | "gpt-oss") {
            return true;
        }
        Self::model_families(model)
            .iter()
            .any(|family| matches!(family.as_str(), "gptoss" | "gpt-oss"))
    }

    fn default_think_for_model(model: &ProviderModel) -> Option<OllamaThinkValue> {
        (Self::supports_thinking(model) && !Self::uses_thinking_levels(model))
            .then_some(OllamaThinkValue::Boolean(false))
    }

    fn thinking_value_from_template(
        model: &ProviderModel,
        template: &serde_json::Value,
    ) -> ExtSettingControl {
        let think = template.get("think");
        if Self::uses_thinking_levels(model) {
            let value = match think {
                Some(serde_json::Value::String(value))
                    if THINKING_OPTIONS.contains(&value.as_str()) =>
                {
                    value.clone()
                }
                Some(serde_json::Value::Bool(false)) => THINK_MEDIUM.to_string(),
                _ => THINK_MEDIUM.to_string(),
            };
            return ExtSettingControl::Select {
                value,
                options: THINKING_OPTIONS
                    .iter()
                    .copied()
                    .map(|value| ExtSettingOption {
                        value,
                        label_key: match value {
                            THINK_LOW => "reasoning-effort-low",
                            THINK_MEDIUM => "reasoning-effort-medium",
                            THINK_HIGH => "reasoning-effort-high",
                            _ => "field-thinking",
                        },
                    })
                    .collect(),
            };
        }
        let enabled = match think {
            Some(serde_json::Value::Bool(value)) => *value,
            Some(serde_json::Value::String(value)) => !value.is_empty(),
            _ => false,
        };
        ExtSettingControl::Boolean(enabled)
    }

    fn web_search_enabled(template: &serde_json::Value) -> bool {
        template
            .get("web_search")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    fn remove_field(template: &mut serde_json::Value, key: &str) -> AiChatResult<()> {
        let Some(object) = template.as_object_mut() else {
            return Err(AiChatError::StreamError(
                "request template must be a JSON object".to_string(),
            ));
        };
        object.remove(key);
        Ok(())
    }

    fn set_field(
        template: &mut serde_json::Value,
        key: &str,
        value: serde_json::Value,
    ) -> AiChatResult<()> {
        let Some(object) = template.as_object_mut() else {
            return Err(AiChatError::StreamError(
                "request template must be a JSON object".to_string(),
            ));
        };
        object.insert(key.to_string(), value);
        Ok(())
    }

    fn to_ollama_message(message: Message) -> OllamaChatMessage {
        OllamaChatMessage {
            role: match message.role {
                Role::Developer => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            }
            .to_string(),
            content: message.content,
            ..Default::default()
        }
    }

    fn tool_definitions() -> Vec<OllamaToolDefinition> {
        vec![
            OllamaToolDefinition {
                tool_type: "function",
                function: OllamaFunctionDefinition {
                    name: "web_search",
                    description: "Search the web for current information",
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" },
                            "max_results": { "type": "integer" }
                        },
                        "required": ["query"]
                    }),
                },
            },
            OllamaToolDefinition {
                tool_type: "function",
                function: OllamaFunctionDefinition {
                    name: "web_fetch",
                    description: "Fetch a web page by URL",
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" }
                        },
                        "required": ["url"]
                    }),
                },
            },
        ]
    }

    fn client(config: &AiChatConfig, base_url: &str) -> AiChatResult<Client> {
        let mut client = reqwest::ClientBuilder::new();
        if let Some(proxy) = config
            .get_http_proxy()
            .filter(|_| !should_bypass_proxy(base_url))
        {
            client = client.proxy(reqwest::Proxy::all(proxy)?);
        }
        Ok(client.build()?)
    }

    async fn execute_tool_call(
        client: &Client,
        base_url: &str,
        tool_call: &OllamaToolCall,
    ) -> AiChatResult<(OllamaChatMessage, Vec<UrlCitation>)> {
        match tool_call.function.name.as_str() {
            "web_search" => {
                let query = tool_call
                    .function
                    .arguments
                    .get("query")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        AiChatError::StreamError("web_search tool requires query".to_string())
                    })?;
                let max_results = tool_call
                    .function
                    .arguments
                    .get("max_results")
                    .and_then(serde_json::Value::as_u64);
                let response = client
                    .post(web_search_url(base_url))
                    .json(&json!({
                        "query": query,
                        "max_results": max_results,
                    }))
                    .send()
                    .await?;
                let response =
                    error_for_status_with_ollama_message(response, "web_search request").await?;
                let result = response.json::<WebSearchResponse>().await?;
                let citations = result
                    .results
                    .iter()
                    .map(|entry| UrlCitation {
                        title: Some(entry.title.clone()),
                        url: entry.url.clone(),
                        start_index: None,
                        end_index: None,
                    })
                    .collect::<Vec<_>>();
                Ok((
                    OllamaChatMessage {
                        role: "tool".to_string(),
                        content: serde_json::to_string(&result)?,
                        tool_name: "web_search".to_string(),
                        tool_call_id: tool_call.id.clone(),
                        ..Default::default()
                    },
                    citations,
                ))
            }
            "web_fetch" => {
                let url = tool_call
                    .function
                    .arguments
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        AiChatError::StreamError("web_fetch tool requires url".to_string())
                    })?;
                let response = client
                    .post(web_fetch_url(base_url))
                    .json(&json!({ "url": url }))
                    .send()
                    .await?;
                let response =
                    error_for_status_with_ollama_message(response, "web_fetch request").await?;
                let result = response.json::<WebFetchResponse>().await?;
                let citations = vec![UrlCitation {
                    title: (!result.title.trim().is_empty()).then(|| result.title.clone()),
                    url: url.to_string(),
                    start_index: None,
                    end_index: None,
                }];
                Ok((
                    OllamaChatMessage {
                        role: "tool".to_string(),
                        content: serde_json::to_string(&result)?,
                        tool_name: "web_fetch".to_string(),
                        tool_call_id: tool_call.id.clone(),
                        ..Default::default()
                    },
                    citations,
                ))
            }
            name => Err(AiChatError::StreamError(format!(
                "unsupported Ollama tool call: {name}"
            ))),
        }
    }

    async fn execute_tools(
        client: &Client,
        base_url: &str,
        tool_calls: &mut [OllamaToolCall],
    ) -> AiChatResult<(Vec<OllamaChatMessage>, Vec<UrlCitation>)> {
        let mut messages = Vec::new();
        let mut citations = Vec::new();
        for (index, tool_call) in tool_calls.iter_mut().enumerate() {
            if tool_call.id.trim().is_empty() {
                tool_call.id = format!("ollama-tool-{index}");
            }
            let (message, tool_citations) =
                Self::execute_tool_call(client, base_url, tool_call).await?;
            messages.push(message);
            citations.extend(tool_citations);
        }
        Ok((messages, citations))
    }

    fn merge_content(
        round_message: &OllamaChatMessage,
        citations: Vec<UrlCitation>,
        previous_reasoning: Option<String>,
    ) -> Content {
        let mut content = Content::new(round_message.content.clone());
        let reasoning_summary =
            previous_reasoning.unwrap_or_else(|| round_message.thinking.clone());
        content.reasoning_summary =
            (!reasoning_summary.trim().is_empty()).then_some(reasoning_summary);
        content.citations = citations;
        content
    }
}

impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "Ollama"
    }

    fn is_configured(&self, settings: &serde_json::Value) -> bool {
        settings
            .get("baseUrl")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|base_url| !base_url.trim().is_empty())
    }

    fn default_template_for_model(&self, model: &ProviderModel) -> AiChatResult<serde_json::Value> {
        Ok(serde_json::to_value(OllamaRequestTemplate {
            model: model.id.clone(),
            stream: model.capability.stream_flag(),
            think: Self::default_think_for_model(model),
            web_search: false,
        })?)
    }

    fn request_body(
        &self,
        template: &serde_json::Value,
        history_messages: Vec<Message>,
    ) -> AiChatResult<serde_json::Value> {
        let template = OllamaRequestTemplate::deserialize(template)?;
        let request = OllamaStoredRequest {
            model: template.model,
            messages: history_messages
                .into_iter()
                .map(Self::to_ollama_message)
                .collect(),
            stream: template.stream,
            think: template.think,
            web_search: template.web_search,
        };
        Ok(serde_json::to_value(request)?)
    }

    fn fetch_by_request_body<'a>(
        &self,
        config: AiChatConfig,
        settings: toml::Value,
        request_body: &'a serde_json::Value,
    ) -> BoxStream<'a, AiChatResult<FetchUpdate>> {
        async_stream::try_stream! {
            let settings: OllamaSettings = settings.try_into()?;
            let settings = settings.normalized();
            let client = Self::client(&config, &settings.base_url)?;
            let mut request = OllamaStoredRequest::deserialize(request_body)?;
            let mut final_citations = Vec::new();
            let mut accumulated_reasoning = String::new();

            loop {
                let chat_request = OllamaChatRequest {
                    model: request.model.clone(),
                    messages: request.messages.clone(),
                    stream: request.stream,
                    think: request.think.clone(),
                    tools: (request.web_search).then(Self::tool_definitions),
                };
                let response = client
                    .post(chat_url(&settings.base_url))
                    .json(&chat_request)
                    .send()
                    .await?;
                let response = error_for_status_with_ollama_message(response, "chat request").await?;

                let round = if !chat_request.stream {
                    let response = response.json::<OllamaChatResponse>().await?;
                    RoundResult {
                        message: response.message,
                    }
                } else {
                    let mut round_message = OllamaChatMessage {
                        role: "assistant".to_string(),
                        ..Default::default()
                    };
                    let mut buffer = Vec::new();
                    let mut stream = response.bytes_stream();
                    let mut emitted_thinking_started = false;
                    let mut done_received = false;

                    while !done_received {
                        let Some(chunk) = stream.next().await else {
                            break;
                        };
                        let chunk = chunk?;
                        buffer.extend_from_slice(&chunk);

                        while let Some(position) = buffer.iter().position(|byte| *byte == b'\n') {
                            let line = buffer.drain(..=position).collect::<Vec<_>>();
                            let line = String::from_utf8_lossy(&line);
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            let event = serde_json::from_str::<OllamaChatResponse>(line)?;
                            if !event.message.thinking.is_empty() {
                                if !emitted_thinking_started {
                                    yield FetchUpdate::ThinkingStarted;
                                    emitted_thinking_started = true;
                                }
                                accumulated_reasoning.push_str(&event.message.thinking);
                                yield FetchUpdate::ReasoningSummaryDelta(event.message.thinking.clone());
                                round_message.thinking.push_str(&event.message.thinking);
                            }
                            if !event.message.content.is_empty() {
                                yield FetchUpdate::TextDelta(event.message.content.clone());
                                round_message.content.push_str(&event.message.content);
                            }
                            if !event.message.tool_calls.is_empty() {
                                round_message.tool_calls = event.message.tool_calls.clone();
                            }
                            if !event.message.role.is_empty() {
                                round_message.role = event.message.role;
                            }
                            if event.done {
                                done_received = true;
                                break;
                            }
                        }
                    }

                    RoundResult {
                        message: round_message,
                    }
                };

                if request.web_search && !round.message.tool_calls.is_empty() {
                    let mut tool_calls = round.message.tool_calls.clone();
                    let (tool_messages, citations) =
                        Self::execute_tools(&client, &settings.base_url, &mut tool_calls).await?;
                    final_citations.extend(citations);
                    request.messages.push(OllamaChatMessage {
                        role: "assistant".to_string(),
                        content: round.message.content.clone(),
                        thinking: round.message.thinking.clone(),
                        tool_calls,
                        ..Default::default()
                    });
                    request.messages.extend(tool_messages);
                    continue;
                }

                let content = Self::merge_content(
                    &round.message,
                    final_citations,
                    (!accumulated_reasoning.is_empty()).then_some(accumulated_reasoning),
                );
                yield FetchUpdate::Complete(content);
                break;
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
            let settings: OllamaSettings = settings.try_into()?;
            let settings = settings.normalized();
            let client = Self::client(&config, &settings.base_url)?;
            let response = client.get(tags_url(&settings.base_url)).send().await?;
            let response = error_for_status_with_ollama_message(response, "tags request")
                .await?
                .json::<OllamaTagsResponse>()
                .await?;
            let mut models = Vec::new();
            for model in response.models {
                let show = client
                    .post(show_url(&settings.base_url))
                    .json(&json!({ "model": model.model }))
                    .send()
                    .await?;
                let show = error_for_status_with_ollama_message(show, "show request")
                    .await?
                    .json::<OllamaShowResponse>()
                    .await
                    .map_err(|err| {
                        AiChatError::StreamError(format!(
                            "decode Ollama show response failed for model {}: {}",
                            model.name, err
                        ))
                    })?;
                if !show
                    .capabilities
                    .iter()
                    .any(|capability| capability == "completion")
                {
                    continue;
                }
                models.push(
                    ProviderModel::new(
                        OllamaProvider.name(),
                        model.name,
                        ProviderModelCapability::Streaming,
                    )
                    .with_metadata(json!({
                        "capabilities": show.capabilities,
                        "family": show.details.family,
                        "families": show.details.families,
                    })),
                );
            }
            models.sort_by(|left, right| left.id.cmp(&right.id));
            Ok(models)
        }
        .boxed()
    }

    fn settings_spec(&self) -> ProviderSettingsSpec {
        ProviderSettingsSpec {
            provider_name: self.name(),
            title_key: "settings-ollama-title",
            fields: OLLAMA_SETTINGS_FIELDS,
        }
    }

    fn read_settings_field(&self, key: &str, config: &AiChatConfig) -> Option<String> {
        let settings = ollama_settings_from_config(config);
        match key {
            BASE_URL_FIELD_KEY => Some(settings.base_url),
            _ => None,
        }
    }

    fn write_settings_field(&self, key: &str, value: String, cx: &mut App) -> AiChatResult<()> {
        let mut settings = ollama_settings(cx);
        match key {
            BASE_URL_FIELD_KEY => {
                settings.base_url = if value.trim().is_empty() {
                    default_base_url()
                } else {
                    normalize_base_url(&value)
                };
            }
            _ => {
                return Err(AiChatError::StreamError(format!(
                    "unsupported Ollama settings field: {key}"
                )));
            }
        }
        save_ollama_settings(settings, cx);
        Ok(())
    }

    fn ext_settings(
        &self,
        model: &ProviderModel,
        template: &serde_json::Value,
    ) -> AiChatResult<Vec<ExtSettingItem>> {
        let mut settings = Vec::new();
        if Self::supports_thinking(model) {
            settings.push(ExtSettingItem {
                key: THINK_KEY,
                label_key: "field-thinking",
                tooltip: None,
                control: Self::thinking_value_from_template(model, template),
            });
        }
        if Self::supports_tools(model) {
            settings.push(ExtSettingItem {
                key: WEB_SEARCH_KEY,
                label_key: "field-web-search",
                tooltip: Some(WEB_SEARCH_TOOLTIP_KEY),
                control: ExtSettingControl::Boolean(Self::web_search_enabled(template)),
            });
        }
        Ok(settings)
    }

    fn apply_ext_setting(
        &self,
        model: &ProviderModel,
        template: &mut serde_json::Value,
        setting: &ExtSettingItem,
    ) -> AiChatResult<()> {
        match setting.key {
            THINK_KEY => {
                if !Self::supports_thinking(model) {
                    let _ = Self::remove_field(template, THINK_KEY);
                    return Ok(());
                }
                if Self::uses_thinking_levels(model) {
                    let ExtSettingControl::Select { value, .. } = &setting.control else {
                        return Err(AiChatError::StreamError(
                            "ollama think level must use select control".to_string(),
                        ));
                    };
                    if !THINKING_OPTIONS.contains(&value.as_str()) {
                        return Err(AiChatError::StreamError(format!(
                            "unsupported ollama think level: {value}"
                        )));
                    }
                    if value == THINK_MEDIUM {
                        return Self::remove_field(template, THINK_KEY);
                    }
                    return Self::set_field(
                        template,
                        THINK_KEY,
                        serde_json::Value::String(value.clone()),
                    );
                }
                let ExtSettingControl::Boolean(value) = &setting.control else {
                    return Err(AiChatError::StreamError(
                        "ollama think must use boolean control".to_string(),
                    ));
                };
                Self::set_field(template, THINK_KEY, serde_json::Value::Bool(*value))
            }
            WEB_SEARCH_KEY => {
                let ExtSettingControl::Boolean(value) = &setting.control else {
                    return Err(AiChatError::StreamError(
                        "ollama web_search must use boolean control".to_string(),
                    ));
                };
                if !Self::supports_tools(model) || !*value {
                    return Self::remove_field(template, WEB_SEARCH_KEY);
                }
                Self::set_field(template, WEB_SEARCH_KEY, serde_json::Value::Bool(true))
            }
            key => Err(AiChatError::StreamError(format!(
                "unsupported Ollama setting: {key}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExtSettingControl, OllamaChatMessage, OllamaProvider, OllamaStoredRequest, Provider,
        ProviderModel, ProviderModelCapability, THINK_HIGH, THINK_KEY, THINK_LOW, THINK_MEDIUM,
        WEB_SEARCH_KEY, WEB_SEARCH_TOOLTIP_KEY, should_bypass_proxy,
    };
    use crate::{database::Role, llm::ExtSettingItem};
    use serde_json::json;

    fn model_with_metadata(
        id: &str,
        capabilities: &[&str],
        family: &str,
        families: &[&str],
    ) -> ProviderModel {
        ProviderModel::new("Ollama", id, ProviderModelCapability::Streaming).with_metadata(json!({
            "capabilities": capabilities,
            "family": family,
            "families": families,
        }))
    }

    #[test]
    fn ext_settings_use_boolean_for_standard_thinking_models() -> anyhow::Result<()> {
        let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
        let settings = OllamaProvider.ext_settings(&model, &json!({}))?;
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].key, THINK_KEY);
        assert_eq!(settings[0].tooltip, None);
        assert_eq!(settings[0].control, ExtSettingControl::Boolean(false));
        Ok(())
    }

    #[test]
    fn ext_settings_use_select_for_gptoss_models() -> anyhow::Result<()> {
        let model = model_with_metadata(
            "gpt-oss:20b",
            &["completion", "thinking", "tools"],
            "gptoss",
            &["gptoss"],
        );
        let settings = OllamaProvider.ext_settings(&model, &json!({ "web_search": true }))?;
        assert_eq!(
            settings[0].control,
            ExtSettingControl::Select {
                value: THINK_MEDIUM.to_string(),
                options: vec![
                    crate::llm::ExtSettingOption {
                        value: THINK_LOW,
                        label_key: "reasoning-effort-low",
                    },
                    crate::llm::ExtSettingOption {
                        value: THINK_MEDIUM,
                        label_key: "reasoning-effort-medium",
                    },
                    crate::llm::ExtSettingOption {
                        value: THINK_HIGH,
                        label_key: "reasoning-effort-high",
                    },
                ],
            }
        );
        assert_eq!(settings[1].key, WEB_SEARCH_KEY);
        assert_eq!(settings[1].tooltip, Some(WEB_SEARCH_TOOLTIP_KEY));
        assert_eq!(settings[1].control, ExtSettingControl::Boolean(true));
        Ok(())
    }

    #[test]
    fn default_template_disables_standard_thinking_models() -> anyhow::Result<()> {
        let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
        let template = OllamaProvider.default_template_for_model(&model)?;
        assert_eq!(template["think"], false);
        Ok(())
    }

    #[test]
    fn default_template_omits_think_for_non_thinking_models() -> anyhow::Result<()> {
        let model = model_with_metadata("llama3.2", &["completion"], "llama", &["llama"]);
        let template = OllamaProvider.default_template_for_model(&model)?;
        assert!(template.get("think").is_none());
        Ok(())
    }

    #[test]
    fn default_template_keeps_gptoss_level_thinking_default() -> anyhow::Result<()> {
        let model = model_with_metadata(
            "gpt-oss:20b",
            &["completion", "thinking"],
            "gptoss",
            &["gptoss"],
        );
        let template = OllamaProvider.default_template_for_model(&model)?;
        assert!(template.get("think").is_none());
        Ok(())
    }

    #[test]
    fn request_body_preserves_default_false_think() -> anyhow::Result<()> {
        let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
        let template = OllamaProvider.default_template_for_model(&model)?;
        let request = OllamaProvider.request_body(
            &template,
            vec![crate::llm::Message::new(Role::User, "hello".to_string())],
        )?;
        assert_eq!(request["think"], false);
        Ok(())
    }

    #[test]
    fn apply_ext_setting_writes_boolean_think_explicitly() -> anyhow::Result<()> {
        let model = model_with_metadata("qwen3", &["completion", "thinking"], "qwen3", &["qwen3"]);
        let mut template = OllamaProvider.default_template_for_model(&model)?;
        OllamaProvider.apply_ext_setting(
            &model,
            &mut template,
            &ExtSettingItem {
                key: THINK_KEY,
                label_key: "field-thinking",
                tooltip: None,
                control: ExtSettingControl::Boolean(true),
            },
        )?;
        assert_eq!(template["think"], true);

        OllamaProvider.apply_ext_setting(
            &model,
            &mut template,
            &ExtSettingItem {
                key: THINK_KEY,
                label_key: "field-thinking",
                tooltip: None,
                control: ExtSettingControl::Boolean(false),
            },
        )?;
        assert_eq!(template["think"], false);
        Ok(())
    }

    #[test]
    fn apply_ext_setting_writes_gptoss_think_levels() -> anyhow::Result<()> {
        let model = model_with_metadata(
            "gpt-oss:20b",
            &["completion", "thinking"],
            "gptoss",
            &["gptoss"],
        );
        let mut template = OllamaProvider.default_template_for_model(&model)?;
        OllamaProvider.apply_ext_setting(
            &model,
            &mut template,
            &ExtSettingItem {
                key: THINK_KEY,
                label_key: "field-thinking",
                tooltip: None,
                control: ExtSettingControl::Select {
                    value: THINK_HIGH.to_string(),
                    options: vec![],
                },
            },
        )?;
        assert_eq!(template["think"], THINK_HIGH);

        OllamaProvider.apply_ext_setting(
            &model,
            &mut template,
            &ExtSettingItem {
                key: THINK_KEY,
                label_key: "field-thinking",
                tooltip: None,
                control: ExtSettingControl::Select {
                    value: THINK_MEDIUM.to_string(),
                    options: vec![],
                },
            },
        )?;
        assert!(template.get("think").is_none());
        Ok(())
    }

    #[test]
    fn bypass_proxy_for_loopback_ollama_hosts() {
        assert!(should_bypass_proxy("http://localhost:11434"));
        assert!(should_bypass_proxy("http://127.0.0.1:11434"));
        assert!(should_bypass_proxy("http://[::1]:11434"));
        assert!(should_bypass_proxy("http://localhost:11434/api/chat"));
    }

    #[test]
    fn keep_proxy_for_non_loopback_ollama_hosts() {
        assert!(!should_bypass_proxy("http://192.168.1.10:11434"));
        assert!(!should_bypass_proxy("https://ollama.example.com"));
        assert!(!should_bypass_proxy("not-a-url"));
    }

    #[test]
    fn request_body_maps_developer_role_to_system() -> anyhow::Result<()> {
        let request = OllamaProvider.request_body(
            &json!({
                "model": "qwen3",
                "stream": true,
                "web_search": true
            }),
            vec![crate::llm::Message::new(
                Role::Developer,
                "system prompt".to_string(),
            )],
        )?;
        let request = serde_json::from_value::<OllamaStoredRequest>(request)?;
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "system");
        assert_eq!(request.messages[0].content, "system prompt");
        assert!(request.web_search);
        Ok(())
    }

    #[test]
    fn merge_content_does_not_duplicate_streamed_reasoning() {
        let content = OllamaProvider::merge_content(
            &OllamaChatMessage {
                content: "final answer".to_string(),
                thinking: "abc".to_string(),
                ..Default::default()
            },
            Vec::new(),
            Some("abc".to_string()),
        );
        assert_eq!(content.text, "final answer");
        assert_eq!(content.reasoning_summary.as_deref(), Some("abc"));
    }

    #[test]
    fn show_response_treats_null_slices_as_empty_vectors() -> anyhow::Result<()> {
        let response = serde_json::from_value::<super::OllamaShowResponse>(json!({
            "details": {
                "family": "qwen3_5",
                "families": null
            },
            "capabilities": null
        }))?;
        assert_eq!(response.details.family, "qwen3_5");
        assert!(response.details.families.is_empty());
        assert!(response.capabilities.is_empty());
        Ok(())
    }

    #[test]
    fn format_ollama_error_message_uses_json_error_field() {
        let message = super::format_ollama_error_message(
            "chat request",
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"error":"mlx runner failed"}"#,
        );
        assert_eq!(
            message,
            "Ollama chat request failed (500 Internal Server Error): mlx runner failed"
        );
    }

    #[test]
    fn format_ollama_error_message_falls_back_to_plain_text_body() {
        let message = super::format_ollama_error_message(
            "show request",
            reqwest::StatusCode::BAD_GATEWAY,
            "upstream gateway failure",
        );
        assert_eq!(
            message,
            "Ollama show request failed (502 Bad Gateway): upstream gateway failure"
        );
    }
}
