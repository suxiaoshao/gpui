use app_assets::{define_lucide_icons, define_svg_icons};
use gpui::{AssetSource, SharedString};
use gpui_component::Icon;
use rust_embed::RustEmbed;
use std::{borrow::Cow, collections::BTreeSet};

pub(crate) const APP_ICON_ASSET_PATH: &str = "build-assets/icon/app-icon.png";

define_lucide_icons!(
    pub(crate) enum IconName {
        Check => "check",
        ChevronDown => "chevron-down",
        ChevronRight => "chevron-right",
        ChevronUp => "chevron-up",
        CircleAlert => "circle-alert",
        CircleCheck => "circle-check",
        Clipboard => "clipboard",
        Copy => "copy",
        Database => "database",
        Cloud => "cloud",
        Cpu => "cpu",
        File => "file",
        FilePen => "file-pen",
        FileSearch => "file-search",
        FileText => "file-text",
        Ellipsis => "ellipsis",
        Eye => "eye",
        ExternalLink => "external-link",
        Folder => "folder",
        FolderMinus => "folder-minus",
        FolderOpen => "folder-open",
        FolderPlus => "folder-plus",
        FolderX => "folder-x",
        Keyboard => "keyboard",
        KeyRound => "key-round",
        Languages => "languages",
        Link => "link",
        LogIn => "log-in",
        LogOut => "log-out",
        Lightbulb => "lightbulb",
        Palette => "palette",
        Pencil => "pencil",
        Pin => "pin",
        PinOff => "pin-off",
        Plus => "plus",
        Plug => "plug",
        RefreshCcw => "refresh-ccw",
        Search => "search",
        Send => "send",
        Server => "server",
        Settings => "settings",
        Shield => "shield",
        ShieldAlert => "shield-alert",
        ShieldCheck => "shield-check",
        MessageSquare => "message-square",
        Minus => "minus",
        Paperclip => "paperclip",
        Sparkles => "sparkles",
        Square => "square",
        SquarePen => "square-pen",
        Terminal => "terminal",
        Trash => "trash",
        Unlink => "unlink",
        Wrench => "wrench",
        X => "x",
    }
);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProviderVisual {
    pub(crate) logo: Option<ProviderLogoName>,
    pub(crate) fallback: IconName,
}

impl ProviderVisual {
    pub(crate) const fn logo(logo: ProviderLogoName, fallback: IconName) -> Self {
        Self {
            logo: Some(logo),
            fallback,
        }
    }

    pub(crate) const fn fallback(fallback: IconName) -> Self {
        Self {
            logo: None,
            fallback,
        }
    }
}

pub(crate) fn provider_visual_for_kind(kind: &str) -> ProviderVisual {
    match kind {
        "openai" => ProviderVisual::logo(ProviderLogoName::OpenAI, IconName::Cloud),
        "anthropic" => ProviderVisual::logo(ProviderLogoName::Anthropic, IconName::Cloud),
        "gemini" => ProviderVisual::logo(ProviderLogoName::GoogleGemini, IconName::Cloud),
        "ollama" => ProviderVisual::logo(ProviderLogoName::Ollama, IconName::Cpu),
        "openrouter" => ProviderVisual::logo(ProviderLogoName::OpenRouter, IconName::Cloud),
        "deepseek" => ProviderVisual::logo(ProviderLogoName::DeepSeek, IconName::Cloud),
        "moonshot" => ProviderVisual::logo(ProviderLogoName::Moonshot, IconName::Cloud),
        "zai" => ProviderVisual::logo(ProviderLogoName::Zai, IconName::Cloud),
        "azure_openai" => ProviderVisual::logo(ProviderLogoName::AzureOpenAI, IconName::Cloud),
        "mistral" => ProviderVisual::logo(ProviderLogoName::Mistral, IconName::Cloud),
        "xai" => ProviderVisual::logo(ProviderLogoName::Xai, IconName::Cloud),
        "groq" => ProviderVisual::logo(ProviderLogoName::Groq, IconName::Cloud),
        "perplexity" => ProviderVisual::logo(ProviderLogoName::Perplexity, IconName::Cloud),
        "together" => ProviderVisual::logo(ProviderLogoName::Together, IconName::Cloud),
        "custom_openai_compatible" => ProviderVisual::fallback(IconName::Server),
        "ollama_compatible" => ProviderVisual::fallback(IconName::Cpu),
        _ => ProviderVisual::fallback(IconName::Cloud),
    }
}

pub(crate) fn provider_visual_icon(visual: ProviderVisual) -> Icon {
    match visual.logo {
        Some(logo) => Icon::new(logo),
        None => Icon::new(visual.fallback),
    }
}

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "themes/**/*.json"]
struct AssetsInner;

#[derive(RustEmbed)]
#[folder = "."]
#[include = "build-assets/icon/app-icon.png"]
struct BuildAssets;

impl AssetSource for AssetsInner {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Ok(Self::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|item| {
                let item = item.into_owned();
                (path.is_empty() || item.starts_with(path)).then(|| item.into())
            })
            .collect())
    }
}

impl AssetSource for BuildAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Ok(Self::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|item| {
                let item = item.into_owned();
                (path.is_empty() || item.starts_with(path)).then(|| item.into())
            })
            .collect())
    }
}

pub(crate) struct Assets {
    assets: AssetsInner,
    build_assets: BuildAssets,
    provider_logo_assets: ProviderLogoAssets,
    lucide_assets: LucideAssets,
    component_assets: gpui_component_assets::Assets,
}

pub(crate) fn bundled_theme_sets() -> Vec<String> {
    AssetsInner::iter()
        .filter(|path| path.starts_with("themes/gpui-component/") && path.ends_with(".json"))
        .filter_map(|path| {
            AssetsInner::get(path.as_ref()).and_then(|file| {
                std::str::from_utf8(file.data.as_ref())
                    .ok()
                    .map(ToOwned::to_owned)
            })
        })
        .collect()
}

impl Default for Assets {
    fn default() -> Self {
        Self {
            assets: AssetsInner,
            build_assets: BuildAssets,
            provider_logo_assets: ProviderLogoAssets,
            lucide_assets: LucideAssets,
            component_assets: gpui_component_assets::Assets,
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        for source in [
            &self.assets as &dyn AssetSource,
            &self.build_assets as &dyn AssetSource,
            &self.provider_logo_assets as &dyn AssetSource,
            &self.lucide_assets as &dyn AssetSource,
        ] {
            if let Some(data) = source.load(path)? {
                return Ok(Some(data));
            }
        }

        self.component_assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut names = BTreeSet::new();

        names.extend(self.assets.list(path)?);
        names.extend(self.build_assets.list(path)?);
        names.extend(self.provider_logo_assets.list(path)?);
        names.extend(self.lucide_assets.list(path)?);
        names.extend(self.component_assets.list(path)?);

        Ok(names.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{APP_ICON_ASSET_PATH, Assets, IconName, ProviderLogoName, bundled_theme_sets};
    use app_assets::{SvgIconMetadata, SvgIconNamed};
    use gpui::{AssetSource, SharedString};
    use gpui_component::IconNamed;

    #[test]
    fn declared_icons_have_lucide_paths() {
        assert_eq!(
            IconName::Database.path(),
            SharedString::from("icons/database.svg")
        );
        assert_eq!(
            IconName::Keyboard.path(),
            SharedString::from("icons/keyboard.svg")
        );
        assert_eq!(
            IconName::Folder.path(),
            SharedString::from("icons/folder.svg")
        );
        assert_eq!(
            IconName::FolderOpen.path(),
            SharedString::from("icons/folder-open.svg")
        );
        assert_eq!(
            IconName::FolderPlus.path(),
            SharedString::from("icons/folder-plus.svg")
        );
        assert_eq!(
            IconName::FolderX.path(),
            SharedString::from("icons/folder-x.svg")
        );
        assert_eq!(
            IconName::Lightbulb.path(),
            SharedString::from("icons/lightbulb.svg")
        );
        assert_eq!(
            IconName::Search.path(),
            SharedString::from("icons/search.svg")
        );
        assert_eq!(IconName::Send.path(), SharedString::from("icons/send.svg"));
        assert_eq!(
            IconName::Shield.path(),
            SharedString::from("icons/shield.svg")
        );
        assert_eq!(
            IconName::ShieldCheck.path(),
            SharedString::from("icons/shield-check.svg")
        );
        assert_eq!(
            IconName::Square.path(),
            SharedString::from("icons/square.svg")
        );
        assert_eq!(
            IconName::Terminal.path(),
            SharedString::from("icons/terminal.svg")
        );
        assert_eq!(
            IconName::Trash.path(),
            SharedString::from("icons/trash.svg")
        );
    }

    #[test]
    fn assets_load_app_local_lucide_icons() {
        let assets = Assets::default();
        let icon = assets
            .load("icons/database.svg")
            .expect("load database icon")
            .expect("database icon exists");

        assert!(!icon.is_empty());
    }

    #[test]
    fn declared_provider_logos_have_paths_and_metadata() {
        assert_eq!(
            ProviderLogoName::OpenAI.path(),
            SharedString::from("provider-icons/openai.svg")
        );
        assert_eq!(
            ProviderLogoName::GoogleGemini.metadata(),
            SvgIconMetadata {
                source: "simple-icons",
                slug: Some("googlegemini")
            }
        );
        assert_eq!(
            ProviderLogoName::Together.metadata(),
            SvgIconMetadata {
                source: "official-together",
                slug: Some("together-ai-logo-suite")
            }
        );
    }

    #[test]
    fn assets_load_provider_logos() {
        let assets = Assets::default();
        for path in [
            "provider-icons/openai.svg",
            "provider-icons/anthropic.svg",
            "provider-icons/azure-openai.svg",
            "provider-icons/groq.svg",
            "provider-icons/zai.svg",
            "provider-icons/together.svg",
        ] {
            let icon = assets
                .load(path)
                .unwrap_or_else(|_| panic!("load provider logo {path}"))
                .unwrap_or_else(|| panic!("provider logo {path} exists"));

            assert!(!icon.is_empty(), "provider logo {path} is not empty");
        }
    }

    #[test]
    fn inverse_provider_marks_are_monochrome_positive_shapes() {
        let assets = Assets::default();

        for path in ["provider-icons/groq.svg", "provider-icons/zai.svg"] {
            let icon = assets
                .load(path)
                .unwrap_or_else(|_| panic!("load provider logo {path}"))
                .unwrap_or_else(|| panic!("provider logo {path} exists"));
            let icon = std::str::from_utf8(icon.as_ref())
                .unwrap_or_else(|_| panic!("provider logo {path} is valid utf-8"));

            assert!(
                icon.contains("currentColor"),
                "provider logo {path} follows Icon text-color semantics"
            );
            assert!(
                !icon.contains("#F54F35") && !icon.contains("#2D2D2D"),
                "provider logo {path} must not depend on inverse colored backgrounds"
            );
        }
    }

    #[test]
    fn provider_visuals_prefer_known_logos_and_keep_fallbacks() {
        let openai = super::provider_visual_for_kind("openai");
        assert_eq!(openai.logo, Some(ProviderLogoName::OpenAI));
        assert_eq!(openai.fallback, IconName::Cloud);

        let anthropic = super::provider_visual_for_kind("anthropic");
        assert_eq!(anthropic.logo, Some(ProviderLogoName::Anthropic));
        assert_eq!(anthropic.fallback, IconName::Cloud);

        let xai = super::provider_visual_for_kind("xai");
        assert_eq!(xai.logo, Some(ProviderLogoName::Xai));
        assert_eq!(xai.fallback, IconName::Cloud);

        let custom = super::provider_visual_for_kind("custom_openai_compatible");
        assert_eq!(custom.logo, None);
        assert_eq!(custom.fallback, IconName::Server);
    }

    #[test]
    fn assets_embed_bundled_theme_sets() {
        let theme_sets = bundled_theme_sets();

        assert!(theme_sets.len() >= 20);
        assert!(
            theme_sets
                .iter()
                .any(|theme_set| theme_set.contains("Ayu Light"))
        );
    }

    #[test]
    fn assets_list_app_local_lucide_icons() {
        let assets = Assets::default();
        let icons = assets.list("icons/").expect("list icons");

        assert!(icons.contains(&SharedString::from("icons/database.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder-open.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder-plus.svg")));
        assert!(icons.contains(&SharedString::from("icons/folder-x.svg")));
        assert!(icons.contains(&SharedString::from("icons/keyboard.svg")));
        assert!(icons.contains(&SharedString::from("icons/lightbulb.svg")));
        assert!(icons.contains(&SharedString::from("icons/send.svg")));
    }

    #[test]
    fn assets_list_provider_logos() {
        let assets = Assets::default();
        let icons = assets.list("provider-icons/").expect("list provider logos");

        assert!(icons.contains(&SharedString::from("provider-icons/openai.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/anthropic.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/google-gemini.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/azure-openai.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/xai.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/groq.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/together.svg")));
        assert!(icons.contains(&SharedString::from("provider-icons/zai.svg")));
    }

    #[test]
    fn app_icon_asset_loads() {
        let assets = Assets::default();
        let icon = assets
            .load(APP_ICON_ASSET_PATH)
            .expect("load app icon")
            .expect("app icon exists");

        assert!(!icon.is_empty());
    }
}
