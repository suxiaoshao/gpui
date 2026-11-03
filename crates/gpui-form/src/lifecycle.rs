use crate::{FormVersion, PathKey, topology::CanonicalAddress};

/// A domain merge candidate for one schema field during an edit-preserving rebase.
///
/// Declare a pure `fn(&T, &T, &T) -> FieldMerge<T>` with `#[form(merge = path)]`.
/// Its inputs are the baseline, local stored value and remote value. Form installs
/// the candidate atomically with the other fields and retains the remote baseline.
/// A field with unresolved drafts keeps its local value without calling the hook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldMerge<T> {
    value: T,
    conflicted: bool,
}

impl<T> FieldMerge<T> {
    /// Accept a candidate without reporting a domain conflict.
    pub fn merged(value: T) -> Self {
        Self {
            value,
            conflicted: false,
        }
    }

    /// Accept a candidate and report a conflict at this field's path.
    pub fn conflicted(value: T) -> Self {
        Self {
            value,
            conflicted: true,
        }
    }
}

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
        let draft = self.has_draft();
        let keep_local = draft || (local != base && local != remote);
        if keep_local {
            if remote != base && (draft || local != remote) {
                self.record_conflict();
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

    pub fn merge_with<T: Clone + PartialEq>(
        &mut self,
        base: &T,
        local: &T,
        remote: &T,
        merge: fn(&T, &T, &T) -> FieldMerge<T>,
    ) -> T {
        if self.has_draft() {
            return self.atomic(base, local, remote);
        }

        let candidate = merge(base, local, remote);
        if candidate.conflicted {
            self.record_conflict();
        }
        if &candidate.value != local {
            self.external.push(self.address.clone());
        }
        self.baseline.push((
            self.address.clone(),
            if &candidate.value == remote {
                BaselineChoice::Current
            } else if remote == base && &candidate.value == local {
                BaselineChoice::Previous
            } else {
                BaselineChoice::Unmapped
            },
        ));
        candidate.value
    }

    fn has_draft(&self) -> bool {
        self.drafts
            .iter()
            .any(|draft| crate::validation::intersects(draft, &self.address))
    }

    fn record_conflict(&mut self) {
        if !self.conflicts.contains(&self.address) {
            self.conflicts.push(self.address.clone());
        }
    }
}
