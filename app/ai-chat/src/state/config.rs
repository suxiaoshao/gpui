/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-05-01 10:33:44
 * @FilePath: /tauri/packages/ChatGPT/src-tauri/src/plugins/config/config_data.rs
 */
use crate::{
    APP_NAME,
    errors::{AiChatError, AiChatResult},
    hotkey::GlobalHotkeyState,
    llm::{
        OllamaProvider, OllamaSettings, OpenAIProvider, OpenAISettings, Provider, provider_names,
    },
};
use gpui::*;
use gpui_component::{ThemeConfig, ThemeRegistry};
use serde::{Deserialize, Serialize};
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

impl ThemeMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            "system" => ThemeMode::System,
            _ => ThemeMode::System,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct ThemeOption {
    theme: ThemeMode,
    color: String,
}

impl Default for ThemeOption {
    fn default() -> Self {
        Self {
            theme: Default::default(),
            color: "#3271ae".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh")]
    Chinese,
    #[default]
    System,
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
        let appearance = window.appearance();
        let theme = match (appearance, self.theme.theme) {
            (_, ThemeMode::Light)
            | (WindowAppearance::Light | WindowAppearance::VibrantLight, ThemeMode::System) => {
                theme_registry.default_light_theme()
            }
            (_, ThemeMode::Dark)
            | (WindowAppearance::Dark | WindowAppearance::VibrantDark, ThemeMode::System) => {
                theme_registry.default_dark_theme()
            }
        };
        Rc::clone(theme)
    }
    pub(crate) fn theme_mode(&self) -> ThemeMode {
        self.theme.theme
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
        if let Err(err) =
            GlobalHotkeyState::update_temporary_hotkey(
                self.temporary_hotkey.as_deref(),
                hotkey.as_deref(),
                cx,
            )
        {
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
