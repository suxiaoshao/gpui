use gpui::{App, AppContext};
use gpui_store::{
    SharedStore, StoreBackend, StoreBackendFuture, StoreBackendId, StoreCommitAck,
    StoreCommitBackend, StoreState,
};
use jaco_core::{
    AppLanguage, AppSettingsPayload, AppThemeMode, AppThemeSettings, ProjectId, ProviderId,
    ProviderModelId, ReasoningSelectionSnapshot, ToolApprovalMode, default_tool_approval_mode,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tracing::{Level, event};

use crate::{
    app::APP_NAME,
    errors::{JacoError, JacoResult},
};

mod mcp;

#[cfg(test)]
pub(crate) use mcp::McpToolApprovalMode;
pub(crate) use mcp::{
    McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind, delete_mcp_server,
    is_reserved_mcp_header, is_valid_mcp_env_var_name, is_valid_mcp_server_id,
    set_mcp_server_enabled, upsert_mcp_server,
};

const CONFIG_FILE_NAME: &str = "config.toml";
pub(crate) const CONFIG_DIR_ENV: &str = "JACO_CONFIG_DIR";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct JacoConfig {
    pub(crate) storage: StorageConfig,
    pub(crate) app_settings: AppSettingsConfig,
    pub(crate) chat_form: ChatFormConfig,
    pub(crate) mcp_servers: BTreeMap<String, McpServerTomlConfig>,
    #[serde(skip)]
    load_error: Option<JacoConfigLoadError>,
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

impl StoreState for JacoConfig {}

pub(crate) type JacoConfigStore = SharedStore<JacoConfig, JacoConfigBackend>;

#[derive(Clone, Debug)]
pub(crate) struct JacoConfigBackend {
    path: PathBuf,
}

impl JacoConfigBackend {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JacoConfigLoadError {
    path: PathBuf,
    message: String,
}

impl JacoConfigLoadError {
    fn new(path: PathBuf, error: toml::de::Error) -> Self {
        Self {
            path,
            message: error.to_string(),
        }
    }

    pub(crate) fn path_display(&self) -> String {
        self.path.display().to_string()
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    fn save_blocked_message(&self) -> String {
        format!(
            "config.toml is invalid; fix {} before saving settings: {}",
            self.path.display(),
            self.message
        )
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct StorageConfig {
    pub(crate) data_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct AppSettingsConfig {
    pub(crate) language: AppLanguage,
    pub(crate) theme: AppThemeConfig,
    pub(crate) temporary_hotkey: Option<String>,
    pub(crate) http_proxy: Option<String>,
    pub(crate) default_project_id: Option<ProjectId>,
}

impl Default for AppSettingsConfig {
    fn default() -> Self {
        Self::from(AppSettingsPayload::default())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct AppThemeConfig {
    pub(crate) mode: AppThemeMode,
    pub(crate) light_theme: Option<String>,
    pub(crate) dark_theme: Option<String>,
    pub(crate) custom_theme_colors: Vec<String>,
}

impl Default for AppThemeConfig {
    fn default() -> Self {
        Self::from(AppThemeSettings::default())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct ChatFormConfig {
    pub(crate) model: Option<ChatFormModelConfig>,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    #[serde(default = "default_tool_approval_mode")]
    pub(crate) approval_mode: ToolApprovalMode,
}

impl Default for ChatFormConfig {
    fn default() -> Self {
        Self {
            model: None,
            reasoning_selection: None,
            approval_mode: default_tool_approval_mode(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ChatFormModelConfig {
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JacoAppSettings {
    payload: AppSettingsPayload,
}

impl PartialEq for JacoConfig {
    fn eq(&self, other: &Self) -> bool {
        self.storage == other.storage
            && self.app_settings == other.app_settings
            && self.chat_form == other.chat_form
            && self.mcp_servers == other.mcp_servers
            && self.load_error == other.load_error
    }
}

impl StoreBackend<JacoConfig> for JacoConfigBackend {
    type Snapshot = JacoConfig;
    type Event = ();
    type Subscription = ();
    type Error = JacoError;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new(format!("file:{}", self.path.display()))
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(Some(JacoConfig::load_or_create_from_path(&self.path)?))
    }

    fn reconcile(&self, state: &mut JacoConfig, snapshot: Self::Snapshot) -> bool {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    }
}

impl StoreCommitBackend<JacoConfig> for JacoConfigBackend {
    fn commit_snapshot(
        &self,
        draft: &JacoConfig,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Self::Snapshot>>, Self::Error> {
        if let Some(load_error) = draft.load_error.as_ref() {
            return Err(JacoError::Config(load_error.save_blocked_message()));
        }

        draft.save_to_path(&self.path)?;
        Ok(Some(StoreCommitAck::with_snapshot(draft.clone())))
    }
}

impl JacoConfig {
    fn load_or_create_from_path(path: &Path) -> JacoResult<Self> {
        match fs::read_to_string(path) {
            Ok(source) => match toml::from_str::<Self>(&source) {
                Ok(mut config) => {
                    config.config_path = Some(path.to_path_buf());
                    Ok(config)
                }
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "parse jaco config.toml failed");
                    Ok(Self {
                        load_error: Some(JacoConfigLoadError::new(path.to_path_buf(), err)),
                        config_path: Some(path.to_path_buf()),
                        ..Default::default()
                    })
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let config = Self {
                    config_path: Some(path.to_path_buf()),
                    ..Default::default()
                };
                config.save_to_path(path)?;
                Ok(config)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) fn path() -> JacoResult<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE_NAME))
    }

    pub(crate) fn config_dir() -> JacoResult<PathBuf> {
        let dir = match override_dir_from_env(CONFIG_DIR_ENV) {
            Some(dir) => dir,
            None => dirs_next::config_dir()
                .ok_or(JacoError::ConfigDirUnavailable)?
                .join(APP_NAME),
        };
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub(crate) fn data_dir(&self) -> JacoResult<PathBuf> {
        match self.storage.data_dir.as_ref() {
            Some(path) => Ok(path.clone()),
            None => Self::config_dir(),
        }
    }

    pub(crate) fn app_settings_payload(&self) -> AppSettingsPayload {
        self.app_settings.payload()
    }

    #[cfg(test)]
    fn save(&self) -> JacoResult<()> {
        let path = match self.config_path.as_ref() {
            Some(path) => path.clone(),
            None => Self::path()?,
        };
        self.save_to_path(&path)
    }

    fn save_to_path(&self, path: &Path) -> JacoResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn with_app_settings_for_test(
        config_path: PathBuf,
        payload: AppSettingsPayload,
    ) -> Self {
        Self {
            app_settings: AppSettingsConfig::from(payload),
            config_path: Some(config_path),
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn load_from_path_for_test(path: &Path) -> JacoResult<Self> {
        Self::load_or_create_from_path(path)
    }

    #[cfg(test)]
    pub(crate) fn save_for_test(&self) -> JacoResult<()> {
        self.save()
    }
}

impl JacoAppSettings {
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

    pub(crate) fn http_proxy(&self) -> Option<&str> {
        self.payload.http_proxy.as_deref()
    }

    pub(crate) fn default_project_id(&self) -> Option<&ProjectId> {
        self.payload.default_project_id.as_ref()
    }
}

fn override_dir_from_env(name: &str) -> Option<PathBuf> {
    override_dir_from_value(std::env::var_os(name))
}

fn override_dir_from_value(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

pub(crate) fn store(cx: &impl AppContext) -> JacoConfigStore {
    JacoConfigStore::global(cx)
}

pub(crate) fn read<R>(cx: &impl AppContext, f: impl FnOnce(&JacoConfig) -> R) -> R {
    store(cx).read(cx, f)
}

pub(crate) fn data_dir(cx: &impl AppContext) -> JacoResult<PathBuf> {
    read(cx, JacoConfig::data_dir)
}

pub(crate) fn app_settings(cx: &impl AppContext) -> JacoAppSettings {
    read(cx, |config| {
        JacoAppSettings::new(config.app_settings_payload())
    })
}

pub(crate) fn config_load_error(cx: &impl AppContext) -> Option<JacoConfigLoadError> {
    read(cx, |config| config.load_error.clone())
}

impl AppSettingsConfig {
    fn payload(&self) -> AppSettingsPayload {
        AppSettingsPayload {
            language: self.language,
            theme: self.theme.settings(),
            temporary_hotkey: self.temporary_hotkey.clone(),
            http_proxy: self.http_proxy.clone(),
            default_project_id: self.default_project_id.clone(),
        }
    }
}

impl From<AppSettingsPayload> for AppSettingsConfig {
    fn from(payload: AppSettingsPayload) -> Self {
        Self {
            language: payload.language,
            theme: AppThemeConfig::from(payload.theme),
            temporary_hotkey: payload.temporary_hotkey,
            http_proxy: payload.http_proxy,
            default_project_id: payload.default_project_id,
        }
    }
}

impl AppThemeConfig {
    fn settings(&self) -> AppThemeSettings {
        AppThemeSettings {
            mode: self.mode,
            light_theme: self.light_theme.clone(),
            dark_theme: self.dark_theme.clone(),
            custom_theme_colors: self.custom_theme_colors.clone(),
        }
    }
}

impl From<AppThemeSettings> for AppThemeConfig {
    fn from(settings: AppThemeSettings) -> Self {
        Self {
            mode: settings.mode,
            light_theme: settings.light_theme,
            dark_theme: settings.dark_theme,
            custom_theme_colors: settings.custom_theme_colors,
        }
    }
}

pub(crate) fn update_app_settings(
    cx: &mut App,
    update: impl FnOnce(&mut AppSettingsPayload),
) -> JacoResult<AppSettingsPayload> {
    let config_store = store(cx);
    let mut next_payload = config_store.read(cx, JacoConfig::app_settings_payload);
    let store_update = config_store.try_update(cx, |config| {
        let mut payload = config.app_settings_payload();
        update(&mut payload);
        next_payload = payload.clone();
        config.app_settings = AppSettingsConfig::from(payload);
    })?;

    if store_update.changed_state() {
        cx.refresh_windows();
    }

    Ok(next_payload)
}

#[cfg(test)]
pub(crate) fn update_chat_form_config(
    cx: &mut App,
    update: impl FnOnce(&mut ChatFormConfig),
) -> JacoResult<ChatFormConfig> {
    let config_store = store(cx);
    let mut next_chat_form = config_store.read_cloned(cx, |config| &config.chat_form);
    config_store.try_update_field(
        cx,
        |config| &mut config.chat_form,
        |chat_form| {
            update(chat_form);
            next_chat_form = chat_form.clone();
        },
    )?;
    Ok(next_chat_form)
}

pub(crate) fn init(cx: &mut App) -> JacoResult<()> {
    let path = JacoConfig::path()?;
    let config_store = JacoConfigStore::install_global_with_backend(
        cx,
        JacoConfig::default(),
        JacoConfigBackend::new(path),
    )?;
    let data_dir = data_dir(cx)?;
    let enabled_mcp_servers = match config_store.read(cx, JacoConfig::mcp_config_layer) {
        Ok(layer) => layer.servers.len(),
        Err(err) => {
            event!(Level::ERROR, error = ?err, "parse jaco MCP config failed");
            0
        }
    };
    event!(
        Level::INFO,
        data_dir = ?data_dir,
        enabled_mcp_servers,
        "loaded jaco config"
    );
    Ok(())
}

pub(crate) fn init_app_settings(cx: &mut App) -> JacoResult<()> {
    let settings = app_settings(cx);
    event!(
        Level::INFO,
        language = ?settings.language(),
        theme = ?settings.theme().mode,
        temporary_hotkey = ?settings.temporary_hotkey(),
        http_proxy = ?settings.http_proxy(),
        default_project_id = ?settings.default_project_id(),
        "loaded jaco app settings"
    );
    Ok(())
}

#[cfg(test)]
pub(crate) fn install_for_test(cx: &mut App, config: JacoConfig) -> JacoResult<()> {
    let path = match config.config_path.clone() {
        Some(path) => path,
        None => JacoConfig::path()?,
    };
    JacoConfigStore::install_global_with_backend(cx, config, JacoConfigBackend::new(path))?;
    Ok(())
}

#[cfg(test)]
mod tests;
