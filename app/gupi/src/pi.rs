use gpui_kit::*;
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_tokio::Tokio;
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::{Semaphore, oneshot},
    time::Instant,
};

const OUTPUT_LIMIT: u64 = 16 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
static STARTUP_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeFailure {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("executable unavailable: {0}")]
    Command(String),
    #[error("probe timed out")]
    Timeout,
    #[error("probe output exceeded limit")]
    OutputLimit,
    #[error("invalid version output or unsuccessful exit")]
    InvalidVersion,
}
impl ProbeFailure {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Timeout => "error-pi-timeout",
            Self::OutputLimit => "error-pi-output",
            Self::InvalidVersion => "error-pi-version",
            Self::Io(_) | Self::Command(_) => "error-pi-probe",
        }
    }
}
#[derive(Clone)]
pub(crate) struct PiProbeData {
    pub command: PathBuf,
    pub version: String,
}
type PiOperation = refresh::Operation<PiProbeData, ProbeFailure, Task<()>>;
async fn bounded(reader: impl AsyncRead + Unpin) -> Result<Vec<u8>, ProbeFailure> {
    let mut bytes = Vec::new();
    reader
        .take(OUTPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > OUTPUT_LIMIT {
        return Err(ProbeFailure::OutputLimit);
    }
    Ok(bytes)
}
async fn probe(command: String, deadline: Instant) -> Result<PiProbeData, ProbeFailure> {
    let slots = STARTUP_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(2)))
        .clone();
    probe_with(command, deadline, slots, resolve_command, spawn_command).await
}
fn resolve_command(command: &str) -> Result<PathBuf, ProbeFailure> {
    which::which(command).map_err(|e| ProbeFailure::Command(e.to_string()))
}
fn spawn_command(path: &std::path::Path) -> Result<Child, ProbeFailure> {
    let mut process = Command::new(path);
    process
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    process.creation_flags(0x08000000);
    Ok(process.spawn()?)
}
async fn probe_with(
    command: String,
    deadline: Instant,
    slots: Arc<Semaphore>,
    resolve: impl FnOnce(&str) -> Result<PathBuf, ProbeFailure> + Send + 'static,
    spawn: impl FnOnce(&std::path::Path) -> Result<Child, ProbeFailure> + Send + 'static,
) -> Result<PiProbeData, ProbeFailure> {
    let startup = async move {
        let slot = slots
            .acquire_owned()
            .await
            .expect("startup semaphore is never closed");
        let (send, recv) = oneshot::channel();
        // The receiver is owned by this future. Dropping it tells a surviving blocking
        // producer to stop; failed delivery drops any late Child with kill_on_drop set.
        let worker = tokio::task::spawn_blocking(move || {
            let _slot = slot;
            if send.is_closed() {
                return;
            }
            if Instant::now() >= deadline {
                let _ = send.send(Err(ProbeFailure::Timeout));
                return;
            }
            let path = match resolve(&command) {
                Ok(path) => path,
                Err(error) => {
                    let _ = send.send(Err(error));
                    return;
                }
            };
            if send.is_closed() {
                return;
            }
            if Instant::now() >= deadline {
                let _ = send.send(Err(ProbeFailure::Timeout));
                return;
            }
            let result = spawn(&path).map(|child| (path, child));
            if Instant::now() >= deadline {
                drop(result);
                let _ = send.send(Err(ProbeFailure::Timeout));
            } else {
                let _ = send.send(result);
            }
        });
        worker
            .await
            .map_err(|e| ProbeFailure::Command(e.to_string()))?;
        recv.await
            .map_err(|e| ProbeFailure::Command(e.to_string()))?
    };
    let (path, mut child) = tokio::time::timeout_at(deadline, startup)
        .await
        .map_err(|_| ProbeFailure::Timeout)??;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let result = {
        let attempt = async {
            let (stdout, _, status) = tokio::try_join!(bounded(stdout), bounded(stderr), async {
                child.wait().await.map_err(ProbeFailure::Io)
            })?;
            let version = std::str::from_utf8(&stdout)
                .map_err(|_| ProbeFailure::InvalidVersion)?
                .trim();
            let parts: Vec<_> = version.trim_start_matches('v').split('.').collect();
            if !status.success()
                || parts.len() != 3
                || parts
                    .iter()
                    .any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()))
            {
                return Err(ProbeFailure::InvalidVersion);
            }
            Ok(PiProbeData {
                command: path,
                version: version.to_owned(),
            })
        };
        tokio::time::timeout_at(deadline, attempt)
            .await
            .unwrap_or(Err(ProbeFailure::Timeout))
    };
    if result.is_err() {
        let cleanup = async {
            if child.try_wait()?.is_none() {
                child.kill().await?;
            }
            Ok::<_, std::io::Error>(())
        };
        match tokio::time::timeout(CLEANUP_TIMEOUT, cleanup).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(%error, "Pi probe cleanup failed; dropping child"),
            Err(_) => tracing::warn!("Pi probe cleanup timed out; dropping child"),
        }
    }
    result
}
pub(crate) struct PiProbeController {
    pub operation: PiOperation,
    requested: Option<String>,
    pub draining: bool,
}
impl PiProbeController {
    pub fn new() -> Self {
        Self {
            operation: PiOperation::new(),
            requested: None,
            draining: false,
        }
    }
    pub fn matches_command(&self, command: Option<&str>) -> bool {
        self.requested.as_deref() == Some(command_key(command))
    }
    pub fn ready_for(&self, command: Option<&str>) -> Option<&PiProbeData> {
        self.operation.data().filter(|_| {
            self.matches_command(command)
                && !self.operation.is_running()
                && self.operation.problem().is_none()
        })
    }
    pub fn request(&mut self, command: Option<String>, retry: bool, cx: &mut Context<Self>) {
        if self.draining {
            return;
        }
        let command = command_key(command.as_deref()).to_owned();
        if self.requested.as_ref() == Some(&command) && (!retry || self.operation.is_running()) {
            return;
        }
        let changed = self.requested.as_ref() != Some(&command);
        self.operation.transition(Cancel);
        self.requested = Some(command.clone());
        if changed {
            self.operation = PiOperation::new();
        }
        self.start(command, cx);
    }
    pub fn stop(&mut self) {
        self.draining = true;
        self.operation.transition(Cancel);
    }
    fn start(&mut self, command: String, cx: &mut Context<Self>) {
        let deadline = Instant::now() + PROBE_TIMEOUT;
        let worker = Tokio::spawn(cx, probe(command, deadline));
        let task = cx.spawn(async move |owner, cx| {
            let started = std::time::Instant::now();
            let result = worker
                .await
                .unwrap_or_else(|e| Err(ProbeFailure::Command(e.to_string())));
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                success = result.is_ok(),
                problem = result.as_ref().err().map(ProbeFailure::key),
                "Pi probe completed"
            );
            let _ = owner.update(cx, |owner, cx| {
                owner.operation.transition(Complete(result));
                cx.notify();
            });
        });
        match &self.operation {
            PiOperation::Idle(_) => self.operation.transition(Load(task)),
            PiOperation::Unavailable(_) => self.operation.transition(Retry(task)),
            _ => self.operation.transition(Refresh(task)),
        }
        cx.notify();
    }
}
fn command_key(command: Option<&str>) -> &str {
    command
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pi")
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    async fn stalled_startup(lookup: bool) {
        let started = std::time::Instant::now();
        let result = probe_with(
            "fixture".into(),
            Instant::now() + Duration::from_millis(40),
            Arc::new(Semaphore::new(2)),
            move |_| {
                if lookup {
                    std::thread::sleep(Duration::from_millis(400));
                }
                Ok(PathBuf::from("fixture"))
            },
            move |_| {
                if !lookup {
                    std::thread::sleep(Duration::from_millis(400));
                }
                Err(ProbeFailure::Command("fixture launch finished".into()))
            },
        )
        .await;
        let elapsed = started.elapsed();
        eprintln!(
            "stalled {}: {elapsed:?}, error={:?}",
            if lookup { "lookup" } else { "spawn" },
            result.as_ref().err()
        );
        assert!(
            elapsed < Duration::from_millis(250),
            "startup exceeded the deadline: {elapsed:?}"
        );
        assert!(matches!(result, Err(ProbeFailure::Timeout)));
    }

    #[tokio::test]
    async fn startup_deadline_covers_lookup() {
        stalled_startup(true).await;
    }

    #[tokio::test]
    async fn startup_deadline_covers_spawn() {
        stalled_startup(false).await;
    }

    async fn stalled_cancellation(lookup: bool) {
        let (started, entered) = oneshot::channel();
        let (resolve_started, spawn_started) = if lookup {
            (Some(started), None)
        } else {
            (None, Some(started))
        };
        let task = tokio::spawn(probe_with(
            "fixture".into(),
            Instant::now() + Duration::from_secs(5),
            Arc::new(Semaphore::new(2)),
            move |_| {
                if let Some(started) = resolve_started {
                    started.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(400));
                }
                Ok(PathBuf::from("fixture"))
            },
            move |_| {
                if let Some(started) = spawn_started {
                    started.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(400));
                }
                Err(ProbeFailure::Command("fixture launch finished".into()))
            },
        ));
        entered.await.unwrap();
        let started = std::time::Instant::now();
        task.abort();
        let result = task.await;
        let elapsed = started.elapsed();
        eprintln!(
            "cancel stalled {}: {elapsed:?}",
            if lookup { "lookup" } else { "spawn" }
        );
        assert!(
            elapsed < Duration::from_millis(250),
            "cancellation waited for blocking startup: {elapsed:?}"
        );
        assert!(result.is_err_and(|e| e.is_cancelled()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn startup_cancellation_does_not_wait_for_lookup() {
        stalled_cancellation(true).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn startup_cancellation_does_not_wait_for_spawn() {
        stalled_cancellation(false).await;
    }

    #[tokio::test]
    async fn cancelled_launches_keep_their_slots_until_blocking_work_finishes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let slots = Arc::new(Semaphore::new(2));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut releases = Vec::new();
        for _ in 0..2 {
            let (started, entered) = oneshot::channel();
            let (release, wait) = std::sync::mpsc::channel();
            releases.push(release);
            let calls = calls.clone();
            let task = tokio::spawn(probe_with(
                "fixture".into(),
                Instant::now() + Duration::from_secs(5),
                slots.clone(),
                move |_| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    started.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(PathBuf::from("fixture"))
                },
                |_| panic!("cancelled lookup must not spawn a child"),
            ));
            entered.await.unwrap();
            task.abort();
            assert!(task.await.is_err_and(|e| e.is_cancelled()));
        }
        assert_eq!(slots.available_permits(), 0);

        // Repeated timeouts must remain queued, without adding blocking workers.
        for _ in 0..3 {
            let result = probe_with(
                "fixture".into(),
                Instant::now() + Duration::from_millis(20),
                slots.clone(),
                |_| panic!("both startup slots are still occupied"),
                |_| panic!("queued request must not spawn"),
            )
            .await;
            assert!(matches!(result, Err(ProbeFailure::Timeout)));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        for release in releases {
            release.send(()).unwrap();
        }
        let permits = tokio::time::timeout(Duration::from_secs(2), slots.acquire_many(2))
            .await
            .unwrap()
            .unwrap();
        drop(permits);
        assert_eq!(slots.available_permits(), 2);
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::{PiProbeController, PiProbeData, ProbeFailure};
    use gpui_kit::Task;
    use gpui_operation::{Complete, Refresh, Settle, Transition};

    #[test]
    fn readiness_requires_the_committed_command_and_a_completed_success() {
        let mut probe = PiProbeController::new();
        probe.requested = Some("replacement-pi".into());
        probe.operation.transition(Settle(Ok(PiProbeData {
            command: "resolved/replacement-pi".into(),
            version: "0.85.1".into(),
        })));
        // A successful draft check cannot authorize the previously applied command.
        assert!(probe.ready_for(Some("invalid-pi")).is_none());
        assert!(probe.ready_for(None).is_none());
        assert!(probe.ready_for(Some("replacement-pi")).is_some());
        assert!(probe.ready_for(Some("  replacement-pi  ")).is_some());

        // Retained data cannot authorize startup during or after a failed refresh.
        probe.operation.transition(Refresh(Task::ready(())));
        assert!(probe.ready_for(Some("replacement-pi")).is_none());
        probe
            .operation
            .transition(Complete(Err(ProbeFailure::InvalidVersion)));
        assert!(probe.operation.data().is_some());
        assert!(probe.ready_for(Some("replacement-pi")).is_none());
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{
        Arc, Duration, Instant, PiProbeController, PiProbeData, ProbeFailure, Semaphore, Stdio,
        Tokio, oneshot, probe, probe_with, resolve_command, spawn_command,
    };
    use gpui_kit as gpui;
    use gpui_kit::{AppContext, TestAppContext};
    use std::os::unix::fs::PermissionsExt;
    async fn fixture(body: &str, timeout: Duration) -> Result<PiProbeData, ProbeFailure> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        probe(
            path.to_string_lossy().into_owned(),
            Instant::now() + timeout,
        )
        .await
    }
    #[tokio::test]
    async fn process_contract_is_bounded() {
        assert_eq!(
            fixture("echo 0.85.1", Duration::from_secs(3))
                .await
                .unwrap()
                .version,
            "0.85.1"
        );
        assert!(matches!(
            fixture("echo bad", Duration::from_secs(3)).await,
            Err(ProbeFailure::InvalidVersion)
        ));
        assert!(matches!(
            fixture("while :; do :; done", Duration::from_millis(30)).await,
            Err(ProbeFailure::Timeout)
        ));
        assert!(matches!(
            fixture(
                "while :; do echo 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done",
                Duration::from_secs(3)
            )
            .await,
            Err(ProbeFailure::OutputLimit)
        ));
    }
    #[tokio::test]
    async fn timeout_reaps_and_cancellation_terminates_the_owned_process() {
        for cancel_early in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("pi");
            let pid_path = dir.path().join("pid");
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\necho $$ > '{}'\nwhile :; do :; done\n",
                    pid_path.display()
                ),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let task = tokio::spawn(probe(
                path.to_string_lossy().into_owned(),
                Instant::now() + Duration::from_secs(5),
            ));
            {
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !pid_path.exists() {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
                if cancel_early {
                    task.abort();
                }
            }
            let result = task.await;
            if cancel_early {
                assert!(result.is_err_and(|e| e.is_cancelled()));
            } else {
                assert!(matches!(result, Ok(Err(ProbeFailure::Timeout))));
            }
            let pid = std::fs::read_to_string(pid_path).unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                while std::process::Command::new("/bin/kill")
                    .args(["-0", pid.trim()])
                    .stdout(std::process::Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
                {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("owned probe process must terminate");
        }
    }

    #[tokio::test]
    async fn cancelled_receiver_terminates_a_late_spawned_child() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi");
        std::fs::write(&path, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (started, entered) = oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let slots = Arc::new(Semaphore::new(2));
        let task = tokio::spawn(probe_with(
            path.to_string_lossy().into_owned(),
            Instant::now() + Duration::from_secs(5),
            slots.clone(),
            resolve_command,
            move |path| {
                let child = spawn_command(path)?;
                started.send(child.id().unwrap()).unwrap();
                // Hold the real child before delivery, as a slow spawn would.
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(child)
            },
        ));
        let pid = entered.await.unwrap();
        task.abort();
        assert!(task.await.is_err_and(|e| e.is_cancelled()));
        release.send(()).unwrap();
        let permits = tokio::time::timeout(Duration::from_secs(2), slots.acquire_many(2))
            .await
            .unwrap()
            .unwrap();
        drop(permits);
        tokio::time::timeout(Duration::from_secs(2), async {
            while std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("late child must be terminated and reaped");
    }

    #[gpui::test]
    async fn replacing_and_stopping_a_probe_cancel_its_completion(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_tokio::init);
        let dir = tempfile::tempdir().unwrap();
        let old_path = dir.path().join("old-pi");
        let new_path = dir.path().join("new-pi");
        let pid_path = dir.path().join("pid");
        std::fs::write(
            &old_path,
            format!(
                "#!/bin/sh\necho $$ > '{}'\nwhile :; do :; done\n",
                pid_path.display()
            ),
        )
        .unwrap();
        std::fs::write(&new_path, "#!/bin/sh\necho 0.85.2\n").unwrap();
        for path in [&old_path, &new_path] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let old = old_path.to_string_lossy().into_owned();
        let new = new_path.to_string_lossy().into_owned();
        let owner = cx.new(|_| PiProbeController::new());
        owner.update(cx, |owner, cx| {
            owner.request(Some(old.clone()), true, cx);
            assert!(owner.operation.is_running());
        });
        cx.update(|cx| {
            Tokio::spawn(cx, async move {
                tokio::time::timeout(Duration::from_secs(2), async {
                    while !pid_path.exists() {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
            })
        })
        .await
        .unwrap();
        owner.update(cx, |owner, cx| {
            owner.request(Some(new.clone()), true, cx);
            assert!(owner.matches_command(Some(&new)));
            assert!(owner.operation.is_running());
        });
        cx.condition(&owner, |owner, _| !owner.operation.is_running())
            .await;
        owner.read_with(cx, |owner, _| {
            assert_eq!(owner.ready_for(Some(&new)).unwrap().version, "0.85.2");
            assert!(owner.ready_for(Some(&old)).is_none());
        });
        owner.update(cx, |owner, cx| {
            owner.request(Some(old), true, cx);
            assert!(owner.operation.is_running());
            owner.stop();
            assert!(
                !owner.operation.is_running(),
                "quit must not wait for the probe"
            );
            owner.request(Some(new), true, cx);
            assert!(!owner.operation.is_running(), "draining rejects new probes");
        });
    }
}
