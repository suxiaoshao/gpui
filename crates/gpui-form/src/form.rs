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
    PathKey, PrepareError, Prepared, ResolveError, RuleRegistrationError, ValidationIssue,
    ValidationMessage, ValidationReport, ValidationRule, ValidationSource, ValidationTrigger,
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
    DraftChanged(ModelChange<M>),
    SessionChanged(ModelChange<M>),
    ValidationChanged { revision: FormRevision },
}

pub struct Form<M: FormSchema> {
    value: M,
    baseline: M,
    baseline_topology: crate::topology::TopologySnapshot,
    pub(crate) drafts: crate::draft::Drafts,
    pub(crate) edit_clock: crate::draft::EditClock,
    runtime: Runtime,
    topology: TopologyIndex,
    rules: Vec<ValidationRule<M>>,
    started_required: HashSet<PathKey>,
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
        let version = FormVersion::new(topology.session(), runtime.revision(), 0);
        let mut validation = ValidationReport::default();
        let mut started_required = HashSet::new();
        validation::validate(
            &value,
            &topology,
            version,
            ValidationTrigger::Mount,
            None,
            &[],
            &mut [],
            &mut started_required,
            &mut validation,
        );
        Self {
            baseline: value.clone(),
            baseline_topology: topology.snapshot(),
            drafts: Default::default(),
            edit_clock: Default::default(),
            value,
            runtime,
            topology,
            rules: Vec::new(),
            started_required,
            validation,
            async_validation: AsyncValidationRuntime::new(),
        }
    }

    pub fn with_rules(
        mut self,
        rules: Vec<ValidationRule<M>>,
    ) -> Result<Self, RuleRegistrationError> {
        ValidationRule::check_registration(&rules, &self.topology.snapshot())?;
        self.rules = rules;
        self.run_validation(ValidationTrigger::Mount, None, self.version());
        Ok(self)
    }

    pub(crate) fn value(&self) -> &M {
        &self.value
    }

    pub fn revision(&self) -> FormRevision {
        self.runtime.revision()
    }

    pub(crate) fn version(&self) -> FormVersion {
        FormVersion::new(self.session(), self.revision(), self.edit_clock.epoch())
    }

    pub fn is_dirty(&self) -> bool {
        self.value != self.baseline || !self.drafts.is_empty()
    }

    pub fn validation_report(&self) -> ValidationReport {
        let mut report = self.validation.clone();
        report.issues.retain(ValidationIssue::is_active);
        report
    }

    pub fn is_valid(&self) -> bool {
        self.drafts.is_empty() && self.validation.is_valid()
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

    pub fn replace_rules(
        &mut self,
        rules: Vec<ValidationRule<M>>,
        cx: &mut Context<Self>,
    ) -> Result<(), RuleRegistrationError> {
        ValidationRule::check_registration(&rules, &self.topology.snapshot())?;
        let before = self.validation.clone();
        self.rules = rules;
        self.validation
            .issues
            .retain(|i| !matches!(i.owner, Some(validation::RuleOwner::Custom(..))));
        self.run_validation(ValidationTrigger::External, None, self.version());
        if self.validation != before {
            self.publish_validation(cx);
        }
        Ok(())
    }

    pub fn validate(&mut self, trigger: ValidationTrigger, cx: &mut Context<Self>) {
        self.validate_at(trigger, None, cx);
    }

    pub fn prepare(&mut self, cx: &mut Context<Self>) -> Result<Prepared<M>, PrepareError> {
        if self.edit_clock.is_pending() {
            return Err(PrepareError::EditsPending);
        }
        if !self.drafts.is_empty() {
            return Err(PrepareError::UnresolvedDrafts);
        }
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

    pub fn replace(
        &mut self,
        value: M,
        policy: crate::DraftPolicy,
        cx: &mut Context<Self>,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        self.check_lifecycle(policy)?;
        self.install_model(value, false, ModelChangeKind::Replace, policy, cx);
        Ok(self.lifecycle_report(Vec::new()))
    }
    pub fn reset(
        &mut self,
        policy: crate::DraftPolicy,
        cx: &mut Context<Self>,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        self.check_lifecycle(policy)?;
        self.install_model(
            self.baseline.clone(),
            false,
            ModelChangeKind::Reset,
            policy,
            cx,
        );
        Ok(self.lifecycle_report(Vec::new()))
    }
    pub fn rebase(
        &mut self,
        value: M,
        policy: crate::DraftPolicy,
        cx: &mut Context<Self>,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        self.check_lifecycle(policy)?;
        self.install_model(value, true, ModelChangeKind::Rebase, policy, cx);
        Ok(self.lifecycle_report(Vec::new()))
    }
    fn check_lifecycle(&self, policy: crate::DraftPolicy) -> Result<(), crate::LifecycleError> {
        if policy == crate::DraftPolicy::Preserve {
            if self.edit_clock.is_pending() {
                return Err(crate::LifecycleError::EditsPending);
            }
            let retired: Vec<_> = self
                .drafts
                .keys()
                .filter(|(key, _)| key.address().has_dynamic_occurrence())
                .map(|(key, _)| key.clone())
                .collect();
            if !retired.is_empty() {
                return Err(crate::LifecycleError::WouldRetireDrafts(retired));
            }
        }
        Ok(())
    }
    fn lifecycle_report(&self, conflicts: Vec<PathKey>) -> crate::LifecycleReport {
        crate::LifecycleReport {
            version: self.version(),
            conflicts,
        }
    }
    pub fn is_current(&self, version: FormVersion) -> bool {
        self.version() == version && !self.edit_clock.is_pending()
    }

    pub fn rebase_preserving_edits(
        &mut self,
        remote: M,
        cx: &mut Context<Self>,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        if self.edit_clock.is_pending() {
            return Err(crate::LifecycleError::EditsPending);
        }
        let mut plan = crate::lifecycle::MergePlan::new(
            self.drafts.values().map(|d| d.address.clone()).collect(),
        );
        let merged = M::__merge(&self.baseline, &self.value, &remote, &mut plan);
        let mut live = self.topology.stage();
        for address in &plan.external {
            live.retire_descendants(address);
        }
        materialize_model_in(&merged, &mut live);
        let live_snapshot = live.snapshot();
        let mut baseline = self.topology.stage();
        for (address, choice) in &plan.baseline {
            match choice {
                crate::lifecycle::BaselineChoice::Current => {
                    baseline.copy_subtree(address, &live_snapshot)
                }
                crate::lifecycle::BaselineChoice::Previous => {
                    baseline.copy_subtree(address, &self.baseline_topology)
                }
                crate::lifecycle::BaselineChoice::Unmapped => baseline.retire_descendants(address),
            }
        }
        // Unmapped remote structure deliberately has no occurrence correspondence.
        // Never allocate baseline-only IDs that a later live insertion could reuse.
        let before = self.topology.dynamic_addresses();
        let changed = self.value != merged;
        let baseline_changed = self.baseline != remote;
        self.value = merged;
        self.baseline = remote;
        self.baseline_topology = baseline.snapshot();
        self.topology.commit(live);
        let conflicts = plan
            .conflicts
            .iter()
            .filter_map(|a| self.topology.key(a))
            .collect();
        if changed || baseline_changed {
            self.validation.retain_current(&self.topology);
            self.invalidate_async_for_model_change(&plan.external);
            self.run_validation(ValidationTrigger::External, None, self.next_version());
            let retired = difference(&before, &self.topology.dynamic_addresses());
            if changed {
                self.publish_model(
                    ModelChangeKind::Rebase,
                    ChangeSet::whole_model(retired),
                    None,
                    cx,
                );
            } else {
                self.publish_session(ModelChangeKind::Rebase, ChangeSet::whole_model(retired), cx);
            }
        }
        Ok(self.lifecycle_report(conflicts))
    }

    pub fn rebase_if_current(
        &mut self,
        version: FormVersion,
        value: M,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.drafts.is_empty()
            || self.edit_clock.is_pending()
            || !version.is_current(self.session(), self.revision(), self.edit_clock.epoch())
        {
            return false;
        }
        self.install_model(
            value,
            true,
            ModelChangeKind::Rebase,
            crate::DraftPolicy::Discard,
            cx,
        );
        true
    }

    fn install_model(
        &mut self,
        value: M,
        rebase: bool,
        kind: ModelChangeKind,
        policy: crate::DraftPolicy,
        cx: &mut Context<Self>,
    ) {
        let model_changed = self.value != value;
        let had_drafts = !self.drafts.is_empty();
        let abandoning_drafts = policy == crate::DraftPolicy::Discard && had_drafts;
        let abandoning_tickets =
            policy == crate::DraftPolicy::Discard && self.edit_clock.is_pending();
        let changed = model_changed
            || (rebase && self.baseline != value)
            || !self.topology.dynamic_addresses().is_empty()
            || abandoning_drafts
            || abandoning_tickets;
        if !changed {
            let before = self.validation.clone();
            let was_validating = self.async_validation.is_pending();
            self.cancel_all_async_validation();
            self.validation
                .issues
                .retain(|i| i.source() == &ValidationSource::Control);
            self.started_required.clear();
            for rule in &mut self.rules {
                rule.started = false;
            }
            self.run_validation(ValidationTrigger::Mount, None, self.version());
            if self.validation != before || was_validating {
                self.publish_validation(cx);
            }
            return;
        }
        if policy == crate::DraftPolicy::Discard {
            self.drafts.clear();
            self.edit_clock.cancel_all();
        }
        let retired = self.topology.dynamic_addresses();
        if rebase {
            self.baseline = value.clone();
        }
        self.value = value;
        self.topology.reset();
        materialize_model(&self.value, &self.topology);
        if rebase || kind == ModelChangeKind::Reset {
            self.baseline_topology = self.topology.snapshot();
        }
        self.cancel_all_async_validation();
        self.validation.issues.retain(|i| {
            i.source() == &ValidationSource::Control && policy == crate::DraftPolicy::Preserve
        });
        self.started_required.clear();
        for rule in &mut self.rules {
            rule.started = false;
        }
        self.rules
            .retain(|r| r.owner(&self.topology.snapshot()).is_some());
        self.run_validation(ValidationTrigger::Mount, None, self.next_version());
        if model_changed {
            self.publish_model(kind, ChangeSet::whole_model(retired), None, cx);
        } else {
            self.publish_session(kind, ChangeSet::whole_model(retired), cx);
        }
        if had_drafts && policy == crate::DraftPolicy::Discard {
            self.emit_draft(root_address(), false, cx);
        }
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
        self.commit_topology_change(vec![address], Vec::new(), true, cx);
    }

    pub(crate) fn commit_topology_retiring(
        &mut self,
        address: CanonicalAddress,
        retired: Vec<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        self.commit_topology_change(vec![address], retired, true, cx);
    }

    pub(crate) fn commit_topology_scopes_retiring(
        &mut self,
        primary: CanonicalAddress,
        scopes: Vec<CanonicalAddress>,
        retired: Vec<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        let _ = primary;
        self.commit_topology_change(scopes, retired, true, cx);
    }

    pub(crate) fn commit_topology_replacing(
        &mut self,
        address: CanonicalAddress,
        retired: Vec<CanonicalAddress>,
        model_changed: bool,
        cx: &mut Context<Self>,
    ) {
        self.commit_topology_change(vec![address], retired, model_changed, cx);
    }

    fn commit_topology_change(
        &mut self,
        scopes: Vec<CanonicalAddress>,
        retired: Vec<CanonicalAddress>,
        model_changed: bool,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_async_for_model_change(&scopes);
        let mut edit = self.topology.stage();
        materialize_model_in(&self.value, &mut edit);
        self.topology.commit(edit);
        self.refresh_validation(&scopes);
        let mut changes = ChangeSet::aggregate(scopes, retired);
        if model_changed {
            self.publish_model(ModelChangeKind::Edit, changes, None, cx);
        } else {
            changes.value = crate::change::ValueImpact::Scopes(Vec::new());
            self.publish_session(ModelChangeKind::Edit, changes, cx);
        }
    }

    fn run_validation(
        &mut self,
        trigger: ValidationTrigger,
        scopes: Option<&[CanonicalAddress]>,
        version: FormVersion,
    ) {
        let scopes = if trigger == ValidationTrigger::External {
            None
        } else {
            scopes
        };
        self.rules
            .retain(|r| r.owner(&self.topology.snapshot()).is_some());
        let owners: HashSet<_> = self
            .rules
            .iter()
            .filter_map(|r| r.owner(&self.topology.snapshot()))
            .collect();
        self.validation.issues.retain(|i| match &i.owner {
            Some(owner @ validation::RuleOwner::Custom(..)) => owners.contains(owner),
            _ => true,
        });
        validation::validate(
            &self.value,
            &self.topology,
            version,
            trigger,
            scopes,
            &self
                .drafts
                .values()
                .map(|d| d.address.clone())
                .collect::<Vec<_>>(),
            &mut self.rules,
            &mut self.started_required,
            &mut self.validation,
        );
    }

    fn refresh_validation(&mut self, scopes: &[CanonicalAddress]) {
        self.validation.retain_current(&self.topology);
        self.run_validation(ValidationTrigger::Change, Some(scopes), self.next_version());
    }

    pub(crate) fn validate_at(
        &mut self,
        trigger: ValidationTrigger,
        address: Option<CanonicalAddress>,
        cx: &mut Context<Self>,
    ) {
        let before = self.validation.clone();
        self.run_validation(
            trigger,
            address.as_ref().map(std::slice::from_ref),
            self.version(),
        );
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
        _active: Weak<AtomicBool>,
        issue: Option<(String, ValidationMessage)>,
    ) -> bool {
        let issue = issue.map(|(code, message)| ValidationIssue {
            owner: None,
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
            control_active: None,
        });
        self.validation.replace_control(control, issue)
    }

    pub(crate) fn complete_control_write(
        &mut self,
        control: u64,
        address: CanonicalAddress,
        changed: bool,
        origin: ControlOrigin,
        cx: &mut Context<Self>,
    ) {
        let before = self.drafts.len();
        self.drafts.retain(|_, d| d.control != control);
        let draft_changed = before != self.drafts.len();
        let issue_changed = self.validation.replace_control(control, None);
        if changed {
            self.commit_control_value(address.clone(), origin, cx);
            if draft_changed {
                self.emit_draft(address, false, cx);
            }
        } else if draft_changed {
            self.refresh_validation(std::slice::from_ref(&address));
            self.emit_draft(address, true, cx);
        } else if issue_changed {
            self.publish_validation(cx);
        }
    }

    pub(crate) fn clear_control_issue(&mut self, control: u64, cx: &mut Context<Self>) {
        if self.validation.replace_control(control, None) {
            self.publish_validation(cx);
        }
    }

    /// Explicitly abandon editor drafts (including queued input) within a typed subtree.
    pub fn discard_drafts(
        &mut self,
        path: impl crate::ChangeTarget<M>,
        cx: &mut Context<Self>,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        let target = path.__change_target();
        let key = self.topology.key(&target.address);
        if target.session.is_some_and(|s| s != self.session()) || (target.dynamic && key.is_none())
        {
            return Err(crate::LifecycleError::InvalidTarget);
        }
        let tickets_changed = self.edit_clock.cancel_at(&target.address);
        let controls: HashSet<_> = self
            .drafts
            .values()
            .filter(|d| target.address.is_prefix_of(&d.address))
            .map(|d| d.control)
            .collect();
        self.drafts
            .retain(|_, d| !target.address.is_prefix_of(&d.address));
        for control in &controls {
            self.validation.replace_control(*control, None);
        }
        if !controls.is_empty() || tickets_changed {
            self.invalidate_async_for_model_change(std::slice::from_ref(&target.address));
            self.refresh_validation(std::slice::from_ref(&target.address));
            self.publish_session(
                ModelChangeKind::Replace,
                ChangeSet::subtree(target.address.clone(), false, Vec::new()),
                cx,
            );
            if !controls.is_empty() {
                self.emit_draft(target.address, false, cx);
            }
        }
        Ok(self.lifecycle_report(Vec::new()))
    }

    pub(crate) fn ensure_can_remove(&self, address: &CanonicalAddress) -> Result<(), ResolveError> {
        let paths: Vec<_> = self
            .drafts
            .keys()
            .filter(|(key, _)| address.is_prefix_of(key.address()))
            .map(|(key, _)| key.clone())
            .collect();
        if paths.is_empty() {
            Ok(())
        } else {
            Err(ResolveError::WouldRetireDrafts { paths })
        }
    }

    pub(crate) fn ensure_can_retire(&self, address: &CanonicalAddress) -> Result<(), ResolveError> {
        let paths: Vec<_> = self
            .drafts
            .iter()
            .filter(|((key, _), _)| {
                key.address().has_dynamic_occurrence()
                    && address.is_prefix_of(key.address())
                    && key.address() != address
            })
            .map(|((key, _), _)| key.clone())
            .collect();
        if paths.is_empty() {
            Ok(())
        } else {
            Err(ResolveError::WouldRetireDrafts { paths })
        }
    }

    pub(crate) fn baseline_for<T: Clone + 'static>(&self, core: &PathCore<M, T>) -> Option<T> {
        if core
            .guards
            .iter()
            .any(|g| self.baseline_topology.incarnation(&g.address) != Some(g.incarnation))
        {
            return None;
        }
        core.access
            .get(&self.baseline, &self.baseline_topology)
            .ok()
            .cloned()
    }

    pub(crate) fn has_drafts_at(&self, address: &CanonicalAddress) -> bool {
        self.drafts
            .values()
            .any(|d| validation::intersects(address, &d.address))
    }

    pub(crate) fn editor_draft<D: Clone + PartialEq + 'static>(
        &self,
        key: &PathKey,
        editor: &crate::EditorKey,
        mapping: std::any::TypeId,
    ) -> Result<Option<D>, crate::EditorTypeMismatch> {
        self.drafts
            .get(&(key.clone(), editor.clone()))
            .map(|d| {
                if d.mapping != mapping {
                    return Err(crate::EditorTypeMismatch);
                }
                d.display
                    .downcast_ref::<D>()
                    .cloned()
                    .ok_or(crate::EditorTypeMismatch)
            })
            .transpose()
    }

    pub(crate) fn restore_editor(
        &mut self,
        key: &PathKey,
        editor: &crate::EditorKey,
        control: u64,
    ) {
        if let Some(draft) = self.drafts.get_mut(&(key.clone(), editor.clone())) {
            let old = draft.control;
            draft.control = control;
            for issue in &mut self.validation.issues {
                if issue.control == Some(old) {
                    issue.control = Some(control);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn accept_draft<D: Clone + PartialEq + 'static>(
        &mut self,
        key: PathKey,
        editor: crate::EditorKey,
        control: u64,
        display: D,
        mapping: std::any::TypeId,
        code: String,
        message: ValidationMessage,
        cx: &mut Context<Self>,
    ) {
        let address = key.address().clone();
        let record = crate::draft::DraftRecord::new(address.clone(), control, display, mapping);
        let changed = self
            .drafts
            .get(&(key.clone(), editor.clone()))
            .is_none_or(|old| !old.same_display(&record));
        self.drafts.insert((key, editor), record);
        let issue_changed =
            self.set_control_issue(control, address.clone(), Weak::new(), Some((code, message)));
        if changed {
            self.invalidate_async_for_model_change(std::slice::from_ref(&address));
            self.refresh_validation(std::slice::from_ref(&address));
            self.emit_draft(address, true, cx);
        } else if issue_changed {
            self.publish_validation(cx);
        }
    }

    fn emit_draft(&mut self, address: CanonicalAddress, advance: bool, cx: &mut Context<Self>) {
        if advance {
            self.runtime.advance();
        }
        cx.emit(FormEvent::DraftChanged(ModelChange::new(
            self.revision(),
            ModelChangeKind::Edit,
            self.session(),
            ChangeSet::subtree(address, false, Vec::new()),
            None,
        )));
        if advance {
            cx.notify();
        }
    }

    fn publish_session(
        &mut self,
        kind: ModelChangeKind,
        changes: ChangeSet,
        cx: &mut Context<Self>,
    ) {
        let Effect::Publish(FormEvent::ModelChanged(change)) = self.runtime.transition(
            Message::<M>::model_applied(kind, self.session(), changes, None),
        ) else {
            unreachable!()
        };
        cx.emit(FormEvent::SessionChanged(change));
        cx.notify();
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
        FormVersion::new(self.session(), revision, self.edit_clock.epoch())
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
        self.cancel_async_rule(&address, &source);
        let AsyncValidationEffect::Reserved(generation) =
            self.async_validation
                .transition(AsyncValidationMessage::Reserve {
                    address: address.clone(),
                    rule: source.clone(),
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
        let current = version.is_current(self.session(), self.revision(), self.edit_clock.epoch())
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
            owner: None,
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

    fn cancel_async_rule(&mut self, address: &CanonicalAddress, source: &str) {
        let AsyncValidationEffect::Cancelled(generations) =
            self.async_validation
                .transition(AsyncValidationMessage::CancelRule {
                    address: address.clone(),
                    rule: source.to_owned().into(),
                })
        else {
            unreachable!("cancellation always returns cancelled generations")
        };
        self.validation.remove_async_generations(&generations);
        self.validation.issues.retain(|issue| {
            !(issue.address == *address && matches!(&issue.source, ValidationSource::Async(rule) if rule.as_ref() == source))
        });
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
