mod access;

use std::{marker::PhantomData, sync::Arc};

use gpui::{App, Context, Entity, Window};

use crate::{
    CaseDef, ChildDef, FieldDef, Form, FormSchema, ItemToken, ItemsDef, MutationError, PathKey,
    ResolveError, RootDef, TopologyError, ValidationIssue, ValidationMessage, ValidationPath,
    ValidationRequest, ValidationSink, ValidationTrigger,
    topology::{CanonicalAddress, DynamicGuard, SessionId, root_address},
};
use access::{Access, CaseAccess, FieldAccess, ItemAccess, OptionalAccess, RootAccess};

pub(crate) struct PathCore<Root, T> {
    pub(crate) access: Arc<dyn Access<Root, T>>,
    pub(crate) address: CanonicalAddress,
    pub(crate) session: Option<SessionId>,
    pub(crate) guards: Vec<DynamicGuard>,
    pub(crate) identity: Option<PathKey>,
}

impl<Root, T> Clone for PathCore<Root, T> {
    fn clone(&self) -> Self {
        Self {
            access: self.access.clone(),
            address: self.address.clone(),
            session: self.session,
            guards: self.guards.clone(),
            identity: self.identity.clone(),
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
            identity: None,
        }
    }
}

impl<Root: FormSchema, T: 'static> PathCore<Root, T> {
    pub(crate) fn then<U: 'static>(
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
            identity: self.identity,
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
                    path: guard.key.clone(),
                });
            }
        }
        Ok(())
    }

    fn key_for(&self, form: &Form<Root>) -> PathKey {
        self.identity
            .clone()
            .or_else(|| form.topology().key(&self.address))
            .expect("form construction or topology edit must materialize every path identity")
    }

    pub(crate) fn change_address(&self) -> &CanonicalAddress {
        &self.address
    }

    pub(crate) fn change_session(&self) -> Option<SessionId> {
        self.session
    }

    pub(crate) fn is_active_in(&self, topology: &crate::topology::TopologyIndex) -> bool {
        self.guards.iter().all(|guard| {
            topology.incarnation(&guard.address) == Some(guard.incarnation)
                && topology.key(&guard.address).as_ref() == Some(&guard.key)
        })
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

/// A dynamic path that is valid only for one validation request snapshot.
///
/// It can be composed and used to read or report validation issues, but it has
/// no mutation or control-binding API.
pub struct ValidationDynamicPath<'a, Root: FormSchema, T: 'static> {
    pub(crate) core: PathCore<Root, T>,
    marker: PhantomData<&'a ()>,
}

impl<Root: FormSchema, T: 'static> Clone for ValidationDynamicPath<'_, Root, T> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            marker: PhantomData,
        }
    }
}

/// A collection path that is valid only for one validation request snapshot.
pub struct ValidationDynamicItemsPath<'a, Root: FormSchema, Item: FormSchema> {
    pub(crate) core: PathCore<Root, Vec<Item>>,
    marker: PhantomData<&'a ()>,
}

impl<Root: FormSchema, Item: FormSchema> Clone for ValidationDynamicItemsPath<'_, Root, Item> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            marker: PhantomData,
        }
    }
}

/// An item occurrence that is valid only for one validation request snapshot.
pub struct ValidationItemPath<'a, Root: FormSchema, Item: FormSchema> {
    path: ValidationDynamicPath<'a, Root, Item>,
}

impl<Root: FormSchema, Item: FormSchema> Clone for ValidationItemPath<'_, Root, Item> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
        }
    }
}

pub struct ValidationCaseResolver<'a, Root, Enum, Payload>
where
    Root: FormSchema,
    Enum: FormSchema,
    Payload: FormSchema,
{
    path: ValidationDynamicPath<'a, Root, Enum>,
    case: CaseDef<Enum, Payload>,
}

pub struct ValidationOptionalResolver<'a, Root: FormSchema, T: FormSchema> {
    path: ValidationDynamicPath<'a, Root, Option<T>>,
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

#[doc(hidden)]
pub trait ValidationPathEdge<'a, Root: FormSchema, Owner: 'static>: sealed::Sealed + Sized {
    type Output;

    fn __from_validation(self, parent: ValidationDynamicPath<'a, Root, Owner>) -> Self::Output;
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

impl<'a, Root: FormSchema, Owner: 'static, T: 'static> ValidationPathEdge<'a, Root, Owner>
    for FieldDef<Owner, T>
{
    type Output = ValidationDynamicPath<'a, Root, T>;

    fn __from_validation(self, parent: ValidationDynamicPath<'a, Root, Owner>) -> Self::Output {
        ValidationDynamicPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
            marker: PhantomData,
        }
    }
}

impl<'a, Root: FormSchema, Owner: 'static, T: 'static> ValidationPathEdge<'a, Root, Owner>
    for ChildDef<Owner, T>
{
    type Output = ValidationDynamicPath<'a, Root, T>;

    fn __from_validation(self, parent: ValidationDynamicPath<'a, Root, Owner>) -> Self::Output {
        ValidationDynamicPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
            marker: PhantomData,
        }
    }
}

impl<'a, Root: FormSchema, Owner: 'static, T: FormSchema> ValidationPathEdge<'a, Root, Owner>
    for ItemsDef<Owner, T>
{
    type Output = ValidationDynamicItemsPath<'a, Root, T>;

    fn __from_validation(self, parent: ValidationDynamicPath<'a, Root, Owner>) -> Self::Output {
        ValidationDynamicItemsPath {
            core: parent.core.then(self.name(), self.read(), self.read_mut()),
            marker: PhantomData,
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

impl<'a, Root: FormSchema, T: 'static> ValidationDynamicPath<'a, Root, T> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::Output
    where
        Edge: ValidationPathEdge<'a, Root, T>,
    {
        edge.__from_validation(self)
    }
}

impl<'a, Root: FormSchema, Enum: FormSchema> ValidationDynamicPath<'a, Root, Enum> {
    pub fn case<Payload: FormSchema>(
        self,
        case: CaseDef<Enum, Payload>,
    ) -> ValidationCaseResolver<'a, Root, Enum, Payload> {
        ValidationCaseResolver { path: self, case }
    }
}

impl<'a, Root: FormSchema, T: FormSchema> ValidationDynamicPath<'a, Root, Option<T>> {
    pub fn some(self) -> ValidationOptionalResolver<'a, Root, T> {
        ValidationOptionalResolver { path: self }
    }
}

impl<'a, Root: FormSchema, Item: FormSchema> ValidationItemPath<'a, Root, Item> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::Output
    where
        Edge: ValidationPathEdge<'a, Root, Item>,
    {
        edge.__from_validation(self.path)
    }

    pub fn case<Payload: FormSchema>(
        self,
        case: CaseDef<Item, Payload>,
    ) -> ValidationCaseResolver<'a, Root, Item, Payload> {
        self.path.case(case)
    }
}

impl<'a, Root, Enum, Payload> ValidationCaseResolver<'a, Root, Enum, Payload>
where
    Root: FormSchema,
    Enum: FormSchema,
    Payload: FormSchema,
{
    pub fn resolve(
        self,
        request: &ValidationRequest<'a, Root>,
    ) -> Result<Option<ValidationDynamicPath<'a, Root, Payload>>, ResolveError> {
        request.try_case(self.path, self.case)
    }
}

impl<'a, Root: FormSchema, T: FormSchema> ValidationOptionalResolver<'a, Root, T> {
    pub fn resolve(
        self,
        request: &ValidationRequest<'a, Root>,
    ) -> Result<Option<ValidationDynamicPath<'a, Root, T>>, ResolveError> {
        request.try_some(self.path)
    }
}

impl<Root: FormSchema, Item: FormSchema> ItemPath<Root, Item> {
    pub fn then<Edge>(self, edge: Edge) -> Edge::DynamicOutput
    where
        Edge: PathEdge<Root, Item>,
    {
        edge.__from_dynamic(self.path)
    }

    pub fn case<Payload: FormSchema>(
        self,
        case: CaseDef<Item, Payload>,
    ) -> CaseResolver<Root, Item, Payload> {
        self.path.case(case)
    }

    pub fn key(&self) -> PathKey {
        self.path
            .core
            .identity
            .clone()
            .expect("item paths always have a materialized dynamic identity")
    }

    pub(crate) fn change_target_info(&self) -> (&CanonicalAddress, SessionId, &PathKey) {
        (
            &self.path.core.address,
            self.path
                .core
                .session
                .expect("item paths are session bound"),
            self.path
                .core
                .identity
                .as_ref()
                .expect("item paths always have a materialized dynamic identity"),
        )
    }

    pub fn try_get(&self, form: &Entity<Form<Root>>, cx: &App) -> Result<Item, ResolveError> {
        self.path.try_get(form, cx)
    }

    pub fn try_set(
        &self,
        form: &Entity<Form<Root>>,
        value: Item,
        cx: &mut App,
    ) -> Result<bool, ResolveError> {
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

    pub fn get(&self, form: &Entity<Form<Root>>, cx: &App) -> T {
        let form = form.read(cx);
        self.core
            .access
            .get(form.value(), &form.topology().snapshot())
            .expect("a total path must always resolve")
            .clone()
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) -> bool {
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
                return false;
            }
            form.commit_value(path.core.address.clone(), cx);
            true
        })
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

    pub fn bind_control_in<Owner>(
        &self,
        form: &Entity<Form<Root>>,
        owner: &Entity<Owner>,
        project: impl Fn(&mut Owner, crate::ControlProjection<T>, &mut Window, &mut Context<Owner>)
        + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::ControlBinding, crate::ControlWriter<Root, T>)
    where
        Owner: 'static,
    {
        crate::control::bind_total_in(form, self.clone(), owner, project, window, cx)
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> DynamicPath<Root, T> {
    pub fn try_get(&self, form: &Entity<Form<Root>>, cx: &App) -> Result<T, ResolveError> {
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
    ) -> Result<bool, ResolveError> {
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
                return Ok(false);
            }
            form.commit_value(path.core.address.clone(), cx);
            Ok(true)
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

    pub fn try_bind_control_in<Owner>(
        &self,
        form: &Entity<Form<Root>>,
        owner: &Entity<Owner>,
        project: impl Fn(&mut Owner, crate::ControlProjection<T>, &mut Window, &mut Context<Owner>)
        + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(crate::ControlBinding, crate::ControlWriter<Root, T>), ResolveError>
    where
        Owner: 'static,
    {
        crate::control::bind_dynamic_in(form, self.clone(), owner, project, window, cx)
    }
}

pub struct CaseResolver<Root: FormSchema, Enum: FormSchema, Payload: FormSchema> {
    core: PathCore<Root, Enum>,
    case: CaseDef<Enum, Payload>,
    dynamic_start: bool,
}

impl<Root: FormSchema, Enum: FormSchema, Payload: FormSchema> CaseResolver<Root, Enum, Payload> {
    pub fn resolve(
        self,
        form: &Entity<Form<Root>>,
        cx: &App,
    ) -> Result<Option<DynamicPath<Root, Payload>>, ResolveError> {
        let form = form.read(cx);
        if self.dynamic_start {
            self.core.check(form)?;
        }
        locate_case_in(self.core, form.value(), form.topology(), self.case)
    }
}

pub struct OptionalResolver<Root: FormSchema, T: FormSchema> {
    core: PathCore<Root, Option<T>>,
    dynamic_start: bool,
}

impl<Root: FormSchema, T: FormSchema> OptionalResolver<Root, T> {
    pub fn resolve(
        self,
        form: &Entity<Form<Root>>,
        cx: &App,
    ) -> Result<Option<DynamicPath<Root, T>>, ResolveError> {
        let form = form.read(cx);
        if self.dynamic_start {
            self.core.check(form)?;
        }
        locate_some_in(self.core, form.value(), form.topology())
    }
}

impl<Root: FormSchema, Enum: FormSchema> TotalPath<Root, Enum> {
    pub fn case<Payload: FormSchema>(
        self,
        case: CaseDef<Enum, Payload>,
    ) -> CaseResolver<Root, Enum, Payload> {
        CaseResolver {
            core: self.core,
            case,
            dynamic_start: false,
        }
    }
}

impl<Root: FormSchema, Enum: FormSchema> DynamicPath<Root, Enum> {
    pub fn case<Payload: FormSchema>(
        self,
        case: CaseDef<Enum, Payload>,
    ) -> CaseResolver<Root, Enum, Payload> {
        CaseResolver {
            core: self.core,
            case,
            dynamic_start: true,
        }
    }
}

impl<Root: FormSchema, T: FormSchema> TotalPath<Root, Option<T>> {
    pub fn some(self) -> OptionalResolver<Root, T> {
        OptionalResolver {
            core: self.core,
            dynamic_start: false,
        }
    }
}

impl<Root: FormSchema, T: FormSchema> DynamicPath<Root, Option<T>> {
    pub fn some(self) -> OptionalResolver<Root, T> {
        OptionalResolver {
            core: self.core,
            dynamic_start: true,
        }
    }
}

pub(crate) fn locate_case_in<Root: FormSchema, Enum: FormSchema, Payload: FormSchema>(
    core: PathCore<Root, Enum>,
    model: &Root,
    topology: &crate::topology::TopologyIndex,
    case: CaseDef<Enum, Payload>,
) -> Result<Option<DynamicPath<Root, Payload>>, ResolveError> {
    let snapshot = topology.snapshot();
    locate_case_in_snapshot(core, model, &snapshot, case)
}

fn locate_case_in_snapshot<Root: FormSchema, Enum: FormSchema, Payload: FormSchema>(
    core: PathCore<Root, Enum>,
    model: &Root,
    topology: &crate::topology::TopologySnapshot,
    case: CaseDef<Enum, Payload>,
) -> Result<Option<DynamicPath<Root, Payload>>, ResolveError> {
    if (case.read())(core.access.get(model, topology)?).is_none() {
        return Ok(None);
    }
    let occurrence = topology
        .active_case(&core.address, case.name())
        .expect("active case topology must be materialized before resolving");
    let address = core.address.case_occurrence(case.name(), occurrence);
    let incarnation = address
        .final_occurrence()
        .expect("case addresses always end in an occurrence");
    let key = topology
        .key(&address)
        .expect("active case identity must be materialized before resolving a path");
    let mut guards = core.guards;
    guards.push(DynamicGuard {
        address: address.clone(),
        incarnation,
        key: key.clone(),
    });
    Ok(Some(DynamicPath {
        core: PathCore {
            access: Arc::new(CaseAccess {
                parent: core.access,
                case,
                key: key.clone(),
            }),
            address,
            session: Some(topology.session()),
            guards,
            identity: Some(key),
        },
    }))
}

pub(crate) fn locate_some_in<Root: FormSchema, T: FormSchema>(
    core: PathCore<Root, Option<T>>,
    model: &Root,
    topology: &crate::topology::TopologyIndex,
) -> Result<Option<DynamicPath<Root, T>>, ResolveError> {
    let snapshot = topology.snapshot();
    locate_some_in_snapshot(core, model, &snapshot)
}

fn locate_some_in_snapshot<Root: FormSchema, T: FormSchema>(
    core: PathCore<Root, Option<T>>,
    model: &Root,
    topology: &crate::topology::TopologySnapshot,
) -> Result<Option<DynamicPath<Root, T>>, ResolveError> {
    if core.access.get(model, topology)?.is_none() {
        return Ok(None);
    }
    let occurrence = topology
        .active_some(&core.address)
        .expect("active optional topology must be materialized before resolving");
    let address = core.address.some_occurrence(occurrence);
    let incarnation = address
        .final_occurrence()
        .expect("optional addresses always end in an occurrence");
    let key = topology
        .key(&address)
        .expect("active optional identity must be materialized before resolving a path");
    let mut guards = core.guards;
    guards.push(DynamicGuard {
        address: address.clone(),
        incarnation,
        key: key.clone(),
    });
    Ok(Some(DynamicPath {
        core: PathCore {
            access: Arc::new(OptionalAccess {
                parent: core.access,
                key: key.clone(),
            }),
            address,
            session: Some(topology.session()),
            guards,
            identity: Some(key),
        },
    }))
}

pub(crate) fn item_paths_in<Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    model: &Root,
    topology: &crate::topology::TopologyIndex,
) -> Result<Vec<ItemPath<Root, Item>>, ResolveError> {
    let snapshot = topology.snapshot();
    item_paths_in_snapshot(core, model, &snapshot)
}

fn item_paths_in_snapshot<Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    model: &Root,
    topology: &crate::topology::TopologySnapshot,
) -> Result<Vec<ItemPath<Root, Item>>, ResolveError> {
    if core
        .session
        .is_some_and(|session| session != topology.session())
    {
        return Err(ResolveError::WrongSession {
            path: core
                .identity
                .clone()
                .or_else(|| topology.key(&core.address))
                .expect("form construction must materialize total path identities"),
        });
    }
    for guard in &core.guards {
        if topology.incarnation(&guard.address) != Some(guard.incarnation) {
            return Err(ResolveError::Retired {
                path: guard.key.clone(),
            });
        }
    }
    let len = core.access.get(model, topology)?.len();
    let tokens = topology
        .items(&core.address)
        .expect("form construction must materialize every collection topology");
    debug_assert_eq!(tokens.len(), len, "topology must match the model snapshot");
    Ok(tokens
        .into_iter()
        .map(|token| make_item_path_in_snapshot(core, topology, token))
        .collect())
}

pub(crate) fn make_item_path_in<Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    topology: &crate::topology::TopologyIndex,
    token: ItemToken,
) -> ItemPath<Root, Item> {
    let snapshot = topology.snapshot();
    make_item_path_in_snapshot(core, &snapshot, token)
}

fn make_item_path_in_snapshot<Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    topology: &crate::topology::TopologySnapshot,
    token: ItemToken,
) -> ItemPath<Root, Item> {
    let address = core.address.item(token);
    let incarnation = address
        .final_occurrence()
        .expect("item addresses always end in an occurrence");
    let key = topology
        .key(&address)
        .expect("item identity must be materialized before reading items");
    let mut guards = core.guards.clone();
    guards.push(DynamicGuard {
        address: address.clone(),
        incarnation,
        key: key.clone(),
    });
    ItemPath {
        path: DynamicPath {
            core: PathCore {
                access: Arc::new(ItemAccess {
                    collection: core.access.clone(),
                    collection_address: core.address.clone(),
                    token,
                    key: key.clone(),
                }),
                address,
                session: Some(topology.session()),
                guards,
                identity: Some(key),
            },
        },
        collection: core.address.clone(),
        collection_access: core.access.clone(),
        token,
    }
}

pub(crate) fn validation_item_paths_in<'a, Root: FormSchema, Item: FormSchema>(
    core: &PathCore<Root, Vec<Item>>,
    model: &'a Root,
    topology: &crate::topology::TopologySnapshot,
) -> Result<Vec<ValidationItemPath<'a, Root, Item>>, ResolveError> {
    Ok(item_paths_in_snapshot(core, model, topology)?
        .into_iter()
        .map(|item| ValidationItemPath {
            path: ValidationDynamicPath {
                core: item.path.core,
                marker: PhantomData,
            },
        })
        .collect())
}

pub(crate) fn validation_case_in<'a, Root, Enum, Payload>(
    core: PathCore<Root, Enum>,
    model: &'a Root,
    topology: &crate::topology::TopologySnapshot,
    case: CaseDef<Enum, Payload>,
) -> Result<Option<ValidationDynamicPath<'a, Root, Payload>>, ResolveError>
where
    Root: FormSchema,
    Enum: FormSchema,
    Payload: FormSchema,
{
    Ok(
        locate_case_in_snapshot(core, model, topology, case)?.map(|path| ValidationDynamicPath {
            core: path.core,
            marker: PhantomData,
        }),
    )
}

pub(crate) fn validation_some_in<'a, Root: FormSchema, T: FormSchema>(
    core: PathCore<Root, Option<T>>,
    model: &'a Root,
    topology: &crate::topology::TopologySnapshot,
) -> Result<Option<ValidationDynamicPath<'a, Root, T>>, ResolveError> {
    Ok(
        locate_some_in_snapshot(core, model, topology)?.map(|path| ValidationDynamicPath {
            core: path.core,
            marker: PhantomData,
        }),
    )
}

impl<Root: FormSchema, Item: FormSchema> TotalItemsPath<Root, Item> {
    pub fn items(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ItemPath<Root, Item>> {
        let form = form.read(cx);
        item_paths_in(&self.core, form.value(), form.topology())
            .expect("a total collection path must always resolve")
    }
}

impl<Root: FormSchema, Item: FormSchema> DynamicItemsPath<Root, Item> {
    pub fn try_items(
        &self,
        form: &Entity<Form<Root>>,
        cx: &App,
    ) -> Result<Vec<ItemPath<Root, Item>>, ResolveError> {
        let form = form.read(cx);
        item_paths_in(&self.core, form.value(), form.topology())
    }
}

macro_rules! impl_items_path {
    ($path:ident) => {
        impl<Root: FormSchema, Item: FormSchema> $path<Root, Item> {
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
                        edit.ensure_items(&path.core.address, len);
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
                        edit.ensure_items(&path.core.address, len);
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
                        edit.ensure_items(&path.core.address, len);
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
                    let retired = item.path.core.address.clone();
                    let value = {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let len = path.core.access.get(model, &snapshot)?.len();
                        let mut edit = topology.edit();
                        edit.ensure_items(&path.core.address, len);
                        let index = edit
                            .item_index(&path.core.address, item.token)
                            .ok_or_else(|| ResolveError::MissingItem { path: item.key() })?;
                        edit.remove_item(&path.core.address, index)?;
                        let value = path.core.access.get_mut(model, &snapshot)?.remove(index);
                        topology.commit(edit);
                        value
                    };
                    form.commit_topology_retiring(path.core.address.clone(), vec![retired], cx);
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
                    let retired = form
                        .topology()
                        .items(&path.core.address)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|token| path.core.address.item(token))
                        .collect();
                    {
                        let (model, topology) = form.model_and_topology();
                        let snapshot = topology.snapshot();
                        let mut edit = topology.edit();
                        edit.retire_descendants(&path.core.address);
                        edit.ensure_items(&path.core.address, values.len());
                        *path.core.access.get_mut(model, &snapshot)? = values;
                        topology.commit(edit);
                    }
                    form.commit_topology_retiring(path.core.address.clone(), retired, cx);
                    Ok(item_paths_in(&path.core, form.value(), form.topology())?)
                })
            }
        }
    };
}

impl_items_path!(TotalItemsPath);
impl_items_path!(DynamicItemsPath);

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
        let (staged_model, edit, token) = {
            let topology = form.topology();
            let snapshot = topology.snapshot();
            let mut staged_model = form.value().clone();
            let mut edit = topology.edit();
            let source_index = edit
                .item_index(&source.collection, source.token)
                .ok_or_else(|| ResolveError::MissingItem { path: source.key() })?;
            let destination_len = destination.access.get(&staged_model, &snapshot)?.len();
            edit.ensure_items(&destination.address, destination_len);
            let destination_index = match position {
                Position::Start => 0,
                Position::End => destination_len,
            };
            let token = edit.insert_item(&destination.address, destination_index)?;
            edit.remove_item(&source.collection, source_index)?;
            let moved = source
                .collection_access
                .get_mut(&mut staged_model, &snapshot)?
                .remove(source_index);
            let staged_snapshot = edit.snapshot();
            destination
                .access
                .get_mut(&mut staged_model, &staged_snapshot)?
                .insert(destination_index, moved);
            (staged_model, edit, token)
        };
        {
            let (model, topology) = form.model_and_topology();
            *model = staged_model;
            topology.commit(edit);
        }
        form.commit_topology_scopes_retiring(
            destination.address.clone(),
            vec![source.collection.clone(), destination.address.clone()],
            vec![source.path.core.address.clone()],
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

impl<Root: FormSchema> IntoTotalPath<Root, Root> for RootDef<Root> {
    fn into_total_path(self) -> TotalPath<Root, Root> {
        TotalPath {
            core: PathCore::root(),
        }
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
    pub fn get(&self, form: &Entity<Form<Root>>, cx: &App) -> T {
        (*self).into_total_path().get(form, cx)
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) -> bool {
        (*self).into_total_path().set(form, value, cx)
    }

    pub fn validate(&self, form: &Entity<Form<Root>>, trigger: ValidationTrigger, cx: &mut App) {
        (*self).into_total_path().validate(form, trigger, cx);
    }

    pub fn errors(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ValidationIssue> {
        (*self).into_total_path().errors(form, cx)
    }

    pub fn bind_control_in<Owner>(
        &self,
        form: &Entity<Form<Root>>,
        owner: &Entity<Owner>,
        project: impl Fn(&mut Owner, crate::ControlProjection<T>, &mut Window, &mut Context<Owner>)
        + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::ControlBinding, crate::ControlWriter<Root, T>)
    where
        Owner: 'static,
    {
        (*self)
            .into_total_path()
            .bind_control_in(form, owner, project, window, cx)
    }
}

impl<Root: FormSchema> RootDef<Root> {
    pub fn get(&self, form: &Entity<Form<Root>>, cx: &App) -> Root {
        (*self).into_total_path().get(form, cx)
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: Root, cx: &mut App) -> bool {
        (*self).into_total_path().set(form, value, cx)
    }

    pub fn validate(&self, form: &Entity<Form<Root>>, trigger: ValidationTrigger, cx: &mut App) {
        (*self).into_total_path().validate(form, trigger, cx);
    }

    pub fn errors(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ValidationIssue> {
        (*self).into_total_path().errors(form, cx)
    }

    pub fn bind_control_in<Owner>(
        &self,
        form: &Entity<Form<Root>>,
        owner: &Entity<Owner>,
        project: impl Fn(&mut Owner, crate::ControlProjection<Root>, &mut Window, &mut Context<Owner>)
        + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::ControlBinding, crate::ControlWriter<Root, Root>)
    where
        Owner: 'static,
    {
        (*self)
            .into_total_path()
            .bind_control_in(form, owner, project, window, cx)
    }
}

impl<Root: FormSchema, T: FormSchema> ItemsDef<Root, T> {
    pub fn items(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ItemPath<Root, T>> {
        RootDef::<Root>::__new().then(*self).items(form, cx)
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> ChildDef<Root, T> {
    pub fn get(&self, form: &Entity<Form<Root>>, cx: &App) -> T {
        (*self).into_total_path().get(form, cx)
    }

    pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) -> bool {
        (*self).into_total_path().set(form, value, cx)
    }

    pub fn validate(&self, form: &Entity<Form<Root>>, trigger: ValidationTrigger, cx: &mut App) {
        (*self).into_total_path().validate(form, trigger, cx);
    }

    pub fn errors(&self, form: &Entity<Form<Root>>, cx: &App) -> Vec<ValidationIssue> {
        (*self).into_total_path().errors(form, cx)
    }
}

macro_rules! impl_definition_resolvers {
    ($definition:ident) => {
        impl<Root: FormSchema, Enum: FormSchema> $definition<Root, Enum> {
            pub fn case<Payload: FormSchema>(
                self,
                case: CaseDef<Enum, Payload>,
            ) -> CaseResolver<Root, Enum, Payload> {
                self.into_total_path().case(case)
            }
        }

        impl<Root: FormSchema, T: FormSchema> $definition<Root, Option<T>> {
            pub fn some(self) -> OptionalResolver<Root, T> {
                self.into_total_path().some()
            }
        }
    };
}

impl_definition_resolvers!(FieldDef);
impl_definition_resolvers!(ChildDef);

impl<Root: FormSchema, T> crate::validation::sealed::Sealed for FieldDef<Root, T> {}
impl<Root: FormSchema, T> crate::validation::sealed::Sealed for ChildDef<Root, T> {}
impl<Root: FormSchema, T: 'static> crate::validation::sealed::Sealed for TotalPath<Root, T> {}
impl<Root: FormSchema, T: 'static> crate::validation::sealed::Sealed
    for ValidationDynamicPath<'_, Root, T>
{
}
impl<Root: FormSchema, T: FormSchema> crate::validation::sealed::Sealed
    for ValidationDynamicItemsPath<'_, Root, T>
{
}
impl<Root: FormSchema, T: FormSchema> crate::validation::sealed::Sealed
    for ValidationItemPath<'_, Root, T>
{
}

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

impl<Root: FormSchema, T: 'static> ValidationPath<Root> for ValidationDynamicPath<'_, Root, T> {
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

impl<Root: FormSchema, T: FormSchema> ValidationPath<Root>
    for ValidationDynamicItemsPath<'_, Root, T>
{
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

impl<Root: FormSchema, T: FormSchema> ValidationPath<Root> for ValidationItemPath<'_, Root, T> {
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
