use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// A Form-session identity. It is intentionally crate-private: a path from one
/// session must never be usable in another one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionId(pub(crate) u64);

/// Runtime identity for an item, active enum case, or active `Some` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct OccurrenceId(pub(super) u64);

/// Role-specific names for the same runtime-owned occurrence identity. Neither
/// is supplied by the model or UI.
pub(crate) type ItemToken = OccurrenceId;
pub(crate) type Incarnation = OccurrenceId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct OpaquePathId(pub(super) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FieldId(pub(crate) &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CaseId(pub(crate) &'static str);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AddressSegment {
    Field(FieldId),
    Item(OccurrenceId),
    Case(CaseId, OccurrenceId),
    Some(OccurrenceId),
}

/// The actual schema/runtime address. This never crosses the public API
/// boundary; `PathKey` is the public, opaque UI identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct CanonicalAddress(Arc<[AddressSegment]>);

impl CanonicalAddress {
    pub(crate) fn field(&self, name: &'static str) -> Self {
        self.with(AddressSegment::Field(FieldId(name)))
    }

    pub(crate) fn item(&self, occurrence: OccurrenceId) -> Self {
        self.with(AddressSegment::Item(occurrence))
    }

    pub(crate) fn case_occurrence(&self, name: &'static str, occurrence: OccurrenceId) -> Self {
        self.with(AddressSegment::Case(CaseId(name), occurrence))
    }

    pub(crate) fn some_occurrence(&self, occurrence: OccurrenceId) -> Self {
        self.with(AddressSegment::Some(occurrence))
    }

    pub(crate) fn is_prefix_of(&self, other: &Self) -> bool {
        other.0.starts_with(&self.0)
    }

    pub(crate) fn final_occurrence(&self) -> Option<OccurrenceId> {
        match self.0.last() {
            Some(AddressSegment::Item(occurrence))
            | Some(AddressSegment::Case(_, occurrence))
            | Some(AddressSegment::Some(occurrence)) => Some(*occurrence),
            Some(AddressSegment::Field(_)) | None => None,
        }
    }

    pub(crate) fn has_dynamic_occurrence(&self) -> bool {
        self.0.iter().any(|segment| {
            matches!(
                segment,
                AddressSegment::Item(_) | AddressSegment::Case(_, _) | AddressSegment::Some(_)
            )
        })
    }

    fn with(&self, segment: AddressSegment) -> Self {
        let mut segments = Vec::with_capacity(self.0.len() + 1);
        segments.extend(self.0.iter().cloned());
        segments.push(segment);
        Self(segments.into())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DynamicGuard {
    pub(crate) address: CanonicalAddress,
    pub(crate) incarnation: Incarnation,
    pub(crate) key: PathKey,
}

#[derive(Debug)]
pub(crate) struct PathIdentity {
    pub(crate) session: SessionId,
    pub(crate) id: OpaquePathId,
    pub(crate) address: CanonicalAddress,
}

/// Public UI identity backed by a session-local opaque id. The canonical
/// address remains available only inside this crate for impact/topology work.
#[derive(Clone)]
pub struct PathKey(Arc<PathIdentity>);

impl PathKey {
    pub(crate) fn from_identity(identity: Arc<PathIdentity>) -> Self {
        Self(identity)
    }

    pub(crate) fn session(&self) -> SessionId {
        self.0.session
    }

    pub(crate) fn address(&self) -> &CanonicalAddress {
        &self.0.address
    }
}

impl PartialEq for PathKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.session == other.0.session && self.0.id == other.0.id
    }
}

impl Eq for PathKey {}

impl Hash for PathKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.session.hash(state);
        self.0.id.hash(state);
    }
}

impl fmt::Debug for PathKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PathKey")
            .field("session", &self.0.session.0)
            .field("id", &self.0.id.0)
            .finish()
    }
}

impl From<PathKey> for gpui::ElementId {
    fn from(value: PathKey) -> Self {
        Self::Name(format!("form-path-{:016x}-{:016x}", value.0.session.0, value.0.id.0).into())
    }
}

impl From<&PathKey> for gpui::ElementId {
    fn from(value: &PathKey) -> Self {
        value.clone().into()
    }
}
