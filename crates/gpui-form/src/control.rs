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
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
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
        self.stored(form).cloned()
    }

    fn stored<'a>(&self, form: &'a Form<Root>) -> Result<&'a T, ResolveError> {
        let core = match self {
            Self::Total(core) => core,
            Self::Dynamic(core) => {
                core.check(form)?;
                core
            }
        };
        core.access.get(form.value(), &form.topology().snapshot())
    }

    fn set(
        &self,
        form: &mut Form<Root>,
        value: T,
        editor: &crate::EditorKey,
        origin: ControlOrigin,
        cx: &mut Context<Form<Root>>,
    ) -> Result<(), ResolveError> {
        if let Self::Dynamic(core) = self {
            core.check(form)?;
        }
        let core = match self {
            Self::Total(core) | Self::Dynamic(core) => core,
        };
        let can_retire = form.ensure_can_retire(&core.address);
        let changed = {
            let (model, topology) = form.model_and_topology();
            let snapshot = topology.snapshot();
            let current = core.access.get(model, &snapshot)?;
            if current == &value {
                false
            } else {
                can_retire?;
                *core.access.get_mut(model, &snapshot)? = value;
                true
            }
        };
        form.complete_control_write(editor, core.address.clone(), changed, origin, cx);
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
type EditorDiscard =
    dyn Fn(&crate::EditorKey, &mut App) -> Result<crate::LifecycleReport, crate::LifecycleError>;
type EditorQuery =
    dyn Fn(&crate::EditorKey, &App) -> Result<crate::EditorStatus, crate::LifecycleError>;

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
            .try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .expect("form binding lifecycle generation exhausted");
    }

    fn advance_editor_sequence(&self) -> u64 {
        self.editor_sequence
            .try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
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
    edit_clock: crate::draft::EditClock,
    refresh: Box<dyn Fn(&mut App)>,
    discard_editor: Box<EditorDiscard>,
    editor_status: Box<EditorQuery>,
}

impl ControlBinding {
    pub fn refresh(&self, cx: &mut App) {
        (self.refresh)(cx);
    }

    /// Discard one stable editor at this binding's exact path.
    pub fn discard_editor_draft(
        &self,
        editor: impl Into<crate::EditorKey>,
        cx: &mut App,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        (self.discard_editor)(&editor.into(), cx)
    }

    pub fn editor_status(
        &self,
        editor: impl Into<crate::EditorKey>,
        cx: &App,
    ) -> Result<crate::EditorStatus, crate::LifecycleError> {
        (self.editor_status)(&editor.into(), cx)
    }
}

impl Drop for ControlBinding {
    fn drop(&mut self) {
        self.edit_clock.cancel_control(self.shared.control_id);
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
    pub(crate) form: WeakEntity<Form<Root>>,
    path: BindingPath<Root, T>,
    shared: Arc<BindingShared>,
    state: RcWeak<RefCell<BindingState>>,
    edit_clock: crate::draft::EditClock,
    editor: crate::EditorKey,
}

impl<Root: FormSchema, T: 'static> Clone for ControlWriter<Root, T> {
    fn clone(&self) -> Self {
        Self {
            form: self.form.clone(),
            path: self.path.clone(),
            shared: self.shared.clone(),
            state: self.state.clone(),
            edit_clock: self.edit_clock.clone(),
            editor: self.editor.clone(),
        }
    }
}

impl<Root: FormSchema, T: Clone + PartialEq + 'static> ControlWriter<Root, T> {
    pub fn discard_draft(
        &self,
        cx: &mut App,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        let form = self
            .form
            .upgrade()
            .ok_or(crate::LifecycleError::InvalidTarget)?;
        form.update(cx, |form, cx| {
            form.discard_editor_at(self.path.target(), &self.editor, cx)
        })
    }

    pub fn editor_status(&self, cx: &App) -> Result<crate::EditorStatus, crate::LifecycleError> {
        let form = self
            .form
            .upgrade()
            .ok_or(crate::LifecycleError::InvalidTarget)?;
        form.read(cx)
            .editor_status_at(self.path.target(), &self.editor)
    }

    fn begin_native_edit<Owner>(&self, cx: &mut Context<Owner>) -> Option<crate::draft::EditTicket>
    where
        Owner: 'static,
    {
        let form = self.form.upgrade()?;
        if !self.path.is_current(form.read(cx)) {
            return None;
        }
        self.shared.advance_editor_sequence();
        let revision = form.read(cx).revision();
        if let Some(state) = self.state.upgrade() {
            let _ = (&mut *state.borrow_mut()).transition(Message::SuppressThrough(revision));
        }
        Some(self.edit_clock.reserve(
            self.shared.control_id,
            self.path.target().address,
            self.editor.clone(),
        ))
    }

    pub fn defer_set<Owner>(&self, value: T, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static,
    {
        self.defer_mapped_update(
            value.clone(),
            std::any::TypeId::of::<T>(),
            crate::StorageUpdate::new(move |_| Ok(value)),
            window,
            cx,
        );
    }

    pub(crate) fn defer_mapped_update<Owner: 'static, D: Clone + PartialEq + 'static>(
        &self,
        display: D,
        mapping: std::any::TypeId,
        update: crate::StorageUpdate<T>,
        window: &Window,
        cx: &mut Context<Owner>,
    ) {
        if !self.shared.active.load(Ordering::Acquire) {
            return;
        }
        let Some(ticket) = self.begin_native_edit(cx) else {
            return;
        };
        let editor_sequence = self.shared.editor_sequence.load(Ordering::Acquire);
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
            if !writer.path.is_current(form.read(cx)) || !ticket.accept() {
                return;
            }
            let origin = ControlOrigin {
                control_id: writer.shared.control_id,
                lifecycle_generation,
                editor_sequence,
            };
            form.update(cx, |form, cx| {
                let Ok(latest) = writer.path.stored(form) else {
                    // Resolution failures cancel an edit whose target no longer exists.
                    return;
                };
                let problem = match update.apply(latest) {
                    Ok(value) => match writer.path.set(form, value, &writer.editor, origin, cx) {
                        Ok(()) => return,
                        Err(error @ ResolveError::WouldRetireDrafts { .. }) => {
                            crate::EditProblem::new(
                                "form-edit-would-retire-drafts",
                                error.to_string(),
                            )
                        }
                        Err(_) => return, // Lifecycle cancellation must not create orphan drafts.
                    },
                    Err(problem) => problem,
                };
                let key = form
                    .topology()
                    .key(&writer.path.target().address)
                    .expect("admitted path remains active after a rejected candidate");
                let (code, message) = problem.into_parts();
                form.accept_draft(
                    key,
                    writer.editor,
                    writer.shared.control_id,
                    display,
                    mapping,
                    code,
                    message,
                    cx,
                );
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

    /// Submit unresolved display data to the Form, independently of native entity lifetime.
    pub fn defer_draft<Owner, D: Clone + PartialEq + 'static>(
        &self,
        display: D,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
        window: &Window,
        cx: &mut Context<Owner>,
    ) where
        Owner: 'static,
    {
        self.defer_mapped_draft(
            display,
            std::any::TypeId::of::<D>(),
            code,
            message,
            window,
            cx,
        );
    }

    pub fn defer_draft_for<Mapping: 'static, Owner: 'static, D: Clone + PartialEq + 'static>(
        &self,
        display: D,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
        window: &Window,
        cx: &mut Context<Owner>,
    ) {
        self.defer_mapped_draft(
            display,
            std::any::TypeId::of::<Mapping>(),
            code,
            message,
            window,
            cx,
        );
    }

    pub fn draft_for<Mapping: 'static, D: Clone + PartialEq + 'static>(
        &self,
        cx: &App,
    ) -> Result<Option<D>, crate::EditorTypeMismatch> {
        self.mapped_draft(std::any::TypeId::of::<Mapping>(), cx)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn defer_mapped_draft<Owner, D: Clone + PartialEq + 'static>(
        &self,
        display: D,
        mapping: std::any::TypeId,
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
        let Some(ticket) = self.begin_native_edit(cx) else {
            return;
        };
        let writer = self.clone();
        let code = code.into().into_owned();
        let generation = self.shared.lifecycle_generation.load(Ordering::Acquire);
        cx.defer_in(window, move |_, _, cx| {
            if !writer.shared.active.load(Ordering::Acquire)
                || generation != writer.shared.lifecycle_generation.load(Ordering::Acquire)
            {
                return;
            }
            let Some(form) = writer.form.upgrade() else {
                return;
            };
            if !writer.path.is_current(form.read(cx)) || !ticket.accept() {
                return;
            }
            let key = form
                .read(cx)
                .topology()
                .key(&writer.path.target().address)
                .expect("bound path");
            form.update(cx, |form, cx| {
                form.accept_draft(
                    key,
                    writer.editor.clone(),
                    writer.shared.control_id,
                    display,
                    mapping,
                    code,
                    message,
                    cx,
                )
            });
        });
    }

    pub fn draft<D: Clone + PartialEq + 'static>(
        &self,
        cx: &App,
    ) -> Result<Option<D>, crate::EditorTypeMismatch> {
        self.mapped_draft(std::any::TypeId::of::<D>(), cx)
    }

    pub(crate) fn mapped_draft<D: Clone + PartialEq + 'static>(
        &self,
        mapping: std::any::TypeId,
        cx: &App,
    ) -> Result<Option<D>, crate::EditorTypeMismatch> {
        let Some(form) = self.form.upgrade() else {
            return Ok(None);
        };
        let form = form.read(cx);
        let Some(key) = form.topology().key(&self.path.target().address) else {
            return Ok(None);
        };
        form.editor_draft(&key, &self.editor, mapping)
    }

    /// Reconnect to an application-stable editor identity before subscribing to native events.
    pub fn with_editor_key(mut self, editor: impl Into<crate::EditorKey>, cx: &mut App) -> Self {
        self.editor = editor.into();
        if let Some(form) = self.form.upgrade() {
            let key = form.read(cx).topology().key(&self.path.target().address);
            if let Some(key) = key {
                form.update(cx, |form, _| {
                    form.restore_editor(&key, &self.editor, self.shared.control_id)
                });
            }
        }
        self
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
                form.clear_editor_issue(&writer.path.target().address, &writer.editor, cx);
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
    let edit_clock = form.read(cx).edit_clock.clone();
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
        let change = match event {
            FormEvent::ModelChanged(change) | FormEvent::SessionChanged(change) => change,
            _ => return,
        };
        if !change.targets_control(subscription_shared.control_id) {
            return;
        }
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
        } else if change.kind() != crate::ModelChangeKind::Edit && impact.is_affected() {
            if !change.is_editor_scoped() {
                subscription_shared.advance_generation();
            }
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

    let refresh_form = weak_form.clone();
    let refresh_path = path.clone();
    let refresh_shared = shared.clone();
    let refresh_state = state.clone();
    let refresh_projector = projector.clone();
    let refresh_owner = owner.downgrade();
    let refresh_window = window.window_handle();
    let refresh = Box::new(move |cx: &mut App| {
        if let Some(form) = refresh_form.upgrade() {
            let effect = (&mut *refresh_state.borrow_mut()).transition(Message::RefreshValue {
                revision: form.read(cx).revision(),
                editor_sequence: refresh_shared.editor_sequence.load(Ordering::Acquire),
            });
            if matches!(effect, Effect::ScheduleDrain) {
                schedule_drain(
                    DrainRequest {
                        form: refresh_form.clone(),
                        path: refresh_path.clone(),
                        shared: refresh_shared.clone(),
                        state: refresh_state.clone(),
                        projector: refresh_projector.clone(),
                        owner: refresh_owner.clone(),
                        window: refresh_window,
                    },
                    cx,
                );
            }
        }
    });

    let discard_form = weak_form.clone();
    let discard_target = path.target();
    let status_form = weak_form.clone();
    let status_target = discard_target.clone();
    let writer = ControlWriter {
        form: weak_form,
        path,
        shared: shared.clone(),
        edit_clock: edit_clock.clone(),
        editor: crate::EditorKey::new(format!("control-{}", shared.control_id)),
        state: Rc::downgrade(&state),
    };
    (
        ControlBinding {
            _subscription: subscription,
            refresh,
            edit_clock,
            state,
            shared,
            discard_editor: Box::new(move |editor, cx| {
                let form = discard_form
                    .upgrade()
                    .ok_or(crate::LifecycleError::InvalidTarget)?;
                form.update(cx, |form, cx| {
                    form.discard_editor_at(discard_target.clone(), editor, cx)
                })
            }),
            editor_status: Box::new(move |editor, cx| {
                let form = status_form
                    .upgrade()
                    .ok_or(crate::LifecycleError::InvalidTarget)?;
                form.read(cx)
                    .editor_status_at(status_target.clone(), editor)
            }),
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
                            if form_ref.edit_clock.is_control_pending(shared.control_id)
                                || form_ref
                                    .drafts
                                    .values()
                                    .any(|d| d.control == shared.control_id)
                            {
                                return;
                            }
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
                    }
                    Effect::None | Effect::ScheduleDrain => {}
                }
            });
        });
    });
}
