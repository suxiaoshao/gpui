use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use ai_chat_agent::{
    McpConfigLayer, McpServerConfig, McpServerTransport, McpStdioTransport,
    McpStreamableHttpTransport,
};
use ai_chat_core::{
    AppLanguage, AppSettingsPayload, AppThemeMode, AppThemeSettings, ProjectId, ProviderId,
    ProviderModelId, ReasoningSelectionSnapshot, ToolApprovalMode, default_tool_approval_mode,
};
use gpui::{App, AppContext};
use gpui_store::{
    SharedStore, StoreBackend, StoreBackendFuture, StoreBackendId, StoreCommitAck,
    StoreCommitBackend, StoreState,
};
use serde::{Deserialize, Serialize};
use tracing::{Level, event};

use crate::{
    app::APP_NAME,
    errors::{AiChat2Error, AiChat2Result},
};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct AiChat2Config {
    pub(crate) storage: StorageConfig,
    pub(crate) app_settings: AppSettingsConfig,
    pub(crate) chat_form: ChatFormConfig,
    pub(crate) mcp_servers: BTreeMap<String, McpServerTomlConfig>,
    #[serde(skip)]
    load_error: Option<AiChat2ConfigLoadError>,
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

impl StoreState for AiChat2Config {}

pub(crate) type AiChat2ConfigStore = SharedStore<AiChat2Config, AiChat2ConfigBackend>;

#[derive(Clone, Debug)]
pub(crate) struct AiChat2ConfigBackend {
    path: PathBuf,
}

impl AiChat2ConfigBackend {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiChat2ConfigLoadError {
    path: PathBuf,
    message: String,
}

impl AiChat2ConfigLoadError {
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub(crate) struct McpServerTomlConfig {
    #[serde(default = "default_mcp_server_enabled")]
    pub(crate) enabled: bool,
    pub(crate) display_name: Option<String>,
    pub(crate) transport: McpTransportKind,
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) oauth: Option<toml::Value>,
    #[serde(default)]
    pub(crate) env: BTreeMap<String, String>,
    pub(crate) cwd: Option<PathBuf>,
}

impl Default for McpServerTomlConfig {
    fn default() -> Self {
        Self {
            enabled: default_mcp_server_enabled(),
            display_name: None,
            transport: McpTransportKind::Stdio,
            command: None,
            args: Vec::new(),
            url: None,
            headers: BTreeMap::new(),
            oauth: None,
            env: BTreeMap::new(),
            cwd: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum McpTransportKind {
    #[default]
    Stdio,
    StreamableHttp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiChat2AppSettings {
    payload: AppSettingsPayload,
}

impl PartialEq for AiChat2Config {
    fn eq(&self, other: &Self) -> bool {
        self.storage == other.storage
            && self.app_settings == other.app_settings
            && self.chat_form == other.chat_form
            && self.mcp_servers == other.mcp_servers
            && self.load_error == other.load_error
    }
}

impl StoreBackend<AiChat2Config> for AiChat2ConfigBackend {
    type Snapshot = AiChat2Config;
    type Event = ();
    type Subscription = ();
    type Error = AiChat2Error;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new(format!("file:{}", self.path.display()))
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(Some(AiChat2Config::load_or_create_from_path(&self.path)?))
    }

    fn reconcile(&self, state: &mut AiChat2Config, snapshot: Self::Snapshot) -> bool {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    }
}

impl StoreCommitBackend<AiChat2Config> for AiChat2ConfigBackend {
    fn commit_snapshot(
        &self,
        draft: &AiChat2Config,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Self::Snapshot>>, Self::Error> {
        if let Some(load_error) = draft.load_error.as_ref() {
            return Err(AiChat2Error::Config(load_error.save_blocked_message()));
        }

        draft.save_to_path(&self.path)?;
        Ok(Some(StoreCommitAck::with_snapshot(draft.clone())))
    }
}

impl AiChat2Config {
    fn load_or_create_from_path(path: &Path) -> AiChat2Result<Self> {
        match fs::read_to_string(path) {
            Ok(source) => match toml::from_str::<Self>(&source) {
                Ok(mut config) => {
                    config.config_path = Some(path.to_path_buf());
                    Ok(config)
                }
                Err(err) => {
                    event!(Level::ERROR, error = ?err, "parse ai-chat2 config.toml failed");
                    Ok(Self {
                        load_error: Some(AiChat2ConfigLoadError::new(path.to_path_buf(), err)),
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

    pub(crate) fn app_settings_payload(&self) -> AppSettingsPayload {
        self.app_settings.payload()
    }

    pub(crate) fn mcp_config_layer(&self) -> AiChat2Result<McpConfigLayer> {
        let mut servers = Vec::new();
        for (server_id, server) in &self.mcp_servers {
            if server.enabled {
                servers.push(server.to_agent_config(server_id)?);
            }
        }
        Ok(McpConfigLayer { servers })
    }

    #[cfg(test)]
    fn save(&self) -> AiChat2Result<()> {
        let path = match self.config_path.as_ref() {
            Some(path) => path.clone(),
            None => Self::path()?,
        };
        self.save_to_path(&path)
    }

    fn save_to_path(&self, path: &Path) -> AiChat2Result<()> {
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
    pub(crate) fn load_from_path_for_test(path: &Path) -> AiChat2Result<Self> {
        Self::load_or_create_from_path(path)
    }

    #[cfg(test)]
    pub(crate) fn save_for_test(&self) -> AiChat2Result<()> {
        self.save()
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

    pub(crate) fn http_proxy(&self) -> Option<&str> {
        self.payload.http_proxy.as_deref()
    }

    pub(crate) fn default_project_id(&self) -> Option<&ProjectId> {
        self.payload.default_project_id.as_ref()
    }
}

pub(crate) fn store(cx: &impl AppContext) -> AiChat2ConfigStore {
    AiChat2ConfigStore::global(cx)
}

pub(crate) fn read<R>(cx: &impl AppContext, f: impl FnOnce(&AiChat2Config) -> R) -> R {
    store(cx).read(cx, f)
}

pub(crate) fn data_dir(cx: &impl AppContext) -> AiChat2Result<PathBuf> {
    read(cx, AiChat2Config::data_dir)
}

pub(crate) fn app_settings(cx: &impl AppContext) -> AiChat2AppSettings {
    read(cx, |config| {
        AiChat2AppSettings::new(config.app_settings_payload())
    })
}

pub(crate) fn config_load_error(cx: &impl AppContext) -> Option<AiChat2ConfigLoadError> {
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

impl McpServerTomlConfig {
    fn to_agent_config(&self, server_id: &str) -> AiChat2Result<McpServerConfig> {
        let transport = match self.transport {
            McpTransportKind::Stdio => {
                let command = self.command.clone().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing command"))
                })?;
                McpServerTransport::Stdio(McpStdioTransport {
                    command,
                    args: self.args.clone(),
                })
            }
            McpTransportKind::StreamableHttp => {
                let url = self.url.clone().ok_or_else(|| {
                    AiChat2Error::Config(format!("mcp server `{server_id}` is missing url"))
                })?;
                McpServerTransport::StreamableHttp(McpStreamableHttpTransport {
                    url,
                    headers: self.headers.clone(),
                    oauth: self.oauth.as_ref().map(toml_value_to_json).transpose()?,
                })
            }
        };

        Ok(McpServerConfig {
            server_id: server_id.to_string(),
            display_name: self.display_name.clone(),
            transport,
            env: self.env.clone(),
            cwd: self.cwd.clone(),
        })
    }
}

fn toml_value_to_json(value: &toml::Value) -> AiChat2Result<serde_json::Value> {
    serde_json::to_value(value)
        .map_err(|err| AiChat2Error::Config(format!("invalid MCP OAuth config: {err}")))
}

fn default_mcp_server_enabled() -> bool {
    true
}

pub(crate) fn update_app_settings(
    cx: &mut App,
    update: impl FnOnce(&mut AppSettingsPayload),
) -> AiChat2Result<AppSettingsPayload> {
    let config_store = store(cx);
    let mut next_payload = config_store.read(cx, AiChat2Config::app_settings_payload);
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
) -> AiChat2Result<ChatFormConfig> {
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

pub(crate) fn init(cx: &mut App) -> AiChat2Result<()> {
    let path = AiChat2Config::path()?;
    let config_store = AiChat2ConfigStore::install_global_with_backend(
        cx,
        AiChat2Config::default(),
        AiChat2ConfigBackend::new(path),
    )?;
    let data_dir = data_dir(cx)?;
    let enabled_mcp_servers = match config_store.read(cx, AiChat2Config::mcp_config_layer) {
        Ok(layer) => layer.servers.len(),
        Err(err) => {
            event!(Level::ERROR, error = ?err, "parse ai-chat2 MCP config failed");
            0
        }
    };
    event!(
        Level::INFO,
        data_dir = ?data_dir,
        enabled_mcp_servers,
        "loaded ai-chat2 config"
    );
    Ok(())
}

pub(crate) fn init_app_settings(cx: &mut App) -> AiChat2Result<()> {
    let settings = app_settings(cx);
    event!(
        Level::INFO,
        language = ?settings.language(),
        theme = ?settings.theme().mode,
        temporary_hotkey = ?settings.temporary_hotkey(),
        http_proxy = ?settings.http_proxy(),
        default_project_id = ?settings.default_project_id(),
        "loaded ai-chat2 app settings"
    );
    Ok(())
}

#[cfg(test)]
pub(crate) fn install_for_test(cx: &mut App, config: AiChat2Config) -> AiChat2Result<()> {
    let path = config.config_path.clone().unwrap_or(AiChat2Config::path()?);
    AiChat2ConfigStore::install_global_with_backend(cx, config, AiChat2ConfigBackend::new(path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AiChat2AppSettings, AiChat2Config, AppSettingsConfig, ChatFormConfig, ChatFormModelConfig,
        StorageConfig, install_for_test, update_app_settings, update_chat_form_config,
    };
    use ai_chat_agent::McpServerTransport;
    use ai_chat_core::{
        AppLanguage, AppSettingsPayload, AppThemeMode, AppThemeSettings,
        ReasoningSelectionSnapshot, TokenBudgetSelectionMode, ToolApprovalMode,
    };
    use gpui::TestAppContext;
    use std::path::PathBuf;

    #[test]
    fn toml_config_stores_storage_and_app_settings() {
        let config = AiChat2Config {
            storage: StorageConfig {
                data_dir: Some(PathBuf::from("/tmp/ai-chat2")),
            },
            app_settings: AppSettingsPayload {
                language: AppLanguage::Chinese,
                theme: AppThemeSettings {
                    mode: AppThemeMode::Light,
                    light_theme: Some("preset:Default Light".to_string()),
                    dark_theme: Some("preset:Default Dark".to_string()),
                    custom_theme_colors: vec!["#3271AE".to_string()],
                },
                temporary_hotkey: Some("cmd+shift+j".to_string()),
                http_proxy: Some("http://127.0.0.1:8080".to_string()),
                default_project_id: Some("project-1".to_string()),
            }
            .into(),
            chat_form: ChatFormConfig {
                model: Some(ChatFormModelConfig {
                    provider_id: "provider-1".to_string(),
                    model_id: "gpt-5".to_string(),
                }),
                reasoning_selection: Some(ReasoningSelectionSnapshot::TokenBudget {
                    mode: TokenBudgetSelectionMode::Custom,
                    value: Some(2048),
                }),
                approval_mode: ToolApprovalMode::FullAccess,
            },
            ..Default::default()
        };

        let serialized = toml::to_string(&config).unwrap();

        assert!(serialized.contains("[storage]"));
        assert!(serialized.contains("data_dir"));
        assert!(serialized.contains("[app_settings]"));
        assert!(serialized.contains(r#"language = "zh-CN""#));
        assert!(serialized.contains(r#"temporary_hotkey = "cmd+shift+j""#));
        assert!(serialized.contains("[app_settings.theme]"));
        assert!(serialized.contains("custom_theme_colors"));
        assert!(serialized.contains("[chat_form]"));
        assert!(serialized.contains(r#"approval_mode = "full_access""#));
        assert!(serialized.contains("[chat_form.model]"));
        assert!(serialized.contains(r#"provider_id = "provider-1""#));
        assert!(serialized.contains("[chat_form.reasoning_selection]"));
        assert!(serialized.contains(r#"type = "tokenBudget""#));
        assert_eq!(
            toml::from_str::<AiChat2Config>(&serialized).unwrap(),
            config
        );
    }

    #[test]
    fn toml_config_ignores_unknown_fields_for_compatibility() {
        let config = toml::from_str::<AiChat2Config>(
            r#"
unknown_top_level = true

[storage]
data_dir = "/tmp/ai-chat2"
unknown_storage = "kept by newer app"

[app_settings]
language = "zh-CN"
unknown_app_setting = true

[app_settings.theme]
mode = "dark"
unknown_theme_setting = "kept by newer app"

[chat_form]
approval_mode = "auto_approve"
unknown_chat_form_setting = "kept by newer app"

[chat_form.model]
provider_id = "provider-1"
model_id = "gpt-5"
unknown_model_setting = true
"#,
        )
        .unwrap();

        assert_eq!(
            config.storage,
            StorageConfig {
                data_dir: Some(PathBuf::from("/tmp/ai-chat2")),
            }
        );
        assert_eq!(config.app_settings.language, AppLanguage::Chinese);
        assert_eq!(config.app_settings.theme.mode, AppThemeMode::Dark);
        assert_eq!(
            config.chat_form.approval_mode,
            ToolApprovalMode::AutoApprove
        );
        assert_eq!(
            config
                .chat_form
                .model
                .as_ref()
                .map(|model| (model.provider_id.as_str(), model.model_id.as_str())),
            Some(("provider-1", "gpt-5"))
        );
    }

    #[test]
    fn new_toml_config_writes_default_app_settings() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");

        let config = AiChat2Config::load_or_create_from_path(&path).expect("create config");

        assert_eq!(config, AiChat2Config::default());
        let source = std::fs::read_to_string(&path).expect("read config");
        assert!(source.contains("[app_settings]"));
        assert!(source.contains(r#"language = "system""#));
        assert!(source.contains("[app_settings.theme]"));
        assert!(source.contains(r#"mode = "system""#));
        assert!(source.contains("[chat_form]"));
        assert!(source.contains(r#"approval_mode = "request_approval""#));
    }

    #[test]
    fn malformed_toml_config_preserves_file_and_returns_diagnostic_default() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "not = [valid").expect("write malformed config");

        let config = AiChat2Config::load_or_create_from_path(&path).expect("load fallback config");

        assert_eq!(config.storage, StorageConfig::default());
        assert_eq!(config.app_settings, AppSettingsConfig::default());
        assert_eq!(config.chat_form, ChatFormConfig::default());
        assert!(config.mcp_servers.is_empty());
        let load_error = config.load_error.expect("load error");
        assert_eq!(load_error.path_display(), path.display().to_string());
        assert!(!load_error.message().is_empty());
        let source = std::fs::read_to_string(&path).expect("read preserved config");
        assert_eq!(source, "not = [valid");
    }

    #[gpui::test]
    fn malformed_toml_config_blocks_later_config_writes(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let malformed_source = "not = [valid";
        std::fs::write(&path, malformed_source).expect("write malformed config");
        let config = AiChat2Config::load_or_create_from_path(&path).expect("load fallback config");

        let (settings_error, chat_form_error) = cx.update(|cx| {
            install_for_test(cx, config).expect("install config store");
            let settings_error = update_app_settings(cx, |payload| {
                payload.http_proxy = Some("http://127.0.0.1:8080".to_string());
            })
            .expect_err("settings update should fail")
            .to_string();
            let chat_form_error = update_chat_form_config(cx, |config| {
                config.approval_mode = ToolApprovalMode::FullAccess;
            })
            .expect_err("chat form update should fail")
            .to_string();
            (settings_error, chat_form_error)
        });

        assert!(settings_error.contains("config.toml is invalid"));
        assert!(chat_form_error.contains("config.toml is invalid"));
        let source = std::fs::read_to_string(&path).expect("read preserved config");
        assert_eq!(source, malformed_source);
    }

    #[test]
    fn app_settings_expose_typed_config_preferences() {
        let settings = AiChat2AppSettings::new(AppSettingsPayload {
            language: AppLanguage::Chinese,
            theme: AppThemeSettings {
                mode: AppThemeMode::Light,
                ..Default::default()
            },
            temporary_hotkey: Some("cmd+shift+j".to_string()),
            http_proxy: Some("http://127.0.0.1:8080".to_string()),
            default_project_id: Some("project-1".to_string()),
        });

        assert_eq!(settings.language(), AppLanguage::Chinese);
        assert_eq!(settings.theme().mode, AppThemeMode::Light);
        assert_eq!(settings.temporary_hotkey(), Some("cmd+shift+j"));
        assert_eq!(settings.http_proxy(), Some("http://127.0.0.1:8080"));
        assert_eq!(
            settings.default_project_id().map(String::as_str),
            Some("project-1")
        );
    }

    #[test]
    fn mcp_config_layer_filters_disabled_servers_and_maps_transports() {
        let config = toml::from_str::<AiChat2Config>(
            r#"
[mcp_servers.filesystem]
enabled = true
display_name = "Filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
cwd = "/tmp"

[mcp_servers.filesystem.env]
NODE_ENV = "production"

[mcp_servers.linear]
enabled = false
transport = "streamable_http"
url = "https://example.com/mcp"

[mcp_servers.linear.headers]
Authorization = "Bearer ${LINEAR_MCP_TOKEN}"

[mcp_servers.docs]
transport = "streamable_http"
url = "https://docs.example.com/mcp"

[mcp_servers.docs.headers]
X-Docs = "enabled"
"#,
        )
        .unwrap();

        let layer = config.mcp_config_layer().unwrap();

        assert_eq!(layer.servers.len(), 2);
        assert_eq!(layer.servers[0].server_id, "docs");
        assert_eq!(layer.servers[0].display_name, None);
        match &layer.servers[0].transport {
            McpServerTransport::StreamableHttp(http) => {
                assert_eq!(http.url, "https://docs.example.com/mcp");
                assert_eq!(
                    http.headers.get("X-Docs").map(String::as_str),
                    Some("enabled")
                );
            }
            McpServerTransport::Stdio(_) => panic!("expected streamable HTTP transport"),
        }
        assert_eq!(layer.servers[1].server_id, "filesystem");
        assert_eq!(layer.servers[1].display_name.as_deref(), Some("Filesystem"));
        assert_eq!(
            layer.servers[1].env.get("NODE_ENV").map(String::as_str),
            Some("production")
        );
        match &layer.servers[1].transport {
            McpServerTransport::Stdio(stdio) => {
                assert_eq!(stdio.command, "npx");
                assert_eq!(stdio.args.len(), 3);
            }
            McpServerTransport::StreamableHttp(_) => panic!("expected stdio transport"),
        }
    }

    #[gpui::test]
    fn noop_app_settings_update_does_not_persist(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let payload = AppSettingsPayload::default();
        let config = AiChat2Config::with_app_settings_for_test(path.clone(), payload.clone());
        config.save_for_test().expect("save config");
        let before = std::fs::read_to_string(&path).expect("read config before");

        let result = cx.update(|cx| {
            install_for_test(cx, config).expect("install config store");
            update_app_settings(cx, |_| {}).expect("update settings")
        });

        let after = std::fs::read_to_string(&path).expect("read config after");
        assert_eq!(result, payload);
        assert_eq!(after, before);
    }

    #[gpui::test]
    fn noop_chat_form_update_does_not_persist(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("config.toml");
        let mut config = AiChat2Config {
            config_path: Some(path.clone()),
            ..Default::default()
        };
        config.chat_form.approval_mode = ToolApprovalMode::FullAccess;
        config.save_for_test().expect("save config");
        let before = std::fs::read_to_string(&path).expect("read config before");

        let chat_form = cx.update(|cx| {
            install_for_test(cx, config).expect("install config store");
            update_chat_form_config(cx, |config| {
                config.approval_mode = ToolApprovalMode::FullAccess;
            })
            .expect("update chat form")
        });

        let after = std::fs::read_to_string(&path).expect("read config after");
        assert_eq!(chat_form.approval_mode, ToolApprovalMode::FullAccess);
        assert_eq!(after, before);
    }
}
