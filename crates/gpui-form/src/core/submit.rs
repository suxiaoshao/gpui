use gpui::Task;

use crate::FormValidationReport;

#[derive(Debug, Default)]
pub struct SubmitRuntime {
    task: Option<Task<()>>,
    submission_attempts: u32,
    last_outcome: Option<SubmitOutcome>,
}

impl SubmitRuntime {
    pub fn is_submitting(&self) -> bool {
        self.task.is_some()
    }

    pub fn submission_attempts(&self) -> u32 {
        self.submission_attempts
    }

    pub fn last_outcome(&self) -> Option<SubmitOutcome> {
        self.last_outcome
    }

    pub fn begin_submit(&mut self) {
        self.last_outcome = None;
        self.submission_attempts = self.submission_attempts.saturating_add(1);
    }

    pub fn set_task(&mut self, task: Task<()>) {
        self.task = Some(task);
    }

    pub fn clear_task(&mut self) {
        self.task = None;
    }

    pub fn finish_success(&mut self) {
        self.clear_task();
        self.last_outcome = Some(SubmitOutcome::Success);
    }

    pub fn finish_failure(&mut self) {
        self.clear_task();
        self.last_outcome = Some(SubmitOutcome::Failure);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitOutcome {
    Success,
    Failure,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SubmitError<E> {
    Invalid(FormValidationReport),
    Busy,
    Handler(E),
}
