mod binding;
mod delta;
mod error;
mod local;
mod selection;
mod shared;
mod source;
mod store;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;

pub use binding::StoreBinding;
pub use delta::StoreDelta;
pub use error::StoreSourceUnsupported;
pub use local::LocalStore;
pub use selection::StoreSelection;
pub use shared::{SharedStore, StoreRuntime};
pub use source::{
    MemorySource, StoreSource, StoreSourceBuilder, StoreSourceCallback, StoreSourceFuture,
    StoreSourceId, StoreSourcePolicy, StoreSourceWriteAck,
};
pub use store::{StoreCore, StoreRevision, StoreState, StoreUpdate, StoreUpdateOrigin};

pub(crate) use selection::SnapshotCell;
