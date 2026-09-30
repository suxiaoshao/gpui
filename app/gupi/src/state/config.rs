use crate::foundation::{paths, persistence};
use gpui_form::{Form, FormSchema, FormVersion};
use gpui_kit::*;
use gpui_operation::{Complete, Load, Refresh, Repair, Transition, repair};
use gpui_store::Store;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AppLanguage {
    #[default]
    System,
    English,
    Chinese,
    TraditionalChinese,
    Japanese,
    Korean,
    German,
    French,
    Spanish,
    BrazilianPortuguese,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FormSchema)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct AppConfig {
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub keybindings: super::keybindings::Overrides,
    pub shortcuts: super::shortcuts::Shortcuts,
    pub pi_command: Option<String>,
    pub theme: ThemeMode,
    pub icon_theme: super::icons::IconTheme,
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
    pub language: AppLanguage,
    pub notifications: super::notifications::Preferences,
}
impl AppConfig {
    pub fn pi_executable(&self) -> PathBuf {
        self.pi_command.as_deref().unwrap_or("pi").into()
    }
    pub fn normalized(mut self) -> Result<Self, String> {
        for (id, binding) in &self.keybindings {
            if !super::keybindings::COMMANDS.iter().any(|c| c.id == id) {
                return Err("error-config-validation".into());
            }
            super::keybindings::syntax(binding).map_err(str::to_owned)?;
        }
        self.shortcuts.validate()?;
        let globals = std::iter::once(&self.shortcuts.launcher)
            .chain(
                self.shortcuts
                    .tasks
                    .iter()
                    .filter(|t| t.enabled)
                    .map(|t| &t.binding),
            )
            .filter(|s| !s.is_empty())
            .map(|s| super::shortcuts::system_binding(s))
            .collect::<Result<Vec<_>, _>>()?;
        for command in super::keybindings::COMMANDS {
            let binding = command.value(&self.keybindings);
            if !binding.is_empty()
                && let Ok(binding) = super::shortcuts::system_binding(binding)
                && globals.contains(&binding)
            {
                return Err("settings-key-conflict".into());
            }
        }

        self.pi_command = PiSettings {
            command: self.pi_command,
        }
        .normalized()?
        .command;
        Ok(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, FormSchema)]
pub(crate) struct PiSettings {
    pub command: Option<String>,
}
impl PiSettings {
    pub fn normalized(mut self) -> Result<Self, String> {
        self.command = self
            .command
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if let Some(command) = &self.command {
            let path = std::path::Path::new(command);
            if command.chars().any(char::is_control)
                || (!path.is_absolute()
                    && (command.contains(char::is_whitespace) || path.components().count() != 1))
            {
                return Err("error-config-validation".into());
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PreferenceChange {
    Keybinding(String, Option<String>),
    ResetKeybindings,
    Notifications(super::notifications::Preferences),
    Shortcuts(super::shortcuts::Shortcuts),
    Language(AppLanguage),
    Theme(ThemeMode),
    IconTheme(super::icons::IconTheme),
    LightTheme(Option<String>),
    DarkTheme(Option<String>),
}
impl PreferenceChange {
    fn apply(self, config: &mut AppConfig) {
        match self {
            Self::Notifications(value) => config.notifications = value,
            Self::ResetKeybindings => {
                config.keybindings.clear();
                config.shortcuts.clear_bindings();
            }
            Self::Shortcuts(value) => config.shortcuts = value,
            Self::Keybinding(id, binding) => match binding {
                Some(binding) => {
                    config.keybindings.insert(id, binding);
                }
                None => {
                    config.keybindings.remove(&id);
                }
            },
            Self::Language(value) => config.language = value,
            Self::Theme(value) => config.theme = value,
            Self::IconTheme(value) => config.icon_theme = value,
            Self::LightTheme(value) => config.light_theme = value,
            Self::DarkTheme(value) => config.dark_theme = value,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum ConfigContents {
    Missing,
    Configured(AppConfig),
}
#[derive(Clone, Debug)]
pub(crate) struct ConfigData {
    pub path: PathBuf,
    pub contents: ConfigContents,
    pub backup: Option<PathBuf>,
}
impl ConfigData {
    pub fn configured(&self) -> Option<&AppConfig> {
        match &self.contents {
            ConfigContents::Missing => None,
            ConfigContents::Configured(value) => Some(value),
        }
    }
}
struct PendingConfig {
    value: AppConfig,
    baseline: Option<AppConfig>,
    version: Option<FormVersion>,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigProblem {
    #[error("configuration read failed: {0}")]
    Read(String),
    #[error("configuration parse failed: {0}")]
    Parse(String),
    #[error("invalid configuration: {0}")]
    Validation(String),
    #[error("configuration write failed: {failure}")]
    Write {
        failure: std::io::Error,
        backup: Option<PathBuf>,
    },
}
impl ConfigProblem {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Read(_) => "error-config-read",
            Self::Parse(_) => "error-config-parse",
            Self::Validation(_) => "error-config-validation",
            Self::Write { .. } => "error-config-write",
        }
    }
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Write { .. })
    }
    pub fn backup(&self) -> Option<&PathBuf> {
        match self {
            Self::Write { backup, .. } => backup.as_ref(),
            Self::Read(_) | Self::Parse(_) | Self::Validation(_) => None,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum ConfigRepair {
    Reload,
    SaveDraft,
    SavePi,
    SavePreference(PreferenceChange),
    WriteCommitted,
    BackupAndReset,
}
pub(crate) type ConfigOperation =
    repair::Operation<ConfigData, ConfigProblem, ConfigRepair, Task<()>>;
pub(crate) type ConfigStore = Store<ConfigOperation>;

pub(crate) fn read_config(
    path: PathBuf,
    require_configured: bool,
) -> Result<ConfigData, ConfigProblem> {
    let bytes = persistence::read(&path).map_err(|e| ConfigProblem::Read(e.to_string()))?;
    let contents = match &bytes {
        None if require_configured => {
            return Err(ConfigProblem::Read("file removed".into()));
        }
        None => ConfigContents::Missing,
        Some(bytes) => {
            let text =
                std::str::from_utf8(bytes).map_err(|e| ConfigProblem::Parse(e.to_string()))?;
            let value: AppConfig = toml::from_str(text)
                .map_err(|_| ConfigProblem::Parse("invalid TOML or unsupported fields".into()))?;
            ConfigContents::Configured(value.normalized().map_err(ConfigProblem::Validation)?)
        }
    };
    Ok(ConfigData {
        path,
        contents,
        backup: None,
    })
}
fn write_config(
    path: PathBuf,
    pending: PendingConfig,
    reset: bool,
    reset_keybindings: bool,
    mut backup: Option<PathBuf>,
) -> Result<ConfigData, ConfigProblem> {
    let mut value = if reset {
        if let Some(bytes) =
            persistence::read(&path).map_err(|e| ConfigProblem::Read(e.to_string()))?
        {
            backup = Some(persistence::backup(&path, &bytes).map_err(|failure| {
                ConfigProblem::Write {
                    failure,
                    backup: backup.clone(),
                }
            })?);
        }
        pending.value
    } else {
        let mut latest = read_config(path.clone(), false)?
            .configured()
            .cloned()
            .unwrap_or_default();
        if let Some(baseline) = pending.baseline {
            let value = pending.value;
            if value.pi_command != baseline.pi_command {
                latest.pi_command = value.pi_command;
            }
            if value.theme != baseline.theme {
                latest.theme = value.theme;
            }
            if value.icon_theme != baseline.icon_theme {
                latest.icon_theme = value.icon_theme;
            }
            if value.light_theme != baseline.light_theme {
                latest.light_theme = value.light_theme;
            }
            if value.dark_theme != baseline.dark_theme {
                latest.dark_theme = value.dark_theme;
            }
            if value.notifications != baseline.notifications {
                latest.notifications = value.notifications;
            }
            if value.shortcuts != baseline.shortcuts {
                latest.shortcuts = value.shortcuts;
            }
            if value.language != baseline.language {
                latest.language = value.language;
            }
            for id in value.keybindings.keys().chain(baseline.keybindings.keys()) {
                if value.keybindings.get(id) != baseline.keybindings.get(id) {
                    if let Some(binding) = value.keybindings.get(id) {
                        latest.keybindings.insert(id.clone(), binding.clone());
                    } else {
                        latest.keybindings.remove(id);
                    }
                }
            }
            latest
        } else {
            // The explicit "write applied settings" action writes all applied values.
            pending.value
        }
    };
    if reset_keybindings {
        value.keybindings.clear();
        value.shortcuts.clear_bindings();
    }
    let value = value.normalized().map_err(ConfigProblem::Validation)?;
    let bytes =
        toml::to_string_pretty(&value).map_err(|e| ConfigProblem::Validation(e.to_string()))?;
    persistence::write_atomic(&path, bytes.as_bytes()).map_err(|failure| ConfigProblem::Write {
        failure,
        backup: backup.clone(),
    })?;
    Ok(ConfigData {
        path,
        contents: ConfigContents::Configured(value),
        backup,
    })
}

pub(crate) struct ConfigController {
    pub store: ConfigStore,
    pub form: WeakEntity<Form<AppConfig>>,
    pub pi_form: Entity<Form<PiSettings>>,
    pub draining: bool,
    path: Result<PathBuf, String>,
}
impl ConfigController {
    pub fn new(form: &Entity<Form<AppConfig>>, cx: &mut Context<Self>) -> Self {
        Self::at_path(
            form,
            paths::config_dir()
                .map(|p| p.join("config.toml"))
                .map_err(|e| e.to_string()),
            cx,
        )
    }
    pub(crate) fn at_path(
        form: &Entity<Form<AppConfig>>,
        path: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            store: Store::new(cx, ConfigOperation::new()),
            form: form.downgrade(),
            pi_form: cx.new(|_| Form::new(PiSettings::default())),
            draining: false,
            path,
        }
    }
    pub fn busy(&self, cx: &App) -> bool {
        self.draining || self.store.read(cx, |op| op.is_running())
    }
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref().ok()
    }
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.start(ConfigRepair::Reload, None, cx);
    }
    pub fn is_onboarding(&self, cx: &App) -> bool {
        self.store.read(cx, |op| {
            op.data().is_some_and(|data| data.configured().is_none())
        })
    }
    pub fn preferences(&self, cx: &App) -> AppConfig {
        self.store
            .read(cx, |op| op.data().and_then(ConfigData::configured).cloned())
            .unwrap_or_else(|| {
                self.form
                    .upgrade()
                    .map(|form| AppConfig::ROOT.get(&form, cx))
                    .unwrap_or_default()
            })
    }
    pub fn set_preference(&mut self, change: PreferenceChange, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        if self.is_onboarding(cx) {
            if let Some(form) = self.form.upgrade() {
                let mut draft = AppConfig::ROOT.get(&form, cx);
                change.apply(&mut draft);
                AppConfig::ROOT.set(&form, draft, cx);
            }
            return;
        }
        let baseline = self.preferences(cx);
        let mut value = baseline.clone();
        change.clone().apply(&mut value);
        if value == baseline {
            return;
        }
        self.start(
            ConfigRepair::SavePreference(change),
            Some(PendingConfig {
                value,
                baseline: Some(baseline),
                version: None,
            }),
            cx,
        );
    }
    pub fn submit_pi(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.busy(cx) {
            return Ok(());
        }
        let (version, draft) = self
            .pi_form
            .update(cx, |form, cx| form.prepare(cx))
            .map_err(|_| "error-config-validation".to_owned())?
            .into_parts();
        let baseline = self.preferences(cx);
        let mut value = baseline.clone();
        value.pi_command = draft.normalized()?.command;
        self.start(
            ConfigRepair::SavePi,
            Some(PendingConfig {
                value,
                baseline: Some(baseline),
                version: Some(version),
            }),
            cx,
        );
        Ok(())
    }
    pub fn submit_draft(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.busy(cx) {
            return Ok(());
        }
        let (version, value) = self
            .form
            .update(cx, |form, cx| form.prepare(cx))
            .map_err(|_| "error-config-validation".to_owned())?
            .map_err(|_| "error-config-validation".to_owned())?
            .into_parts();
        let pending = PendingConfig {
            value: value.normalized()?,
            baseline: Some(self.store.read(cx, |op| {
                op.data()
                    .and_then(ConfigData::configured)
                    .cloned()
                    .unwrap_or_default()
            })),
            version: Some(version),
        };
        self.start(ConfigRepair::SaveDraft, Some(pending), cx);
        Ok(())
    }
    pub fn write_committed(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let pending = self.store.read(cx, |op| {
            let d = op.data()?;
            let value = d.configured()?.clone();
            Some(PendingConfig {
                value,
                baseline: None,
                version: None,
            })
        });
        if let Some(pending) = pending {
            self.start(ConfigRepair::WriteCommitted, Some(pending), cx);
        }
    }
    pub fn repair(&mut self, action: ConfigRepair, cx: &mut Context<Self>) -> Result<(), String> {
        if self.busy(cx) {
            return Ok(());
        }
        match action {
            ConfigRepair::Reload => self.reload(cx),
            ConfigRepair::SaveDraft => return self.submit_draft(cx),
            ConfigRepair::SavePi => return self.submit_pi(cx),
            ConfigRepair::SavePreference(change) => self.set_preference(change, cx),
            ConfigRepair::WriteCommitted => self.write_committed(cx),
            ConfigRepair::BackupAndReset => {
                let pending = PendingConfig {
                    value: AppConfig::default(),
                    baseline: None,
                    version: None,
                };
                self.start(action, Some(pending), cx);
            }
        }
        Ok(())
    }
    fn start(
        &mut self,
        action: ConfigRepair,
        pending: Option<PendingConfig>,
        cx: &mut Context<Self>,
    ) {
        if self.busy(cx) {
            return;
        }
        if matches!(action, ConfigRepair::BackupAndReset)
            && !self.store.read(cx, |op| op.problem().is_some())
        {
            return;
        }
        let version = pending.as_ref().and_then(|p| p.version);
        let rebase_setup = matches!(
            action,
            ConfigRepair::Reload | ConfigRepair::SaveDraft | ConfigRepair::BackupAndReset
        );
        let rebase_pi = rebase_setup || matches!(action, ConfigRepair::SavePi);
        let pi_version = matches!(action, ConfigRepair::SavePi)
            .then_some(version)
            .flatten();
        let reset = matches!(action, ConfigRepair::BackupAndReset);
        let reset_keybindings = matches!(
            action,
            ConfigRepair::SavePreference(PreferenceChange::ResetKeybindings)
        );
        let backup = self
            .store
            .read(cx, |op| op.problem().and_then(|p| p.backup().cloned()));
        let require_configured = self.store.read(cx, |op| {
            op.data().and_then(ConfigData::configured).is_some()
        });
        let path = self
            .store
            .read(cx, |op| op.data().map(|d| d.path.clone()))
            .map(Ok)
            .unwrap_or_else(|| self.path.clone());
        let registration = pending
            .as_ref()
            .map(|p| crate::app::shortcuts::prepare(&p.value, cx))
            .transpose();
        let started = std::time::Instant::now();
        let task = cx.spawn(async move |owner, cx| {
            let result = smol::unblock(move || {
                registration.map_err(ConfigProblem::Validation)?;
                let path = path.map_err(ConfigProblem::Read)?;
                match pending {
                    Some(pending) => write_config(path, pending, reset, reset_keybindings, backup),
                    None => read_config(path, require_configured),
                }
            })
            .await;
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                success = result.is_ok(),
                problem = result.as_ref().err().map(ConfigProblem::key),
                "configuration operation completed"
            );
            let _ = owner.update(cx, |owner, cx| {
                let value = result
                    .as_ref()
                    .ok()
                    .map(|d| d.configured().cloned().unwrap_or_default());
                let applied = value.clone().unwrap_or_else(|| owner.preferences(cx));
                crate::app::shortcuts::apply(&applied, cx);
                owner.store.update(cx, |op| op.transition(Complete(result)));
                if let Some(value) = value {
                    if rebase_pi {
                        owner.pi_form.update(cx, |form, cx| {
                            let draft = PiSettings {
                                command: value.pi_command.clone(),
                            };
                            if let Some(version) = pi_version {
                                form.rebase_if_current(version, draft, cx);
                            } else {
                                form.rebase(draft, cx);
                            }
                        });
                    }
                    if rebase_setup {
                        let _ = owner.form.update(cx, |form, cx| {
                            if let Some(version) = version {
                                form.rebase_if_current(version, value, cx);
                            } else {
                                form.rebase(value, cx);
                            }
                        });
                    }
                }
                cx.notify();
            });
        });
        self.store.update(cx, |op| match op {
            ConfigOperation::Idle(_) => op.transition(Load(task)),
            ConfigOperation::Ready(_) => op.transition(Refresh(task)),
            _ => op.transition(Repair {
                repair: action,
                task,
            }),
        });
        cx.notify();
    }
}
