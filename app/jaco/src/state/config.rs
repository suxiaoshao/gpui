use crate::{
    app::APP_NAME,
    errors::{JacoError, JacoResult},
    foundation::persistence,
};
use gpui::{App, AppContext, Task};
use gpui_operation::{Complete, Refresh, Repair, Settle, Transition, repair};
use gpui_store::{Select, Store};
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
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
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
}

pub(crate) type ConfigOperation =
    repair::Operation<ConfigData, ConfigProblem, ConfigRepair, Task<()>>;
pub(crate) type JacoConfigStore = Store<ConfigOperation>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigGateStatus {
    phase: gpui_operation::repair::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectConfigGateStatus;

impl Select<ConfigOperation> for SelectConfigGateStatus {
    type Output = ConfigGateStatus;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        ConfigGateStatus {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ConfigData {
    value: JacoConfig,
    path: PathBuf,
    source_bytes: Vec<u8>,
    data_dir: PathBuf,
}

impl ConfigData {
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

impl Deref for ConfigData {
    type Target = JacoConfig;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PendingConfig {
    data: ConfigData,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigBackupIntent {
    CreateDefault,
    OverwritePending,
}

#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum ConfigProblem {
    #[error("could not resolve the configuration directory: {message}")]
    ResolveDirectory { message: String },
    #[error("could not read {path}: {message}")]
    Read { path: PathBuf, message: String },
    #[error("could not parse {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("could not derive the database target from {path}: {message}")]
    Target { path: PathBuf, message: String },
    #[error("another Jaco process is writing {path}: {message}")]
    Locked {
        path: PathBuf,
        message: String,
        pending: Arc<PendingConfig>,
    },
    #[error("{path} changed outside Jaco")]
    ExternalChange {
        path: PathBuf,
        pending: Arc<PendingConfig>,
    },
    #[error("could not write {path}: {message}")]
    Write {
        path: PathBuf,
        message: String,
        pending: Arc<PendingConfig>,
    },
    #[error("could not back up {path}: {message}")]
    Backup {
        path: PathBuf,
        message: String,
        intent: ConfigBackupIntent,
        pending: Option<Arc<PendingConfig>>,
    },
    #[error("the backup at {backup_path} succeeded, but writing {path} failed: {message}")]
    WriteAfterBackup {
        path: PathBuf,
        backup_path: PathBuf,
        message: String,
        intent: ConfigBackupIntent,
        pending: Option<Arc<PendingConfig>>,
    },
}

impl ConfigProblem {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Read { path, .. }
            | Self::Parse { path, .. }
            | Self::Target { path, .. }
            | Self::Locked { path, .. }
            | Self::ExternalChange { path, .. }
            | Self::Write { path, .. }
            | Self::Backup { path, .. }
            | Self::WriteAfterBackup { path, .. } => path,
            Self::ResolveDirectory { .. } => Path::new(""),
        }
    }

    pub(crate) fn supports(&self, repair: ConfigRepair) -> bool {
        match repair {
            ConfigRepair::Reload => true,
            ConfigRepair::RetryWrite => {
                matches!(
                    self,
                    Self::Locked { .. } | Self::Write { .. } | Self::WriteAfterBackup { .. }
                )
            }
            ConfigRepair::BackupAndCreateDefault => matches!(
                self,
                Self::Parse { .. }
                    | Self::Target { .. }
                    | Self::Backup {
                        intent: ConfigBackupIntent::CreateDefault,
                        ..
                    }
            ),
            ConfigRepair::BackupAndOverwritePending => matches!(
                self,
                Self::ExternalChange { .. }
                    | Self::Backup {
                        intent: ConfigBackupIntent::OverwritePending,
                        ..
                    }
            ),
        }
    }

    fn pending(&self) -> Option<Arc<PendingConfig>> {
        match self {
            Self::Locked { pending, .. }
            | Self::ExternalChange { pending, .. }
            | Self::Write { pending, .. } => Some(pending.clone()),
            Self::Backup { pending, .. } | Self::WriteAfterBackup { pending, .. } => {
                pending.clone()
            }
            _ => None,
        }
    }

    fn backup_path(&self) -> Option<PathBuf> {
        match self {
            Self::WriteAfterBackup { backup_path, .. } => Some(backup_path.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigRepair {
    Reload,
    RetryWrite,
    BackupAndCreateDefault,
    BackupAndOverwritePending,
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
    }
}

impl JacoConfig {
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

    pub(crate) fn app_settings_payload(&self) -> AppSettingsPayload {
        self.app_settings.payload()
    }

    #[cfg(test)]
    pub(crate) fn with_app_settings_for_test(
        _config_path: PathBuf,
        payload: AppSettingsPayload,
    ) -> Self {
        Self {
            app_settings: AppSettingsConfig::from(payload),
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn load_from_path_for_test(path: &Path) -> JacoResult<Self> {
        load_for_operation(path)
            .map(|data| data.value)
            .map_err(|error| JacoError::Config(error.to_string()))
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
    store(cx).read(cx, |operation| {
        f(&operation
            .data()
            .expect("config command requires ConfigOperation data")
            .value)
    })
}

pub(crate) fn data_dir(cx: &impl AppContext) -> JacoResult<PathBuf> {
    store(cx).read(cx, |operation| {
        operation
            .data()
            .map(|data| data.data_dir.clone())
            .ok_or_else(|| JacoError::Config("config is not ready".to_string()))
    })
}

pub(crate) fn app_settings(cx: &impl AppContext) -> JacoAppSettings {
    read(cx, |config| {
        JacoAppSettings::new(config.app_settings_payload())
    })
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
    let current = ready_data(cx)?;
    let mut next_payload = current.app_settings_payload();
    update(&mut next_payload);
    let committed_payload = next_payload.clone();
    commit_update(
        current,
        move |config| config.app_settings = AppSettingsConfig::from(committed_payload),
        cx,
    )?;
    Ok(next_payload)
}

pub(crate) fn update_chat_form_config(
    cx: &mut App,
    update: impl FnOnce(&mut ChatFormConfig),
) -> JacoResult<ChatFormConfig> {
    let current = ready_data(cx)?;
    let mut next_chat_form = current.chat_form.clone();
    update(&mut next_chat_form);
    let committed_chat_form = next_chat_form.clone();
    commit_update(
        current,
        move |config| config.chat_form = committed_chat_form,
        cx,
    )?;
    Ok(next_chat_form)
}

pub(crate) fn init(cx: &mut App) -> JacoResult<()> {
    let result = JacoConfig::path()
        .map_err(|error| ConfigProblem::ResolveDirectory {
            message: error.to_string(),
        })
        .and_then(|path| load_for_operation(&path));
    JacoConfigStore::install_global(cx, initial_operation(result));
    Ok(())
}

fn initial_operation(result: Result<ConfigData, ConfigProblem>) -> ConfigOperation {
    let mut operation = ConfigOperation::new();
    operation.transition(Settle(result));
    operation
}

#[cfg(test)]
pub(crate) fn install_for_test(cx: &mut App, path: PathBuf, config: JacoConfig) -> JacoResult<()> {
    let source_bytes = fs::read(&path).unwrap_or_else(|_| {
        toml::to_string_pretty(&config)
            .expect("test config must encode")
            .into_bytes()
    });
    let data = data_from_value(path, config, source_bytes)
        .map_err(|error| JacoError::Config(error.to_string()))?;
    JacoConfigStore::install_global(cx, initial_operation(Ok(data)));
    Ok(())
}

fn ready_data(cx: &impl AppContext) -> JacoResult<ConfigData> {
    store(cx).read(cx, |operation| match operation {
        ConfigOperation::Ready(ready) => Ok(ready.data().clone()),
        _ => Err(JacoError::Config(
            "config is not ready for mutation".to_string(),
        )),
    })
}

fn update_config<R>(cx: &mut App, update: impl FnOnce(&mut JacoConfig) -> R) -> JacoResult<R> {
    let current = ready_data(cx)?;
    let mut next = current.value.clone();
    let result = update(&mut next);
    commit_update(current, move |config| *config = next, cx)?;
    Ok(result)
}

fn commit_update(
    current: ConfigData,
    update: impl FnOnce(&mut JacoConfig),
    cx: &mut App,
) -> JacoResult<()> {
    let mut value = current.value.clone();
    update(&mut value);
    if value == current.value {
        return Ok(());
    }
    let bytes = toml::to_string_pretty(&value)
        .map_err(|error| JacoError::Config(format!("encode config: {error}")))?
        .into_bytes();
    let pending = Arc::new(PendingConfig {
        data: data_from_value(current.path.clone(), value, bytes.clone())
            .map_err(|error| JacoError::Config(error.to_string()))?,
        bytes,
    });
    let result = write_pending(&current, pending);
    let error = result
        .as_ref()
        .err()
        .map(|problem| JacoError::Config(problem.to_string()));
    store(cx).update(cx, |operation| {
        operation.transition(Settle(result));
    });
    error.map_or(Ok(()), Err)
}

fn load_for_operation(path: &Path) -> Result<ConfigData, ConfigProblem> {
    match fs::read(path) {
        Ok(source) => decode_data(path, source),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let value = JacoConfig::default();
            let bytes = toml::to_string_pretty(&value)
                .map_err(|error| ConfigProblem::Write {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                    pending: Arc::new(PendingConfig {
                        data: data_from_value(path.to_path_buf(), value.clone(), Vec::new())
                            .expect("default config target must be valid"),
                        bytes: Vec::new(),
                    }),
                })?
                .into_bytes();
            let pending = Arc::new(PendingConfig {
                data: data_from_value(path.to_path_buf(), value, bytes.clone())?,
                bytes,
            });
            write_pending_at(path, None, pending)
        }
        Err(error) => Err(ConfigProblem::Read {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn decode_data(path: &Path, source: Vec<u8>) -> Result<ConfigData, ConfigProblem> {
    let text = std::str::from_utf8(&source).map_err(|error| ConfigProblem::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let value = toml::from_str::<JacoConfig>(text).map_err(|error| ConfigProblem::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    data_from_value(path.to_path_buf(), value, source)
}

fn data_from_value(
    path: PathBuf,
    value: JacoConfig,
    source_bytes: Vec<u8>,
) -> Result<ConfigData, ConfigProblem> {
    let parent = path.parent().ok_or_else(|| ConfigProblem::Target {
        path: path.clone(),
        message: "configuration path has no parent".to_string(),
    })?;
    let data_dir = match value.storage.data_dir.as_ref() {
        Some(data_dir) if data_dir.is_absolute() => data_dir.clone(),
        Some(data_dir) => normalize_lexically(parent.join(data_dir)),
        None => parent.to_path_buf(),
    };
    if data_dir.as_os_str().is_empty() {
        return Err(ConfigProblem::Target {
            path,
            message: "database directory is empty".to_string(),
        });
    }
    Ok(ConfigData {
        value,
        path,
        source_bytes,
        data_dir,
    })
}

fn write_pending(
    current: &ConfigData,
    pending: Arc<PendingConfig>,
) -> Result<ConfigData, ConfigProblem> {
    write_pending_at(&current.path, Some(&current.source_bytes), pending)
}

fn write_pending_at(
    path: &Path,
    expected: Option<&[u8]>,
    pending: Arc<PendingConfig>,
) -> Result<ConfigData, ConfigProblem> {
    let lock_path = path.with_extension("toml.lock");
    let _lock =
        persistence::FileLock::acquire(&lock_path).map_err(|error| ConfigProblem::Locked {
            path: path.to_path_buf(),
            message: error.to_string(),
            pending: pending.clone(),
        })?;
    let committed =
        persistence::atomic_replace(path, expected, &pending.bytes).map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                ConfigProblem::ExternalChange {
                    path: path.to_path_buf(),
                    pending: pending.clone(),
                }
            } else {
                ConfigProblem::Write {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                    pending: pending.clone(),
                }
            }
        })?;
    data_from_value(path.to_path_buf(), pending.data.value.clone(), committed)
}

pub(crate) fn request_reload(cx: &mut App) {
    let config_store = store(cx);
    let (path, refresh_ready) = config_store.read(cx, |operation| {
        (
            operation.data().map(|data| data.path.clone()).or_else(|| {
                operation
                    .problem()
                    .map(|problem| problem.path().to_path_buf())
            }),
            matches!(operation, ConfigOperation::Ready(_)),
        )
    });
    let path = path
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| JacoConfig::path().ok());
    let completion_store = config_store.clone();
    let task = cx.spawn(async move |cx| {
        let result = smol::unblock(move || {
            path.ok_or_else(|| ConfigProblem::ResolveDirectory {
                message: "configuration directory is unavailable".to_string(),
            })
            .and_then(|path| load_for_operation(&path))
        })
        .await;
        cx.update(|cx| {
            completion_store.update(cx, |operation| {
                if matches!(
                    operation,
                    ConfigOperation::Refreshing(_)
                        | ConfigOperation::RepairingUnavailable(_)
                        | ConfigOperation::RepairingDegraded(_)
                ) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    config_store.update(cx, |operation| {
        if refresh_ready && matches!(operation, ConfigOperation::Ready(_)) {
            operation.transition(Refresh(task));
        } else if !refresh_ready
            && matches!(
                operation,
                ConfigOperation::Unavailable(_) | ConfigOperation::Degraded(_)
            )
        {
            operation.transition(Repair {
                repair: ConfigRepair::Reload,
                task,
            });
        }
    });
}

pub(crate) fn request_repair(repair: ConfigRepair, cx: &mut App) -> JacoResult<()> {
    let (path, problem, current) = store(cx).read(cx, |operation| {
        let problem = operation.problem().cloned();
        let path = operation
            .data()
            .map(|data| data.path.clone())
            .or_else(|| problem.as_ref().map(|problem| problem.path().to_path_buf()));
        (path, problem, operation.data().cloned())
    });
    let problem = problem.ok_or_else(|| JacoError::Config("config has no problem".to_string()))?;
    if !problem.supports(repair) {
        return Err(JacoError::Config(format!(
            "{repair:?} is not supported for the current config problem"
        )));
    }
    let path = path
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| JacoError::Config("configuration path is unavailable".to_string()))?;
    #[allow(clippy::large_enum_variant)]
    enum RepairAttempt {
        Reload {
            path: PathBuf,
        },
        RetryWrite {
            path: PathBuf,
            current: Option<ConfigData>,
            pending: Arc<PendingConfig>,
            backup_path: Option<PathBuf>,
        },
        BackupAndCreateDefault {
            path: PathBuf,
        },
        BackupAndOverwritePending {
            path: PathBuf,
            pending: Arc<PendingConfig>,
        },
    }
    let attempt = match repair {
        ConfigRepair::Reload => RepairAttempt::Reload { path },
        ConfigRepair::RetryWrite => RepairAttempt::RetryWrite {
            path,
            current,
            pending: problem
                .pending()
                .ok_or_else(|| JacoError::Config("pending config is unavailable".to_string()))?,
            backup_path: problem.backup_path(),
        },
        ConfigRepair::BackupAndCreateDefault => RepairAttempt::BackupAndCreateDefault { path },
        ConfigRepair::BackupAndOverwritePending => RepairAttempt::BackupAndOverwritePending {
            path,
            pending: problem
                .pending()
                .ok_or_else(|| JacoError::Config("pending config is unavailable".to_string()))?,
        },
    };
    let config_store = store(cx);
    let completion_store = config_store.clone();
    let task = cx.spawn(async move |cx| {
        let result = smol::unblock(move || match attempt {
            RepairAttempt::Reload { path } => load_for_operation(&path),
            RepairAttempt::RetryWrite {
                path,
                current,
                pending,
                backup_path,
            } => match (current, backup_path) {
                (Some(current), Some(backup_path)) => {
                    write_pending_after_backup(&current, pending, backup_path)
                }
                (Some(current), None) => write_pending(&current, pending),
                (None, _) => write_pending_at(&path, None, pending),
            },
            RepairAttempt::BackupAndCreateDefault { path } => {
                backup_and_replace(&path, ConfigBackupIntent::CreateDefault, None)
            }
            RepairAttempt::BackupAndOverwritePending { path, pending } => {
                backup_and_replace(&path, ConfigBackupIntent::OverwritePending, Some(pending))
            }
        })
        .await;
        cx.update(|cx| {
            completion_store.update(cx, |operation| {
                if matches!(
                    operation,
                    ConfigOperation::RepairingUnavailable(_)
                        | ConfigOperation::RepairingDegraded(_)
                ) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    config_store.update(cx, |operation| {
        if matches!(
            operation,
            ConfigOperation::Unavailable(_) | ConfigOperation::Degraded(_)
        ) {
            operation.transition(Repair { repair, task });
        }
    });
    Ok(())
}

fn backup_and_replace(
    path: &Path,
    intent: ConfigBackupIntent,
    pending: Option<Arc<PendingConfig>>,
) -> Result<ConfigData, ConfigProblem> {
    if let Ok(valid) = load_for_operation(path) {
        return Ok(valid);
    }
    let pending = match pending {
        Some(pending) => pending,
        None => {
            let value = JacoConfig::default();
            let bytes = toml::to_string_pretty(&value)
                .map_err(|error| ConfigProblem::Write {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                    pending: Arc::new(PendingConfig {
                        data: data_from_value(path.to_path_buf(), value.clone(), Vec::new())
                            .expect("default config target must be valid"),
                        bytes: Vec::new(),
                    }),
                })?
                .into_bytes();
            Arc::new(PendingConfig {
                data: data_from_value(path.to_path_buf(), value, bytes.clone())?,
                bytes,
            })
        }
    };
    let lock_path = path.with_extension("toml.lock");
    let _lock =
        persistence::FileLock::acquire(&lock_path).map_err(|error| ConfigProblem::Locked {
            path: path.to_path_buf(),
            message: error.to_string(),
            pending: pending.clone(),
        })?;
    let original = fs::read(path).map_err(|error| ConfigProblem::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let parent = path.parent().ok_or_else(|| ConfigProblem::Target {
        path: path.to_path_buf(),
        message: "configuration path has no parent".to_string(),
    })?;
    let backup_path = persistence::next_available_path(parent, "config.invalid", "toml");
    persistence::copy_new_synced(&original, &backup_path).map_err(|error| {
        ConfigProblem::Backup {
            path: path.to_path_buf(),
            message: error.to_string(),
            intent,
            pending: Some(pending.clone()),
        }
    })?;
    if fs::read(path).ok().as_deref() != Some(original.as_slice()) {
        return Err(ConfigProblem::ExternalChange {
            path: path.to_path_buf(),
            pending,
        });
    }
    let committed =
        persistence::atomic_replace(path, Some(&original), &pending.bytes).map_err(|error| {
            ConfigProblem::WriteAfterBackup {
                path: path.to_path_buf(),
                backup_path: backup_path.clone(),
                message: error.to_string(),
                intent,
                pending: Some(pending.clone()),
            }
        })?;
    data_from_value(path.to_path_buf(), pending.data.value.clone(), committed)
}

fn write_pending_after_backup(
    current: &ConfigData,
    pending: Arc<PendingConfig>,
    backup_path: PathBuf,
) -> Result<ConfigData, ConfigProblem> {
    write_pending(current, pending.clone()).map_err(|problem| ConfigProblem::WriteAfterBackup {
        path: current.path.clone(),
        backup_path,
        message: problem.to_string(),
        intent: ConfigBackupIntent::OverwritePending,
        pending: Some(pending),
    })
}

fn normalize_lexically(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests;
