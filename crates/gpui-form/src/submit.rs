use crate::FormRevision;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared<T> {
    revision: FormRevision,
    value: T,
}

impl<T> Prepared<T> {
    pub(crate) fn new(revision: FormRevision, value: T) -> Self {
        Self { revision, value }
    }

    pub fn revision(&self) -> FormRevision {
        self.revision
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Prepared<U> {
        Prepared {
            revision: self.revision,
            value: map(self.value),
        }
    }

    pub fn into_parts(self) -> (FormRevision, T) {
        (self.revision, self.value)
    }
}
