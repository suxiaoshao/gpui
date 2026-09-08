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
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
static STARTUP_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
#[derive(Debug, thiserror::Error)]
pub enum ProbeFailure {
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
#[derive(Clone, Debug)]
pub struct PiProbeData {
    pub command: PathBuf,
    pub version: String,
}
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
pub async fn probe(command: String, deadline: Instant) -> Result<PiProbeData, ProbeFailure> {
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

#[cfg(all(test, unix))]
mod tests {
    use super::{
        Arc, Duration, Instant, PiProbeData, ProbeFailure, Semaphore, Stdio, oneshot, probe,
        probe_with, resolve_command, spawn_command,
    };
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
}
