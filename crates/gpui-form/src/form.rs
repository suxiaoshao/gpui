mod transition;

use std::{
    borrow::Cow,
    collections::HashSet,
    future::Future,
    sync::{Weak, atomic::AtomicBool},
};

use gpui::{Context, EventEmitter};
use gpui_operation::Transition as _;

use crate::{
    DynamicPath, FormSchema, FormVersion, IntoTotalPath, ModelChange, ModelChangeKind, PathCore,
    PathKey, PrepareError, Prepared, ResolveError, ValidationIssue, ValidationMessage,
    ValidationReport, ValidationSource, ValidationTrigger, Validator,
    change::{ChangeSet, ControlOrigin},
    schema::SchemaVisitor,
    topology::{CanonicalAddress, SessionId, TopologyEdit, TopologyIndex, root_address},
    validation::{self, AsyncValidationEffect, AsyncValidationMessage, AsyncValidationRuntime},
};

use self::transition::{Effect, Message, Runtime};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormRevision(pub(crate) u64);

impl FormRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormEvent<M: FormSchema> {
    ModelChanged(ModelChange<M>),
    ValidationChanged { revision: FormRevision },
}

pub struct Form<M: FormSchema> {
    value: M,
    baseline: M,
    runtime: Runtime,
    topology: TopologyIndex,
    validator: Option<Box<dyn Validator<M>>>,
    validation: ValidationReport,
    async_validation: AsyncValidationRuntime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsyncValidationIssue {
    code: Cow<'static, str>,
    message: ValidationMessage,
}

impl AsyncValidationIssue {
    pub fn new(code: impl Into<Cow<'static, str>>, message: impl Into<ValidationMessage>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl<M: FormSchema> EventEmitter<FormEvent<M>> for Form<M> {}

impl<M: FormSchema> Form<M> {
    pub fn new(value: M) -> Self {
        let topology = TopologyIndex::new();
        materialize_model(&value, &topology);
        let runtime = Runtime::new();
        let version = FormVersion::new(topology.session(), runtime.revision());
        let validation = ValidationReport {
            issues: validation::validate(
                &value,
                &topology,
                version,
                ValidationTrigger::Mount,
                None,
                None,
            ),
        };
        Self {
            baseline: value.clone(),
            value,
            runtime,
            topology,
            validator: None,
            validation,
            async_validation: AsyncValidationRuntime::new(),
        }
    }

    pub fn with_validator<V: Validator<M>>(mut self, validator: V) -> Self {
        self.validator = Some(Box::new(validator));
        self.validation = ValidationReport {
            issues: validation::validate(
                &self.value,
                &self.topology,
                self.version(),
                ValidationTrigger::Mount,
                None,
                self.validator.as_deref(),
            ),
        };
        self
    }

    pub(crate) fn value(&self) -> &M {
        &self.value
    }

    pub fn revision(&self) -> FormRevision {
        self.runtime.revision()
    }

    pub(crate) fn version(&self) -> FormVersion {
        FormVersion::new(self.session(), self.revision())
    }

    pub fn is_dirty(&self) -> bool {
        self.value != self.baseline
    }

    pub fn validation_report(&self) -> ValidationReport {
        let mut report = self.validation.clone();
        report.issues.retain(ValidationIssue::is_active);
        report
    }

    pub fn is_valid(&self) -> bool {
        self.validation.is_valid()
    }

    pub fn is_validating(&self) -> bool {
        self.async_validation.is_pending()
    }

    pub fn first_error_path(&self) -> Option<PathKey> {
        self.validation
            .issues()
            .iter()
            .find(|issue| issue.is_active())
            .map(|issue| issue.path().clone())
    }

    pub fn replace_validator<V>(&mut self, validator: V, cx: &mut Context<Self>)
    where
        V: Validator<M>,
    {
        let before = self.validation.clone();
        let was_validating = self.async_validation.is_pending();
        self.validator = Some(Box::new(validator));
        self.validation.issues.retain(|issue| {
            matches!(
                issue.source(),
                ValidationSource::Required | ValidationSource::Control
            )
        });
        self.cancel_all_async_validation();
        let next = validation::validate(
            &self.value,
            &self.topology,
            self.version(),
            ValidationTrigger::External,
            None,
            self.validator.as_deref(),
        );
        self.validation
            .replace_scope(None, ValidationTrigger::External, next);
        if self.validation != before || was_validating != self.async_validation.is_pending() {
            self.publish_validation(cx);
        }
    }

    pub fn validate(&mut self, trigger: ValidationTrigger, cx: &mut Context<Self>) {
        self.validate_at(trigger, None, cx);
    }

    pub fn prepare(&mut self, cx: &mut Context<Self>) -> Result<Prepared<M>, PrepareError> {
        self.validate_at(ValidationTrigger::Submit, None, cx);
        self.validation.issues.retain(ValidationIssue::is_active);
        if !self.validation.is_valid() {
            return Err(PrepareError::Validation(self.validation.clone()));
        }
        if self.async_validation.is_pending() {
            return Err(PrepareError::ValidationPending);
        }
        Ok(Prepared::new(self.version(), self.value.clone()))
    }

    pub fn replace(&mut self, value: M, cx: &mut Context<Self>) {
        self.install_model(value, false, ModelChangeKind::Replace, cx);
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.install_model(self.baseline.clone(), false, ModelChangeKind::Reset, cx);
    }

    pub fn rebase(&mut self, value: M, cx: &mut Context<Self>) {
        self.install_model(value, true, ModelChangeKind::Rebase, cx);
    }

    pub fn rebase_if_current(
        &mut self,
        version: FormVersion,
        value: M,
        cx: &mut Context<Self>,
    ) -> bool {
        if !version.is_current(self.session(), self.revision()) {
            return false;
        }
        self.install_model(value, true, ModelChangeKind::Rebase, cx);
        true
    }

    fn install_model(
        &mut self,
        value: M,
        rebase: bool,
        kind: ModelChangeKind,
        cx: &mut Context<Self>,
    ) {
        let retired = self.topology.dynamic_addresses();
        if rebase {
            self.baseline = value.clone();
        }
        self.value = value;
        self.topology.reset();
        materialize_model(&self.value, &self.topology);
        self.cancel_all_async_validation();
        self.validation = ValidationReport {
            issues: validation::validate(
                &self.value,
                &self.topology,
                self.next_version(),
                ValidationTrigger::Mount,
                None,
                self.validator.as_deref(),
            ),
        };
        self.publish_model(kind, ChangeSet::whole_model(retired), None, cx);
    }

    pub(crate) fn session(&self) -> SessionId {
        self.topology.session()
    }

    pub(crate) fn topology(&self) -> &TopologyIndex {
        &self.topology
    }

    pub(crate) fn model_and_topology(&mut self) -> (&mut M, &TopologyIndex) {
        (&mut self.value, &self.topology)
    }

    pub(crate) fn commit_value(&mut self, address: CanonicalAddress, cx: &mut Context<Self>) {
        self.commit_value_from(address, None, cx);
    }

    pub(crate) fn commit_control_value(
        &mut self,
        address: CanonicalAddress,
        origin: ControlOrigin,
        cx: &mut Context<Self>,
    ) {
        self.commit_value_from(address, Some(origin), cx);
    }

    fn commit_value_from(
        &mut self,
        address: CanonicalAddress,
        origin: Option<ControlOrigin>,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_async_for_model_change(std::slice::from_ref(&address));
        self.validation.remove_control_intersecting(&address);
        let before = self.topology.dynamic_addresses();
        let mut edit = self.topology.stage();
        edit.retire_descendants(&address);
        materialize_model_in(&self.value, &mut edit);
        self.topology.commit(edit);
        let after = self.topology.dynamic_addresses();
        let retired = difference(&before, &after);
        let structure_changed =
            before.iter().collect::<HashSet<_>>() != after.iter().collect::<HashSet<_>>();
        self.refresh_validation(std::slice::from_ref(&address));
        self.publish_model(
            ModelChangeKind::Edit,
            ChangeSet::subtree(address, structure_changed, retired),
            origin,
            cx,
        );
    }

    pub(crate) fn commit_topology(&mut self, address: CanonicalAddress, cx: &mut Context<Self>) {
        self.commit_topology_change(address.clone(), vec![address], Vec::new(), cx);
    }

    pub(crate) fn commit_topology_retiring(
        &mut self,
        address: CanonicalAddress,
        retired: Vec<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        self.commit_topology_change(address.clone(), vec![address], retired, cx);
    }

    pub(crate) fn commit_topology_scopes_retiring(
        &mut self,
        primary: CanonicalAddress,
        scopes: Vec<CanonicalAddress>,
        retired: Vec<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        self.commit_topology_change(primary, scopes, retired, cx);
    }

    fn commit_topology_change(
        &mut self,
        _primary: CanonicalAddress,
        scopes: Vec<CanonicalAddress>,
        retired: Vec<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_async_for_model_change(&scopes);
        let mut edit = self.topology.stage();
        materialize_model_in(&self.value, &mut edit);
        self.topology.commit(edit);
        self.refresh_validation(&scopes);
        self.publish_model(
            ModelChangeKind::Edit,
            ChangeSet::aggregate(scopes, retired),
            None,
            cx,
        );
    }

    fn refresh_validation(&mut self, scopes: &[CanonicalAddress]) {
        self.validation.retain_current(&self.topology);
        self.validation.invalidate_sync(scopes);
        for address in scopes {
            let next = validation::validate(
                &self.value,
                &self.topology,
                self.next_version(),
                ValidationTrigger::Change,
                Some(address),
                self.validator.as_deref(),
            );
            self.validation
                .replace_scope(Some(address), ValidationTrigger::Change, next);
        }
    }

    pub(crate) fn validate_at(
        &mut self,
        trigger: ValidationTrigger,
        address: Option<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        let next = validation::validate(
            &self.value,
            &self.topology,
            self.version(),
            trigger,
            address.as_ref(),
            self.validator.as_deref(),
        );
        let before = self.validation.clone();
        self.validation
            .replace_scope(address.as_ref(), trigger, next);
        if self.validation != before {
            self.publish_validation(cx);
        }
    }

    pub(crate) fn errors_at(&self, address: &CanonicalAddress) -> Vec<ValidationIssue> {
        self.validation.errors_at(address)
    }

    pub(crate) fn set_control_issue(
        &mut self,
        control: u64,
        address: CanonicalAddress,
        active: Weak<AtomicBool>,
        issue: Option<(String, ValidationMessage)>,
        cx: &mut Context<Self>,
    ) {
        let issue = issue.map(|(code, message)| ValidationIssue {
            path: self
                .topology
                .key(&address)
                .expect("a bound path identity must remain materialized while active"),
            address,
            source: ValidationSource::Control,
            trigger: ValidationTrigger::Change,
            code: code.into(),
            message,
            control: Some(control),
            async_generation: None,
            control_active: Some(active),
        });
        if self.validation.replace_control(control, issue) {
            self.publish_validation(cx);
        }
    }

    pub(crate) fn complete_control_write(
        &mut self,
        control: u64,
        address: CanonicalAddress,
        changed: bool,
        origin: ControlOrigin,
        cx: &mut Context<Self>,
    ) {
        let issue_changed = self.validation.replace_control(control, None);
        if changed {
            self.commit_control_value(address, origin, cx);
        } else if issue_changed {
            self.publish_validation(cx);
        }
    }

    pub(crate) fn clear_control_issue(&mut self, control: u64, cx: &mut Context<Self>) {
        if self.validation.replace_control(control, None) {
            self.publish_validation(cx);
        }
    }

    fn publish_model(
        &mut self,
        kind: ModelChangeKind,
        changes: ChangeSet,
        origin: Option<ControlOrigin>,
        cx: &mut Context<Self>,
    ) {
        let effect = self.runtime.transition(Message::<M>::model_applied(
            kind,
            self.session(),
            changes,
            origin,
        ));
        self.publish(effect, cx);
    }

    fn publish_validation(&mut self, cx: &mut Context<Self>) {
        let effect = self.runtime.transition(Message::<M>::validation_changed());
        self.publish(effect, cx);
    }

    fn publish(&mut self, effect: Effect<M>, cx: &mut Context<Self>) {
        let Effect::Publish(event) = effect;
        cx.emit(event);
        cx.notify();
    }

    fn next_version(&self) -> FormVersion {
        let revision = self
            .revision()
            .0
            .checked_add(1)
            .map(FormRevision)
            .expect("form revision overflow");
        FormVersion::new(self.session(), revision)
    }

    pub fn start_async_validation<T, Path, Check, CheckFuture>(
        &mut self,
        path: Path,
        source: impl Into<Cow<'static, str>>,
        check: Check,
        cx: &mut Context<Self>,
    ) -> Result<(), ResolveError>
    where
        T: Clone + PartialEq + 'static,
        Path: IntoTotalPath<M, T>,
        Check: FnOnce(T) -> CheckFuture + 'static,
        CheckFuture: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
    {
        let core = path.into_total_path().core;
        self.start_async_core(core, source.into(), check, cx)
    }

    pub fn start_dynamic_async_validation<T, Check, CheckFuture>(
        &mut self,
        path: DynamicPath<M, T>,
        source: impl Into<Cow<'static, str>>,
        check: Check,
        cx: &mut Context<Self>,
    ) -> Result<(), ResolveError>
    where
        T: Clone + PartialEq + 'static,
        Check: FnOnce(T) -> CheckFuture + 'static,
        CheckFuture: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
    {
        path.core.check(self)?;
        self.start_async_core(path.core, source.into(), check, cx)
    }

    fn start_async_core<T, Check, CheckFuture>(
        &mut self,
        core: PathCore<M, T>,
        source: Cow<'static, str>,
        check: Check,
        cx: &mut Context<Self>,
    ) -> Result<(), ResolveError>
    where
        T: Clone + PartialEq + 'static,
        Check: FnOnce(T) -> CheckFuture + 'static,
        CheckFuture: Future<Output = Result<(), AsyncValidationIssue>> + 'static,
    {
        let value = core
            .access
            .get(&self.value, &self.topology.snapshot())?
            .clone();
        let address = core.address.clone();
        let key = self
            .topology
            .key(&address)
            .expect("async validation path must be materialized");
        self.cancel_async_intersecting(&address);
        let AsyncValidationEffect::Reserved(generation) =
            self.async_validation
                .transition(AsyncValidationMessage::Reserve {
                    address: address.clone(),
                })
        else {
            unreachable!("reserve always returns a generation")
        };
        let version = self.version();
        let weak_form = cx.entity().downgrade();
        let completion_address = address.clone();
        let completion_source = source.clone();
        let task = cx.spawn(async move |_, cx| {
            let result = check(value).await;
            let _ = weak_form.update(cx, |form, cx| {
                form.complete_async_validation(
                    generation,
                    version,
                    key,
                    completion_address,
                    completion_source,
                    result.err(),
                    cx,
                );
            });
        });
        let _ = self
            .async_validation
            .transition(AsyncValidationMessage::Attach { generation, task });
        self.publish_validation(cx);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_async_validation(
        &mut self,
        generation: u64,
        version: FormVersion,
        key: PathKey,
        address: CanonicalAddress,
        source: Cow<'static, str>,
        issue: Option<AsyncValidationIssue>,
        cx: &mut Context<Self>,
    ) {
        let current = version.is_current(self.session(), self.revision())
            && self.topology.key(&address).as_ref() == Some(&key);
        let AsyncValidationEffect::Completed { accepted } =
            self.async_validation
                .transition(AsyncValidationMessage::Complete {
                    generation,
                    fresh: current,
                })
        else {
            unreachable!("completion always returns a completion effect")
        };
        if !accepted {
            return;
        }
        let issue = issue.map(|issue| ValidationIssue {
            path: key,
            address,
            source: ValidationSource::Async(source),
            trigger: ValidationTrigger::External,
            code: issue.code,
            message: issue.message,
            control: None,
            async_generation: Some(generation),
            control_active: None,
        });
        self.validation.replace_async(generation, issue);
        self.publish_validation(cx);
    }

    fn cancel_async_intersecting(&mut self, address: &CanonicalAddress) {
        let AsyncValidationEffect::Cancelled(generations) =
            self.async_validation
                .transition(AsyncValidationMessage::CancelIntersecting {
                    address: address.clone(),
                })
        else {
            unreachable!("cancellation always returns cancelled generations")
        };
        self.validation.remove_async_generations(&generations);
        self.validation.remove_async_intersecting(address);
    }

    fn invalidate_async_for_model_change(&mut self, scopes: &[CanonicalAddress]) {
        // Every pending task is bound to one global FormVersion, so any model revision
        // invalidates its snapshot proof. Completed issues remain scope-aware facts.
        self.cancel_all_async_validation();
        for address in scopes {
            self.validation.remove_async_intersecting(address);
        }
    }

    fn cancel_all_async_validation(&mut self) {
        let AsyncValidationEffect::Cancelled(generations) = self
            .async_validation
            .transition(AsyncValidationMessage::CancelAll)
        else {
            unreachable!("cancellation always returns cancelled generations")
        };
        self.validation.remove_async_generations(&generations);
    }
}

fn difference(before: &[CanonicalAddress], after: &[CanonicalAddress]) -> Vec<CanonicalAddress> {
    before
        .iter()
        .filter(|address| !after.contains(address))
        .cloned()
        .collect()
}

fn materialize_model<M: FormSchema>(model: &M, topology: &TopologyIndex) {
    let mut edit = topology.stage();
    materialize_model_in(model, &mut edit);
    topology.commit(edit);
}

fn materialize_model_in<M: FormSchema>(model: &M, edit: &mut TopologyEdit) {
    edit.materialize_total(&root_address());
    let mut visitor = TopologyMaterializer {
        edit,
        address: root_address(),
    };
    model.__visit(&mut visitor);
}

struct TopologyMaterializer<'a> {
    edit: &'a mut TopologyEdit,
    address: CanonicalAddress,
}

impl SchemaVisitor for TopologyMaterializer<'_> {
    fn field(&mut self, schema: crate::FieldSchema, _missing: bool) {
        self.edit
            .materialize_total(&self.address.field(schema.name()));
    }

    fn child(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let address = self.address.field(name);
        self.edit.materialize_total(&address);
        let mut nested = TopologyMaterializer {
            edit: self.edit,
            address,
        };
        visit(&mut nested);
    }

    fn optional(
        &mut self,
        name: &'static str,
        present: bool,
        visit: &mut dyn FnMut(&mut dyn SchemaVisitor),
    ) {
        let parent = self.address.field(name);
        self.edit.materialize_total(&parent);
        if !present {
            self.edit.deactivate_some(&parent);
            return;
        }
        let address = self.edit.activate_some(&parent);
        let mut nested = TopologyMaterializer {
            edit: self.edit,
            address,
        };
        visit(&mut nested);
    }

    fn items(
        &mut self,
        name: &'static str,
        len: usize,
        visit: &mut dyn FnMut(usize, &mut dyn SchemaVisitor),
    ) {
        let collection = self.address.field(name);
        self.edit.materialize_total(&collection);
        let occurrences = self.edit.ensure_items(&collection, len);
        for (index, occurrence) in occurrences.into_iter().enumerate() {
            let mut nested = TopologyMaterializer {
                edit: self.edit,
                address: collection.item(occurrence),
            };
            visit(index, &mut nested);
        }
    }

    fn case(&mut self, name: &'static str, visit: &mut dyn FnMut(&mut dyn SchemaVisitor)) {
        let address = self.edit.activate_case(&self.address, name);
        let mut nested = TopologyMaterializer {
            edit: self.edit,
            address,
        };
        visit(&mut nested);
    }

    fn unit_case(&mut self, name: &'static str) {
        self.edit.activate_case(&self.address, name);
    }
}
