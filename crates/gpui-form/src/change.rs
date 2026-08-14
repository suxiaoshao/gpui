use std::{fmt, marker::PhantomData};

use crate::{
    ChildDef, DynamicItemsPath, DynamicPath, FieldDef, FormRevision, FormSchema, ItemPath,
    ItemsDef, PathKey, RootDef, TotalItemsPath, TotalPath,
    topology::{CanonicalAddress, SessionId, root_address},
};

mod sealed {
    use crate::{FormSchema, change::ChangeTargetInfo};

    pub trait Sealed<M: FormSchema> {
        fn __change_target(&self) -> ChangeTargetInfo;
    }
}

/// A typed schema location that can be compared with a [`ModelChange`].
///
/// This trait is sealed. Form schema descriptors and paths implement it; applications
/// cannot provide their own address interpretation.
pub trait ChangeTarget<M: FormSchema>: sealed::Sealed<M> {}

impl<M, T> ChangeTarget<M> for T
where
    M: FormSchema,
    T: sealed::Sealed<M>,
{
}

#[doc(hidden)]
#[derive(Clone)]
pub struct ChangeTargetInfo {
    pub(crate) session: Option<SessionId>,
    pub(crate) address: CanonicalAddress,
    pub(crate) dynamic: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelChangeKind {
    Edit,
    Replace,
    Reset,
    Rebase,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathImpact {
    value_changed: bool,
    structure_changed: bool,
    retired: bool,
}

impl PathImpact {
    pub const fn value_changed(self) -> bool {
        self.value_changed
    }

    pub const fn structure_changed(self) -> bool {
        self.structure_changed
    }

    pub const fn retired(self) -> bool {
        self.retired
    }

    pub const fn is_affected(self) -> bool {
        self.value_changed || self.structure_changed || self.retired
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ValueScope {
    SubtreeReplaced(CanonicalAddress),
    AggregateChanged(CanonicalAddress),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ValueImpact {
    Scopes(Vec<ValueScope>),
    All,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StructuralImpact {
    None,
    Roots(Vec<CanonicalAddress>),
    All,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RetiredImpact {
    None,
    Roots(Vec<CanonicalAddress>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangeSet {
    pub(crate) value: ValueImpact,
    pub(crate) structure: StructuralImpact,
    pub(crate) retired: RetiredImpact,
}

impl ChangeSet {
    pub(crate) fn subtree(
        address: CanonicalAddress,
        structure_changed: bool,
        retired: Vec<CanonicalAddress>,
    ) -> Self {
        Self {
            value: ValueImpact::Scopes(vec![ValueScope::SubtreeReplaced(address.clone())]),
            structure: if structure_changed {
                StructuralImpact::Roots(vec![address])
            } else {
                StructuralImpact::None
            },
            retired: if retired.is_empty() {
                RetiredImpact::None
            } else {
                RetiredImpact::Roots(compress_roots(retired))
            },
        }
    }

    pub(crate) fn aggregate(
        addresses: Vec<CanonicalAddress>,
        retired: Vec<CanonicalAddress>,
    ) -> Self {
        let addresses = deduplicate(addresses);
        let value = ValueImpact::Scopes(
            addresses
                .iter()
                .cloned()
                .map(ValueScope::AggregateChanged)
                .collect(),
        );
        let structure = StructuralImpact::Roots(addresses);
        let retired = if retired.is_empty() {
            RetiredImpact::None
        } else {
            RetiredImpact::Roots(compress_roots(retired))
        };
        Self {
            value,
            structure,
            retired,
        }
    }

    pub(crate) fn whole_model(retired: Vec<CanonicalAddress>) -> Self {
        Self {
            value: ValueImpact::All,
            structure: StructuralImpact::All,
            retired: if retired.is_empty() {
                RetiredImpact::None
            } else {
                RetiredImpact::Roots(compress_roots(retired))
            },
        }
    }

    fn impact(&self, target: &ChangeTargetInfo) -> PathImpact {
        let value_changed = match &self.value {
            ValueImpact::All => true,
            ValueImpact::Scopes(scopes) => scopes.iter().any(|scope| match scope {
                ValueScope::SubtreeReplaced(root) => intersects(root, &target.address),
                ValueScope::AggregateChanged(root) => target.address.is_prefix_of(root),
            }),
        };
        let structure_changed = match &self.structure {
            StructuralImpact::None => false,
            StructuralImpact::All => true,
            StructuralImpact::Roots(roots) => {
                roots.iter().any(|root| target.address.is_prefix_of(root))
            }
        };
        let retired = target.dynamic
            && match &self.retired {
                RetiredImpact::None => false,
                RetiredImpact::Roots(roots) => {
                    roots.iter().any(|root| root.is_prefix_of(&target.address))
                }
            };

        PathImpact {
            value_changed,
            structure_changed,
            retired,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ControlOrigin {
    pub(crate) control_id: u64,
    pub(crate) lifecycle_generation: u64,
    pub(crate) editor_sequence: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ModelChange<M: FormSchema> {
    revision: FormRevision,
    kind: ModelChangeKind,
    session: SessionId,
    changes: ChangeSet,
    pub(crate) origin: Option<ControlOrigin>,
    marker: PhantomData<fn() -> M>,
}

impl<M: FormSchema> fmt::Debug for ModelChange<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelChange")
            .field("revision", &self.revision)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl<M: FormSchema> ModelChange<M> {
    pub(crate) fn new(
        revision: FormRevision,
        kind: ModelChangeKind,
        session: SessionId,
        changes: ChangeSet,
        origin: Option<ControlOrigin>,
    ) -> Self {
        Self {
            revision,
            kind,
            session,
            changes,
            origin,
            marker: PhantomData,
        }
    }

    pub const fn revision(&self) -> FormRevision {
        self.revision
    }

    pub const fn kind(&self) -> ModelChangeKind {
        self.kind
    }

    pub fn impact(&self, target: &impl ChangeTarget<M>) -> PathImpact {
        let target = sealed::Sealed::__change_target(target);
        if target
            .session
            .is_some_and(|session| session != self.session)
        {
            return PathImpact::default();
        }
        self.changes.impact(&target)
    }

    pub fn affects(&self, target: &impl ChangeTarget<M>) -> bool {
        self.impact(target).is_affected()
    }

    pub(crate) fn impact_info(&self, target: &ChangeTargetInfo) -> PathImpact {
        if target
            .session
            .is_some_and(|session| session != self.session)
        {
            return PathImpact::default();
        }
        self.changes.impact(target)
    }

    pub(crate) const fn origin(&self) -> Option<ControlOrigin> {
        self.origin
    }
}

fn intersects(left: &CanonicalAddress, right: &CanonicalAddress) -> bool {
    left.is_prefix_of(right) || right.is_prefix_of(left)
}

fn compress_roots(mut roots: Vec<CanonicalAddress>) -> Vec<CanonicalAddress> {
    let mut compressed = Vec::new();
    while let Some(candidate) = roots.pop() {
        if compressed
            .iter()
            .any(|root: &CanonicalAddress| root.is_prefix_of(&candidate))
        {
            continue;
        }
        compressed.retain(|root| !candidate.is_prefix_of(root));
        compressed.push(candidate);
    }
    compressed
}

fn deduplicate(mut addresses: Vec<CanonicalAddress>) -> Vec<CanonicalAddress> {
    let mut deduplicated = Vec::new();
    while let Some(address) = addresses.pop() {
        if !deduplicated.contains(&address) {
            deduplicated.push(address);
        }
    }
    deduplicated
}

impl<M: FormSchema> sealed::Sealed<M> for RootDef<M> {
    fn __change_target(&self) -> ChangeTargetInfo {
        ChangeTargetInfo {
            session: None,
            address: root_address(),
            dynamic: false,
        }
    }
}

macro_rules! impl_root_definition_target {
    ($definition:ident) => {
        impl<M: FormSchema, T: 'static> sealed::Sealed<M> for $definition<M, T> {
            fn __change_target(&self) -> ChangeTargetInfo {
                ChangeTargetInfo {
                    session: None,
                    address: root_address().field(self.name()),
                    dynamic: false,
                }
            }
        }
    };
}

impl_root_definition_target!(FieldDef);
impl_root_definition_target!(ChildDef);
impl_root_definition_target!(ItemsDef);

macro_rules! impl_path_target {
    ($path:ident, $dynamic:literal) => {
        impl<M: FormSchema, T: 'static> sealed::Sealed<M> for $path<M, T> {
            fn __change_target(&self) -> ChangeTargetInfo {
                ChangeTargetInfo {
                    session: self.core.change_session(),
                    address: self.core.change_address().clone(),
                    dynamic: $dynamic,
                }
            }
        }
    };
}

impl_path_target!(TotalPath, false);
impl_path_target!(DynamicPath, true);

impl<M: FormSchema, T: FormSchema> sealed::Sealed<M> for TotalItemsPath<M, T> {
    fn __change_target(&self) -> ChangeTargetInfo {
        ChangeTargetInfo {
            session: self.core.change_session(),
            address: self.core.change_address().clone(),
            dynamic: false,
        }
    }
}

impl<M: FormSchema, T: FormSchema> sealed::Sealed<M> for DynamicItemsPath<M, T> {
    fn __change_target(&self) -> ChangeTargetInfo {
        ChangeTargetInfo {
            session: self.core.change_session(),
            address: self.core.change_address().clone(),
            dynamic: true,
        }
    }
}

impl<M: FormSchema, T: FormSchema> sealed::Sealed<M> for ItemPath<M, T> {
    fn __change_target(&self) -> ChangeTargetInfo {
        let (address, session, _) = self.change_target_info();
        ChangeTargetInfo {
            session: Some(session),
            address: address.clone(),
            dynamic: true,
        }
    }
}

impl<M: FormSchema> sealed::Sealed<M> for PathKey {
    fn __change_target(&self) -> ChangeTargetInfo {
        ChangeTargetInfo {
            session: Some(self.session()),
            address: self.address().clone(),
            dynamic: self.address().has_dynamic_occurrence(),
        }
    }
}
