mod asset;
pub(super) mod audio;
mod session;
pub(super) mod video;

use std::{fmt, sync::Arc};

use super::{ResponseData, ViewerMode};

pub(crate) use asset::{ResponseAssetLease, ResponseAssetProblem, ResponseAssetProblemKind};
pub(crate) use session::{
    DecoderPolicy, MediaCommand, MediaDriver, MediaDriverEvent, MediaDriverEvents, MediaKind,
    MediaMessage, MediaMetadata, MediaPhase, MediaPosition, MediaProblem, MediaProblemDetail,
    MediaProblemKind, MediaRuntime,
};

/// Response-pane-owned identity for every asynchronous preview event.
///
/// The pane creates a new token whenever the response or selected preview mode
/// changes. Media code only compares it; it never derives a token from an
/// address, URL, or response bytes.
#[derive(Clone)]
pub(crate) struct PreviewToken {
    response: Arc<ResponseData>,
    requested_mode: ViewerMode,
    effective_mode: ViewerMode,
    generation: u64,
}

impl PreviewToken {
    pub(crate) fn new(
        response: Arc<ResponseData>,
        requested_mode: ViewerMode,
        effective_mode: ViewerMode,
        generation: u64,
    ) -> Self {
        Self {
            response,
            requested_mode,
            effective_mode,
            generation,
        }
    }

    pub(crate) fn response(&self) -> &Arc<ResponseData> {
        &self.response
    }

    pub(crate) const fn effective_mode(&self) -> ViewerMode {
        self.effective_mode
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.requested_mode == other.requested_mode
            && self.effective_mode == other.effective_mode
            && Arc::ptr_eq(&self.response, &other.response)
    }
}

impl fmt::Debug for PreviewToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreviewToken")
            .field("requested_mode", &self.requested_mode)
            .field("effective_mode", &self.effective_mode)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}
