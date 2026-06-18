use std::fmt;

use crate::StoreSourceId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreSourceUnsupported {
    source_id: StoreSourceId,
    operation: &'static str,
}

impl StoreSourceUnsupported {
    pub fn new(source_id: StoreSourceId, operation: &'static str) -> Self {
        Self {
            source_id,
            operation,
        }
    }

    pub fn source_id(&self) -> &StoreSourceId {
        &self.source_id
    }

    pub fn operation(&self) -> &'static str {
        self.operation
    }
}

impl fmt::Display for StoreSourceUnsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "store source '{}' does not support {}",
            self.source_id, self.operation
        )
    }
}

impl std::error::Error for StoreSourceUnsupported {}
