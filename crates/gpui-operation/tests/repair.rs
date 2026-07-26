use std::{cell::RefCell, fmt, rc::Rc};

use gpui_operation::{
    Cancel, Complete, Load, Refresh, Repair, Transition,
    repair::{
        Degraded, FetchCompleted, Idle, Operation as RepairOperation, Phase as RepairPhase, Ready,
        RefreshCompleted, RepairWithDataCompleted, RepairWithoutDataCompleted, Unavailable,
    },
};

// ── Fixtures ────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
struct IntData(i32);

#[derive(Debug, PartialEq)]
struct IntProblem(i32);

impl fmt::Display for IntProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "problem-{}", self.0)
    }
}

impl std::error::Error for IntProblem {}

#[derive(Debug, PartialEq)]
struct IntRepair(i32);

struct Task;

struct Add(i32);

impl Transition<Add> for &mut IntData {
    type Output = ();

    fn transition(self, message: Add) {
        self.0 += message.0;
    }
}

fn make_ready<Data, Repair>(data: Data) -> Ready<Data, Repair> {
    match Idle::new()
        .transition(Load(Task))
        .transition(Complete::<Data, IntProblem>(Ok(data)))
    {
        FetchCompleted::Ready(r) => r,
        _ => unreachable!(),
    }
}

fn make_unavailable<Problem: std::error::Error, Repair>(
    problem: Problem,
) -> Unavailable<Problem, Repair> {
    match Idle::new()
        .transition(Load(Task))
        .transition(Complete::<IntData, Problem>(Err(problem)))
    {
        FetchCompleted::Unavailable(ua) => ua,
        _ => unreachable!(),
    }
}

fn make_degraded<Data, Problem: std::error::Error, Repair>(
    data: Data,
    problem: Problem,
) -> Degraded<Data, Problem, Repair> {
    let ready = make_ready(data);
    match ready
        .transition(Refresh(Task))
        .transition(Complete::<Data, Problem>(Err(problem)))
    {
        RefreshCompleted::Degraded(d) => d,
        _ => unreachable!(),
    }
}

// ── OP-R2 / OP-R8: Load completion produces repair ready or unavailable ─

#[test]
fn load_completion_produces_repair_ready_or_unavailable() {
    let fetching = Idle::<IntRepair>::new().transition(Load(Task));

    let completed: FetchCompleted<IntData, IntProblem, IntRepair> =
        fetching.transition(Complete(Ok(IntData(42))));
    match completed {
        FetchCompleted::Ready(r) => assert_eq!(r.data(), &IntData(42)),
        _ => panic!("expected Ready"),
    }

    let fetching = Idle::<IntRepair>::new().transition(Load(Task));
    let completed: FetchCompleted<IntData, IntProblem, IntRepair> =
        fetching.transition(Complete(Err(IntProblem(3))));
    match completed {
        FetchCompleted::Unavailable(ua) => assert_eq!(ua.problem(), &IntProblem(3)),
        _ => panic!("expected Unavailable"),
    }
}

// ── OP-R8: Normal refresh maps both results ─────────────────────────────

#[test]
fn normal_refresh_maps_both_results() {
    let ready = make_ready::<_, IntRepair>(IntData(10));
    let fetching = ready.transition(Refresh(Task));

    let completed: RefreshCompleted<IntData, IntProblem, IntRepair> =
        fetching.transition(Complete(Ok(IntData(11))));
    match completed {
        RefreshCompleted::Ready(r) => assert_eq!(r.data(), &IntData(11)),
        _ => panic!("expected Ready"),
    }

    let ready = make_ready::<_, IntRepair>(IntData(20));
    let fetching = ready.transition(Refresh(Task));
    let completed: RefreshCompleted<IntData, IntProblem, IntRepair> =
        fetching.transition(Complete(Err(IntProblem(21))));
    match completed {
        RefreshCompleted::Degraded(d) => {
            assert_eq!(d.data(), &IntData(20));
            assert_eq!(d.problem(), &IntProblem(21));
        }
        _ => panic!("expected Degraded"),
    }
}

// ── OP-R8: Repair without data maps both results ────────────────────────

#[test]
fn repair_without_data_maps_both_results() {
    let unavail = make_unavailable::<_, IntRepair>(IntProblem(5));
    let repairing = unavail.transition(Repair {
        repair: IntRepair(10),
        task: Task,
    });

    let completed: RepairWithoutDataCompleted<IntData, IntProblem, IntRepair> =
        repairing.transition(Complete(Ok(IntData(15))));
    match completed {
        RepairWithoutDataCompleted::Ready(r) => assert_eq!(r.data(), &IntData(15)),
        _ => panic!("expected Ready"),
    }

    let unavail = make_unavailable::<_, IntRepair>(IntProblem(6));
    let repairing = unavail.transition(Repair {
        repair: IntRepair(11),
        task: Task,
    });
    let completed: RepairWithoutDataCompleted<IntData, IntProblem, IntRepair> =
        repairing.transition(Complete(Err(IntProblem(16))));
    match completed {
        RepairWithoutDataCompleted::Unavailable(ua) => assert_eq!(ua.problem(), &IntProblem(16)),
        _ => panic!("expected Unavailable"),
    }
}

// ── OP-R8: Repair with data maps both results ───────────────────────────

#[test]
fn repair_with_data_maps_both_results() {
    let degraded = make_degraded::<_, _, IntRepair>(IntData(30), IntProblem(31));
    let repairing = degraded.transition(Repair {
        repair: IntRepair(20),
        task: Task,
    });

    let completed: RepairWithDataCompleted<IntData, IntProblem, IntRepair> =
        repairing.transition(Complete(Ok(IntData(32))));
    match completed {
        RepairWithDataCompleted::Ready(r) => assert_eq!(r.data(), &IntData(32)),
        _ => panic!("expected Ready"),
    }

    let degraded = make_degraded::<_, _, IntRepair>(IntData(40), IntProblem(41));
    let repairing = degraded.transition(Repair {
        repair: IntRepair(21),
        task: Task,
    });
    let completed: RepairWithDataCompleted<IntData, IntProblem, IntRepair> =
        repairing.transition(Complete(Err(IntProblem(42))));
    match completed {
        RepairWithDataCompleted::Degraded(d) => {
            assert_eq!(d.data(), &IntData(40));
            assert_eq!(d.problem(), &IntProblem(42));
        }
        _ => panic!("expected Degraded"),
    }
}

// ── OP-R4: Cancel restores all four repair previous states ──────────────

#[test]
fn cancel_restores_all_four_repair_previous_states() {
    let _: Idle<IntRepair> = Idle::<IntRepair>::new()
        .transition(Load(Task))
        .transition(Cancel);

    let ready = make_ready::<_, IntRepair>(IntData(50));
    let fetching = ready.transition(Refresh(Task));
    assert_eq!(fetching.transition(Cancel).data(), &IntData(50));

    let unavail = make_unavailable::<_, IntRepair>(IntProblem(60));
    let repairing = unavail.transition(Repair {
        repair: IntRepair(1),
        task: Task,
    });
    let restored: Unavailable<IntProblem, IntRepair> = repairing.transition(Cancel);
    assert_eq!(restored.problem(), &IntProblem(60));

    let degraded = make_degraded::<_, _, IntRepair>(IntData(70), IntProblem(71));
    let repairing = degraded.transition(Repair {
        repair: IntRepair(2),
        task: Task,
    });
    let restored: Degraded<IntData, IntProblem, IntRepair> = repairing.transition(Cancel);
    assert_eq!(restored.data(), &IntData(70));
    assert_eq!(restored.problem(), &IntProblem(71));
}

// ── OP-R5: Running values have deterministic drop order ─────────────────

type DropOrder = Rc<RefCell<Vec<&'static str>>>;

struct OrderedTask {
    name: &'static str,
    order: DropOrder,
}

impl Drop for OrderedTask {
    fn drop(&mut self) {
        self.order.borrow_mut().push(self.name);
    }
}

struct OrderedRepair {
    name: &'static str,
    order: DropOrder,
}

impl Drop for OrderedRepair {
    fn drop(&mut self) {
        self.order.borrow_mut().push(self.name);
    }
}

struct OrderedData {
    name: &'static str,
    order: DropOrder,
}

impl Drop for OrderedData {
    fn drop(&mut self) {
        self.order.borrow_mut().push(self.name);
    }
}

#[derive(Debug)]
struct OrderedProblem {
    name: &'static str,
    order: DropOrder,
}

impl fmt::Display for OrderedProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

impl std::error::Error for OrderedProblem {}

impl Drop for OrderedProblem {
    fn drop(&mut self) {
        self.order.borrow_mut().push(self.name);
    }
}

#[test]
fn cancel_drops_task_then_selected_repair_before_restoring_previous() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let unavailable = make_unavailable::<_, OrderedRepair>(OrderedProblem {
        name: "previous",
        order: order.clone(),
    });
    let repairing = unavailable.transition(Repair {
        repair: OrderedRepair {
            name: "repair",
            order: order.clone(),
        },
        task: OrderedTask {
            name: "task",
            order: order.clone(),
        },
    });

    let restored: Unavailable<OrderedProblem, OrderedRepair> = repairing.transition(Cancel);
    assert_eq!(&*order.borrow(), &["task", "repair"]);

    drop(restored);
    assert_eq!(&*order.borrow(), &["task", "repair", "previous"]);
}

#[test]
fn repair_completion_drops_task_then_repair_and_previous_on_both_results() {
    let success_order = Rc::new(RefCell::new(Vec::new()));
    let unavailable = make_unavailable::<_, OrderedRepair>(OrderedProblem {
        name: "previous",
        order: success_order.clone(),
    });
    let repairing = unavailable.transition(Repair {
        repair: OrderedRepair {
            name: "repair",
            order: success_order.clone(),
        },
        task: OrderedTask {
            name: "task",
            order: success_order.clone(),
        },
    });

    let _: RepairWithoutDataCompleted<IntData, OrderedProblem, OrderedRepair> =
        repairing.transition(Complete(Ok(IntData(91))));
    assert_eq!(&*success_order.borrow(), &["task", "repair", "previous"]);

    let failure_order = Rc::new(RefCell::new(Vec::new()));
    let unavailable = make_unavailable::<_, OrderedRepair>(OrderedProblem {
        name: "previous",
        order: failure_order.clone(),
    });
    let repairing = unavailable.transition(Repair {
        repair: OrderedRepair {
            name: "repair",
            order: failure_order.clone(),
        },
        task: OrderedTask {
            name: "task",
            order: failure_order.clone(),
        },
    });

    let completed: RepairWithoutDataCompleted<IntData, OrderedProblem, OrderedRepair> = repairing
        .transition(Complete(Err(OrderedProblem {
            name: "next",
            order: failure_order.clone(),
        })));
    assert_eq!(&*failure_order.borrow(), &["task", "repair", "previous"]);

    drop(completed);
    assert_eq!(
        &*failure_order.borrow(),
        &["task", "repair", "previous", "next"]
    );
}

#[test]
fn dropping_repairing_drops_task_repair_then_previous() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let degraded = make_degraded::<_, _, OrderedRepair>(
        OrderedData {
            name: "data",
            order: order.clone(),
        },
        OrderedProblem {
            name: "problem",
            order: order.clone(),
        },
    );
    let repairing = degraded.transition(Repair {
        repair: OrderedRepair {
            name: "repair",
            order: order.clone(),
        },
        task: OrderedTask {
            name: "task",
            order: order.clone(),
        },
    });

    drop(repairing);
    assert_eq!(&*order.borrow(), &["task", "repair", "data", "problem"]);
}

// ── OP-R2: Repair kind is part of settled state type ────────────────────

#[test]
fn repair_kind_is_part_of_settled_state_type() {
    // Different repair types produce different state types.
    // We verify that the same transition logic works for two different repair markers.

    #[derive(Debug, PartialEq)]
    struct NetRepair(String);

    #[derive(Debug, PartialEq)]
    struct DbRepair(i64);

    // Both Idle types are distinct
    let net_idle: Idle<NetRepair> = Idle::new();
    let db_idle: Idle<DbRepair> = Idle::new();

    let net_fetching = net_idle.transition(Load(Task));
    let _: FetchCompleted<IntData, IntProblem, NetRepair> =
        net_fetching.transition(Complete::<IntData, IntProblem>(Ok(IntData(1))));

    let db_fetching = db_idle.transition(Load(Task));
    let _: FetchCompleted<IntData, IntProblem, DbRepair> =
        db_fetching.transition(Complete::<IntData, IntProblem>(Ok(IntData(2))));
}

// ── OP-R5: Non-Clone, non-Send payloads in repair ───────────────────────

#[test]
fn repair_transitions_accept_non_clone_non_send_payloads() {
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Debug)]
    struct LocalData(Rc<Cell<i32>>);

    #[derive(Debug)]
    #[allow(dead_code)]
    struct LocalProblem(Rc<()>);

    impl fmt::Display for LocalProblem {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("local problem")
        }
    }

    impl std::error::Error for LocalProblem {}

    #[allow(dead_code)]
    struct LocalRepair(Rc<()>);
    #[allow(dead_code)]
    struct LocalTask(Rc<()>);

    let value = Rc::new(Cell::new(42));

    let fetching = Idle::<LocalRepair>::new().transition(Load(LocalTask(Rc::new(()))));
    let ready = match fetching.transition(Complete::<LocalData, LocalProblem>(Ok(LocalData(
        value.clone(),
    )))) {
        FetchCompleted::Ready(r) => r,
        _ => unreachable!(),
    };
    assert_eq!(ready.data().0.get(), 42);

    let fetching = ready.transition(Refresh(LocalTask(Rc::new(()))));
    let degraded: RefreshCompleted<LocalData, LocalProblem, LocalRepair> = fetching.transition(
        Complete::<LocalData, LocalProblem>(Err(LocalProblem(Rc::new(())))),
    );
    match degraded {
        RefreshCompleted::Degraded(d) => {
            assert_eq!(d.data().0.get(), 42);
        }
        _ => panic!("expected Degraded"),
    }
}

// ── Runtime enum ────────────────────────────────────────────────────────

fn runtime_ready(data: IntData) -> RepairOperation<IntData, IntProblem, IntRepair, Task> {
    let mut operation = RepairOperation::new();
    operation.transition(Load(Task));
    operation.transition(Complete(Ok(data)));
    operation
}

fn runtime_unavailable(
    problem: IntProblem,
) -> RepairOperation<IntData, IntProblem, IntRepair, Task> {
    let mut operation = RepairOperation::new();
    operation.transition(Load(Task));
    operation.transition(Complete(Err(problem)));
    operation
}

fn runtime_degraded(
    data: IntData,
    problem: IntProblem,
) -> RepairOperation<IntData, IntProblem, IntRepair, Task> {
    let mut operation = runtime_ready(data);
    operation.transition(Refresh(Task));
    operation.transition(Complete(Err(problem)));
    operation
}

#[test]
fn runtime_start_cancel_and_projections_cover_every_phase() {
    let mut idle = RepairOperation::<IntData, IntProblem, IntRepair, Task>::new();
    assert_eq!(idle.phase(), RepairPhase::Idle);
    assert_eq!(idle.data(), None);
    assert_eq!(idle.problem(), None);
    assert_eq!(idle.active_repair(), None);
    assert!(!idle.is_running());

    idle.transition(Load(Task));
    assert_eq!(idle.phase(), RepairPhase::Loading);
    assert_eq!(idle.data(), None);
    assert_eq!(idle.problem(), None);
    assert_eq!(idle.active_repair(), None);
    assert!(idle.is_running());
    idle.transition(Cancel);
    assert_eq!(idle.phase(), RepairPhase::Idle);

    let mut ready = runtime_ready(IntData(10));
    assert_eq!(ready.phase(), RepairPhase::Ready);
    assert_eq!(ready.data(), Some(&IntData(10)));
    assert_eq!(ready.problem(), None);
    assert_eq!(ready.active_repair(), None);
    assert!(!ready.is_running());

    ready.transition(Refresh(Task));
    assert_eq!(ready.phase(), RepairPhase::Refreshing);
    assert_eq!(ready.data(), Some(&IntData(10)));
    assert_eq!(ready.problem(), None);
    assert_eq!(ready.active_repair(), None);
    assert!(ready.is_running());
    ready.transition(Cancel);
    assert_eq!(ready.phase(), RepairPhase::Ready);
    assert_eq!(ready.data(), Some(&IntData(10)));

    let mut unavailable = runtime_unavailable(IntProblem(20));
    assert_eq!(unavailable.phase(), RepairPhase::Unavailable);
    assert_eq!(unavailable.data(), None);
    assert_eq!(unavailable.problem(), Some(&IntProblem(20)));
    assert_eq!(unavailable.active_repair(), None);
    assert!(!unavailable.is_running());

    unavailable.transition(Repair {
        repair: IntRepair(21),
        task: Task,
    });
    assert_eq!(unavailable.phase(), RepairPhase::RepairingUnavailable);
    assert_eq!(unavailable.data(), None);
    assert_eq!(unavailable.problem(), Some(&IntProblem(20)));
    assert_eq!(unavailable.active_repair(), Some(&IntRepair(21)));
    assert!(unavailable.is_running());
    unavailable.transition(Cancel);
    assert_eq!(unavailable.phase(), RepairPhase::Unavailable);
    assert_eq!(unavailable.problem(), Some(&IntProblem(20)));
    assert_eq!(unavailable.active_repair(), None);

    let mut degraded = runtime_degraded(IntData(30), IntProblem(31));
    assert_eq!(degraded.phase(), RepairPhase::Degraded);
    assert_eq!(degraded.data(), Some(&IntData(30)));
    assert_eq!(degraded.problem(), Some(&IntProblem(31)));
    assert_eq!(degraded.active_repair(), None);
    assert!(!degraded.is_running());

    degraded.transition(Repair {
        repair: IntRepair(32),
        task: Task,
    });
    assert_eq!(degraded.phase(), RepairPhase::RepairingDegraded);
    assert_eq!(degraded.data(), Some(&IntData(30)));
    assert_eq!(degraded.problem(), Some(&IntProblem(31)));
    assert_eq!(degraded.active_repair(), Some(&IntRepair(32)));
    assert!(degraded.is_running());
    degraded.transition(Cancel);
    assert_eq!(degraded.phase(), RepairPhase::Degraded);
    assert_eq!(degraded.data(), Some(&IntData(30)));
    assert_eq!(degraded.problem(), Some(&IntProblem(31)));
    assert_eq!(degraded.active_repair(), None);

    degraded.transition(Cancel);
    assert_eq!(degraded.phase(), RepairPhase::Degraded);
}

#[test]
fn runtime_completion_covers_every_running_phase_and_result() {
    let mut loading_success = RepairOperation::<IntData, IntProblem, IntRepair, Task>::new();
    loading_success.transition(Load(Task));
    loading_success.transition(Complete(Ok(IntData(1))));
    assert_eq!(loading_success.phase(), RepairPhase::Ready);
    assert_eq!(loading_success.data(), Some(&IntData(1)));

    let mut loading_failure = RepairOperation::<IntData, IntProblem, IntRepair, Task>::new();
    loading_failure.transition(Load(Task));
    loading_failure.transition(Complete(Err(IntProblem(2))));
    assert_eq!(loading_failure.phase(), RepairPhase::Unavailable);
    assert_eq!(loading_failure.problem(), Some(&IntProblem(2)));

    let mut refreshing_success = runtime_ready(IntData(3));
    refreshing_success.transition(Refresh(Task));
    refreshing_success.transition(Complete(Ok(IntData(4))));
    assert_eq!(refreshing_success.phase(), RepairPhase::Ready);
    assert_eq!(refreshing_success.data(), Some(&IntData(4)));

    let mut refreshing_failure = runtime_ready(IntData(5));
    refreshing_failure.transition(Refresh(Task));
    refreshing_failure.transition(Complete(Err(IntProblem(6))));
    assert_eq!(refreshing_failure.phase(), RepairPhase::Degraded);
    assert_eq!(refreshing_failure.data(), Some(&IntData(5)));
    assert_eq!(refreshing_failure.problem(), Some(&IntProblem(6)));

    let mut unavailable_success = runtime_unavailable(IntProblem(7));
    unavailable_success.transition(Repair {
        repair: IntRepair(8),
        task: Task,
    });
    unavailable_success.transition(Complete(Ok(IntData(9))));
    assert_eq!(unavailable_success.phase(), RepairPhase::Ready);
    assert_eq!(unavailable_success.data(), Some(&IntData(9)));
    assert_eq!(unavailable_success.problem(), None);
    assert_eq!(unavailable_success.active_repair(), None);

    let mut unavailable_failure = runtime_unavailable(IntProblem(10));
    unavailable_failure.transition(Repair {
        repair: IntRepair(11),
        task: Task,
    });
    unavailable_failure.transition(Complete(Err(IntProblem(12))));
    assert_eq!(unavailable_failure.phase(), RepairPhase::Unavailable);
    assert_eq!(unavailable_failure.problem(), Some(&IntProblem(12)));
    assert_eq!(unavailable_failure.active_repair(), None);

    let mut degraded_success = runtime_degraded(IntData(13), IntProblem(14));
    degraded_success.transition(Repair {
        repair: IntRepair(15),
        task: Task,
    });
    degraded_success.transition(Complete(Ok(IntData(16))));
    assert_eq!(degraded_success.phase(), RepairPhase::Ready);
    assert_eq!(degraded_success.data(), Some(&IntData(16)));
    assert_eq!(degraded_success.problem(), None);
    assert_eq!(degraded_success.active_repair(), None);

    let mut degraded_failure = runtime_degraded(IntData(17), IntProblem(18));
    degraded_failure.transition(Repair {
        repair: IntRepair(19),
        task: Task,
    });
    degraded_failure.transition(Complete(Err(IntProblem(20))));
    assert_eq!(degraded_failure.phase(), RepairPhase::Degraded);
    assert_eq!(degraded_failure.data(), Some(&IntData(17)));
    assert_eq!(degraded_failure.problem(), Some(&IntProblem(20)));
    assert_eq!(degraded_failure.active_repair(), None);
}

#[test]
fn runtime_ignores_invalid_messages_and_drops_their_payloads() {
    use std::cell::Cell;

    struct TrackedTask {
        drops: Rc<Cell<usize>>,
    }

    impl Drop for TrackedTask {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    let active_task_drops = Rc::new(Cell::new(0));
    let ignored_task_drops = Rc::new(Cell::new(0));
    let mut running = RepairOperation::<(), IntProblem, IntRepair, TrackedTask>::new();
    running.transition(Load(TrackedTask {
        drops: active_task_drops.clone(),
    }));
    running.transition(Load(TrackedTask {
        drops: ignored_task_drops.clone(),
    }));
    assert_eq!(running.phase(), RepairPhase::Loading);
    assert_eq!(ignored_task_drops.get(), 1);

    running.transition(Cancel);
    assert_eq!(active_task_drops.get(), 1);

    struct TrackedRepair {
        drops: Rc<Cell<usize>>,
    }

    impl Drop for TrackedRepair {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    let repair_drops = Rc::new(Cell::new(0));
    let repair_task_drops = Rc::new(Cell::new(0));
    let mut ready = RepairOperation::<IntData, IntProblem, TrackedRepair, TrackedTask>::new();
    ready.transition(Load(TrackedTask {
        drops: active_task_drops.clone(),
    }));
    ready.transition(Complete(Ok(IntData(4))));

    ready.transition(Repair {
        repair: TrackedRepair {
            drops: repair_drops.clone(),
        },
        task: TrackedTask {
            drops: repair_task_drops.clone(),
        },
    });
    assert_eq!(ready.phase(), RepairPhase::Ready);
    assert_eq!(repair_drops.get(), 1);
    assert_eq!(repair_task_drops.get(), 1);

    struct TrackedData {
        drops: Rc<Cell<usize>>,
    }

    impl Drop for TrackedData {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    let data_drops = Rc::new(Cell::new(0));
    let mut settled = RepairOperation::<TrackedData, IntProblem, IntRepair, Task>::new();
    settled.transition(Complete(Ok(TrackedData {
        drops: data_drops.clone(),
    })));
    assert_eq!(settled.phase(), RepairPhase::Idle);
    assert_eq!(data_drops.get(), 1);
}

#[test]
fn runtime_default_requires_no_payload_defaults() {
    struct NoDefaultData;

    #[derive(Debug)]
    struct NoDefaultProblem;

    impl fmt::Display for NoDefaultProblem {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("no default problem")
        }
    }

    impl std::error::Error for NoDefaultProblem {}

    struct NoDefaultRepair;
    struct NoDefaultTask;

    let operation: RepairOperation<
        NoDefaultData,
        NoDefaultProblem,
        NoDefaultRepair,
        NoDefaultTask,
    > = Default::default();
    assert_eq!(operation.phase(), RepairPhase::Idle);
}

#[test]
fn runtime_accepts_non_clone_non_send_payloads() {
    use std::cell::Cell;

    #[derive(Debug)]
    struct LocalData(Rc<Cell<i32>>);

    #[derive(Debug)]
    struct LocalProblem(Rc<()>);

    impl fmt::Display for LocalProblem {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("local problem")
        }
    }

    impl std::error::Error for LocalProblem {}

    struct LocalRepair(Rc<Cell<i32>>);
    struct LocalTask {
        _not_send: Rc<()>,
    }

    let problem_marker = Rc::new(());
    let repair_marker = Rc::new(Cell::new(9));
    let mut operation = RepairOperation::<LocalData, LocalProblem, LocalRepair, LocalTask>::new();
    operation.transition(Load(LocalTask {
        _not_send: Rc::new(()),
    }));
    operation.transition(Complete(Err(LocalProblem(problem_marker.clone()))));
    assert!(Rc::ptr_eq(&operation.problem().unwrap().0, &problem_marker));

    operation.transition(Repair {
        repair: LocalRepair(repair_marker.clone()),
        task: LocalTask {
            _not_send: Rc::new(()),
        },
    });
    assert_eq!(operation.active_repair().unwrap().0.get(), 9);

    let value = Rc::new(Cell::new(42));
    operation.transition(Complete(Ok(LocalData(value.clone()))));
    assert_eq!(operation.phase(), RepairPhase::Ready);
    assert_eq!(operation.data().unwrap().0.get(), 42);
}

#[test]
fn runtime_applies_domain_messages_only_while_ready() {
    let mut operation = runtime_ready(IntData(40));
    let RepairOperation::Ready(ready) = &mut operation else {
        panic!("expected ready");
    };
    ready.transition(Add(2));
    assert_eq!(operation.data(), Some(&IntData(42)));

    operation.transition(Refresh(Task));
    assert_eq!(
        operation.data(),
        Some(&IntData(42)),
        "retained data is read-only while refreshing"
    );

    operation.transition(Complete(Err(IntProblem(1))));
    assert_eq!(
        operation.data(),
        Some(&IntData(42)),
        "degraded data is read-only"
    );
}

#[test]
fn runtime_complete_and_cancel_drop_task_then_repair_after_installing_final_state() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let mut operation =
        RepairOperation::<IntData, OrderedProblem, OrderedRepair, OrderedTask>::new();

    operation.transition(Load(OrderedTask {
        name: "load-task",
        order: order.clone(),
    }));
    operation.transition(Complete(Err(OrderedProblem {
        name: "previous",
        order: order.clone(),
    })));
    assert_eq!(&*order.borrow(), &["load-task"]);

    order.borrow_mut().clear();
    operation.transition(Repair {
        repair: OrderedRepair {
            name: "repair",
            order: order.clone(),
        },
        task: OrderedTask {
            name: "repair-task",
            order: order.clone(),
        },
    });
    operation.transition(Complete(Err(OrderedProblem {
        name: "next",
        order: order.clone(),
    })));
    assert_eq!(&*order.borrow(), &["repair-task", "repair", "previous"]);
    assert_eq!(operation.phase(), RepairPhase::Unavailable);

    order.borrow_mut().clear();
    operation.transition(Repair {
        repair: OrderedRepair {
            name: "cancel-repair",
            order: order.clone(),
        },
        task: OrderedTask {
            name: "cancel-task",
            order: order.clone(),
        },
    });
    operation.transition(Cancel);
    assert_eq!(&*order.borrow(), &["cancel-task", "cancel-repair"]);
    assert_eq!(operation.phase(), RepairPhase::Unavailable);
}

#[test]
fn runtime_remains_valid_when_task_drop_panics_during_complete_or_cancel() {
    struct MaybePanickingTask {
        panic_on_drop: bool,
    }

    impl Drop for MaybePanickingTask {
        fn drop(&mut self) {
            assert!(!self.panic_on_drop, "task drop panic");
        }
    }

    fn unavailable() -> RepairOperation<IntData, IntProblem, IntRepair, MaybePanickingTask> {
        let mut operation = RepairOperation::new();
        operation.transition(Load(MaybePanickingTask {
            panic_on_drop: false,
        }));
        operation.transition(Complete(Err(IntProblem(7))));
        operation
    }

    let mut completing = unavailable();
    completing.transition(Repair {
        repair: IntRepair(8),
        task: MaybePanickingTask {
            panic_on_drop: true,
        },
    });

    let completion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        completing.transition(Complete(Ok(IntData(42))));
    }));
    assert!(completion.is_err());
    assert_eq!(completing.phase(), RepairPhase::Ready);
    assert_eq!(completing.data(), Some(&IntData(42)));

    let mut cancelling = unavailable();
    cancelling.transition(Repair {
        repair: IntRepair(9),
        task: MaybePanickingTask {
            panic_on_drop: true,
        },
    });

    let cancellation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cancelling.transition(Cancel);
    }));
    assert!(cancellation.is_err());
    assert_eq!(cancelling.phase(), RepairPhase::Unavailable);
    assert_eq!(cancelling.problem(), Some(&IntProblem(7)));
}
