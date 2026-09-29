use gpui_kit::*;
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use gpui_tokio::Tokio;
use pi_rpc::probe::{PROBE_TIMEOUT, PiProbeData, ProbeFailure, probe_with_env};
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
        self.start(command, retry, cx);
    }
    pub fn stop(&mut self) {
        self.draining = true;
        self.operation.transition(Cancel);
    }
    fn start(&mut self, command: String, reload_environment: bool, cx: &mut Context<Self>) {
        let environment = crate::state::environment::current(cx);
        let worker = Tokio::spawn(cx, async move {
            let snapshot = environment.load(reload_environment).await;
            probe_with_env(
                command,
                snapshot.variables(),
                Instant::now() + PROBE_TIMEOUT,
            )
            .await
            .map_err(|error| match error {
                ProbeFailure::Command(error) => ProbeFailure::Command(snapshot.explain(error)),
                ProbeFailure::Io(error) => ProbeFailure::Command(snapshot.explain(error)),
                error => error,
            })
        });
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
