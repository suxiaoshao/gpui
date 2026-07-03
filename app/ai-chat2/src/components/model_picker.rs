use crate::{
    components::picker::PickerSection,
    foundation::{
        I18n,
        assets::{ProviderVisual, provider_visual_for_kind, provider_visual_icon},
    },
    state::providers::{ProviderModelChoice, ProviderModelKey},
};
use ai_chat_core::ModelCapabilitiesSnapshot;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt, h_flex,
    label::Label,
    select::{SearchableVec, SelectGroup, SelectItem},
    tag::Tag,
    v_flex,
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

    fn render_row(&self, cx: &mut App) -> AnyElement {
        let capability_labels = capability_tag_labels(&self.capabilities, cx.global::<I18n>());

        h_flex()
            .min_w_0()
            .items_start()
            .gap_2()
            .child(
                provider_visual_icon(self.visual)
                    .size_4()
                    .flex_none()
                    .mt(px(1.))
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .gap_0p5()
                    .child(Label::new(self.display_name()).text_sm().font_medium())
                    .when(!capability_labels.is_empty(), |this| {
                        this.child(capability_tags(capability_labels))
                    }),
            )
            .into_any_element()
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
        self.display_name()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            h_flex()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(provider_visual_icon(self.visual).size_4())
                .child(
                    Label::new(self.title())
                        .text_sm()
                        .whitespace_nowrap()
                        .truncate(),
                )
                .into_any_element(),
        )
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.render_row(cx)
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
    grouped_model_options(choices)
        .into_iter()
        .map(|(provider, items)| PickerSection::section(provider, items))
        .collect()
}

pub(crate) fn model_select_groups(
    choices: &[ProviderModelChoice],
) -> SearchableVec<SelectGroup<ModelOption>> {
    SearchableVec::new(
        grouped_model_options(choices)
            .into_iter()
            .map(|(provider, items)| SelectGroup::new(provider).items(items))
            .collect::<Vec<_>>(),
    )
}

fn grouped_model_options(choices: &[ProviderModelChoice]) -> Vec<(SharedString, Vec<ModelOption>)> {
    let mut sections = Vec::new();
    let mut provider: Option<SharedString> = None;
    let mut items = Vec::new();

    for choice in choices {
        let provider_name: SharedString = choice.provider_display_name.clone().into();
        if provider.as_ref() != Some(&provider_name) {
            if let Some(provider) = provider.take() {
                sections.push((provider, items));
                items = Vec::new();
            }
            provider = Some(provider_name);
        }

        items.push(ModelOption::new(choice));
    }

    if let Some(provider) = provider {
        sections.push((provider, items));
    }

    sections
}

pub(crate) fn provider_visual_for_model_choice(choice: &ProviderModelChoice) -> ProviderVisual {
    provider_visual_for_kind(&choice.provider_kind)
}

fn capability_tags(labels: Vec<SharedString>) -> AnyElement {
    h_flex()
        .gap_1()
        .children(
            labels
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
    if capabilities.file_input.is_some() {
        labels.push(i18n.t("capability-file").into());
    }
    if capabilities.audio_input {
        labels.push(i18n.t("capability-audio").into());
    }
    if capabilities.image_generation {
        labels.push(i18n.t("capability-image-generation").into());
    }
    if capabilities.hosted_web_search {
        labels.push(i18n.t("capability-web-search").into());
    }
    if capabilities.remote_mcp {
        labels.push(i18n.t("capability-mcp").into());
    }
    if capabilities.structured_output {
        labels.push(i18n.t("capability-structured").into());
    }
    if capabilities.stateful_response_continuation {
        labels.push(i18n.t("capability-continuation").into());
    }
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
    if capabilities.file_input.is_some() {
        tokens.push("file input files 文件");
    }
    if capabilities.audio_input {
        tokens.push("audio input 音频");
    }
    if capabilities.image_generation {
        tokens.push("image generation 生成图片 生图");
    }
    if capabilities.hosted_web_search {
        tokens.push("web search 搜索");
    }
    if capabilities.remote_mcp {
        tokens.push("mcp remote mcp");
    }
    if capabilities.structured_output {
        tokens.push("structured output 结构化输出");
    }
    if capabilities.stateful_response_continuation {
        tokens.push("stateful continuation response continuation 续接");
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::{
        capability_tag_labels, model_sections, model_select_groups,
        provider_visual_for_model_choice,
    };
    use crate::{
        foundation::{I18n, assets::ProviderLogoName},
        state::providers::ProviderModelChoice,
    };
    use ai_chat_core::{
        CapabilitySourceSnapshot, FileInputCapabilitySnapshot, ImageInputCapabilitySnapshot,
        ModelCapabilitiesSnapshot, ReasoningCapabilitySnapshot, ReasoningControlSnapshot,
        conservative_model_capabilities,
    };
    use gpui_component::select::{SelectDelegate, SelectItem};

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
    fn model_select_groups_reuse_model_options() {
        let choices = vec![
            choice("provider-1", "openai", "OpenAI", "gpt-5", Some("GPT Five")),
            choice("provider-2", "ollama", "Ollama", "llama3.2", None),
        ];
        let groups = model_select_groups(&choices);

        assert_eq!(groups.items_count(0), 1);
        assert_eq!(groups.items_count(1), 1);
        assert_eq!(
            groups.position(&choices[1].key()),
            Some(gpui_component::IndexPath::default().section(1).row(0))
        );
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
    fn model_option_title_uses_model_without_provider_prefix() {
        let choices = vec![choice(
            "provider-1",
            "openai",
            "OpenAI",
            "gpt-5",
            Some("GPT Five"),
        )];
        let sections = model_sections(&choices);
        let option = sections[0].items[0].as_ref();

        assert_eq!(option.title().as_ref(), "GPT Five");
        assert!(option.matches("openai"));
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
    fn capability_tags_include_distinctive_model_capabilities() {
        let i18n = I18n::english_for_test();
        let labels = capability_tag_labels(&capabilities_with_distinctive_features(), &i18n);
        let labels = labels
            .iter()
            .map(|label| label.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "reasoning",
                "tools",
                "vision",
                "files",
                "audio",
                "image generation",
                "web search",
                "MCP",
                "structured",
                "continuation",
            ]
        );
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
            "capability-file",
            "capability-audio",
            "capability-image-generation",
            "capability-web-search",
            "capability-mcp",
            "capability-structured",
            "capability-continuation",
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

    fn capabilities_with_distinctive_features() -> ModelCapabilitiesSnapshot {
        let mut capabilities = capabilities_with_reasoning();
        capabilities.image_input = Some(ImageInputCapabilitySnapshot {
            max_images: Some(4),
        });
        capabilities.file_input = Some(FileInputCapabilitySnapshot { max_files: Some(8) });
        capabilities.audio_input = true;
        capabilities.image_generation = true;
        capabilities.hosted_web_search = true;
        capabilities.remote_mcp = true;
        capabilities.stateful_response_continuation = true;
        capabilities
    }
}
