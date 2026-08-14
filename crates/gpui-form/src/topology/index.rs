use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::TopologyError;

use super::address::{
    CanonicalAddress, CaseId, Incarnation, ItemToken, OccurrenceId, OpaquePathId, PathIdentity,
    PathKey, SessionId,
};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
struct TopologyData {
    next_occurrence: u64,
    next_path_id: u64,
    items: HashMap<CanonicalAddress, Vec<ItemToken>>,
    active_cases: HashMap<CanonicalAddress, (CaseId, OccurrenceId)>,
    active_optionals: HashMap<CanonicalAddress, OccurrenceId>,
    identities: HashMap<CanonicalAddress, Arc<PathIdentity>>,
}

pub(crate) struct TopologyEdit {
    session: SessionId,
    data: TopologyData,
}

pub(crate) struct TopologyIndex {
    session: SessionId,
    data: RefCell<Arc<TopologyData>>,
}

#[derive(Clone)]
pub(crate) struct TopologySnapshot {
    session: SessionId,
    data: Arc<TopologyData>,
}

impl TopologyIndex {
    /// Session allocation is an internal invariant. Public Form construction
    /// never exposes a recoverable identity-build error.
    pub(crate) fn new() -> Self {
        let session = NEXT_SESSION_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map(SessionId)
            .expect("form session identity space exhausted");
        Self {
            session,
            data: RefCell::new(Arc::new(TopologyData {
                next_occurrence: 1,
                next_path_id: 1,
                ..Default::default()
            })),
        }
    }

    pub(crate) fn session(&self) -> SessionId {
        self.session
    }

    pub(crate) fn snapshot(&self) -> TopologySnapshot {
        TopologySnapshot {
            session: self.session,
            data: self.data.borrow().clone(),
        }
    }

    pub(crate) fn stage(&self) -> TopologyEdit {
        TopologyEdit {
            session: self.session,
            data: self.data.borrow().as_ref().clone(),
        }
    }

    pub(crate) fn edit(&self) -> TopologyEdit {
        self.stage()
    }

    pub(crate) fn commit(&self, edit: TopologyEdit) {
        debug_assert_eq!(edit.session, self.session);
        *self.data.borrow_mut() = Arc::new(edit.data);
    }

    /// Read-only identity lookup used by keys, resolvers, snapshots and impact
    /// checks. It deliberately never creates an identity.
    pub(crate) fn key(&self, address: &CanonicalAddress) -> Option<PathKey> {
        self.data
            .borrow()
            .identities
            .get(address)
            .cloned()
            .map(PathKey::from_identity)
    }

    pub(crate) fn incarnation(&self, address: &CanonicalAddress) -> Option<Incarnation> {
        self.data
            .borrow()
            .identities
            .contains_key(address)
            .then(|| address.final_occurrence())
            .flatten()
    }

    pub(crate) fn items(&self, address: &CanonicalAddress) -> Option<Vec<ItemToken>> {
        self.data.borrow().items.get(address).cloned()
    }

    pub(crate) fn dynamic_addresses(&self) -> Vec<CanonicalAddress> {
        self.data
            .borrow()
            .identities
            .keys()
            .filter(|address| address.has_dynamic_occurrence())
            .cloned()
            .collect()
    }

    pub(crate) fn reset(&self) {
        let mut data = self.data.borrow_mut();
        let data = Arc::make_mut(&mut data);
        data.items.clear();
        data.active_cases.clear();
        data.active_optionals.clear();
        data.identities
            .retain(|address, _| !address.has_dynamic_occurrence());
    }
}

impl TopologyEdit {
    pub(crate) fn snapshot(&self) -> TopologySnapshot {
        TopologySnapshot {
            session: self.session,
            data: Arc::new(self.data.clone()),
        }
    }

    fn next_occurrence(&mut self) -> OccurrenceId {
        let value = self.data.next_occurrence;
        self.data.next_occurrence = value
            .checked_add(1)
            .expect("form occurrence identity space exhausted");
        OccurrenceId(value)
    }

    fn intern(&mut self, address: &CanonicalAddress) -> Arc<PathIdentity> {
        if let Some(identity) = self.data.identities.get(address) {
            return identity.clone();
        }
        let id = self.data.next_path_id;
        self.data.next_path_id = id
            .checked_add(1)
            .expect("form path identity space exhausted");
        let identity = Arc::new(PathIdentity {
            session: self.session,
            id: OpaquePathId(id),
            address: address.clone(),
        });
        self.data
            .identities
            .insert(address.clone(), identity.clone());
        identity
    }

    pub(crate) fn materialize_total(&mut self, address: &CanonicalAddress) -> PathKey {
        PathKey::from_identity(self.intern(address))
    }

    pub(crate) fn ensure_items(
        &mut self,
        address: &CanonicalAddress,
        len: usize,
    ) -> Vec<ItemToken> {
        self.intern(address);
        let current_len = self.data.items.entry(address.clone()).or_default().len();
        if current_len < len {
            let mut added = Vec::with_capacity(len - current_len);
            for _ in current_len..len {
                let occurrence = self.next_occurrence();
                self.intern(&address.item(occurrence));
                added.push(occurrence);
            }
            self.data
                .items
                .entry(address.clone())
                .or_default()
                .extend(added);
        } else if current_len > len {
            let removed = self
                .data
                .items
                .get_mut(address)
                .expect("item sequence exists")
                .split_off(len);
            for occurrence in removed {
                retire_prefix(&mut self.data, &address.item(occurrence));
            }
        }
        self.data.items.get(address).cloned().unwrap_or_default()
    }

    pub(crate) fn activate_case(
        &mut self,
        parent: &CanonicalAddress,
        name: &'static str,
    ) -> CanonicalAddress {
        if let Some((case, occurrence)) = self.data.active_cases.get(parent)
            && case.0 == name
        {
            return parent.case_occurrence(name, *occurrence);
        }
        self.retire_descendants(parent);
        let occurrence = self.next_occurrence();
        let address = parent.case_occurrence(name, occurrence);
        self.intern(&address);
        self.data
            .active_cases
            .insert(parent.clone(), (CaseId(name), occurrence));
        address
    }

    pub(crate) fn activate_some(&mut self, parent: &CanonicalAddress) -> CanonicalAddress {
        if let Some(occurrence) = self.data.active_optionals.get(parent) {
            return parent.some_occurrence(*occurrence);
        }
        let occurrence = self.next_occurrence();
        let address = parent.some_occurrence(occurrence);
        self.intern(&address);
        self.data
            .active_optionals
            .insert(parent.clone(), occurrence);
        address
    }

    pub(crate) fn deactivate_some(&mut self, parent: &CanonicalAddress) {
        self.data.active_optionals.remove(parent);
        self.retire_descendants(parent);
    }

    pub(crate) fn item_index(
        &self,
        collection: &CanonicalAddress,
        token: ItemToken,
    ) -> Option<usize> {
        self.data
            .items
            .get(collection)
            .and_then(|tokens| tokens.iter().position(|candidate| *candidate == token))
    }

    pub(crate) fn insert_item(
        &mut self,
        collection: &CanonicalAddress,
        index: usize,
    ) -> Result<ItemToken, TopologyError> {
        let len = self.data.items.get(collection).map_or(0, Vec::len);
        if index > len {
            return Err(TopologyError::InvalidAnchor {
                path: self.materialize_total(collection),
            });
        }
        let occurrence = self.next_occurrence();
        self.intern(&collection.item(occurrence));
        self.data
            .items
            .entry(collection.clone())
            .or_default()
            .insert(index, occurrence);
        Ok(occurrence)
    }

    pub(crate) fn remove_item(
        &mut self,
        collection: &CanonicalAddress,
        index: usize,
    ) -> Result<ItemToken, TopologyError> {
        let Some(tokens) = self.data.items.get_mut(collection) else {
            return Err(TopologyError::WrongCollection {
                path: self.materialize_total(collection),
            });
        };
        if index >= tokens.len() {
            return Err(TopologyError::InvalidAnchor {
                path: self.materialize_total(collection),
            });
        }
        let occurrence = tokens.remove(index);
        retire_prefix(&mut self.data, &collection.item(occurrence));
        Ok(occurrence)
    }

    pub(crate) fn move_item(
        &mut self,
        collection: &CanonicalAddress,
        source: usize,
        target: usize,
    ) -> Result<(), TopologyError> {
        let Some(tokens) = self.data.items.get_mut(collection) else {
            return Err(TopologyError::WrongCollection {
                path: self.materialize_total(collection),
            });
        };
        if source >= tokens.len() || target > tokens.len() {
            return Err(TopologyError::InvalidAnchor {
                path: self.materialize_total(collection),
            });
        }
        if source == target || source + 1 == target {
            return Ok(());
        }
        let occurrence = tokens.remove(source);
        let adjusted = if source < target { target - 1 } else { target };
        tokens.insert(adjusted, occurrence);
        Ok(())
    }

    pub(crate) fn retire_descendants(&mut self, prefix: &CanonicalAddress) {
        retire_descendants(&mut self.data, prefix);
    }
}

fn retire_prefix(data: &mut TopologyData, prefix: &CanonicalAddress) {
    data.items
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.active_cases
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.active_optionals
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.identities
        .retain(|address, _| !prefix.is_prefix_of(address));
}

fn retire_descendants(data: &mut TopologyData, prefix: &CanonicalAddress) {
    data.items
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.active_cases
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.active_optionals
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.identities
        .retain(|address, _| address == prefix || !prefix.is_prefix_of(address));
}

impl TopologySnapshot {
    pub(crate) fn session(&self) -> SessionId {
        self.session
    }

    pub(crate) fn key(&self, address: &CanonicalAddress) -> Option<PathKey> {
        self.data
            .identities
            .get(address)
            .cloned()
            .map(PathKey::from_identity)
    }

    pub(crate) fn items(&self, address: &CanonicalAddress) -> Option<Vec<ItemToken>> {
        self.data.items.get(address).cloned()
    }

    pub(crate) fn item_index(
        &self,
        collection: &CanonicalAddress,
        token: ItemToken,
    ) -> Option<usize> {
        self.data
            .items
            .get(collection)
            .and_then(|tokens| tokens.iter().position(|candidate| *candidate == token))
    }

    pub(crate) fn incarnation(&self, address: &CanonicalAddress) -> Option<Incarnation> {
        self.data
            .identities
            .contains_key(address)
            .then(|| address.final_occurrence())
            .flatten()
    }

    pub(crate) fn active_case(
        &self,
        parent: &CanonicalAddress,
        name: &'static str,
    ) -> Option<OccurrenceId> {
        self.data
            .active_cases
            .get(parent)
            .and_then(|(case, occurrence)| (case.0 == name).then_some(*occurrence))
    }

    pub(crate) fn active_some(&self, parent: &CanonicalAddress) -> Option<OccurrenceId> {
        self.data.active_optionals.get(parent).copied()
    }
}

pub(crate) fn root_address() -> CanonicalAddress {
    CanonicalAddress::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_identity_allocation_does_not_mutate_live_topology() {
        let topology = TopologyIndex::new();
        let collection = root_address().field("rows");
        let mut edit = topology.stage();
        edit.data.next_occurrence = u64::MAX;
        let exhausted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = edit.insert_item(&collection, 0);
        }));
        assert!(exhausted.is_err());
        assert!(topology.items(&collection).is_none());
        assert!(topology.key(&collection).is_none());
    }

    #[test]
    fn same_parent_move_keeps_item_identity() {
        let topology = TopologyIndex::new();
        let collection = root_address().field("rows");
        let mut edit = topology.stage();
        edit.ensure_items(&collection, 2);
        let items = edit.data.items[&collection].clone();
        edit.move_item(&collection, 0, 2).unwrap();
        topology.commit(edit);
        assert_eq!(
            topology.items(&collection).unwrap(),
            vec![items[1], items[0]]
        );
        assert!(topology.key(&collection.item(items[0])).is_some());
    }

    #[test]
    fn case_and_optional_reactivation_get_fresh_identities() {
        let topology = TopologyIndex::new();
        let kind = root_address().field("kind");
        let optional = root_address().field("optional");
        let mut edit = topology.stage();
        edit.materialize_total(&kind);
        edit.materialize_total(&optional);

        let first_case = edit.activate_case(&kind, "alpha");
        let first_case_key = PathKey::from_identity(edit.data.identities[&first_case].clone());
        edit.activate_case(&kind, "beta");
        let second_case = edit.activate_case(&kind, "alpha");
        let second_case_key = PathKey::from_identity(edit.data.identities[&second_case].clone());

        let first_some = edit.activate_some(&optional);
        let first_some_key = PathKey::from_identity(edit.data.identities[&first_some].clone());
        edit.deactivate_some(&optional);
        let second_some = edit.activate_some(&optional);
        let second_some_key = PathKey::from_identity(edit.data.identities[&second_some].clone());

        assert_ne!(first_case, second_case);
        assert_ne!(first_case_key, second_case_key);
        assert_ne!(first_some, second_some);
        assert_ne!(first_some_key, second_some_key);
    }

    #[test]
    fn snapshot_lookups_do_not_allocate_identities() {
        let topology = TopologyIndex::new();
        let collection = root_address().field("rows");
        let mut edit = topology.stage();
        edit.ensure_items(&collection, 1);
        topology.commit(edit);
        let before = {
            let data = topology.data.borrow();
            (
                data.next_occurrence,
                data.next_path_id,
                data.identities.len(),
            )
        };

        let snapshot = topology.snapshot();
        let items = snapshot.items(&collection).unwrap();
        let item = collection.item(items[0]);
        assert!(snapshot.key(&collection).is_some());
        assert!(snapshot.key(&item).is_some());
        assert_eq!(snapshot.items(&collection), Some(items));

        let after = {
            let data = topology.data.borrow();
            (
                data.next_occurrence,
                data.next_path_id,
                data.identities.len(),
            )
        };
        assert_eq!(before, after);
    }
}
