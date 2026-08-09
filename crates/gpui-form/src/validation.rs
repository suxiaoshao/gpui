use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    marker::PhantomData,
    sync::{Weak, atomic::AtomicBool},
};

use crate::{
    CaseDef, FieldSchema, FormSchema, IntoTotalPath, PathKey, ResolveError, TotalItemsPath,
    ValidationDynamicItemsPath, ValidationDynamicPath, ValidationItemPath,
    path::{validation_case_in, validation_item_paths_in, validation_some_in},
    schema::SchemaVisitor,
    submit::FormVersion,
    topology::{CanonicalAddress, SessionId, TopologyIndex, TopologySnapshot},
};

mod report;
mod transition;
mod trigger;

pub(crate) use transition::{
    AsyncValidationRuntime, Effect as AsyncValidationEffect, Message as AsyncValidationMessage,
};
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
    Control,
    Async(Cow<'static, str>),
    Internal,
}

#[derive(Clone)]
pub struct ValidationIssue {
    pub(crate) path: PathKey,
    pub(crate) address: CanonicalAddress,
    pub(crate) source: ValidationSource,
    pub(crate) trigger: ValidationTrigger,
    pub(crate) code: Cow<'static, str>,
    pub(crate) message: ValidationMessage,
    pub(crate) control: Option<u64>,
    pub(crate) async_generation: Option<u64>,
    pub(crate) control_active: Option<Weak<AtomicBool>>,
}

impl fmt::Debug for ValidationIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidationIssue")
            .field("path", &self.path)
            .field("source", &self.source)
            .field("trigger", &self.trigger)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ValidationIssue {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.address == other.address
            && self.source == other.source
            && self.trigger == other.trigger
            && self.code == other.code
            && self.message == other.message
            && self.control == other.control
            && self.async_generation == other.async_generation
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
                ValidationSource::Control | ValidationSource::Async(_)
            ) {
                return true;
            }
            let same_bucket = issue.trigger == trigger
                && scope.is_none_or(|scope| intersects(scope, &issue.address));
            let superseded = replacements.iter().any(|(address, source, code, trigger)| {
                address == &issue.address
                    && source == &issue.source
                    && code == &issue.code
                    && *trigger == issue.trigger
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
                    ValidationSource::Control | ValidationSource::Async(_)
                ) || !scopes.iter().any(|scope| intersects(scope, &issue.address)))
        });
    }

    pub(crate) fn remove_control_intersecting(&mut self, address: &CanonicalAddress) -> bool {
        let before = self.issues.len();
        self.issues.retain(|issue| {
            !matches!(issue.source, ValidationSource::Control)
                || !intersects(address, &issue.address)
        });
        self.issues.len() != before
    }

    pub(crate) fn retain_current(&mut self, topology: &TopologyIndex) {
        self.issues.retain(|issue| {
            issue.is_active() && topology.key(&issue.address).as_ref() == Some(&issue.path)
        });
    }

    pub(crate) fn replace_control(&mut self, control: u64, issue: Option<ValidationIssue>) -> bool {
        let before = self.issues.len();
        self.issues
            .retain(|candidate| candidate.control != Some(control));
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
        self.issues
            .retain(|candidate| candidate.async_generation != Some(generation));
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
            candidate
                .async_generation
                .is_none_or(|generation| !generations.contains(&generation))
        });
    }

    pub(crate) fn remove_async_intersecting(&mut self, address: &CanonicalAddress) {
        self.issues.retain(|candidate| {
            !matches!(candidate.source, ValidationSource::Async(_))
                || !intersects(address, &candidate.address)
        });
    }
}

pub(crate) fn intersects(left: &CanonicalAddress, right: &CanonicalAddress) -> bool {
    left.is_prefix_of(right) || right.is_prefix_of(left)
}

fn snapshot_key(topology: &TopologySnapshot, address: &CanonicalAddress) -> PathKey {
    topology
        .key(address)
        .expect("validation topology must be materialized before snapshot")
}

pub trait Validator<M: FormSchema>: 'static {
    fn validate(&self, request: ValidationRequest<'_, M>, out: &mut ValidationSink<'_, M>);
}

impl<M: FormSchema, F> Validator<M> for F
where
    F: for<'a, 'b> Fn(ValidationRequest<'a, M>, &mut ValidationSink<'b, M>) + 'static,
{
    fn validate(&self, request: ValidationRequest<'_, M>, out: &mut ValidationSink<'_, M>) {
        self(request, out);
    }
}

pub struct ValidationRequest<'a, M: FormSchema> {
    model: &'a M,
    version: FormVersion,
    trigger: ValidationTrigger,
    scope: Option<&'a CanonicalAddress>,
    session: SessionId,
    pub(crate) topology: TopologySnapshot,
    marker: PhantomData<fn() -> M>,
}

impl<M: FormSchema> Clone for ValidationRequest<'_, M> {
    fn clone(&self) -> Self {
        Self {
            model: self.model,
            version: self.version,
            trigger: self.trigger,
            scope: self.scope,
            session: self.session,
            topology: self.topology.clone(),
            marker: PhantomData,
        }
    }
}

impl<'a, M: FormSchema> ValidationRequest<'a, M> {
    /// Returns the immutable model belonging to this validation snapshot.
    pub fn model(&self) -> &'a M {
        self.model
    }

    pub fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    pub fn includes<P: ValidationPath<M>>(&self, path: &P) -> bool {
        path.__included(self)
    }

    /// Enumerates a statically located collection against this validation snapshot.
    pub fn items<Item: FormSchema>(
        &self,
        path: &TotalItemsPath<M, Item>,
    ) -> Vec<ValidationItemPath<'a, M, Item>> {
        validation_item_paths_in(&path.core, self.model, &self.topology)
            .expect("a total validation collection must always resolve")
    }

    /// Enumerates a dynamically located collection against this validation snapshot.
    pub fn try_items<Item: FormSchema>(
        &self,
        path: &ValidationDynamicItemsPath<'a, M, Item>,
    ) -> Result<Vec<ValidationItemPath<'a, M, Item>>, ResolveError> {
        self.check_dynamic_core(&path.core)?;
        validation_item_paths_in(&path.core, self.model, &self.topology)
    }

    /// Resolves a dynamic value without consulting a newer live Form session.
    pub fn get<T: 'static>(&self, path: &crate::TotalPath<M, T>) -> &'a T {
        path.core
            .access
            .get(self.model, &self.topology)
            .expect("a total validation path must always resolve")
    }

    pub fn try_get<T: 'static>(
        &self,
        path: &ValidationDynamicPath<'a, M, T>,
    ) -> Result<&'a T, ResolveError> {
        self.check_dynamic_core(&path.core)?;
        path.core.access.get(self.model, &self.topology)
    }

    /// Resolves an enum payload and binds the returned path to this snapshot's incarnation.
    pub(crate) fn try_case<Enum: FormSchema, Payload: FormSchema>(
        &self,
        path: ValidationDynamicPath<'a, M, Enum>,
        case: CaseDef<Enum, Payload>,
    ) -> Result<Option<ValidationDynamicPath<'a, M, Payload>>, ResolveError> {
        self.check_dynamic_core(&path.core)?;
        validation_case_in(path.core, self.model, &self.topology, case)
    }

    /// Resolves an enum payload from a total path in this snapshot.
    pub fn case<Enum, Payload, Path>(
        &self,
        path: Path,
        case: CaseDef<Enum, Payload>,
    ) -> Option<ValidationDynamicPath<'a, M, Payload>>
    where
        Enum: FormSchema,
        Payload: FormSchema,
        Path: IntoTotalPath<M, Enum>,
    {
        validation_case_in(
            path.into_total_path().core,
            self.model,
            &self.topology,
            case,
        )
        .expect("a total validation path must always resolve")
    }

    /// Resolves an optional payload and binds the returned path to this snapshot's incarnation.
    pub(crate) fn try_some<T: FormSchema>(
        &self,
        path: ValidationDynamicPath<'a, M, Option<T>>,
    ) -> Result<Option<ValidationDynamicPath<'a, M, T>>, ResolveError> {
        self.check_dynamic_core(&path.core)?;
        validation_some_in(path.core, self.model, &self.topology)
    }

    /// Resolves an optional payload from a total path in this snapshot.
    pub fn some<T, Path>(&self, path: Path) -> Option<ValidationDynamicPath<'a, M, T>>
    where
        T: FormSchema,
        Path: IntoTotalPath<M, Option<T>>,
    {
        validation_some_in(path.into_total_path().core, self.model, &self.topology)
            .expect("a total validation path must always resolve")
    }

    fn check_dynamic_core<T: 'static>(
        &self,
        core: &crate::PathCore<M, T>,
    ) -> Result<(), ResolveError> {
        if core.session.is_some_and(|session| session != self.session) {
            return Err(ResolveError::WrongSession {
                path: core
                    .identity
                    .clone()
                    .or_else(|| self.topology.key(&core.address))
                    .expect("validation paths must have a materialized identity"),
            });
        }
        if let Some(guard) = core
            .guards
            .iter()
            .find(|guard| self.topology.incarnation(&guard.address) != Some(guard.incarnation))
        {
            return Err(ResolveError::Retired {
                path: guard.key.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn includes_address(&self, address: &CanonicalAddress) -> bool {
        self.scope.is_none_or(|scope| intersects(scope, address))
    }

    pub(crate) fn version(&self) -> FormVersion {
        self.version
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
    topology: TopologySnapshot,
    trigger: ValidationTrigger,
    source: ValidationSource,
    issues: Vec<ValidationIssue>,
    marker: PhantomData<&'a M>,
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
        let path = snapshot_key(&self.topology, &address);
        self.issues.push(ValidationIssue {
            path,
            address,
            source: self.source.clone(),
            trigger: self.trigger,
            code,
            message,
            control: None,
            async_generation: None,
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
        let path = snapshot_key(&self.topology, &address);
        self.issues.push(ValidationIssue {
            path,
            address,
            source,
            trigger: self.trigger,
            code: code.into(),
            message,
            control: None,
            async_generation: None,
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

pub(crate) fn validate<'a, M: FormSchema>(
    model: &'a M,
    topology: &'a TopologyIndex,
    version: FormVersion,
    trigger: ValidationTrigger,
    scope: Option<&CanonicalAddress>,
    validator: Option<&dyn Validator<M>>,
) -> Vec<ValidationIssue> {
    let model = model.clone();
    let model = &model;
    let snapshot = topology.snapshot();
    let mut required = RequiredVisitor {
        topology: &snapshot,
        trigger,
        scope,
        address: CanonicalAddress::default(),
        issues: Vec::new(),
        validator_enabled: false,
    };
    model.__visit(&mut required);
    let validator_enabled = matches!(
        trigger,
        ValidationTrigger::Submit | ValidationTrigger::External
    ) || required.validator_enabled;
    let mut issues = required.issues;

    if validator_enabled && let Some(validator) = validator {
        let request = ValidationRequest {
            model,
            version,
            trigger,
            scope,
            session: topology.session(),
            topology: snapshot.clone(),
            marker: PhantomData,
        };
        debug_assert_eq!(request.version(), version);
        let mut sink = ValidationSink {
            topology: snapshot.clone(),
            trigger,
            source: ValidationSource::Validator(Cow::Borrowed("validator")),
            issues: Vec::new(),
            marker: PhantomData,
        };
        validator.validate(request, &mut sink);
        issues.extend(sink.issues);
    }

    issues
}

struct RequiredVisitor<'a> {
    topology: &'a TopologySnapshot,
    trigger: ValidationTrigger,
    scope: Option<&'a CanonicalAddress>,
    address: CanonicalAddress,
    issues: Vec<ValidationIssue>,
    validator_enabled: bool,
}

impl RequiredVisitor<'_> {
    fn nested(&self, address: CanonicalAddress) -> Self {
        Self {
            topology: self.topology,
            trigger: self.trigger,
            scope: self.scope,
            address,
            issues: Vec::new(),
            validator_enabled: false,
        }
    }

    fn absorb(&mut self, nested: Self) {
        self.issues.extend(nested.issues);
        self.validator_enabled |= nested.validator_enabled;
    }
}

impl SchemaVisitor for RequiredVisitor<'_> {
    fn field(&mut self, schema: FieldSchema, missing: bool) {
        let address = self.address.field(schema.name());
        if schema.triggers().includes(self.trigger)
            && self.scope.is_none_or(|scope| intersects(scope, &address))
        {
            self.validator_enabled = true;
        }
        if !missing
            || !schema.is_required()
            || !schema.triggers().includes(self.trigger)
            || !self.scope.is_none_or(|scope| intersects(scope, &address))
        {
            return;
        }
        let path = snapshot_key(self.topology, &address);
        self.issues.push(ValidationIssue {
            path,
            address,
            source: ValidationSource::Required,
            trigger: self.trigger,
            code: Cow::Borrowed("required"),
            message: ValidationMessage::key("gpui-form-error-required"),
            control: None,
            async_generation: None,
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
        let parent = self.address.field(name);
        let occurrence = self
            .topology
            .active_some(&parent)
            .expect("present optional topology must be materialized");
        let address = parent.some_occurrence(occurrence);
        snapshot_key(self.topology, &address);
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
            .items(&collection)
            .expect("validation topology must be materialized before snapshot");
        debug_assert_eq!(tokens.len(), len);
        for (index, token) in tokens.into_iter().enumerate() {
            let address = collection.item(token);
            snapshot_key(self.topology, &address);
            let mut nested = self.nested(address);
            visit(index, &mut nested);
            self.absorb(nested);
        }
    }

    fn case(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let occurrence = self
            .topology
            .active_case(&self.address, name)
            .expect("active case topology must be materialized");
        let address = self.address.case_occurrence(name, occurrence);
        snapshot_key(self.topology, &address);
        let mut nested = self.nested(address);
        visit(&mut nested);
        self.absorb(nested);
    }

    fn unit_case(&mut self, name: &'static str) {
        let occurrence = self
            .topology
            .active_case(&self.address, name)
            .expect("active unit case topology must be materialized");
        snapshot_key(
            self.topology,
            &self.address.case_occurrence(name, occurrence),
        );
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} validation issue(s)", self.issues.len())
    }
}
