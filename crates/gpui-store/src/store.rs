use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{App, AppContext, Context, Entity, Global, Subscription, WeakEntity, Window};

use crate::{
    StoreChange,
    observation::{self, ObservationPhase},
    select::Select,
    selection::StoreSelection,
};

#[allow(clippy::type_complexity)]
type WholeObserver<Owner, S> =
    Rc<RefCell<Option<Box<dyn FnMut(&mut Owner, &S, &mut Context<Owner>)>>>>;

#[allow(clippy::type_complexity)]
type SelectedObserver<Owner, T> =
    Rc<RefCell<Option<Box<dyn FnMut(&mut Owner, &T, &mut Context<Owner>)>>>>;

#[allow(clippy::type_complexity)]
type WindowObserver<Owner, T> =
    Rc<RefCell<Option<Box<dyn FnMut(&mut Owner, &T, &mut Window, &mut Context<Owner>)>>>>;

// ── Internal state ──────────────────────────────────────────────────────

pub(crate) struct StoreInner<S> {
    state: Rc<RefCell<S>>,
}

impl<S> StoreInner<S> {
    pub(crate) fn new(state: S) -> Self {
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }

    pub(crate) fn state_cell(&self) -> Rc<RefCell<S>> {
        self.state.clone()
    }
}

// ── Public handle ───────────────────────────────────────────────────────

/// A handle to a single authoritative, type-safe, in-memory state.
///
/// `Store<S>` is cheap to clone — all clones share the same state. Mutations
/// are performed through [`set`], [`update`], and [`update_if`]. Read-only
/// derived views are provided by [`select`] and [`observe`].
///
/// [`set`]: Store::set
/// [`update`]: Store::update
/// [`update_if`]: Store::update_if
/// [`select`]: Store::select
/// [`observe`]: Store::observe
pub struct Store<S: 'static> {
    entity: Entity<StoreInner<S>>,
}

impl<S: 'static> Clone for Store<S> {
    fn clone(&self) -> Self {
        Self {
            entity: self.entity.clone(),
        }
    }
}

impl<S: 'static> Global for Store<S> {}

// ── Construction and global ─────────────────────────────────────────────

impl<S: 'static> Store<S> {
    /// Creates a new `Store<S>`.
    #[must_use]
    pub fn new(cx: &mut impl AppContext, state: S) -> Self {
        Self {
            entity: cx.new(|_| StoreInner::new(state)),
        }
    }

    /// Creates a new `Store<S>` and installs it as the typed global for `S`.
    pub fn install_global(cx: &mut App, state: S) -> Self {
        let store = Self::new(cx, state);
        cx.set_global(store.clone());
        store
    }

    /// Retrieves the typed-global `Store<S>`. Panics if not installed.
    #[must_use]
    pub fn global(cx: &impl AppContext) -> Self {
        cx.read_global(|store: &Self, _| store.clone())
    }
}

// ── Read ────────────────────────────────────────────────────────────────

impl<S: 'static> Store<S> {
    /// Synchronously reads a projection of the current state without cloning
    /// `S`.
    pub fn read<R>(&self, cx: &impl AppContext, read: impl FnOnce(&S) -> R) -> R {
        self.entity
            .read_with(cx, |inner, _| read(&inner.state.borrow()))
    }
}

// ── Mutation ────────────────────────────────────────────────────────────

impl<S: 'static> Store<S> {
    /// Replaces the entire state and notifies observers.
    ///
    /// The old `S` is moved out of the entity lease before being dropped,
    /// so a panic in its destructor cannot corrupt the entity slot.
    pub fn set(&self, cx: &mut impl AppContext, state: S) {
        let old_state = self.entity.update(cx, |inner, cx| {
            let old = inner.state.replace(state);
            cx.notify();
            old
        });
        drop(old_state);
    }

    /// Mutates the state in place and always notifies observers.
    pub fn update<R>(&self, cx: &mut impl AppContext, update: impl FnOnce(&mut S) -> R) -> R {
        self.entity.update(cx, |inner, cx| {
            let result = update(&mut inner.state.borrow_mut());
            cx.notify();
            result
        })
    }

    /// Mutates the state and only notifies when the caller returns
    /// [`StoreChange::Changed`].
    ///
    /// When [`StoreChange::Unchanged`] is returned the mutation is
    /// **not** rolled back — the caller commits to having made no
    /// observable change.
    pub fn update_if<R>(
        &self,
        cx: &mut impl AppContext,
        update: impl FnOnce(&mut S) -> StoreChange<R>,
    ) -> StoreChange<R> {
        self.entity.update(cx, |inner, cx| {
            let outcome = update(&mut inner.state.borrow_mut());
            if outcome.is_changed() {
                cx.notify();
            }
            outcome
        })
    }
}

// ── Selection ───────────────────────────────────────────────────────────

impl<S: 'static> Store<S> {
    /// Creates an owner-bound [`StoreSelection`] that stays in sync with the
    /// store and notifies the owner only when the selected value changes.
    pub fn select<Owner, Selector>(
        &self,
        cx: &mut Context<Owner>,
        selector: Selector,
    ) -> StoreSelection<Selector::Output>
    where
        Owner: 'static,
        Selector: Select<S> + 'static,
        Selector::Output: PartialEq + 'static,
    {
        let selector = Rc::new(selector);
        let initial = self
            .entity
            .read_with(cx, |inner, _| selector.select(&inner.state.borrow()));
        let snapshot = Rc::new(SelectionCell::new(initial));

        let observed_snapshot = snapshot.clone();
        let observed_selector = selector.clone();
        let entity = self.entity.clone();
        let subscription = cx.observe(&entity, move |_owner, observed, cx| {
            let next = observed.read_with(cx, |inner, _| {
                observed_selector.select(&inner.state.borrow())
            });
            if observed_snapshot.read(|current| current != &next) {
                observed_snapshot.replace(next);
                cx.notify();
            }
        });

        StoreSelection {
            snapshot,
            _subscription: subscription,
        }
    }
}

// ── Observation ─────────────────────────────────────────────────────────

impl<S: 'static> Store<S> {
    /// Observes the entire store state with a guaranteed initial delivery.
    ///
    /// The callback receives `(&mut Owner, &S, &mut Context<Owner>)` on each
    /// delivery, including the initial one.
    pub fn observe<Owner>(
        &self,
        cx: &mut Context<Owner>,
        observe: impl FnMut(&mut Owner, &S, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
    {
        let observer: WholeObserver<Owner, S> = Rc::new(RefCell::new(Some(Box::new(observe))));
        let phase = Rc::new(Cell::new(ObservationPhase::Pending));

        let phase_guard = phase.clone();
        let observer_guard = observer.clone();

        let source_sub = observation::observe_whole(self, cx, observer.clone(), phase.clone());

        let guard = Subscription::new(move || {
            phase_guard.set(ObservationPhase::Cancelled);
            *observer_guard.borrow_mut() = None;
        });

        Subscription::join(source_sub, guard)
    }

    /// Observes a selected projection with a guaranteed initial delivery.
    ///
    /// The callback only fires when the selected value changes (according
    /// to `PartialEq`).
    pub fn observe_select<Owner, Selector>(
        &self,
        cx: &mut Context<Owner>,
        selector: Selector,
        observe: impl FnMut(&mut Owner, &Selector::Output, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        Selector: Select<S> + 'static,
        Selector::Output: PartialEq + 'static,
    {
        let selector = Rc::new(selector);
        let observer: SelectedObserver<Owner, Selector::Output> =
            Rc::new(RefCell::new(Some(Box::new(observe))));
        let phase = Rc::new(Cell::new(ObservationPhase::Pending));
        let current: Rc<RefCell<Option<Selector::Output>>> = Rc::new(RefCell::new(None));

        let phase_guard = phase.clone();
        let observer_guard = observer.clone();

        let source_sub = observation::observe_selected(
            self,
            cx,
            selector,
            observer.clone(),
            phase.clone(),
            current,
        );

        let guard = Subscription::new(move || {
            phase_guard.set(ObservationPhase::Cancelled);
            *observer_guard.borrow_mut() = None;
        });

        Subscription::join(source_sub, guard)
    }

    /// Window-aware whole-store observation with guaranteed initial delivery.
    ///
    /// Like [`observe`] but provides a `&mut Window` in the callback and
    /// automatically cancels when the target window is closed.
    ///
    /// [`observe`]: Store::observe
    pub fn observe_in<Owner>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        observe: impl FnMut(&mut Owner, &S, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
    {
        let observer: WindowObserver<Owner, S> = Rc::new(RefCell::new(Some(Box::new(observe))));
        let phase = Rc::new(Cell::new(ObservationPhase::Pending));

        let window_id = window.window_handle().window_id();
        let window_phase = phase.clone();
        let window_close_sub = cx.on_window_closed(move |_app, closed_id| {
            if closed_id == window_id {
                window_phase.set(ObservationPhase::Cancelled);
            }
        });

        let source_phase = phase.clone();
        let source_observer = observer.clone();
        let source_entity = self.entity.clone();
        let source_sub = cx.observe_in(
            &source_entity,
            window,
            move |owner, observed, window, cx| {
                if source_phase.get() != ObservationPhase::Active {
                    return;
                }
                let state_rc = observed.read_with(cx, |inner, _| inner.state_cell());
                let state = state_rc.borrow();
                let mut callback = source_observer.borrow_mut().take();
                if let Some(ref mut observe) = callback {
                    observe(owner, &state, window, cx);
                }
                drop(state);
                if source_phase.get() == ObservationPhase::Active
                    && let Some(observe) = callback
                {
                    *source_observer.borrow_mut() = Some(observe);
                }
            },
        );

        let weak_store = self.downgrade();
        let initial_phase = phase.clone();
        let initial_observer = observer.clone();
        cx.defer_in(window, move |owner, window, cx| {
            if initial_phase.get() == ObservationPhase::Cancelled {
                return;
            }
            let Some(store_entity) = weak_store.upgrade() else {
                initial_phase.set(ObservationPhase::Cancelled);
                return;
            };
            initial_phase.set(ObservationPhase::Active);
            let state_rc = store_entity.read_with(cx, |inner, _| inner.state_cell());
            let state = state_rc.borrow();
            let mut callback = initial_observer.borrow_mut().take();
            if let Some(ref mut observe) = callback {
                observe(owner, &state, window, cx);
            }
            drop(state);
            if initial_phase.get() == ObservationPhase::Active
                && let Some(observe) = callback
            {
                *initial_observer.borrow_mut() = Some(observe);
            }
        });

        let phase_guard = phase.clone();
        let observer_guard = observer.clone();
        let guard = Subscription::new(move || {
            phase_guard.set(ObservationPhase::Cancelled);
            *observer_guard.borrow_mut() = None;
        });

        Subscription::join(Subscription::join(source_sub, window_close_sub), guard)
    }

    /// Window-aware selected observation with guaranteed initial delivery.
    ///
    /// Like [`observe_select`] but provides a `&mut Window` in the callback
    /// and automatically cancels when the target window is closed.
    ///
    /// [`observe_select`]: Store::observe_select
    pub fn observe_select_in<Owner, Selector>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        selector: Selector,
        observe: impl FnMut(&mut Owner, &Selector::Output, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        Selector: Select<S> + 'static,
        Selector::Output: PartialEq + 'static,
    {
        let selector = Rc::new(selector);
        let phase = Rc::new(Cell::new(ObservationPhase::Pending));
        let current: Rc<RefCell<Option<Selector::Output>>> = Rc::new(RefCell::new(None));

        // Register window-close guard
        let window_id = window.window_handle().window_id();
        let window_phase = phase.clone();
        let window_close_sub = cx.on_window_closed(move |_app, closed_id| {
            if closed_id == window_id {
                window_phase.set(ObservationPhase::Cancelled);
            }
        });

        // Source observer
        let observer: WindowObserver<Owner, Selector::Output> =
            Rc::new(RefCell::new(Some(Box::new(observe))));

        let source_phase = phase.clone();
        let source_observer = observer.clone();
        let source_selector = selector.clone();
        let source_current = current.clone();
        let source_entity = self.entity.clone();

        let source_sub = cx.observe_in(
            &source_entity,
            window,
            move |owner, observed, window, cx| {
                match source_phase.get() {
                    ObservationPhase::Pending => return,
                    ObservationPhase::Cancelled => return,
                    ObservationPhase::Active => {}
                }
                let next = observed
                    .read_with(cx, |inner, _| source_selector.select(&inner.state.borrow()));
                let mut current_guard = source_current.borrow_mut();
                let changed = current_guard.as_ref() != Some(&next);
                if changed {
                    *current_guard = Some(next);
                    drop(current_guard);
                    let mut callback = source_observer.borrow_mut().take();
                    if let Some(ref mut observer) = callback {
                        let guard = source_current.borrow();
                        if let Some(ref current_value) = *guard {
                            observer(owner, current_value, window, cx);
                        }
                    }
                    #[allow(clippy::collapsible_if)]
                    if source_phase.get() == ObservationPhase::Active {
                        if let Some(observer) = callback {
                            *source_observer.borrow_mut() = Some(observer);
                        }
                    }
                }
            },
        );

        // Initial delivery via defer_in
        let weak_store = self.downgrade();
        let initial_phase = phase.clone();
        let initial_observer = observer.clone();
        let initial_selector = selector.clone();
        let initial_current = current.clone();

        cx.defer_in(window, move |owner, window, cx| {
            if initial_phase.get() == ObservationPhase::Cancelled {
                return;
            }
            let Some(store_entity) = weak_store.upgrade() else {
                initial_phase.set(ObservationPhase::Cancelled);
                return;
            };
            initial_phase.set(ObservationPhase::Active);
            let output = store_entity.read_with(cx, |inner, _| {
                initial_selector.select(&inner.state.borrow())
            });
            *initial_current.borrow_mut() = Some(output);
            let mut callback = initial_observer.borrow_mut().take();
            if let Some(ref mut observer) = callback {
                let guard = initial_current.borrow();
                if let Some(ref current_value) = *guard {
                    observer(owner, current_value, window, cx);
                }
            }
            #[allow(clippy::collapsible_if)]
            if initial_phase.get() == ObservationPhase::Active {
                if let Some(observer) = callback {
                    *initial_observer.borrow_mut() = Some(observer);
                }
            }
        });

        let phase_guard = phase.clone();
        let observer_guard = observer.clone();
        let guard = Subscription::new(move || {
            phase_guard.set(ObservationPhase::Cancelled);
            *observer_guard.borrow_mut() = None;
        });

        Subscription::join(Subscription::join(source_sub, window_close_sub), guard)
    }
}

// ── Internal helpers ────────────────────────────────────────────────────

impl<S: 'static> Store<S> {
    pub(crate) fn entity(&self) -> &Entity<StoreInner<S>> {
        &self.entity
    }

    pub(crate) fn downgrade(&self) -> WeakEntity<StoreInner<S>> {
        self.entity.downgrade()
    }
}

// ── Selection cell (internal) ───────────────────────────────────────────

pub(crate) struct SelectionCell<T> {
    value: RefCell<T>,
}

impl<T> SelectionCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: RefCell::new(value),
        }
    }

    pub(crate) fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.value.borrow())
    }

    pub(crate) fn replace(&self, value: T) {
        *self.value.borrow_mut() = value;
    }
}
