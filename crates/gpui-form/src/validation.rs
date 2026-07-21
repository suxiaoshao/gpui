use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
};

use gpui::{App, Task};

use crate::{
    array::FormItemId,
    control::{ControlId, ControlLifetime},
    error::{ValidationIssue, ValidationMessage, ValidationReport, ValidationSource},
    path::FieldPath,
    trigger::ValidationTrigger,
};

#[derive(Clone, Debug, Default)]
pub struct NoValidationContext;

#[derive(Clone, Debug, PartialEq)]
pub struct AsyncValidationIssue {
    pub code: Cow<'static, str>,
    pub message: ValidationMessage,
}

impl AsyncValidationIssue {
    pub fn new(code: impl Into<Cow<'static, str>>, message: ValidationMessage) -> Self {
        Self {
            code: code.into(),
            message,
        }
    }
}

pub trait ValidationContextValue: Clone + 'static {}

impl<T> ValidationContextValue for T where T: Clone + 'static {}

#[derive(Clone, Copy, Debug)]
pub struct ValidationContext<'a, C = NoValidationContext>
where
    C: ValidationContextValue,
{
    pub external: &'a C,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationScope {
    Form,
    Field(FieldPath),
    Group(FieldPath),
    ArrayItem { path: FieldPath, id: FormItemId },
}

impl ValidationScope {
    pub fn includes(&self, path: Option<&FieldPath>) -> bool {
        match (self, path) {
            (Self::Form, _) => true,
            (Self::Field(expected), Some(path)) => {
                expected.starts_with(path) || path.starts_with(expected)
            }
            (Self::Group(group), Some(path)) => group.starts_with(path) || path.starts_with(group),
            (Self::ArrayItem { path: array, id }, Some(path)) => {
                let item = array.join_item(*id);
                item.starts_with(path) || path.starts_with(&item)
            }
            _ => false,
        }
    }
}

pub trait ValidationAdapter<Model>: Default + 'static {
    type Context: ValidationContextValue;

    fn validate(
        &self,
        model: &Model,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport;
}

#[derive(Clone, Debug, Default)]
pub struct NoopValidationAdapter;

impl<Model: 'static> ValidationAdapter<Model> for NoopValidationAdapter {
    type Context = NoValidationContext;

    fn validate(
        &self,
        _model: &Model,
        _trigger: ValidationTrigger,
        _scope: ValidationScope,
        _context: ValidationContext<'_, Self::Context>,
        _cx: &App,
    ) -> ValidationAdapterReport {
        ValidationAdapterReport::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationAdapterReport {
    issues: Vec<ValidationIssue>,
}

impl ValidationAdapterReport {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }
}

#[derive(Clone, Debug)]
struct ControlIssue {
    lifetime: ControlLifetime,
    issue: ValidationIssue,
}

struct AsyncValidationEntry {
    generation: u64,
    task: Option<Task<()>>,
    issue: Option<ValidationIssue>,
}

#[derive(Default)]
pub struct FormValidationRuntime {
    generated_issues: Vec<ValidationIssue>,
    adapter_issues: Vec<ValidationIssue>,
    control_issues: BTreeMap<ControlId, ControlIssue>,
    async_generation: u64,
    async_entries: BTreeMap<(FieldPath, Cow<'static, str>), AsyncValidationEntry>,
}

impl FormValidationRuntime {
    pub fn report(&self) -> ValidationReport {
        let mut issues = self.generated_issues.clone();
        issues.extend(self.adapter_issues.iter().cloned());
        issues.extend(
            self.async_entries
                .values()
                .filter_map(|entry| entry.issue.clone()),
        );
        issues.extend(
            self.control_issues
                .values()
                .filter(|entry| entry.lifetime.is_alive())
                .map(|entry| entry.issue.clone()),
        );
        ValidationReport::new(issues)
    }

    #[doc(hidden)]
    pub fn replace_generated(&mut self, scope: &ValidationScope, next: Vec<ValidationIssue>) {
        self.generated_issues
            .retain(|issue| !scope.includes(issue.path.as_ref()));
        self.generated_issues.extend(next);
    }

    #[doc(hidden)]
    pub fn replace_adapter(&mut self, next: Vec<ValidationIssue>) {
        self.adapter_issues = next;
    }

    pub fn clear(&mut self) {
        self.generated_issues.clear();
        self.adapter_issues.clear();
        self.control_issues.clear();
        self.async_entries.clear();
    }

    pub fn clear_for_model_replacement(&mut self) {
        self.generated_issues.clear();
        self.adapter_issues.clear();
        self.async_entries.clear();
    }

    pub(crate) fn set_control_issue(
        &mut self,
        id: ControlId,
        lifetime: ControlLifetime,
        issue: ValidationIssue,
    ) {
        self.control_issues
            .insert(id, ControlIssue { lifetime, issue });
    }

    pub(crate) fn clear_control_issue(&mut self, id: ControlId) -> bool {
        self.control_issues.remove(&id).is_some()
    }

    pub fn is_validating(&self) -> bool {
        self.async_entries
            .values()
            .any(|entry| entry.task.is_some())
    }

    pub fn is_validating_at(&self, path: &FieldPath) -> bool {
        self.async_entries.iter().any(|((pending_path, _), entry)| {
            (pending_path.starts_with(path) || path.starts_with(pending_path))
                && entry.task.is_some()
        })
    }

    pub(crate) fn next_async_generation(&mut self) -> u64 {
        self.async_generation = self
            .async_generation
            .checked_add(1)
            .expect("async validation generation overflow");
        self.async_generation
    }

    pub(crate) fn set_async_task(
        &mut self,
        path: FieldPath,
        source: Cow<'static, str>,
        generation: u64,
        task: Task<()>,
    ) {
        self.async_entries.insert(
            (path, source),
            AsyncValidationEntry {
                generation,
                task: Some(task),
                issue: None,
            },
        );
    }

    pub(crate) fn cancel_async(&mut self, path: &FieldPath, source: &str) -> bool {
        self.async_entries
            .remove(&(path.clone(), Cow::Owned(source.to_owned())))
            .is_some()
    }

    #[doc(hidden)]
    pub fn invalidate_path(&mut self, path: &FieldPath) {
        self.generated_issues.retain(|issue| {
            issue.path.as_ref().is_none_or(|issue_path| {
                !issue_path.starts_with(path) && !path.starts_with(issue_path)
            })
        });
        self.async_entries.retain(|(entry_path, _), _| {
            !entry_path.starts_with(path) && !path.starts_with(entry_path)
        });
    }

    pub(crate) fn finish_async(
        &mut self,
        path: &FieldPath,
        source: &str,
        generation: u64,
        issue: Option<ValidationIssue>,
    ) -> bool {
        let key = (path.clone(), Cow::Owned(source.to_owned()));
        let Some(entry) = self.async_entries.get_mut(&key) else {
            return false;
        };
        if entry.generation != generation {
            return false;
        }
        entry.task = None;
        entry.issue = issue;
        true
    }
}

#[doc(hidden)]
pub trait StructuralValidate {
    fn structural_issues(
        &self,
        base: &FieldPath,
        trigger: ValidationTrigger,
        scope: &ValidationScope,
        issues: &mut Vec<ValidationIssue>,
    );
}

pub trait RequiredValue {
    fn is_missing(&self) -> bool;
}

impl RequiredValue for String {
    fn is_missing(&self) -> bool {
        self.trim().is_empty()
    }
}

impl RequiredValue for str {
    fn is_missing(&self) -> bool {
        self.trim().is_empty()
    }
}

impl<T> RequiredValue for Option<T> {
    fn is_missing(&self) -> bool {
        self.is_none()
    }
}

impl<T> RequiredValue for Vec<T> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl RequiredValue for bool {
    fn is_missing(&self) -> bool {
        !self
    }
}

impl<K, V, S> RequiredValue for HashMap<K, V, S> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl<K, V> RequiredValue for BTreeMap<K, V> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl<T, S> RequiredValue for HashSet<T, S> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

impl<T> RequiredValue for BTreeSet<T> {
    fn is_missing(&self) -> bool {
        self.is_empty()
    }
}

pub fn required_issue(path: FieldPath, trigger: ValidationTrigger) -> ValidationIssue {
    ValidationIssue::field(
        path,
        trigger,
        ValidationSource::Required,
        "required",
        ValidationMessage::key("gpui-form-error-required"),
    )
}

pub trait GardePathMapper {
    fn map_garde_path(&self, path: &str) -> Result<FieldPath, GardePathError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GardePathError {
    UnknownField {
        path: String,
    },
    InvalidIndex {
        path: String,
        value: String,
    },
    IndexOutOfBounds {
        path: String,
        index: usize,
        len: usize,
    },
    InvalidItemId {
        path: String,
        index: usize,
    },
    DuplicateItemId {
        path: String,
        index: usize,
    },
}

impl fmt::Display for GardePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField { path } => write!(f, "unknown field in Garde path `{path}`"),
            Self::InvalidIndex { path, value } => {
                write!(f, "invalid array index `{value}` in Garde path `{path}`")
            }
            Self::IndexOutOfBounds { path, index, len } => write!(
                f,
                "array index {index} is out of bounds for length {len} in Garde path `{path}`"
            ),
            Self::InvalidItemId { path, index } => write!(
                f,
                "array item {index} has no valid stable id for Garde path `{path}`"
            ),
            Self::DuplicateItemId { path, index } => write!(
                f,
                "array item {index} has a duplicate stable id for Garde path `{path}`"
            ),
        }
    }
}

impl std::error::Error for GardePathError {}

#[cfg(feature = "garde-adapter")]
pub trait GardeI18nProvider<C>: 'static
where
    C: ValidationContextValue,
{
    type Handler<'a>: garde::i18n::I18n + 'a
    where
        C: 'a;

    fn handler<'a>(context: &'a C, cx: &'a App) -> Self::Handler<'a>;
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultGardeI18nProvider;

#[cfg(feature = "garde-adapter")]
impl<C> GardeI18nProvider<C> for DefaultGardeI18nProvider
where
    C: ValidationContextValue,
{
    type Handler<'a>
        = garde::i18n::DefaultI18n
    where
        C: 'a;

    fn handler<'a>(_context: &'a C, _cx: &'a App) -> Self::Handler<'a> {
        garde::i18n::DefaultI18n
    }
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Copy, Debug)]
pub struct GardeAdapter<T, P = DefaultGardeI18nProvider> {
    marker: std::marker::PhantomData<fn() -> (T, P)>,
}

#[cfg(feature = "garde-adapter")]
impl<T, P> Default for GardeAdapter<T, P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "garde-adapter")]
impl<T, P> GardeAdapter<T, P> {
    pub fn new() -> Self {
        Self {
            marker: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "garde-adapter")]
impl<T, P> ValidationAdapter<T> for GardeAdapter<T, P>
where
    T: garde::Validate + GardePathMapper + 'static,
    T::Context: ValidationContextValue,
    P: GardeI18nProvider<T::Context>,
{
    type Context = T::Context;

    fn validate(
        &self,
        model: &T,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport {
        let handler = P::handler(context.external, cx);
        let result = garde::i18n::with_i18n(handler, || {
            garde::Validate::validate_with(model, context.external)
        });
        let Err(report) = result else {
            return ValidationAdapterReport::default();
        };

        let mut issues = Vec::new();
        for (path, error) in report.into_inner() {
            let garde_path = path.to_string();
            if garde_path.is_empty() {
                if scope.includes(None) {
                    issues.push(ValidationIssue::form(
                        trigger,
                        ValidationSource::Garde,
                        "garde",
                        ValidationMessage::localized(error.message().to_string()),
                    ));
                }
                continue;
            }

            match model.map_garde_path(&garde_path) {
                Ok(path) if scope.includes(Some(&path)) => issues.push(ValidationIssue::field(
                    path,
                    trigger,
                    ValidationSource::Garde,
                    "garde",
                    ValidationMessage::localized(error.message().to_string()),
                )),
                Ok(_) => {}
                Err(reason) => issues.push(
                    ValidationIssue::form(
                        trigger,
                        ValidationSource::Internal,
                        "garde_path_mapping",
                        ValidationMessage::key("gpui-form-error-internal"),
                    )
                    .with_param("path", garde_path)
                    .with_param("reason", reason.to_string()),
                ),
            }
        }
        ValidationAdapterReport::new(issues)
    }
}
