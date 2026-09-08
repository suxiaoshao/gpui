use crate::foundation::{
    paths,
    persistence::{self, Failure},
};
use gpui_form::{Form, FormSchema};
use gpui_kit::*;
use gpui_operation::{Complete, Load, Refresh, Repair, Transition, repair};
use gpui_store::Store;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

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
    pub source_bytes: Option<Vec<u8>>,
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
#[derive(Clone, Debug)]
pub(crate) struct PendingConfig {
    pub value: AppConfig,
    pub bytes: Vec<u8>,
    pub expected_source: Option<Vec<u8>>,
    pub rebase_on_success: bool,
}
#[derive(Debug, thiserror::Error)]
#[error("{key}: {detail}")]
pub(crate) struct ConfigProblem {
    pub key: &'static str,
    pub detail: String,
    pub pending: Option<Arc<PendingConfig>>,
    pub backup: Option<PathBuf>,
    pub reconcile: bool,
    pub conflict: bool,
}
impl ConfigProblem {
    fn new(key: &'static str, detail: impl ToString) -> Self {
        Self {
            key,
            detail: detail.to_string(),
            pending: None,
            backup: None,
            reconcile: false,
            conflict: false,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum ConfigRepair {
    Reload,
    RetryWrite,
    BackupAndReset,
    BackupAndWrite,
    SubmitReplacement(Arc<PendingConfig>),
}
pub(crate) type ConfigOperation =
    repair::Operation<ConfigData, ConfigProblem, ConfigRepair, Task<()>>;
pub(crate) type ConfigStore = Store<ConfigOperation>;

pub(crate) fn read_config(
    path: PathBuf,
    require_configured: bool,
) -> Result<ConfigData, ConfigProblem> {
    let bytes = persistence::read(&path).map_err(|e| ConfigProblem::new("error-config-read", e))?;
    let contents = match &bytes {
        None if require_configured => {
            return Err(ConfigProblem::new("error-config-read", "file removed"));
        }
        None => ConfigContents::Missing,
        Some(bytes) => {
            let text = std::str::from_utf8(bytes)
                .map_err(|e| ConfigProblem::new("error-config-parse", e))?;
            let value: AppConfig = toml::from_str(text).map_err(|_| {
                ConfigProblem::new("error-config-parse", "invalid TOML or unsupported fields")
            })?;
            ConfigContents::Configured(
                value
                    .normalized()
                    .map_err(|e| ConfigProblem::new("error-config-validation", e))?,
            )
        }
    };
    Ok(ConfigData {
        path,
        contents,
        source_bytes: bytes,
        backup: None,
    })
}
fn write_config(
    path: PathBuf,
    mut pending: Arc<PendingConfig>,
    overwrite: bool,
    mut backup: Option<PathBuf>,
) -> Result<ConfigData, ConfigProblem> {
    let result = (|| {
        let current;
        let expected = if overwrite {
            current = persistence::read(&path)?;
            if let Some(bytes) = &current {
                backup = Some(persistence::backup(&path, bytes)?);
            }
            pending = Arc::new(PendingConfig {
                expected_source: current.clone(),
                ..(*pending).clone()
            });
            current.as_deref()
        } else {
            pending.expected_source.as_deref()
        };
        persistence::replace(&path, expected, &pending.bytes)
    })();
    result.map_err(|e: Failure| ConfigProblem {
        key: if matches!(e, Failure::Conflict) {
            "error-config-conflict"
        } else {
            "error-config-write"
        },
        detail: e.to_string(),
        reconcile: matches!(e, Failure::NeedsReconcile(_)),
        conflict: matches!(e, Failure::Conflict),
        pending: Some(pending.clone()),
        backup: backup.clone(),
    })?;
    Ok(ConfigData {
        path,
        contents: ConfigContents::Configured(pending.value.clone()),
        source_bytes: Some(pending.bytes.clone()),
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
        self.start(ConfigRepair::Reload, None, false, true, cx);
    }
    pub fn submit(&mut self, value: AppConfig, cx: &mut Context<Self>) -> Result<(), String> {
        if self.busy(cx) {
            return Ok(());
        }
        let value = value.normalized()?;
        let bytes = toml::to_string_pretty(&value)
            .map_err(|e| e.to_string())?
            .into_bytes();
        let expected_source = self
            .store
            .read(cx, |op| op.data().and_then(|d| d.source_bytes.clone()));
        let pending = Arc::new(PendingConfig {
            value,
            bytes,
            expected_source,
            rebase_on_success: true,
        });
        self.start(
            ConfigRepair::SubmitReplacement(pending.clone()),
            Some(pending),
            false,
            true,
            cx,
        );
        Ok(())
    }
    pub fn write_committed(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let pending = self.store.read(cx, |op| {
            let d = op.data()?;
            let value = d.configured()?.clone();
            Some(Arc::new(PendingConfig {
                bytes: toml::to_string_pretty(&value).ok()?.into_bytes(),
                value,
                expected_source: d.source_bytes.clone(),
                rebase_on_success: false,
            }))
        });
        if let Some(pending) = pending {
            self.start(
                ConfigRepair::SubmitReplacement(pending.clone()),
                Some(pending),
                false,
                false,
                cx,
            );
        }
    }
    pub fn repair(&mut self, action: ConfigRepair, cx: &mut Context<Self>) {
        let pending = self
            .store
            .read(cx, |op| op.problem().and_then(|p| p.pending.clone()));
        match action {
            ConfigRepair::Reload => self.reload(cx),
            ConfigRepair::RetryWrite => self.start(action, pending, false, true, cx),
            ConfigRepair::BackupAndWrite => self.start(action, pending, true, true, cx),
            ConfigRepair::BackupAndReset => {
                let value = AppConfig::default();
                let pending = Arc::new(PendingConfig {
                    bytes: toml::to_string_pretty(&value)
                        .expect("config serialization")
                        .into_bytes(),
                    value,
                    expected_source: None,
                    rebase_on_success: true,
                });
                self.start(action, Some(pending), true, true, cx);
            }
            ConfigRepair::SubmitReplacement(pending) => self.start(
                ConfigRepair::SubmitReplacement(pending.clone()),
                Some(pending),
                false,
                true,
                cx,
            ),
        }
    }
    fn start(
        &mut self,
        action: ConfigRepair,
        pending: Option<Arc<PendingConfig>>,
        overwrite: bool,
        rebase: bool,
        cx: &mut Context<Self>,
    ) {
        if self.busy(cx) {
            return;
        }
        let allowed = self.store.read(cx, |op| match op.problem() {
            Some(p) if p.reconcile => matches!(action, ConfigRepair::Reload),
            Some(p) if p.conflict => {
                matches!(action, ConfigRepair::Reload | ConfigRepair::BackupAndWrite)
            }
            Some(_) => !matches!(action, ConfigRepair::BackupAndWrite),
            None => !matches!(
                action,
                ConfigRepair::RetryWrite
                    | ConfigRepair::BackupAndWrite
                    | ConfigRepair::BackupAndReset
            ),
        });
        if !allowed {
            return;
        }
        let rebase = pending
            .as_ref()
            .map(|p| p.rebase_on_success)
            .unwrap_or(rebase);
        let backup = self
            .store
            .read(cx, |op| op.problem().and_then(|p| p.backup.clone()));
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
                let path = path.map_err(|e| ConfigProblem::new("error-config-read", e))?;
                match pending {
                    Some(pending) => write_config(path, pending, overwrite, backup),
                    None => read_config(path, require_configured),
                }
            })
            .await;
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                success = result.is_ok(),
                problem = result.as_ref().err().map(|e| e.key),
                "configuration operation completed"
            );
            let _ = owner.update(cx, |owner, cx| {
                let value = result
                    .as_ref()
                    .ok()
                    .map(|d| d.configured().cloned().unwrap_or_default());
                owner.store.update(cx, |op| op.transition(Complete(result)));
                if rebase && let Some(value) = value {
                    let _ = owner.form.update(cx, |form, cx| form.rebase(value, cx));
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
        AppConfig, ConfigContents, ConfigController, ConfigData, ConfigOperation, ConfigProblem,
        ConfigRepair, ThemeMode, read_config,
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
            read_config(path, false).unwrap_err().key,
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
    async fn committed_write_keeps_draft_and_conflict_requires_explicit_repair(
        cx: &mut TestAppContext,
    ) {
        cx.executor().allow_parking();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let owner = cx.new(|cx| ConfigController::at_path(&form, Ok(path.clone()), cx));
        owner.update(cx, |owner, cx| owner.reload(cx));
        settled(&owner, cx).await;
        assert!(!path.exists());
        owner.update(cx, |owner, cx| {
            owner.submit(AppConfig::default(), cx).unwrap();
            // All competing commands are rejected while the first save owns its task.
            owner
                .submit(
                    AppConfig {
                        theme: ThemeMode::Dark,
                        ..Default::default()
                    },
                    cx,
                )
                .unwrap();
            owner.reload(cx);
            owner.write_committed(cx);
            assert!(owner.store.read(cx, |op| op.is_running()));
        });
        settled(&owner, cx).await;
        assert_eq!(
            read_config(path.clone(), true)
                .unwrap()
                .configured()
                .unwrap()
                .theme,
            ThemeMode::System
        );
        cx.update(|cx| AppConfig::THEME.set(&form, ThemeMode::Dark, cx));
        owner.update(cx, |owner, cx| owner.write_committed(cx));
        settled(&owner, cx).await;
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Dark
        );
        assert!(form.read_with(cx, |form, _| form.is_dirty()));
        std::fs::write(&path, "theme = 'light'\n").unwrap();
        owner.update(cx, |owner, cx| owner.write_committed(cx));
        settled(&owner, cx).await;
        owner.update(cx, |owner, cx| {
            assert!(owner.store.read(cx, |op| op.problem().unwrap().conflict));
            owner.repair(ConfigRepair::RetryWrite, cx);
            assert!(!owner.store.read(cx, |op| op.is_running()));
            owner.repair(ConfigRepair::BackupAndWrite, cx);
        });
        settled(&owner, cx).await;
        assert!(form.read_with(cx, |form, _| form.is_dirty()));
        assert_eq!(
            cx.update(|cx| AppConfig::THEME.get(&form, cx)),
            ThemeMode::Dark
        );
        assert_eq!(
            read_config(path.clone(), true)
                .unwrap()
                .configured()
                .unwrap()
                .theme,
            ThemeMode::System
        );
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("config-backup-")
            })
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).unwrap(),
            "theme = 'light'\n"
        );
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
            )
        });
    }
    #[gpui::test]
    fn uncertain_commit_only_allows_reload(cx: &mut TestAppContext) {
        use gpui_operation::{Settle, Transition};
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let dir = tempfile::tempdir().unwrap();
        let owner =
            cx.new(|cx| ConfigController::at_path(&form, Ok(dir.path().join("config.toml")), cx));
        owner.update(cx, |owner, cx| {
            owner.store.update(cx, |op| {
                op.transition(Settle(Ok(ConfigData {
                    path: dir.path().join("config.toml"),
                    contents: ConfigContents::Configured(AppConfig::default()),
                    source_bytes: None,
                    backup: None,
                })));
                let mut problem = ConfigProblem::new("error-config-write", "uncertain commit");
                problem.reconcile = true;
                op.transition(Settle(Err(problem)));
            });
            owner.submit(AppConfig::default(), cx).unwrap();
            owner.write_committed(cx);
            owner.repair(ConfigRepair::BackupAndReset, cx);
            assert!(
                owner
                    .store
                    .read(cx, |op| matches!(op, ConfigOperation::Degraded(_)))
            );
        });
    }
}
