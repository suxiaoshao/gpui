use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AppLanguage {
    #[serde(rename = "en-US", alias = "en")]
    English,
    #[serde(rename = "zh-CN", alias = "zh")]
    Chinese,
    #[default]
    #[serde(other, rename = "system")]
    System,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppThemeMode {
    Light,
    Dark,
    #[default]
    #[serde(other)]
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppThemeSettings {
    #[serde(default)]
    pub mode: AppThemeMode,
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
    #[serde(default)]
    pub custom_theme_colors: Vec<String>,
}

impl Default for AppThemeSettings {
    fn default() -> Self {
        Self {
            mode: AppThemeMode::System,
            light_theme: None,
            dark_theme: None,
            custom_theme_colors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettingsPayload {
    #[serde(default)]
    pub language: AppLanguage,
    #[serde(default)]
    pub theme: AppThemeSettings,
    #[serde(default)]
    pub temporary_hotkey: Option<String>,
    #[serde(default)]
    pub http_proxy: Option<String>,
    #[serde(default)]
    pub default_project_id: Option<ProjectId>,
}

impl Default for AppSettingsPayload {
    fn default() -> Self {
        Self {
            language: AppLanguage::System,
            theme: AppThemeSettings::default(),
            temporary_hotkey: None,
            http_proxy: None,
            default_project_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRawPayload {
    pub provider_kind: String,
    pub value: serde_json::Value,
}
