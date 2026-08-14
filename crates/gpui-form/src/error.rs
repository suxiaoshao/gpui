use std::{error::Error, fmt};

use crate::{PathKey, ValidationReport};

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveError {
    WrongSession {
        path: PathKey,
    },
    Retired {
        path: PathKey,
    },
    MissingOptional {
        path: PathKey,
    },
    InactiveCase {
        path: PathKey,
        expected: &'static str,
    },
    MissingItem {
        path: PathKey,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSession { .. } => f.write_str("the path belongs to another form session"),
            Self::Retired { .. } => f.write_str("the dynamic form path has been retired"),
            Self::MissingOptional { .. } => f.write_str("the optional form value is absent"),
            Self::InactiveCase { expected, .. } => {
                write!(f, "the enum case `{expected}` is not active")
            }
            Self::MissingItem { .. } => f.write_str("the form item no longer exists"),
        }
    }
}

impl Error for ResolveError {}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TopologyError {
    WrongCollection { path: PathKey },
    InvalidAnchor { path: PathKey },
    MoveIntoDescendant { path: PathKey },
}

impl fmt::Display for TopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongCollection { .. } => {
                f.write_str("the item does not belong to this collection")
            }
            Self::InvalidAnchor { .. } => f.write_str("the collection anchor is invalid"),
            Self::MoveIntoDescendant { .. } => {
                f.write_str("an item cannot be moved into its own descendant")
            }
        }
    }
}

impl Error for TopologyError {}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MutationError {
    Resolve(ResolveError),
    Topology(TopologyError),
}

impl fmt::Display for MutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve(error) => error.fmt(f),
            Self::Topology(error) => error.fmt(f),
        }
    }
}

impl Error for MutationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resolve(error) => Some(error),
            Self::Topology(error) => Some(error),
        }
    }
}

impl From<ResolveError> for MutationError {
    fn from(value: ResolveError) -> Self {
        Self::Resolve(value)
    }
}

impl From<TopologyError> for MutationError {
    fn from(value: TopologyError) -> Self {
        Self::Topology(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrepareError {
    Validation(ValidationReport),
    ValidationPending,
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(_) => f.write_str("form validation failed"),
            Self::ValidationPending => f.write_str("form validation is still running"),
        }
    }
}

impl Error for PrepareError {}
