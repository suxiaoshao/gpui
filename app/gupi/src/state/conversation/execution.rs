//! Event-owned execution facts. Cached RPC snapshots are not a second lifecycle.
use serde_json::Value;
use std::collections::HashSet;

#[derive(Default)]
pub(super) enum RunState {
    #[default]
    Idle,
    Active {
        messages: HashSet<String>,
    },
}
impl RunState {
    pub fn active_messages(&self) -> Option<&HashSet<String>> {
        match self {
            Self::Idle => None,
            Self::Active { messages } => Some(messages),
        }
    }
    pub fn start(&mut self) {
        *self = Self::Active {
            messages: HashSet::new(),
        };
    }
    pub fn observe_streaming(&mut self, streaming: bool) {
        // agent_end may precede queued work/retries. Only agent_settled ends a run.
        if streaming && self.active_messages().is_none() {
            self.start();
        }
    }
    pub fn record_message(&mut self, signature: String) {
        if let Self::Active { messages } = self {
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
