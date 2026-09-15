//! Event-owned execution facts. Cached RPC snapshots are not a second lifecycle.
use serde_json::Value;
use std::collections::HashSet;

#[derive(Default)]
pub(super) enum RunState {
    #[default]
    Idle,
    Active {
        messages: HashSet<String>,
        started_at: i64,
    },
}
impl RunState {
    pub fn active_messages(&self) -> Option<&HashSet<String>> {
        match self {
            Self::Idle => None,
            Self::Active { messages, .. } => Some(messages),
        }
    }
    pub fn started_at(&self) -> Option<i64> {
        match self {
            Self::Idle => None,
            Self::Active { started_at, .. } => Some(*started_at),
        }
    }
    pub fn start(&mut self) {
        *self = Self::Active {
            messages: HashSet::new(),
            started_at: (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
        };
    }
    pub fn observe_streaming(&mut self, streaming: bool) {
        // agent_end may precede queued work/retries. Only agent_settled ends a run.
        if streaming && self.active_messages().is_none() {
            self.start();
        }
    }
    pub fn record_message(&mut self, signature: String) {
        if let Self::Active { messages, .. } = self {
            messages.insert(signature);
        }
    }
}

#[derive(Clone)]
pub(crate) enum ToolExecution {
    Running(Value),
    Complete(Value),
    Failed(Value),
}
