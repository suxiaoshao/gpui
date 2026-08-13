use std::{
    fmt, mem,
    path::{Path, PathBuf},
};

use gpui::{App, AppContext, Task};
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

    #[cfg(test)]
    fn kind(&self) -> DatabaseProblemKind {
        self.kind
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
            store(cx).update(cx, |resource| {
                if matches!(resource, DatabaseResource::Repairing { .. }) {
                    resource.transition(DatabaseMessage::Repaired(result));
                }
            });
        });
    });
    store(cx).update(cx, |resource| {
        resource.transition(DatabaseMessage::Repair { repair, task });
    });
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RepairPoint {
    CreateBackup,
    CopyMain,
    CopyWal,
    SyncBackupMain,
    SyncBackupWal,
    SyncBackupDirectory,
    BuildStaging,
    OpenStaging,
    ValidateStaging,
    MoveMainToRollback,
    MoveWalToRollback,
    SyncRollback,
    PromoteMain,
    PromoteWal,
    SyncPromoted,
    OpenLive,
    ValidateLive,
    SyncValidated,
    QuarantineMain,
    QuarantineWal,
    RestoreMain,
    RestoreWal,
    SyncRestored,
    CleanupMain,
    CleanupWal,
    SyncCleanup,
}

trait RepairHooks {
    fn check(&self, _point: RepairPoint) -> std::io::Result<()> {
        Ok(())
    }
}

impl RepairHooks for () {}

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
    backup_and_rebuild_with_hooks(path, backup_dir, &())
}

fn backup_and_rebuild_with_hooks(
    path: &Path,
    backup_dir: &Path,
    hooks: &impl RepairHooks,
) -> Result<DatabaseReady, DatabaseProblem> {
    backup_live_artifacts(path, backup_dir, hooks)?;

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

    build_staging(&staging, hooks).map_err(|problem| problem.with_backup(backup_dir))?;

    if let Err(primary) = move_live_to_rollback(path, &wal, &rollback, &rollback_wal, parent, hooks)
    {
        return Err(primary.with_backup(backup_dir));
    }

    let promotion = promote_and_validate(path, &wal, &staging, &staging_wal, parent, hooks);
    let pool = match promotion {
        Ok(pool) => pool,
        Err(primary) => {
            let rollback_result = quarantine_and_restore(
                QuarantineArtifacts {
                    rollback: RollbackArtifacts {
                        live: path,
                        live_wal: &wal,
                        rollback: &rollback,
                        rollback_wal: &rollback_wal,
                        parent,
                    },
                    failed: &failed,
                    failed_wal: &failed_wal,
                },
                hooks,
            );
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

    cleanup_rollback(&rollback, &rollback_wal, parent, hooks);
    Ok(DatabaseReady {
        pool,
        completed_backup: Some(backup_dir.to_path_buf()),
    })
}

fn backup_live_artifacts(
    path: &Path,
    backup_dir: &Path,
    hooks: &impl RepairHooks,
) -> Result<(), DatabaseProblem> {
    if backup_dir.exists() {
        return Err(DatabaseProblem::at(
            DatabaseProblemKind::Backup,
            "backup directory already exists",
        )
        .with_backup(backup_dir));
    }
    hooks
        .check(RepairPoint::CreateBackup)
        .and_then(|()| std::fs::create_dir_all(backup_dir))
        .map_err(|error| {
            DatabaseProblem::at(DatabaseProblemKind::Backup, error).with_backup(backup_dir)
        })?;

    let wal = wal_path(path);
    let artifacts = [
        (path, RepairPoint::CopyMain, RepairPoint::SyncBackupMain),
        (
            wal.as_path(),
            RepairPoint::CopyWal,
            RepairPoint::SyncBackupWal,
        ),
    ];
    let mut copied = false;
    for (source, copy_point, sync_point) in artifacts {
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
        hooks
            .check(copy_point)
            .and_then(|()| std::fs::copy(source, &target).map(|_| ()))
            .and_then(|()| hooks.check(sync_point))
            .and_then(|()| std::fs::File::open(&target)?.sync_all())
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
    hooks
        .check(RepairPoint::SyncBackupDirectory)
        .and_then(|()| sync_directory(backup_dir))
        .map_err(|error| {
            DatabaseProblem::at(DatabaseProblemKind::Backup, error).with_backup(backup_dir)
        })
}

fn build_staging(staging: &Path, hooks: &impl RepairHooks) -> Result<(), DatabaseProblem> {
    hooks
        .check(RepairPoint::BuildStaging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    let pool = establish_connection_at(staging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    checkpoint(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    drop(pool);

    hooks
        .check(RepairPoint::OpenStaging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    let pool = open_connection_at(staging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::BuildStaging, error))?;
    hooks
        .check(RepairPoint::ValidateStaging)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
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
    hooks: &impl RepairHooks,
) -> Result<(), DatabaseProblem> {
    let mut moved_main = false;
    let mut moved_wal = false;
    let move_result = (|| -> std::io::Result<()> {
        if path.exists() {
            hooks.check(RepairPoint::MoveMainToRollback)?;
            std::fs::rename(path, rollback)?;
            moved_main = true;
        }
        if wal.exists() {
            hooks.check(RepairPoint::MoveWalToRollback)?;
            std::fs::rename(wal, rollback_wal)?;
            moved_wal = true;
        }
        hooks.check(RepairPoint::SyncRollback)?;
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
            hooks,
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
    hooks: &impl RepairHooks,
) -> Result<DbConn, DatabaseProblem> {
    hooks
        .check(RepairPoint::PromoteMain)
        .and_then(|()| std::fs::rename(staging, path))
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;
    if staging_wal.exists() {
        hooks
            .check(RepairPoint::PromoteWal)
            .and_then(|()| std::fs::rename(staging_wal, wal))
            .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;
    }
    hooks
        .check(RepairPoint::SyncPromoted)
        .and_then(|()| sync_directory(parent))
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;

    hooks
        .check(RepairPoint::OpenLive)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    let pool = open_connection_at(path)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    hooks
        .check(RepairPoint::ValidateLive)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    validate_schema(&pool)
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Validate, error))?;
    hooks
        .check(RepairPoint::SyncValidated)
        .and_then(|()| sync_directory(parent))
        .map_err(|error| DatabaseProblem::at(DatabaseProblemKind::Swap, error))?;
    Ok(pool)
}

fn quarantine_and_restore(
    artifacts: QuarantineArtifacts<'_>,
    hooks: &impl RepairHooks,
) -> std::io::Result<()> {
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
        && let Err(error) = hooks
            .check(RepairPoint::QuarantineMain)
            .and_then(|()| std::fs::rename(live, failed))
    {
        errors.push(format!("quarantine main: {error}"));
    }
    if live_wal.exists()
        && let Err(error) = hooks
            .check(RepairPoint::QuarantineWal)
            .and_then(|()| std::fs::rename(live_wal, failed_wal))
    {
        errors.push(format!("quarantine WAL: {error}"));
    }
    if rollback.exists()
        && let Err(error) = hooks
            .check(RepairPoint::RestoreMain)
            .and_then(|()| std::fs::rename(rollback, live))
    {
        errors.push(format!("restore main: {error}"));
    }
    if rollback_wal.exists()
        && let Err(error) = hooks
            .check(RepairPoint::RestoreWal)
            .and_then(|()| std::fs::rename(rollback_wal, live_wal))
    {
        errors.push(format!("restore WAL: {error}"));
    }
    if let Err(error) = hooks
        .check(RepairPoint::SyncRestored)
        .and_then(|()| sync_directory(parent))
    {
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
    hooks: &impl RepairHooks,
) -> std::io::Result<()> {
    let RollbackArtifacts {
        live,
        live_wal,
        rollback,
        rollback_wal,
        parent,
    } = artifacts;
    let mut errors = Vec::new();
    if moved_main
        && let Err(error) = hooks
            .check(RepairPoint::RestoreMain)
            .and_then(|()| std::fs::rename(rollback, live))
    {
        errors.push(format!("restore main: {error}"));
    }
    if moved_wal
        && let Err(error) = hooks
            .check(RepairPoint::RestoreWal)
            .and_then(|()| std::fs::rename(rollback_wal, live_wal))
    {
        errors.push(format!("restore WAL: {error}"));
    }
    if let Err(error) = hooks
        .check(RepairPoint::SyncRestored)
        .and_then(|()| sync_directory(parent))
    {
        errors.push(format!("sync restored parent: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(std::io::Error::other(errors.join("; ")))
    }
}

fn cleanup_rollback(rollback: &Path, rollback_wal: &Path, parent: &Path, hooks: &impl RepairHooks) {
    for (artifact, point) in [
        (rollback, RepairPoint::CleanupMain),
        (rollback_wal, RepairPoint::CleanupWal),
    ] {
        if !artifact.exists() {
            continue;
        }
        if let Err(error) = hooks
            .check(point)
            .and_then(|()| std::fs::remove_file(artifact))
        {
            tracing::warn!(path = %artifact.display(), %error, "database rollback artifact cleanup failed");
        }
    }
    if let Err(error) = hooks
        .check(RepairPoint::SyncCleanup)
        .and_then(|()| sync_directory(parent))
    {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "feiwen-database-test-{}-{label}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            debug_assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct FailAt(HashSet<RepairPoint>);

    impl FailAt {
        fn new(points: impl IntoIterator<Item = RepairPoint>) -> Self {
            Self(points.into_iter().collect())
        }
    }

    impl RepairHooks for FailAt {
        fn check(&self, point: RepairPoint) -> std::io::Result<()> {
            if self.0.contains(&point) {
                Err(std::io::Error::other(format!("injected {point:?}")))
            } else {
                Ok(())
            }
        }
    }

    fn create_live_with_row(path: &Path) {
        let pool = establish_connection_at(path).unwrap();
        pool.get()
            .unwrap()
            .execute(
                r#"INSERT INTO novel (
                    id, name, "desc", is_limit, latest_chapter_name,
                    latest_chapter_id, word_count, author_name
                ) VALUES (1, 'old', 'old', false, 'chapter', 1, 1, 'author')"#,
                [],
            )
            .unwrap();
        checkpoint(&pool).unwrap();
    }

    fn assert_live_row(path: &Path) {
        let pool = open_connection_at(path).unwrap();
        validate_schema(&pool).unwrap();
        let count = pool
            .get()
            .unwrap()
            .query_row("SELECT count(*) FROM novel", [], |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn invalid_messages_do_not_replace_ready_database() {
        let pool = establish_connection_at(Path::new(":memory:")).unwrap();
        let mut resource = DatabaseResource::Ready {
            pool,
            completed_backup: None,
        };
        resource.transition(DatabaseMessage::Loaded(Err(DatabaseProblem::new("late"))));
        assert_eq!(resource.phase(), DatabasePhase::Ready);
        resource.transition(DatabaseMessage::Repair {
            repair: DatabaseRepair::Reopen,
            task: Task::ready(()),
        });
        assert_eq!(resource.phase(), DatabasePhase::Ready);
    }

    #[test]
    fn reopen_requires_an_existing_valid_schema() {
        let directory = TestDirectory::new("reopen-validation");
        let missing = directory.0.join("missing.duckdb");
        assert_eq!(
            reopen(&missing).unwrap_err().kind(),
            DatabaseProblemKind::Reopen
        );

        let invalid = directory.0.join("invalid.duckdb");
        let pool = open_connection_at(&invalid).unwrap();
        drop(pool);
        assert_eq!(
            reopen(&invalid).unwrap_err().kind(),
            DatabaseProblemKind::Validate
        );
    }

    #[test]
    fn backup_and_rebuild_preserves_backup_and_installs_empty_schema() {
        let directory = TestDirectory::new("rebuild");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);

        let backup = directory.0.join("backup");
        let rebuilt = backup_and_rebuild(&live, &backup).unwrap();
        assert_eq!(rebuilt.completed_backup.as_deref(), Some(backup.as_path()));
        assert!(backup.join("data.duckdb").exists());
        let count = rebuilt
            .pool
            .get()
            .unwrap()
            .query_row("SELECT count(*) FROM novel", [], |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn backup_failure_does_not_build_staging() {
        let directory = TestDirectory::new("backup-failure");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error =
            backup_and_rebuild_with_hooks(&live, &backup, &FailAt::new([RepairPoint::CopyMain]))
                .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Backup);
        assert_live_row(&live);
        assert!(std::fs::read_dir(&directory.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("staging-")
        }));
    }

    #[test]
    fn backup_copies_and_syncs_main_and_wal_artifacts() {
        let directory = TestDirectory::new("backup-artifacts");
        let live = directory.0.join("data.duckdb");
        let wal = wal_path(&live);
        let backup = directory.0.join("backup");
        std::fs::write(&live, b"main").unwrap();
        std::fs::write(&wal, b"wal").unwrap();

        backup_live_artifacts(&live, &backup, &()).unwrap();

        assert_eq!(std::fs::read(backup.join("data.duckdb")).unwrap(), b"main");
        assert_eq!(
            std::fs::read(backup.join("data.duckdb.wal")).unwrap(),
            b"wal"
        );
    }

    #[test]
    fn directory_sync_accepts_an_existing_directory() {
        let directory = TestDirectory::new("directory-sync");

        sync_directory(&directory.0).unwrap();
    }

    #[test]
    fn staging_validation_failure_does_not_touch_live() {
        let directory = TestDirectory::new("staging-validation");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error = backup_and_rebuild_with_hooks(
            &live,
            &backup,
            &FailAt::new([RepairPoint::ValidateStaging]),
        )
        .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Validate);
        assert_live_row(&live);
    }

    #[test]
    fn rollback_directory_sync_failure_restores_live() {
        let directory = TestDirectory::new("rollback-sync");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error = backup_and_rebuild_with_hooks(
            &live,
            &backup,
            &FailAt::new([RepairPoint::SyncRollback]),
        )
        .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Swap);
        assert_live_row(&live);
    }

    #[test]
    fn promoted_directory_sync_failure_quarantines_and_restores_live() {
        let directory = TestDirectory::new("promoted-sync");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error = backup_and_rebuild_with_hooks(
            &live,
            &backup,
            &FailAt::new([RepairPoint::SyncPromoted]),
        )
        .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Swap);
        assert_live_row(&live);
        assert!(std::fs::read_dir(&directory.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("failed-")
        }));
    }

    #[test]
    fn promotion_rename_failure_restores_live() {
        let directory = TestDirectory::new("promotion-rename");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error =
            backup_and_rebuild_with_hooks(&live, &backup, &FailAt::new([RepairPoint::PromoteMain]))
                .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Swap);
        assert_live_row(&live);
    }

    #[test]
    fn rebuilt_database_open_failure_restores_live() {
        let directory = TestDirectory::new("live-open");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error =
            backup_and_rebuild_with_hooks(&live, &backup, &FailAt::new([RepairPoint::OpenLive]))
                .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Validate);
        assert_live_row(&live);
    }

    #[test]
    fn wal_move_failure_restores_main_and_leaves_wal_in_place() {
        let directory = TestDirectory::new("wal-move");
        let live = directory.0.join("data.duckdb");
        let wal = wal_path(&live);
        let rollback = directory.0.join("rollback.duckdb");
        let rollback_wal = wal_path(&rollback);
        std::fs::write(&live, b"live").unwrap();
        std::fs::write(&wal, b"wal").unwrap();

        let error = move_live_to_rollback(
            &live,
            &wal,
            &rollback,
            &rollback_wal,
            &directory.0,
            &FailAt::new([RepairPoint::MoveWalToRollback]),
        )
        .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Swap);
        assert_eq!(std::fs::read(&live).unwrap(), b"live");
        assert_eq!(std::fs::read(&wal).unwrap(), b"wal");
        assert!(!rollback.exists());
    }

    #[test]
    fn validation_and_rollback_failure_report_both_and_backup_path() {
        let directory = TestDirectory::new("combined-failure");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let error = backup_and_rebuild_with_hooks(
            &live,
            &backup,
            &FailAt::new([RepairPoint::ValidateLive, RepairPoint::RestoreMain]),
        )
        .unwrap_err();

        assert_eq!(error.kind(), DatabaseProblemKind::Rollback);
        assert!(error.rollback_message.is_some());
        assert_eq!(error.backup_dir.as_deref(), Some(backup.as_path()));
        assert!(error.to_string().contains("backup:"));
    }

    #[test]
    fn post_commit_cleanup_failure_keeps_rebuilt_database_ready() {
        let directory = TestDirectory::new("cleanup-failure");
        let live = directory.0.join("data.duckdb");
        create_live_with_row(&live);
        let backup = directory.0.join("backup");

        let rebuilt = backup_and_rebuild_with_hooks(
            &live,
            &backup,
            &FailAt::new([RepairPoint::CleanupMain, RepairPoint::SyncCleanup]),
        )
        .unwrap();

        validate_schema(&rebuilt.pool).unwrap();
        assert_eq!(rebuilt.completed_backup.as_deref(), Some(backup.as_path()));
        assert!(backup.join("data.duckdb").exists());
    }

    #[test]
    fn backup_and_rebuild_rejects_missing_artifacts() {
        let directory = TestDirectory::new("missing");
        let error = backup_and_rebuild(
            &directory.0.join("missing.duckdb"),
            &directory.0.join("backup"),
        )
        .unwrap_err();
        assert_eq!(error.kind(), DatabaseProblemKind::Backup);
        assert!(error.to_string().contains("missing"));
    }
}
