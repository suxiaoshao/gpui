use crate::foundation::assets::{ProviderVisual, provider_visual_for_kind};

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
pub(super) struct ProviderSpec {
    pub(super) kind: ProviderKindKey,
    pub(super) display_name: &'static str,
    pub(super) description_key: &'static str,
    pub(super) visual: ProviderVisual,
    pub(super) form_kind: ProviderFormKind,
    pub(super) model_listing: ModelListingStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProviderFormKind {
    ApiKey,
    Ollama,
    CustomOpenAiCompatible,
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
            visual: provider_visual_for_kind("ollama"),
            form_kind: ProviderFormKind::Ollama,
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
        visual: provider_visual_for_kind(kind),
        form_kind: ProviderFormKind::ApiKey,
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
        visual: provider_visual_for_kind("custom_openai_compatible"),
        form_kind: ProviderFormKind::CustomOpenAiCompatible,
        model_listing: ModelListingStrategy::Manual,
    }
}
