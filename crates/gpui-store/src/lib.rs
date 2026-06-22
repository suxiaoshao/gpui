mod backend;
mod binding;
mod delta;
mod error;
mod local;
mod selection;
mod shared;
mod store;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;

pub use backend::{
    MemoryBackend, StoreBackend, StoreBackendBuilder, StoreBackendCallback, StoreBackendFuture,
    StoreBackendId, StoreCommitAck, StoreCommitBackend,
};
pub use binding::StoreBinding;
pub use delta::StoreDelta;
pub use error::StoreBackendUnsupported;
pub use local::LocalStore;
pub use selection::StoreSelection;
pub use shared::{SharedStore, StoreRuntime};
pub use store::{StoreCore, StoreRevision, StoreState, StoreUpdate, StoreUpdateOrigin};

pub(crate) use selection::SnapshotCell;
