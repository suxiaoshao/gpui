use std::{
    borrow::Cow,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use gpui::{App, Context, Entity, Window};
use gpui_operation::Transition as _;

use crate::{
    field::{FieldMutationError, PartialFormField},
    form::transition::{ClearControlValidationIssue, SetControlValidationIssue},
    form::{FormState, apply_validation_effect},
    validation::report::{ValidationIssue, ValidationMessage, ValidationSource},
    validation::trigger::ValidationTrigger,
};

static NEXT_CONTROL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ControlId(u64);

impl ControlId {
    fn next() -> Self {
        Self(NEXT_CONTROL_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub(crate) struct ControlLease {
    active: AtomicBool,
}

impl ControlLease {
    fn new() -> Self {
        Self {
            active: AtomicBool::new(true),
        }
    }

    fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ControlLifetime(pub(crate) Weak<ControlLease>);

impl ControlLifetime {
    pub(crate) fn is_alive(&self) -> bool {
        self.0.upgrade().is_some_and(|lease| lease.is_active())
    }
}

pub struct ControlBinding<Form, T>
where
    Form: FormState,
{
    form: gpui::WeakEntity<Form>,
    field: PartialFormField<Form, T>,
    id: ControlId,
    lease: Arc<ControlLease>,
}

impl<Form, T> Clone for ControlBinding<Form, T>
where
    Form: FormState,
{
    fn clone(&self) -> Self {
        Self {
            form: self.form.clone(),
            field: self.field.clone(),
            id: self.id,
            lease: self.lease.clone(),
        }
    }
}

impl<Form, T> Drop for ControlBinding<Form, T>
where
    Form: FormState,
{
    fn drop(&mut self) {
        if Arc::strong_count(&self.lease) == 1 {
            self.lease.deactivate();
        }
    }
}

impl<Form, T> ControlBinding<Form, T>
where
    Form: FormState,
    T: Clone + PartialEq + 'static,
{
    pub(crate) fn new(form: &Entity<Form>, field: PartialFormField<Form, T>) -> Self {
        Self {
            form: form.downgrade(),
            field,
            id: ControlId::next(),
            lease: Arc::new(ControlLease::new()),
        }
    }

    fn clear_issue(form: &Entity<Form>, id: ControlId, cx: &mut App) {
        form.update(cx, move |form, form_cx| {
            let effect = form
                .__runtime_mut()
                .validation_mut()
                .transition(ClearControlValidationIssue { id });
            apply_validation_effect(effect, form_cx);
        });
    }

    fn deactivate(&self, form: Option<&Entity<Form>>, cx: &mut App) {
        self.lease.deactivate();
        if let Some(form) = form {
            Self::clear_issue(form, self.id, cx);
        }
    }

    pub fn defer_set<Owner>(&self, value: T, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let binding = self.clone();
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(_lease) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if matches!(
                binding.field.try_set(&form, value, cx),
                Err(FieldMutationError::Access(_))
            ) {
                binding.deactivate(Some(&form), cx);
            }
        });
    }

    pub fn defer_blur<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let binding = self.clone();
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(_lease) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if binding
                .field
                .try_validate(&form, ValidationTrigger::Blur, cx)
                .is_err()
            {
                binding.deactivate(Some(&form), cx);
            }
        });
    }

    pub fn defer_set_issue<Owner>(
        &self,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
        window: &Window,
        cx: &mut Context<Owner>,
    ) where
        Owner: 'static,
    {
        let binding = self.clone();
        let lease = Arc::downgrade(&self.lease);
        let code = code.into();
        cx.defer_in(window, move |_, _, cx| {
            let Some(active) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if binding.field.try_value(&form, cx).is_err() {
                binding.deactivate(Some(&form), cx);
                return;
            }
            let issue = ValidationIssue::field(
                binding.field.path().clone(),
                ValidationTrigger::Change,
                ValidationSource::Control,
                code,
                message,
            );
            form.update(cx, move |form, form_cx| {
                let effect =
                    form.__runtime_mut()
                        .validation_mut()
                        .transition(SetControlValidationIssue {
                            id: binding.id,
                            lease: active,
                            issue,
                        });
                apply_validation_effect(effect, form_cx);
            });
        });
    }

    pub fn defer_clear_issue<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let binding = self.clone();
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(_lease) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if binding.field.try_value(&form, cx).is_err() {
                binding.deactivate(Some(&form), cx);
                return;
            }
            Self::clear_issue(&form, binding.id, cx);
        });
    }
}
