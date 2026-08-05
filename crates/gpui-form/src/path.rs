mod access;

use std::{marker::PhantomData, sync::Arc};

use gpui::{App, Entity};

use crate::{
    CaseDef, ChildDef, FieldDef, FieldSchema, Form, FormSchema, ItemToken, ItemsDef, MutationError,
    PathKey, ResolveError, RootDef, TopologyError, ValidationIssue, ValidationMessage,
    ValidationPath, ValidationRequest, ValidationSink, ValidationTrigger,
    topology::{CanonicalAddress, DynamicGuard, SessionId, root_address},
};
use access::{Access, CaseAccess, FieldAccess, ItemAccess, OptionalAccess, RootAccess};

pub(crate) struct PathCore<Root, T> {
    pub(crate) access: Arc<dyn Access<Root, T>>,
    pub(crate) address: CanonicalAddress,
    pub(crate) session: Option<SessionId>,
    pub(crate) guards: Vec<DynamicGuard>,
}

impl<Root, T> Clone for PathCore<Root, T> {
    fn clone(&self) -> Self {
        Self {
            access: self.access.clone(),
            address: self.address.clone(),
            session: self.session,
            guards: self.guards.clone(),
        }
    }
}

impl<Root: FormSchema> PathCore<Root, Root> {
    fn root() -> Self {
        Self {
            access: Arc::new(RootAccess(PhantomData)),
            address: root_address(),
            session: None,
            guards: Vec::new(),
        }
    }
}

impl<Root: FormSchema, T: 'static> PathCore<Root, T> {
    fn then<U: 'static>(
        self,
        name: &'static str,
        read: fn(&T) -> &U,
        read_mut: fn(&mut T) -> &mut U,
    ) -> PathCore<Root, U> {
        PathCore {
            access: Arc::new(FieldAccess {
                parent: self.access,
                read,
                read_mut,
            }),
            address: self.address.field(name),
            session: self.session,
            guards: self.guards,
        }
    }

    pub(crate) fn check(&self, form: &Form<Root>) -> Result<(), ResolveError> {
        if let Some(session) = self.session
            && session != form.session()
        {
            return Err(ResolveError::WrongSession {
                path: self.key_for(form),
            });
        }
        for guard in &self.guards {
            if form.topology().incarnation(&guard.address) != Some(guard.incarnation) {
                return Err(ResolveError::Retired {
                    path: PathKey::new(
                        self.session.unwrap_or_else(|| form.session()),
                        &guard.address,
                        guard.incarnation,
                    ),
                });
            }
        }
        Ok(())
    }

    fn key_for(&self, form: &Form<Root>) -> PathKey {
        let session = self.session.unwrap_or_else(|| form.session());
        let incarnation = self
            .guards
            .last()
            .map(|guard| guard.incarnation)
            .unwrap_or_else(|| {
                form.topology()
                    .ensure_incarnation(&self.address)
                    .expect("form identity exhausted after construction")
            });
        PathKey::new(session, &self.address, incarnation)
    }
}

pub struct TotalPath<Root: FormSchema, T: 'static> {
    pub(crate) core: PathCore<Root, T>,
}

impl<Root: FormSchema, T: 'static> Clone for TotalPath<Root, T> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}

pub struct DynamicPath<Root: FormSchema, T: 'static> {
    pub(crate) core: PathCore<Root, T>,
}

impl<Root: FormSchema, T: 'static> Clone for DynamicPath<Root, T> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}

pub struct TotalItemsPath<Root: FormSchema, Item: FormSchema> {
    pub(crate) core: PathCore<Root, Vec<Item>>,
}

impl<Root: FormSchema, Item: FormSchema> Clone for TotalItemsPath<Root, Item> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}

pub struct DynamicItemsPath<Root: FormSchema, Item: FormSchema> {
    pub(crate) core: PathCore<Root, Vec<Item>>,
}

impl<Root: FormSchema, Item: FormSchema> Clone for DynamicItemsPath<Root, Item> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}

pub struct ItemPath<Root: FormSchema, Item: FormSchema> {
    path: DynamicPath<Root, Item>,
    collection: CanonicalAddress,
    collection_access: Arc<dyn Access<Root, Vec<Item>>>,
    token: ItemToken,
}

impl<Root: FormSchema, Item: FormSchema> Clone for ItemPath<Root, Item> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            collection: self.collection.clone(),
            collection_access: self.collection_access.clone(),
            token: self.token,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Start,
    End,
}

mod sealed {
    pub trait Sealed {}
}

#[doc(hidden)]
pub trait PathEdge<Root: FormSchema, Owner: 'static>: sealed::Sealed + Sized {
    type TotalOutput;
    type DynamicOutput;

    fn __from_total(self, parent: TotalPath<Root, Owner>) -> Self::TotalOutput;
    fn __from_dynamic(self, parent: DynamicPath<Root, Owner>) -> Self::DynamicOutput;
}

impl<Owner, T> sealed::Sealed for FieldDef<Owner, T> {}
impl<Owner, T> sealed::Sealed for ChildDef<Owner, T> {}
impl<Owner, T> sealed::Sealed for ItemsDef<Owner, T> {}

impl<Root: FormSchema, Owner: 'static, T: 'static> PathEdge<Root, Owner> for FieldDef<Owner, T> {
    type TotalOutput = TotalPath<Root, T>;
    type DynamicOutput = DynamicPath<Root, T>;

    fn __from_total(self, parent: TotalPath<Root, Owner>) -> Self::TotalOutput {
        TotalPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
        }
    }

    fn __from_dynamic(self, parent: DynamicPath<Root, Owner>) -> Self::DynamicOutput {
        DynamicPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
        }
    }
}

impl<Root: FormSchema, Owner: 'static, T: 'static> PathEdge<Root, Owner> for ChildDef<Owner, T> {
    type TotalOutput = TotalPath<Root, T>;
    type DynamicOutput = DynamicPath<Root, T>;

    fn __from_total(self, parent: TotalPath<Root, Owner>) -> Self::TotalOutput {
        TotalPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
        }
    }

    fn __from_dynamic(self, parent: DynamicPath<Root, Owner>) -> Self::DynamicOutput {
        DynamicPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
        }
    }
}

impl<Root: FormSchema, Owner: 'static, T: FormSchema> PathEdge<Root, Owner> for ItemsDef<Owner, T> {
    type TotalOutput = TotalItemsPath<Root, T>;
    type DynamicOutput = DynamicItemsPath<Root, T>;

    fn __from_total(self, parent: TotalPath<Root, Owner>) -> Self::TotalOutput {
        TotalItemsPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
        }
    }

    fn __from_dynamic(self, parent: DynamicPath<Root, Owner>) -> Self::DynamicOutput {
        DynamicItemsPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
        }
    }
}

impl<Root: FormSchema> RootDef<Root> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::TotalOutput
    where
        Edge: PathEdge<Root, Root>,
    {
        edge.__from_total(TotalPath {
            core: PathCore::root(),
        })
    }
}

impl<Root: FormSchema, T: 'static> TotalPath<Root, T> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::TotalOutput
    where
        Edge: PathEdge<Root, T>,
    {
        edge.__from_total(self)
    }
}

impl<Root: FormSchema, T: 'static> DynamicPath<Root, T> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::DynamicOutput
    where
        Edge: PathEdge<Root, T>,
    {
        edge.__from_dynamic(self)
    }
}

impl<Root: FormSchema, Item: FormSchema> ItemPath<Root, Item> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::DynamicOutput
    where
        Edge: PathEdge<Root, Item>,
    {
        edge.__from_dynamic(self.path)
    }

    pub fn key(&self) -> PathKey {
        let guard = self
            .path
            .core
            .guards
            .last()
            .expect("item paths always have a dynamic guard");
        PathKey::new(
            self.path
                .core
                .session
                .expect("item paths are session bound"),
            &self.path.core.address,
            guard.incarnation,
        )
    }

    pub fn try_value(&self, form: &Entity<Form<Root>>, cx: &App) -> Result<Item, ResolveError> {
        self.path.try_value(form, cx)
    }

    pub fn try_set(
        &self,
        form: &Entity<Form<Root>>,
        value: Item,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        self.path.try_set(form, value, cx)
    }

    pub fn move_to<Destination>(
        self,
        form: &Entity<Form<Root>>,
        destination: Destination,
        position: Position,
        cx: &mut App,
    ) -> Result<ItemPath<Root, Item>, MutationError>
    where
        Destination: IntoItemsPath<Root, Item>,
    {
        move_between(form, self, destination.into_items_path().core, position, cx)
    }
}

#[doc(hidden)]
pub trait IntoItemsPath<Root: FormSchema, Item: FormSchema> {
    fn into_items_path(self) -> DynamicItemsPath<Root, Item>;
}

impl<Root: FormSchema, Item: FormSchema> IntoItemsPath<Root, Item> for TotalItemsPath<Root, Item> {
    fn into_items_path(self) -> DynamicItemsPath<Root, Item> {
        DynamicItemsPath { core: self.core }
    }
}

impl<Root: FormSchema, Item: FormSchema> IntoItemsPath<Root, Item>
    for DynamicItemsPath<Root, Item>
{
    fn into_items_path(self) -> DynamicItemsPath<Root, Item> {
        self
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> TotalPath<Root, T> {
    pub fn key(&self, form: &Form<Root>) -> PathKey {
        self.core.key_for(form)
    }

    pub fn value(&self, form: &Entity<Form<Root>>, cx: &App) -> T {
        let form = form.read(cx);
        self.core
            .access
            .get(form.value(), &form.topology().snapshot())
            .expect("a total path must always resolve")
            .clone()
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) {
        let path = self.clone();
        form.update(cx, move |form, cx| {
            let changed = {
                let (model, topology) = form.model_and_topology();
                let snapshot = topology.snapshot();
                let current = path
                    .core
                    .access
                    .get(model, &snapshot)
                    .expect("a total path must always resolve");
                if current == &value {
                    false
                } else {
                    *path
                        .core
                        .access
                        .get_mut(model, &snapshot)
                        .expect("a total path must always resolve") = value;
                    true
                }
            };
            if !changed {
                return;
            }
            form.commit_value(path.core.address.clone(), cx);
        });
    }

    pub fn validate(&self, form: &Entity<Form<Root>>, trigger: ValidationTrigger, cx: &mut App) {
        let address = self.core.address.clone();
        form.update(cx, move |form, cx| {
            form.validate_at(trigger, Some(address), cx)
        });
    }

    pub fn errors(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ValidationIssue> {
        form.read(cx).errors_at(&self.core.address)
    }

    pub fn bind_control(
        &self,
        form: &Entity<Form<Root>>,
        cx: &mut App,
    ) -> crate::ControlBinding<Root, T> {
        crate::ControlBinding::total(form, self.clone(), cx)
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> DynamicPath<Root, T> {
    pub fn try_value(&self, form: &Entity<Form<Root>>, cx: &App) -> Result<T, ResolveError> {
        let form = form.read(cx);
        self.core.check(form)?;
        Ok(self
            .core
            .access
            .get(form.value(), &form.topology().snapshot())?
            .clone())
    }

    pub fn try_set(
        &self,
        form: &Entity<Form<Root>>,
        value: T,
        cx: &mut App,
    ) -> Result<(), MutationError> {
        let path = self.clone();
        form.update(cx, move |form, cx| {
            path.core.check(form)?;
            let changed = {
                let (model, topology) = form.model_and_topology();
                let snapshot = topology.snapshot();
                let current = path.core.access.get(model, &snapshot)?;
                if current == &value {
                    false
                } else {
                    *path.core.access.get_mut(model, &snapshot)? = value;
                    true
                }
            };
            if !changed {
                return Ok(());
            }
            form.commit_value(path.core.address.clone(), cx);
            Ok(())
        })
    }

    pub fn try_validate(
        &self,
        form: &Entity<Form<Root>>,
        trigger: ValidationTrigger,
        cx: &mut App,
    ) -> Result<(), ResolveError> {
        let path = self.clone();
        form.update(cx, move |form, cx| {
            path.core.check(form)?;
            form.validate_at(trigger, Some(path.core.address), cx);
            Ok(())
        })
    }

    pub fn try_errors(
        &self,
        form: &Entity<Form<Root>>,
        cx: &App,
    ) -> Result<Vec<ValidationIssue>, ResolveError> {
        let form = form.read(cx);
        self.core.check(form)?;
        Ok(form.errors_at(&self.core.address))
    }

    pub fn try_bind_control(
        &self,
        form: &Entity<Form<Root>>,
        cx: &mut App,
    ) -> Result<crate::ControlBinding<Root, T>, ResolveError> {
        let _ = self.try_value(form, cx)?;
        Ok(crate::ControlBinding::dynamic(form, self.clone(), cx))
    }
}

impl<Root: FormSchema, Enum: FormSchema> TotalPath<Root, Enum> {
    pub fn try_case<Payload: FormSchema>(
        self,
        form: &Form<Root>,
        case: CaseDef<Enum, Payload>,
    ) -> Result<DynamicPath<Root, Payload>, ResolveError> {
        locate_case_in(self.core, form.value(), form.topology(), case)
    }
}

impl<Root: FormSchema, Enum: FormSchema> DynamicPath<Root, Enum> {
    pub fn try_case<Payload: FormSchema>(
        self,
        form: &Form<Root>,
        case: CaseDef<Enum, Payload>,
    ) -> Result<DynamicPath<Root, Payload>, ResolveError> {
        self.core.check(form)?;
        locate_case_in(self.core, form.value(), form.topology(), case)
    }
}

impl<Root: FormSchema, T: FormSchema> TotalPath<Root, Option<T>> {
    pub fn try_some(self, form: &Form<Root>) -> Result<DynamicPath<Root, T>, ResolveError> {
        locate_some_in(self.core, form.value(), form.topology())
    }
}

impl<Root: FormSchema, T: FormSchema> DynamicPath<Root, Option<T>> {
    pub fn try_some(self, form: &Form<Root>) -> Result<DynamicPath<Root, T>, ResolveError> {
        self.core.check(form)?;
        locate_some_in(self.core, form.value(), form.topology())
    }
}

pub(crate) fn locate_case_in<Root: FormSchema, Enum: FormSchema, Payload: FormSchema>(
    core: PathCore<Root, Enum>,
    model: &Root,
    topology: &crate::topology::TopologyIndex,
    case: CaseDef<Enum, Payload>,
) -> Result<DynamicPath<Root, Payload>, ResolveError> {
    let address = core.address.case(case.name());
    let provisional = PathKey::total(topology.session(), &address);
    if (case.read())(core.access.get(model, &topology.snapshot())?).is_none() {
        return Err(ResolveError::InactiveCase {
            path: provisional,
            expected: case.name(),
        });
    }
    let incarnation = topology
        .ensure_incarnation(&address)
        .expect("form identity exhausted after construction");
    let key = PathKey::new(topology.session(), &address, incarnation);
    let mut guards = core.guards;
    guards.push(DynamicGuard {
        address: address.clone(),
        incarnation,
    });
    Ok(DynamicPath {
        core: PathCore {
            access: Arc::new(CaseAccess {
                parent: core.access,
                case,
                key,
            }),
            address,
            session: Some(topology.session()),
            guards,
        },
    })
}

pub(crate) fn locate_some_in<Root: FormSchema, T: FormSchema>(
    core: PathCore<Root, Option<T>>,
    model: &Root,
    topology: &crate::topology::TopologyIndex,
) -> Result<DynamicPath<Root, T>, ResolveError> {
    let address = core.address.some();
    let provisional = PathKey::total(topology.session(), &address);
    if core.access.get(model, &topology.snapshot())?.is_none() {
        return Err(ResolveError::MissingOptional { path: provisional });
    }
    let incarnation = topology
        .ensure_incarnation(&address)
        .expect("form identity exhausted after construction");
    let key = PathKey::new(topology.session(), &address, incarnation);
    let mut guards = core.guards;
    guards.push(DynamicGuard {
        address: address.clone(),
        incarnation,
    });
    Ok(DynamicPath {
        core: PathCore {
            access: Arc::new(OptionalAccess {
                parent: core.access,
                key,
            }),
            address,
            session: Some(topology.session()),
            guards,
        },
    })
}

pub(crate) fn item_paths_in<Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    model: &Root,
    topology: &crate::topology::TopologyIndex,
) -> Result<Vec<ItemPath<Root, Item>>, MutationError> {
    if core
        .session
        .is_some_and(|session| session != topology.session())
    {
        return Err(ResolveError::WrongSession {
            path: PathKey::total(topology.session(), &core.address),
        }
        .into());
    }
    for guard in &core.guards {
        if topology.incarnation(&guard.address) != Some(guard.incarnation) {
            return Err(ResolveError::Retired {
                path: PathKey::new(
                    core.session.unwrap_or_else(|| topology.session()),
                    &guard.address,
                    guard.incarnation,
                ),
            }
            .into());
        }
    }
    let snapshot = topology.snapshot();
    let len = core.access.get(model, &snapshot)?.len();
    let tokens = topology.ensure_items(&core.address, len)?;
    Ok(tokens
        .into_iter()
        .map(|token| make_item_path_in(core, topology, token))
        .collect())
}

pub(crate) fn make_item_path_in<Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    topology: &crate::topology::TopologyIndex,
    token: ItemToken,
) -> ItemPath<Root, Item> {
    let address = core.address.item(token);
    let incarnation = topology
        .ensure_incarnation(&address)
        .expect("form identity exhausted after construction");
    let key = PathKey::new(topology.session(), &address, incarnation);
    let mut guards = core.guards.clone();
    guards.push(DynamicGuard {
        address: address.clone(),
        incarnation,
    });
    ItemPath {
        path: DynamicPath {
            core: PathCore {
                access: Arc::new(ItemAccess {
                    collection: core.access.clone(),
                    collection_address: core.address.clone(),
                    token,
                    key,
                }),
                address,
                session: Some(topology.session()),
                guards,
            },
        },
        collection: core.address.clone(),
        collection_access: core.access.clone(),
        token,
    }
}

macro_rules! impl_items_path {
    ($path:ident, $items_method:ident) => {
        impl<Root: FormSchema, Item: FormSchema> $path<Root, Item> {
            pub fn $items_method(
                &self,
                form: &Entity<Form<Root>>,
                cx: &mut App,
            ) -> Result<Vec<ItemPath<Root, Item>>, MutationError> {
                let form = form.read(cx);
                item_paths_in(&self.core, form.value(), form.topology())
            }

            pub fn append(
                &self,
                form: &Entity<Form<Root>>,
                value: Item,
                cx: &mut App,
            ) -> Result<ItemPath<Root, Item>, MutationError> {
                let path = self.clone();
                form.update(cx, move |form, cx| {
                    path.core.check(form)?;
                    let token = {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let len = path.core.access.get(model, &snapshot)?.len();
                        let mut edit = topology.edit();
                        edit.ensure_items(&path.core.address, len)?;
                        let token = edit.insert_item(&path.core.address, len)?;
                        path.core.access.get_mut(model, &snapshot)?.push(value);
                        topology.commit(edit);
                        token
                    };
                    form.commit_topology(path.core.address.clone(), cx);
                    Ok(make_item_path_in(&path.core, form.topology(), token))
                })
            }

            pub fn insert_before(
                &self,
                form: &Entity<Form<Root>>,
                anchor: &ItemPath<Root, Item>,
                value: Item,
                cx: &mut App,
            ) -> Result<ItemPath<Root, Item>, MutationError> {
                let path = self.clone();
                let anchor = anchor.clone();
                form.update(cx, move |form, cx| {
                    path.core.check(form)?;
                    anchor.path.core.check(form)?;
                    if anchor.collection != path.core.address {
                        return Err(TopologyError::WrongCollection { path: anchor.key() }.into());
                    }
                    let token = {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let len = path.core.access.get(model, &snapshot)?.len();
                        let mut edit = topology.edit();
                        edit.ensure_items(&path.core.address, len)?;
                        let index = edit
                            .item_index(&path.core.address, anchor.token)
                            .ok_or_else(|| ResolveError::MissingItem { path: anchor.key() })?;
                        let token = edit.insert_item(&path.core.address, index)?;
                        path.core
                            .access
                            .get_mut(model, &snapshot)?
                            .insert(index, value);
                        topology.commit(edit);
                        token
                    };
                    form.commit_topology(path.core.address.clone(), cx);
                    Ok(make_item_path_in(&path.core, form.topology(), token))
                })
            }

            pub fn move_before(
                &self,
                form: &Entity<Form<Root>>,
                item: &ItemPath<Root, Item>,
                anchor: &ItemPath<Root, Item>,
                cx: &mut App,
            ) -> Result<(), MutationError> {
                let path = self.clone();
                let item = item.clone();
                let anchor = anchor.clone();
                form.update(cx, move |form, cx| {
                    path.core.check(form)?;
                    item.path.core.check(form)?;
                    anchor.path.core.check(form)?;
                    if item.collection != path.core.address
                        || anchor.collection != path.core.address
                    {
                        return Err(TopologyError::WrongCollection { path: item.key() }.into());
                    }
                    let changed = {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let len = path.core.access.get(model, &snapshot)?.len();
                        let mut edit = topology.edit();
                        edit.ensure_items(&path.core.address, len)?;
                        let source = edit
                            .item_index(&path.core.address, item.token)
                            .ok_or_else(|| ResolveError::MissingItem { path: item.key() })?;
                        let target = edit
                            .item_index(&path.core.address, anchor.token)
                            .ok_or_else(|| ResolveError::MissingItem { path: anchor.key() })?;
                        if source == target || source + 1 == target {
                            false
                        } else {
                            edit.move_item(&path.core.address, source, target)?;
                            let values = path.core.access.get_mut(model, &snapshot)?;
                            let value = values.remove(source);
                            let adjusted = if source < target { target - 1 } else { target };
                            values.insert(adjusted, value);
                            topology.commit(edit);
                            true
                        }
                    };
                    if !changed {
                        return Ok(());
                    }
                    form.commit_topology(path.core.address.clone(), cx);
                    Ok(())
                })
            }

            pub fn remove(
                &self,
                form: &Entity<Form<Root>>,
                item: ItemPath<Root, Item>,
                cx: &mut App,
            ) -> Result<Item, MutationError> {
                let path = self.clone();
                form.update(cx, move |form, cx| {
                    path.core.check(form)?;
                    item.path.core.check(form)?;
                    if item.collection != path.core.address {
                        return Err(TopologyError::WrongCollection { path: item.key() }.into());
                    }
                    let value = {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let len = path.core.access.get(model, &snapshot)?.len();
                        let mut edit = topology.edit();
                        edit.ensure_items(&path.core.address, len)?;
                        let index = edit
                            .item_index(&path.core.address, item.token)
                            .ok_or_else(|| ResolveError::MissingItem { path: item.key() })?;
                        edit.remove_item(&path.core.address, index)?;
                        let value = path.core.access.get_mut(model, &snapshot)?.remove(index);
                        topology.commit(edit);
                        value
                    };
                    form.commit_topology(path.core.address.clone(), cx);
                    Ok(value)
                })
            }

            pub fn replace_all(
                &self,
                form: &Entity<Form<Root>>,
                values: Vec<Item>,
                cx: &mut App,
            ) -> Result<Vec<ItemPath<Root, Item>>, MutationError> {
                let path = self.clone();
                form.update(cx, move |form, cx| {
                    path.core.check(form)?;
                    {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let mut edit = topology.edit();
                        edit.retire_descendants(&path.core.address);
                        edit.ensure_items(&path.core.address, values.len())?;
                        *path.core.access.get_mut(model, &snapshot)? = values;
                        topology.commit(edit);
                    }
                    form.commit_topology(path.core.address.clone(), cx);
                    item_paths_in(&path.core, form.value(), form.topology())
                })
            }
        }
    };
}

impl_items_path!(TotalItemsPath, items);
impl_items_path!(DynamicItemsPath, try_items);

fn move_between<Root: FormSchema, Item: FormSchema>(
    form: &Entity<Form<Root>>,
    source: ItemPath<Root, Item>,
    destination: PathCore<Root, Vec<Item>>,
    position: Position,
    cx: &mut App,
) -> Result<ItemPath<Root, Item>, MutationError> {
    form.update(cx, move |form, cx| {
        source.path.core.check(form)?;
        destination.check(form)?;
        if source.path.core.address.is_prefix_of(&destination.address) {
            return Err(TopologyError::MoveIntoDescendant { path: source.key() }.into());
        }
        if source.collection == destination.address {
            return Err(TopologyError::WrongCollection { path: source.key() }.into());
        }
        let (token, previous_topology, source_index, destination_index, moved) = {
            let (model, topology) = form.model_and_topology();
            let snapshot = topology.snapshot();
            let mut edit = topology.edit();
            let source_index = edit
                .item_index(&source.collection, source.token)
                .ok_or_else(|| ResolveError::MissingItem { path: source.key() })?;
            let destination_len = destination.access.get(model, &snapshot)?.len();
            edit.ensure_items(&destination.address, destination_len)?;
            let destination_index = match position {
                Position::Start => 0,
                Position::End => destination_len,
            };
            let token = edit.insert_item(&destination.address, destination_index)?;
            edit.remove_item(&source.collection, source_index)?;
            let moved = source
                .collection_access
                .get_mut(model, &snapshot)?
                .remove(source_index);
            let previous_topology = topology.replace_with(edit);
            (
                token,
                previous_topology,
                source_index,
                destination_index,
                moved,
            )
        };
        let mut moved = Some(moved);
        let insert_result = {
            let (model, topology) = form.model_and_topology();
            let snapshot = topology.snapshot();
            destination.access.get_mut(model, &snapshot).map(|values| {
                values.insert(
                    destination_index,
                    moved.take().expect("moved value is inserted once"),
                );
            })
        };
        if let Err(error) = insert_result {
            let (model, topology) = form.model_and_topology();
            topology.commit(previous_topology);
            let snapshot = topology.snapshot();
            source
                .collection_access
                .get_mut(model, &snapshot)
                .expect("preflighted source collection must remain reachable")
                .insert(
                    source_index,
                    moved.take().expect("failed insertion retains moved value"),
                );
            return Err(error.into());
        }
        form.commit_topology_scopes(
            destination.address.clone(),
            vec![source.collection.clone(), destination.address.clone()],
            cx,
        );
        Ok(make_item_path_in(&destination, form.topology(), token))
    })
}

pub trait IntoTotalPath<Root: FormSchema, T: 'static> {
    fn into_total_path(self) -> TotalPath<Root, T>;
}

impl<Root: FormSchema, T: 'static> IntoTotalPath<Root, T> for TotalPath<Root, T> {
    fn into_total_path(self) -> TotalPath<Root, T> {
        self
    }
}

impl<Root: FormSchema, T: 'static> IntoTotalPath<Root, T> for FieldDef<Root, T> {
    fn into_total_path(self) -> TotalPath<Root, T> {
        RootDef::<Root>::__new().then(self)
    }
}

impl<Root: FormSchema, T: 'static> IntoTotalPath<Root, T> for ChildDef<Root, T> {
    fn into_total_path(self) -> TotalPath<Root, T> {
        RootDef::<Root>::__new().then(self)
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> FieldDef<Root, T> {
    pub fn value(&self, form: &Entity<Form<Root>>, cx: &App) -> T {
        (*self).into_total_path().value(form, cx)
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) {
        (*self).into_total_path().set(form, value, cx);
    }

    pub fn validate(&self, form: &Entity<Form<Root>>, trigger: ValidationTrigger, cx: &mut App) {
        (*self).into_total_path().validate(form, trigger, cx);
    }

    pub fn errors(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ValidationIssue> {
        (*self).into_total_path().errors(form, cx)
    }

    pub fn bind_control(
        &self,
        form: &Entity<Form<Root>>,
        cx: &mut App,
    ) -> crate::ControlBinding<Root, T> {
        (*self).into_total_path().bind_control(form, cx)
    }

    pub fn schema_info(&self) -> FieldSchema {
        self.schema()
    }
}

impl<Root: FormSchema, T: FormSchema> ItemsDef<Root, T> {
    pub fn items(
        &self,
        form: &Entity<Form<Root>>,
        cx: &mut App,
    ) -> Result<Vec<ItemPath<Root, T>>, MutationError> {
        RootDef::<Root>::__new().then(*self).items(form, cx)
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> ChildDef<Root, T> {
    pub fn value(&self, form: &Entity<Form<Root>>, cx: &App) -> T {
        (*self).into_total_path().value(form, cx)
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) {
        (*self).into_total_path().set(form, value, cx);
    }

    pub fn validate(&self, form: &Entity<Form<Root>>, trigger: ValidationTrigger, cx: &mut App) {
        (*self).into_total_path().validate(form, trigger, cx);
    }

    pub fn errors(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ValidationIssue> {
        (*self).into_total_path().errors(form, cx)
    }
}

impl<Root: FormSchema, T> crate::validation::sealed::Sealed for FieldDef<Root, T> {}
impl<Root: FormSchema, T> crate::validation::sealed::Sealed for ChildDef<Root, T> {}
impl<Root: FormSchema, T: 'static> crate::validation::sealed::Sealed for TotalPath<Root, T> {}
impl<Root: FormSchema, T: 'static> crate::validation::sealed::Sealed for DynamicPath<Root, T> {}
impl<Root: FormSchema, T: FormSchema> crate::validation::sealed::Sealed for ItemPath<Root, T> {}

macro_rules! impl_definition_validation_path {
    ($definition:ident) => {
        impl<Root: FormSchema, T: 'static> ValidationPath<Root> for $definition<Root, T> {
            fn __included(&self, request: &ValidationRequest<'_, Root>) -> bool {
                request.includes_address(&root_address().field(self.name()))
            }

            fn __push(
                self,
                sink: &mut ValidationSink<'_, Root>,
                code: std::borrow::Cow<'static, str>,
                message: ValidationMessage,
            ) {
                sink.push(root_address().field(self.name()), code, message);
            }
        }
    };
}

impl_definition_validation_path!(FieldDef);
impl_definition_validation_path!(ChildDef);

impl<Root: FormSchema, T: 'static> ValidationPath<Root> for TotalPath<Root, T> {
    fn __included(&self, request: &ValidationRequest<'_, Root>) -> bool {
        request.includes_address(&self.core.address)
    }

    fn __push(
        self,
        sink: &mut ValidationSink<'_, Root>,
        code: std::borrow::Cow<'static, str>,
        message: ValidationMessage,
    ) {
        sink.push(self.core.address, code, message);
    }
}

impl<Root: FormSchema, T: 'static> ValidationPath<Root> for DynamicPath<Root, T> {
    fn __included(&self, request: &ValidationRequest<'_, Root>) -> bool {
        request.includes_dynamic(self.core.session, &self.core.guards, &self.core.address)
    }

    fn __push(
        self,
        sink: &mut ValidationSink<'_, Root>,
        code: std::borrow::Cow<'static, str>,
        message: ValidationMessage,
    ) {
        sink.push(self.core.address, code, message);
    }
}

impl<Root: FormSchema, T: FormSchema> ValidationPath<Root> for ItemPath<Root, T> {
    fn __included(&self, request: &ValidationRequest<'_, Root>) -> bool {
        request.includes_dynamic(
            self.path.core.session,
            &self.path.core.guards,
            &self.path.core.address,
        )
    }

    fn __push(
        self,
        sink: &mut ValidationSink<'_, Root>,
        code: std::borrow::Cow<'static, str>,
        message: ValidationMessage,
    ) {
        sink.push(self.path.core.address, code, message);
    }
}
