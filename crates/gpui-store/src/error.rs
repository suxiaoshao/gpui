use std::fmt;

use crate::StoreBackendId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreBackendUnsupported {
    backend_id: StoreBackendId,
    operation: &'static str,
}

impl StoreBackendUnsupported {
    pub fn new(backend_id: StoreBackendId, operation: &'static str) -> Self {
        Self {
            backend_id,
            operation,
        }
    }

    pub fn backend_id(&self) -> &StoreBackendId {
        &self.backend_id
    }

    pub fn operation(&self) -> &'static str {
        self.operation
    }
}

impl fmt::Display for StoreBackendUnsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "store backend '{}' does not support {}",
            self.backend_id, self.operation
        )
    }
}

impl std::error::Error for StoreBackendUnsupported {}
