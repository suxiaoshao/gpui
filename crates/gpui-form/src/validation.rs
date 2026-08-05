use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    marker::PhantomData,
    sync::{Weak, atomic::AtomicBool},
};

use crate::{
    CaseDef, DynamicItemsPath, DynamicPath, FieldSchema, FormSchema, ItemPath, MutationError,
    PathKey, ResolveError, TotalItemsPath,
    path::{item_paths_in, locate_case_in, locate_some_in},
    schema::SchemaVisitor,
    topology::{CanonicalAddress, SessionId, TopologyIndex},
};

mod report;
mod trigger;

pub use trigger::ValidationTrigger;

#[derive(Clone, Debug, PartialEq)]
pub enum ErrorParamValue {
    String(Cow<'static, str>),
    Integer(i64),
    Unsigned(u64),
    Float(f64),
    Bool(bool),
}

macro_rules! impl_param_from {
    ($variant:ident: $($ty:ty),+ $(,)?) => {
        $(impl From<$ty> for ErrorParamValue {
            fn from(value: $ty) -> Self { Self::$variant(value.into()) }
        })+
    };
}

impl_param_from!(String: String, &'static str, Cow<'static, str>);
impl_param_from!(Integer: i8, i16, i32, i64);
impl_param_from!(Unsigned: u8, u16, u32, u64);
impl_param_from!(Float: f32, f64);

impl From<isize> for ErrorParamValue {
    fn from(value: isize) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<usize> for ErrorParamValue {
    fn from(value: usize) -> Self {
        Self::Unsigned(value as u64)
    }
}

impl From<bool> for ErrorParamValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

pub type ErrorParams = BTreeMap<Cow<'static, str>, ErrorParamValue>;

#[derive(Clone, Debug, PartialEq)]
pub enum ValidationMessage {
    Literal(Cow<'static, str>),
    Key {
        key: Cow<'static, str>,
        params: ErrorParams,
    },
}

impl ValidationMessage {
    pub fn literal(value: impl Into<Cow<'static, str>>) -> Self {
        Self::Literal(value.into())
    }

    pub fn key(value: impl Into<Cow<'static, str>>) -> Self {
        Self::Key {
            key: value.into(),
            params: ErrorParams::new(),
        }
    }

    pub fn with_param(
        mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<ErrorParamValue>,
    ) -> Self {
        if let Self::Key { params, .. } = &mut self {
            params.insert(name.into(), value.into());
        }
        self
    }
}

impl From<&'static str> for ValidationMessage {
    fn from(value: &'static str) -> Self {
        Self::literal(value)
    }
}

impl From<String> for ValidationMessage {
    fn from(value: String) -> Self {
        Self::literal(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValidationSource {
    Required,
    Validator(Cow<'static, str>),
    Control(u64),
    Async {
        source: Cow<'static, str>,
        generation: u64,
    },
    Internal,
}

#[derive(Clone, Debug)]
pub struct ValidationIssue {
    pub(crate) path: PathKey,
    pub(crate) address: CanonicalAddress,
    pub(crate) source: ValidationSource,
    pub(crate) trigger: ValidationTrigger,
    pub(crate) code: Cow<'static, str>,
    pub(crate) message: ValidationMessage,
    pub(crate) control_active: Option<Weak<AtomicBool>>,
}

impl PartialEq for ValidationIssue {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.address == other.address
            && self.source == other.source
            && self.trigger == other.trigger
            && self.code == other.code
            && self.message == other.message
    }
}

impl ValidationIssue {
    pub(crate) fn is_active(&self) -> bool {
        self.control_active.as_ref().is_none_or(|active| {
            active
                .upgrade()
                .is_some_and(|active| active.load(std::sync::atomic::Ordering::Acquire))
        })
    }

    pub fn path(&self) -> &PathKey {
        &self.path
    }

    pub fn source(&self) -> &ValidationSource {
        &self.source
    }

    pub fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &ValidationMessage {
        &self.message
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationReport {
    pub(crate) issues: Vec<ValidationIssue>,
}

impl Eq for ValidationReport {}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.issues.iter().all(|issue| !issue.is_active())
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub(crate) fn errors_at(&self, address: &CanonicalAddress) -> Vec<ValidationIssue> {
        self.issues
            .iter()
            .filter(|issue| issue.address == *address && issue.is_active())
            .cloned()
            .collect()
    }

    pub(crate) fn replace_scope(
        &mut self,
        scope: Option<&CanonicalAddress>,
        trigger: ValidationTrigger,
        mut next: Vec<ValidationIssue>,
    ) {
        let replacements = report::replacement_keys(&next);
        self.issues.retain(|issue| {
            if !issue.is_active() {
                return false;
            }
            if matches!(
                issue.source,
                ValidationSource::Control(_) | ValidationSource::Async { .. }
            ) {
                return true;
            }
            let same_bucket = issue.trigger == trigger
                && scope.is_none_or(|scope| intersects(scope, &issue.address));
            let superseded = replacements.iter().any(|(address, source, code)| {
                address == &issue.address && source == &issue.source && code == &issue.code
            });
            !same_bucket && !superseded
        });
        self.issues.append(&mut next);
    }

    pub(crate) fn invalidate_sync(&mut self, scopes: &[CanonicalAddress]) {
        self.issues.retain(|issue| {
            issue.is_active()
                && (matches!(
                    issue.source,
                    ValidationSource::Control(_) | ValidationSource::Async { .. }
                ) || !scopes.iter().any(|scope| intersects(scope, &issue.address)))
        });
    }

    pub(crate) fn retain_current(&mut self, topology: &TopologyIndex) {
        self.issues.retain(|issue| {
            issue.is_active()
                && topology
                    .incarnation(&issue.address)
                    .is_some_and(|incarnation| {
                        issue
                            .path
                            .matches(topology.session(), &issue.address, incarnation)
                    })
        });
    }

    pub(crate) fn replace_control(&mut self, control: u64, issue: Option<ValidationIssue>) -> bool {
        let before = self.issues.len();
        self.issues
            .retain(|candidate| candidate.source != ValidationSource::Control(control));
        let adding = issue.is_some();
        if let Some(issue) = issue {
            self.issues.push(issue);
        }
        before != self.issues.len() || adding
    }

    pub(crate) fn replace_async(
        &mut self,
        generation: u64,
        issue: Option<ValidationIssue>,
    ) -> bool {
        let before = self.issues.len();
        self.issues.retain(|candidate| {
            !matches!(
                candidate.source,
                ValidationSource::Async {
                    generation: candidate_generation,
                    ..
                } if candidate_generation == generation
            )
        });
        let adding = issue.is_some();
        if let Some(issue) = issue {
            self.issues.push(issue);
        }
        before != self.issues.len() || adding
    }

    pub(crate) fn remove_async_generations(
        &mut self,
        generations: &std::collections::HashSet<u64>,
    ) {
        self.issues.retain(|candidate| {
            !matches!(
                candidate.source,
                ValidationSource::Async { generation, .. }
                    if generations.contains(&generation)
            )
        });
    }

    pub(crate) fn remove_async_intersecting(&mut self, address: &CanonicalAddress) {
        self.issues.retain(|candidate| {
            !matches!(candidate.source, ValidationSource::Async { .. })
                || !intersects(address, &candidate.address)
        });
    }
}

pub(crate) fn intersects(left: &CanonicalAddress, right: &CanonicalAddress) -> bool {
    left.is_prefix_of(right) || right.is_prefix_of(left)
}

pub trait Validator<M: FormSchema>: 'static {
    fn validate(
        &self,
        model: &M,
        request: ValidationRequest<'_, M>,
        out: &mut ValidationSink<'_, M>,
    );
}

impl<M: FormSchema, F> Validator<M> for F
where
    F: for<'a, 'b> Fn(&M, ValidationRequest<'a, M>, &mut ValidationSink<'b, M>) + 'static,
{
    fn validate(
        &self,
        model: &M,
        request: ValidationRequest<'_, M>,
        out: &mut ValidationSink<'_, M>,
    ) {
        self(model, request, out);
    }
}

pub struct ValidationRequest<'a, M: FormSchema> {
    trigger: ValidationTrigger,
    scope: Option<&'a CanonicalAddress>,
    session: SessionId,
    pub(crate) topology: &'a TopologyIndex,
    marker: PhantomData<fn() -> M>,
}

impl<M: FormSchema> Copy for ValidationRequest<'_, M> {}

impl<M: FormSchema> Clone for ValidationRequest<'_, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, M: FormSchema> ValidationRequest<'a, M> {
    pub fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    pub fn includes<P: ValidationPath<M>>(&self, path: &P) -> bool {
        path.__included(self)
    }

    /// Enumerates a statically located collection against this validation snapshot.
    pub fn items<Item: FormSchema>(
        &self,
        model: &M,
        path: &TotalItemsPath<M, Item>,
    ) -> Result<Vec<ItemPath<M, Item>>, MutationError> {
        item_paths_in(&path.core, model, self.topology)
    }

    /// Enumerates a dynamically located collection against this validation snapshot.
    pub fn dynamic_items<Item: FormSchema>(
        &self,
        model: &M,
        path: &DynamicItemsPath<M, Item>,
    ) -> Result<Vec<ItemPath<M, Item>>, MutationError> {
        item_paths_in(&path.core, model, self.topology)
    }

    /// Resolves a dynamic value without consulting a newer live Form session.
    pub fn value<'m, T: 'static>(
        &self,
        model: &'m M,
        path: &DynamicPath<M, T>,
    ) -> Result<&'m T, ResolveError> {
        self.check_dynamic(path)?;
        path.core.access.get(model, &self.topology.snapshot())
    }

    /// Resolves an enum payload and binds the returned path to this snapshot's incarnation.
    pub fn try_case<Enum: FormSchema, Payload: FormSchema>(
        &self,
        model: &M,
        path: DynamicPath<M, Enum>,
        case: CaseDef<Enum, Payload>,
    ) -> Result<DynamicPath<M, Payload>, ResolveError> {
        self.check_dynamic(&path)?;
        locate_case_in(path.core, model, self.topology, case)
    }

    /// Resolves an optional payload and binds the returned path to this snapshot's incarnation.
    pub fn try_some<T: FormSchema>(
        &self,
        model: &M,
        path: DynamicPath<M, Option<T>>,
    ) -> Result<DynamicPath<M, T>, ResolveError> {
        self.check_dynamic(&path)?;
        locate_some_in(path.core, model, self.topology)
    }

    fn check_dynamic<T: 'static>(&self, path: &DynamicPath<M, T>) -> Result<(), ResolveError> {
        if path
            .core
            .session
            .is_some_and(|session| session != self.session)
        {
            return Err(ResolveError::WrongSession {
                path: PathKey::total(self.session, &path.core.address),
            });
        }
        if let Some(guard) = path
            .core
            .guards
            .iter()
            .find(|guard| self.topology.incarnation(&guard.address) != Some(guard.incarnation))
        {
            return Err(ResolveError::Retired {
                path: PathKey::new(
                    path.core.session.unwrap_or(self.session),
                    &guard.address,
                    guard.incarnation,
                ),
            });
        }
        Ok(())
    }

    pub(crate) fn includes_address(&self, address: &CanonicalAddress) -> bool {
        self.scope.is_none_or(|scope| intersects(scope, address))
    }

    pub(crate) fn includes_dynamic(
        &self,
        session: Option<SessionId>,
        guards: &[crate::topology::DynamicGuard],
        address: &CanonicalAddress,
    ) -> bool {
        if session.is_some_and(|session| session != self.session) {
            return false;
        }
        if guards
            .iter()
            .any(|guard| self.topology.incarnation(&guard.address) != Some(guard.incarnation))
        {
            return false;
        }
        self.includes_address(address)
    }
}

pub struct ValidationSink<'a, M: FormSchema> {
    session: SessionId,
    topology: &'a TopologyIndex,
    trigger: ValidationTrigger,
    source: ValidationSource,
    issues: Vec<ValidationIssue>,
    marker: PhantomData<fn() -> M>,
}

impl<'a, M: FormSchema> ValidationSink<'a, M> {
    pub fn at<P: ValidationPath<M>>(&mut self, path: P) -> ValidationIssueBuilder<'_, 'a, M, P> {
        ValidationIssueBuilder { sink: self, path }
    }

    pub fn with_source(mut self, source: impl Into<Cow<'static, str>>) -> Self {
        self.source = ValidationSource::Validator(source.into());
        self
    }

    pub(crate) fn push(
        &mut self,
        address: CanonicalAddress,
        code: Cow<'static, str>,
        message: ValidationMessage,
    ) {
        let incarnation = self
            .topology
            .ensure_incarnation(&address)
            .expect("form identity exhausted after construction");
        self.issues.push(ValidationIssue {
            path: PathKey::new(self.session, &address, incarnation),
            address,
            source: self.source.clone(),
            trigger: self.trigger,
            code,
            message,
            control_active: None,
        });
    }

    pub(crate) fn push_with_source(
        &mut self,
        address: CanonicalAddress,
        source: ValidationSource,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
    ) {
        let incarnation = self
            .topology
            .ensure_incarnation(&address)
            .expect("form identity exhausted after construction");
        self.issues.push(ValidationIssue {
            path: PathKey::new(self.session, &address, incarnation),
            address,
            source,
            trigger: self.trigger,
            code: code.into(),
            message,
            control_active: None,
        });
    }
}

pub struct ValidationIssueBuilder<'s, 'a, M: FormSchema, P: ValidationPath<M>> {
    sink: &'s mut ValidationSink<'a, M>,
    path: P,
}

impl<M: FormSchema, P: ValidationPath<M>> ValidationIssueBuilder<'_, '_, M, P> {
    pub fn error(self, code: impl Into<Cow<'static, str>>, message: ValidationMessage) {
        self.path.__push(self.sink, code.into(), message);
    }
}

pub trait ValidationPath<M: FormSchema>: sealed::Sealed {
    #[doc(hidden)]
    fn __included(&self, request: &ValidationRequest<'_, M>) -> bool;

    #[doc(hidden)]
    fn __push(
        self,
        sink: &mut ValidationSink<'_, M>,
        code: Cow<'static, str>,
        message: ValidationMessage,
    );
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub(crate) fn validate<M: FormSchema>(
    model: &M,
    topology: &TopologyIndex,
    trigger: ValidationTrigger,
    scope: Option<&CanonicalAddress>,
    validator: Option<&dyn Validator<M>>,
) -> Vec<ValidationIssue> {
    let mut required = RequiredVisitor {
        topology,
        trigger,
        scope,
        address: CanonicalAddress::default(),
        issues: Vec::new(),
    };
    model.__visit(&mut required);
    let mut issues = required.issues;

    if let Some(validator) = validator {
        let request = ValidationRequest {
            trigger,
            scope,
            session: topology.session(),
            topology,
            marker: PhantomData,
        };
        let mut sink = ValidationSink {
            session: topology.session(),
            topology,
            trigger,
            source: ValidationSource::Validator(Cow::Borrowed("validator")),
            issues: Vec::new(),
            marker: PhantomData,
        };
        validator.validate(model, request, &mut sink);
        issues.extend(sink.issues);
    }

    issues
}

struct RequiredVisitor<'a> {
    topology: &'a TopologyIndex,
    trigger: ValidationTrigger,
    scope: Option<&'a CanonicalAddress>,
    address: CanonicalAddress,
    issues: Vec<ValidationIssue>,
}

impl RequiredVisitor<'_> {
    fn nested(&self, address: CanonicalAddress) -> Self {
        Self {
            topology: self.topology,
            trigger: self.trigger,
            scope: self.scope,
            address,
            issues: Vec::new(),
        }
    }

    fn absorb(&mut self, nested: Self) {
        self.issues.extend(nested.issues);
    }
}

impl SchemaVisitor for RequiredVisitor<'_> {
    fn field(&mut self, schema: FieldSchema, missing: bool) {
        let address = self.address.field(schema.name());
        if !missing
            || !schema.is_required()
            || !schema.triggers().includes(self.trigger)
            || !self.scope.is_none_or(|scope| intersects(scope, &address))
        {
            return;
        }
        let incarnation = self
            .topology
            .ensure_incarnation(&address)
            .expect("form identity exhausted after construction");
        self.issues.push(ValidationIssue {
            path: PathKey::new(self.topology.session(), &address, incarnation),
            address,
            source: ValidationSource::Required,
            trigger: self.trigger,
            code: Cow::Borrowed("required"),
            message: ValidationMessage::key("gpui-form-error-required"),
            control_active: None,
        });
    }

    fn child(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let mut nested = self.nested(self.address.field(name));
        visit(&mut nested);
        self.absorb(nested);
    }

    fn optional(
        &mut self,
        name: &'static str,
        present: bool,
        visit: &mut dyn FnMut(&mut dyn SchemaVisitor),
    ) {
        if !present {
            return;
        }
        let address = self.address.field(name).some();
        self.topology
            .ensure_incarnation(&address)
            .expect("form identity exhausted after construction");
        let mut nested = self.nested(address);
        visit(&mut nested);
        self.absorb(nested);
    }

    fn items(
        &mut self,
        name: &'static str,
        len: usize,
        visit: &mut dyn FnMut(usize, &mut dyn SchemaVisitor),
    ) {
        let collection = self.address.field(name);
        let tokens = self
            .topology
            .ensure_items(&collection, len)
            .expect("form identity exhausted after construction");
        for (index, token) in tokens.into_iter().enumerate() {
            let address = collection.item(token);
            self.topology
                .ensure_incarnation(&address)
                .expect("form identity exhausted after construction");
            let mut nested = self.nested(address);
            visit(index, &mut nested);
            self.absorb(nested);
        }
    }

    fn case(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let address = self.address.case(name);
        self.topology
            .ensure_incarnation(&address)
            .expect("form identity exhausted after construction");
        let mut nested = self.nested(address);
        visit(&mut nested);
        self.absorb(nested);
    }

    fn unit_case(&mut self, name: &'static str) {
        self.topology
            .ensure_incarnation(&self.address.case(name))
            .expect("form identity exhausted after construction");
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} validation issue(s)", self.issues.len())
    }
}
