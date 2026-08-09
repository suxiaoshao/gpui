use std::fmt;

use crate::{FormRevision, topology::SessionId};

/// An opaque optimistic-concurrency token bound to one Form editing session.
///
/// A version can only be obtained from [`Prepared`]. It deliberately exposes
/// neither its session identity nor its revision: callers can retain and pass
/// it back to `Form::rebase_if_current`, but cannot manufacture a matching
/// token for another editing session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormVersion {
    session: SessionId,
    revision: FormRevision,
}

impl FormVersion {
    pub(crate) const fn new(session: SessionId, revision: FormRevision) -> Self {
        Self { session, revision }
    }

    pub(crate) fn is_current(self, session: SessionId, revision: FormRevision) -> bool {
        self.session == session && self.revision == revision
    }
}

impl fmt::Debug for FormVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormVersion").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared<T> {
    version: FormVersion,
    value: T,
}

impl<T> Prepared<T> {
    pub(crate) fn new(version: FormVersion, value: T) -> Self {
        Self { version, value }
    }

    pub fn version(&self) -> FormVersion {
        self.version
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Prepared<U> {
        Prepared {
            version: self.version,
            value: map(self.value),
        }
    }

    pub fn into_parts(self) -> (FormVersion, T) {
        (self.version, self.value)
    }
}
