use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionId(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ItemToken(pub(super) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Incarnation(pub(super) u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct TopologyEpoch(pub(super) u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum AddressSegment {
    Field(&'static str),
    Item(ItemToken),
    Case(&'static str),
    Some,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct CanonicalAddress(Vec<AddressSegment>);

impl CanonicalAddress {
    pub(crate) fn field(&self, name: &'static str) -> Self {
        self.with(AddressSegment::Field(name))
    }
    pub(crate) fn item(&self, token: ItemToken) -> Self {
        self.with(AddressSegment::Item(token))
    }
    pub(crate) fn case(&self, name: &'static str) -> Self {
        self.with(AddressSegment::Case(name))
    }
    pub(crate) fn some(&self) -> Self {
        self.with(AddressSegment::Some)
    }
    fn with(&self, segment: AddressSegment) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment);
        Self(segments)
    }
    pub(crate) fn is_prefix_of(&self, other: &Self) -> bool {
        other.0.starts_with(&self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DynamicGuard {
    pub(crate) address: CanonicalAddress,
    pub(crate) incarnation: Incarnation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathKey {
    session: u64,
    address_hash: u64,
    incarnation: u64,
}

impl Hash for PathKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.session.hash(state);
        self.address_hash.hash(state);
        self.incarnation.hash(state);
    }
}

impl PathKey {
    pub(crate) fn new(
        session: SessionId,
        address: &CanonicalAddress,
        incarnation: Incarnation,
    ) -> Self {
        Self {
            session: session.0,
            address_hash: address_hash(address),
            incarnation: incarnation.0,
        }
    }
    pub(crate) fn total(session: SessionId, address: &CanonicalAddress) -> Self {
        Self::new(session, address, Incarnation(0))
    }
    pub(crate) fn matches(
        &self,
        session: SessionId,
        address: &CanonicalAddress,
        incarnation: Incarnation,
    ) -> bool {
        self == &Self::new(session, address, incarnation)
    }
}

impl From<PathKey> for gpui::ElementId {
    fn from(value: PathKey) -> Self {
        Self::Name(
            format!(
                "form-path-{:016x}-{:016x}-{:016x}",
                value.session, value.address_hash, value.incarnation
            )
            .into(),
        )
    }
}

impl From<&PathKey> for gpui::ElementId {
    fn from(value: &PathKey) -> Self {
        value.clone().into()
    }
}

fn address_hash(address: &CanonicalAddress) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    address.hash(&mut hasher);
    hasher.finish()
}
