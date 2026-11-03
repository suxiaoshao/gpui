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

mod rules;
pub(crate) use rules::RuleOwner;
pub use rules::{RuleId, RuleRegistrationError, ValidationRule};
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
    pub(crate) owner: Option<RuleOwner>,
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
        self.owner == other.owner
            && self.path == other.path
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

    pub(crate) fn replace_owner(&mut self, owner: &RuleOwner, mut next: Vec<ValidationIssue>) {
        self.issues
            .retain(|issue| issue.owner.as_ref() != Some(owner));
        self.issues.append(&mut next);
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

    pub fn version(&self) -> FormVersion {
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
    owner: RuleOwner,
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
            owner: Some(self.owner.clone()),
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
            owner: Some(self.owner.clone()),
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate<M: FormSchema>(
    model: &M,
    topology: &TopologyIndex,
    version: FormVersion,
    trigger: ValidationTrigger,
    scopes: Option<&[CanonicalAddress]>,
    drafts: &[CanonicalAddress],
    rules: &mut [ValidationRule<M>],
    started: &mut std::collections::HashSet<PathKey>,
    report: &mut ValidationReport,
) {
    let snapshot = topology.snapshot();
    started.retain(|key| snapshot.key(key.address()).as_ref() == Some(key));
    let mut required = RequiredVisitor {
        topology: &snapshot,
        trigger,
        scopes,
        started,
        drafts,
        address: CanonicalAddress::default(),
        replacements: Vec::new(),
    };
    model.__visit(&mut required);
    for (owner, next) in required.replacements {
        report.replace_owner(&owner, next);
    }
    for rule in rules {
        let Some(owner) = rule.owner(&snapshot) else {
            continue;
        };
        if !rule.matches(scopes, trigger) {
            continue;
        }
        rule.started = true;
        if rule
            .dependencies
            .iter()
            .any(|dep| drafts.iter().any(|d| intersects(d, &dep.address)))
        {
            report.replace_owner(&owner, Vec::new());
            continue;
        }
        let request = ValidationRequest {
            model,
            version,
            trigger,
            scope: None,
            session: topology.session(),
            topology: snapshot.clone(),
            marker: PhantomData,
        };
        let mut sink = ValidationSink {
            topology: snapshot.clone(),
            trigger,
            source: ValidationSource::Validator(rule.id.as_str().to_owned().into()),
            owner: owner.clone(),
            issues: Vec::new(),
            marker: PhantomData,
        };
        (rule.check)(request, &mut sink);
        report.replace_owner(&owner, sink.issues);
    }
}

struct RequiredVisitor<'a> {
    topology: &'a TopologySnapshot,
    trigger: ValidationTrigger,
    scopes: Option<&'a [CanonicalAddress]>,
    drafts: &'a [CanonicalAddress],
    started: &'a mut std::collections::HashSet<PathKey>,
    address: CanonicalAddress,
    replacements: Vec<(RuleOwner, Vec<ValidationIssue>)>,
}

impl RequiredVisitor<'_> {
    fn visit_nested(
        &mut self,
        address: CanonicalAddress,
        visit: &mut dyn FnMut(&mut dyn SchemaVisitor),
    ) {
        let old = std::mem::replace(&mut self.address, address);
        visit(self);
        self.address = old;
    }
}

impl SchemaVisitor for RequiredVisitor<'_> {
    fn field(&mut self, schema: FieldSchema, missing: bool) {
        if !schema.is_required() {
            return;
        }
        let address = self.address.field(schema.name());
        let path = snapshot_key(self.topology, &address);
        let affected = self
            .scopes
            .is_none_or(|scopes| scopes.iter().any(|s| intersects(s, &address)));
        if !affected
            || !(schema.triggers().includes(self.trigger)
                || self.trigger == ValidationTrigger::External
                || self.started.contains(&path))
        {
            return;
        }
        self.started.insert(path.clone());
        let owner = RuleOwner::Required(path.clone());
        let next = if missing && !self.drafts.iter().any(|d| intersects(d, &address)) {
            vec![ValidationIssue {
                owner: Some(owner.clone()),
                path,
                address,
                source: ValidationSource::Required,
                trigger: self.trigger,
                code: Cow::Borrowed("required"),
                message: ValidationMessage::key("gpui-form-error-required"),
                control: None,
                async_generation: None,
                control_active: None,
            }]
        } else {
            Vec::new()
        };
        self.replacements.push((owner, next));
    }

    fn child(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        self.visit_nested(self.address.field(name), visit);
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
        self.visit_nested(address, visit);
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
            self.visit_nested(address, &mut |v| visit(index, v));
        }
    }

    fn case(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let occurrence = self
            .topology
            .active_case(&self.address, name)
            .expect("active case topology must be materialized");
        let address = self.address.case_occurrence(name, occurrence);
        snapshot_key(self.topology, &address);
        self.visit_nested(address, visit);
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
