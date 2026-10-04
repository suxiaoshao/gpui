mod collector;
mod data;
mod decoding;
pub mod prepared;
mod problem;
pub mod transport;
pub use data::*;
pub use decoding::*;
pub use problem::*;

#[derive(Clone)]
#[non_exhaustive]
pub struct ResponseReceipt {
    pub head: ResponseHead,
    pub progress: ResponseProgress,
    pub head_after: std::time::Duration,
}
#[derive(Clone)]
#[non_exhaustive]
pub struct FailedAttempt {
    pub problem: RequestProblem,
    pub receipt: Option<ResponseReceipt>,
    pub failed_after: std::time::Duration,
}
/// Immutable display projection; execution task ownership stays in RequestView.
#[derive(Clone, Default)]
pub enum ResponseState {
    #[default]
    Idle,
    Sending,
    Receiving {
        receipt: ResponseReceipt,
    },
    Ready {
        response: std::sync::Arc<ResponseData>,
    },
    Failed {
        attempt: FailedAttempt,
    },
}
impl ResponseState {
    pub fn response(&self) -> Option<&std::sync::Arc<ResponseData>> {
        if let Self::Ready { response } = self {
            Some(response)
        } else {
            None
        }
    }
}

impl ResponseReceipt {
    pub fn new(
        head: ResponseHead,
        progress: ResponseProgress,
        head_after: std::time::Duration,
    ) -> Self {
        Self {
            head,
            progress,
            head_after,
        }
    }
}

impl FailedAttempt {
    pub fn new(
        problem: RequestProblem,
        receipt: Option<ResponseReceipt>,
        failed_after: std::time::Duration,
    ) -> Self {
        Self {
            problem,
            receipt,
            failed_after,
        }
    }
}
