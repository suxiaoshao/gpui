/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-05-01 10:33:44
 * @FilePath: /tauri/packages/ChatGPT/src-tauri/src/plugins/config/config_data.rs
 */
use crate::{
    app::APP_NAME,
    errors::{AiChatError, AiChatResult},
    hotkey::GlobalHotkeyState,
    llm::{
        OllamaProvider, OllamaSettings, OpenAIProvider, OpenAISettings, Provider, provider_names,
    },
    state::theme as app_theme,
};
use gpui::*;
use gpui_component::{ThemeConfig, ThemeMode as ComponentThemeMode, ThemeRegistry};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, fmt::Display, io::ErrorKind, path::PathBuf, rc::Rc};
use toml::Value;
use tracing::{Level, event};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ThemeMode {
    Dark,
    Light,
    #[default]
    #[serde(other)]
    System,
}

impl Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeMode::Dark => write!(f, "dark"),
            ThemeMode::Light => write!(f, "light"),
            ThemeMode::System => write!(f, "system"),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ThemeOption {
    #[serde(default = "Default::default")]
    theme: ThemeMode,
    #[serde(rename = "lightTheme")]
    light_theme: String,
    #[serde(rename = "darkTheme")]
    dark_theme: String,
    #[serde(rename = "customThemeColors")]
    custom_theme_colors: Vec<String>,
}

impl Default for ThemeOption {
    fn default() -> Self {
        Self {
            theme: Default::default(),
            light_theme: app_theme::DEFAULT_LIGHT_THEME_ID.to_string(),
            dark_theme: app_theme::DEFAULT_DARK_THEME_ID.to_string(),
            custom_theme_colors: vec![app_theme::DEFAULT_CUSTOM_THEME_COLOR.to_string()],
        }
    }
}

impl<'de> Deserialize<'de> for ThemeOption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(default)]
        struct RawThemeOption {
            theme: ThemeMode,
            #[serde(rename = "lightTheme")]
            light_theme: Option<String>,
            #[serde(rename = "darkTheme")]
            dark_theme: Option<String>,
            #[serde(rename = "customThemeColors")]
            custom_theme_colors: Option<Vec<String>>,
            #[serde(rename = "customColor", alias = "color")]
            custom_color: Option<String>,
        }

        impl Default for RawThemeOption {
            fn default() -> Self {
                Self {
                    theme: ThemeMode::System,
                    light_theme: None,
                    dark_theme: None,
                    custom_theme_colors: None,
                    custom_color: None,
                }
            }
        }

        let raw = RawThemeOption::deserialize(deserializer)?;
        let custom_theme_colors_missing = raw.custom_theme_colors.is_none();
        let mut custom_theme_colors =
            normalize_custom_theme_colors(raw.custom_theme_colors.unwrap_or_default().into_iter());
        if let Some(color) = raw.custom_color
            && let Some(color) = app_theme::normalize_hex_color(&color)
        {
            append_custom_theme_color(&mut custom_theme_colors, color);
        }

        let light_theme = raw
            .light_theme
            .map(|theme| app_theme::normalize_theme_id(&theme))
            .unwrap_or_else(|| app_theme::DEFAULT_LIGHT_THEME_ID.to_string());
        let dark_theme = raw
            .dark_theme
            .map(|theme| app_theme::normalize_theme_id(&theme))
            .unwrap_or_else(|| app_theme::DEFAULT_DARK_THEME_ID.to_string());
        for theme_id in [&light_theme, &dark_theme] {
            if let Some(color) = app_theme::material_you_color_from_id(theme_id) {
                append_custom_theme_color(&mut custom_theme_colors, color);
            }
        }
        if custom_theme_colors.is_empty() && custom_theme_colors_missing {
            custom_theme_colors.push(app_theme::DEFAULT_CUSTOM_THEME_COLOR.to_string());
        }

        Ok(Self {
            theme: raw.theme,
            light_theme,
            dark_theme,
            custom_theme_colors,
        })
    }
}

fn normalize_custom_theme_colors(colors: impl Iterator<Item = String>) -> Vec<String> {
    colors
        .filter_map(|color| app_theme::normalize_hex_color(&color))
        .fold(Vec::new(), |mut colors, color| {
            append_custom_theme_color(&mut colors, color);
            colors
        })
}

fn append_custom_theme_color(colors: &mut Vec<String>, color: String) {
    if !colors.contains(&color) {
        colors.push(color);
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh")]
    Chinese,
    #[default]
    #[serde(other)]
    System,
}

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::English => write!(f, "en"),
            Self::Chinese => write!(f, "zh"),
            Self::System => write!(f, "system"),
        }
    }
}

impl Language {
    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "en" => Self::English,
            "zh" => Self::Chinese,
            "system" => Self::System,
            _ => Self::System,
        }
    }

    pub(crate) fn options() -> [Self; 3] {
        [Self::System, Self::English, Self::Chinese]
    }

    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Self::System => "language-system",
            Self::English => "language-english",
            Self::Chinese => "language-chinese",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AiChatConfig {
    #[serde(default = "Default::default")]
    theme: ThemeOption,
    #[serde(default = "Default::default")]
    language: Language,
    #[serde(rename = "httpProxy")]
    pub http_proxy: Option<String>,
    #[serde(rename = "temporaryHotkey")]
    pub temporary_hotkey: Option<String>,
    #[serde(rename = "providerSettings", alias = "adapterSettings", default)]
    provider_settings: HashMap<String, toml::Value>,
}

impl Default for AiChatConfig {
    fn default() -> Self {
        let mut provider_settings = HashMap::new();
        if let Ok(settings) = Value::try_from(OllamaSettings::default()) {
            provider_settings.insert(OllamaProvider.name().to_string(), settings);
        }
        if let Ok(settings) = Value::try_from(OpenAISettings::default()) {
            provider_settings.insert(OpenAIProvider.name().to_string(), settings);
        }
        Self {
            theme: Default::default(),
            language: Default::default(),
            http_proxy: Default::default(),
            temporary_hotkey: Default::default(),
            provider_settings,
        }
    }
}

impl Global for AiChatConfig {}

impl AiChatConfig {
    fn normalize_provider_settings(&mut self) {
        let current_openai = self.provider_settings.remove(OpenAIProvider.name());
        let legacy_stream = self.provider_settings.remove("OpenAI Stream");
        let legacy_openai = self.provider_settings.remove("OpenAI");
        let selected = current_openai.or(legacy_stream).or(legacy_openai);
        self.provider_settings
            .retain(|name, _| provider_names().contains(&name.as_str()));
        let normalized = selected
            .and_then(|value| value.try_into::<OpenAISettings>().ok())
            .map(OpenAISettings::normalized)
            .unwrap_or_default();
        if let Ok(settings) = Value::try_from(normalized) {
            self.provider_settings
                .insert(OpenAIProvider.name().to_string(), settings);
        }
        let ollama = self
            .provider_settings
            .remove(OllamaProvider.name())
            .and_then(|value| value.try_into::<OllamaSettings>().ok())
            .map(OllamaSettings::normalized)
            .unwrap_or_default();
        if let Ok(settings) = Value::try_from(ollama) {
            self.provider_settings
                .insert(OllamaProvider.name().to_string(), settings);
        }
    }

    pub fn path() -> AiChatResult<PathBuf> {
        let file = dirs_next::config_dir()
            .ok_or(AiChatError::DbPath)?
            .join(APP_NAME);
        if !file.exists() {
            std::fs::create_dir_all(&file)?;
        }
        let file = file.join(CONFIG_FILE_NAME);
        Ok(file)
    }
    pub fn save(&self) -> AiChatResult<()> {
        let config_path = Self::path()?;
        let mut config = self.clone();
        config.normalize_provider_settings();
        let config_str = toml::to_string_pretty(&config)?;
        std::fs::write(config_path, config_str)?;
        Ok(())
    }
    pub fn get() -> AiChatResult<Self> {
        //data path
        let config_path = AiChatConfig::path()?;
        let config = match std::fs::read_to_string(&config_path) {
            Ok(file) => match toml::from_str(&file) {
                Ok(config) => config,
                Err(_) => {
                    let config = Self::default();
                    let config_str = toml::to_string_pretty(&config)?;
                    std::fs::write(&config_path, config_str)?;
                    config
                }
            },
            Err(e) => {
                if let ErrorKind::NotFound = e.kind() {
                    let config = Self::default();
                    let config_str = toml::to_string_pretty(&config)?;
                    std::fs::write(&config_path, config_str)?;
                    config
                } else {
                    return Err(e.into());
                }
            }
        };
        let mut config = config;
        config.normalize_provider_settings();
        Ok(config)
    }
    pub(crate) fn get_provider_settings(&self, provider: &str) -> Option<&toml::Value> {
        self.provider_settings.get(provider)
    }
    pub(crate) fn set_provider_settings(&mut self, provider: &str, settings: toml::Value) {
        self.provider_settings
            .insert(provider.to_string(), settings);
    }
    pub(crate) fn get_http_proxy(&self) -> Option<&str> {
        self.http_proxy.as_deref()
    }
    pub(crate) fn model_settings_fingerprint(&self) -> AiChatResult<String> {
        #[derive(Serialize)]
        struct Fingerprint<'a> {
            http_proxy: &'a Option<String>,
            provider_settings: &'a HashMap<String, toml::Value>,
        }

        let mut config = self.clone();
        config.normalize_provider_settings();
        Ok(serde_json::to_string(&Fingerprint {
            http_proxy: &config.http_proxy,
            provider_settings: &config.provider_settings,
        })?)
    }
    pub(crate) fn gpui_theme(
        &self,
        theme_registry: &ThemeRegistry,
        window: &mut Window,
    ) -> Rc<ThemeConfig> {
        let mode = self.resolved_component_theme_mode(window.appearance());
        app_theme::resolve_theme_config(
            theme_registry,
            mode,
            self.theme_id_for_component_mode(mode),
            &self.theme.custom_theme_colors,
        )
    }
    pub(crate) fn theme_mode(&self) -> ThemeMode {
        self.theme.theme
    }
    pub(crate) fn resolved_component_theme_mode(
        &self,
        appearance: WindowAppearance,
    ) -> ComponentThemeMode {
        match (appearance, self.theme.theme) {
            (_, ThemeMode::Light)
            | (WindowAppearance::Light | WindowAppearance::VibrantLight, ThemeMode::System) => {
                ComponentThemeMode::Light
            }
            (_, ThemeMode::Dark)
            | (WindowAppearance::Dark | WindowAppearance::VibrantDark, ThemeMode::System) => {
                ComponentThemeMode::Dark
            }
        }
    }
    pub(crate) fn theme_id_for_component_mode(&self, mode: ComponentThemeMode) -> &str {
        match mode {
            ComponentThemeMode::Light => &self.theme.light_theme,
            ComponentThemeMode::Dark => &self.theme.dark_theme,
        }
    }
    pub(crate) fn light_theme_id(&self) -> &str {
        &self.theme.light_theme
    }
    pub(crate) fn dark_theme_id(&self) -> &str {
        &self.theme.dark_theme
    }
    pub(crate) fn custom_theme_colors(&self) -> &[String] {
        &self.theme.custom_theme_colors
    }
    pub(crate) fn language(&self) -> Language {
        self.language
    }
    pub(crate) fn set_language(&mut self, language: Language) {
        self.language = language;
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save language: {}", err);
            }
        }
    }
    pub(crate) fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.theme.theme = mode;
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save theme mode: {}", err);
            }
        }
    }
    pub(crate) fn set_light_theme_id(&mut self, theme_id: impl Into<String>) {
        let theme_id = app_theme::normalize_theme_id(&theme_id.into());
        self.add_custom_color_from_theme_id(&theme_id);
        self.theme.light_theme = theme_id;
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save light theme: {}", err);
            }
        }
    }
    pub(crate) fn set_dark_theme_id(&mut self, theme_id: impl Into<String>) {
        let theme_id = app_theme::normalize_theme_id(&theme_id.into());
        self.add_custom_color_from_theme_id(&theme_id);
        self.theme.dark_theme = theme_id;
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save dark theme: {}", err);
            }
        }
    }
    pub(crate) fn add_custom_theme_color(&mut self, color: &str) -> Option<String> {
        let color = app_theme::normalize_hex_color(color)?;
        if !self.theme.custom_theme_colors.contains(&color) {
            self.theme.custom_theme_colors.push(color.clone());
        }
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save custom theme color: {}", err);
            }
        }
        app_theme::material_you_theme_id(&color)
    }
    pub(crate) fn delete_custom_theme_color(&mut self, theme_id_or_color: &str) -> bool {
        let changed = self.remove_custom_theme_color(theme_id_or_color);
        if changed {
            match self.save() {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, "Failed to save custom theme colors: {}", err);
                }
            }
        }
        changed
    }
    fn remove_custom_theme_color(&mut self, theme_id_or_color: &str) -> bool {
        let Some(color) = app_theme::material_you_color_from_id(theme_id_or_color)
            .or_else(|| app_theme::normalize_hex_color(theme_id_or_color))
        else {
            return false;
        };
        let before_len = self.theme.custom_theme_colors.len();
        self.theme
            .custom_theme_colors
            .retain(|existing| existing != &color);
        if self.theme.custom_theme_colors.len() == before_len {
            return false;
        }

        let Some(theme_id) = app_theme::material_you_theme_id(&color) else {
            return false;
        };
        if self.theme.light_theme == theme_id {
            self.theme.light_theme = app_theme::DEFAULT_LIGHT_THEME_ID.to_string();
        }
        if self.theme.dark_theme == theme_id {
            self.theme.dark_theme = app_theme::DEFAULT_DARK_THEME_ID.to_string();
        }
        true
    }
    fn add_custom_color_from_theme_id(&mut self, theme_id: &str) {
        let Some(color) = app_theme::material_you_color_from_id(theme_id) else {
            return;
        };
        if !self.theme.custom_theme_colors.contains(&color) {
            self.theme.custom_theme_colors.push(color);
        }
    }
    pub(crate) fn set_http_proxy(&mut self, proxy: Option<String>) {
        self.http_proxy = proxy;
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save HTTP proxy: {}", err);
            }
        }
    }
    pub(crate) fn set_temporary_hotkey(&mut self, hotkey: Option<String>, cx: &mut App) {
        if let Err(err) = GlobalHotkeyState::update_temporary_hotkey(
            self.temporary_hotkey.as_deref(),
            hotkey.as_deref(),
            cx,
        ) {
            event!(Level::ERROR, "Failed to update temporary hotkey: {}", err);
            return;
        }
        self.temporary_hotkey = hotkey;
        match self.save() {
            Ok(_) => {}
            Err(err) => {
                event!(Level::ERROR, "Failed to save temporary hotkey: {}", err);
            }
        }
    }
}

pub fn init(cx: &mut App) {
    let config = AiChatConfig::get().unwrap_or_default();
    cx.set_global(config);
}

#[cfg(test)]
mod tests {
    use super::{AiChatConfig, Language, ThemeMode};
    use crate::state::theme;
    use gpui::WindowAppearance;
    use gpui_component::ThemeMode as ComponentThemeMode;

    #[test]
    fn unknown_language_deserializes_to_system_without_dropping_settings() -> anyhow::Result<()> {
        let config: AiChatConfig = toml::from_str(
            r#"
language = "ja"
httpProxy = "http://127.0.0.1:8080"

[providerSettings.OpenAI]
apiKey = "sk-test"
"#,
        )?;

        assert_eq!(config.language(), Language::System);
        assert_eq!(config.get_http_proxy(), Some("http://127.0.0.1:8080"));
        assert_eq!(
            config
                .get_provider_settings("OpenAI")
                .and_then(|settings| settings.get("apiKey"))
                .and_then(toml::Value::as_str),
            Some("sk-test")
        );

        Ok(())
    }

    #[test]
    fn legacy_theme_color_deserializes_to_custom_theme_color() -> anyhow::Result<()> {
        let config: AiChatConfig = toml::from_str(
            r##"
[theme]
theme = "dark"
color = "#123456"
"##,
        )?;

        assert_eq!(config.theme_mode(), ThemeMode::Dark);
        assert_eq!(config.custom_theme_colors(), &["#123456".to_string()]);
        assert_eq!(config.light_theme_id(), theme::DEFAULT_LIGHT_THEME_ID);
        assert_eq!(config.dark_theme_id(), theme::DEFAULT_DARK_THEME_ID);

        Ok(())
    }

    #[test]
    fn missing_theme_names_default_to_gpui_component_defaults() -> anyhow::Result<()> {
        let config: AiChatConfig = toml::from_str(
            r#"
[theme]
theme = "system"
"#,
        )?;

        assert_eq!(config.light_theme_id(), theme::DEFAULT_LIGHT_THEME_ID);
        assert_eq!(config.dark_theme_id(), theme::DEFAULT_DARK_THEME_ID);
        assert_eq!(
            config.custom_theme_colors(),
            &[theme::DEFAULT_CUSTOM_THEME_COLOR.to_string()]
        );

        Ok(())
    }

    #[test]
    fn explicit_empty_custom_theme_colors_stays_empty() -> anyhow::Result<()> {
        let config: AiChatConfig = toml::from_str(
            r#"
[theme]
customThemeColors = []
"#,
        )?;

        assert!(config.custom_theme_colors().is_empty());

        Ok(())
    }

    #[test]
    fn theme_selection_serializes_to_toml() -> anyhow::Result<()> {
        let config: AiChatConfig = toml::from_str(
            r##"
[theme]
theme = "system"
lightTheme = "Ayu Light"
darkTheme = "material-you:#123456"
customThemeColors = ["#123456"]
"##,
        )?;

        let toml = toml::to_string(&config)?;

        assert!(toml.contains("lightTheme = \"preset:Ayu Light\""));
        assert!(toml.contains("darkTheme = \"material-you:#123456\""));
        assert!(toml.contains("\"#123456\""));

        Ok(())
    }

    #[test]
    fn selected_material_themes_deserialize_into_custom_theme_colors() -> anyhow::Result<()> {
        let config: AiChatConfig = toml::from_str(
            r##"
[theme]
theme = "system"
lightTheme = "material-you:#aabbcc"
darkTheme = "material-you:#00aa00"
"##,
        )?;

        assert_eq!(config.light_theme_id(), "material-you:#AABBCC");
        assert_eq!(config.dark_theme_id(), "material-you:#00AA00");
        assert_eq!(
            config.custom_theme_colors(),
            &["#AABBCC".to_string(), "#00AA00".to_string()]
        );

        Ok(())
    }

    #[test]
    fn remove_custom_theme_color_removes_unselected_material_theme() -> anyhow::Result<()> {
        let mut config: AiChatConfig = toml::from_str(
            r##"
[theme]
theme = "system"
lightTheme = "preset:Default Light"
darkTheme = "preset:Default Dark"
customThemeColors = ["#111111", "#222222"]
"##,
        )?;

        assert!(config.remove_custom_theme_color("#111111"));
        assert!(!config.remove_custom_theme_color("#111111"));
        assert_eq!(config.custom_theme_colors(), &["#222222".to_string()]);
        assert_eq!(config.light_theme_id(), theme::DEFAULT_LIGHT_THEME_ID);
        assert_eq!(config.dark_theme_id(), theme::DEFAULT_DARK_THEME_ID);

        Ok(())
    }

    #[test]
    fn remove_custom_theme_color_resets_selected_material_themes() -> anyhow::Result<()> {
        let mut config: AiChatConfig = toml::from_str(
            r##"
[theme]
theme = "system"
lightTheme = "material-you:#123456"
darkTheme = "material-you:#123456"
customThemeColors = ["#123456", "#654321"]
"##,
        )?;

        assert!(config.remove_custom_theme_color("material-you:#123456"));
        assert_eq!(config.custom_theme_colors(), &["#654321".to_string()]);
        assert_eq!(config.light_theme_id(), theme::DEFAULT_LIGHT_THEME_ID);
        assert_eq!(config.dark_theme_id(), theme::DEFAULT_DARK_THEME_ID);

        Ok(())
    }

    #[test]
    fn remove_custom_theme_color_resets_canonicalized_material_theme_ids() -> anyhow::Result<()> {
        let mut config: AiChatConfig = toml::from_str(
            r##"
[theme]
theme = "system"
lightTheme = "material-you:#aabbcc"
darkTheme = "material-you:aabbcc"
customThemeColors = ["#aabbcc", "#654321"]
"##,
        )?;

        assert_eq!(config.light_theme_id(), "material-you:#AABBCC");
        assert_eq!(config.dark_theme_id(), "material-you:#AABBCC");

        assert!(config.remove_custom_theme_color("material-you:#aabbcc"));
        assert_eq!(config.custom_theme_colors(), &["#654321".to_string()]);
        assert_eq!(config.light_theme_id(), theme::DEFAULT_LIGHT_THEME_ID);
        assert_eq!(config.dark_theme_id(), theme::DEFAULT_DARK_THEME_ID);

        Ok(())
    }

    #[test]
    fn configured_mode_resolves_current_component_mode() {
        let light_config: AiChatConfig = toml::from_str(
            r#"
[theme]
theme = "light"
"#,
        )
        .expect("valid light config");
        let dark_config: AiChatConfig = toml::from_str(
            r#"
[theme]
theme = "dark"
"#,
        )
        .expect("valid dark config");
        let system_config: AiChatConfig = toml::from_str(
            r#"
[theme]
theme = "system"
"#,
        )
        .expect("valid system config");

        assert_eq!(
            light_config.resolved_component_theme_mode(WindowAppearance::Dark),
            ComponentThemeMode::Light
        );

        assert_eq!(
            dark_config.resolved_component_theme_mode(WindowAppearance::Light),
            ComponentThemeMode::Dark
        );

        assert_eq!(
            system_config.resolved_component_theme_mode(WindowAppearance::VibrantDark),
            ComponentThemeMode::Dark
        );
    }
}
