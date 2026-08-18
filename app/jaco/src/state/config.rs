use crate::{
    app::file_watch::{self, FileWatchBinding},
    errors::{JacoError, JacoResult},
    foundation::{paths, persistence},
};
use gpui::{App, AppContext, Context, Entity, Global, Subscription, Task};
use gpui_operation::{Complete, Load, Refresh, Repair, Settle, Transition, repair};
use gpui_store::{Select, Store};
use jaco_core::{
    AppLanguage, AppSettingsPayload, AppThemeMode, AppThemeSettings, ProjectId, ProviderId,
    ProviderModelId, ReasoningSelectionSnapshot, ToolApprovalMode, default_tool_approval_mode,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

mod mcp;

#[cfg(test)]
pub(crate) use mcp::McpToolApprovalMode;
pub(crate) use mcp::{
    McpOAuthTomlConfig, McpServerTomlConfig, McpTransportKind, delete_mcp_server,
    is_reserved_mcp_header, is_valid_mcp_env_var_name, is_valid_mcp_server_id,
    set_mcp_server_enabled, upsert_mcp_server_if_unchanged,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct JacoConfig {
    pub(crate) app_settings: AppSettingsConfig,
    pub(crate) chat_form: ChatFormConfig,
    pub(crate) mcp_servers: BTreeMap<String, McpServerTomlConfig>,
}

pub(crate) type ConfigOperation =
    repair::Operation<ConfigData, ConfigProblem, ConfigRepair, Task<()>>;
pub(crate) type JacoConfigStore = Store<ConfigOperation>;

struct ConfigFileObserver {
    _binding: Option<FileWatchBinding>,
    _config_subscription: Option<Subscription>,
    probe_task: Option<Task<()>>,
    pending_dirty: bool,
}

#[derive(Clone)]
struct ConfigFileObserverGlobal {
    _observer: Entity<ConfigFileObserver>,
}

impl Global for ConfigFileObserverGlobal {}

#[derive(Clone)]
struct ConfigProbeStart {
    source_bytes: Option<Vec<u8>>,
}

type ConfigProbeResult = Result<ConfigData, ConfigProblem>;

const MISSING_CONFIRMATION_DELAY: Duration = Duration::from_millis(100);

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

impl JacoConfig {
    pub(crate) fn path() -> JacoResult<PathBuf> {
        paths::config_file()
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

pub(crate) fn init_file_observer(cx: &mut App) {
    if cx.has_global::<ConfigFileObserverGlobal>() {
        return;
    }
    let targets = JacoConfig::path()
        .ok()
        .and_then(|path| match file_watch::exact_file(path) {
            Ok(target) => Some(vec![target]),
            Err(problem) => {
                file_watch::report_problem(problem, cx);
                None
            }
        })
        .unwrap_or_default();
    let observer = cx.new(|cx| {
        let mut observer = ConfigFileObserver {
            _binding: None,
            _config_subscription: None,
            probe_task: None,
            pending_dirty: false,
        };
        observer._binding = Some(file_watch::bind(
            targets,
            cx,
            |observer: &mut ConfigFileObserver, cx| {
                observer.on_dirty(cx);
            },
        ));
        observer._config_subscription = Some(store(cx).observe(cx, |observer, operation, cx| {
            observer.on_config_operation_changed(operation, cx);
        }));
        observer
    });
    cx.set_global(ConfigFileObserverGlobal {
        _observer: observer.clone(),
    });
    observer.update(cx, |observer, cx| observer.start_probe(cx));
}

pub(crate) fn shutdown_file_observer(cx: &mut App) {
    let Some(observer) = cx
        .try_global::<ConfigFileObserverGlobal>()
        .map(|global| global._observer.clone())
    else {
        return;
    };
    observer.update(cx, |observer, _| {
        observer.probe_task.take();
        observer._binding.take();
        observer._config_subscription.take();
        observer.pending_dirty = false;
    });
}

impl ConfigFileObserver {
    fn on_dirty(&mut self, cx: &mut Context<Self>) {
        if crate::app::is_shutting_down() {
            return;
        }
        if self.probe_task.is_some() || store(cx).read(cx, ConfigOperation::is_running) {
            self.pending_dirty = true;
            return;
        }
        self.start_probe(cx);
    }

    fn on_config_operation_changed(&mut self, operation: &ConfigOperation, cx: &mut Context<Self>) {
        if !crate::app::is_shutting_down()
            && !operation.is_running()
            && self.probe_task.is_none()
            && self.pending_dirty
        {
            let observer = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = observer.update(cx, |observer, cx| observer.consume_pending(cx));
            });
        }
    }

    fn start_probe(&mut self, cx: &mut Context<Self>) {
        if crate::app::is_shutting_down() {
            return;
        }
        if self.probe_task.is_some() {
            self.pending_dirty = true;
            return;
        }
        let (operation_running, source_bytes, path) = store(cx).read(cx, |operation| {
            let data = operation.data();
            let path = data
                .map(|data| data.path.clone())
                .or_else(|| {
                    operation
                        .problem()
                        .map(|problem| problem.path().to_path_buf())
                })
                .filter(|path| !path.as_os_str().is_empty())
                .or_else(|| JacoConfig::path().ok());
            (
                operation.is_running(),
                data.map(|data| data.source_bytes.clone()),
                path,
            )
        });
        if operation_running {
            self.pending_dirty = true;
            return;
        }
        let start = ConfigProbeStart { source_bytes };
        let task = cx.spawn(async move |observer, cx| {
            if crate::app::is_shutting_down() {
                return;
            }
            let result = match path {
                Some(path) => {
                    let first_path = path.clone();
                    match smol::unblock(move || read_for_observer(&first_path)).await {
                        Ok(Some(data)) => Ok(data),
                        Ok(None) => {
                            cx.background_executor()
                                .timer(MISSING_CONFIRMATION_DELAY)
                                .await;
                            if crate::app::is_shutting_down() {
                                return;
                            }
                            smol::unblock(move || load_or_create(&path)).await
                        }
                        Err(problem) => Err(problem),
                    }
                }
                None => Err(ConfigProblem::ResolveDirectory {
                    message: "configuration directory is unavailable".to_string(),
                }),
            };
            let _ = observer.update(cx, |observer, cx| {
                observer.finish_probe(start, result, cx);
            });
        });
        self.probe_task = Some(task);
    }

    fn finish_probe(
        &mut self,
        start: ConfigProbeStart,
        result: ConfigProbeResult,
        cx: &mut Context<Self>,
    ) {
        self.probe_task.take();
        if crate::app::is_shutting_down() {
            return;
        }
        let (operation_running, current_source_bytes) = store(cx).read(cx, |operation| {
            (
                operation.is_running(),
                operation.data().map(|data| data.source_bytes.clone()),
            )
        });
        if operation_running || current_source_bytes != start.source_bytes {
            self.pending_dirty = true;
            self.consume_pending(cx);
            return;
        }
        if result
            .as_ref()
            .ok()
            .is_some_and(|data| Some(&data.source_bytes) == current_source_bytes.as_ref())
        {
            self.consume_pending(cx);
            return;
        }
        apply_observed_probe(result, cx);
    }

    fn consume_pending(&mut self, cx: &mut Context<Self>) {
        if self.pending_dirty
            && self.probe_task.is_none()
            && !store(cx).read(cx, ConfigOperation::is_running)
        {
            self.pending_dirty = false;
            self.start_probe(cx);
        }
    }
}

fn apply_observed_probe(result: ConfigProbeResult, cx: &mut App) {
    if crate::app::is_shutting_down() {
        return;
    }
    let task = cx.spawn(async move |cx| {
        cx.update(|cx| {
            if crate::app::is_shutting_down() {
                return;
            }
            store(cx).update(cx, |operation| {
                if matches!(
                    operation,
                    ConfigOperation::Loading(_)
                        | ConfigOperation::Refreshing(_)
                        | ConfigOperation::RepairingUnavailable(_)
                        | ConfigOperation::RepairingDegraded(_)
                ) {
                    operation.transition(Complete(result));
                }
            });
        });
    });
    store(cx).update(cx, |operation| match operation {
        ConfigOperation::Idle(_) => operation.transition(Load(task)),
        ConfigOperation::Ready(_) => operation.transition(Refresh(task)),
        ConfigOperation::Unavailable(_) | ConfigOperation::Degraded(_) => {
            operation.transition(Repair {
                repair: ConfigRepair::Reload,
                task,
            });
        }
        ConfigOperation::Loading(_)
        | ConfigOperation::Refreshing(_)
        | ConfigOperation::RepairingUnavailable(_)
        | ConfigOperation::RepairingDegraded(_) => {}
    });
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
    let data = data_from_value(path, config, source_bytes);
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
        data: data_from_value(current.path.clone(), value, bytes.clone()),
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
    load_or_create(path)
}

fn read_for_observer(path: &Path) -> Result<Option<ConfigData>, ConfigProblem> {
    match fs::read(path) {
        Ok(source) => decode_data(path, source).map(Some),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ConfigProblem::Read {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn load_or_create(path: &Path) -> Result<ConfigData, ConfigProblem> {
    match fs::read(path) {
        Ok(source) => decode_data(path, source),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let parent = path.parent().ok_or_else(|| ConfigProblem::Write {
                path: path.to_path_buf(),
                message: "configuration path has no parent".to_string(),
                pending: default_pending(path),
            })?;
            fs::create_dir_all(parent).map_err(|error| ConfigProblem::Write {
                path: path.to_path_buf(),
                message: error.to_string(),
                pending: default_pending(path),
            })?;
            let value = JacoConfig::default();
            let bytes = toml::to_string_pretty(&value)
                .map_err(|error| ConfigProblem::Write {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                    pending: Arc::new(PendingConfig {
                        data: data_from_value(path.to_path_buf(), value.clone(), Vec::new()),
                        bytes: Vec::new(),
                    }),
                })?
                .into_bytes();
            let pending = Arc::new(PendingConfig {
                data: data_from_value(path.to_path_buf(), value, bytes.clone()),
                bytes,
            });
            match write_pending_at(path, None, pending) {
                Ok(data) => Ok(data),
                Err(ConfigProblem::ExternalChange { .. }) => fs::read(path)
                    .map_err(|error| ConfigProblem::Read {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    })
                    .and_then(|source| decode_data(path, source)),
                Err(problem) => Err(problem),
            }
        }
        Err(error) => Err(ConfigProblem::Read {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn default_pending(path: &Path) -> Arc<PendingConfig> {
    let value = JacoConfig::default();
    let bytes = toml::to_string_pretty(&value)
        .expect("default config must encode")
        .into_bytes();
    Arc::new(PendingConfig {
        data: data_from_value(path.to_path_buf(), value, bytes.clone()),
        bytes,
    })
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
    Ok(data_from_value(path.to_path_buf(), value, source))
}

fn data_from_value(path: PathBuf, value: JacoConfig, source_bytes: Vec<u8>) -> ConfigData {
    ConfigData {
        value,
        path,
        source_bytes,
    }
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
    Ok(data_from_value(
        path.to_path_buf(),
        pending.data.value.clone(),
        committed,
    ))
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
    let task = cx.spawn(async move |cx| {
        let result = smol::unblock(move || {
            path.ok_or_else(|| ConfigProblem::ResolveDirectory {
                message: "configuration directory is unavailable".to_string(),
            })
            .and_then(|path| load_for_operation(&path))
        })
        .await;
        cx.update(|cx| {
            store(cx).update(cx, |operation| {
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
            store(cx).update(cx, |operation| {
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
                        data: data_from_value(path.to_path_buf(), value.clone(), Vec::new()),
                        bytes: Vec::new(),
                    }),
                })?
                .into_bytes();
            Arc::new(PendingConfig {
                data: data_from_value(path.to_path_buf(), value, bytes.clone()),
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
    let parent = path.parent().ok_or_else(|| ConfigProblem::Backup {
        path: path.to_path_buf(),
        message: "configuration path has no parent".to_string(),
        intent,
        pending: Some(pending.clone()),
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
    Ok(data_from_value(
        path.to_path_buf(),
        pending.data.value.clone(),
        committed,
    ))
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

#[cfg(test)]
mod tests;
