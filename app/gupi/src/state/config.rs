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
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FormSchema)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct AppConfig {
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub keybindings: super::keybindings::Overrides,
    pub shortcuts: super::shortcuts::Shortcuts,
    pub pi_command: Option<String>,
    pub theme: ThemeMode,
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
    pub language: AppLanguage,
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
    Shortcuts(super::shortcuts::Shortcuts),
    Language(AppLanguage),
    Theme(ThemeMode),
    LightTheme(Option<String>),
    DarkTheme(Option<String>),
}
impl PreferenceChange {
    fn apply(self, config: &mut AppConfig) {
        match self {
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
            if value.light_theme != baseline.light_theme {
                latest.light_theme = value.light_theme;
            }
            if value.dark_theme != baseline.dark_theme {
                latest.dark_theme = value.dark_theme;
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
    fn at_path(
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
#[cfg(test)]
mod tests {
    use super::{
        AppConfig, AppLanguage, ConfigController, ConfigRepair, PiSettings, PreferenceChange,
        ThemeMode, read_config,
    };
    use gpui_form::Form;
    use gpui_kit as gpui;
    use gpui_kit::{AppContext, Entity, TestAppContext};
    #[test]
    fn missing_is_not_configured_and_reload_retains_missing_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(
            read_config(path.clone(), false)
                .unwrap()
                .configured()
                .is_none()
        );
        assert!(!path.exists());
        assert!(read_config(path.clone(), true).is_err());
        std::fs::write(&path, "unexpected = true").unwrap();
        assert_eq!(
            read_config(path, false).unwrap_err().key(),
            "error-config-parse"
        );
    }
    async fn settled(owner: &Entity<ConfigController>, cx: &mut TestAppContext) {
        cx.condition(owner, |owner, cx| {
            !owner.store.read(cx, |op| op.is_running())
        })
        .await;
    }

    #[gpui::test]
    async fn shortcut_save_merges_only_the_selected_override(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = 'system'\n").unwrap();
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        std::fs::write(
            &path,
            "theme = 'dark'\n[keybindings]\nquick_open = 'secondary-alt-p'\n",
        )
        .unwrap();
        owner.update(cx, |owner, cx| {
            owner.set_preference(
                PreferenceChange::Keybinding("new".into(), Some("secondary-shift-n".into())),
                cx,
            )
        });
        settled(&owner, cx).await;
        let saved = read_config(path.clone(), true)
            .unwrap()
            .configured()
            .unwrap()
            .clone();
        assert_eq!(saved.theme, ThemeMode::Dark);
        assert_eq!(
            saved.keybindings.get("new").map(String::as_str),
            Some("secondary-shift-n")
        );
        assert_eq!(
            saved.keybindings.get("quick_open").map(String::as_str),
            Some("secondary-alt-p")
        );
        owner.update(cx, |owner, cx| {
            owner.set_preference(PreferenceChange::Keybinding("new".into(), None), cx)
        });
        settled(&owner, cx).await;
        let saved = read_config(path.clone(), true)
            .unwrap()
            .configured()
            .unwrap()
            .clone();
        assert!(!saved.keybindings.contains_key("new"));
        assert!(saved.keybindings.contains_key("quick_open"));
        // Reset every override, including changes written externally since the last read.
        std::fs::write(
            &path,
            "theme = 'light'\n[keybindings]\nquick_open = 'secondary-alt-p'\nnew = ''\n[shortcuts]\nlauncher = 'ctrl-alt-t'\n[[shortcuts.tasks]]\nid = 'test'\nname = 'Test'\nbinding = 'ctrl-alt-y'\ntemplate = '/test.md'\n",
        )
        .unwrap();
        owner.update(cx, |owner, cx| {
            owner.set_preference(PreferenceChange::ResetKeybindings, cx);
        });
        settled(&owner, cx).await;
        let saved = read_config(path, true)
            .unwrap()
            .configured()
            .unwrap()
            .clone();
        assert_eq!(saved.theme, ThemeMode::Light);
        assert!(saved.keybindings.is_empty());
        assert!(!saved.shortcuts.has_bindings());
        assert_eq!(saved.shortcuts.tasks.len(), 1);
        assert_eq!(saved.shortcuts.tasks[0].name, "Test");
        assert!(saved.shortcuts.tasks[0].enabled);
    }

    #[gpui::test]
    async fn saving_merges_changed_fields_into_latest_file(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "pi_command = 'old-pi'\nlight_theme = 'old-light'\n").unwrap();
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        cx.update(|cx| {
            AppConfig::THEME.set(&form, ThemeMode::Dark, cx);
            AppConfig::LIGHT_THEME.set(&form, None, cx);
            AppConfig::DARK_THEME.set(&form, Some("chosen-dark".into()), cx);
        });
        std::fs::write(&path, "pi_command = 'external-pi'\ntheme = 'light'\nlight_theme = 'external-light'\nlanguage = 'chinese'\n").unwrap();
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        let saved = read_config(path.clone(), true)
            .unwrap()
            .configured()
            .unwrap()
            .clone();
        assert_eq!(
            saved,
            AppConfig {
                pi_command: Some("external-pi".into()),
                theme: ThemeMode::Dark,
                light_theme: None,
                dark_theme: Some("chosen-dark".into()),
                language: AppLanguage::Chinese,
                keybindings: Default::default(),
                shortcuts: Default::default(),
            }
        );
        assert!(!form.read_with(cx, |form, _| form.is_dirty()));
        assert_eq!(
            cx.update(|cx| AppConfig::PI_COMMAND.get(&form, cx)),
            saved.pi_command
        );
        cx.update(|cx| {
            AppConfig::PI_COMMAND.set(&form, None, cx);
            AppConfig::LANGUAGE.set(&form, AppLanguage::English, cx);
        });
        std::fs::write(&path, "pi_command = 'new-external-pi'\ntheme = 'light'\n").unwrap();
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        let saved = read_config(path, true)
            .unwrap()
            .configured()
            .unwrap()
            .clone();
        assert_eq!(saved.pi_command, None);
        assert_eq!(saved.language, AppLanguage::English);
        assert_eq!(saved.theme, ThemeMode::Light);
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .all(|entry| entry.unwrap().file_name() == "config.toml")
        );
    }

    #[gpui::test]
    async fn first_save_preserves_settings_written_after_initial_load(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        cx.update(|cx| AppConfig::THEME.set(&form, ThemeMode::Dark, cx));
        std::fs::write(&path, "pi_command = 'external-pi'\n").unwrap();
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        let saved = read_config(path, true)
            .unwrap()
            .configured()
            .unwrap()
            .clone();
        assert_eq!(saved.theme, ThemeMode::Dark);
        assert_eq!(saved.pi_command.as_deref(), Some("external-pi"));
    }

    #[gpui::test]
    async fn writing_applied_settings_keeps_draft_and_serializes_operations(
        cx: &mut TestAppContext,
    ) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        owner.update(cx, |owner, cx| {
            owner.submit_draft(cx).unwrap();
            owner.submit_draft(cx).unwrap();
            owner.reload(cx);
            owner.write_committed(cx);
            assert!(owner.busy(cx));
        });
        settled(&owner, cx).await;
        cx.update(|cx| AppConfig::THEME.set(&form, ThemeMode::Dark, cx));
        std::fs::write(&path, "theme = 'light'\n").unwrap();
        owner.update(cx, |owner, cx| owner.write_committed(cx));
        settled(&owner, cx).await;
        assert_eq!(
            read_config(path.clone(), true)
                .unwrap()
                .configured()
                .unwrap()
                .theme,
            ThemeMode::System
        );
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Dark
        );
        assert!(form.read_with(cx, |form, _| form.is_dirty()));
        std::fs::write(&path, "broken = [").unwrap();
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Dark
        );
        owner.read_with(cx, |owner, cx| {
            assert_eq!(
                owner
                    .store
                    .read(cx, |op| op.data().unwrap().configured().unwrap().theme),
                ThemeMode::System
            );
        });
    }

    #[gpui::test]
    async fn malformed_file_is_only_replaced_by_explicit_backup_reset(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = 'system'\n").unwrap();
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        cx.update(|cx| AppConfig::THEME.set(&form, ThemeMode::Dark, cx));
        let broken = "broken = [";
        std::fs::write(&path, broken).unwrap();
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Dark
        );
        owner.read_with(cx, |owner, cx| {
            assert_eq!(
                owner.store.read(cx, |op| op.problem().unwrap().key()),
                "error-config-parse"
            )
        });
        owner.update(cx, |owner, cx| {
            owner.repair(ConfigRepair::BackupAndReset, cx).unwrap()
        });
        settled(&owner, cx).await;
        let backup = owner.read_with(cx, |owner, cx| {
            owner
                .store
                .read(cx, |op| op.data().unwrap().backup.clone().unwrap())
        });
        assert_eq!(std::fs::read_to_string(backup).unwrap(), broken);
        assert_eq!(
            read_config(path, true).unwrap().configured().unwrap(),
            &AppConfig::default()
        );
        assert!(!form.read_with(cx, |form, _| form.is_dirty()));
    }
    #[gpui::test]
    async fn saving_after_failure_uses_the_current_draft(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("config");
        let path = parent.join("config.toml");
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;

        // A file in place of the directory causes a real pre-commit write failure.
        std::fs::write(&parent, "blocked").unwrap();
        cx.update(|cx| AppConfig::THEME.set(&form, ThemeMode::Light, cx));
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        assert!(owner.read_with(cx, |owner, cx| {
            owner.store.read(cx, |op| op.problem().is_some())
        }));
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Light
        );

        cx.update(|cx| AppConfig::THEME.set(&form, ThemeMode::Dark, cx));
        std::fs::remove_file(&parent).unwrap();
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        assert_eq!(
            read_config(path, true).unwrap().configured().unwrap().theme,
            ThemeMode::Dark
        );
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Dark
        );
        assert!(!form.read_with(cx, |form, _| form.is_dirty()));
    }

    #[gpui::test]
    async fn preferences_never_submit_or_rebase_the_pi_editor(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "pi_command = 'saved-pi'\n").unwrap();
        let setup = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&setup, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        let pi = owner.read_with(cx, |owner, _| owner.pi_form.clone());
        cx.update(|cx| PiSettings::COMMAND.set(&pi, Some("draft-pi".into()), cx));
        for change in [
            PreferenceChange::Theme(ThemeMode::Dark),
            PreferenceChange::Language(AppLanguage::English),
            PreferenceChange::LightTheme(Some("test-light".into())),
            PreferenceChange::DarkTheme(Some("test-dark".into())),
        ] {
            owner.update(cx, |owner, cx| owner.set_preference(change, cx));
            settled(&owner, cx).await;
            assert_eq!(
                read_config(path.clone(), true)
                    .unwrap()
                    .configured()
                    .unwrap()
                    .pi_command
                    .as_deref(),
                Some("saved-pi")
            );
            assert_eq!(
                cx.update(|cx| PiSettings::COMMAND.get(&pi, cx)),
                Some("draft-pi".into())
            );
            assert!(pi.read_with(cx, |form, _| form.is_dirty()));
        }
        owner.update(cx, |owner, cx| owner.submit_pi(cx).unwrap());
        settled(&owner, cx).await;
        let saved = read_config(path, true).unwrap();
        let saved = saved.configured().unwrap();
        assert_eq!(saved.pi_command.as_deref(), Some("draft-pi"));
        assert_eq!(saved.theme, ThemeMode::Dark);
        assert_eq!(saved.language, AppLanguage::English);
        assert_eq!(saved.light_theme.as_deref(), Some("test-light"));
        assert_eq!(saved.dark_theme.as_deref(), Some("test-dark"));
        assert!(!pi.read_with(cx, |form, _| form.is_dirty()));
    }

    #[gpui::test]
    async fn onboarding_preferences_wait_for_explicit_confirmation(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let setup = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&setup, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        owner.update(cx, |owner, cx| {
            owner.set_preference(PreferenceChange::Language(AppLanguage::Chinese), cx);
            owner.set_preference(PreferenceChange::Theme(ThemeMode::Light), cx);
            assert!(!owner.busy(cx));
            assert_eq!(owner.preferences(cx).language, AppLanguage::Chinese);
        });
        assert!(!path.exists());
        owner.update(cx, |owner, cx| owner.submit_draft(cx).unwrap());
        settled(&owner, cx).await;
        let saved = read_config(path, true).unwrap();
        assert_eq!(saved.configured().unwrap().language, AppLanguage::Chinese);
        assert_eq!(saved.configured().unwrap().theme, ThemeMode::Light);
    }

    #[gpui::test]
    async fn failed_preference_save_keeps_applied_value_and_pi_input(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = 'light'\n").unwrap();
        let setup = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&setup, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        let pi = owner.read_with(cx, |owner, _| owner.pi_form.clone());
        cx.update(|cx| PiSettings::COMMAND.set(&pi, Some("draft-pi".into()), cx));
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        owner.update(cx, |owner, cx| {
            owner.set_preference(PreferenceChange::Theme(ThemeMode::Dark), cx)
        });
        settled(&owner, cx).await;
        owner.read_with(cx, |owner, cx| {
            assert!(!owner.busy(cx));
            assert!(owner.store.read(cx, |op| op.problem().is_some()));
            assert_eq!(owner.preferences(cx).theme, ThemeMode::Light);
        });
        assert_eq!(
            cx.update(|cx| PiSettings::COMMAND.get(&pi, cx)),
            Some("draft-pi".into())
        );
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, "theme = 'light'\n").unwrap();
        owner.update(cx, |owner, cx| {
            owner.set_preference(PreferenceChange::Theme(ThemeMode::Dark), cx)
        });
        settled(&owner, cx).await;
        assert_eq!(
            read_config(path, true).unwrap().configured().unwrap().theme,
            ThemeMode::Dark
        );
        assert!(pi.read_with(cx, |form, _| form.is_dirty()));
    }

    #[gpui::test]
    async fn pi_save_does_not_replace_newer_edits(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = 'system'\n").unwrap();
        let setup = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&setup, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        let pi = owner.read_with(cx, |owner, _| owner.pi_form.clone());
        owner.update(cx, |owner, cx| {
            PiSettings::COMMAND.set(&pi, Some("first-pi".into()), cx);
            owner.submit_pi(cx).unwrap();
            PiSettings::COMMAND.set(&pi, Some("newer-pi".into()), cx);
        });
        settled(&owner, cx).await;
        assert_eq!(
            read_config(path, true)
                .unwrap()
                .configured()
                .unwrap()
                .pi_command
                .as_deref(),
            Some("first-pi")
        );
        assert_eq!(
            cx.update(|cx| PiSettings::COMMAND.get(&pi, cx)),
            Some("newer-pi".into())
        );
        assert!(pi.read_with(cx, |form, _| form.is_dirty()));
    }
}
