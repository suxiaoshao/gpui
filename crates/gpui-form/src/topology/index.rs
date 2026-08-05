use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{FormBuildError, TopologyError};

use super::address::{CanonicalAddress, Incarnation, ItemToken, PathKey, SessionId, TopologyEpoch};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
struct TopologyData {
    next_identity: u64,
    epoch: TopologyEpoch,
    items: HashMap<CanonicalAddress, Vec<ItemToken>>,
    incarnations: HashMap<CanonicalAddress, Incarnation>,
}

pub(crate) struct TopologyEdit {
    session: SessionId,
    data: TopologyData,
}

pub(crate) struct TopologyIndex {
    session: SessionId,
    data: RefCell<TopologyData>,
}

pub(crate) struct TopologySnapshot<'a> {
    pub(crate) index: &'a TopologyIndex,
    pub(crate) epoch: TopologyEpoch,
}

impl TopologyIndex {
    pub(crate) fn new() -> Result<Self, FormBuildError> {
        let session = NEXT_SESSION_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map(SessionId)
            .map_err(|_| FormBuildError::IdentityExhausted)?;
        Ok(Self {
            session,
            data: RefCell::new(TopologyData {
                next_identity: 1,
                ..Default::default()
            }),
        })
    }

    pub(crate) fn session(&self) -> SessionId {
        self.session
    }

    pub(crate) fn snapshot(&self) -> TopologySnapshot<'_> {
        TopologySnapshot {
            index: self,
            epoch: self.data.borrow().epoch,
        }
    }

    pub(crate) fn epoch(&self) -> TopologyEpoch {
        self.data.borrow().epoch
    }

    pub(crate) fn edit(&self) -> TopologyEdit {
        TopologyEdit {
            session: self.session,
            data: self.data.borrow().clone(),
        }
    }

    pub(crate) fn commit(&self, edit: TopologyEdit) {
        debug_assert_eq!(edit.session, self.session);
        *self.data.borrow_mut() = edit.data;
    }

    pub(crate) fn replace_with(&self, edit: TopologyEdit) -> TopologyEdit {
        debug_assert_eq!(edit.session, self.session);
        let previous = std::mem::replace(&mut *self.data.borrow_mut(), edit.data);
        TopologyEdit {
            session: self.session,
            data: previous,
        }
    }

    fn next(data: &mut TopologyData) -> Result<u64, TopologyError> {
        let value = data.next_identity;
        data.next_identity = data
            .next_identity
            .checked_add(1)
            .ok_or(TopologyError::IdentityExhausted)?;
        Ok(value)
    }

    pub(crate) fn ensure_incarnation(
        &self,
        address: &CanonicalAddress,
    ) -> Result<Incarnation, TopologyError> {
        if let Some(value) = self.data.borrow().incarnations.get(address).copied() {
            return Ok(value);
        }
        let mut data = self.data.borrow_mut();
        if let Some(value) = data.incarnations.get(address).copied() {
            return Ok(value);
        }
        let value = Incarnation(Self::next(&mut data)?);
        data.incarnations.insert(address.clone(), value);
        Ok(value)
    }

    pub(crate) fn incarnation(&self, address: &CanonicalAddress) -> Option<Incarnation> {
        self.data.borrow().incarnations.get(address).copied()
    }

    pub(crate) fn ensure_items(
        &self,
        address: &CanonicalAddress,
        len: usize,
    ) -> Result<Vec<ItemToken>, TopologyError> {
        let mut data = self.data.borrow_mut();
        let current_len = data.items.get(address).map_or(0, Vec::len);
        if current_len < len {
            let mut added = Vec::with_capacity(len - current_len);
            for _ in current_len..len {
                added.push(ItemToken(Self::next(&mut data)?));
            }
            data.items.entry(address.clone()).or_default().extend(added);
        } else if current_len > len {
            let removed = data
                .items
                .get_mut(address)
                .expect("item sequence exists")
                .split_off(len);
            for token in removed {
                retire_prefix(&mut data, &address.item(token));
            }
        }
        Ok(data.items.get(address).cloned().unwrap_or_default())
    }

    pub(crate) fn item_index(
        &self,
        collection: &CanonicalAddress,
        token: ItemToken,
    ) -> Option<usize> {
        self.data
            .borrow()
            .items
            .get(collection)
            .and_then(|tokens| tokens.iter().position(|candidate| *candidate == token))
    }

    pub(crate) fn retire_below(&self, prefix: &CanonicalAddress) {
        let mut data = self.data.borrow_mut();
        data.items
            .retain(|address, _| address == prefix || !prefix.is_prefix_of(address));
        data.incarnations
            .retain(|address, _| address == prefix || !prefix.is_prefix_of(address));
    }

    pub(crate) fn reset(&self) {
        let mut data = self.data.borrow_mut();
        data.items.clear();
        data.incarnations.clear();
        data.epoch.0 = data
            .epoch
            .0
            .checked_add(1)
            .expect("form topology epoch overflow");
    }
}

impl TopologyEdit {
    pub(crate) fn ensure_items(
        &mut self,
        address: &CanonicalAddress,
        len: usize,
    ) -> Result<Vec<ItemToken>, TopologyError> {
        let current_len = self.data.items.get(address).map_or(0, Vec::len);
        if current_len < len {
            let mut added = Vec::with_capacity(len - current_len);
            for _ in current_len..len {
                added.push(ItemToken(TopologyIndex::next(&mut self.data)?));
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
            for token in removed {
                retire_prefix(&mut self.data, &address.item(token));
            }
        }
        Ok(self.data.items.get(address).cloned().unwrap_or_default())
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
                path: PathKey::total(self.session, collection),
            });
        }
        let token = ItemToken(TopologyIndex::next(&mut self.data)?);
        self.data
            .items
            .entry(collection.clone())
            .or_default()
            .insert(index, token);
        Ok(token)
    }

    pub(crate) fn remove_item(
        &mut self,
        collection: &CanonicalAddress,
        index: usize,
    ) -> Result<ItemToken, TopologyError> {
        let Some(tokens) = self.data.items.get_mut(collection) else {
            return Err(TopologyError::WrongCollection {
                path: PathKey::total(self.session, collection),
            });
        };
        if index >= tokens.len() {
            return Err(TopologyError::InvalidAnchor {
                path: PathKey::total(self.session, collection),
            });
        }
        let token = tokens.remove(index);
        retire_prefix(&mut self.data, &collection.item(token));
        Ok(token)
    }

    pub(crate) fn move_item(
        &mut self,
        collection: &CanonicalAddress,
        source: usize,
        target: usize,
    ) -> Result<(), TopologyError> {
        let Some(tokens) = self.data.items.get_mut(collection) else {
            return Err(TopologyError::WrongCollection {
                path: PathKey::total(self.session, collection),
            });
        };
        if source >= tokens.len() || target > tokens.len() {
            return Err(TopologyError::InvalidAnchor {
                path: PathKey::total(self.session, collection),
            });
        }
        if source == target || source + 1 == target {
            return Ok(());
        }
        let token = tokens.remove(source);
        let adjusted = if source < target { target - 1 } else { target };
        tokens.insert(adjusted, token);
        Ok(())
    }

    pub(crate) fn retire_descendants(&mut self, prefix: &CanonicalAddress) {
        retire_prefix(&mut self.data, prefix);
    }
}

fn retire_prefix(data: &mut TopologyData, prefix: &CanonicalAddress) {
    data.items
        .retain(|address, _| !prefix.is_prefix_of(address));
    data.incarnations
        .retain(|address, _| !prefix.is_prefix_of(address));
}

impl TopologySnapshot<'_> {
    pub(crate) fn assert_current(&self) {
        debug_assert_eq!(
            self.index.epoch(),
            self.epoch,
            "a form operation must use one current topology snapshot"
        );
    }
}

pub(crate) fn root_address() -> CanonicalAddress {
    CanonicalAddress::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_staged_identity_allocation_does_not_mutate_live_topology() {
        let topology = TopologyIndex::new().unwrap();
        let collection = root_address().field("rows");
        let existing = topology.ensure_items(&collection, 1).unwrap();
        let epoch = topology.epoch();

        let mut edit = topology.edit();
        edit.data.next_identity = u64::MAX;
        assert_eq!(
            edit.insert_item(&collection, 1),
            Err(TopologyError::IdentityExhausted)
        );

        assert_eq!(topology.epoch(), epoch);
        assert_eq!(topology.ensure_items(&collection, 1).unwrap(), existing);
        assert_eq!(topology.item_index(&collection, existing[0]), Some(0));
    }
}
