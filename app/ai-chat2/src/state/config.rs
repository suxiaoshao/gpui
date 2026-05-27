use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use ai_chat_core::{AppLanguage, AppSettingsPayload, AppThemeSettings, ProjectId};
use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use tracing::{Level, event};

use crate::{
    app::APP_NAME,
    database,
    errors::{AiChat2Error, AiChat2Result},
};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct AiChat2Config {
    pub(crate) storage: StorageConfig,
}

impl Global for AiChat2Config {}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct StorageConfig {
    pub(crate) data_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiChat2AppSettings {
    payload: AppSettingsPayload,
}

impl Global for AiChat2AppSettings {}

impl AiChat2Config {
    pub(crate) fn load_or_create() -> AiChat2Result<Self> {
        let path = Self::path()?;
        match fs::read_to_string(&path) {
            Ok(source) => Ok(toml::from_str(&source)?),
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let config = Self::default();
                config.save_to_path(&path)?;
                Ok(config)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) fn path() -> AiChat2Result<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE_NAME))
    }

    pub(crate) fn config_dir() -> AiChat2Result<PathBuf> {
        let dir = dirs_next::config_dir()
            .ok_or(AiChat2Error::ConfigDirUnavailable)?
            .join(APP_NAME);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub(crate) fn data_dir(&self) -> AiChat2Result<PathBuf> {
        match self.storage.data_dir.as_ref() {
            Some(path) => Ok(path.clone()),
            None => Self::config_dir(),
        }
    }

    fn save_to_path(&self, path: &Path) -> AiChat2Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

impl AiChat2AppSettings {
    pub(crate) fn new(payload: AppSettingsPayload) -> Self {
        Self { payload }
    }

    pub(crate) fn language(&self) -> AppLanguage {
        self.payload.language
    }

    pub(crate) fn theme(&self) -> &AppThemeSettings {
        &self.payload.theme
    }

    pub(crate) fn temporary_hotkey(&self) -> Option<&str> {
        self.payload.temporary_hotkey.as_deref()
    }

    pub(crate) fn default_project_id(&self) -> Option<&ProjectId> {
        self.payload.default_project_id.as_ref()
    }
}

pub(crate) fn init(cx: &mut App) -> AiChat2Result<()> {
    let config = AiChat2Config::load_or_create()?;
    event!(Level::INFO, data_dir = ?config.data_dir()?, "loaded ai-chat2 config");
    cx.set_global(config);
    Ok(())
}

pub(crate) fn init_app_settings(cx: &mut App) -> AiChat2Result<()> {
    let repository = database::repository(cx);
    let payload = match repository.get_app_settings()? {
        Some(record) => record.settings,
        None => {
            let payload = AppSettingsPayload::default();
            repository.set_app_settings(payload.clone())?;
            payload
        }
    };

    let settings = AiChat2AppSettings::new(payload);
    event!(
        Level::INFO,
        language = ?settings.language(),
        theme = ?settings.theme().mode,
        temporary_hotkey = ?settings.temporary_hotkey(),
        default_project_id = ?settings.default_project_id(),
        "loaded ai-chat2 app settings"
    );
    cx.set_global(settings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AiChat2AppSettings, AiChat2Config, StorageConfig};
    use ai_chat_core::{AppLanguage, AppSettingsPayload, AppThemeMode, AppThemeSettings};
    use std::path::PathBuf;

    #[test]
    fn toml_config_only_stores_machine_local_storage() {
        let config = AiChat2Config {
            storage: StorageConfig {
                data_dir: Some(PathBuf::from("/tmp/ai-chat2")),
            },
        };

        let serialized = toml::to_string(&config).unwrap();

        assert!(serialized.contains("[storage]"));
        assert!(serialized.contains("data_dir"));
        assert!(!serialized.contains("language"));
        assert!(!serialized.contains("theme"));
        assert_eq!(
            toml::from_str::<AiChat2Config>(&serialized).unwrap(),
            config
        );
    }

    #[test]
    fn app_settings_expose_typed_db_preferences() {
        let settings = AiChat2AppSettings::new(AppSettingsPayload {
            language: AppLanguage::Chinese,
            theme: AppThemeSettings {
                mode: AppThemeMode::Light,
                ..Default::default()
            },
            temporary_hotkey: Some("cmd+shift+j".to_string()),
            default_project_id: Some("project-1".to_string()),
        });

        assert_eq!(settings.language(), AppLanguage::Chinese);
        assert_eq!(settings.theme().mode, AppThemeMode::Light);
        assert_eq!(settings.temporary_hotkey(), Some("cmd+shift+j"));
        assert_eq!(
            settings.default_project_id().map(String::as_str),
            Some("project-1")
        );
    }
}
