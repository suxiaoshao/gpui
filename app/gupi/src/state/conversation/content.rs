//! Transcript availability is independent of the Pi handshake and auxiliary reads.
use super::{Session, loading::CoreRead};
use crate::state::history::History;
use pi_rpc::protocol::Entries;
use std::sync::LazyLock;

pub(super) enum Transcript {
    New,
    Unloaded,
    Ready(History),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoadStage {
    CheckingFile,
    Connecting,
    History,
}

/// A derived presentation: views must exhaustively handle every lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodyState<'a> {
    New,
    Loading(LoadStage),
    Ready,
    Refreshing(LoadStage),
    Failed(&'a str),
    RefreshFailed(&'a str),
}

impl Transcript {
    pub fn history(&self) -> &History {
        static EMPTY: LazyLock<History> = LazyLock::new(History::default);
        match self {
            Self::New | Self::Unloaded => &EMPTY,
            Self::Ready(history) => history,
        }
    }

    pub fn replace(&mut self, entries: Entries) {
        match self {
            Self::New | Self::Unloaded => {
                let mut history = History::default();
                history.replace(entries);
                *self = Self::Ready(history);
            }
            Self::Ready(history) => history.replace(entries),
        }
    }

    pub fn receive_message(&mut self) {
        match self {
            Self::New => *self = Self::Ready(History::default()),
            Self::Unloaded | Self::Ready(_) => {}
        }
    }
}

impl Session {
    /// Empty projections are useful to row builders, but never determine readiness.
    pub fn history(&self) -> &History {
        self.transcript.history()
    }

    pub fn body_state(&self) -> BodyState<'_> {
        match (&self.transcript, &self.core_read) {
            (
                Transcript::New,
                CoreRead::Idle | CoreRead::CheckingFile { .. } | CoreRead::Reading { .. },
            ) => BodyState::New,
            (Transcript::New | Transcript::Unloaded, CoreRead::Failed(error)) => {
                BodyState::Failed(error)
            }
            (Transcript::Unloaded, CoreRead::Idle) => match self.error.as_deref() {
                Some(error) => BodyState::Failed(error),
                None => BodyState::Loading(LoadStage::Connecting),
            },
            (Transcript::Unloaded, CoreRead::CheckingFile { .. }) => {
                BodyState::Loading(LoadStage::CheckingFile)
            }
            (Transcript::Unloaded, CoreRead::Reading { .. }) => {
                BodyState::Loading(LoadStage::History)
            }
            (Transcript::Ready(_), CoreRead::Idle) => BodyState::Ready,
            (Transcript::Ready(_), CoreRead::CheckingFile { .. }) => {
                BodyState::Refreshing(LoadStage::CheckingFile)
            }
            (Transcript::Ready(_), CoreRead::Reading { .. }) => {
                BodyState::Refreshing(LoadStage::History)
            }
            (Transcript::Ready(_), CoreRead::Failed(error)) => BodyState::RefreshFailed(error),
        }
    }
}
