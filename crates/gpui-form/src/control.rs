use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use gpui::{App, Context, Entity, WeakEntity, Window};

use crate::{
    DynamicPath, Form, FormSchema, MutationError, PathCore, ResolveError, TotalPath,
    ValidationMessage, ValidationTrigger, topology::TopologyEpoch,
};

static NEXT_CONTROL_ID: AtomicU64 = AtomicU64::new(1);

enum BindingPath<Root: FormSchema, T: 'static> {
    Total(PathCore<Root, T>),
    Dynamic(PathCore<Root, T>),
}

impl<Root: FormSchema, T: 'static> Clone for BindingPath<Root, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Total(path) => Self::Total(path.clone()),
            Self::Dynamic(path) => Self::Dynamic(path.clone()),
        }
    }
}

pub struct ControlBinding<Root: FormSchema, T: 'static> {
    form: WeakEntity<Form<Root>>,
    path: BindingPath<Root, T>,
    control_id: u64,
    active: Arc<AtomicBool>,
    epoch: TopologyEpoch,
}

pub struct ControlLease {
    active: Arc<AtomicBool>,
}

impl Drop for ControlLease {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

impl<Root: FormSchema, T: 'static> Clone for ControlBinding<Root, T> {
    fn clone(&self) -> Self {
        Self {
            form: self.form.clone(),
            path: self.path.clone(),
            control_id: self.control_id,
            active: self.active.clone(),
            epoch: self.epoch,
        }
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> ControlBinding<Root, T> {
    pub(crate) fn total(form: &Entity<Form<Root>>, path: TotalPath<Root, T>, cx: &mut App) -> Self {
        Self::new(form, BindingPath::Total(path.core), cx)
    }

    pub(crate) fn dynamic(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, T>,
        cx: &mut App,
    ) -> Self {
        Self::new(form, BindingPath::Dynamic(path.core), cx)
    }

    fn new(form: &Entity<Form<Root>>, path: BindingPath<Root, T>, cx: &mut App) -> Self {
        let control_id = NEXT_CONTROL_ID.fetch_add(1, Ordering::Relaxed);
        let active = Arc::new(AtomicBool::new(true));
        let (address, incarnation, epoch) = {
            let current = form.read(cx);
            let core = match &path {
                BindingPath::Total(core) | BindingPath::Dynamic(core) => core,
            };
            let incarnation = current
                .topology()
                .ensure_incarnation(&core.address)
                .expect("form identity exhausted after construction");
            (
                core.address.clone(),
                incarnation,
                current.topology().epoch(),
            )
        };
        form.update(cx, |form, _| {
            form.register_control(
                control_id,
                address,
                incarnation,
                epoch,
                Arc::downgrade(&active),
            );
        });
        Self {
            form: form.downgrade(),
            path,
            control_id,
            active,
            epoch,
        }
    }

    pub fn lease(&self) -> ControlLease {
        ControlLease {
            active: self.active.clone(),
        }
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn is_current(&self, form: &Form<Root>) -> bool {
        if !self.is_active() || form.topology().epoch() != self.epoch {
            return false;
        }
        let core = match &self.path {
            BindingPath::Total(path) | BindingPath::Dynamic(path) => path,
        };
        !matches!(self.path, BindingPath::Dynamic(_)) || core.check(form).is_ok()
    }

    pub fn value(&self, cx: &App) -> Result<T, ResolveError> {
        if !self.is_active() {
            return Err(ResolveError::Retired {
                path: self.fallback_key(),
            });
        }
        let form = self.form.upgrade().ok_or_else(|| ResolveError::Retired {
            path: self.fallback_key(),
        })?;
        let form = form.read(cx);
        if !self.is_current(form) {
            return Err(ResolveError::Retired {
                path: self.fallback_key(),
            });
        }
        let core = match &self.path {
            BindingPath::Total(path) | BindingPath::Dynamic(path) => path,
        };
        if matches!(self.path, BindingPath::Dynamic(_)) {
            core.check(form)?;
        }
        Ok(core
            .access
            .get(form.value(), &form.topology().snapshot())?
            .clone())
    }

    fn fallback_key(&self) -> crate::PathKey {
        let core = match &self.path {
            BindingPath::Total(path) | BindingPath::Dynamic(path) => path,
        };
        let session = core.session.unwrap_or(crate::topology::SessionId(0));
        crate::PathKey::total(session, &core.address)
    }

    pub fn defer_set<Owner>(&self, value: T, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let binding = self.clone();
        cx.defer_in(window, move |_, _, cx| {
            if !binding.is_active() {
                return;
            }
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if !binding.is_current(form.read(cx)) {
                return;
            }
            let _ = binding.set(&form, value, cx);
        });
    }

    fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) -> Result<(), MutationError> {
        match &self.path {
            BindingPath::Total(core) => TotalPath { core: core.clone() }.set(form, value, cx),
            BindingPath::Dynamic(core) => {
                DynamicPath { core: core.clone() }.try_set(form, value, cx)?;
            }
        }
        Ok(())
    }

    pub fn defer_blur<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let binding = self.clone();
        cx.defer_in(window, move |_, _, cx| {
            if !binding.is_active() {
                return;
            }
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if !binding.is_current(form.read(cx)) {
                return;
            }
            match &binding.path {
                BindingPath::Total(core) => {
                    TotalPath { core: core.clone() }.validate(&form, ValidationTrigger::Blur, cx);
                }
                BindingPath::Dynamic(core) => {
                    let _ = DynamicPath { core: core.clone() }.try_validate(
                        &form,
                        ValidationTrigger::Blur,
                        cx,
                    );
                }
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
        let code = code.into().into_owned();
        cx.defer_in(window, move |_, _, cx| {
            if !binding.is_active() {
                return;
            }
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            let core = match &binding.path {
                BindingPath::Total(path) | BindingPath::Dynamic(path) => path,
            };
            let address = core.address.clone();
            let is_current = binding.is_current(form.read(cx));
            if !is_current {
                return;
            }
            form.update(cx, |form, cx| {
                form.set_control_issue(binding.control_id, address, Some((code, message)), cx);
            });
        });
    }

    pub fn defer_clear_issue<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let binding = self.clone();
        cx.defer_in(window, move |_, _, cx| {
            if !binding.is_active() {
                return;
            }
            let Some(form) = binding.form.upgrade() else {
                return;
            };
            if !binding.is_current(form.read(cx)) {
                return;
            }
            let core = match &binding.path {
                BindingPath::Total(path) | BindingPath::Dynamic(path) => path,
            };
            let address = core.address.clone();
            form.update(cx, |form, cx| {
                form.set_control_issue(binding.control_id, address, None, cx);
            });
        });
    }
}
