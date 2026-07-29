pub(crate) mod operation;
pub(crate) mod session;

use std::{
    collections::VecDeque,
    fmt,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{App, AppContext, BorrowAppContext, Entity, Global, Subscription};
use gpui_operation::Transition;
use gpui_store::{Select, Store};
use jaco_agent::AgentPersistence;
#[cfg(test)]
use jaco_db::FreshRepository;
use jaco_db::{DbError, FreshStore};

use crate::{
    errors::{JacoError, JacoResult},
    foundation::persistence::FileLock,
    state::config,
};

pub(crate) use self::operation::{DatabaseOperation, DatabasePhase};
use self::{operation::DatabaseMessage, session::DatabaseSession};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DatabaseTarget {
    pub(crate) data_dir: PathBuf,
    pub(crate) database_path: PathBuf,
}

#[derive(Clone, Copy, Default)]
struct SelectDatabaseTarget;

impl Select<config::ConfigOperation> for SelectDatabaseTarget {
    type Output = Option<DatabaseTarget>;

    fn select(&self, operation: &config::ConfigOperation) -> Self::Output {
        operation.data().map(DatabaseTarget::from_config)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectDatabaseReady;

impl Select<DatabaseResource> for SelectDatabaseReady {
    type Output = bool;

    fn select(&self, resource: &DatabaseResource) -> Self::Output {
        matches!(
            resource,
            DatabaseResource::Bound {
                operation: DatabaseOperation::Ready(_),
                ..
            }
        )
    }
}

impl DatabaseTarget {
    pub(crate) fn from_config(data: &config::ConfigData) -> Self {
        let data_dir = data.data_dir().to_path_buf();
        Self {
            database_path: data_dir.join(jaco_db::DATABASE_FILE),
            data_dir,
        }
    }

    fn lock_path(&self) -> PathBuf {
        self.data_dir.join("jaco.sqlite3.lock")
    }
}

pub(crate) struct DatabaseTargetLease {
    _lock: FileLock,
}

impl DatabaseTargetLease {
    fn acquire(target: &DatabaseTarget) -> Result<Self, DatabaseProblem> {
        FileLock::acquire(&target.lock_path())
            .map(|lock| Self { _lock: lock })
            .map_err(|source| DatabaseProblem::InUse {
                target: target.clone(),
                message: source.to_string(),
            })
    }
}

pub(crate) struct DatabaseData {
    pub(crate) session: Entity<DatabaseSession>,
}

impl Clone for DatabaseData {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
        }
    }
}

impl fmt::Debug for DatabaseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseData").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) enum DatabaseProblem {
    Open {
        target: DatabaseTarget,
        source: DbError,
    },
    InUse {
        target: DatabaseTarget,
        message: String,
    },
    Backup {
        backup_dir: PathBuf,
        message: String,
    },
    CreateFresh {
        target: DatabaseTarget,
        message: String,
    },
}

impl DatabaseProblem {
    pub(crate) fn can_create_fresh(&self) -> bool {
        !matches!(self, Self::InUse { .. })
    }
}

impl fmt::Display for DatabaseProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { target, source } => {
                write!(
                    f,
                    "could not open {}: {source}",
                    target.database_path.display()
                )
            }
            Self::InUse { target, message } => {
                write!(f, "{} is in use: {message}", target.database_path.display())
            }
            Self::Backup {
                backup_dir,
                message,
                ..
            } => write!(
                f,
                "could not back up the database to {}: {message}",
                backup_dir.display()
            ),
            Self::CreateFresh { target, message } => write!(
                f,
                "the backup was preserved, but a fresh database could not be created at {}: {message}",
                target.database_path.display()
            ),
        }
    }
}

impl std::error::Error for DatabaseProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseRepair {
    Refresh,
    BackupAndCreateFresh { backup_dir: PathBuf },
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum DatabaseResource {
    AwaitingConfig,
    Bound {
        target: DatabaseTarget,
        operation: DatabaseOperation,
    },
}

pub(crate) type DatabaseStore = Store<DatabaseResource>;

struct DatabaseRepairNotices(VecDeque<DatabaseBackupOutcome>);

impl Global for DatabaseRepairNotices {}

struct DatabaseConfigObserver {
    _subscription: Subscription,
}

struct DatabaseConfigObserverGlobal {
    _observer: Entity<DatabaseConfigObserver>,
}

impl Global for DatabaseConfigObserverGlobal {}

pub(crate) fn init_store(cx: &mut App) {
    cx.set_global(DatabaseRepairNotices(VecDeque::new()));
    let target = config::store(cx).read(cx, |operation| {
        operation.data().map(DatabaseTarget::from_config)
    });
    let resource = match target {
        Some(target) => {
            let result = open_initial(&target, cx);
            let mut operation = DatabaseOperation::new();
            operation.transition(DatabaseMessage::Settle(result));
            DatabaseResource::Bound { target, operation }
        }
        None => DatabaseResource::AwaitingConfig,
    };
    DatabaseStore::install_global(cx, resource);
    let observer = cx.new(|cx| {
        let subscription =
            config::store(cx).observe_select(cx, SelectDatabaseTarget, |_observer, target, cx| {
                sync_target(target.clone(), cx)
            });
        DatabaseConfigObserver {
            _subscription: subscription,
        }
    });
    cx.set_global(DatabaseConfigObserverGlobal {
        _observer: observer,
    });
}

fn sync_target(target: Option<DatabaseTarget>, cx: &mut App) {
    let database = store(cx);
    let should_rebind = database.read(cx, |resource| match (resource, &target) {
        (DatabaseResource::AwaitingConfig, None) => false,
        (
            DatabaseResource::Bound {
                target: current, ..
            },
            Some(target),
        ) => current != target,
        _ => true,
    });
    if !should_rebind {
        return;
    }
    match target {
        Some(target) => {
            database.set(
                cx,
                DatabaseResource::Bound {
                    target: target.clone(),
                    operation: DatabaseOperation::new(),
                },
            );
            start_initial_open(target, cx);
        }
        None => database.set(cx, DatabaseResource::AwaitingConfig),
    }
}

fn start_initial_open(target: DatabaseTarget, cx: &mut App) {
    let database = store(cx);
    let completion_store = database.clone();
    let load_target = target.clone();
    let completion_target = target.clone();
    let task = cx.spawn(async move |cx| {
        let opened = smol::unblock(move || {
            let lease = DatabaseTargetLease::acquire(&load_target)?;
            let store = FreshStore::open_or_create_initial(&load_target.database_path).map_err(
                |source| DatabaseProblem::Open {
                    target: load_target.clone(),
                    source,
                },
            )?;
            Ok::<_, DatabaseProblem>((store, lease))
        })
        .await;
        cx.update(|cx| {
            let result = opened.and_then(|(store, lease)| create_data(store, lease, cx));
            completion_store.update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &completion_target
                    && matches!(operation, DatabaseOperation::Loading { .. })
                {
                    operation.transition(DatabaseMessage::Loaded(result));
                }
            });
        });
    });
    database.update(cx, |resource| {
        let DatabaseResource::Bound {
            target: current,
            operation,
        } = resource
        else {
            return;
        };
        if current == &target && matches!(operation, DatabaseOperation::Idle) {
            operation.transition(DatabaseMessage::Load(task));
        }
    });
}

fn open_initial(
    target: &DatabaseTarget,
    cx: &mut impl AppContext,
) -> Result<DatabaseData, DatabaseProblem> {
    let lease = DatabaseTargetLease::acquire(target)?;
    let store = FreshStore::open_or_create_initial(&target.database_path).map_err(|source| {
        DatabaseProblem::Open {
            target: target.clone(),
            source,
        }
    })?;
    create_data(store, lease, cx)
}

fn create_data(
    store: FreshStore,
    lease: DatabaseTargetLease,
    cx: &mut impl AppContext,
) -> Result<DatabaseData, DatabaseProblem> {
    let session = cx.new(|_| DatabaseSession::new(store, lease));
    Ok(DatabaseData { session })
}

pub(crate) fn store(cx: &impl AppContext) -> DatabaseStore {
    DatabaseStore::global(cx)
}

pub(crate) fn is_ready(cx: &impl AppContext) -> bool {
    !crate::app::is_shutting_down()
        && config::store(cx).read(cx, |operation| {
            matches!(operation, config::ConfigOperation::Ready(_))
        })
        && store(cx).read(cx, |resource| {
            matches!(
                resource,
                DatabaseResource::Bound {
                    operation: DatabaseOperation::Ready(_),
                    ..
                }
            )
        })
}

#[cfg(test)]
pub(crate) fn with_ready_repository<R>(
    cx: &impl AppContext,
    command: impl FnOnce(&FreshRepository) -> jaco_db::Result<R>,
) -> jaco_db::Result<R> {
    ensure_config_ready(cx)?;
    let session = store(cx).read(cx, |resource| match resource {
        DatabaseResource::Bound {
            operation: DatabaseOperation::Ready(data),
            ..
        } => Some(data.session.clone()),
        _ => None,
    });
    let session = session.ok_or_else(|| {
        DbError::Invariant("database command requires an exact Ready session".to_string())
    })?;
    let repository = session
        .read_with(cx, |session, _| session.repository())
        .map_err(|error| DbError::Invariant(error.to_string()))?;
    command(&repository)
}

pub(crate) fn ready_agent_persistence(
    cx: &impl AppContext,
) -> jaco_db::Result<Arc<dyn AgentPersistence>> {
    ensure_config_ready(cx)?;
    let session = store(cx).read(cx, |resource| match resource {
        DatabaseResource::Bound {
            operation: DatabaseOperation::Ready(data),
            ..
        } => Some(data.session.clone()),
        _ => None,
    });
    let session = session.ok_or_else(|| {
        DbError::Invariant("agent runtime requires an exact Ready session".to_string())
    })?;
    session
        .read_with(cx, |session, _| session.agent_persistence())
        .map_err(|error| DbError::Invariant(error.to_string()))
}

pub(crate) fn ready_executor(
    cx: &impl AppContext,
) -> jaco_db::Result<crate::database::session::SessionDatabaseExecutor> {
    ensure_config_ready(cx)?;
    let session = store(cx).read(cx, |resource| match resource {
        DatabaseResource::Bound {
            operation: DatabaseOperation::Ready(data),
            ..
        } => Some(data.session.clone()),
        _ => None,
    });
    let session = session.ok_or_else(|| {
        DbError::Invariant("database job requires an exact Ready session".to_string())
    })?;
    session
        .read_with(cx, |session, _| session.executor())
        .map_err(|error| DbError::Invariant(error.to_string()))
}

fn ensure_config_ready(cx: &impl AppContext) -> jaco_db::Result<()> {
    if crate::app::is_shutting_down() {
        return Err(DbError::Invariant(
            "application is shutting down".to_string(),
        ));
    }
    config::store(cx).read(cx, |operation| {
        matches!(operation, config::ConfigOperation::Ready(_))
            .then_some(())
            .ok_or_else(|| DbError::Invariant("config resource is not exactly Ready".to_string()))
    })
}

pub(crate) fn request_refresh(cx: &mut App) {
    let database = store(cx);
    let phase = database.read(cx, |resource| match resource {
        DatabaseResource::AwaitingConfig => None,
        DatabaseResource::Bound { target, operation } => Some((target.clone(), operation.phase())),
    });
    let Some((target, phase)) = phase else {
        return;
    };

    match phase {
        DatabasePhase::Ready => {
            let executor = database.read(cx, |resource| match resource {
                DatabaseResource::Bound {
                    target: current,
                    operation: DatabaseOperation::Ready(data),
                } if current == &target => data
                    .session
                    .read_with(cx, |session, _| session.executor().ok()),
                DatabaseResource::AwaitingConfig | DatabaseResource::Bound { .. } => None,
            });
            let completion_target = target.clone();
            let task = cx.spawn(async move |cx| {
                let result = match executor {
                    Some(executor) => executor.validate().await,
                    None => Err(DbError::Invariant(
                        "database session executor is unavailable".to_string(),
                    )),
                };
                cx.update(|cx| match result {
                    Ok(()) => store(cx).update(cx, |resource| {
                        let DatabaseResource::Bound {
                            target: current,
                            operation,
                        } = resource
                        else {
                            return;
                        };
                        if current == &completion_target
                            && matches!(operation, DatabaseOperation::Refreshing { .. })
                        {
                            operation.transition(DatabaseMessage::Refreshed);
                        }
                    }),
                    Err(source) => retire_failed_refresh(
                        &completion_target,
                        DatabaseProblem::Open {
                            target: completion_target.clone(),
                            source,
                        },
                        cx,
                    ),
                });
            });
            database.update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &target && matches!(operation, DatabaseOperation::Ready(_)) {
                    operation.transition(DatabaseMessage::Refresh(task));
                }
            });
        }
        DatabasePhase::Unavailable => {
            let completion_target = target.clone();
            let task = cx.spawn(async move |cx| {
                let open_target = completion_target.clone();
                let opened = smol::unblock(move || {
                    let lease = DatabaseTargetLease::acquire(&open_target)?;
                    let store = FreshStore::reopen_validated_existing(&open_target.database_path)
                        .map_err(|source| DatabaseProblem::Open {
                        target: open_target.clone(),
                        source,
                    })?;
                    Ok::<_, DatabaseProblem>((store, lease))
                })
                .await;
                cx.update(|cx| {
                    let result = opened.and_then(|(store, lease)| create_data(store, lease, cx));
                    store(cx).update(cx, |resource| {
                        let DatabaseResource::Bound {
                            target: current,
                            operation,
                        } = resource
                        else {
                            return;
                        };
                        if current == &completion_target
                            && matches!(operation, DatabaseOperation::Repairing { .. })
                        {
                            operation.transition(DatabaseMessage::Repaired(result));
                        }
                    });
                });
            });
            database.update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &target && matches!(operation, DatabaseOperation::Unavailable(_)) {
                    operation.transition(DatabaseMessage::Repair {
                        repair: DatabaseRepair::Refresh,
                        task,
                    });
                }
            });
        }
        DatabasePhase::Idle
        | DatabasePhase::Loading
        | DatabasePhase::Refreshing
        | DatabasePhase::Retiring
        | DatabasePhase::Repairing => {}
    }
}

fn retire_failed_refresh(target: &DatabaseTarget, problem: DatabaseProblem, cx: &mut App) {
    let database = store(cx);
    let session = database.read(cx, |resource| match resource {
        DatabaseResource::Bound {
            target: current,
            operation,
        } if current == target && matches!(operation, DatabaseOperation::Refreshing { .. }) => {
            operation.session().cloned()
        }
        DatabaseResource::AwaitingConfig | DatabaseResource::Bound { .. } => None,
    });
    let Some(session) = session else {
        return;
    };
    let active = session.update(cx, |session, _| session.take_active());
    let completion_target = target.clone();
    let retire = cx.spawn(async move |cx| {
        if let Some(active) = active.as_ref() {
            while active.active_jobs() != 0 {
                smol::Timer::after(std::time::Duration::from_millis(10)).await;
            }
        }
        drop(active);
        cx.update(|cx| {
            store(cx).update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &completion_target
                    && matches!(operation, DatabaseOperation::Retiring { .. })
                {
                    operation.transition(DatabaseMessage::Retired);
                }
            });
        });
    });
    database.update(cx, |resource| {
        let DatabaseResource::Bound {
            target: current,
            operation,
        } = resource
        else {
            return;
        };
        if current == target && matches!(operation, DatabaseOperation::Refreshing { .. }) {
            operation.transition(DatabaseMessage::RefreshFailed { problem, retire });
        }
    });
}

pub(crate) fn backup_and_create_fresh(backup_dir: PathBuf, cx: &mut App) -> JacoResult<()> {
    let database = store(cx);
    let target = database.read(cx, |resource| match resource {
        DatabaseResource::Bound {
            target,
            operation: DatabaseOperation::Unavailable(problem),
        } if problem.can_create_fresh() => Some(target.clone()),
        DatabaseResource::AwaitingConfig | DatabaseResource::Bound { .. } => None,
    });
    let target = target.ok_or_else(|| {
        JacoError::Config("fresh database repair is not available for this problem".to_string())
    })?;
    let repair_target = target.clone();
    let repair_backup_dir = backup_dir.clone();
    let transition_target = target.clone();
    let transition_backup_dir = backup_dir.clone();
    let task = cx.spawn(async move |cx| {
        let repaired = smol::unblock(move || {
            let lease = DatabaseTargetLease::acquire(&repair_target)?;
            backup_database_files(&repair_target, &repair_backup_dir)?;
            let fresh = create_fresh_database(&repair_target);
            Ok::<_, DatabaseProblem>((fresh, lease))
        })
        .await;
        cx.update(|cx| {
            let (result, notice) = match repaired {
                Ok((Ok(fresh), lease)) => (
                    create_data(fresh, lease, cx),
                    Some(DatabaseBackupOutcome {
                        backup_dir: backup_dir.clone(),
                        fresh_error: None,
                    }),
                ),
                Ok((Err(error), _lease)) => {
                    let message = error.to_string();
                    (
                        Err(error),
                        Some(DatabaseBackupOutcome {
                            backup_dir: backup_dir.clone(),
                            fresh_error: Some(message),
                        }),
                    )
                }
                Err(error) => (Err(error), None),
            };
            if let Some(notice) = notice {
                cx.update_global::<DatabaseRepairNotices, _>(|notices, _| {
                    notices.0.push_back(notice);
                });
            }
            store(cx).update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &target && matches!(operation, DatabaseOperation::Repairing { .. }) {
                    operation.transition(DatabaseMessage::Repaired(result));
                }
            });
        });
    });
    database.update(cx, |resource| {
        let DatabaseResource::Bound {
            target: current,
            operation,
        } = resource
        else {
            return;
        };
        if current == &transition_target && matches!(operation, DatabaseOperation::Unavailable(_)) {
            operation.transition(DatabaseMessage::Repair {
                repair: DatabaseRepair::BackupAndCreateFresh {
                    backup_dir: transition_backup_dir.clone(),
                },
                task,
            });
        }
    });
    Ok(())
}

pub(crate) struct DatabaseBackupOutcome {
    pub(crate) backup_dir: PathBuf,
    pub(crate) fresh_error: Option<String>,
}

pub(crate) fn take_backup_outcome(cx: &mut App) -> Option<DatabaseBackupOutcome> {
    cx.update_global::<DatabaseRepairNotices, _>(|notices, _| notices.0.pop_front())
}

fn backup_database_files(
    target: &DatabaseTarget,
    backup_dir: &Path,
) -> Result<(), DatabaseProblem> {
    fs::create_dir(backup_dir).map_err(|error| DatabaseProblem::Backup {
        backup_dir: backup_dir.to_path_buf(),
        message: error.to_string(),
    })?;
    for artifact in database_artifacts(&target.database_path) {
        if !artifact.exists() {
            continue;
        }
        let name = artifact
            .file_name()
            .expect("database artifact must have a file name");
        let destination = backup_dir.join(name);
        fs::copy(&artifact, &destination).map_err(|error| DatabaseProblem::Backup {
            backup_dir: backup_dir.to_path_buf(),
            message: error.to_string(),
        })?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&destination)
            .and_then(|file| file.sync_all())
            .map_err(|error| DatabaseProblem::Backup {
                backup_dir: backup_dir.to_path_buf(),
                message: error.to_string(),
            })?;
    }
    crate::foundation::persistence::sync_directory(backup_dir).map_err(|error| {
        DatabaseProblem::Backup {
            backup_dir: backup_dir.to_path_buf(),
            message: error.to_string(),
        }
    })
}

fn create_fresh_database(target: &DatabaseTarget) -> Result<FreshStore, DatabaseProblem> {
    let staging_dir = tempfile::Builder::new()
        .prefix(".jaco-repair-")
        .tempdir_in(&target.data_dir)
        .map_err(|error| DatabaseProblem::CreateFresh {
            target: target.clone(),
            message: error.to_string(),
        })?;
    let staging = staging_dir.path().join(jaco_db::DATABASE_FILE);
    let staging_store = FreshStore::create_fresh_staging(&staging).map_err(|error| {
        DatabaseProblem::CreateFresh {
            target: target.clone(),
            message: error.to_string(),
        }
    })?;
    staging_store
        .validate()
        .map_err(|error| DatabaseProblem::CreateFresh {
            target: target.clone(),
            message: error.to_string(),
        })?;
    drop(staging_store);

    for artifact in database_artifacts(&target.database_path) {
        if artifact.exists() {
            fs::remove_file(&artifact).map_err(|error| DatabaseProblem::CreateFresh {
                target: target.clone(),
                message: error.to_string(),
            })?;
        }
    }
    fs::rename(&staging, &target.database_path).map_err(|error| DatabaseProblem::CreateFresh {
        target: target.clone(),
        message: error.to_string(),
    })?;
    crate::foundation::persistence::sync_directory(&target.data_dir).map_err(|error| {
        DatabaseProblem::CreateFresh {
            target: target.clone(),
            message: error.to_string(),
        }
    })?;
    FreshStore::reopen_validated_existing(&target.database_path).map_err(|error| {
        DatabaseProblem::CreateFresh {
            target: target.clone(),
            message: error.to_string(),
        }
    })
}

fn database_artifacts(database_path: &Path) -> [PathBuf; 3] {
    [
        database_path.to_path_buf(),
        PathBuf::from(format!("{}-wal", database_path.display())),
        PathBuf::from(format!("{}-shm", database_path.display())),
    ]
}

#[cfg(test)]
pub(crate) fn install_for_test(cx: &mut App, data_dir: &Path) {
    if !cx.has_global::<config::JacoConfigStore>() {
        let config_path = data_dir.join("config.toml");
        let mut value =
            config::JacoConfig::load_from_path_for_test(&config_path).expect("load test config");
        value.storage.data_dir = Some(data_dir.to_path_buf());
        config::install_for_test(cx, config_path, value).expect("install test config");
    }
    let target = DatabaseTarget {
        data_dir: data_dir.to_path_buf(),
        database_path: data_dir.join(jaco_db::DATABASE_FILE),
    };
    let result = open_initial(&target, cx);
    let mut operation = DatabaseOperation::new();
    operation.transition(DatabaseMessage::Settle(result));
    let resource = DatabaseResource::Bound { target, operation };
    DatabaseStore::install_global(cx, resource);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Task, TestAppContext};

    #[gpui::test]
    fn initial_database_open_is_settled_before_init_returns(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            let config_path = dir.path().join("config.toml");
            let config = config::JacoConfig {
                storage: config::StorageConfig {
                    data_dir: Some(dir.path().to_path_buf()),
                },
                ..Default::default()
            };
            config::install_for_test(cx, config_path, config).expect("install config store");

            init_store(cx);

            store(cx).read(cx, |resource| {
                let DatabaseResource::Bound { operation, .. } = resource else {
                    panic!("database should bind to ready config");
                };
                assert!(matches!(operation, DatabaseOperation::Ready(_)));
                assert!(!operation.is_running());
            });
        });
    }

    #[gpui::test]
    fn chat_preferences_do_not_rebind_database(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let config = config::JacoConfig {
            storage: config::StorageConfig {
                data_dir: Some(dir.path().to_path_buf()),
            },
            ..Default::default()
        };
        fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
        cx.update(|cx| {
            config::install_for_test(cx, config_path, config).expect("install config store");
            init_store(cx);
        });
        cx.run_until_parked();

        let original = cx.update(|cx| {
            store(cx).read(cx, |resource| match resource {
                DatabaseResource::Bound {
                    operation: DatabaseOperation::Ready(data),
                    ..
                } => data.session.clone(),
                _ => panic!("database should be ready"),
            })
        });
        cx.update(|cx| {
            config::update_chat_form_config(cx, |chat_form| {
                chat_form.model = Some(config::ChatFormModelConfig {
                    provider_id: "provider-1".to_string(),
                    model_id: "gpt-5".to_string(),
                });
            })
            .expect("save chat preferences");
        });
        cx.run_until_parked();

        let current = cx.update(|cx| {
            store(cx).read(cx, |resource| match resource {
                DatabaseResource::Bound {
                    operation: DatabaseOperation::Ready(data),
                    ..
                } => data.session.clone(),
                _ => panic!("database should remain ready"),
            })
        });
        assert_eq!(current, original);
    }

    #[gpui::test]
    fn failed_destructive_repair_remains_retryable_without_retained_data(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| install_for_test(cx, dir.path()));
        let target = cx.update(|cx| {
            store(cx).read(cx, |resource| match resource {
                DatabaseResource::Bound { target, .. } => target.clone(),
                DatabaseResource::AwaitingConfig => panic!("database should be bound"),
            })
        });

        cx.update(|cx| {
            store(cx).update(cx, |resource| {
                let DatabaseResource::Bound { operation, .. } = resource else {
                    panic!("database should be bound");
                };
                operation.transition(DatabaseMessage::Refresh(Task::ready(())));
            });
            retire_failed_refresh(
                &target,
                DatabaseProblem::Open {
                    target: target.clone(),
                    source: DbError::Invariant("validation failed".to_string()),
                },
                cx,
            );
        });
        cx.run_until_parked();

        cx.update(|cx| {
            store(cx).read(cx, |resource| {
                let DatabaseResource::Bound { operation, .. } = resource else {
                    panic!("database should be bound");
                };
                assert_eq!(operation.phase(), DatabasePhase::Unavailable);
                assert!(operation.session().is_none());
            });
        });

        let first_backup = dir.path().join("first-backup");
        cx.update(|cx| {
            store(cx).update(cx, |resource| {
                let DatabaseResource::Bound { operation, .. } = resource else {
                    panic!("database should be bound");
                };
                operation.transition(DatabaseMessage::Repair {
                    repair: DatabaseRepair::BackupAndCreateFresh {
                        backup_dir: first_backup.clone(),
                    },
                    task: Task::ready(()),
                });
                operation.transition(DatabaseMessage::Repaired(Err(DatabaseProblem::Backup {
                    backup_dir: first_backup.clone(),
                    message: "first backup failed".to_string(),
                })));
                assert_eq!(operation.phase(), DatabasePhase::Unavailable);
                assert!(matches!(
                    operation.problem(),
                    Some(DatabaseProblem::Backup { backup_dir, .. })
                        if backup_dir == &first_backup
                ));
            });
        });

        let second_backup = dir.path().join("second-backup");
        cx.update(|cx| {
            store(cx).update(cx, |resource| {
                let DatabaseResource::Bound { operation, .. } = resource else {
                    panic!("database should be bound");
                };
                operation.transition(DatabaseMessage::Repair {
                    repair: DatabaseRepair::BackupAndCreateFresh {
                        backup_dir: second_backup.clone(),
                    },
                    task: Task::ready(()),
                });
                operation.transition(DatabaseMessage::Repaired(Err(DatabaseProblem::Backup {
                    backup_dir: second_backup.clone(),
                    message: "second backup failed".to_string(),
                })));
                assert_eq!(operation.phase(), DatabasePhase::Unavailable);
                assert!(matches!(
                    operation.problem(),
                    Some(DatabaseProblem::Backup { backup_dir, .. })
                        if backup_dir == &second_backup
                ));
            });
        });
    }

    #[test]
    fn backup_database_files_copies_and_syncs_existing_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let database_path = dir.path().join(jaco_db::DATABASE_FILE);
        fs::write(&database_path, b"database").unwrap();
        fs::write(format!("{}-wal", database_path.display()), b"wal").unwrap();
        let target = DatabaseTarget {
            data_dir: dir.path().to_path_buf(),
            database_path,
        };
        let backup_dir = dir.path().join("backup");

        backup_database_files(&target, &backup_dir).expect("backup database artifacts");

        assert_eq!(
            fs::read(backup_dir.join(jaco_db::DATABASE_FILE)).unwrap(),
            b"database"
        );
        assert_eq!(
            fs::read(backup_dir.join(format!("{}-wal", jaco_db::DATABASE_FILE))).unwrap(),
            b"wal"
        );
    }

    #[test]
    fn stale_repair_staging_artifact_does_not_block_a_new_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let stale_dir = dir.path().join(".jaco-repair-stale");
        fs::create_dir(&stale_dir).unwrap();
        fs::write(stale_dir.join(jaco_db::DATABASE_FILE), b"interrupted").unwrap();
        let target = DatabaseTarget {
            data_dir: dir.path().to_path_buf(),
            database_path: dir.path().join(jaco_db::DATABASE_FILE),
        };

        let store = create_fresh_database(&target).expect("create a fresh database");

        store.validate().expect("fresh database is valid");
        assert!(stale_dir.join(jaco_db::DATABASE_FILE).exists());
    }
}
