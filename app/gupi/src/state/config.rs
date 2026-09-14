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
    pub pi_command: Option<String>,
    pub theme: ThemeMode,
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
    pub language: AppLanguage,
}
impl AppConfig {
    pub fn normalized(mut self) -> Result<Self, String> {
        self.pi_command = self
            .pi_command
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if let Some(command) = &self.pi_command {
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
    mut backup: Option<PathBuf>,
) -> Result<ConfigData, ConfigProblem> {
    let value = if reset {
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
            if value.language != baseline.language {
                latest.language = value.language;
            }
            latest
        } else {
            // The explicit "write applied settings" action writes all applied values.
            pending.value
        }
    };
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
            draining: false,
            path,
        }
    }
    pub fn busy(&self, cx: &App) -> bool {
        self.draining || self.store.read(cx, |op| op.is_running())
    }
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.start(ConfigRepair::Reload, None, cx);
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
        let rebase = !matches!(action, ConfigRepair::WriteCommitted);
        let reset = matches!(action, ConfigRepair::BackupAndReset);
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
        let started = std::time::Instant::now();
        let task = cx.spawn(async move |owner, cx| {
            let result = smol::unblock(move || {
                let path = path.map_err(ConfigProblem::Read)?;
                match pending {
                    Some(pending) => write_config(path, pending, reset, backup),
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
                owner.store.update(cx, |op| op.transition(Complete(result)));
                if rebase && let Some(value) = value {
                    let _ = owner.form.update(cx, |form, cx| {
                        if let Some(version) = version {
                            form.rebase_if_current(version, value, cx);
                        } else {
                            form.rebase(value, cx);
                        }
                    });
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
    use super::{AppConfig, AppLanguage, ConfigController, ConfigRepair, ThemeMode, read_config};
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
}
