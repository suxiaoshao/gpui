use std::marker::PhantomData;

use gpui_operation::Transition;

use crate::{
    FormEvent, FormRevision, FormSchema, ModelChange, ModelChangeKind,
    change::{ChangeSet, ControlOrigin},
    topology::SessionId,
};

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

pub(super) enum Message<M: FormSchema> {
    ModelApplied {
        kind: ModelChangeKind,
        session: SessionId,
        changes: ChangeSet,
        origin: Option<ControlOrigin>,
        marker: PhantomData<fn() -> M>,
    },
    ValidationChanged(PhantomData<fn() -> M>),
}

impl<M: FormSchema> Message<M> {
    pub(super) fn model_applied(
        kind: ModelChangeKind,
        session: SessionId,
        changes: ChangeSet,
        origin: Option<ControlOrigin>,
    ) -> Self {
        Self::ModelApplied {
            kind,
            session,
            changes,
            origin,
            marker: PhantomData,
        }
    }

    pub(super) fn validation_changed() -> Self {
        Self::ValidationChanged(PhantomData)
    }
}

pub(super) enum Effect<M: FormSchema> {
    Publish(FormEvent<M>),
}

impl<M: FormSchema> Transition<Message<M>> for &mut Runtime {
    type Output = Effect<M>;

    fn transition(self, message: Message<M>) -> Self::Output {
        match message {
            Message::ModelApplied {
                kind,
                session,
                changes,
                origin,
                ..
            } => {
                self.revision.0 = self
                    .revision
                    .0
                    .checked_add(1)
                    .expect("form revision overflow");
                Effect::Publish(FormEvent::ModelChanged(ModelChange::new(
                    self.revision,
                    kind,
                    session,
                    changes,
                    origin,
                )))
            }
            Message::ValidationChanged(_) => Effect::Publish(FormEvent::ValidationChanged {
                revision: self.revision,
            }),
        }
    }
}
