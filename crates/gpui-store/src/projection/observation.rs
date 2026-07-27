use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{App, Context, Subscription};

use super::Select;
use crate::store::Store;

#[allow(clippy::type_complexity)]
type Observer<Owner, S> = Rc<RefCell<Option<Box<dyn FnMut(&mut Owner, &S, &mut Context<Owner>)>>>>;

#[allow(clippy::type_complexity)]
type SelectedObserver<Owner, T> =
    Rc<RefCell<Option<Box<dyn FnMut(&mut Owner, &T, &mut Context<Owner>)>>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObservationPhase {
    Pending,
    Active,
    Cancelled,
}

/// Calls a whole-store observer while releasing the `RefCell` borrow so that
/// the callback can safely cancel itself (which would re-borrow the same
/// cell).
fn call_whole<Owner, S>(
    observer_rc: &Observer<Owner, S>,
    phase: &Rc<Cell<ObservationPhase>>,
    owner: &mut Owner,
    state: &S,
    cx: &mut Context<Owner>,
) {
    let mut callback = observer_rc.borrow_mut().take();
    if let Some(ref mut observer) = callback {
        observer(owner, state, cx);
    }
    #[allow(clippy::collapsible_if)]
    if phase.get() == ObservationPhase::Active {
        if let Some(observer) = callback {
            *observer_rc.borrow_mut() = Some(observer);
        }
    }
}

/// Calls a selected observer while releasing the `RefCell` borrow.
fn call_selected<Owner, T>(
    observer_rc: &SelectedObserver<Owner, T>,
    phase: &Rc<Cell<ObservationPhase>>,
    owner: &mut Owner,
    value: &T,
    cx: &mut Context<Owner>,
) {
    let mut callback = observer_rc.borrow_mut().take();
    if let Some(ref mut observer) = callback {
        observer(owner, value, cx);
    }
    #[allow(clippy::collapsible_if)]
    if phase.get() == ObservationPhase::Active {
        if let Some(observer) = callback {
            *observer_rc.borrow_mut() = Some(observer);
        }
    }
}

pub(crate) fn observe_whole<S: 'static, Owner: 'static>(
    store: &Store<S>,
    cx: &mut Context<Owner>,
    observer: Observer<Owner, S>,
    phase: Rc<Cell<ObservationPhase>>,
) -> Subscription {
    let source_phase = phase.clone();
    let source_observer = observer.clone();
    let source_entity = store.entity().clone();

    let source_sub = cx.observe(&source_entity, move |owner, observed, cx| {
        match source_phase.get() {
            ObservationPhase::Pending => return,
            ObservationPhase::Cancelled => return,
            ObservationPhase::Active => {}
        }
        let state_rc = observed.read_with(cx, |inner, _| inner.state_cell());
        let state = state_rc.borrow();
        call_whole(&source_observer, &source_phase, owner, &state, cx);
    });

    let weak_store = store.downgrade();
    let weak_owner = cx.weak_entity();
    let initial_phase = phase.clone();
    let initial_observer = observer.clone();

    cx.defer(move |app: &mut App| {
        if initial_phase.get() == ObservationPhase::Cancelled {
            return;
        }
        let Some(store_entity) = weak_store.upgrade() else {
            initial_phase.set(ObservationPhase::Cancelled);
            return;
        };
        match weak_owner.update(app, |owner, cx| {
            if initial_phase.get() == ObservationPhase::Cancelled {
                return;
            }
            initial_phase.set(ObservationPhase::Active);
            let state_rc = store_entity.read_with(cx, |inner, _| inner.state_cell());
            let state = state_rc.borrow();
            call_whole(&initial_observer, &initial_phase, owner, &state, cx);
        }) {
            Ok(()) => {}
            Err(_) => {
                initial_phase.set(ObservationPhase::Cancelled);
            }
        }
    });

    source_sub
}

pub(crate) fn observe_selected<S, Owner, Selector>(
    store: &Store<S>,
    cx: &mut Context<Owner>,
    selector: Rc<Selector>,
    observer: SelectedObserver<Owner, Selector::Output>,
    phase: Rc<Cell<ObservationPhase>>,
    current: Rc<RefCell<Option<Selector::Output>>>,
) -> Subscription
where
    S: 'static,
    Owner: 'static,
    Selector: Select<S> + 'static,
    Selector::Output: PartialEq + 'static,
{
    let source_phase = phase.clone();
    let source_observer = observer.clone();
    let source_selector = selector.clone();
    let source_current = current.clone();
    let source_entity = store.entity().clone();

    let source_sub = cx.observe(&source_entity, move |owner, observed, cx| {
        if source_phase.get() != ObservationPhase::Active {
            return;
        }
        let state_rc = observed.read_with(cx, |inner, _| inner.state_cell());
        let state = state_rc.borrow();
        let next = source_selector.select(&state);
        drop(state);

        let mut current_guard = source_current.borrow_mut();
        let changed = current_guard.as_ref() != Some(&next);

        if changed {
            *current_guard = Some(next);
            drop(current_guard);
            let guard = source_current.borrow();
            if let Some(ref current_value) = *guard {
                call_selected(&source_observer, &source_phase, owner, current_value, cx);
            }
        }
    });

    let weak_store = store.downgrade();
    let weak_owner = cx.weak_entity();
    let initial_phase = phase.clone();
    let initial_observer = observer.clone();
    let initial_selector = selector.clone();
    let initial_current = current.clone();

    cx.defer(move |app: &mut App| {
        if initial_phase.get() == ObservationPhase::Cancelled {
            return;
        }
        let Some(store_entity) = weak_store.upgrade() else {
            initial_phase.set(ObservationPhase::Cancelled);
            return;
        };
        match weak_owner.update(app, |owner, cx| {
            if initial_phase.get() == ObservationPhase::Cancelled {
                return;
            }
            initial_phase.set(ObservationPhase::Active);
            let state_rc = store_entity.read_with(cx, |inner, _| inner.state_cell());
            let state = state_rc.borrow();
            let output = initial_selector.select(&state);
            drop(state);
            *initial_current.borrow_mut() = Some(output);
            let guard = initial_current.borrow();
            if let Some(ref current_value) = *guard {
                call_selected(&initial_observer, &initial_phase, owner, current_value, cx);
            }
        }) {
            Ok(()) => {}
            Err(_) => {
                initial_phase.set(ObservationPhase::Cancelled);
            }
        }
    });

    source_sub
}
