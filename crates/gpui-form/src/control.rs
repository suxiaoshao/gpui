use std::{
    borrow::Cow,
    ops::Deref,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use gpui::{App, Context, Entity, Window};

use crate::{
    error::{ValidationIssue, ValidationMessage, ValidationSource},
    field::{FormField, FormFieldError},
    form::{FormEvent, FormStore},
    trigger::ValidationTrigger,
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
pub(crate) struct ControlLifetime(Weak<ControlLease>);

impl ControlLifetime {
    pub(crate) fn is_alive(&self) -> bool {
        self.0.upgrade().is_some_and(|lease| lease.is_active())
    }
}

pub struct ControlAttachment<Form, T>
where
    Form: FormStore,
{
    field: FormField<Form, T>,
    id: ControlId,
    lease: Arc<ControlLease>,
}

impl<Form, T> Clone for ControlAttachment<Form, T>
where
    Form: FormStore,
{
    fn clone(&self) -> Self {
        Self {
            field: self.field.clone(),
            id: self.id,
            lease: self.lease.clone(),
        }
    }
}

impl<Form, T> ControlAttachment<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub(crate) fn new(field: FormField<Form, T>) -> Self {
        Self {
            field,
            id: ControlId::next(),
            lease: Arc::new(ControlLease::new()),
        }
    }

    fn deactivate_and_clear_issue(
        field: &FormField<Form, T>,
        id: ControlId,
        lease: &ControlLease,
        cx: &mut App,
    ) {
        lease.deactivate();
        let _ = field.update_form_if_alive(cx, move |form, form_cx| {
            if form
                .__runtime_mut()
                .validation_mut()
                .clear_control_issue(id)
            {
                form_cx.emit(FormEvent::RuntimeChanged);
                form_cx.notify();
            }
        });
    }

    pub fn defer_set_user_value<Owner>(&self, value: T, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let field = self.field.clone();
        let id = self.id;
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(lease) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            if matches!(
                field.set_user_value(value, cx),
                Err(FormFieldError::ValueUnavailable)
            ) {
                Self::deactivate_and_clear_issue(&field, id, &lease, cx);
            }
        });
    }

    pub fn defer_blur<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let field = self.field.clone();
        let id = self.id;
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(lease) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            if matches!(
                field.validate(ValidationTrigger::Blur, cx),
                Err(FormFieldError::ValueUnavailable)
            ) {
                Self::deactivate_and_clear_issue(&field, id, &lease, cx);
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
        let field = self.field.clone();
        let id = self.id;
        let code = code.into();
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(active) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            let lifetime = ControlLifetime(Arc::downgrade(&active));
            let issue = ValidationIssue::field(
                field.path().clone(),
                ValidationTrigger::Change,
                ValidationSource::Control,
                code,
                message,
            );
            let result = field.update_form(cx, move |form, form_cx| {
                form.__runtime_mut()
                    .validation_mut()
                    .set_control_issue(id, lifetime, issue);
                form_cx.emit(FormEvent::RuntimeChanged);
                form_cx.notify();
            });
            if matches!(result, Err(FormFieldError::ValueUnavailable)) {
                Self::deactivate_and_clear_issue(&field, id, &active, cx);
            }
        });
    }

    pub fn defer_clear_issue<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let field = self.field.clone();
        let id = self.id;
        let lease = Arc::downgrade(&self.lease);
        cx.defer_in(window, move |_, _, cx| {
            let Some(active) = lease.upgrade().filter(|lease| lease.is_active()) else {
                return;
            };
            let result = field.update_form(cx, move |form, form_cx| {
                if form
                    .__runtime_mut()
                    .validation_mut()
                    .clear_control_issue(id)
                {
                    form_cx.emit(FormEvent::RuntimeChanged);
                    form_cx.notify();
                }
            });
            if matches!(result, Err(FormFieldError::ValueUnavailable)) {
                Self::deactivate_and_clear_issue(&field, id, &active, cx);
            }
        });
    }
}

pub trait FormControl<T>: Deref<Target = Entity<Self::State>> + Sized
where
    T: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Error;

    fn new<Form, Owner, Build>(
        field: FormField<Form, T>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State;
}
