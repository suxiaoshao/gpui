use std::{
    fmt, mem,
    path::{Path, PathBuf},
};

use gpui_kit::{App, AppContext, Task};
use gpui_operation::Transition;
use gpui_store::Store;

use super::{DbConn, establish_connection_at, get_data_url, open_connection_at, validate_schema};

pub(crate) type DatabaseStore = Store<DatabaseResource>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DatabasePhase {
    Loading,
    Ready,
    Unavailable,
    Repairing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DatabaseProblemKind {
    Open,
    Reopen,
    Backup,
    BuildStaging,
    Swap,
    Validate,
    Rollback,
    Access,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DatabaseProblem {
    kind: DatabaseProblemKind,
    message: String,
    rollback_message: Option<String>,
    backup_dir: Option<PathBuf>,
}

impl DatabaseProblem {
    pub(crate) fn new(error: impl fmt::Display) -> Self {
        Self::at(DatabaseProblemKind::Access, error)
    }

    fn at(kind: DatabaseProblemKind, error: impl fmt::Display) -> Self {
        Self {
            kind,
            message: error.to_string(),
            rollback_message: None,
            backup_dir: None,
        }
    }

    fn with_backup(mut self, backup_dir: &Path) -> Self {
        self.backup_dir = Some(backup_dir.to_path_buf());
        self
    }

    fn rollback(
        primary: DatabaseProblem,
        rollback_error: impl fmt::Display,
        backup_dir: &Path,
    ) -> Self {
        Self {
            kind: DatabaseProblemKind::Rollback,
            message: format!("{}: {}", problem_kind_label(primary.kind), primary.message),
            rollback_message: Some(rollback_error.to_string()),
            backup_dir: Some(backup_dir.to_path_buf()),
        }
    }
}

impl fmt::Display for DatabaseProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", problem_kind_label(self.kind), self.message)?;
        if let Some(rollback) = &self.rollback_message {
            write!(f, "; rollback failed: {rollback}")?;
        }
        if let Some(backup_dir) = &self.backup_dir {
            write!(f, "; backup: {}", backup_dir.display())?;
        }
        Ok(())
    }
}

impl std::error::Error for DatabaseProblem {}

fn problem_kind_label(kind: DatabaseProblemKind) -> &'static str {
    match kind {
        DatabaseProblemKind::Open => "open database failed",
        DatabaseProblemKind::Reopen => "reopen database failed",
        DatabaseProblemKind::Backup => "backup database failed",
        DatabaseProblemKind::BuildStaging => "build staging database failed",
        DatabaseProblemKind::Swap => "swap database failed",
        DatabaseProblemKind::Validate => "validate database failed",
        DatabaseProblemKind::Rollback => "restore original database failed",
        DatabaseProblemKind::Access => "database unavailable",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DatabaseRepair {
    Reopen,
    BackupAndRebuild { backup_dir: PathBuf },
}

struct DatabaseReady {
    pool: DbConn,
    completed_backup: Option<PathBuf>,
}

impl fmt::Debug for DatabaseReady {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseReady")
            .field("completed_backup", &self.completed_backup)
            .finish_non_exhaustive()
    }
}

pub(crate) enum DatabaseResource {
    Loading {
        task: Option<Task<()>>,
    },
    Ready {
        pool: DbConn,
        completed_backup: Option<PathBuf>,
    },
    Unavailable {
        problem: DatabaseProblem,
    },
    Repairing {
        _repair: DatabaseRepair,
        problem: DatabaseProblem,
        task: Option<Task<()>>,
    },
}

enum DatabaseMessage {
    Loaded(Result<DatabaseReady, DatabaseProblem>),
    Repair {
        repair: DatabaseRepair,
        task: Task<()>,
    },
    Repaired(Result<DatabaseReady, DatabaseProblem>),
}

impl DatabaseResource {
    pub(crate) fn phase(&self) -> DatabasePhase {
        match self {
            Self::Loading { .. } => DatabasePhase::Loading,
            Self::Ready { .. } => DatabasePhase::Ready,
            Self::Unavailable { .. } => DatabasePhase::Unavailable,
            Self::Repairing { .. } => DatabasePhase::Repairing,
        }
    }

    pub(crate) fn problem(&self) -> Option<&DatabaseProblem> {
        match self {
            Self::Unavailable { problem } | Self::Repairing { problem, .. } => Some(problem),
            Self::Loading { .. } | Self::Ready { .. } => None,
        }
    }

    pub(crate) fn completed_backup(&self) -> Option<PathBuf> {
        match self {
            Self::Ready {
                completed_backup, ..
            } => completed_backup.clone(),
            _ => None,
        }
    }
}

impl Transition<DatabaseMessage> for &mut DatabaseResource {
    type Output = ();

    fn transition(self, message: DatabaseMessage) {
        let current = mem::replace(self, DatabaseResource::Loading { task: None });
        match (current, message) {
            (DatabaseResource::Loading { task }, DatabaseMessage::Loaded(result)) => {
                *self = settled(result);
                drop(task);
            }
            (
                DatabaseResource::Unavailable { problem },
                DatabaseMessage::Repair { repair, task },
            ) => {
                *self = DatabaseResource::Repairing {
                    _repair: repair,
                    problem,
                    task: Some(task),
                };
            }
            (DatabaseResource::Repairing { task, .. }, DatabaseMessage::Repaired(result)) => {
                *self = settled(result);
                drop(task);
            }
            (current, message) => {
                *self = current;
                tracing::debug!(message = message.name(), "ignored database transition");
            }
        }
    }
}

impl DatabaseMessage {
    fn name(&self) -> &'static str {
        match self {
            Self::Loaded(_) => "Loaded",
            Self::Repair { .. } => "Repair",
            Self::Repaired(_) => "Repaired",
        }
    }
}

fn settled(result: Result<DatabaseReady, DatabaseProblem>) -> DatabaseResource {
    match result {
        Ok(DatabaseReady {
            pool,
            completed_backup,
        }) => DatabaseResource::Ready {
            pool,
            completed_backup,
        },
        Err(problem) => DatabaseResource::Unavailable { problem },
    }
}

pub(crate) fn init(cx: &mut App) {
    let database = DatabaseStore::install_global(cx, DatabaseResource::Loading { task: None });
    let task = cx.spawn(async move |cx| {
        let result = cx
            .background_spawn(async move {
                let path = get_data_url()
                    .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Open, error))?;
                let pool = establish_connection_at(&path)
                    .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Open, error))?;
                validate_schema(&pool)
                    .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
                Ok(DatabaseReady {
                    pool,
                    completed_backup: None,
                })
            })
            .await;
        cx.update(|cx| {
            store(cx).update(cx, |resource| {
                if matches!(resource, DatabaseResource::Loading { .. }) {
                    resource.transition(DatabaseMessage::Loaded(result));
                }
            });
            if is_ready(cx) {
                super::catalog::request_load(cx);
            }
        });
    });
    database.update(cx, |resource| {
        if let DatabaseResource::Loading { task: slot } = resource {
            *slot = Some(task);
        }
    });
}

pub(crate) fn store(cx: &impl AppContext) -> DatabaseStore {
    DatabaseStore::global(cx)
}

pub(crate) fn phase(cx: &impl AppContext) -> DatabasePhase {
    store(cx).read(cx, DatabaseResource::phase)
}

pub(crate) fn is_ready(cx: &impl AppContext) -> bool {
    phase(cx) == DatabasePhase::Ready
}

pub(crate) fn ready_pool(cx: &impl AppContext) -> Result<DbConn, DatabaseProblem> {
    store(cx).read(cx, |resource| match resource {
        DatabaseResource::Ready { pool, .. } => Ok(pool.clone()),
        DatabaseResource::Unavailable { problem } | DatabaseResource::Repairing { problem, .. } => {
            Err(problem.clone())
        }
        DatabaseResource::Loading { .. } => Err(DatabaseProblem::new("数据库仍在加载")),
    })
}

pub(crate) fn request_reopen(cx: &mut App) {
    request_repair(DatabaseRepair::Reopen, cx);
}

pub(crate) fn request_backup_and_rebuild(backup_dir: PathBuf, cx: &mut App) {
    request_repair(DatabaseRepair::BackupAndRebuild { backup_dir }, cx);
}

fn request_repair(repair: DatabaseRepair, cx: &mut App) {
    if phase(cx) != DatabasePhase::Unavailable {
        return;
    }
    let worker_repair = repair.clone();
    let task = cx.spawn(async move |cx| {
        let result = cx
            .background_spawn(async move {
                match worker_repair {
                    DatabaseRepair::Reopen => {
                        let path = get_data_url().map_err(|error| {
                            DatabaseProblem::at(DatabaseProblemKind::Reopen, error)
                        })?;
                        reopen(&path)
                    }
                    DatabaseRepair::BackupAndRebuild { backup_dir } => {
                        let path = get_data_url().map_err(|error| {
                            DatabaseProblem::at(DatabaseProblemKind::Backup, error)
                                .with_backup(&backup_dir)
                        })?;
                        backup_and_rebuild(&path, &backup_dir)
                    }
                }
            })
            .await;
        cx.update(|cx| {
            complete_repair(result, cx);
        });
    });
    store(cx).update(cx, |resource| {
        resource.transition(DatabaseMessage::Repair { repair, task });
    });
}

fn complete_repair(result: Result<DatabaseReady, DatabaseProblem>, cx: &mut App) {
    let accepted = store(cx).update(cx, |resource| {
        if matches!(resource, DatabaseResource::Repairing { .. }) {
            resource.transition(DatabaseMessage::Repaired(result));
            true
        } else {
            false
        }
    });
    if accepted && is_ready(cx) {
        super::catalog::request_load(cx);
    }
}

fn reopen(path: &Path) -> Result<DatabaseReady, DatabaseProblem> {
    if !path.exists() {
        return Err(DatabaseProblem::at(
            DatabaseProblemKind::Reopen,
            "database file does not exist",
        ));
    }
    let pool = open_connection_at(path)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Reopen, error))?;
    validate_schema(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    Ok(DatabaseReady {
        pool,
        completed_backup: None,
    })
}

struct RollbackArtifacts<'a> {
    live: &'a Path,
    live_wal: &'a Path,
    rollback: &'a Path,
    rollback_wal: &'a Path,
    parent: &'a Path,
}

struct QuarantineArtifacts<'a> {
    rollback: RollbackArtifacts<'a>,
    failed: &'a Path,
    failed_wal: &'a Path,
}

fn backup_and_rebuild(path: &Path, backup_dir: &Path) -> Result<DatabaseReady, DatabaseProblem> {
    backup_live_artifacts(path, backup_dir)?;

    let parent = path.parent().ok_or_else(|| {
        DatabaseProblem::at(
            DatabaseProblemKind::BuildStaging,
            "database has no parent directory",
        )
        .with_backup(backup_dir)
    })?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging = parent.join(format!("data.duckdb.staging-{nonce}"));
    let rollback = parent.join(format!("data.duckdb.rollback-{nonce}"));
    let failed = parent.join(format!("data.duckdb.failed-{nonce}"));
    let wal = wal_path(path);
    let staging_wal = wal_path(&staging);
    let rollback_wal = wal_path(&rollback);
    let failed_wal = wal_path(&failed);

    build_staging(&staging).map_err(|problem| problem.with_backup(backup_dir))?;

    if let Err(primary) = move_live_to_rollback(path, &wal, &rollback, &rollback_wal, parent) {
        return Err(primary.with_backup(backup_dir));
    }

    let promotion = promote_and_validate(path, &wal, &staging, &staging_wal, parent);
    let pool = match promotion {
        Ok(pool) => pool,
        Err(primary) => {
            let rollback_result = quarantine_and_restore(QuarantineArtifacts {
                rollback: RollbackArtifacts {
                    live: path,
                    live_wal: &wal,
                    rollback: &rollback,
                    rollback_wal: &rollback_wal,
                    parent,
                },
                failed: &failed,
                failed_wal: &failed_wal,
            });
            return match rollback_result {
                Ok(()) => Err(primary.with_backup(backup_dir)),
                Err(rollback_error) => Err(DatabaseProblem::rollback(
                    primary,
                    rollback_error,
                    backup_dir,
                )),
            };
        }
    };

    cleanup_rollback(&rollback, &rollback_wal, parent);
    Ok(DatabaseReady {
        pool,
        completed_backup: Some(backup_dir.to_path_buf()),
    })
}

fn backup_live_artifacts(path: &Path, backup_dir: &Path) -> Result<(), DatabaseProblem> {
    if backup_dir.exists() {
        return Err(DatabaseProblem::at(
            DatabaseProblemKind::Backup,
            "backup directory already exists",
        )
        .with_backup(backup_dir));
    }
    std::fs::create_dir_all(backup_dir).map_err(|error| {
        DatabaseProblem::at(DatabaseProblemKind::Backup, error).with_backup(backup_dir)
    })?;

    let wal = wal_path(path);
    let artifacts = [path, wal.as_path()];
    let mut copied = false;
    for source in artifacts {
        if !source.exists() {
            continue;
        }
        let target = backup_dir.join(source.file_name().ok_or_else(|| {
            DatabaseProblem::at(
                DatabaseProblemKind::Backup,
                "database artifact has no file name",
            )
            .with_backup(backup_dir)
        })?);
        std::fs::copy(source, &target)
            .map(|_| ())
            .and_then(|()| {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&target)?
                    .sync_all()
            })
            .map_err(|error| {
                DatabaseProblem::at(DatabaseProblemKind::Backup, error).with_backup(backup_dir)
            })?;
        copied = true;
    }
    if !copied {
        return Err(DatabaseProblem::at(
            DatabaseProblemKind::Backup,
            "database artifacts are missing",
        )
        .with_backup(backup_dir));
    }
    sync_directory(backup_dir).map_err(|error| {
        DatabaseProblem::at(DatabaseProblemKind::Backup, error).with_backup(backup_dir)
    })
}

fn build_staging(staging: &Path) -> Result<(), DatabaseProblem> {
    let pool = establish_connection_at(staging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    checkpoint(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    drop(pool);

    let pool = open_connection_at(staging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    validate_schema(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    checkpoint(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    drop(pool);
    Ok(())
}

fn move_live_to_rollback(
    path: &Path,
    wal: &Path,
    rollback: &Path,
    rollback_wal: &Path,
    parent: &Path,
) -> Result<(), DatabaseProblem> {
    let mut moved_main = false;
    let mut moved_wal = false;
    let move_result = (|| -> std::io::Result<()> {
        if path.exists() {
            std::fs::rename(path, rollback)?;
            moved_main = true;
        }
        if wal.exists() {
            std::fs::rename(wal, rollback_wal)?;
            moved_wal = true;
        }
        sync_directory(parent)
    })();
    if let Err(primary_error) = move_result {
        let primary = DatabaseProblem::at(DatabaseProblemKind::Swap, primary_error);
        let rollback_result = restore_moved_artifacts(
            RollbackArtifacts {
                live: path,
                live_wal: wal,
                rollback,
                rollback_wal,
                parent,
            },
            moved_main,
            moved_wal,
        );
        return match rollback_result {
            Ok(()) => Err(primary),
            Err(rollback_error) => Err(DatabaseProblem::rollback(primary, rollback_error, parent)),
        };
    }
    Ok(())
}

fn promote_and_validate(
    path: &Path,
    wal: &Path,
    staging: &Path,
    staging_wal: &Path,
    parent: &Path,
) -> Result<DbConn, DatabaseProblem> {
    std::fs::rename(staging, path)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;
    if staging_wal.exists() {
        std::fs::rename(staging_wal, wal)
            .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;
    }
    sync_directory(parent)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;

    let pool = open_connection_at(path)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    validate_schema(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    sync_directory(parent)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;
    Ok(pool)
}

fn quarantine_and_restore(artifacts: QuarantineArtifacts<'_>) -> std::io::Result<()> {
    let QuarantineArtifacts {
        rollback:
            RollbackArtifacts {
                live,
                live_wal,
                rollback,
                rollback_wal,
                parent,
            },
        failed,
        failed_wal,
    } = artifacts;
    let mut errors = Vec::new();
    if live.exists()
        && let Err(error) = std::fs::rename(live, failed)
    {
        errors.push(format!("quarantine main: {error}"));
    }
    if live_wal.exists()
        && let Err(error) = std::fs::rename(live_wal, failed_wal)
    {
        errors.push(format!("quarantine WAL: {error}"));
    }
    if rollback.exists()
        && let Err(error) = std::fs::rename(rollback, live)
    {
        errors.push(format!("restore main: {error}"));
    }
    if rollback_wal.exists()
        && let Err(error) = std::fs::rename(rollback_wal, live_wal)
    {
        errors.push(format!("restore WAL: {error}"));
    }
    if let Err(error) = sync_directory(parent) {
        errors.push(format!("sync restored parent: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(errors.join("; ")))
    }
}

fn restore_moved_artifacts(
    artifacts: RollbackArtifacts<'_>,
    moved_main: bool,
    moved_wal: bool,
) -> std::io::Result<()> {
    let RollbackArtifacts {
        live,
        live_wal,
        rollback,
        rollback_wal,
        parent,
    } = artifacts;
    let mut errors = Vec::new();
    if moved_main && let Err(error) = std::fs::rename(rollback, live) {
        errors.push(format!("restore main: {error}"));
    }
    if moved_wal && let Err(error) = std::fs::rename(rollback_wal, live_wal) {
        errors.push(format!("restore WAL: {error}"));
    }
    if let Err(error) = sync_directory(parent) {
        errors.push(format!("sync restored parent: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(errors.join("; ")))
    }
}

fn cleanup_rollback(rollback: &Path, rollback_wal: &Path, parent: &Path) {
    for artifact in [rollback, rollback_wal] {
        if !artifact.exists() {
            continue;
        }
        if let Err(error) = std::fs::remove_file(artifact) {
            tracing::warn!(path = %artifact.display(), %error, "database rollback artifact cleanup failed");
        }
    }
    if let Err(error) = sync_directory(parent) {
        tracing::warn!(path = %parent.display(), %error, "database rollback cleanup directory sync failed");
    }
}

fn checkpoint(pool: &DbConn) -> super::super::errors::FeiwenResult<()> {
    pool.get()?.execute_batch("CHECKPOINT")?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(target_os = "windows")]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    // Windows cannot open a directory with std::fs::File. Database artifacts
    // are flushed and synced through their file handles before this boundary.
    Ok(())
}

fn wal_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.wal", path.display()))
}
