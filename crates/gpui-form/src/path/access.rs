use std::{marker::PhantomData, sync::Arc};

use crate::{
    CaseDef, ItemToken, PathKey, ResolveError,
    topology::{CanonicalAddress, TopologySnapshot},
};

pub(crate) trait Access<Root, T>: 'static {
    fn get<'a>(&self, root: &'a Root, topology: &TopologySnapshot) -> Result<&'a T, ResolveError>;
    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a mut T, ResolveError>;
}

pub(super) struct RootAccess<Root>(pub(super) PhantomData<fn() -> Root>);

impl<Root: 'static> Access<Root, Root> for RootAccess<Root> {
    fn get<'a>(
        &self,
        root: &'a Root,
        _topology: &TopologySnapshot,
    ) -> Result<&'a Root, ResolveError> {
        Ok(root)
    }
    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        _topology: &TopologySnapshot,
    ) -> Result<&'a mut Root, ResolveError> {
        Ok(root)
    }
}

pub(super) struct FieldAccess<Root, Owner, T> {
    pub(super) parent: Arc<dyn Access<Root, Owner>>,
    pub(super) read: fn(&Owner) -> &T,
    pub(super) read_mut: fn(&mut Owner) -> &mut T,
}

impl<Root: 'static, Owner: 'static, T: 'static> Access<Root, T> for FieldAccess<Root, Owner, T> {
    fn get<'a>(&self, root: &'a Root, topology: &TopologySnapshot) -> Result<&'a T, ResolveError> {
        Ok((self.read)(self.parent.get(root, topology)?))
    }
    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a mut T, ResolveError> {
        Ok((self.read_mut)(self.parent.get_mut(root, topology)?))
    }
}

pub(super) struct OptionalAccess<Root, T> {
    pub(super) parent: Arc<dyn Access<Root, Option<T>>>,
    pub(super) key: PathKey,
}

impl<Root: 'static, T: 'static> Access<Root, T> for OptionalAccess<Root, T> {
    fn get<'a>(&self, root: &'a Root, topology: &TopologySnapshot) -> Result<&'a T, ResolveError> {
        self.parent
            .get(root, topology)?
            .as_ref()
            .ok_or_else(|| ResolveError::MissingOptional {
                path: self.key.clone(),
            })
    }
    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a mut T, ResolveError> {
        self.parent
            .get_mut(root, topology)?
            .as_mut()
            .ok_or_else(|| ResolveError::MissingOptional {
                path: self.key.clone(),
            })
    }
}

pub(super) struct CaseAccess<Root, Enum, Payload> {
    pub(super) parent: Arc<dyn Access<Root, Enum>>,
    pub(super) case: CaseDef<Enum, Payload>,
    pub(super) key: PathKey,
}

impl<Root: 'static, Enum: 'static, Payload: 'static> Access<Root, Payload>
    for CaseAccess<Root, Enum, Payload>
{
    fn get<'a>(
        &self,
        root: &'a Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a Payload, ResolveError> {
        (self.case.read())(self.parent.get(root, topology)?).ok_or_else(|| {
            ResolveError::InactiveCase {
                path: self.key.clone(),
                expected: self.case.name(),
            }
        })
    }
    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a mut Payload, ResolveError> {
        (self.case.read_mut())(self.parent.get_mut(root, topology)?).ok_or_else(|| {
            ResolveError::InactiveCase {
                path: self.key.clone(),
                expected: self.case.name(),
            }
        })
    }
}

pub(super) struct ItemAccess<Root, Item> {
    pub(super) collection: Arc<dyn Access<Root, Vec<Item>>>,
    pub(super) collection_address: CanonicalAddress,
    pub(super) token: ItemToken,
    pub(super) key: PathKey,
}

impl<Root: 'static, Item: 'static> Access<Root, Item> for ItemAccess<Root, Item> {
    fn get<'a>(
        &self,
        root: &'a Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a Item, ResolveError> {
        let index = topology
            .item_index(&self.collection_address, self.token)
            .ok_or_else(|| ResolveError::MissingItem {
                path: self.key.clone(),
            })?;
        self.collection
            .get(root, topology)?
            .get(index)
            .ok_or_else(|| ResolveError::MissingItem {
                path: self.key.clone(),
            })
    }
    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        topology: &TopologySnapshot,
    ) -> Result<&'a mut Item, ResolveError> {
        let index = topology
            .item_index(&self.collection_address, self.token)
            .ok_or_else(|| ResolveError::MissingItem {
                path: self.key.clone(),
            })?;
        self.collection
            .get_mut(root, topology)?
            .get_mut(index)
            .ok_or_else(|| ResolveError::MissingItem {
                path: self.key.clone(),
            })
    }
}
