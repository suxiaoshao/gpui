use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use gpui::{App, AppContext as _, TestAppContext};

use crate::{
    LocalStore, SharedStore, StoreBackend, StoreBackendCallback, StoreBackendId, StoreBinding,
    StoreCommitAck, StoreCommitBackend, StoreSelection, StoreState, test_support::NotifyCounter,
};

#[derive(Clone, Debug, Default, PartialEq)]
struct TestState {
    count: i32,
    label: String,
    external_value: i32,
}

impl StoreState for TestState {}

struct LocalOwner {
    store: LocalStore<TestState>,
}

#[gpui::test]
fn local_store_noop_update_does_not_notify_owner(cx: &mut TestAppContext) {
    let (owner, counter) = cx.update(|cx| {
        let owner = cx.new(|_| LocalOwner {
            store: LocalStore::new(TestState::default()),
        });
        let counter = cx.new(|cx| NotifyCounter::new(owner.clone(), cx));
        (owner, counter)
    });

    let update = cx.update(|cx| {
        owner.update(cx, |owner, cx| {
            owner.store.set(cx, |state| &mut state.count, 0)
        })
    });

    assert!(!update.changed_state());
    assert_eq!(update.revision().get(), 0);
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        0
    );
}

#[gpui::test]
fn local_store_changed_update_notifies_owner(cx: &mut TestAppContext) {
    let (owner, counter) = cx.update(|cx| {
        let owner = cx.new(|_| LocalOwner {
            store: LocalStore::new(TestState::default()),
        });
        let counter = cx.new(|cx| NotifyCounter::new(owner.clone(), cx));
        (owner, counter)
    });

    let update = cx.update(|cx| {
        owner.update(cx, |owner, cx| {
            owner.store.set(cx, |state| &mut state.count, 1)
        })
    });

    assert!(update.changed_state());
    assert_eq!(update.revision().get(), 1);
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );
}

#[gpui::test]
fn shared_store_noop_update_does_not_bump_revision_or_notify(cx: &mut TestAppContext) {
    let (store, counter) = cx.update(|cx| {
        let store = SharedStore::new(cx, TestState::default());
        let counter = cx.new(|cx| NotifyCounter::new(store.entity(), cx));
        (store, counter)
    });

    let update = cx.update(|cx| store.set(cx, |state| &mut state.count, 0));

    assert!(!update.changed_state());
    assert_eq!(update.revision().get(), 0);
    assert_eq!(cx.update(|cx| store.revision(cx).get()), 0);
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        0
    );
}

#[gpui::test]
fn shared_store_changed_update_bumps_revision_and_notifies(cx: &mut TestAppContext) {
    let (store, counter) = cx.update(|cx| {
        let store = SharedStore::new(cx, TestState::default());
        let counter = cx.new(|cx| NotifyCounter::new(store.entity(), cx));
        (store, counter)
    });

    let update = cx.update(|cx| store.set(cx, |state| &mut state.count, 1));

    assert!(update.changed_state());
    assert_eq!(update.revision().get(), 1);
    assert_eq!(cx.update(|cx| store.revision(cx).get()), 1);
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );
}

struct SelectionOwner {
    count: StoreSelection<i32>,
}

#[gpui::test]
fn selection_notifies_owner_only_when_snapshot_changes(cx: &mut TestAppContext) {
    let (store, owner, counter) = cx.update(|cx| {
        let store = SharedStore::new(cx, TestState::default());
        let owner = cx.new(|cx| SelectionOwner {
            count: store.select(cx, |state| state.count),
        });
        let counter = cx.new(|cx| NotifyCounter::new(owner.clone(), cx));
        (store, owner, counter)
    });

    cx.update(|cx| {
        store.set(cx, |state| &mut state.label, "changed".to_string());
    });

    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.count.read(|count| *count))),
        0
    );
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        0
    );

    cx.update(|cx| {
        store.set(cx, |state| &mut state.count, 7);
    });

    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.count.read(|count| *count))),
        7
    );
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );
}

struct BindingOwner {
    count: StoreBinding<i32>,
}

struct LabelBindingOwner {
    label: StoreBinding<String>,
}

#[gpui::test]
fn binding_writes_back_to_store_and_refreshes_snapshot(cx: &mut TestAppContext) {
    let (store, owner, counter) = cx.update(|cx| {
        let store = SharedStore::new(cx, TestState::default());
        let owner = cx.new(|cx| BindingOwner {
            count: store.bind(cx, |state| state.count, |state, count| state.count = count),
        });
        let counter = cx.new(|cx| NotifyCounter::new(owner.clone(), cx));
        (store, owner, counter)
    });

    let update = cx.update(|cx| owner.update(cx, |owner, cx| owner.count.set(cx, 9)));

    assert!(update.changed_state());
    assert_eq!(cx.update(|cx| store.read(cx, |state| state.count)), 9);
    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.count.read(|count| *count))),
        9
    );
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );

    let update = cx.update(|cx| owner.update(cx, |owner, cx| owner.count.set(cx, 9)));
    assert!(!update.changed_state());
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );
}

#[gpui::test]
fn binding_owned_snapshot_survives_store_replacement(cx: &mut TestAppContext) {
    let owner = cx.update(|cx| {
        let store = SharedStore::new(
            cx,
            TestState {
                label: "old".to_string(),
                ..TestState::default()
            },
        );
        cx.new(|cx| LabelBindingOwner {
            label: store.bind(
                cx,
                |state| state.label.clone(),
                |state, label| state.label = label,
            ),
        })
    });

    let old_snapshot = cx.update(|cx| owner.read_with(cx, |owner, _| owner.label.snapshot()));

    cx.update(|cx| {
        owner.update(cx, |owner, cx| {
            owner.label.set(cx, "new".to_string());
        });
    });

    assert_eq!(old_snapshot.as_str(), "old");
    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.label.cloned())),
        "new"
    );
}

struct LabelSelectionOwner {
    label: StoreSelection<String>,
}

#[gpui::test]
fn selection_owned_snapshot_survives_store_replacement(cx: &mut TestAppContext) {
    let (store, owner) = cx.update(|cx| {
        let store = SharedStore::new(
            cx,
            TestState {
                label: "old".to_string(),
                ..TestState::default()
            },
        );
        let owner = cx.new(|cx| LabelSelectionOwner {
            label: store.select(cx, |state| state.label.clone()),
        });
        (store, owner)
    });

    let old_snapshot = cx.update(|cx| owner.read_with(cx, |owner, _| owner.label.snapshot()));

    cx.update(|cx| {
        store.set(cx, |state| &mut state.label, "new".to_string());
    });

    assert_eq!(old_snapshot.as_str(), "old");
    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.label.cloned())),
        "new"
    );
}

#[derive(Clone)]
struct FakeBackend {
    inner: Rc<RefCell<FakeBackendInner>>,
}

struct FakeBackendInner {
    snapshot: i32,
    callbacks: Vec<StoreBackendCallback<()>>,
    reconcile_calls: usize,
    commit_calls: usize,
}

impl FakeBackend {
    fn new(snapshot: i32) -> Self {
        Self {
            inner: Rc::new(RefCell::new(FakeBackendInner {
                snapshot,
                callbacks: Vec::new(),
                reconcile_calls: 0,
                commit_calls: 0,
            })),
        }
    }

    fn set_snapshot(&self, snapshot: i32) {
        self.inner.borrow_mut().snapshot = snapshot;
    }

    fn emit(&self, cx: &mut App) {
        let mut callbacks = {
            let mut inner = self.inner.borrow_mut();
            std::mem::take(&mut inner.callbacks)
        };

        for callback in &mut callbacks {
            callback((), cx);
        }

        self.inner.borrow_mut().callbacks = callbacks;
    }

    fn reconcile_calls(&self) -> usize {
        self.inner.borrow().reconcile_calls
    }

    fn commit_calls(&self) -> usize {
        self.inner.borrow().commit_calls
    }
}

impl StoreBackend<TestState> for FakeBackend {
    type Snapshot = i32;
    type Event = ();
    type Subscription = ();
    type Error = String;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new("fake")
    }

    fn load(&self) -> Result<Option<Self::Snapshot>, Self::Error> {
        Ok(Some(self.inner.borrow().snapshot))
    }

    fn subscribe(
        &self,
        on_change: StoreBackendCallback<Self::Event>,
    ) -> Result<Option<Self::Subscription>, Self::Error> {
        self.inner.borrow_mut().callbacks.push(on_change);
        Ok(Some(()))
    }

    fn reconcile(&self, state: &mut TestState, snapshot: Self::Snapshot) -> bool {
        self.inner.borrow_mut().reconcile_calls += 1;
        if state.external_value == snapshot {
            return false;
        }

        state.external_value = snapshot;
        true
    }
}

#[derive(Clone)]
struct FakeCommitBackend {
    backend: FakeBackend,
}

impl FakeCommitBackend {
    fn new(snapshot: i32) -> Self {
        Self {
            backend: FakeBackend::new(snapshot),
        }
    }

    fn emit(&self, cx: &mut App) {
        self.backend.emit(cx);
    }

    fn reconcile_calls(&self) -> usize {
        self.backend.reconcile_calls()
    }

    fn commit_calls(&self) -> usize {
        self.backend.commit_calls()
    }
}

impl StoreBackend<TestState> for FakeCommitBackend {
    type Snapshot = i32;
    type Event = ();
    type Subscription = ();
    type Error = String;

    fn backend_id(&self) -> StoreBackendId {
        self.backend.backend_id()
    }

    fn load(&self) -> Result<Option<Self::Snapshot>, Self::Error> {
        self.backend.load()
    }

    fn subscribe(
        &self,
        on_change: StoreBackendCallback<Self::Event>,
    ) -> Result<Option<Self::Subscription>, Self::Error> {
        self.backend.subscribe(on_change)
    }

    fn reconcile(&self, state: &mut TestState, snapshot: Self::Snapshot) -> bool {
        self.backend.reconcile(state, snapshot)
    }
}

impl StoreCommitBackend<TestState> for FakeCommitBackend {
    fn commit_snapshot(
        &self,
        draft: &TestState,
    ) -> Result<Option<StoreCommitAck<Self::Snapshot>>, Self::Error> {
        let mut inner = self.backend.inner.borrow_mut();
        inner.commit_calls += 1;
        inner.snapshot = draft.count;
        Ok(Some(StoreCommitAck::with_snapshot(inner.snapshot)))
    }
}

#[gpui::test]
fn backend_equal_event_does_not_reconcile_or_notify(cx: &mut TestAppContext) {
    let fake = FakeBackend::new(5);
    let (store, counter) = cx.update(|cx| {
        let store = SharedStore::new_with_backend(cx, TestState::default(), fake.clone()).unwrap();
        let counter = cx.new(|cx| NotifyCounter::new(store.entity(), cx));
        (store, counter)
    });
    assert_eq!(fake.reconcile_calls(), 1);

    cx.update(|cx| fake.emit(cx));

    assert_eq!(fake.reconcile_calls(), 1);
    assert_eq!(
        cx.update(|cx| store.read(cx, |state| state.external_value)),
        5
    );
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        0
    );
}

#[gpui::test]
fn backend_changed_event_reconciles_and_notifies(cx: &mut TestAppContext) {
    let fake = FakeBackend::new(5);
    let (store, counter) = cx.update(|cx| {
        let store = SharedStore::new_with_backend(cx, TestState::default(), fake.clone()).unwrap();
        let counter = cx.new(|cx| NotifyCounter::new(store.entity(), cx));
        (store, counter)
    });

    fake.set_snapshot(8);
    cx.update(|cx| fake.emit(cx));

    assert_eq!(fake.reconcile_calls(), 2);
    assert_eq!(
        cx.update(|cx| store.read(cx, |state| state.external_value)),
        8
    );
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );
}

#[gpui::test]
fn committed_backend_commit_runs_once_and_filters_ack_snapshot(cx: &mut TestAppContext) {
    let fake = FakeCommitBackend::new(0);
    let store = cx.update(|cx| {
        SharedStore::new_with_backend(cx, TestState::default(), fake.clone()).unwrap()
    });

    cx.update(|cx| {
        store.try_set(cx, |state| &mut state.count, 11).unwrap();
    });

    assert_eq!(fake.commit_calls(), 1);
    assert_eq!(fake.reconcile_calls(), 2);
    assert_eq!(
        cx.update(|cx| {
            store
                .entity()
                .read_with(cx, |runtime, _| runtime.last_external_snapshot().copied())
        }),
        Some(11)
    );

    cx.update(|cx| fake.emit(cx));

    assert_eq!(fake.reconcile_calls(), 2);
    assert_eq!(fake.commit_calls(), 1);
}

struct LocalBackendOwner {
    store: LocalStore<TestState, FakeBackend>,
}

#[gpui::test]
fn local_store_can_load_from_backend(cx: &mut TestAppContext) {
    let fake = FakeBackend::new(6);
    let owner = cx.update(|cx| {
        cx.new(|cx| LocalBackendOwner {
            store: LocalStore::with_backend(cx, TestState::default(), fake.clone()).unwrap(),
        })
    });

    assert_eq!(fake.reconcile_calls(), 1);
    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.store.read().external_value)),
        6
    );
}

#[gpui::test]
fn local_store_backend_event_uses_owner_accessor(cx: &mut TestAppContext) {
    let fake = FakeBackend::new(6);
    let (owner, counter) = cx.update(|cx| {
        let owner = cx.new(|cx| {
            let mut store =
                LocalStore::with_backend(cx, TestState::default(), fake.clone()).unwrap();
            store
                .subscribe(cx, |owner: &mut LocalBackendOwner| &mut owner.store)
                .unwrap();
            LocalBackendOwner { store }
        });
        let counter = cx.new(|cx| NotifyCounter::new(owner.clone(), cx));
        (owner, counter)
    });

    fake.set_snapshot(10);
    cx.update(|cx| fake.emit(cx));

    assert_eq!(fake.reconcile_calls(), 2);
    assert_eq!(
        cx.update(|cx| owner.read_with(cx, |owner, _| owner.store.read().external_value)),
        10
    );
    assert_eq!(
        cx.update(|cx| counter.read_with(cx, |counter, _| counter.count())),
        1
    );
}

static ASSERT_SEND_SYNC_NOT_REQUIRED: AtomicUsize = AtomicUsize::new(0);

#[test]
fn backend_does_not_require_send_or_sync() {
    ASSERT_SEND_SYNC_NOT_REQUIRED.store(1, Ordering::SeqCst);
    assert_eq!(ASSERT_SEND_SYNC_NOT_REQUIRED.load(Ordering::SeqCst), 1);
}
