mod transition;

use std::{
    borrow::Cow,
    cell::RefCell,
    rc::{Rc, Weak as RcWeak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use gpui::{AnyWindowHandle, App, Context, Entity, Subscription, WeakEntity, Window};
use gpui_operation::Transition as _;

use crate::{
    DynamicPath, Form, FormEvent, FormSchema, PathCore, ResolveError, TotalPath, ValidationMessage,
    ValidationTrigger,
    change::{ChangeTargetInfo, ControlOrigin},
};

use self::transition::{BindingState, Effect, Lifecycle, Message, PendingProjection};

static NEXT_CONTROL_ID: AtomicU64 = AtomicU64::new(1);

fn next_control_id() -> u64 {
    NEXT_CONTROL_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .expect("form control identity space exhausted")
}

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

impl<Root: FormSchema, T: Clone + PartialEq + 'static> BindingPath<Root, T> {
    fn target(&self) -> ChangeTargetInfo {
        let (core, dynamic) = match self {
            Self::Total(core) => (core, false),
            Self::Dynamic(core) => (core, true),
        };
        ChangeTargetInfo {
            session: core.change_session(),
            address: core.change_address().clone(),
            dynamic,
        }
    }

    fn is_current(&self, form: &Form<Root>) -> bool {
        match self {
            Self::Total(_) => true,
            Self::Dynamic(core) => core.is_active_in(form.topology()),
        }
    }

    fn value(&self, form: &Form<Root>) -> Result<T, ResolveError> {
        let core = match self {
            Self::Total(core) => core,
            Self::Dynamic(core) => {
                core.check(form)?;
                core
            }
        };
        Ok(core
            .access
            .get(form.value(), &form.topology().snapshot())?
            .clone())
    }

    fn set(
        &self,
        form: &mut Form<Root>,
        value: T,
        control_id: u64,
        origin: ControlOrigin,
        cx: &mut Context<Form<Root>>,
    ) -> Result<(), ResolveError> {
        if let Self::Dynamic(core) = self {
            core.check(form)?;
        }
        let core = match self {
            Self::Total(core) | Self::Dynamic(core) => core,
        };
        let changed = {
            let (model, topology) = form.model_and_topology();
            let snapshot = topology.snapshot();
            let current = core.access.get(model, &snapshot)?;
            if current == &value {
                false
            } else {
                *core.access.get_mut(model, &snapshot)? = value;
                true
            }
        };
        form.complete_control_write(control_id, core.address.clone(), changed, origin, cx);
        Ok(())
    }

    fn validate(
        &self,
        form: &mut Form<Root>,
        trigger: ValidationTrigger,
        cx: &mut Context<Form<Root>>,
    ) -> Result<(), ResolveError> {
        if let Self::Dynamic(core) = self {
            core.check(form)?;
        }
        let core = match self {
            Self::Total(core) | Self::Dynamic(core) => core,
        };
        form.validate_at(trigger, Some(core.address.clone()), cx);
        Ok(())
    }
}

struct BindingShared {
    control_id: u64,
    active: Arc<AtomicBool>,
    lifecycle_generation: AtomicU64,
    editor_sequence: AtomicU64,
}

type Projector<T, Owner> =
    dyn Fn(&mut Owner, ControlProjection<T>, &mut Window, &mut Context<Owner>) + 'static;

struct DrainRequest<Root: FormSchema, T: 'static, Owner: 'static> {
    form: WeakEntity<Form<Root>>,
    path: BindingPath<Root, T>,
    shared: Arc<BindingShared>,
    state: Rc<RefCell<BindingState>>,
    projector: Rc<Projector<T, Owner>>,
    owner: WeakEntity<Owner>,
    window: AnyWindowHandle,
}

impl BindingShared {
    fn new(control_id: u64) -> Self {
        Self {
            control_id,
            active: Arc::new(AtomicBool::new(true)),
            lifecycle_generation: AtomicU64::new(1),
            editor_sequence: AtomicU64::new(0),
        }
    }

    fn advance_generation(&self) {
        self.lifecycle_generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .expect("form binding lifecycle generation exhausted");
    }

    fn advance_editor_sequence(&self) -> u64 {
        self.editor_sequence
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map(|value| value + 1)
            .expect("form editor sequence exhausted")
    }
}

/// The non-clone lifecycle owner for one native-control projection subscription.
pub struct ControlBinding {
    _subscription: Subscription,
    state: Rc<RefCell<BindingState>>,
    shared: Arc<BindingShared>,
}

impl Drop for ControlBinding {
    fn drop(&mut self) {
        self.shared.active.store(false, Ordering::Release);
        self.shared.advance_generation();
        let _ = (&mut *self.state.borrow_mut()).transition(Message::Drop);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlProjection<T> {
    Value(T),
    Retired,
}

/// A cloneable weak capability used only by native control callbacks.
pub struct ControlWriter<Root: FormSchema, T: 'static> {
    form: WeakEntity<Form<Root>>,
    path: BindingPath<Root, T>,
    shared: Arc<BindingShared>,
    state: RcWeak<RefCell<BindingState>>,
}

impl<Root: FormSchema, T: 'static> Clone for ControlWriter<Root, T> {
    fn clone(&self) -> Self {
        Self {
            form: self.form.clone(),
            path: self.path.clone(),
            shared: self.shared.clone(),
            state: self.state.clone(),
        }
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> ControlWriter<Root, T> {
    fn begin_native_edit<Owner>(&self, cx: &mut Context<Owner>) -> u64
    where
        Owner: 'static,
    {
        let editor_sequence = self.shared.advance_editor_sequence();
        let Some(form) = self.form.upgrade() else {
            return editor_sequence;
        };
        let revision = form.read(cx).revision();
        if let Some(state) = self.state.upgrade() {
            let _ = (&mut *state.borrow_mut()).transition(Message::SuppressThrough(revision));
        }
        editor_sequence
    }

    pub fn defer_set<Owner>(&self, value: T, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        if !self.shared.active.load(Ordering::Acquire) {
            return;
        }
        let editor_sequence = self.begin_native_edit(cx);
        let lifecycle_generation = self.shared.lifecycle_generation.load(Ordering::Acquire);
        let writer = self.clone();
        cx.defer_in(window, move |_, _, cx| {
            if !writer.shared.active.load(Ordering::Acquire)
                || writer.shared.lifecycle_generation.load(Ordering::Acquire)
                    != lifecycle_generation
            {
                return;
            }
            let Some(form) = writer.form.upgrade() else {
                return;
            };
            let origin = ControlOrigin {
                control_id: writer.shared.control_id,
                lifecycle_generation,
                editor_sequence,
            };
            let _ = form.update(cx, |form, cx| {
                writer
                    .path
                    .set(form, value, writer.shared.control_id, origin, cx)
            });
        });
    }

    pub fn defer_blur<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let writer = self.clone();
        let lifecycle_generation = self.shared.lifecycle_generation.load(Ordering::Acquire);
        cx.defer_in(window, move |_, _, cx| {
            if !writer.shared.active.load(Ordering::Acquire)
                || writer.shared.lifecycle_generation.load(Ordering::Acquire)
                    != lifecycle_generation
            {
                return;
            }
            let Some(form) = writer.form.upgrade() else {
                return;
            };
            let _ = form.update(cx, |form, cx| {
                writer.path.validate(form, ValidationTrigger::Blur, cx)
            });
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
        if !self.shared.active.load(Ordering::Acquire) {
            return;
        }
        // Invalid/incomplete native text is still a newer editor state. It must
        // suppress any older authoritative Value already waiting in the mailbox.
        self.begin_native_edit(cx);
        let writer = self.clone();
        let code = code.into().into_owned();
        let lifecycle_generation = self.shared.lifecycle_generation.load(Ordering::Acquire);
        cx.defer_in(window, move |_, _, cx| {
            if !writer.shared.active.load(Ordering::Acquire)
                || writer.shared.lifecycle_generation.load(Ordering::Acquire)
                    != lifecycle_generation
            {
                return;
            }
            let Some(form) = writer.form.upgrade() else {
                return;
            };
            if !writer.path.is_current(form.read(cx)) {
                return;
            }
            let address = match &writer.path {
                BindingPath::Total(core) | BindingPath::Dynamic(core) => core.address.clone(),
            };
            form.update(cx, |form, cx| {
                form.set_control_issue(
                    writer.shared.control_id,
                    address,
                    Arc::downgrade(&writer.shared.active),
                    Some((code, message)),
                    cx,
                );
            });
        });
    }

    pub fn defer_clear_issue<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        let writer = self.clone();
        let lifecycle_generation = self.shared.lifecycle_generation.load(Ordering::Acquire);
        cx.defer_in(window, move |_, _, cx| {
            if !writer.shared.active.load(Ordering::Acquire)
                || writer.shared.lifecycle_generation.load(Ordering::Acquire)
                    != lifecycle_generation
            {
                return;
            }
            let Some(form) = writer.form.upgrade() else {
                return;
            };
            form.update(cx, |form, cx| {
                form.clear_control_issue(writer.shared.control_id, cx);
            });
        });
    }
}

pub(crate) fn bind_total_in<Root, T, Owner>(
    form: &Entity<Form<Root>>,
    path: TotalPath<Root, T>,
    owner: &Entity<Owner>,
    project: impl Fn(&mut Owner, ControlProjection<T>, &mut Window, &mut Context<Owner>) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> (ControlBinding, ControlWriter<Root, T>)
where
    Root: FormSchema,
    T: Clone + PartialEq + 'static,
    Owner: 'static,
{
    bind_in(
        form,
        BindingPath::Total(path.core),
        owner,
        project,
        window,
        cx,
    )
}

pub(crate) fn bind_dynamic_in<Root, T, Owner>(
    form: &Entity<Form<Root>>,
    path: DynamicPath<Root, T>,
    owner: &Entity<Owner>,
    project: impl Fn(&mut Owner, ControlProjection<T>, &mut Window, &mut Context<Owner>) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Result<(ControlBinding, ControlWriter<Root, T>), ResolveError>
where
    Root: FormSchema,
    T: Clone + PartialEq + 'static,
    Owner: 'static,
{
    path.core.check(form.read(cx))?;
    Ok(bind_in(
        form,
        BindingPath::Dynamic(path.core),
        owner,
        project,
        window,
        cx,
    ))
}

fn bind_in<Root, T, Owner>(
    form: &Entity<Form<Root>>,
    path: BindingPath<Root, T>,
    owner: &Entity<Owner>,
    project: impl Fn(&mut Owner, ControlProjection<T>, &mut Window, &mut Context<Owner>) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> (ControlBinding, ControlWriter<Root, T>)
where
    Root: FormSchema,
    T: Clone + PartialEq + 'static,
    Owner: 'static,
{
    let shared = Arc::new(BindingShared::new(next_control_id()));
    let state = Rc::new(RefCell::new(BindingState::new()));
    let projector: Rc<Projector<T, Owner>> = Rc::new(project);
    let weak_form = form.downgrade();

    let subscription_path = path.clone();
    let subscription_shared = shared.clone();
    let subscription_state = state.clone();
    let subscription_projector = projector.clone();
    let subscription_form = weak_form.clone();
    let subscription_owner = owner.downgrade();
    let subscription_window = window.window_handle();
    let subscription = cx.subscribe(form, move |form, event: &FormEvent<Root>, cx| {
        let FormEvent::ModelChanged(change) = event else {
            return;
        };
        if subscription_state.borrow().lifecycle() != Lifecycle::Active {
            return;
        }

        let impact = change.impact_info(&subscription_path.target());
        let dynamic_retired = matches!(subscription_path, BindingPath::Dynamic(_))
            && (!subscription_path.is_current(form.read(cx)) || impact.retired());
        let effect = if dynamic_retired {
            subscription_shared.active.store(false, Ordering::Release);
            subscription_shared.advance_generation();
            (&mut *subscription_state.borrow_mut()).transition(Message::Retire)
        } else if change.kind() != crate::ModelChangeKind::Edit {
            subscription_shared.advance_generation();
            (&mut *subscription_state.borrow_mut()).transition(Message::QueueValue {
                revision: change.revision(),
                editor_sequence: subscription_shared.editor_sequence.load(Ordering::Acquire),
            })
        } else if impact.value_changed() {
            let self_origin = change.origin().is_some_and(|origin| {
                origin.control_id == subscription_shared.control_id
                    && origin.lifecycle_generation
                        == subscription_shared
                            .lifecycle_generation
                            .load(Ordering::Acquire)
            });
            if self_origin {
                (&mut *subscription_state.borrow_mut())
                    .transition(Message::SuppressThrough(change.revision()))
            } else {
                (&mut *subscription_state.borrow_mut()).transition(Message::QueueValue {
                    revision: change.revision(),
                    editor_sequence: subscription_shared.editor_sequence.load(Ordering::Acquire),
                })
            }
        } else {
            Effect::None
        };

        if matches!(effect, Effect::ScheduleDrain) {
            schedule_drain(
                DrainRequest {
                    form: subscription_form.clone(),
                    path: subscription_path.clone(),
                    shared: subscription_shared.clone(),
                    state: subscription_state.clone(),
                    projector: subscription_projector.clone(),
                    owner: subscription_owner.clone(),
                    window: subscription_window,
                },
                cx,
            );
        }
    });

    let writer = ControlWriter {
        form: weak_form,
        path,
        shared: shared.clone(),
        state: Rc::downgrade(&state),
    };
    (
        ControlBinding {
            _subscription: subscription,
            state,
            shared,
        },
        writer,
    )
}

fn schedule_drain<Root, T, Owner>(request: DrainRequest<Root, T, Owner>, cx: &mut App)
where
    Root: FormSchema,
    T: Clone + PartialEq + 'static,
    Owner: 'static,
{
    let DrainRequest {
        form,
        path,
        shared,
        state,
        projector,
        owner,
        window,
    } = request;
    cx.defer(move |cx| {
        let Some(owner) = owner.upgrade() else {
            return;
        };
        let _ = window.update(cx, |_, window, cx| {
            owner.update(cx, |owner, cx| {
                let effect = (&mut *state.borrow_mut()).transition(Message::Drain);
                match effect {
                    Effect::Deliver(PendingProjection::Retired) => {
                        projector(owner, ControlProjection::Retired, window, cx);
                    }
                    Effect::Deliver(PendingProjection::Value {
                        editor_sequence, ..
                    }) => {
                        if !shared.active.load(Ordering::Acquire)
                            || shared.editor_sequence.load(Ordering::Acquire) > editor_sequence
                        {
                            return;
                        }
                        let Some(form) = form.upgrade() else {
                            return;
                        };
                        let value = {
                            let form_ref = form.read(cx);
                            if !path.is_current(form_ref) {
                                None
                            } else {
                                path.value(form_ref).ok()
                            }
                        };
                        let Some(value) = value else {
                            shared.active.store(false, Ordering::Release);
                            shared.advance_generation();
                            let _ = (&mut *state.borrow_mut()).transition(Message::Retire);
                            if matches!(
                                (&mut *state.borrow_mut()).transition(Message::Drain),
                                Effect::Deliver(PendingProjection::Retired)
                            ) {
                                projector(owner, ControlProjection::Retired, window, cx);
                            }
                            return;
                        };
                        projector(owner, ControlProjection::Value(value), window, cx);
                        form.update(cx, |form, cx| {
                            form.clear_control_issue(shared.control_id, cx);
                        });
                    }
                    Effect::None | Effect::ScheduleDrain => {}
                }
            });
        });
    });
}
