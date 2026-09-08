use gpui_kit::*;
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_tokio::Tokio;
use std::{
    path::PathBuf,
    process::Stdio,
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::oneshot,
};

const OUTPUT_LIMIT: u64 = 16 * 1024;
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
    #[error("probe cancelled")]
    Cancelled,
}
impl ProbeFailure {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Timeout => "error-pi-timeout",
            Self::OutputLimit => "error-pi-output",
            Self::InvalidVersion => "error-pi-version",
            Self::Cancelled => "startup-checking",
            Self::Io(_) | Self::Command(_) => "error-pi-probe",
        }
    }
}
#[derive(Clone)]
pub(crate) struct PiProbeData {
    pub command: PathBuf,
    pub version: String,
    pub checked_at: SystemTime,
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
async fn probe(
    command: String,
    timeout: Duration,
    mut cancel: oneshot::Receiver<()>,
) -> Result<PiProbeData, ProbeFailure> {
    let deadline = tokio::time::Instant::now() + timeout;
    let path = which::which(&command).map_err(|e| ProbeFailure::Command(e.to_string()))?;
    let mut process = Command::new(&path);
    process
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    process.creation_flags(0x08000000);
    let mut child = process.spawn()?;
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
                checked_at: SystemTime::now(),
            })
        };
        tokio::select! {
            result = tokio::time::timeout_at(deadline, attempt) => result.unwrap_or(Err(ProbeFailure::Timeout)),
            _ = &mut cancel => Err(ProbeFailure::Cancelled),
        }
    };
    if result.is_err() && child.try_wait()?.is_none() {
        child.kill().await?;
        child.wait().await?;
    }
    result
}
pub(crate) struct PiProbeController {
    pub operation: PiOperation,
    requested: Option<String>,
    cancel: Option<oneshot::Sender<()>>,
    pub draining: bool,
}
impl PiProbeController {
    pub fn new() -> Self {
        Self {
            operation: PiOperation::new(),
            requested: None,
            cancel: None,
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
        self.requested = Some(command.clone());
        if self.operation.is_running() {
            if let Some(cancel) = self.cancel.take() {
                let _ = cancel.send(());
            }
            return;
        }
        if changed {
            self.operation = PiOperation::new();
        }
        self.start(command, cx);
    }
    pub fn stop(&mut self) {
        self.draining = true;
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
    fn start(&mut self, command: String, cx: &mut Context<Self>) {
        let (send, recv) = oneshot::channel();
        self.cancel = Some(send);
        let input = command.clone();
        let worker = Tokio::spawn(cx, probe(command, Duration::from_secs(5), recv));
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
                owner.cancel = None;
                owner.operation.transition(Complete(result));
                if !owner.draining && owner.requested.as_ref() != Some(&input) {
                    owner.operation = PiOperation::new();
                    if let Some(next) = owner.requested.clone() {
                        owner.start(next, cx);
                    }
                }
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
mod readiness_tests {
    use super::{PiProbeController, PiProbeData, ProbeFailure};
    use gpui_kit::Task;
    use gpui_operation::{Complete, Refresh, Settle, Transition};
    use std::time::SystemTime;

    #[test]
    fn readiness_requires_the_committed_command_and_a_completed_success() {
        let mut probe = PiProbeController::new();
        probe.requested = Some("replacement-pi".into());
        probe.operation.transition(Settle(Ok(PiProbeData {
            command: "resolved/replacement-pi".into(),
            version: "0.85.1".into(),
            checked_at: SystemTime::now(),
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
    use super::{Duration, PiProbeData, ProbeFailure, oneshot, probe};
    use std::os::unix::fs::PermissionsExt;
    async fn fixture(body: &str, timeout: Duration) -> Result<PiProbeData, ProbeFailure> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (_send, recv) = oneshot::channel();
        probe(path.to_string_lossy().into_owned(), timeout, recv).await
    }
    #[tokio::test]
    async fn process_contract_is_bounded() {
        assert_eq!(
            fixture("echo 0.85.1", Duration::from_secs(1))
                .await
                .unwrap()
                .version,
            "0.85.1"
        );
        assert!(matches!(
            fixture("echo bad", Duration::from_secs(1)).await,
            Err(ProbeFailure::InvalidVersion)
        ));
        assert!(matches!(
            fixture("while :; do :; done", Duration::from_millis(30)).await,
            Err(ProbeFailure::Timeout)
        ));
        assert!(matches!(
            fixture(
                "while :; do echo 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done",
                Duration::from_secs(1)
            )
            .await,
            Err(ProbeFailure::OutputLimit)
        ));
    }
    #[tokio::test]
    async fn cancellation_and_timeout_reap_the_owned_process() {
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
            let (send, recv) = oneshot::channel();
            let task = tokio::spawn(probe(
                path.to_string_lossy().into_owned(),
                Duration::from_secs(2),
                recv,
            ));
            let mut signal = Some(send);
            {
                tokio::time::timeout(Duration::from_secs(1), async {
                    while !pid_path.exists() {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
                if cancel_early {
                    signal.take().unwrap().send(()).unwrap();
                }
            }
            let result = task.await.unwrap();
            drop(signal);
            assert!(matches!(
                result,
                Err(ProbeFailure::Cancelled | ProbeFailure::Timeout)
            ));
            let pid = std::fs::read_to_string(pid_path).unwrap();
            assert!(
                !std::process::Command::new("/bin/kill")
                    .args(["-0", pid.trim()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
}
