use gpui_kit::{AssetSource, SharedString};
pub(crate) use gpui_lucide::IconName;
use gpui_lucide::SvgIcon;
use std::borrow::Cow;

struct ProviderLogoName;
#[allow(non_upper_case_globals)]
impl ProviderLogoName {
    const OpenAI: SvgIcon = SvgIcon::new(include_bytes!("../../assets/provider-icons/openai.svg"));
    const Anthropic: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/anthropic.svg"));
    const GoogleGemini: SvgIcon = SvgIcon::new(include_bytes!(
        "../../assets/provider-icons/google-gemini.svg"
    ));
    const Ollama: SvgIcon = SvgIcon::new(include_bytes!("../../assets/provider-icons/ollama.svg"));
    const OpenRouter: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/openrouter.svg"));
    const DeepSeek: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/deepseek.svg"));
    const Moonshot: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/moonshot.svg"));
    const Zai: SvgIcon = SvgIcon::new(include_bytes!("../../assets/provider-icons/zai.svg"));
    const AzureOpenAI: SvgIcon = SvgIcon::new(include_bytes!(
        "../../assets/provider-icons/azure-openai.svg"
    ));
    const Mistral: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/mistral.svg"));
    const Xai: SvgIcon = SvgIcon::new(include_bytes!("../../assets/provider-icons/xai.svg"));
    const Groq: SvgIcon = SvgIcon::new(include_bytes!("../../assets/provider-icons/groq.svg"));
    const Perplexity: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/perplexity.svg"));
    const Together: SvgIcon =
        SvgIcon::new(include_bytes!("../../assets/provider-icons/together.svg"));
}

pub(crate) fn provider_icon(provider: &str) -> gpui_kit::component::Icon {
    provider_logo_icon(provider)
        .unwrap_or_else(|| gpui_kit::component::Icon::new(IconName::Sparkles))
}

pub(crate) fn provider_logo_icon(provider: &str) -> Option<gpui_kit::component::Icon> {
    use gpui_kit::component::Icon;
    let logo = match provider {
        "openai" | "openai-codex" => ProviderLogoName::OpenAI,
        "anthropic" => ProviderLogoName::Anthropic,
        "google" | "google-vertex" => ProviderLogoName::GoogleGemini,
        "ollama" => ProviderLogoName::Ollama,
        "openrouter" => ProviderLogoName::OpenRouter,
        "deepseek" => ProviderLogoName::DeepSeek,
        "moonshotai" | "moonshotai-cn" | "kimi-coding" => ProviderLogoName::Moonshot,
        "zai" | "zai-coding-cn" => ProviderLogoName::Zai,
        "azure-openai-responses" => ProviderLogoName::AzureOpenAI,
        "mistral" => ProviderLogoName::Mistral,
        "xai" => ProviderLogoName::Xai,
        "groq" => ProviderLogoName::Groq,
        "perplexity" => ProviderLogoName::Perplexity,
        "together" => ProviderLogoName::Together,
        _ => return None,
    };
    Some(Icon::new(logo))
}

const LOGO_BLACK: &str = "brand/logo-black.svg";
const LOGO_WHITE: &str = "brand/logo-white.svg";
pub(crate) fn app_logo(cx: &gpui_kit::App) -> &'static str {
    use gpui_kit::component::ActiveTheme;
    let icon = crate::state::icons::current(cx);
    if icon != crate::state::icons::IconTheme::Classic {
        return icon.logo();
    }
    if cx.theme().is_dark() {
        LOGO_WHITE
    } else {
        LOGO_BLACK
    }
}
#[derive(Default)]
pub(crate) struct BrandAssets;
impl AssetSource for BrandAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match path {
            LOGO_BLACK => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/logo-black.svg"
            )))),
            LOGO_WHITE => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/logo-white.svg"
            )))),
            "brand/icon-classic.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-classic.png"
            )))),
            "brand/icon-classic-gradient.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-classic-gradient.png"
            )))),
            "brand/icon-classic-gradient.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-classic-gradient.svg"
            )))),
            "brand/icon-color.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-color.png"
            )))),
            "brand/icon-color.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-color.svg"
            )))),
            "brand/icon-color-gradient.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-color-gradient.png"
            )))),
            "brand/icon-color-gradient.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-color-gradient.svg"
            )))),
            "brand/icon-pride.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-pride.png"
            )))),
            "brand/icon-pride.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-pride.svg"
            )))),
            "brand/icon-ukraine.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-ukraine.png"
            )))),
            "brand/icon-ukraine.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-ukraine.svg"
            )))),
            "brand/icon-ukraine-gradient.png" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-ukraine-gradient.png"
            )))),
            "brand/icon-ukraine-gradient.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/brand/icon-ukraine-gradient.svg"
            )))),
            _ => gpui_kit::assets::Assets.load(path),
        }
    }
    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut items = gpui_kit::assets::Assets.list(path)?;
        items.extend(
            [
                LOGO_BLACK,
                LOGO_WHITE,
                "brand/icon-classic.png",
                "brand/icon-classic-gradient.png",
                "brand/icon-classic-gradient.svg",
                "brand/icon-color.png",
                "brand/icon-color.svg",
                "brand/icon-color-gradient.png",
                "brand/icon-color-gradient.svg",
                "brand/icon-pride.png",
                "brand/icon-pride.svg",
                "brand/icon-ukraine.png",
                "brand/icon-ukraine.svg",
                "brand/icon-ukraine-gradient.png",
                "brand/icon-ukraine-gradient.svg",
            ]
            .into_iter()
            .filter(|name| name.starts_with(path))
            .map(SharedString::from),
        );
        Ok(items)
    }
}
pub(crate) type Assets = BrandAssets;
