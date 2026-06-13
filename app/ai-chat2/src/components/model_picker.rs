use crate::{
    components::picker::PickerSection,
    foundation::{
        I18n,
        assets::{ProviderVisual, provider_visual_for_kind, provider_visual_icon},
    },
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use ai_chat_core::ModelCapabilitiesSnapshot;
use gpui::*;
use gpui_component::{
    ActiveTheme, Sizable, StyledExt, h_flex, label::Label, select::SelectItem, tag::Tag, v_flex,
};

#[derive(Clone, Debug)]
pub(crate) struct ModelOption {
    key: ProviderModelKey,
    provider_kind: String,
    provider_display_name: SharedString,
    model_id: SharedString,
    model_display_name: Option<SharedString>,
    capabilities: ModelCapabilitiesSnapshot,
    visual: ProviderVisual,
}

impl ModelOption {
    pub(crate) fn new(choice: &ProviderModelChoice) -> Self {
        Self {
            key: choice.key(),
            provider_kind: choice.provider_kind.clone(),
            provider_display_name: choice.provider_display_name.clone().into(),
            model_id: choice.model_id.clone().into(),
            model_display_name: choice.model_display_name.clone().map(Into::into),
            capabilities: choice.capabilities.clone(),
            visual: provider_visual_for_model_choice(choice),
        }
    }

    fn display_name(&self) -> SharedString {
        self.model_display_name
            .clone()
            .unwrap_or_else(|| self.model_id.clone())
    }

    pub(crate) fn key(&self) -> ProviderModelKey {
        self.key.clone()
    }
}

impl SelectItem for ModelOption {
    type Value = ProviderModelKey;

    fn title(&self) -> SharedString {
        format!("{} / {}", self.provider_display_name, self.display_name()).into()
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .gap_2()
            .child(
                provider_visual_icon(self.visual)
                    .size_4()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        Label::new(self.display_name())
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .child(
                        Label::new(format!(
                            "{} · {}",
                            self.provider_display_name.as_ref(),
                            self.model_id.as_ref()
                        ))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate(),
                    ),
            )
            .child(capability_tags(&self.capabilities, cx.global::<I18n>()))
    }

    fn value(&self) -> &Self::Value {
        &self.key
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        let search_text = format!(
            "{} {} {} {} {}",
            self.provider_display_name,
            self.provider_kind,
            self.model_id,
            self.display_name(),
            capability_search_tokens(&self.capabilities).join(" ")
        )
        .to_lowercase();
        search_text.contains(&query)
    }
}

pub(crate) fn model_sections(choices: &[ProviderModelChoice]) -> Vec<PickerSection<ModelOption>> {
    let mut sections = Vec::new();
    let mut provider: Option<SharedString> = None;
    let mut items = Vec::new();

    for choice in choices {
        let provider_name: SharedString = choice.provider_display_name.clone().into();
        if provider.as_ref() != Some(&provider_name) {
            if let Some(provider) = provider.take() {
                sections.push(PickerSection::section(provider, items));
                items = Vec::new();
            }
            provider = Some(provider_name);
        }

        items.push(ModelOption::new(choice));
    }

    if let Some(provider) = provider {
        sections.push(PickerSection::section(provider, items));
    }

    sections
}

pub(crate) fn provider_visual_for_model_choice(choice: &ProviderModelChoice) -> ProviderVisual {
    provider_visual_for_kind(&choice.provider_kind)
}

fn capability_tags(capabilities: &ModelCapabilitiesSnapshot, i18n: &I18n) -> AnyElement {
    h_flex()
        .gap_1()
        .children(
            capability_tag_labels(capabilities, i18n)
                .into_iter()
                .map(|label| Tag::secondary().small().child(label)),
        )
        .into_any_element()
}

fn capability_tag_labels(
    capabilities: &ModelCapabilitiesSnapshot,
    i18n: &I18n,
) -> Vec<SharedString> {
    let mut labels = Vec::new();
    if capabilities.reasoning.is_some() {
        labels.push(i18n.t("capability-reasoning").into());
    }
    if capabilities.tool_calling.is_some() {
        labels.push(i18n.t("capability-tools").into());
    }
    if capabilities.image_input.is_some() {
        labels.push(i18n.t("capability-vision").into());
    }
    if capabilities.structured_output {
        labels.push(i18n.t("capability-structured").into());
    }
    labels.truncate(3);
    labels
}

fn capability_search_tokens(capabilities: &ModelCapabilitiesSnapshot) -> Vec<&'static str> {
    let mut tokens = Vec::new();
    if capabilities.reasoning.is_some() {
        tokens.push("reasoning 推理");
    }
    if capabilities.tool_calling.is_some() {
        tokens.push("tools tool calling 工具");
    }
    if capabilities.image_input.is_some() {
        tokens.push("vision image input 视觉 图片");
    }
    if capabilities.structured_output {
        tokens.push("structured output 结构化输出");
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::{capability_tag_labels, model_sections, provider_visual_for_model_choice};
    use crate::{
        foundation::{I18n, assets::ProviderLogoName},
        state::providers::ProviderModelChoice,
    };
    use ai_chat_core::{
        CapabilitySourceSnapshot, ModelCapabilitiesSnapshot, ReasoningCapabilitySnapshot,
        ReasoningControlSnapshot, conservative_model_capabilities,
    };
    use gpui_component::select::SelectItem;

    #[test]
    fn model_sections_group_choices_by_provider() {
        let choices = vec![
            choice("provider-1", "openai", "OpenAI", "gpt-5", Some("GPT Five")),
            choice("provider-1", "openai", "OpenAI", "gpt-4.1", None),
            choice("provider-2", "ollama", "Ollama", "llama3.2", None),
        ];

        let sections = model_sections(&choices);

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].items.len(), 2);
        assert_eq!(sections[1].items.len(), 1);
        assert_eq!(sections[0].items[0].value().model_id, "gpt-5");
        assert_eq!(sections[1].items[0].value().provider_id, "provider-2");
    }

    #[test]
    fn model_option_matches_provider_model_and_capabilities() {
        let choices = vec![choice(
            "provider-1",
            "openai",
            "OpenAI",
            "gpt-5",
            Some("GPT Five"),
        )];
        let sections = model_sections(&choices);
        let option = sections[0].items[0].as_ref();

        assert!(option.matches("openai"));
        assert!(option.matches("five"));
        assert!(option.matches("gpt-5"));
        assert!(option.matches("tools"));
    }

    #[test]
    fn model_option_tracks_provider_visual() {
        let choices = vec![choice("provider-1", "openai", "OpenAI", "gpt-5", None)];
        let sections = model_sections(&choices);
        let option = sections[0].items[0].as_ref();

        assert_eq!(option.visual.logo, Some(ProviderLogoName::OpenAI));
    }

    #[test]
    fn selected_model_trigger_visual_tracks_provider_visual() {
        let choice = choice(
            "provider-1",
            "together",
            "Together",
            "deepseek-ai/DeepSeek-V3",
            None,
        );

        assert_eq!(
            provider_visual_for_model_choice(&choice).logo,
            Some(ProviderLogoName::Together)
        );
    }

    #[test]
    fn capability_tags_are_limited_to_three_labels() {
        let i18n = I18n::english_for_test();
        let labels = capability_tag_labels(&capabilities_with_reasoning(), &i18n);

        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].as_ref(), "reasoning");
    }

    #[test]
    fn model_picker_i18n_keys_are_present() {
        let locales = [I18n::english_for_test(), I18n::for_locale_tag("zh-CN")];
        let keys = [
            "chat-form-model-none-configured",
            "chat-form-model-load-failed",
            "chat-form-configure-providers",
            "capability-reasoning",
            "capability-tools",
            "capability-vision",
            "capability-structured",
        ];

        for i18n in locales {
            for key in keys {
                assert_ne!(i18n.t(key), key, "missing model picker i18n key {key}");
            }
        }
    }

    fn choice(
        provider_id: &str,
        provider_kind: &str,
        provider_display_name: &str,
        model_id: &str,
        model_display_name: Option<&str>,
    ) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider_id.to_string(),
            provider_kind: provider_kind.to_string(),
            provider_display_name: provider_display_name.to_string(),
            model_id: model_id.to_string(),
            model_display_name: model_display_name.map(ToString::to_string),
            capabilities: conservative_model_capabilities(provider_kind),
        }
    }

    fn capabilities_with_reasoning() -> ModelCapabilitiesSnapshot {
        let mut capabilities = conservative_model_capabilities("openai");
        capabilities.reasoning = Some(ReasoningCapabilitySnapshot {
            default_effort: "medium".to_string(),
            efforts: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            summaries: true,
            control: Some(ReasoningControlSnapshot::Levels {
                values: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
                default_value: Some("medium".to_string()),
            }),
            source: CapabilitySourceSnapshot::Manual {
                source: "test".to_string(),
            },
        });
        capabilities
    }
}
