mod transition;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Weak, atomic::AtomicBool},
};

use gpui::{Context, EventEmitter, Task};
use gpui_operation::Transition as _;

use crate::{
    DynamicPath, FormBuildError, FormSchema, IntoTotalPath, PathCore, PathKey, PrepareError,
    Prepared, ResolveError, ValidationIssue, ValidationMessage, ValidationReport, ValidationSource,
    ValidationTrigger, Validator,
    topology::{CanonicalAddress, Incarnation, SessionId, TopologyEpoch, TopologyIndex},
    validation,
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

#[derive(Clone, Debug, PartialEq)]
pub enum FormEvent {
    Committed {
        path: PathKey,
        revision: FormRevision,
    },
    ModelReplaced {
        revision: FormRevision,
    },
    ValidationChanged {
        revision: FormRevision,
    },
}

pub struct Form<M: FormSchema> {
    value: M,
    baseline: M,
    runtime: Runtime,
    topology: TopologyIndex,
    validator: Option<Box<dyn Validator<M>>>,
    validation: ValidationReport,
    async_validation: HashMap<u64, AsyncValidationTask>,
    next_async_generation: u64,
    controls: HashMap<u64, ControlRegistration>,
}

struct ControlRegistration {
    address: CanonicalAddress,
    incarnation: Incarnation,
    epoch: TopologyEpoch,
    active: Weak<AtomicBool>,
}

struct AsyncValidationTask {
    address: CanonicalAddress,
    _task: Task<()>,
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

impl<M: FormSchema> EventEmitter<FormEvent> for Form<M> {}

impl<M: FormSchema> Form<M> {
    pub fn try_new(value: M) -> Result<Self, FormBuildError> {
        Self::build(value, None)
    }

    pub fn try_new_with_validator<V>(value: M, validator: V) -> Result<Self, FormBuildError>
    where
        V: Validator<M>,
    {
        Self::build(value, Some(Box::new(validator)))
    }

    fn build(value: M, validator: Option<Box<dyn Validator<M>>>) -> Result<Self, FormBuildError> {
        let topology = TopologyIndex::new()?;
        let validation = ValidationReport {
            issues: validation::validate(
                &value,
                &topology,
                ValidationTrigger::Mount,
                None,
                validator.as_deref(),
            ),
        };
        Ok(Self {
            baseline: value.clone(),
            value,
            runtime: Runtime::new(),
            topology,
            validator,
            validation,
            async_validation: HashMap::new(),
            next_async_generation: 1,
            controls: HashMap::new(),
        })
    }

    pub fn value(&self) -> &M {
        &self.value
    }

    pub fn baseline(&self) -> &M {
        &self.baseline
    }

    pub fn revision(&self) -> FormRevision {
        self.runtime.revision()
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
        !self.async_validation.is_empty()
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
        self.validator = Some(Box::new(validator));
        self.validation.issues.retain(|issue| {
            matches!(
                issue.source(),
                ValidationSource::Required | ValidationSource::Control(_)
            )
        });
        self.cancel_all_async_validation();
        let effect = self.runtime.transition(Message::ValidationChanged);
        self.publish(effect, cx);
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
        if !self.async_validation.is_empty() {
            return Err(PrepareError::ValidationPending);
        }
        Ok(Prepared::new(self.revision(), self.value.clone()))
    }

    pub fn replace(&mut self, value: M, cx: &mut Context<Self>) {
        self.install_model(value, false, cx);
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.install_model(self.baseline.clone(), false, cx);
    }

    pub fn rebase(&mut self, value: M, cx: &mut Context<Self>) {
        self.install_model(value, true, cx);
    }

    pub fn rebase_if_revision(
        &mut self,
        expected: FormRevision,
        value: M,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.revision() != expected {
            return false;
        }
        self.install_model(value, true, cx);
        true
    }

    fn install_model(&mut self, value: M, rebase: bool, cx: &mut Context<Self>) {
        if rebase {
            self.baseline = value.clone();
        }
        self.value = value;
        self.topology.reset();
        self.retire_stale_controls();
        self.cancel_all_async_validation();
        self.validation = ValidationReport {
            issues: validation::validate(
                &self.value,
                &self.topology,
                ValidationTrigger::Mount,
                None,
                self.validator.as_deref(),
            ),
        };
        let effect = self.runtime.transition(Message::ReplaceModel);
        self.publish(effect, cx);
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
        self.publish_value(address, true, cx);
    }

    pub(crate) fn commit_topology(&mut self, address: CanonicalAddress, cx: &mut Context<Self>) {
        self.publish_values(address.clone(), vec![address], false, cx);
    }

    fn publish_value(
        &mut self,
        address: CanonicalAddress,
        retire_below: bool,
        cx: &mut Context<Self>,
    ) {
        self.publish_values(address.clone(), vec![address], retire_below, cx);
    }

    pub(crate) fn commit_topology_scopes(
        &mut self,
        primary: CanonicalAddress,
        scopes: Vec<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        self.publish_values(primary, scopes, false, cx);
    }

    fn publish_values(
        &mut self,
        primary: CanonicalAddress,
        scopes: Vec<CanonicalAddress>,
        retire_below: bool,
        cx: &mut Context<Self>,
    ) {
        for address in &scopes {
            self.cancel_async_intersecting(address);
        }
        if retire_below {
            self.topology.retire_below(&primary);
        }
        self.retire_stale_controls();
        self.validation.retain_current(&self.topology);
        self.validation.invalidate_sync(&scopes);
        for address in &scopes {
            let next = validation::validate(
                &self.value,
                &self.topology,
                ValidationTrigger::Change,
                Some(address),
                self.validator.as_deref(),
            );
            self.validation
                .replace_scope(Some(address), ValidationTrigger::Change, next);
        }
        let incarnation = self
            .topology
            .ensure_incarnation(&primary)
            .expect("form identity exhausted after construction");
        let path = PathKey::new(self.session(), &primary, incarnation);
        let effect = self.runtime.transition(Message::Commit(path));
        self.publish(effect, cx);
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
            trigger,
            address.as_ref(),
            self.validator.as_deref(),
        );
        let before = self.validation.clone();
        self.validation
            .replace_scope(address.as_ref(), trigger, next);
        if self.validation != before {
            let effect = self.runtime.transition(Message::ValidationChanged);
            self.publish(effect, cx);
        }
    }

    pub(crate) fn errors_at(&self, address: &CanonicalAddress) -> Vec<ValidationIssue> {
        self.validation.errors_at(address)
    }

    pub(crate) fn set_control_issue(
        &mut self,
        control: u64,
        address: CanonicalAddress,
        issue: Option<(String, ValidationMessage)>,
        cx: &mut Context<Self>,
    ) {
        let control_active = self
            .controls
            .get(&control)
            .map(|registration| registration.active.clone());
        let issue = issue.map(|(code, message)| {
            let incarnation = self
                .topology
                .ensure_incarnation(&address)
                .expect("form identity exhausted after construction");
            ValidationIssue {
                path: PathKey::new(self.session(), &address, incarnation),
                address,
                source: ValidationSource::Control(control),
                trigger: ValidationTrigger::Change,
                code: code.into(),
                message,
                control_active,
            }
        });
        if self.validation.replace_control(control, issue) {
            let effect = self.runtime.transition(Message::ValidationChanged);
            self.publish(effect, cx);
        }
    }

    pub(crate) fn register_control(
        &mut self,
        control: u64,
        address: CanonicalAddress,
        incarnation: Incarnation,
        epoch: TopologyEpoch,
        active: Weak<AtomicBool>,
    ) {
        self.controls.insert(
            control,
            ControlRegistration {
                address,
                incarnation,
                epoch,
                active,
            },
        );
    }

    fn retire_stale_controls(&mut self) {
        let epoch = self.topology.epoch();
        self.controls.retain(|_, registration| {
            let Some(active) = registration.active.upgrade() else {
                return false;
            };
            let current = registration.epoch == epoch
                && self.topology.incarnation(&registration.address)
                    == Some(registration.incarnation);
            if !current {
                active.store(false, std::sync::atomic::Ordering::Release);
            }
            current && active.load(std::sync::atomic::Ordering::Acquire)
        });
        self.validation.issues.retain(ValidationIssue::is_active);
    }

    fn publish(&mut self, effect: Effect, cx: &mut Context<Self>) {
        let event = match effect {
            Effect::Committed(event) | Effect::Validation(event) => event,
        };
        cx.emit(event);
        cx.notify();
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
        self.cancel_async_intersecting(&address);
        let generation = self.next_async_generation;
        self.next_async_generation = self
            .next_async_generation
            .checked_add(1)
            .expect("async validation generation space exhausted");
        let revision = self.revision();
        let epoch = self.topology.epoch();
        let incarnation = self
            .topology
            .ensure_incarnation(&address)
            .expect("form identity exhausted after construction");
        let weak_form = cx.entity().downgrade();
        let completion_address = address.clone();
        let completion_source = source.clone();
        let task = cx.spawn(async move |_, cx| {
            let result = check(value).await;
            let _ = weak_form.update(cx, |form, cx| {
                form.complete_async_validation(
                    generation,
                    revision,
                    epoch,
                    incarnation,
                    completion_address,
                    completion_source,
                    result.err(),
                    cx,
                );
            });
        });
        self.async_validation.insert(
            generation,
            AsyncValidationTask {
                address,
                _task: task,
            },
        );
        let effect = self.runtime.transition(Message::ValidationChanged);
        self.publish(effect, cx);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_async_validation(
        &mut self,
        generation: u64,
        revision: FormRevision,
        epoch: TopologyEpoch,
        incarnation: Incarnation,
        address: CanonicalAddress,
        source: Cow<'static, str>,
        issue: Option<AsyncValidationIssue>,
        cx: &mut Context<Self>,
    ) {
        let Some(_) = self.async_validation.remove(&generation) else {
            return;
        };
        let current = self.revision() == revision
            && self.topology.epoch() == epoch
            && self.topology.incarnation(&address) == Some(incarnation);
        if current {
            let issue = issue.map(|issue| ValidationIssue {
                path: PathKey::new(self.session(), &address, incarnation),
                address,
                source: ValidationSource::Async { source, generation },
                trigger: ValidationTrigger::Dynamic,
                code: issue.code,
                message: issue.message,
                control_active: None,
            });
            self.validation.replace_async(generation, issue);
            let effect = self.runtime.transition(Message::ValidationChanged);
            self.publish(effect, cx);
        }
    }

    fn cancel_async_intersecting(&mut self, address: &CanonicalAddress) {
        let generations = self
            .async_validation
            .iter()
            .filter_map(|(generation, task)| {
                validation::intersects(address, &task.address).then_some(*generation)
            })
            .collect::<HashSet<_>>();
        self.async_validation
            .retain(|generation, _| !generations.contains(generation));
        self.validation.remove_async_generations(&generations);
        self.validation.remove_async_intersecting(address);
    }

    fn cancel_all_async_validation(&mut self) {
        let generations = self
            .async_validation
            .keys()
            .copied()
            .collect::<HashSet<_>>();
        self.async_validation.clear();
        self.validation.remove_async_generations(&generations);
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{AppContext as _, Task, TestAppContext};

    use super::*;
    use crate::schema::SchemaVisitor;

    #[derive(Clone, PartialEq)]
    struct Empty;

    impl FormSchema for Empty {
        fn __visit(&self, _visitor: &mut dyn SchemaVisitor) {}
    }

    #[gpui::test]
    fn stale_async_completion_is_not_published(cx: &mut TestAppContext) {
        let events = Rc::new(Cell::new(0));
        let form = cx.update(|cx| {
            let form = cx.new(|_| Form::try_new(Empty).unwrap());
            let count = events.clone();
            cx.subscribe(&form, move |_, _: &FormEvent, _| {
                count.set(count.get() + 1);
            })
            .detach();
            form
        });
        cx.update(|cx| {
            form.update(cx, |form, cx| {
                let address = CanonicalAddress::default();
                let incarnation = form.topology.ensure_incarnation(&address).unwrap();
                form.async_validation.insert(
                    1,
                    AsyncValidationTask {
                        address: address.clone(),
                        _task: Task::ready(()),
                    },
                );
                form.complete_async_validation(
                    1,
                    FormRevision(99),
                    form.topology.epoch(),
                    incarnation,
                    address,
                    Cow::Borrowed("stale"),
                    Some(AsyncValidationIssue::new(
                        "stale",
                        ValidationMessage::literal("stale"),
                    )),
                    cx,
                );
            });
        });
        cx.run_until_parked();
        assert_eq!(events.get(), 0);
        cx.update(|cx| {
            assert!(!form.read(cx).is_validating());
            assert!(form.read(cx).validation_report().is_valid());
        });
    }
}
