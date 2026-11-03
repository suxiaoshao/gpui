use crate::{FormVersion, PathKey, topology::CanonicalAddress};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftPolicy {
    Preserve,
    Discard,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    WouldRetireDrafts(Vec<PathKey>),
    EditsPending,
    InvalidTarget,
}
impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "form lifecycle rejected: {self:?}")
    }
}
impl std::error::Error for LifecycleError {}
#[derive(Clone, Debug)]
pub struct LifecycleReport {
    pub(crate) version: FormVersion,
    pub(crate) conflicts: Vec<PathKey>,
}
impl LifecycleReport {
    pub fn version(&self) -> FormVersion {
        self.version
    }
    pub fn conflicts(&self) -> &[PathKey] {
        &self.conflicts
    }
}

/// Typed derive traversal state. Handwritten schemas must visit every merge unit.
#[doc(hidden)]
pub struct MergePlan {
    address: CanonicalAddress,
    drafts: Vec<CanonicalAddress>,
    pub(crate) conflicts: Vec<CanonicalAddress>,
    pub(crate) external: Vec<CanonicalAddress>,
    pub(crate) baseline: Vec<(CanonicalAddress, BaselineChoice)>,
}
#[derive(Clone, Copy)]
pub(crate) enum BaselineChoice {
    Current,
    Previous,
    Unmapped,
}
impl MergePlan {
    pub(crate) fn new(drafts: Vec<CanonicalAddress>) -> Self {
        Self {
            address: CanonicalAddress::default(),
            drafts,
            conflicts: Vec::new(),
            external: Vec::new(),
            baseline: Vec::new(),
        }
    }
    pub fn child<T>(&mut self, name: &'static str, f: impl FnOnce(&mut Self) -> T) -> T {
        let old = self.address.clone();
        self.address = self.address.field(name);
        let value = f(self);
        self.address = old;
        value
    }
    pub fn atomic<T: Clone + PartialEq>(&mut self, base: &T, local: &T, remote: &T) -> T {
        let draft = self
            .drafts
            .iter()
            .any(|d| crate::validation::intersects(d, &self.address));
        let keep_local = draft || (local != base && local != remote);
        if keep_local {
            if remote != base && (draft || local != remote) {
                self.conflicts.push(self.address.clone());
            }
            self.baseline.push((
                self.address.clone(),
                if remote == base {
                    BaselineChoice::Previous
                } else if local == remote {
                    BaselineChoice::Current
                } else {
                    BaselineChoice::Unmapped
                },
            ));
            local.clone()
        } else {
            if local != remote {
                self.external.push(self.address.clone());
            }
            self.baseline
                .push((self.address.clone(), BaselineChoice::Current));
            remote.clone()
        }
    }
}
