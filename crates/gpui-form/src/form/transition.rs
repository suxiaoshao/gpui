use gpui_operation::Transition;

use crate::PathKey;

use super::{FormEvent, FormRevision};

pub(super) struct Runtime {
    revision: FormRevision,
}

impl Runtime {
    pub(super) const fn new() -> Self {
        Self {
            revision: FormRevision::INITIAL,
        }
    }

    pub(super) const fn revision(&self) -> FormRevision {
        self.revision
    }
}

pub(super) enum Message {
    Commit(PathKey),
    ReplaceModel,
    ValidationChanged,
}

pub(super) enum Effect {
    Committed(FormEvent),
    Validation(FormEvent),
}

impl Transition<Message> for &mut Runtime {
    type Output = Effect;

    fn transition(self, message: Message) -> Self::Output {
        match message {
            Message::Commit(path) => {
                self.revision.0 = self
                    .revision
                    .0
                    .checked_add(1)
                    .expect("form revision overflow");
                Effect::Committed(FormEvent::Committed {
                    path,
                    revision: self.revision,
                })
            }
            Message::ReplaceModel => {
                self.revision.0 = self
                    .revision
                    .0
                    .checked_add(1)
                    .expect("form revision overflow");
                Effect::Committed(FormEvent::ModelReplaced {
                    revision: self.revision,
                })
            }
            Message::ValidationChanged => Effect::Validation(FormEvent::ValidationChanged {
                revision: self.revision,
            }),
        }
    }
}
