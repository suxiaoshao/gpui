pub(crate) mod session;

use std::{
    collections::VecDeque,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
};

use gpui::{App, AppContext, BorrowAppContext, Entity, Global, Subscription, Task};
use gpui_operation::{Complete, Load, Refresh, Repair, Settle, Transition, repair};
use gpui_store::Store;
use jaco_agent::AgentPersistence;
#[cfg(test)]
use jaco_db::FreshRepository;
use jaco_db::{DbError, FreshStore};

use crate::{
    errors::{JacoError, JacoResult},
    foundation::persistence::FileLock,
    state::{config, selectors::SelectDatabaseTarget},
};

use self::session::{DatabaseBinding, DatabaseSession, DatabaseSessionKey};

static NEXT_SESSION_KEY: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DatabaseTarget {
    pub(crate) data_dir: PathBuf,
    pub(crate) database_path: PathBuf,
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
    pub(crate) binding: DatabaseBinding,
    pub(crate) session: Entity<DatabaseSession>,
}

impl Clone for DatabaseData {
    fn clone(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            session: self.session.clone(),
        }
    }
}

impl fmt::Debug for DatabaseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseData")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
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
    Internal {
        target: DatabaseTarget,
        message: String,
    },
}

impl DatabaseProblem {
    pub(crate) fn can_create_fresh(&self) -> bool {
        !matches!(self, Self::InUse { .. } | Self::Internal { .. })
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
            Self::Internal { target, message } => {
                write!(f, "{}: {message}", target.database_path.display())
            }
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

pub(crate) type DatabaseOperation =
    repair::Operation<DatabaseData, DatabaseProblem, DatabaseRepair, Task<()>>;

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
            operation.transition(Settle(result));
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
            let result = opened.and_then(|(store, lease)| {
                create_data(completion_target.clone(), store, lease, cx)
            });
            completion_store.update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &completion_target
                    && matches!(operation, DatabaseOperation::Loading(_))
                {
                    operation.transition(Complete(result));
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
        if current == &target && matches!(operation, DatabaseOperation::Idle(_)) {
            operation.transition(Load(task));
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
    create_data(target.clone(), store, lease, cx)
}

fn create_data(
    target: DatabaseTarget,
    store: FreshStore,
    lease: DatabaseTargetLease,
    cx: &mut impl AppContext,
) -> Result<DatabaseData, DatabaseProblem> {
    let raw_key = NEXT_SESSION_KEY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |key| {
            key.checked_add(1)
        })
        .map_err(|_| DatabaseProblem::Internal {
            target: target.clone(),
            message: "database session key overflow".to_string(),
        })?;
    let binding = DatabaseBinding {
        target,
        session_key: DatabaseSessionKey(raw_key),
    };
    let session = cx.new(|_| DatabaseSession::new(store, lease));
    Ok(DatabaseData { binding, session })
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
            operation: DatabaseOperation::Ready(ready),
            ..
        } => Some(ready.data().session.clone()),
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
            operation: DatabaseOperation::Ready(ready),
            ..
        } => Some(ready.data().session.clone()),
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
            operation: DatabaseOperation::Ready(ready),
            ..
        } => Some(ready.data().session.clone()),
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

pub(crate) fn ready_binding(cx: &impl AppContext) -> Option<DatabaseBinding> {
    if crate::app::is_shutting_down() {
        return None;
    }
    store(cx).read(cx, |resource| match resource {
        DatabaseResource::Bound {
            operation: DatabaseOperation::Ready(ready),
            ..
        } => Some(ready.data().binding.clone()),
        _ => None,
    })
}

pub(crate) fn retained_binding(cx: &impl AppContext) -> Option<DatabaseBinding> {
    store(cx).read(cx, |resource| match resource {
        DatabaseResource::AwaitingConfig => None,
        DatabaseResource::Bound { operation, .. } => {
            operation.data().map(|data| data.binding.clone())
        }
    })
}

pub(crate) fn request_refresh(cx: &mut App) {
    let database = store(cx);
    let snapshot = database.read(cx, |resource| match resource {
        DatabaseResource::AwaitingConfig => None,
        DatabaseResource::Bound { target, operation } => (!operation.is_running())
            .then(|| (target.clone(), operation.phase(), operation.data().cloned())),
    });
    let Some((target, phase, existing)) = snapshot else {
        return;
    };
    let executor = existing.as_ref().and_then(|data| {
        data.session
            .read_with(cx, |session, _| session.executor().ok())
    });
    let completion_target = target.clone();
    let task = cx.spawn(async move |cx| {
        let result = if let Some(data) = existing {
            let validation = match executor {
                Some(executor) => executor.validate().await,
                None => Err(DbError::Invariant(
                    "database session executor is unavailable".to_string(),
                )),
            };
            validation
                .map(|()| data)
                .map_err(|source| DatabaseProblem::Open {
                    target: completion_target.clone(),
                    source,
                })
        } else {
            let open_target = completion_target.clone();
            match smol::unblock(move || {
                let lease = DatabaseTargetLease::acquire(&open_target)?;
                let store = FreshStore::reopen_validated_existing(&open_target.database_path)
                    .map_err(|source| DatabaseProblem::Open {
                        target: open_target.clone(),
                        source,
                    })?;
                Ok::<_, DatabaseProblem>((store, lease))
            })
            .await
            {
                Ok((store, lease)) => {
                    cx.update(|cx| create_data(completion_target.clone(), store, lease, cx))
                }
                Err(error) => Err(error),
            }
        };
        cx.update(|cx| {
            store(cx).update(cx, |resource| {
                let DatabaseResource::Bound {
                    target: current,
                    operation,
                } = resource
                else {
                    return;
                };
                if current == &completion_target && operation.is_running() {
                    operation.transition(Complete(result));
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
        if current != &target {
            return;
        }
        match phase {
            repair::Phase::Ready => operation.transition(Refresh(task)),
            repair::Phase::Unavailable | repair::Phase::Degraded => operation.transition(Repair {
                repair: DatabaseRepair::Refresh,
                task,
            }),
            _ => {}
        }
    });
}

pub(crate) fn backup_and_create_fresh(backup_dir: PathBuf, cx: &mut App) -> JacoResult<()> {
    let database = store(cx);
    let (target, existing, allowed) = database.read(cx, |resource| match resource {
        DatabaseResource::Bound { target, operation } => (
            Some(target.clone()),
            operation.data().cloned(),
            operation
                .problem()
                .is_some_and(DatabaseProblem::can_create_fresh),
        ),
        DatabaseResource::AwaitingConfig => (None, None, false),
    });
    if !allowed {
        return Err(JacoError::Config(
            "fresh database repair is not available for this problem".to_string(),
        ));
    }
    let target =
        target.ok_or_else(|| JacoError::Config("database target is unavailable".to_string()))?;

    let active = if let Some(data) = existing {
        let active = data
            .session
            .update(cx, |session, _| session.take_active())
            .ok_or_else(|| JacoError::Config("database session is already paused".to_string()))?;
        Some(active)
    } else {
        None
    };
    let repair_target = target.clone();
    let repair_backup_dir = backup_dir.clone();
    let transition_target = target.clone();
    let transition_backup_dir = backup_dir.clone();
    let task = cx.spawn(async move |cx| {
        if let Some(active) = active.as_ref() {
            while active.active_jobs() != 0 {
                smol::Timer::after(std::time::Duration::from_millis(10)).await;
            }
        }
        let repaired = smol::unblock(move || {
            let lease = match active {
                Some(active) => {
                    let crate::database::session::ActiveDatabaseSession { store, lease, .. } =
                        active;
                    drop(store);
                    lease
                }
                None => DatabaseTargetLease::acquire(&repair_target)?,
            };
            backup_database_files(&repair_target, &repair_backup_dir)?;
            let fresh = create_fresh_database(&repair_target);
            Ok::<_, DatabaseProblem>((fresh, lease))
        })
        .await;
        cx.update(|cx| {
            let (result, notice) = match repaired {
                Ok((Ok(fresh), lease)) => (
                    create_data(target.clone(), fresh, lease, cx),
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
                if current == &target && operation.is_running() {
                    operation.transition(Complete(result));
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
        if current == &transition_target
            && matches!(
                operation,
                DatabaseOperation::Unavailable(_) | DatabaseOperation::Degraded(_)
            )
        {
            operation.transition(Repair {
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
        fs::File::open(&destination)
            .and_then(|file| file.sync_all())
            .map_err(|error| DatabaseProblem::Backup {
                backup_dir: backup_dir.to_path_buf(),
                message: error.to_string(),
            })?;
    }
    fs::File::open(backup_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| DatabaseProblem::Backup {
            backup_dir: backup_dir.to_path_buf(),
            message: error.to_string(),
        })
}

fn create_fresh_database(target: &DatabaseTarget) -> Result<FreshStore, DatabaseProblem> {
    let staging = target.data_dir.join(format!(
        ".jaco-repair-{}.sqlite3",
        NEXT_SESSION_KEY.load(Ordering::Relaxed)
    ));
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
    fs::File::open(&target.data_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| DatabaseProblem::CreateFresh {
            target: target.clone(),
            message: error.to_string(),
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
    operation.transition(Settle(result));
    let resource = DatabaseResource::Bound { target, operation };
    DatabaseStore::install_global(cx, resource);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

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

        let original = cx.update(|cx| retained_binding(cx).expect("ready database binding"));
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

        let current = cx.update(|cx| retained_binding(cx).expect("retained database binding"));
        assert_eq!(current, original);
    }
}
