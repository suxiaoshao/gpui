use std::collections::{HashMap, HashSet};

use gpui::Task;
use gpui_operation::Transition;

use crate::topology::CanonicalAddress;

pub(crate) struct AsyncValidationRuntime {
    next_generation: u64,
    pending: HashMap<u64, PendingValidation>,
}

struct PendingValidation {
    address: CanonicalAddress,
    task: Option<Task<()>>,
}

impl AsyncValidationRuntime {
    pub(crate) fn new() -> Self {
        Self {
            next_generation: 1,
            pending: HashMap::new(),
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

pub(crate) enum Message {
    Reserve { address: CanonicalAddress },
    Attach { generation: u64, task: Task<()> },
    CancelIntersecting { address: CanonicalAddress },
    CancelAll,
    Complete { generation: u64, fresh: bool },
}

pub(crate) enum Effect {
    Reserved(u64),
    Cancelled(HashSet<u64>),
    Completed { accepted: bool },
    None,
}

impl Transition<Message> for &mut AsyncValidationRuntime {
    type Output = Effect;

    fn transition(self, message: Message) -> Self::Output {
        match message {
            Message::Reserve { address } => {
                let generation = self.next_generation;
                self.next_generation = generation
                    .checked_add(1)
                    .expect("async validation generation space exhausted");
                self.pending.insert(
                    generation,
                    PendingValidation {
                        address,
                        task: None,
                    },
                );
                Effect::Reserved(generation)
            }
            Message::Attach { generation, task } => {
                if let Some(pending) = self.pending.get_mut(&generation) {
                    pending.task = Some(task);
                }
                Effect::None
            }
            Message::CancelIntersecting { address } => {
                let generations = self
                    .pending
                    .iter()
                    .filter_map(|(generation, pending)| {
                        super::intersects(&address, &pending.address).then_some(*generation)
                    })
                    .collect::<HashSet<_>>();
                self.pending
                    .retain(|generation, _| !generations.contains(generation));
                Effect::Cancelled(generations)
            }
            Message::CancelAll => {
                let generations = self.pending.keys().copied().collect::<HashSet<_>>();
                self.pending.clear();
                Effect::Cancelled(generations)
            }
            Message::Complete { generation, fresh } => {
                let existed = self.pending.remove(&generation).is_some();
                Effect::Completed {
                    accepted: existed && fresh,
                }
            }
        }
    }
}
