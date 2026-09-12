use app_assets::{AppAssets, define_lucide_icons, define_svg_icons};
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;
define_lucide_icons!(pub(crate) enum IconName {
    Sparkles => "sparkles",
    UserRound => "user-round", Bot => "bot",
    Settings => "settings", ArrowLeft => "arrow-left", ArrowRight => "arrow-right",
    Check => "check",
    RotateCw => "rotate-cw", FolderOpen => "folder-open", Save => "save", Search => "search",
    Plus => "plus", SquarePen => "square-pen", PanelRight => "panel-right",
    ArrowUp => "arrow-up", ArrowDown => "arrow-down", Database => "database", CircleDashed => "circle-dashed", Square => "square", ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left", ChevronRight => "chevron-right", ChevronUp => "chevron-up", GitBranch => "git-branch", Copy => "copy",
    Ellipsis => "ellipsis", CircleAlert => "circle-alert", MessageCircle => "message-circle",
    List => "list", Tag => "tag",
    ZoomIn => "zoom-in", ZoomOut => "zoom-out", Scan => "scan", LocateFixed => "locate-fixed",
    RefreshCw => "refresh-cw", FileText => "file-text",
    X => "x", Folder => "folder",
    Terminal => "terminal", BookOpen => "book-open", FilePenLine => "file-pen-line",
    FilePlus => "file-plus", Wrench => "wrench", Brain => "brain", ChartNoAxesColumn => "chart-no-axes-column",
});
define_svg_icons!(
    #[asset_source(ProviderLogoAssets)]
    pub(crate) enum ProviderLogoName {
        #[svg("provider-icons/openai.svg", source = "thesvg", slug = "openai")]
        OpenAI,
        #[svg(
            "provider-icons/anthropic.svg",
            source = "simple-icons",
            slug = "anthropic"
        )]
        Anthropic,
        #[svg(
            "provider-icons/google-gemini.svg",
            source = "simple-icons",
            slug = "googlegemini"
        )]
        GoogleGemini,
        #[svg("provider-icons/ollama.svg", source = "simple-icons", slug = "ollama")]
        Ollama,
        #[svg(
            "provider-icons/openrouter.svg",
            source = "simple-icons",
            slug = "openrouter"
        )]
        OpenRouter,
        #[svg(
            "provider-icons/deepseek.svg",
            source = "simple-icons",
            slug = "deepseek"
        )]
        DeepSeek,
        #[svg(
            "provider-icons/moonshot.svg",
            source = "simple-icons",
            slug = "moonshotai"
        )]
        Moonshot,
        #[svg("provider-icons/zai.svg", source = "wikimedia", slug = "z-ai")]
        Zai,
        #[svg(
            "provider-icons/azure-openai.svg",
            source = "thesvg",
            slug = "azure-azure-openai"
        )]
        AzureOpenAI,
        #[svg(
            "provider-icons/mistral.svg",
            source = "simple-icons",
            slug = "mistralai"
        )]
        Mistral,
        #[svg("provider-icons/xai.svg", source = "thesvg", slug = "xai-grok")]
        Xai,
        #[svg("provider-icons/groq.svg", source = "thesvg", slug = "groq")]
        Groq,
        #[svg(
            "provider-icons/perplexity.svg",
            source = "simple-icons",
            slug = "perplexity"
        )]
        Perplexity,
        #[svg(
            "provider-icons/together.svg",
            source = "official-together",
            slug = "together-ai-logo-suite"
        )]
        Together,
    }
);

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
            _ => match ProviderLogoAssets.load(path)? {
                Some(asset) => Ok(Some(asset)),
                None => LucideAssets.load(path),
            },
        }
    }
    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut items = LucideAssets.list(path)?;
        items.extend(ProviderLogoAssets.list(path)?);
        items.extend(
            [LOGO_BLACK, LOGO_WHITE]
                .into_iter()
                .filter(|name| name.starts_with(path))
                .map(SharedString::from),
        );
        Ok(items)
    }
}
pub(crate) type Assets = AppAssets<BrandAssets>;
