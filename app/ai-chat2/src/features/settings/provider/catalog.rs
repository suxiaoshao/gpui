use crate::foundation::assets::IconName;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ProviderKindKey(String);

impl ProviderKindKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProviderKindKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderFieldKind {
    Text,
    Secret,
    Url,
    Select,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderSelectOption {
    pub(super) label_key: &'static str,
    pub(super) value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderFieldSchema {
    pub(super) key: &'static str,
    pub(super) label_key: &'static str,
    pub(super) kind: ProviderFieldKind,
    pub(super) required: bool,
    pub(super) placeholder_key: &'static str,
    pub(super) default_value: Option<&'static str>,
    pub(super) options: Vec<ProviderSelectOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderSpec {
    pub(super) kind: ProviderKindKey,
    pub(super) display_name: &'static str,
    pub(super) description_key: &'static str,
    pub(super) icon: IconName,
    pub(super) fields: Vec<ProviderFieldSchema>,
    pub(super) model_listing: ModelListingStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelListingStrategy {
    RigModels,
    OllamaTagsAndShow,
    Manual,
}

pub(super) fn builtin_provider_specs() -> Vec<ProviderSpec> {
    vec![
        api_key_provider("openai", "OpenAI", "provider-description-openai"),
        api_key_provider("anthropic", "Anthropic", "provider-description-anthropic"),
        api_key_provider("gemini", "Google Gemini", "provider-description-gemini"),
        ProviderSpec {
            kind: "ollama".into(),
            display_name: "Ollama",
            description_key: "provider-description-ollama",
            icon: IconName::Cpu,
            fields: vec![
                ProviderFieldSchema {
                    key: "base_url",
                    label_key: "provider-field-base-url",
                    kind: ProviderFieldKind::Url,
                    required: true,
                    placeholder_key: "provider-placeholder-ollama-base-url",
                    default_value: Some("http://localhost:11434"),
                    options: Vec::new(),
                },
                ProviderFieldSchema {
                    key: "bearer_token",
                    label_key: "provider-field-bearer-token",
                    kind: ProviderFieldKind::Secret,
                    required: false,
                    placeholder_key: "provider-placeholder-bearer-token",
                    default_value: None,
                    options: Vec::new(),
                },
            ],
            model_listing: ModelListingStrategy::OllamaTagsAndShow,
        },
        api_key_provider(
            "openrouter",
            "OpenRouter",
            "provider-description-openrouter",
        ),
        api_key_provider("deepseek", "DeepSeek", "provider-description-deepseek"),
        api_key_provider("moonshot", "Moonshot/Kimi", "provider-description-moonshot"),
        api_key_provider("zai", "Z.AI", "provider-description-zai"),
        api_key_provider(
            "azure_openai",
            "Azure OpenAI",
            "provider-description-azure-openai",
        ),
        api_key_provider("mistral", "Mistral", "provider-description-mistral"),
        api_key_provider("xai", "xAI", "provider-description-xai"),
        api_key_provider("groq", "Groq", "provider-description-groq"),
        api_key_provider(
            "perplexity",
            "Perplexity",
            "provider-description-perplexity",
        ),
        api_key_provider("together", "Together", "provider-description-together"),
        custom_openai_provider(),
    ]
}

fn api_key_provider(
    kind: &'static str,
    display_name: &'static str,
    description: &'static str,
) -> ProviderSpec {
    ProviderSpec {
        kind: kind.into(),
        display_name,
        description_key: description,
        icon: IconName::Cloud,
        fields: vec![
            ProviderFieldSchema {
                key: "api_key",
                label_key: "provider-field-api-key",
                kind: ProviderFieldKind::Secret,
                required: true,
                placeholder_key: "provider-placeholder-api-key",
                default_value: None,
                options: Vec::new(),
            },
            ProviderFieldSchema {
                key: "base_url",
                label_key: "provider-field-base-url",
                kind: ProviderFieldKind::Url,
                required: false,
                placeholder_key: "provider-placeholder-base-url-default",
                default_value: None,
                options: Vec::new(),
            },
        ],
        model_listing: if kind == "azure_openai"
            || kind == "moonshot"
            || kind == "zai"
            || kind == "xai"
            || kind == "groq"
            || kind == "perplexity"
            || kind == "together"
        {
            ModelListingStrategy::Manual
        } else {
            ModelListingStrategy::RigModels
        },
    }
}

fn custom_openai_provider() -> ProviderSpec {
    ProviderSpec {
        kind: "custom_openai_compatible".into(),
        display_name: "Custom OpenAI-compatible",
        description_key: "provider-description-custom-openai-compatible",
        icon: IconName::Server,
        fields: vec![
            ProviderFieldSchema {
                key: "name",
                label_key: "provider-field-name",
                kind: ProviderFieldKind::Text,
                required: true,
                placeholder_key: "provider-placeholder-provider-name",
                default_value: None,
                options: Vec::new(),
            },
            ProviderFieldSchema {
                key: "api_key",
                label_key: "provider-field-api-key",
                kind: ProviderFieldKind::Secret,
                required: true,
                placeholder_key: "provider-placeholder-api-key",
                default_value: None,
                options: Vec::new(),
            },
            ProviderFieldSchema {
                key: "base_url",
                label_key: "provider-field-base-url",
                kind: ProviderFieldKind::Url,
                required: true,
                placeholder_key: "provider-placeholder-custom-base-url",
                default_value: None,
                options: Vec::new(),
            },
            ProviderFieldSchema {
                key: "api_mode",
                label_key: "provider-field-api-mode",
                kind: ProviderFieldKind::Select,
                required: true,
                placeholder_key: "provider-placeholder-api-mode",
                default_value: Some("responses"),
                options: vec![
                    ProviderSelectOption {
                        label_key: "provider-api-mode-responses",
                        value: "responses".to_string(),
                    },
                    ProviderSelectOption {
                        label_key: "provider-api-mode-chat-completions",
                        value: "chat_completions".to_string(),
                    },
                ],
            },
        ],
        model_listing: ModelListingStrategy::Manual,
    }
}
