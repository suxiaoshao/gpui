use std::{borrow::Cow, fmt};

use crate::{
    ChangeTarget, FormSchema, PathKey, ValidationRequest, ValidationSink, ValidationTrigger,
    ValidationTriggers, change::ChangeTargetInfo, topology::TopologySnapshot,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RuleId(Cow<'static, str>);
impl RuleId {
    pub fn new(id: impl Into<Cow<'static, str>>) -> Self {
        Self(id.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl From<&'static str> for RuleId {
    fn from(id: &'static str) -> Self {
        Self::new(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RuleOwner {
    Required(PathKey),
    Custom(RuleId, Option<PathKey>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleRegistrationError {
    DuplicateId(RuleId),
    MissingDependency(RuleId),
    InvalidInstance(RuleId),
}
impl fmt::Display for RuleRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid validation rule registration: {self:?}")
    }
}
impl std::error::Error for RuleRegistrationError {}

type Check<M> = dyn for<'a, 'b> Fn(ValidationRequest<'a, M>, &mut ValidationSink<'b, M>);
pub struct ValidationRule<M: FormSchema> {
    pub(crate) id: RuleId,
    pub(crate) dependencies: Vec<ChangeTargetInfo>,
    instance: Option<ChangeTargetInfo>,
    triggers: ValidationTriggers,
    pub(crate) check: Box<Check<M>>,
    pub(crate) started: bool,
}
impl<M: FormSchema> ValidationRule<M> {
    pub fn new(
        id: impl Into<RuleId>,
        check: impl for<'a, 'b> Fn(ValidationRequest<'a, M>, &mut ValidationSink<'b, M>) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            dependencies: Vec::new(),
            instance: None,
            triggers: ValidationTriggers {
                submit: true,
                ..Default::default()
            },
            check: Box::new(check),
            started: false,
        }
    }
    pub fn dependency(mut self, path: impl ChangeTarget<M>) -> Self {
        self.dependencies.push(path.__change_target());
        self
    }
    /// Distinguishes repeated rule instances by a live occurrence, never by an index.
    pub fn instance(mut self, path: impl ChangeTarget<M>) -> Self {
        self.instance = Some(path.__change_target());
        self
    }
    pub fn triggers(mut self, triggers: ValidationTriggers) -> Self {
        self.triggers = triggers;
        self
    }
    pub fn id(&self) -> &RuleId {
        &self.id
    }
    pub(crate) fn owner(&self, topology: &TopologySnapshot) -> Option<RuleOwner> {
        if self
            .dependencies
            .iter()
            .chain(self.instance.iter())
            .any(|d| {
                d.session.is_some_and(|s| s != topology.session())
                    || (d.dynamic && topology.key(&d.address).is_none())
            })
        {
            return None;
        }
        Some(RuleOwner::Custom(
            self.id.clone(),
            match &self.instance {
                Some(i) => Some(topology.key(&i.address)?),
                None => None,
            },
        ))
    }
    pub(crate) fn matches(
        &self,
        scopes: Option<&[crate::topology::CanonicalAddress]>,
        trigger: ValidationTrigger,
    ) -> bool {
        scopes.is_none_or(|scopes| {
            self.dependencies
                .iter()
                .any(|d| scopes.iter().any(|s| super::intersects(s, &d.address)))
        }) && (self.started
            || self.triggers.includes(trigger)
            || trigger == ValidationTrigger::External)
    }
    pub(crate) fn check_registration(
        rules: &[Self],
        topology: &TopologySnapshot,
    ) -> Result<(), RuleRegistrationError> {
        let mut ids = std::collections::HashSet::new();
        for rule in rules {
            if rule.dependencies.is_empty() {
                return Err(RuleRegistrationError::MissingDependency(rule.id.clone()));
            }
            if rule.instance.as_ref().is_some_and(|i| !i.dynamic) {
                return Err(RuleRegistrationError::InvalidInstance(rule.id.clone()));
            }
            let Some(owner) = rule.owner(topology) else {
                return Err(RuleRegistrationError::InvalidInstance(rule.id.clone()));
            };
            if !ids.insert(owner) {
                return Err(RuleRegistrationError::DuplicateId(rule.id.clone()));
            }
        }
        Ok(())
    }
}
