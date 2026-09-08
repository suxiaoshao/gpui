use gpui_kit::*;
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_tokio::Tokio;
use pi_rpc::probe::{PROBE_TIMEOUT, PiProbeData, ProbeFailure, probe};
use tokio::time::Instant;

type PiOperation = refresh::Operation<PiProbeData, ProbeFailure, Task<()>>;
pub(crate) trait ProbeFailureKey {
    fn key(&self) -> &'static str;
}
impl ProbeFailureKey for ProbeFailure {
    fn key(&self) -> &'static str {
        match self {
            Self::Timeout => "error-pi-timeout",
            Self::OutputLimit => "error-pi-output",
            Self::InvalidVersion => "error-pi-version",
            Self::Io(_) | Self::Command(_) => "error-pi-probe",
        }
    }
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
    use super::PiProbeController;
    use gpui_kit as gpui;
    use gpui_kit::{AppContext, TestAppContext};
    use gpui_tokio::Tokio;
    use std::{os::unix::fs::PermissionsExt, time::Duration};
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
