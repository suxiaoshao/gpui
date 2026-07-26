use std::{cell::Cell, rc::Rc};

use gpui::{AppContext as _, Entity, Subscription};

use crate::StoreSelection;

// ── Shared test types ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
struct AppState {
    value: i32,
    label: String,
}

#[derive(Debug)]
struct DropProbe(Rc<Cell<bool>>);

impl DropProbe {
    fn new() -> (Self, Rc<Cell<bool>>) {
        let dropped = Rc::new(Cell::new(false));
        (Self(dropped.clone()), dropped)
    }
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

struct NotifyCounter {
    count: Rc<Cell<usize>>,
    _subscription: Subscription,
}

impl NotifyCounter {
    fn observing<T: 'static>(
        target: &Entity<T>,
        count: Rc<Cell<usize>>,
        cx: &mut gpui::App,
    ) -> Entity<Self> {
        let t = target.clone();
        let c = count.clone();
        cx.new(|cx| {
            let subscription = cx.observe(&t, move |this: &mut NotifyCounter, _: Entity<T>, _| {
                this.count.set(this.count.get() + 1);
            });
            Self {
                count: c,
                _subscription: subscription,
            }
        })
    }
}

// ── Owner entities ──────────────────────────────────────────────────────

struct SelectionOwner {
    selection: StoreSelection<i32>,
}

struct OptionalSelectionOwner {
    selection: Option<StoreSelection<i32>>,
}

struct ObservedOwner {
    _subscription: Option<Subscription>,
}

// ── core ────────────────────────────────────────────────────────────────

mod core {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use gpui::AppContext as _;

    use crate::tests::{AppState, DropProbe};
    use crate::{Select, Store, StoreChange};

    struct Owner {
        _subscription: Option<gpui::Subscription>,
    }

    #[gpui::test]
    fn store_clone_shares_non_clone_state(cx: &mut gpui::TestAppContext) {
        #[derive(Debug)]
        struct NonCloneState {
            value: i32,
            label: String,
            _probe: DropProbe,
        }

        let (probe, dropped_flag) = DropProbe::new();
        let state = NonCloneState {
            value: 42,
            label: "hello".into(),
            _probe: probe,
        };

        let (store, clone) = cx.update(|cx| {
            let store = Store::new(cx, state);
            let clone = store.clone();
            (store, clone)
        });

        let v1 = cx.update(|cx| store.read(cx, |s| s.value));
        let v2 = cx.update(|cx| clone.read(cx, |s| s.value));
        assert_eq!(v1, 42);
        assert_eq!(v2, 42);

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 99;
                s.label = "updated".into();
            });
        });

        let v1a = cx.update(|cx| store.read(cx, |s| s.value));
        let v2a = cx.update(|cx| clone.read(cx, |s| s.value));
        assert_eq!(v1a, 99);
        assert_eq!(v2a, 99);

        drop(store);
        drop(clone);
        assert!(!dropped_flag.get());
    }

    #[gpui::test]
    fn set_publishes(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "a".into(),
                },
            );
            let owner = cx.new(|_| Owner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1, "initial delivery");

        cx.update(|cx| {
            store.set(
                cx,
                AppState {
                    value: 42,
                    label: "b".into(),
                },
            );
        });
        assert_eq!(callbacks.get(), 2, "set publishes even for different value");

        cx.update(|cx| {
            store.set(
                cx,
                AppState {
                    value: 42,
                    label: "b".into(),
                },
            );
        });
        assert_eq!(callbacks.get(), 3, "set publishes even for equal value");
    }

    #[gpui::test]
    fn update_returns_business_result_and_always_publishes(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 10,
                    label: "init".into(),
                },
            );
            let owner = cx.new(|_| Owner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);

        let result = cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 20;
                999
            })
        });
        assert_eq!(result, 999);
        assert_eq!(callbacks.get(), 2);

        let val = cx.update(|cx| store.read(cx, |s| s.value));
        assert_eq!(val, 20);
    }

    #[gpui::test]
    fn update_if_changed_publishes_and_returns_result(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "x".into(),
                },
            );
            let owner = cx.new(|_| Owner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);

        let outcome = cx.update(|cx| {
            store.update_if(cx, |s| {
                s.value = 99;
                StoreChange::Changed("changed-result")
            })
        });
        assert!(outcome.is_changed());
        assert_eq!(outcome.into_result(), "changed-result");
        assert_eq!(callbacks.get(), 2);
    }

    #[gpui::test]
    fn update_if_unchanged_does_not_publish(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "x".into(),
                },
            );
            let owner = cx.new(|_| Owner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);

        let outcome = cx.update(|cx| {
            store.update_if(cx, |s| {
                s.value = 7;
                StoreChange::Unchanged("unchanged-result")
            })
        });
        assert!(!outcome.is_changed());
        assert_eq!(outcome.into_result(), "unchanged-result");
        assert_eq!(callbacks.get(), 1, "Unchanged must not publish");
    }

    #[gpui::test]
    fn global_round_trip_returns_same_store(cx: &mut gpui::TestAppContext) {
        let val = cx.update(|cx| {
            let _store = Store::install_global(
                cx,
                AppState {
                    value: 77,
                    label: "global".into(),
                },
            );
            let global: Store<AppState> = Store::global(cx);
            global.read(cx, |s| s.value)
        });
        assert_eq!(val, 77);
    }

    #[test]
    fn closure_and_named_selectors_share_contract() {
        struct ValueSelector;
        impl Select<AppState> for ValueSelector {
            type Output = i32;
            fn select(&self, source: &AppState) -> i32 {
                source.value
            }
        }

        struct LabelLenSelector;
        impl Select<AppState> for LabelLenSelector {
            type Output = usize;
            fn select(&self, source: &AppState) -> usize {
                source.label.len()
            }
        }

        let state = AppState {
            value: 42,
            label: "hello".into(),
        };

        let closure_val: &dyn Fn(&AppState) -> i32 = &|s: &AppState| s.value;
        let closure_len: &dyn Fn(&AppState) -> usize = &|s: &AppState| s.label.len();

        assert_eq!(ValueSelector.select(&state), closure_val.select(&state));
        assert_eq!(LabelLenSelector.select(&state), closure_len.select(&state));
    }

    #[test]
    fn store_change_helpers_preserve_decision_and_result() {
        let changed = StoreChange::Changed(42);
        assert!(changed.is_changed());
        assert_eq!(changed.into_result(), 42);

        let unchanged = StoreChange::Unchanged(99);
        assert!(!unchanged.is_changed());
        assert_eq!(unchanged.into_result(), 99);

        let c = StoreChange::<String>::changed("yes".into());
        assert!(c.is_changed());
        assert_eq!(c.into_result(), "yes");

        let u = StoreChange::<String>::unchanged("no".into());
        assert!(!u.is_changed());
        assert_eq!(u.into_result(), "no");
    }

    #[gpui::test]
    fn set_drops_replaced_state(cx: &mut gpui::TestAppContext) {
        #[derive(Debug)]
        struct ProbeState {
            _value: i32,
            _probe: DropProbe,
        }

        let (probe1, dropped1) = DropProbe::new();
        let (probe2, dropped2) = DropProbe::new();

        let _store = cx.update(|cx| {
            let store = Store::new(
                cx,
                ProbeState {
                    _value: 1,
                    _probe: probe1,
                },
            );
            store.set(
                cx,
                ProbeState {
                    _value: 2,
                    _probe: probe2,
                },
            );
            store
        });

        assert!(dropped1.get(), "old state must be dropped after set");
        assert!(!dropped2.get(), "new state must still be alive");
    }

    #[gpui::test]
    fn set_keeps_entity_valid_when_replaced_state_destructor_panics(cx: &mut gpui::TestAppContext) {
        struct PanicOnDropState {
            value: i32,
            panic_on_drop: bool,
        }

        impl Drop for PanicOnDropState {
            fn drop(&mut self) {
                assert!(
                    !self.panic_on_drop,
                    "intentional replaced-state destructor panic"
                );
            }
        }

        let store = cx.update(|cx| {
            Store::new(
                cx,
                PanicOnDropState {
                    value: 1,
                    panic_on_drop: true,
                },
            )
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|cx| {
                store.set(
                    cx,
                    PanicOnDropState {
                        value: 2,
                        panic_on_drop: false,
                    },
                );
            });
        }));
        assert!(result.is_err(), "destructor panic must propagate");

        let value = cx.update(|cx| store.read(cx, |state| state.value));
        assert_eq!(
            value, 2,
            "the new state must remain installed and readable after the panic"
        );
    }
}

// ── selection ───────────────────────────────────────────────────────────

mod selection {
    use std::{cell::Cell, rc::Rc};

    use gpui::AppContext as _;

    use crate::Store;
    use crate::tests::{
        AppState, DropProbe, NotifyCounter, OptionalSelectionOwner, SelectionOwner,
    };

    #[gpui::test]
    fn selection_starts_with_current_value(cx: &mut gpui::TestAppContext) {
        let (_store, val) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 55,
                    label: "init".into(),
                },
            );
            let owner = cx.new(|cx| SelectionOwner {
                selection: store.select(cx, |s: &AppState| s.value),
            });
            let val = owner.read_with(cx, |owner, _| owner.selection.read(|v| *v));
            (store, val)
        });
        assert_eq!(val, 55);
    }

    #[gpui::test]
    fn selection_notifies_owner_only_when_output_changes(cx: &mut gpui::TestAppContext) {
        let notify_count = Rc::new(Cell::new(0));

        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "init".into(),
                },
            );
            let owner = cx.new(|cx| SelectionOwner {
                selection: store.select(cx, |s: &AppState| s.value),
            });
            (store, owner)
        });

        let _counter = cx.update(|cx| NotifyCounter::observing(&owner, notify_count.clone(), cx));

        cx.update(|cx| {
            store.update(cx, |s| {
                s.label = "changed".into();
            });
        });

        assert_eq!(notify_count.get(), 0);

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 7;
            });
        });

        assert_eq!(notify_count.get(), 1);

        let val = cx.update(|cx| owner.read_with(cx, |owner, _| owner.selection.read(|v| *v)));
        assert_eq!(val, 7);
    }

    #[gpui::test]
    fn selection_observes_change_after_registration(cx: &mut gpui::TestAppContext) {
        let notify_count = Rc::new(Cell::new(0));

        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 10,
                    label: "a".into(),
                },
            );
            let owner = cx.new(|cx| SelectionOwner {
                selection: store.select(cx, |s: &AppState| s.value),
            });
            (store, owner)
        });

        let _counter = cx.update(|cx| NotifyCounter::observing(&owner, notify_count.clone(), cx));

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 30;
            });
        });

        assert_eq!(notify_count.get(), 1);

        let val = cx.update(|cx| owner.read_with(cx, |owner, _| owner.selection.read(|v| *v)));
        assert_eq!(val, 30);
    }

    #[gpui::test]
    fn selection_supports_non_clone_output(cx: &mut gpui::TestAppContext) {
        #[derive(Debug, PartialEq)]
        struct NonCloneOutput(i32);

        cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 5,
                    label: "x".into(),
                },
            );
            struct NCOwner {
                _selection: crate::StoreSelection<NonCloneOutput>,
            }
            let owner = cx.new(|cx| NCOwner {
                _selection: store.select(cx, |s: &AppState| NonCloneOutput(s.value)),
            });
            let val = owner.read_with(cx, |owner, _| owner._selection.read(|nc| nc.0));
            assert_eq!(val, 5);
        });
    }

    #[gpui::test]
    fn selection_cloned_returns_owned_output(cx: &mut gpui::TestAppContext) {
        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 7,
                    label: "hello".into(),
                },
            );
            let owner = cx.new(|cx| SelectionOwner {
                selection: store.select(cx, |s: &AppState| s.value),
            });
            (store, owner)
        });

        let val = cx.update(|cx| owner.read_with(cx, |owner, _| owner.selection.cloned()));
        assert_eq!(val, 7);

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 99;
            });
        });

        let val = cx.update(|cx| owner.read_with(cx, |owner, _| owner.selection.cloned()));
        assert_eq!(val, 99);
    }

    #[gpui::test]
    fn dropping_selection_unsubscribes(cx: &mut gpui::TestAppContext) {
        let notify_count = Rc::new(Cell::new(0));

        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "init".into(),
                },
            );
            let owner = cx.new(|cx| OptionalSelectionOwner {
                selection: Some(store.select(cx, |s: &AppState| s.value)),
            });
            (store, owner)
        });

        let _counter = cx.update(|cx| NotifyCounter::observing(&owner, notify_count.clone(), cx));

        cx.update(|cx| {
            owner.update(cx, |owner, _| {
                owner.selection.take();
            });
            store.update(cx, |s| {
                s.value = 1;
            });
        });

        assert_eq!(notify_count.get(), 0);
    }

    #[gpui::test]
    fn selection_does_not_keep_store_alive(cx: &mut gpui::TestAppContext) {
        let (probe, dropped_flag) = DropProbe::new();

        #[derive(Debug)]
        struct ProbeState {
            _value: i32,
            _probe: DropProbe,
        }

        struct PSIOwner {
            _selection: crate::StoreSelection<i32>,
        }

        let owner = cx.update(|cx| {
            let store = Store::new(
                cx,
                ProbeState {
                    _value: 1,
                    _probe: probe,
                },
            );
            let owner = cx.new(|cx| PSIOwner {
                _selection: store.select(cx, |s: &ProbeState| s._value),
            });
            drop(store);
            owner
        });

        assert!(dropped_flag.get(), "ProbeState drops when store is dropped");

        let val = cx.update(|cx| owner.read_with(cx, |owner, _| owner._selection.read(|v| *v)));
        assert_eq!(
            val, 1,
            "selection retains last known value after store gone"
        );

        drop(owner);
    }
}

// ── observation ─────────────────────────────────────────────────────────

mod observation {
    use std::{cell::Cell, rc::Rc};

    use gpui::{AppContext as _, Subscription};

    use crate::tests::{AppState, DropProbe, NotifyCounter, ObservedOwner};
    use crate::{Store, StoreChange};

    #[gpui::test]
    fn observe_initial_delivery_reads_latest_state(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let last_value = Rc::new(Cell::new(0));

        let cb = callbacks.clone();
        let lv = last_value.clone();

        let (_store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "first".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                let lv2 = lv.clone();
                owner._subscription = Some(store.observe(cx, move |_, state, _| {
                    cb2.set(cb2.get() + 1);
                    lv2.set(state.value);
                }));
            });

            store.set(
                cx,
                AppState {
                    value: 42,
                    label: "mutated".into(),
                },
            );

            (store, owner)
        });

        assert_eq!(
            callbacks.get(),
            2,
            "initial delivery plus queued publication"
        );
        assert_eq!(last_value.get(), 42, "all deliveries see mutated state");
    }

    #[gpui::test]
    fn observe_initial_precedes_queued_publication(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let values_seen = Rc::new(Cell::new(Vec::new()));

        let cb = callbacks.clone();
        let vs = values_seen.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "init".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                let vs2 = vs.clone();
                owner._subscription = Some(store.observe(cx, move |_, state, _| {
                    cb2.set(cb2.get() + 1);
                    let mut v = vs2.take();
                    v.push(state.value);
                    vs2.set(v);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);
        {
            let initial_values = values_seen.take();
            assert_eq!(initial_values, vec![0]);
            values_seen.set(initial_values);
        }

        cx.update(|cx| {
            store.set(
                cx,
                AppState {
                    value: 1,
                    label: "one".into(),
                },
            );
        });

        assert_eq!(callbacks.get(), 2);
        {
            let all_values = values_seen.take();
            assert_eq!(all_values, vec![0, 1]);
        }
    }

    #[gpui::test]
    fn dropping_observation_before_initial_suppresses_delivery(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 10,
                    label: "init".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                let sub = store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                });
                drop(sub);
                let _ = owner;
            });
            drop(owner);
        });

        assert_eq!(callbacks.get(), 0, "callback suppressed by early drop");
    }

    #[gpui::test]
    fn initial_owner_or_source_loss_cancels_delivery(cx: &mut gpui::TestAppContext) {
        let owner_loss_callbacks = Rc::new(Cell::new(0));
        let owner_loss_count = owner_loss_callbacks.clone();
        let (_store, _subscription) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "owner-loss".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            let subscription = owner.update(cx, |_, cx| {
                store.observe(cx, move |_, _, _| {
                    owner_loss_count.set(owner_loss_count.get() + 1);
                })
            });
            drop(owner);
            (store, subscription)
        });
        cx.run_until_parked();
        assert_eq!(
            owner_loss_callbacks.get(),
            0,
            "an owner lost before initial delivery must not receive a callback"
        );

        let source_loss_callbacks = Rc::new(Cell::new(0));
        let source_loss_count = source_loss_callbacks.clone();
        let (_owner, _subscription) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 2,
                    label: "source-loss".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            let subscription = owner.update(cx, |_, cx| {
                store.observe(cx, move |_, _, _| {
                    source_loss_count.set(source_loss_count.get() + 1);
                })
            });
            drop(store);
            (owner, subscription)
        });
        cx.run_until_parked();
        assert_eq!(
            source_loss_callbacks.get(),
            0,
            "a source lost before initial delivery must not invoke the callback"
        );
    }

    #[gpui::test]
    fn observe_can_cancel_itself_during_initial_delivery(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let count = callbacks.clone();
        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "self-cancel".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                owner._subscription = Some(store.observe(cx, move |owner, _, _| {
                    count.set(count.get() + 1);
                    owner._subscription.take();
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1, "initial callback must run once");
        let cancelled =
            cx.update(|cx| owner.read_with(cx, |owner, _| owner._subscription.is_none()));
        assert!(cancelled, "callback must be able to drop its subscription");

        cx.update(|cx| {
            store.update(cx, |state| state.value = 2);
        });
        assert_eq!(callbacks.get(), 1, "self-cancelled observer stays inert");
    }

    #[gpui::test]
    fn observe_runs_for_equal_update_publications(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 5,
                    label: "same".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 5;
            });
        });

        assert_eq!(callbacks.get(), 2, "observe fires for equal-state update");
    }

    #[gpui::test]
    fn observe_update_if_unchanged_does_not_run(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 3,
                    label: "val".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);

        cx.update(|cx| {
            let _ = store.update_if(cx, |s| {
                s.value = 9;
                StoreChange::Unchanged(())
            });
        });

        assert_eq!(
            callbacks.get(),
            1,
            "Unchanged update_if must not trigger observe"
        );
    }

    #[gpui::test]
    fn observe_does_not_notify_owner_implicitly(cx: &mut gpui::TestAppContext) {
        let owner_notify_count = Rc::new(Cell::new(0));

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "x".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });

            let _counter = NotifyCounter::observing(&owner, owner_notify_count.clone(), cx);

            owner.update(cx, |owner, cx| {
                owner._subscription = Some(store.observe(cx, |_, _, _| {}));
            });

            (store, owner)
        });

        let after_register = owner_notify_count.get();

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 99;
            });
        });

        assert_eq!(
            owner_notify_count.get(),
            after_register,
            "owner must not be notified implicitly by observe"
        );
    }

    #[gpui::test]
    fn observe_select_delivers_initial_then_only_distinct_values(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let values_seen = Rc::new(Cell::new(Vec::new()));

        let cb = callbacks.clone();
        let vs = values_seen.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 10,
                    label: "a".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                let vs2 = vs.clone();
                owner._subscription = Some(store.observe_select(
                    cx,
                    |s: &AppState| s.value,
                    move |_, val, _| {
                        cb2.set(cb2.get() + 1);
                        let mut v = vs2.take();
                        v.push(*val);
                        vs2.set(v);
                    },
                ));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1, "initial delivery");

        cx.update(|cx| {
            store.update(cx, |s| {
                s.label = "b".into();
            });
        });
        assert_eq!(callbacks.get(), 1);

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 10;
            });
        });
        assert_eq!(callbacks.get(), 1);

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 20;
            });
        });
        assert_eq!(callbacks.get(), 2, "only distinct value change triggers");

        let all_values = values_seen.take();
        assert_eq!(all_values, vec![10, 20]);
    }

    #[gpui::test]
    fn observe_select_supports_non_clone_output(cx: &mut gpui::TestAppContext) {
        #[derive(Debug, PartialEq)]
        struct NonCloneOutput(i32);

        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 5,
                    label: "nc".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe_select(
                    cx,
                    |s: &AppState| NonCloneOutput(s.value),
                    move |_, _val, _| {
                        cb2.set(cb2.get() + 1);
                    },
                ));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1, "initial delivery with non-Clone output");

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 99;
            });
        });
        assert_eq!(callbacks.get(), 2, "distinct change with non-Clone output");
    }

    #[gpui::test]
    fn observe_select_can_cancel_itself_during_active_delivery(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let count = callbacks.clone();
        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "selected-self-cancel".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                owner._subscription = Some(store.observe_select(
                    cx,
                    |state: &AppState| state.value,
                    move |owner, _, _| {
                        let next = count.get() + 1;
                        count.set(next);
                        if next == 2 {
                            owner._subscription.take();
                        }
                    },
                ));
            });
            (store, owner)
        });
        assert_eq!(callbacks.get(), 1, "initial callback");

        cx.update(|cx| {
            store.update(cx, |state| state.value = 2);
        });
        assert_eq!(callbacks.get(), 2, "active callback self-cancels");
        let cancelled =
            cx.update(|cx| owner.read_with(cx, |owner, _| owner._subscription.is_none()));
        assert!(cancelled);

        cx.update(|cx| {
            store.update(cx, |state| state.value = 3);
        });
        assert_eq!(
            callbacks.get(),
            2,
            "cancelled selected observer stays inert"
        );
    }

    #[gpui::test]
    fn observe_select_in_delivers_initial_and_changes_with_window(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        struct SelectInObserver {
            _subscription: Subscription,
        }

        impl gpui::Render for SelectInObserver {
            fn render(
                &mut self,
                _window: &mut gpui::Window,
                _cx: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                gpui::div()
            }
        }

        let (store, _window) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 10,
                    label: "win-init".into(),
                },
            );
            let c = cb.clone();
            let s = store.clone();
            let window = cx
                .open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let c2 = c.clone();
                        let subscription = s.observe_select_in(
                            cx,
                            window,
                            |state: &AppState| state.value,
                            move |_, _val, _window, _cx| {
                                c2.set(c2.get() + 1);
                            },
                        );
                        SelectInObserver {
                            _subscription: subscription,
                        }
                    })
                })
                .unwrap();
            (store, window)
        });

        assert_eq!(
            callbacks.get(),
            1,
            "window observe_select_in delivers initial"
        );

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 20;
            });
        });

        assert_eq!(
            callbacks.get(),
            2,
            "window observe_select_in delivers change"
        );

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 20;
            });
        });

        assert_eq!(
            callbacks.get(),
            2,
            "window observe_select_in suppresses equal values"
        );
    }

    #[gpui::test]
    fn observe_select_in_can_cancel_itself_during_active_delivery(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let count = callbacks.clone();

        struct SelfCancellingObserver {
            subscription: Option<Subscription>,
        }

        impl gpui::Render for SelfCancellingObserver {
            fn render(
                &mut self,
                _window: &mut gpui::Window,
                _cx: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                gpui::div()
            }
        }

        let (store, _window) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "window-self-cancel".into(),
                },
            );
            let observed_store = store.clone();
            let window = cx
                .open_window(Default::default(), |window, cx| {
                    cx.new(|cx| {
                        let mut observer = SelfCancellingObserver { subscription: None };
                        observer.subscription = Some(observed_store.observe_select_in(
                            cx,
                            window,
                            |state: &AppState| state.value,
                            move |observer: &mut SelfCancellingObserver, _, _, _| {
                                let next = count.get() + 1;
                                count.set(next);
                                if next == 2 {
                                    observer.subscription.take();
                                }
                            },
                        ));
                        observer
                    })
                })
                .unwrap();
            (store, window)
        });
        assert_eq!(callbacks.get(), 1, "initial window callback");

        cx.update(|cx| {
            store.update(cx, |state| state.value = 2);
        });
        assert_eq!(callbacks.get(), 2, "active window callback self-cancels");

        cx.update(|cx| {
            store.update(cx, |state| state.value = 3);
        });
        assert_eq!(callbacks.get(), 2, "cancelled window observer stays inert");
    }

    #[gpui::test]
    fn observe_select_in_window_close_cancels_pending_and_active_delivery(
        cx: &mut gpui::TestAppContext,
    ) {
        struct WindowObserver {
            _subscription: Subscription,
        }

        impl gpui::Render for WindowObserver {
            fn render(
                &mut self,
                _window: &mut gpui::Window,
                _cx: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                gpui::div()
            }
        }

        let pending_callbacks = Rc::new(Cell::new(0));
        let pending_count = pending_callbacks.clone();
        let pending_store = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "pending-window-close".into(),
                },
            );
            let observed_store = store.clone();
            let window = cx
                .open_window(Default::default(), |window, cx| {
                    cx.new(|cx| WindowObserver {
                        _subscription: observed_store.observe_select_in(
                            cx,
                            window,
                            |state: &AppState| state.value,
                            move |_, _, _, _| {
                                pending_count.set(pending_count.get() + 1);
                            },
                        ),
                    })
                })
                .unwrap();
            window
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
            store
        });
        cx.run_until_parked();
        assert_eq!(
            pending_callbacks.get(),
            0,
            "closing before initial delivery suppresses the callback"
        );
        cx.update(|cx| {
            pending_store.update(cx, |state| state.value = 2);
        });
        assert_eq!(pending_callbacks.get(), 0);

        let active_callbacks = Rc::new(Cell::new(0));
        let active_count = active_callbacks.clone();
        let (active_store, active_window) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 1,
                    label: "active-window-close".into(),
                },
            );
            let observed_store = store.clone();
            let window = cx
                .open_window(Default::default(), |window, cx| {
                    cx.new(|cx| WindowObserver {
                        _subscription: observed_store.observe_select_in(
                            cx,
                            window,
                            |state: &AppState| state.value,
                            move |_, _, _, _| {
                                active_count.set(active_count.get() + 1);
                            },
                        ),
                    })
                })
                .unwrap();
            (store, window)
        });
        assert_eq!(active_callbacks.get(), 1, "initial callback becomes active");

        cx.update(|cx| {
            active_window
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
        });
        cx.run_until_parked();
        cx.update(|cx| {
            active_store.update(cx, |state| state.value = 2);
        });
        assert_eq!(
            active_callbacks.get(),
            1,
            "closed window observation stays inert"
        );
    }

    #[gpui::test]
    fn dropping_subscription_stops_callbacks(cx: &mut gpui::TestAppContext) {
        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                AppState {
                    value: 0,
                    label: "sub".into(),
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });

            let cb2 = cb.clone();
            let s = store.clone();
            owner.update(cx, move |owner, cx| {
                let subscription = s.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                });
                owner._subscription = Some(subscription);
            });

            (store, owner)
        });

        assert_eq!(callbacks.get(), 1, "initial delivery");

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 1;
            });
        });
        assert_eq!(callbacks.get(), 2);

        cx.update(|cx| {
            owner.update(cx, |owner, _| {
                owner._subscription.take();
            });
        });

        cx.update(|cx| {
            store.update(cx, |s| {
                s.value = 2;
            });
        });

        assert_eq!(callbacks.get(), 2, "no callback after subscription dropped");
    }

    #[gpui::test]
    fn observation_does_not_keep_store_alive(cx: &mut gpui::TestAppContext) {
        let (probe, dropped_flag) = DropProbe::new();

        #[derive(Debug)]
        struct ProbeState {
            _value: i32,
            _probe: DropProbe,
        }

        let callbacks = Rc::new(Cell::new(0));
        let cb = callbacks.clone();

        let (store, _owner) = cx.update(|cx| {
            let store = Store::new(
                cx,
                ProbeState {
                    _value: 42,
                    _probe: probe,
                },
            );
            let owner = cx.new(|_| ObservedOwner {
                _subscription: None,
            });
            owner.update(cx, |owner, cx| {
                let cb2 = cb.clone();
                owner._subscription = Some(store.observe(cx, move |_, _, _| {
                    cb2.set(cb2.get() + 1);
                }));
            });
            (store, owner)
        });

        assert_eq!(callbacks.get(), 1);
        assert!(!dropped_flag.get());

        cx.update(|_cx| {
            drop(store);
        });

        assert!(dropped_flag.get(), "state drops when store is dropped");
    }
}
