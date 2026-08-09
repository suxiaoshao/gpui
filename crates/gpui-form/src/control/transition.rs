use gpui_operation::Transition;

use crate::FormRevision;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lifecycle {
    Active,
    Retired,
    Dropped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingProjection {
    Value {
        revision: FormRevision,
        editor_sequence: u64,
    },
    Retired,
}

pub(super) struct BindingState {
    lifecycle: Lifecycle,
    pending: Option<PendingProjection>,
    drain_scheduled: bool,
    suppressed_through: FormRevision,
}

impl BindingState {
    pub(super) const fn new() -> Self {
        Self {
            lifecycle: Lifecycle::Active,
            pending: None,
            drain_scheduled: false,
            suppressed_through: FormRevision::INITIAL,
        }
    }

    pub(super) const fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }
}

pub(super) enum Message {
    QueueValue {
        revision: FormRevision,
        editor_sequence: u64,
    },
    SuppressThrough(FormRevision),
    Retire,
    Drain,
    Drop,
}

pub(super) enum Effect {
    None,
    ScheduleDrain,
    Deliver(PendingProjection),
}

impl Transition<Message> for &mut BindingState {
    type Output = Effect;

    fn transition(self, message: Message) -> Self::Output {
        match message {
            Message::QueueValue {
                revision,
                editor_sequence,
            } => {
                if self.lifecycle != Lifecycle::Active || revision <= self.suppressed_through {
                    return Effect::None;
                }
                self.pending = Some(PendingProjection::Value {
                    revision,
                    editor_sequence,
                });
                if self.drain_scheduled {
                    Effect::None
                } else {
                    self.drain_scheduled = true;
                    Effect::ScheduleDrain
                }
            }
            Message::SuppressThrough(revision) => {
                self.suppressed_through = self.suppressed_through.max(revision);
                if matches!(
                    self.pending,
                    Some(PendingProjection::Value {
                        revision: pending,
                        ..
                    }) if pending <= revision
                ) {
                    self.pending = None;
                }
                Effect::None
            }
            Message::Retire => {
                if self.lifecycle != Lifecycle::Active {
                    return Effect::None;
                }
                self.lifecycle = Lifecycle::Retired;
                self.pending = Some(PendingProjection::Retired);
                if self.drain_scheduled {
                    Effect::None
                } else {
                    self.drain_scheduled = true;
                    Effect::ScheduleDrain
                }
            }
            Message::Drain => {
                self.drain_scheduled = false;
                match self.pending.take() {
                    Some(PendingProjection::Retired) if self.lifecycle == Lifecycle::Retired => {
                        Effect::Deliver(PendingProjection::Retired)
                    }
                    Some(value @ PendingProjection::Value { .. })
                        if self.lifecycle == Lifecycle::Active =>
                    {
                        Effect::Deliver(value)
                    }
                    _ => Effect::None,
                }
            }
            Message::Drop => {
                self.lifecycle = Lifecycle::Dropped;
                self.pending = None;
                self.drain_scheduled = false;
                Effect::None
            }
        }
    }
}
