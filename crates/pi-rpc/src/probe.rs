use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
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
    probe_with_env(command, Vec::new(), deadline).await
}
pub async fn probe_with_env(
    command: String,
    env: Vec<(OsString, OsString)>,
    deadline: Instant,
) -> Result<PiProbeData, ProbeFailure> {
    let slots = STARTUP_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(2)))
        .clone();
    let resolve_env = env.clone();
    probe_with(
        command,
        deadline,
        slots,
        move |command| resolve_command_with_env(command, &resolve_env),
        move |path| spawn_command_with_env(path, &env),
    )
    .await
}
fn resolve_command_with_env(
    command: &str,
    env: &[(OsString, OsString)],
) -> Result<PathBuf, ProbeFailure> {
    let paths = env
        .iter()
        .rev()
        .find(|(name, _)| is_path_env(name))
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os("PATH"));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    which::which_in(command, paths, cwd).map_err(|e| ProbeFailure::Command(e.to_string()))
}
fn is_path_env(name: &OsStr) -> bool {
    if cfg!(windows) {
        name.to_string_lossy().eq_ignore_ascii_case("PATH")
    } else {
        name == "PATH"
    }
}
fn spawn_command_with_env(
    path: &Path,
    env: &[(OsString, OsString)],
) -> Result<Child, ProbeFailure> {
    let mut process = Command::new(path);
    process
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    process.envs(env.iter().cloned());
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
    {
        let attempt = async {
            let (stdout, _, status) = tokio::try_join!(bounded(stdout), bounded(stderr), async {
                child.wait().await.map_err(ProbeFailure::Io)
            })?;
            let version = std::str::from_utf8(&stdout)
                .map_err(|_| ProbeFailure::InvalidVersion)?
                .trim();
            if !status.success()
                || semver::Version::parse(version.strip_prefix('v').unwrap_or(version)).is_err()
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
    }
}
