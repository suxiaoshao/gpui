/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-05-01 10:33:44
 * @FilePath: /tauri/packages/ChatGPT/src-tauri/src/plugins/config/config_data.rs
 */
use crate::{
    APP_NAME,
    adapter::{Adapter, OpenAIAdapter, OpenAISettings, OpenAIStreamAdapter, OpenAIStreamSettings},
    errors::{AiChatError, AiChatResult},
    hotkey::TemporaryData,
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
    #[serde(rename = "adapterSettings", default)]
    adapter_settings: HashMap<String, toml::Value>,
}

impl Default for AiChatConfig {
    fn default() -> Self {
        let mut adapter_settings = HashMap::new();
        if let Ok(settings) = Value::try_from(OpenAISettings::default()) {
            adapter_settings.insert(OpenAIAdapter::NAME.to_string(), settings);
        }
        if let Ok(settings) = Value::try_from(OpenAIStreamSettings::default()) {
            adapter_settings.insert(OpenAIStreamAdapter::NAME.to_string(), settings);
        }
        Self {
            theme: Default::default(),
            language: Default::default(),
            http_proxy: Default::default(),
            temporary_hotkey: Default::default(),
            adapter_settings,
        }
    }
}

impl Global for AiChatConfig {}

impl AiChatConfig {
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
        let config_str = toml::to_string_pretty(&self)?;
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
        Ok(config)
    }
    pub(crate) fn get_adapter_settings(&self, adapter: &str) -> Option<&toml::Value> {
        self.adapter_settings.get(adapter)
    }
    pub(crate) fn get_adapter_settings_mut(&mut self, adapter: &str) -> Option<&mut toml::Value> {
        self.adapter_settings.get_mut(adapter)
    }
    pub(crate) fn set_adapter_settings(&mut self, adapter: &str, settings: toml::Value) {
        self.adapter_settings.insert(adapter.to_string(), settings);
    }
    pub(crate) fn get_http_proxy(&self) -> Option<&str> {
        self.http_proxy.as_deref()
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
            TemporaryData::update_hotkey(self.temporary_hotkey.as_deref(), hotkey.as_deref(), cx)
        {
            event!(Level::ERROR, "Failed to update temporary hotkey: {}", err);
        };
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
