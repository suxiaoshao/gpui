use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task, Window,
};
use gpui_component::{
    WindowExt as _,
    notification::{Notification, NotificationType},
};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use smol::channel::{Receiver, Sender, TrySendError};
use tracing::{Level, event};

use crate::foundation::I18n;

const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(300);
const WAKE_CHANNEL_CAPACITY: usize = 1;
const MAX_PENDING_PATHS: usize = 1024;

type SystemDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FileWatchTargetKind {
    ExactFile,
    DirectoryTree,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FileWatchTarget {
    kind: FileWatchTargetKind,
    logical_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct WatchRegistrationId(u64);

#[derive(Clone, Debug)]
enum FileWatchEvent {
    Dirty {
        registration_id: WatchRegistrationId,
    },
    Problem(FileWatchProblem),
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum FileWatchProblem {
    #[error("{target_kind:?} file watch target must be an absolute path: {path}")]
    InvalidTarget {
        target_kind: FileWatchTargetKind,
        path: PathBuf,
    },
    #[error("failed to initialize native file monitoring: {cause}")]
    BackendInitialization { cause: String },
    #[error("failed to watch {path}: {cause}")]
    Watch { path: PathBuf, cause: String },
    #[error("failed to stop watching {path}: {cause}")]
    Unwatch { path: PathBuf, cause: String },
    #[error("native file monitoring reported an error: {cause}")]
    Runtime { cause: String },
}

pub(crate) struct FileWatchBinding {
    _registration_id: Option<WatchRegistrationId>,
    _event_subscription: Subscription,
    _unregister_subscription: Subscription,
}

impl FileWatchBinding {
    fn inert() -> Self {
        Self {
            _registration_id: None,
            _event_subscription: Subscription::new(|| {}),
            _unregister_subscription: Subscription::new(|| {}),
        }
    }

    #[cfg(test)]
    fn is_inert(&self) -> bool {
        self._registration_id.is_none()
    }
}

trait FileWatchBackend: Send {
    fn watch(
        &mut self,
        root: &Path,
        mode: RecursiveMode,
    ) -> Result<(), notify_debouncer_full::notify::Error>;
    fn unwatch(&mut self, root: &Path) -> Result<(), notify_debouncer_full::notify::Error>;
    fn shutdown(&mut self);
}

struct SystemFileWatchBackend {
    debouncer: Option<SystemDebouncer>,
}

impl SystemFileWatchBackend {
    fn new(inbox: Arc<Mutex<WatchInbox>>, wake_tx: Sender<()>) -> Result<Self, FileWatchProblem> {
        let debouncer = new_debouncer(
            DEBOUNCE_TIMEOUT,
            None,
            move |result: DebounceEventResult| record_native_result(&inbox, &wake_tx, result),
        )
        .map_err(|error| FileWatchProblem::BackendInitialization {
            cause: error.to_string(),
        })?;
        Ok(Self {
            debouncer: Some(debouncer),
        })
    }
}

impl FileWatchBackend for SystemFileWatchBackend {
    fn watch(
        &mut self,
        root: &Path,
        mode: RecursiveMode,
    ) -> Result<(), notify_debouncer_full::notify::Error> {
        let Some(debouncer) = self.debouncer.as_mut() else {
            return Err(notify_debouncer_full::notify::Error::generic(
                "file watcher is stopped",
            ));
        };
        debouncer.watch(root, mode)
    }

    fn unwatch(&mut self, root: &Path) -> Result<(), notify_debouncer_full::notify::Error> {
        let Some(debouncer) = self.debouncer.as_mut() else {
            return Ok(());
        };
        debouncer.unwatch(root)
    }

    fn shutdown(&mut self) {
        if let Some(debouncer) = self.debouncer.take() {
            debouncer.stop_nonblocking();
        }
    }
}

struct FileWatchServiceGlobal(Entity<FileWatchService>);

impl Global for FileWatchServiceGlobal {}

pub(crate) struct FileWatchService {
    backend: Option<Box<dyn FileWatchBackend>>,
    registry: WatchRegistry,
    inbox: Arc<Mutex<WatchInbox>>,
    wake_tx: Sender<()>,
    wake_rx: Receiver<()>,
    control_tx: Sender<WatchControl>,
    control_rx: Receiver<WatchControl>,
    initial_problem: Option<FileWatchProblem>,
    stopped: bool,
    pump_task: Task<()>,
}

impl EventEmitter<FileWatchEvent> for FileWatchService {}

#[derive(Default)]
struct WatchRegistry {
    next_registration_id: u64,
    registrations: HashMap<WatchRegistrationId, RegistrationEntry>,
    roots: HashMap<PathBuf, RootEntry>,
}

struct RegistrationEntry {
    targets: Vec<TargetEntry>,
}

struct TargetEntry {
    target: FileWatchTarget,
    actual_roots: Vec<(PathBuf, RootRequirement)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RootRequirement {
    mode: RecursiveMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RootEntry {
    non_recursive_refs: usize,
    recursive_refs: usize,
    active_mode: RecursiveMode,
}

#[derive(Default)]
struct WatchInbox {
    paths: BTreeSet<PathBuf>,
    rescan_all: bool,
    runtime_problem: Option<FileWatchProblem>,
}

enum WatchControl {
    Unregister(WatchRegistrationId),
}

struct FileWatchWarningOwner {
    pending: bool,
    shown: bool,
    _subscription: Subscription,
}

impl FileWatchWarningOwner {
    fn note_problem(&mut self) {
        if !self.shown {
            self.pending = true;
        }
    }

    fn take_pending(&mut self) -> bool {
        if self.shown || !self.pending {
            return false;
        }
        self.pending = false;
        self.shown = true;
        true
    }
}

#[derive(Clone)]
struct FileWatchWarningOwnerGlobal(Entity<FileWatchWarningOwner>);

impl Global for FileWatchWarningOwnerGlobal {}

pub(crate) fn exact_file(path: PathBuf) -> Result<FileWatchTarget, FileWatchProblem> {
    target(FileWatchTargetKind::ExactFile, path)
}

pub(crate) fn directory_tree(path: PathBuf) -> Result<FileWatchTarget, FileWatchProblem> {
    target(FileWatchTargetKind::DirectoryTree, path)
}

fn target(kind: FileWatchTargetKind, path: PathBuf) -> Result<FileWatchTarget, FileWatchProblem> {
    let path = normalize_lexically(path);
    if !path.is_absolute() {
        return Err(FileWatchProblem::InvalidTarget {
            target_kind: kind,
            path,
        });
    }
    Ok(FileWatchTarget {
        kind,
        logical_path: path,
    })
}

pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<FileWatchServiceGlobal>() {
        return;
    }

    let inbox = Arc::new(Mutex::new(WatchInbox::default()));
    let (wake_tx, wake_rx) = smol::channel::bounded(WAKE_CHANNEL_CAPACITY);
    let (control_tx, control_rx) = smol::channel::unbounded();
    let backend = SystemFileWatchBackend::new(Arc::clone(&inbox), wake_tx.clone());
    let initial_problem = backend.as_ref().err().cloned();
    if let Some(problem) = initial_problem.as_ref() {
        log_problem(problem);
    }

    let service = cx.new(|_| FileWatchService {
        backend: backend
            .ok()
            .map(|backend| Box::new(backend) as Box<dyn FileWatchBackend>),
        registry: WatchRegistry::default(),
        inbox,
        wake_tx,
        wake_rx,
        control_tx,
        control_rx,
        initial_problem: initial_problem.clone(),
        stopped: false,
        pump_task: Task::ready(()),
    });
    cx.set_global(FileWatchServiceGlobal(service.clone()));
    init_warning_owner(service.clone(), cx);

    let wake_rx = service.read(cx).wake_rx.clone();
    let pump_task = service.update(cx, |_service, cx| {
        cx.spawn(async move |service, cx| {
            while wake_rx.recv().await.is_ok() {
                let Some(service) = service.upgrade() else {
                    break;
                };
                service.update(cx, |service, cx| service.pump_once(cx));
            }
        })
    });
    service.update(cx, |service, _| service.pump_task = pump_task);
}

pub(crate) fn shutdown(cx: &mut App) {
    let Some(service) = service(cx) else {
        return;
    };
    service.update(cx, |service, _| service.shutdown());
}

pub(crate) fn report_problem(problem: FileWatchProblem, cx: &mut App) {
    let Some(service) = service(cx) else {
        log_problem(&problem);
        return;
    };
    service.update(cx, |service, cx| service.report_problem(problem, cx));
}

pub(crate) fn bind<T: 'static>(
    targets: Vec<FileWatchTarget>,
    cx: &mut Context<T>,
    on_dirty: impl Fn(&mut T, &mut Context<T>) + 'static,
) -> FileWatchBinding {
    if targets.is_empty() || !cx.has_global::<FileWatchServiceGlobal>() {
        return FileWatchBinding::inert();
    }

    let service = cx.global::<FileWatchServiceGlobal>().0.clone();
    let registration_id = service.update(cx, |service, cx| service.register(targets, cx));
    let Some(registration_id) = registration_id else {
        return FileWatchBinding::inert();
    };

    let event_subscription = cx.subscribe(
        &service,
        move |owner, _service, event: &FileWatchEvent, cx| {
            if matches!(
                event,
                FileWatchEvent::Dirty {
                    registration_id: dirty_id
                } if *dirty_id == registration_id
            ) {
                on_dirty(owner, cx);
            }
        },
    );
    let (control_tx, wake_tx) = service.read(cx).lifecycle_senders();
    let unregister_subscription = Subscription::new(move || {
        if control_tx
            .try_send(WatchControl::Unregister(registration_id))
            .is_ok()
        {
            let _ = wake_tx.try_send(());
        }
    });

    FileWatchBinding {
        _registration_id: Some(registration_id),
        _event_subscription: event_subscription,
        _unregister_subscription: unregister_subscription,
    }
}

pub(crate) fn flush_pending_warning(window: &mut Window, cx: &mut App) {
    let Some(owner) = cx
        .try_global::<FileWatchWarningOwnerGlobal>()
        .map(|global| global.0.clone())
    else {
        return;
    };
    let show = owner.update(cx, |owner, _| owner.take_pending());
    if !show {
        return;
    }

    window.push_notification(warning_notification(cx), cx);
}

fn service(cx: &App) -> Option<Entity<FileWatchService>> {
    cx.try_global::<FileWatchServiceGlobal>()
        .map(|global| global.0.clone())
}

fn init_warning_owner(service: Entity<FileWatchService>, cx: &mut App) {
    let pending = service.read(cx).initial_problem.is_some();
    let owner = cx.new(|cx| {
        let subscription = cx.subscribe(
            &service,
            |owner: &mut FileWatchWarningOwner, _service, event: &FileWatchEvent, cx| {
                if let FileWatchEvent::Problem(problem) = event {
                    let _ = problem;
                    owner.note_problem();
                    if let Some(root) = super::find_main_window(cx) {
                        cx.defer(move |cx| {
                            let _ = root.update(cx, |_root, window, cx| {
                                flush_pending_warning(window, cx);
                            });
                        });
                    }
                }
            },
        );
        FileWatchWarningOwner {
            pending,
            shown: false,
            _subscription: subscription,
        }
    });
    cx.set_global(FileWatchWarningOwnerGlobal(owner));
}

fn warning_notification(cx: &App) -> Notification {
    Notification::new()
        .title(cx.global::<I18n>().t("file-watch-unavailable-title"))
        .message(cx.global::<I18n>().t("file-watch-unavailable-message"))
        .with_type(NotificationType::Warning)
}

impl FileWatchService {
    fn lifecycle_senders(&self) -> (Sender<WatchControl>, Sender<()>) {
        (self.control_tx.clone(), self.wake_tx.clone())
    }

    fn register(
        &mut self,
        targets: Vec<FileWatchTarget>,
        cx: &mut Context<Self>,
    ) -> Option<WatchRegistrationId> {
        if self.stopped {
            return None;
        }
        let Some(backend) = self.backend.as_deref_mut() else {
            let problem = self.initial_problem.clone().unwrap_or_else(|| {
                FileWatchProblem::BackendInitialization {
                    cause: "native file watcher is unavailable".to_string(),
                }
            });
            self.report_problem(problem, cx);
            return None;
        };

        match self.registry.register(targets, backend) {
            Ok(id) => Some(id),
            Err(problems) => {
                for problem in problems {
                    self.report_problem(problem, cx);
                }
                None
            }
        }
    }

    fn pump_once(&mut self, cx: &mut Context<Self>) {
        if self.stopped {
            return;
        }

        let mut problems = Vec::new();
        while let Ok(control) = self.control_rx.try_recv() {
            match control {
                WatchControl::Unregister(id) => {
                    if let Some(backend) = self.backend.as_deref_mut() {
                        problems.extend(self.registry.unregister(id, backend));
                    } else {
                        self.registry.registrations.remove(&id);
                    }
                }
            }
        }

        let mut batch = match self.inbox.lock() {
            Ok(mut inbox) => std::mem::take(&mut *inbox),
            Err(error) => {
                problems.push(FileWatchProblem::Runtime {
                    cause: error.to_string(),
                });
                WatchInbox {
                    rescan_all: true,
                    ..WatchInbox::default()
                }
            }
        };
        let mut dirty = HashSet::new();
        if let Some(backend) = self.backend.as_deref_mut() {
            let outcome = self
                .registry
                .reconcile(backend, &batch.paths, batch.rescan_all);
            dirty.extend(outcome.dirty);
            problems.extend(outcome.problems);
        }
        if batch.rescan_all {
            dirty.extend(self.registry.registrations.keys().copied());
        } else {
            dirty.extend(self.registry.dirty_for_paths(&batch.paths));
        }
        if let Some(problem) = batch.runtime_problem.take() {
            problems.push(problem);
        }

        for problem in problems {
            self.report_problem(problem, cx);
        }
        if self.stopped {
            return;
        }
        for registration_id in dirty {
            cx.emit(FileWatchEvent::Dirty { registration_id });
        }
    }

    fn report_problem(&mut self, problem: FileWatchProblem, cx: &mut Context<Self>) {
        log_problem(&problem);
        cx.emit(FileWatchEvent::Problem(problem));
    }

    fn shutdown(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.registry.registrations.clear();
        self.registry.roots.clear();
        if let Some(mut backend) = self.backend.take() {
            backend.shutdown();
        }
        self.wake_tx.close();
        self.wake_rx.close();
        self.control_tx.close();
        self.control_rx.close();
        self.pump_task = Task::ready(());
        if let Ok(mut inbox) = self.inbox.lock() {
            *inbox = WatchInbox::default();
        }
    }
}

fn log_problem(problem: &FileWatchProblem) {
    match problem {
        FileWatchProblem::InvalidTarget { target_kind, path } => event!(
            Level::ERROR,
            kind = "invalid_target",
            operation = "construct_target",
            target_kind = ?target_kind,
            path = %path.display(),
            cause = "target is not absolute",
            "jaco external file monitoring failure"
        ),
        FileWatchProblem::BackendInitialization { cause } => event!(
            Level::ERROR,
            kind = "backend_unavailable",
            operation = "initialize",
            target_kind = "all",
            path = "",
            cause,
            "jaco external file monitoring failure"
        ),
        FileWatchProblem::Watch { path, cause } => event!(
            Level::ERROR,
            kind = "backend_operation_failed",
            operation = "watch",
            target_kind = "shared_root",
            path = %path.display(),
            cause,
            "jaco external file monitoring failure"
        ),
        FileWatchProblem::Unwatch { path, cause } => event!(
            Level::ERROR,
            kind = "backend_operation_failed",
            operation = "unwatch",
            target_kind = "shared_root",
            path = %path.display(),
            cause,
            "jaco external file monitoring failure"
        ),
        FileWatchProblem::Runtime { cause } => event!(
            Level::ERROR,
            kind = "runtime_backend_error",
            operation = "callback",
            target_kind = "all",
            path = "",
            cause,
            "jaco external file monitoring failure"
        ),
    }
}

fn record_native_result(
    inbox: &Arc<Mutex<WatchInbox>>,
    wake_tx: &Sender<()>,
    result: DebounceEventResult,
) {
    let mut closed = false;
    if let Ok(mut inbox) = inbox.lock() {
        match result {
            Ok(events) => {
                for event in events {
                    if event.event.need_rescan() {
                        inbox.rescan_all = true;
                    }
                    for path in event.event.paths {
                        if inbox.paths.len() >= MAX_PENDING_PATHS {
                            inbox.rescan_all = true;
                            break;
                        }
                        inbox.paths.insert(normalize_lexically(path));
                    }
                }
            }
            Err(errors) => {
                let cause = errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                if errors.iter().any(|error| error.paths.is_empty()) {
                    inbox.rescan_all = true;
                } else {
                    for path in errors.iter().flat_map(|error| error.paths.iter().cloned()) {
                        if inbox.paths.len() >= MAX_PENDING_PATHS {
                            inbox.rescan_all = true;
                            break;
                        }
                        inbox.paths.insert(normalize_lexically(path));
                    }
                }
                inbox.runtime_problem = Some(FileWatchProblem::Runtime { cause });
            }
        }

        match wake_tx.try_send(()) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                inbox.rescan_all = true;
                event!(
                    Level::WARN,
                    "file watch wake channel full; scheduling all registrations dirty"
                );
            }
            Err(TrySendError::Closed(_)) => closed = true,
        }
    }
    if closed {
        // The callback can race explicit shutdown. The closed channel is expected then.
    }
}

struct ReconcileOutcome {
    dirty: HashSet<WatchRegistrationId>,
    problems: Vec<FileWatchProblem>,
}

impl WatchRegistry {
    fn register(
        &mut self,
        targets: Vec<FileWatchTarget>,
        backend: &mut dyn FileWatchBackend,
    ) -> Result<WatchRegistrationId, Vec<FileWatchProblem>> {
        let id = WatchRegistrationId(self.next_registration_id);
        self.next_registration_id = self.next_registration_id.wrapping_add(1);
        let targets = targets
            .into_iter()
            .map(|target| TargetEntry {
                actual_roots: actual_roots(&target),
                target,
            })
            .collect::<Vec<_>>();
        let requirements = targets
            .iter()
            .flat_map(|target| target.actual_roots.iter().cloned())
            .collect::<Vec<_>>();
        let mut acquired = Vec::new();
        let mut problems = Vec::new();
        for (root, requirement) in requirements {
            match self.add_requirement(&root, requirement, backend) {
                Ok(()) => acquired.push((root, requirement)),
                Err(problem) => {
                    problems.push(problem);
                    for (root, requirement) in acquired.into_iter().rev() {
                        if let Some(problem) = self.remove_requirement(&root, requirement, backend)
                        {
                            problems.push(problem);
                        }
                    }
                    return Err(problems);
                }
            }
        }
        self.registrations.insert(id, RegistrationEntry { targets });
        Ok(id)
    }

    fn unregister(
        &mut self,
        id: WatchRegistrationId,
        backend: &mut dyn FileWatchBackend,
    ) -> Vec<FileWatchProblem> {
        let Some(registration) = self.registrations.remove(&id) else {
            return Vec::new();
        };
        let mut problems = Vec::new();
        for (root, requirement) in registration
            .targets
            .into_iter()
            .flat_map(|target| target.actual_roots)
        {
            if let Some(problem) = self.remove_requirement(&root, requirement, backend) {
                problems.push(problem);
            }
        }
        problems
    }

    fn reconcile(
        &mut self,
        backend: &mut dyn FileWatchBackend,
        changed_paths: &BTreeSet<PathBuf>,
        rescan_all: bool,
    ) -> ReconcileOutcome {
        let mut dirty = HashSet::new();
        let mut problems = Vec::new();
        let roots_before = self.roots.keys().cloned().collect::<HashSet<_>>();
        let plans = self
            .registrations
            .iter()
            .flat_map(|(id, registration)| {
                registration
                    .targets
                    .iter()
                    .enumerate()
                    .map(|(index, target)| (*id, index, actual_roots(&target.target)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        for (id, index, new_roots) in plans {
            let old_roots = self.registrations[&id].targets[index].actual_roots.clone();
            if old_roots == new_roots {
                continue;
            }
            match self.replace_roots(&old_roots, &new_roots, backend) {
                Ok(mut release_problems) => {
                    self.registrations
                        .get_mut(&id)
                        .expect("registration exists during reconciliation")
                        .targets[index]
                        .actual_roots = new_roots;
                    dirty.insert(id);
                    problems.append(&mut release_problems);
                }
                Err(mut acquire_problems) => problems.append(&mut acquire_problems),
            }
        }
        problems.extend(self.repair_missing_roots(backend));
        problems.extend(self.reattach_stable_affected_roots(
            &roots_before,
            changed_paths,
            rescan_all,
            backend,
        ));
        ReconcileOutcome { dirty, problems }
    }

    fn repair_missing_roots(
        &mut self,
        backend: &mut dyn FileWatchBackend,
    ) -> Vec<FileWatchProblem> {
        let mut requirements = HashMap::<PathBuf, RootEntry>::new();
        for (root, requirement) in self.registrations.values().flat_map(|registration| {
            registration
                .targets
                .iter()
                .flat_map(|target| target.actual_roots.iter())
        }) {
            requirements
                .entry(root.clone())
                .and_modify(|entry| *entry = entry.with_added(*requirement))
                .or_insert_with(|| RootEntry::from_requirement(*requirement));
        }

        let mut problems = Vec::new();
        for (root, mut entry) in requirements {
            if self.roots.contains_key(&root) {
                continue;
            }
            let mode = entry.required_mode();
            match backend.watch(&root, mode) {
                Ok(()) => {
                    entry.active_mode = mode;
                    self.roots.insert(root, entry);
                }
                Err(error) => problems.push(FileWatchProblem::Watch {
                    path: root,
                    cause: error.to_string(),
                }),
            }
        }
        problems
    }

    fn reattach_stable_affected_roots(
        &mut self,
        roots_before: &HashSet<PathBuf>,
        changed_paths: &BTreeSet<PathBuf>,
        rescan_all: bool,
        backend: &mut dyn FileWatchBackend,
    ) -> Vec<FileWatchProblem> {
        let roots = self
            .roots
            .iter()
            .filter_map(|(root, entry)| {
                (roots_before.contains(root)
                    && (rescan_all
                        || changed_paths
                            .iter()
                            .any(|changed_path| root.starts_with(changed_path))))
                .then_some((root.clone(), entry.active_mode))
            })
            .collect::<Vec<_>>();
        let mut problems = Vec::new();
        for (root, mode) in roots {
            match backend.unwatch(&root) {
                Ok(()) => {}
                Err(error) if expected_missing_watch(&error) => {}
                Err(error) => {
                    problems.push(FileWatchProblem::Unwatch {
                        path: root,
                        cause: error.to_string(),
                    });
                    continue;
                }
            }
            if let Err(error) = backend.watch(&root, mode) {
                self.roots.remove(&root);
                problems.push(FileWatchProblem::Watch {
                    path: root,
                    cause: error.to_string(),
                });
            }
        }
        problems
    }

    fn replace_roots(
        &mut self,
        old_roots: &[(PathBuf, RootRequirement)],
        new_roots: &[(PathBuf, RootRequirement)],
        backend: &mut dyn FileWatchBackend,
    ) -> Result<Vec<FileWatchProblem>, Vec<FileWatchProblem>> {
        let to_add = new_roots
            .iter()
            .filter(|root| !old_roots.contains(root))
            .cloned()
            .collect::<Vec<_>>();
        let to_remove = old_roots
            .iter()
            .filter(|root| !new_roots.contains(root))
            .cloned()
            .collect::<Vec<_>>();
        let mut acquired = Vec::new();
        let mut problems = Vec::new();
        for (root, requirement) in to_add {
            match self.add_requirement(&root, requirement, backend) {
                Ok(()) => acquired.push((root, requirement)),
                Err(problem) => {
                    problems.push(problem);
                    for (root, requirement) in acquired.into_iter().rev() {
                        if let Some(problem) = self.remove_requirement(&root, requirement, backend)
                        {
                            problems.push(problem);
                        }
                    }
                    return Err(problems);
                }
            }
        }
        for (root, requirement) in to_remove {
            if let Some(problem) = self.remove_requirement(&root, requirement, backend) {
                problems.push(problem);
            }
        }
        Ok(problems)
    }

    fn add_requirement(
        &mut self,
        root: &Path,
        requirement: RootRequirement,
        backend: &mut dyn FileWatchBackend,
    ) -> Result<(), FileWatchProblem> {
        let Some(current) = self.roots.get(root).copied() else {
            backend
                .watch(root, requirement.mode)
                .map_err(|error| FileWatchProblem::Watch {
                    path: root.to_path_buf(),
                    cause: error.to_string(),
                })?;
            self.roots
                .insert(root.to_path_buf(), RootEntry::from_requirement(requirement));
            return Ok(());
        };

        let next = current.with_added(requirement);
        if next.required_mode() != current.active_mode {
            self.switch_mode(root, current.active_mode, next.required_mode(), backend)?;
        }
        self.roots.insert(
            root.to_path_buf(),
            RootEntry {
                active_mode: next.required_mode(),
                ..next
            },
        );
        Ok(())
    }

    fn remove_requirement(
        &mut self,
        root: &Path,
        requirement: RootRequirement,
        backend: &mut dyn FileWatchBackend,
    ) -> Option<FileWatchProblem> {
        let current = self.roots.get(root).copied()?;
        let next = current.with_removed(requirement);
        if next.total_refs() == 0 {
            self.roots.remove(root);
            return backend.unwatch(root).err().and_then(|error| {
                (!expected_missing_watch(&error)).then(|| FileWatchProblem::Unwatch {
                    path: root.to_path_buf(),
                    cause: error.to_string(),
                })
            });
        }
        let required_mode = next.required_mode();
        if required_mode == current.active_mode {
            self.roots.insert(root.to_path_buf(), next);
            return None;
        }
        match self.switch_mode(root, current.active_mode, required_mode, backend) {
            Ok(()) => {
                self.roots.insert(
                    root.to_path_buf(),
                    RootEntry {
                        active_mode: required_mode,
                        ..next
                    },
                );
                None
            }
            Err(problem) => {
                if self.roots.contains_key(root) {
                    self.roots.insert(
                        root.to_path_buf(),
                        RootEntry {
                            active_mode: current.active_mode,
                            ..next
                        },
                    );
                }
                Some(problem)
            }
        }
    }

    fn switch_mode(
        &mut self,
        root: &Path,
        old_mode: RecursiveMode,
        new_mode: RecursiveMode,
        backend: &mut dyn FileWatchBackend,
    ) -> Result<(), FileWatchProblem> {
        if let Err(error) = backend.unwatch(root)
            && !expected_missing_watch(&error)
        {
            return Err(FileWatchProblem::Unwatch {
                path: root.to_path_buf(),
                cause: error.to_string(),
            });
        }
        if let Err(error) = backend.watch(root, new_mode) {
            let restore_error = backend.watch(root, old_mode).err();
            if let Some(restore_error) = restore_error {
                self.roots.remove(root);
                return Err(FileWatchProblem::Watch {
                    path: root.to_path_buf(),
                    cause: format!(
                        "{error}; restoring previous {old_mode:?} watch also failed: {restore_error}"
                    ),
                });
            }
            return Err(FileWatchProblem::Watch {
                path: root.to_path_buf(),
                cause: error.to_string(),
            });
        }
        Ok(())
    }

    fn dirty_for_paths(&self, paths: &BTreeSet<PathBuf>) -> HashSet<WatchRegistrationId> {
        self.registrations
            .iter()
            .filter_map(|(id, registration)| {
                registration
                    .targets
                    .iter()
                    .any(|target| {
                        paths
                            .iter()
                            .any(|path| target_is_related(&target.target, path))
                    })
                    .then_some(*id)
            })
            .collect()
    }
}

fn expected_missing_watch(error: &notify_debouncer_full::notify::Error) -> bool {
    matches!(
        error.kind,
        notify_debouncer_full::notify::ErrorKind::PathNotFound
            | notify_debouncer_full::notify::ErrorKind::WatchNotFound
    )
}

impl RootEntry {
    fn from_requirement(requirement: RootRequirement) -> Self {
        Self {
            non_recursive_refs: usize::from(requirement.mode == RecursiveMode::NonRecursive),
            recursive_refs: usize::from(requirement.mode == RecursiveMode::Recursive),
            active_mode: requirement.mode,
        }
    }

    fn with_added(mut self, requirement: RootRequirement) -> Self {
        match requirement.mode {
            RecursiveMode::NonRecursive => self.non_recursive_refs += 1,
            RecursiveMode::Recursive => self.recursive_refs += 1,
        }
        self
    }

    fn with_removed(mut self, requirement: RootRequirement) -> Self {
        match requirement.mode {
            RecursiveMode::NonRecursive => {
                self.non_recursive_refs = self.non_recursive_refs.saturating_sub(1)
            }
            RecursiveMode::Recursive => self.recursive_refs = self.recursive_refs.saturating_sub(1),
        }
        self
    }

    fn total_refs(self) -> usize {
        self.non_recursive_refs + self.recursive_refs
    }

    fn required_mode(self) -> RecursiveMode {
        if self.recursive_refs > 0 {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        }
    }
}

fn actual_roots(target: &FileWatchTarget) -> Vec<(PathBuf, RootRequirement)> {
    let desired_root = match target.kind {
        FileWatchTargetKind::ExactFile | FileWatchTargetKind::DirectoryTree => target
            .logical_path
            .parent()
            .expect("absolute watched target has a parent")
            .to_path_buf(),
    };
    let desired_mode = match target.kind {
        FileWatchTargetKind::ExactFile => RecursiveMode::NonRecursive,
        FileWatchTargetKind::DirectoryTree => RecursiveMode::Recursive,
    };
    let mut roots = Vec::new();
    if desired_root.is_dir() {
        push_root(&mut roots, desired_root.clone(), desired_mode);
        if let Some(parent) = desired_root.parent()
            && let Some(anchor) = deepest_existing_directory(parent)
        {
            push_root(&mut roots, anchor, RecursiveMode::NonRecursive);
        }
    } else if let Some(frontier) = deepest_existing_directory(&desired_root) {
        push_root(&mut roots, frontier, RecursiveMode::NonRecursive);
    }
    roots.sort_by(|left, right| left.0.cmp(&right.0));
    roots
}

fn push_root(roots: &mut Vec<(PathBuf, RootRequirement)>, path: PathBuf, mode: RecursiveMode) {
    if let Some((_, requirement)) = roots.iter_mut().find(|(root, _)| *root == path) {
        if mode == RecursiveMode::Recursive {
            requirement.mode = RecursiveMode::Recursive;
        }
        return;
    }
    roots.push((path, RootRequirement { mode }));
}

fn deepest_existing_directory(path: &Path) -> Option<PathBuf> {
    let mut candidate = Some(path);
    while let Some(path) = candidate {
        if path.is_dir() {
            return Some(path.to_path_buf());
        }
        candidate = path.parent();
    }
    None
}

fn target_is_related(target: &FileWatchTarget, changed_path: &Path) -> bool {
    match target.kind {
        FileWatchTargetKind::ExactFile => {
            changed_path == target.logical_path || target.logical_path.starts_with(changed_path)
        }
        FileWatchTargetKind::DirectoryTree => {
            changed_path.starts_with(&target.logical_path)
                || target.logical_path.starts_with(changed_path)
        }
    }
}

fn normalize_lexically(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str())
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use notify_debouncer_full::{
        DebouncedEvent,
        notify::{Error, Event, EventKind, event::Flag},
    };

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum BackendCall {
        Watch(PathBuf, RecursiveMode),
        Unwatch(PathBuf),
        Shutdown,
    }

    #[derive(Default)]
    struct FakeBackend {
        calls: Arc<Mutex<Vec<BackendCall>>>,
        fail_watch: Option<(PathBuf, RecursiveMode)>,
        fail_unwatch: Option<PathBuf>,
    }

    impl FileWatchBackend for FakeBackend {
        fn watch(&mut self, root: &Path, mode: RecursiveMode) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(BackendCall::Watch(root.to_path_buf(), mode));
            if self.fail_watch.as_ref() == Some(&(root.to_path_buf(), mode)) {
                self.fail_watch = None;
                return Err(Error::generic("fake watch failure"));
            }
            Ok(())
        }

        fn unwatch(&mut self, root: &Path) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(BackendCall::Unwatch(root.to_path_buf()));
            if self.fail_unwatch.as_deref() == Some(root) {
                return Err(Error::generic("fake unwatch failure"));
            }
            Ok(())
        }

        fn shutdown(&mut self) {
            self.calls.lock().unwrap().push(BackendCall::Shutdown);
        }
    }

    #[test]
    fn targets_require_absolute_paths_and_normalize_lexically() {
        assert!(matches!(
            exact_file(PathBuf::from("config.toml")),
            Err(FileWatchProblem::InvalidTarget { .. })
        ));
        let target = exact_file(PathBuf::from("/tmp/a/../config.toml")).unwrap();
        assert_eq!(target.logical_path, PathBuf::from("/tmp/config.toml"));
    }

    #[test]
    fn exact_file_uses_parent_and_recovery_anchor() {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("config");
        fs::create_dir(&parent).unwrap();
        let target = exact_file(parent.join("config.toml")).unwrap();
        let roots = actual_roots(&target);
        assert!(roots.contains(&(
            parent,
            RootRequirement {
                mode: RecursiveMode::NonRecursive
            }
        )));
        assert!(roots.contains(&(
            temp.path().to_path_buf(),
            RootRequirement {
                mode: RecursiveMode::NonRecursive
            }
        )));
    }

    #[test]
    fn missing_directory_tree_uses_deepest_existing_frontier_then_reconciles() {
        let temp = tempfile::tempdir().unwrap();
        let skills = temp.path().join("project/.agents/skills");
        let target = directory_tree(skills).unwrap();
        let mut backend = FakeBackend::default();
        let mut registry = WatchRegistry::default();
        let id = registry.register(vec![target], &mut backend).unwrap();
        assert_eq!(
            registry.registrations[&id].targets[0].actual_roots[0].0,
            temp.path()
        );

        fs::create_dir(temp.path().join("project")).unwrap();
        let outcome = registry.reconcile(&mut backend, &BTreeSet::new(), false);
        assert!(outcome.dirty.contains(&id));
        assert_eq!(
            registry.registrations[&id].targets[0].actual_roots[0].0,
            temp.path().join("project")
        );
    }

    #[test]
    fn directory_replacement_reattaches_a_stable_actual_root() {
        let temp = tempfile::tempdir().unwrap();
        let agents = temp.path().join(".agents");
        fs::create_dir(&agents).unwrap();
        let mut backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let mut registry = WatchRegistry::default();
        let id = registry
            .register(
                vec![directory_tree(agents.join("skills")).unwrap()],
                &mut backend,
            )
            .unwrap();
        calls.lock().unwrap().clear();

        fs::remove_dir(&agents).unwrap();
        fs::create_dir(&agents).unwrap();
        let changed_paths = BTreeSet::from([agents.clone()]);
        let outcome = registry.reconcile(&mut backend, &changed_paths, false);
        assert!(outcome.problems.is_empty());
        assert!(
            registry.registrations[&id].targets[0]
                .actual_roots
                .iter()
                .any(|(root, requirement)| {
                    root == &agents && requirement.mode == RecursiveMode::Recursive
                })
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                BackendCall::Unwatch(agents.clone()),
                BackendCall::Watch(agents, RecursiveMode::Recursive),
            ]
        );
    }

    #[test]
    fn failed_reattach_is_retried_on_the_next_reconcile() {
        let temp = tempfile::tempdir().unwrap();
        let agents = temp.path().join(".agents");
        fs::create_dir(&agents).unwrap();
        let mut backend = FakeBackend::default();
        let mut registry = WatchRegistry::default();
        registry
            .register(
                vec![directory_tree(agents.join("skills")).unwrap()],
                &mut backend,
            )
            .unwrap();

        backend.fail_watch = Some((agents.clone(), RecursiveMode::Recursive));
        let first = registry.reconcile(&mut backend, &BTreeSet::from([agents.clone()]), false);
        assert_eq!(first.problems.len(), 1);
        assert!(!registry.roots.contains_key(&agents));

        let second = registry.reconcile(&mut backend, &BTreeSet::new(), false);
        assert!(second.problems.is_empty());
        assert_eq!(
            registry.roots[&agents].active_mode,
            RecursiveMode::Recursive
        );
    }

    #[test]
    fn shared_roots_are_reference_counted_and_recursive_dominates() {
        let temp = tempfile::tempdir().unwrap();
        let agents = temp.path().join(".agents");
        fs::create_dir(&agents).unwrap();
        let mut backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let mut registry = WatchRegistry::default();
        let first = registry
            .register(
                vec![directory_tree(agents.join("skills")).unwrap()],
                &mut backend,
            )
            .unwrap();
        let second = registry
            .register(
                vec![directory_tree(agents.join("skills")).unwrap()],
                &mut backend,
            )
            .unwrap();
        assert_eq!(registry.roots[&agents].recursive_refs, 2);
        registry.unregister(first, &mut backend);
        assert_eq!(registry.roots[&agents].recursive_refs, 1);
        registry.unregister(second, &mut backend);
        assert!(!registry.roots.contains_key(&agents));
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| **call == BackendCall::Unwatch(agents.clone()))
                .count(),
            1
        );
    }

    #[test]
    fn recursive_ref_promotes_and_demotes_root_mode() {
        let temp = tempfile::tempdir().unwrap();
        let agents = temp.path().join(".agents");
        fs::create_dir(&agents).unwrap();
        let mut backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let mut registry = WatchRegistry::default();
        let exact_id = registry
            .register(
                vec![exact_file(agents.join("marker.toml")).unwrap()],
                &mut backend,
            )
            .unwrap();
        let tree_id = registry
            .register(
                vec![directory_tree(agents.join("skills")).unwrap()],
                &mut backend,
            )
            .unwrap();
        assert_eq!(
            registry.roots[&agents].active_mode,
            RecursiveMode::Recursive
        );

        registry.unregister(tree_id, &mut backend);
        assert_eq!(
            registry.roots[&agents].active_mode,
            RecursiveMode::NonRecursive
        );
        registry.unregister(exact_id, &mut backend);

        let relevant_calls = calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| match call {
                BackendCall::Watch(path, _) | BackendCall::Unwatch(path) => path == &agents,
                BackendCall::Shutdown => false,
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            relevant_calls,
            vec![
                BackendCall::Watch(agents.clone(), RecursiveMode::NonRecursive),
                BackendCall::Unwatch(agents.clone()),
                BackendCall::Watch(agents.clone(), RecursiveMode::Recursive),
                BackendCall::Unwatch(agents.clone()),
                BackendCall::Watch(agents.clone(), RecursiveMode::NonRecursive),
                BackendCall::Unwatch(agents),
            ]
        );
    }

    #[test]
    fn failed_registration_rolls_back_only_its_roots() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let mut backend = FakeBackend {
            fail_watch: Some((second.clone(), RecursiveMode::NonRecursive)),
            ..FakeBackend::default()
        };
        let mut registry = WatchRegistry::default();
        let result = registry.register(
            vec![
                exact_file(first.join("config.toml")).unwrap(),
                exact_file(second.join("config.toml")).unwrap(),
            ],
            &mut backend,
        );
        assert!(result.is_err());
        assert!(registry.registrations.is_empty());
        assert!(registry.roots.is_empty());
    }

    #[test]
    fn related_paths_are_routed_once_per_registration() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config.toml");
        let skills = temp.path().join(".agents/skills");
        let mut backend = FakeBackend::default();
        let mut registry = WatchRegistry::default();
        let id = registry
            .register(
                vec![
                    exact_file(config.clone()).unwrap(),
                    directory_tree(skills.clone()).unwrap(),
                ],
                &mut backend,
            )
            .unwrap();
        let paths = BTreeSet::from([config, skills.join("nested/SKILL.md")]);
        assert_eq!(registry.dirty_for_paths(&paths), HashSet::from([id]));
        assert!(
            registry
                .dirty_for_paths(&BTreeSet::from([temp.path().join("other")]))
                .is_empty()
        );
    }

    #[test]
    fn wake_overflow_falls_back_to_all_dirty_without_problem() {
        let inbox = Arc::new(Mutex::new(WatchInbox::default()));
        let (wake_tx, _wake_rx) = smol::channel::bounded(1);
        wake_tx.try_send(()).unwrap();
        record_native_result(&inbox, &wake_tx, Ok(Vec::new()));
        let inbox = inbox.lock().unwrap();
        assert!(inbox.rescan_all);
        assert!(inbox.runtime_problem.is_none());
    }

    #[test]
    fn need_rescan_marks_the_inbox_all_dirty() {
        let inbox = Arc::new(Mutex::new(WatchInbox::default()));
        let (wake_tx, wake_rx) = smol::channel::bounded(1);
        let event = Event::new(EventKind::Any).set_flag(Flag::Rescan);
        record_native_result(
            &inbox,
            &wake_tx,
            Ok(vec![DebouncedEvent::new(event, Instant::now())]),
        );
        assert!(wake_rx.try_recv().is_ok());
        assert!(inbox.lock().unwrap().rescan_all);
    }

    #[test]
    fn pathless_runtime_error_marks_all_dirty_and_records_problem() {
        let inbox = Arc::new(Mutex::new(WatchInbox::default()));
        let (wake_tx, wake_rx) = smol::channel::bounded(1);
        record_native_result(
            &inbox,
            &wake_tx,
            Err(vec![Error::generic("fake runtime failure")]),
        );
        assert!(wake_rx.try_recv().is_ok());
        let inbox = inbox.lock().unwrap();
        assert!(inbox.rescan_all);
        assert!(matches!(
            inbox.runtime_problem,
            Some(FileWatchProblem::Runtime { .. })
        ));
    }

    #[test]
    fn shutdown_stops_backend_once_and_is_idempotent() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let inbox = Arc::new(Mutex::new(WatchInbox::default()));
        let (wake_tx, wake_rx) = smol::channel::bounded(1);
        let (control_tx, control_rx) = smol::channel::unbounded();
        let mut service = FileWatchService {
            backend: Some(Box::new(backend)),
            registry: WatchRegistry::default(),
            inbox,
            wake_tx,
            wake_rx,
            control_tx,
            control_rx,
            initial_problem: None,
            stopped: false,
            pump_task: Task::ready(()),
        };
        service.shutdown();
        service.shutdown();
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| **call == BackendCall::Shutdown)
                .count(),
            1
        );
    }

    #[test]
    fn inert_binding_is_available_without_a_service() {
        assert!(FileWatchBinding::inert().is_inert());
    }

    #[test]
    fn late_unregister_after_channels_close_is_a_noop() {
        let (control_tx, control_rx) = smol::channel::unbounded();
        let (wake_tx, wake_rx) = smol::channel::bounded(1);
        control_rx.close();
        wake_rx.close();
        let id = WatchRegistrationId(42);
        let binding = FileWatchBinding {
            _registration_id: Some(id),
            _event_subscription: Subscription::new(|| {}),
            _unregister_subscription: Subscription::new(move || {
                if control_tx.try_send(WatchControl::Unregister(id)).is_ok() {
                    let _ = wake_tx.try_send(());
                }
            }),
        };
        drop(binding);
    }

    #[test]
    fn warning_owner_keeps_pending_until_it_is_shown_once() {
        let mut owner = FileWatchWarningOwner {
            pending: false,
            shown: false,
            _subscription: Subscription::new(|| {}),
        };
        owner.note_problem();
        assert!(owner.pending);
        assert!(owner.take_pending());
        owner.note_problem();
        assert!(!owner.pending);
        assert!(!owner.take_pending());
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "the headless test process does not receive FSEvents reliably"
    )]
    fn real_backend_observes_atomic_replace_through_parent_watch() {
        let temp = tempfile::tempdir().unwrap();
        let watched_root = fs::canonicalize(temp.path()).unwrap();
        let config = watched_root.join("config.toml");
        fs::write(&config, "value = 1\n").unwrap();
        let inbox = Arc::new(Mutex::new(WatchInbox::default()));
        let (wake_tx, wake_rx) = smol::channel::bounded(1);
        let mut backend = SystemFileWatchBackend::new(Arc::clone(&inbox), wake_tx).unwrap();
        backend
            .watch(&watched_root, RecursiveMode::NonRecursive)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        thread::sleep(Duration::from_millis(250));
        let replacement = watched_root.join("config.toml.next");
        fs::write(&replacement, "value = 2\n").unwrap();
        fs::rename(&replacement, &config).unwrap();
        let delivered = wait_for_wake(&wake_rx, deadline);
        backend.shutdown();
        assert!(delivered, "native watcher did not deliver an event");
        let inbox = inbox.lock().unwrap();
        assert!(inbox.rescan_all || inbox.paths.iter().any(|path| path == &config));

        let target = exact_file(config).unwrap();
        let mut registry = WatchRegistry::default();
        let mut fake = FakeBackend::default();
        let registration_id = registry.register(vec![target], &mut fake).unwrap();
        assert!(
            inbox.rescan_all
                || registry
                    .dirty_for_paths(&inbox.paths)
                    .contains(&registration_id)
        );
    }

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "the headless test process does not receive FSEvents reliably"
    )]
    fn real_backend_reattaches_after_watched_directory_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let watched_root = fs::canonicalize(temp.path()).unwrap();
        let agents = watched_root.join(".agents");
        let skills = agents.join("skills");
        fs::create_dir_all(&skills).unwrap();
        let inbox = Arc::new(Mutex::new(WatchInbox::default()));
        let (wake_tx, wake_rx) = smol::channel::bounded(1);
        let mut backend = SystemFileWatchBackend::new(Arc::clone(&inbox), wake_tx).unwrap();
        let mut registry = WatchRegistry::default();
        let registration_id = registry
            .register(vec![directory_tree(skills.clone()).unwrap()], &mut backend)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        thread::sleep(Duration::from_millis(250));

        fs::remove_dir_all(&agents).unwrap();
        fs::create_dir_all(&skills).unwrap();
        assert!(wait_for_wake(&wake_rx, deadline));
        let first_batch = {
            let mut inbox = inbox.lock().unwrap();
            std::mem::take(&mut *inbox)
        };
        let outcome = registry.reconcile(&mut backend, &first_batch.paths, first_batch.rescan_all);
        assert!(outcome.problems.is_empty());
        assert!(
            first_batch.rescan_all
                || registry
                    .dirty_for_paths(&first_batch.paths)
                    .contains(&registration_id)
        );

        let skill_file = skills.join("SKILL.md");
        fs::write(&skill_file, "# watched again\n").unwrap();
        assert!(wait_for_wake(&wake_rx, deadline));
        backend.shutdown();
        let second_batch = inbox.lock().unwrap();
        assert!(
            second_batch.rescan_all
                || registry
                    .dirty_for_paths(&second_batch.paths)
                    .contains(&registration_id)
        );
    }

    fn wait_for_wake(wake_rx: &Receiver<()>, deadline: Instant) -> bool {
        while Instant::now() < deadline {
            if wake_rx.try_recv().is_ok() {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        false
    }
}
