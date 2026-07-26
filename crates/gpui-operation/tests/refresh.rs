use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

use gpui_operation::{
    Cancel, Complete, Load, Refresh, Retry, Transition,
    refresh::{
        Degraded, FetchCompleted, Idle, Operation as RefreshOperation, Phase as RefreshPhase,
        Ready, RefreshCompleted, Unavailable,
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

struct Task;

struct Add(i32);

impl Transition<Add> for &mut IntData {
    type Output = ();

    fn transition(self, message: Add) {
        self.0 += message.0;
    }
}

fn make_ready<Data>(data: Data) -> Ready<Data> {
    match Idle::new()
        .transition(Load(Task))
        .transition(Complete::<Data, IntProblem>(Ok(data)))
    {
        FetchCompleted::Ready(r) => r,
        _ => unreachable!(),
    }
}

fn make_unavailable<Problem: std::error::Error>(problem: Problem) -> Unavailable<Problem> {
    match Idle::new()
        .transition(Load(Task))
        .transition(Complete::<IntData, Problem>(Err(problem)))
    {
        FetchCompleted::Unavailable(ua) => ua,
        _ => unreachable!(),
    }
}

fn make_degraded<Data, Problem: std::error::Error>(
    data: Data,
    problem: Problem,
) -> Degraded<Data, Problem> {
    let ready = make_ready(data);
    match ready
        .transition(Refresh(Task))
        .transition(Complete::<Data, Problem>(Err(problem)))
    {
        RefreshCompleted::Degraded(d) => d,
        _ => unreachable!(),
    }
}

// ── OP-R8: Load completion produces ready or unavailable ────────────────

#[test]
fn load_completion_produces_ready_or_unavailable() {
    let fetching = Idle::new().transition(Load(Task));

    let completed: FetchCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Ok(IntData(42))));
    match completed {
        FetchCompleted::Ready(r) => assert_eq!(r.data(), &IntData(42)),
        _ => panic!("expected Ready"),
    }

    let fetching = Idle::new().transition(Load(Task));
    let completed: FetchCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Err(IntProblem(3))));
    match completed {
        FetchCompleted::Unavailable(ua) => assert_eq!(ua.problem(), &IntProblem(3)),
        _ => panic!("expected Unavailable"),
    }
}

// ── OP-R8: Retry completion replaces unavailable problem ────────────────

#[test]
fn retry_completion_replaces_unavailable_problem_on_both_results() {
    let unavail = make_unavailable(IntProblem(1));
    let fetching = unavail.transition(Retry(Task));

    let completed: FetchCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Ok(IntData(55))));
    match completed {
        FetchCompleted::Ready(r) => assert_eq!(r.data(), &IntData(55)),
        _ => panic!("expected Ready"),
    }

    let unavail = make_unavailable(IntProblem(2));
    let fetching = unavail.transition(Retry(Task));
    let completed: FetchCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Err(IntProblem(3))));
    match completed {
        FetchCompleted::Unavailable(ua) => assert_eq!(ua.problem(), &IntProblem(3)),
        _ => panic!("expected Unavailable"),
    }
}

// ── OP-R8: Ready refresh maps both results ──────────────────────────────

#[test]
fn ready_refresh_maps_both_results() {
    let ready = make_ready(IntData(10));
    let fetching = ready.transition(Refresh(Task));

    let completed: RefreshCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Ok(IntData(11))));
    match completed {
        RefreshCompleted::Ready(r) => assert_eq!(r.data(), &IntData(11)),
        _ => panic!("expected Ready"),
    }

    let ready = make_ready(IntData(20));
    let fetching = ready.transition(Refresh(Task));
    let completed: RefreshCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Err(IntProblem(21))));
    match completed {
        RefreshCompleted::Degraded(d) => {
            assert_eq!(d.data(), &IntData(20));
            assert_eq!(d.problem(), &IntProblem(21));
        }
        _ => panic!("expected Degraded"),
    }
}

// ── OP-R8: Degraded refresh maps both results ───────────────────────────

#[test]
fn degraded_refresh_maps_both_results() {
    let degraded = make_degraded(IntData(30), IntProblem(31));
    let fetching = degraded.transition(Refresh(Task));

    let completed: RefreshCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Ok(IntData(32))));
    match completed {
        RefreshCompleted::Ready(r) => assert_eq!(r.data(), &IntData(32)),
        _ => panic!("expected Ready"),
    }

    let degraded = make_degraded(IntData(40), IntProblem(41));
    let fetching = degraded.transition(Refresh(Task));
    let completed: RefreshCompleted<IntData, IntProblem> =
        fetching.transition(Complete(Err(IntProblem(42))));
    match completed {
        RefreshCompleted::Degraded(d) => {
            assert_eq!(d.data(), &IntData(40));
            assert_eq!(d.problem(), &IntProblem(42));
        }
        _ => panic!("expected Degraded"),
    }
}

// ── OP-R4: Cancel restores all previous states ──────────────────────────

#[test]
fn cancel_restores_all_refresh_previous_states() {
    let _: Idle = Idle::new().transition(Load(Task)).transition(Cancel);

    let ready = make_ready(IntData(50));
    let fetching = ready.transition(Refresh(Task));
    assert_eq!(fetching.transition(Cancel).data(), &IntData(50));

    let unavailable = make_unavailable(IntProblem(60));
    let _: Unavailable<IntProblem> = unavailable.transition(Retry(Task)).transition(Cancel);

    let degraded = make_degraded(IntData(70), IntProblem(71));
    let restored: Degraded<IntData, IntProblem> =
        degraded.transition(Refresh(Task)).transition(Cancel);
    assert_eq!(restored.data(), &IntData(70));
    assert_eq!(restored.problem(), &IntProblem(71));
}

// ── OP-R5: Drop order ───────────────────────────────────────────────────

#[derive(Debug)]
struct DropData {
    id: i32,
    dropped: Rc<Cell<bool>>,
}

impl DropData {
    fn new(id: i32) -> (Self, Rc<Cell<bool>>) {
        let dropped = Rc::new(Cell::new(false));
        let me = Self {
            id,
            dropped: Rc::clone(&dropped),
        };
        (me, dropped)
    }
}

impl Drop for DropData {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

impl PartialEq for DropData {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl fmt::Display for DropData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "drop-data-{}", self.id)
    }
}

impl std::error::Error for DropData {}

struct OrderedTask {
    name: &'static str,
    order: Rc<RefCell<Vec<&'static str>>>,
}

impl Drop for OrderedTask {
    fn drop(&mut self) {
        self.order.borrow_mut().push(self.name);
    }
}

struct OrderedData {
    name: &'static str,
    order: Rc<RefCell<Vec<&'static str>>>,
}

impl Drop for OrderedData {
    fn drop(&mut self) {
        self.order.borrow_mut().push(self.name);
    }
}

#[test]
fn cancel_drops_task_before_previous_payload() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let data = OrderedData {
        name: "previous",
        order: order.clone(),
    };
    let ready = make_ready(data);
    let fetching = ready.transition(Refresh(OrderedTask {
        name: "task",
        order: order.clone(),
    }));

    let ready = fetching.transition(Cancel);
    assert_eq!(&*order.borrow(), &["task"]);

    drop(ready);
    assert_eq!(&*order.borrow(), &["task", "previous"]);
}

#[test]
fn dropping_fetching_drops_task_before_previous() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let task = OrderedTask {
        name: "task",
        order: order.clone(),
    };
    let data = OrderedData {
        name: "previous",
        order: order.clone(),
    };

    let ready = make_ready(data);
    let fetching = ready.transition(Refresh(task));

    drop(fetching);
    assert_eq!(&*order.borrow(), &["task", "previous"]);
}

#[test]
fn completion_drops_task_before_replaced_payload() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let ready = make_ready(OrderedData {
        name: "previous",
        order: order.clone(),
    });
    let fetching = ready.transition(Refresh(OrderedTask {
        name: "task",
        order: order.clone(),
    }));

    let completed: RefreshCompleted<OrderedData, IntProblem> =
        fetching.transition(Complete(Ok(OrderedData {
            name: "next",
            order: order.clone(),
        })));

    assert_eq!(&*order.borrow(), &["task", "previous"]);
    drop(completed);
    assert_eq!(&*order.borrow(), &["task", "previous", "next"]);
}

#[test]
fn completion_drops_old_data_on_success() {
    let (data, data_dropped) = DropData::new(100);
    let ready = make_ready(data);
    let fetching = ready.transition(Refresh(Task));
    let _: RefreshCompleted<DropData, IntProblem> =
        fetching.transition(Complete(Ok(DropData::new(101).0)));

    assert!(data_dropped.get());
}

#[test]
fn completion_keeps_old_data_on_failure() {
    let (data, data_dropped) = DropData::new(110);
    let ready = make_ready(data);
    let fetching = ready.transition(Refresh(Task));
    let degraded: RefreshCompleted<DropData, IntProblem> =
        fetching.transition(Complete::<DropData, IntProblem>(Err(IntProblem(111))));

    match degraded {
        RefreshCompleted::Degraded(d) => {
            assert_eq!(d.data().id, 110);
            assert!(!data_dropped.get()); // data still lives in Degraded
        }
        _ => panic!("expected Degraded"),
    }
}

// ── OP-R5: Non-Clone, non-Send payloads ─────────────────────────────────

#[test]
fn transitions_accept_non_clone_non_send_payloads() {
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
    struct LocalTask(Rc<()>);

    let value = Rc::new(Cell::new(42));

    let fetching = Idle::new().transition(Load(LocalTask(Rc::new(()))));
    let ready = match fetching.transition(Complete::<LocalData, LocalProblem>(Ok(LocalData(
        value.clone(),
    )))) {
        FetchCompleted::Ready(r) => r,
        _ => unreachable!(),
    };
    assert_eq!(ready.data().0.get(), 42);

    let fetching = ready.transition(Refresh(LocalTask(Rc::new(()))));
    let degraded: RefreshCompleted<LocalData, LocalProblem> =
        fetching.transition(Complete(Err(LocalProblem(Rc::new(())))));
    match degraded {
        RefreshCompleted::Degraded(d) => {
            assert_eq!(d.data().0.get(), 42);
        }
        _ => panic!("expected Degraded"),
    }
}

// ── OP-R11: Empty data is ready ─────────────────────────────────────────

#[test]
fn empty_data_is_ready() {
    let fetching = Idle::new().transition(Load(Task));
    let completed: FetchCompleted<Vec<i32>, IntProblem> =
        fetching.transition(Complete(Ok(Vec::new())));
    match completed {
        FetchCompleted::Ready(r) => assert!(r.data().is_empty()),
        _ => panic!("expected Ready"),
    }
}

// ── Runtime enum ────────────────────────────────────────────────────────

fn runtime_ready(data: IntData) -> RefreshOperation<IntData, IntProblem, Task> {
    let mut operation = RefreshOperation::new();
    operation.transition(Load(Task));
    operation.transition(Complete(Ok(data)));
    operation
}

fn runtime_unavailable(problem: IntProblem) -> RefreshOperation<IntData, IntProblem, Task> {
    let mut operation = RefreshOperation::new();
    operation.transition(Load(Task));
    operation.transition(Complete(Err(problem)));
    operation
}

fn runtime_degraded(
    data: IntData,
    problem: IntProblem,
) -> RefreshOperation<IntData, IntProblem, Task> {
    let mut operation = runtime_ready(data);
    operation.transition(Refresh(Task));
    operation.transition(Complete(Err(problem)));
    operation
}

#[test]
fn runtime_start_cancel_and_projections_cover_every_phase() {
    let mut idle = RefreshOperation::<IntData, IntProblem, Task>::new();
    assert_eq!(idle.phase(), RefreshPhase::Idle);
    assert_eq!(idle.data(), None);
    assert_eq!(idle.problem(), None);
    assert!(!idle.is_running());

    idle.transition(Load(Task));
    assert_eq!(idle.phase(), RefreshPhase::Loading);
    assert_eq!(idle.data(), None);
    assert_eq!(idle.problem(), None);
    assert!(idle.is_running());
    idle.transition(Cancel);
    assert_eq!(idle.phase(), RefreshPhase::Idle);

    let mut ready = runtime_ready(IntData(10));
    assert_eq!(ready.phase(), RefreshPhase::Ready);
    assert_eq!(ready.data(), Some(&IntData(10)));
    assert_eq!(ready.problem(), None);
    ready.transition(Refresh(Task));
    assert_eq!(ready.phase(), RefreshPhase::Refreshing);
    assert_eq!(ready.data(), Some(&IntData(10)));
    assert_eq!(ready.problem(), None);
    ready.transition(Cancel);
    assert_eq!(ready.phase(), RefreshPhase::Ready);
    assert_eq!(ready.data(), Some(&IntData(10)));

    let mut unavailable = runtime_unavailable(IntProblem(20));
    assert_eq!(unavailable.phase(), RefreshPhase::Unavailable);
    assert_eq!(unavailable.data(), None);
    assert_eq!(unavailable.problem(), Some(&IntProblem(20)));
    unavailable.transition(Retry(Task));
    assert_eq!(unavailable.phase(), RefreshPhase::Retrying);
    assert_eq!(unavailable.data(), None);
    assert_eq!(unavailable.problem(), Some(&IntProblem(20)));
    unavailable.transition(Cancel);
    assert_eq!(unavailable.phase(), RefreshPhase::Unavailable);
    assert_eq!(unavailable.problem(), Some(&IntProblem(20)));

    let mut degraded = runtime_degraded(IntData(30), IntProblem(31));
    assert_eq!(degraded.phase(), RefreshPhase::Degraded);
    assert_eq!(degraded.data(), Some(&IntData(30)));
    assert_eq!(degraded.problem(), Some(&IntProblem(31)));
    degraded.transition(Refresh(Task));
    assert_eq!(degraded.phase(), RefreshPhase::RefreshingDegraded);
    assert_eq!(degraded.data(), Some(&IntData(30)));
    assert_eq!(degraded.problem(), Some(&IntProblem(31)));
    degraded.transition(Cancel);
    assert_eq!(degraded.phase(), RefreshPhase::Degraded);
    assert_eq!(degraded.data(), Some(&IntData(30)));
    assert_eq!(degraded.problem(), Some(&IntProblem(31)));

    degraded.transition(Cancel);
    assert_eq!(degraded.phase(), RefreshPhase::Degraded);
}

#[test]
fn runtime_completion_covers_every_running_phase_and_result() {
    let mut loading_success = RefreshOperation::<IntData, IntProblem, Task>::new();
    loading_success.transition(Load(Task));
    loading_success.transition(Complete(Ok(IntData(1))));
    assert_eq!(loading_success.phase(), RefreshPhase::Ready);
    assert_eq!(loading_success.data(), Some(&IntData(1)));

    let mut loading_failure = RefreshOperation::<IntData, IntProblem, Task>::new();
    loading_failure.transition(Load(Task));
    loading_failure.transition(Complete(Err(IntProblem(2))));
    assert_eq!(loading_failure.phase(), RefreshPhase::Unavailable);
    assert_eq!(loading_failure.problem(), Some(&IntProblem(2)));

    let mut refreshing_success = runtime_ready(IntData(3));
    refreshing_success.transition(Refresh(Task));
    refreshing_success.transition(Complete(Ok(IntData(4))));
    assert_eq!(refreshing_success.phase(), RefreshPhase::Ready);
    assert_eq!(refreshing_success.data(), Some(&IntData(4)));

    let mut refreshing_failure = runtime_ready(IntData(5));
    refreshing_failure.transition(Refresh(Task));
    refreshing_failure.transition(Complete(Err(IntProblem(6))));
    assert_eq!(refreshing_failure.phase(), RefreshPhase::Degraded);
    assert_eq!(refreshing_failure.data(), Some(&IntData(5)));
    assert_eq!(refreshing_failure.problem(), Some(&IntProblem(6)));

    let mut retrying_success = runtime_unavailable(IntProblem(7));
    retrying_success.transition(Retry(Task));
    retrying_success.transition(Complete(Ok(IntData(8))));
    assert_eq!(retrying_success.phase(), RefreshPhase::Ready);
    assert_eq!(retrying_success.data(), Some(&IntData(8)));
    assert_eq!(retrying_success.problem(), None);

    let mut retrying_failure = runtime_unavailable(IntProblem(9));
    retrying_failure.transition(Retry(Task));
    retrying_failure.transition(Complete(Err(IntProblem(10))));
    assert_eq!(retrying_failure.phase(), RefreshPhase::Unavailable);
    assert_eq!(retrying_failure.problem(), Some(&IntProblem(10)));

    let mut degraded_success = runtime_degraded(IntData(11), IntProblem(12));
    degraded_success.transition(Refresh(Task));
    degraded_success.transition(Complete(Ok(IntData(13))));
    assert_eq!(degraded_success.phase(), RefreshPhase::Ready);
    assert_eq!(degraded_success.data(), Some(&IntData(13)));
    assert_eq!(degraded_success.problem(), None);

    let mut degraded_failure = runtime_degraded(IntData(14), IntProblem(15));
    degraded_failure.transition(Refresh(Task));
    degraded_failure.transition(Complete(Err(IntProblem(16))));
    assert_eq!(degraded_failure.phase(), RefreshPhase::Degraded);
    assert_eq!(degraded_failure.data(), Some(&IntData(14)));
    assert_eq!(degraded_failure.problem(), Some(&IntProblem(16)));
}

#[test]
fn runtime_ignores_invalid_messages_and_drops_their_payloads() {
    struct TrackedTask {
        drops: Rc<Cell<usize>>,
    }

    impl Drop for TrackedTask {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    let active_drops = Rc::new(Cell::new(0));
    let ignored_drops = Rc::new(Cell::new(0));
    let mut running = RefreshOperation::<(), IntProblem, TrackedTask>::new();
    running.transition(Load(TrackedTask {
        drops: active_drops.clone(),
    }));

    running.transition(Load(TrackedTask {
        drops: ignored_drops.clone(),
    }));
    assert_eq!(running.phase(), RefreshPhase::Loading);
    assert_eq!(ignored_drops.get(), 1);

    running.transition(Cancel);
    assert_eq!(active_drops.get(), 1);

    struct TrackedData {
        drops: Rc<Cell<usize>>,
    }

    impl Drop for TrackedData {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    let data_drops = Rc::new(Cell::new(0));
    let mut settled = RefreshOperation::<TrackedData, IntProblem, Task>::new();
    settled.transition(Complete(Ok(TrackedData {
        drops: data_drops.clone(),
    })));
    assert_eq!(settled.phase(), RefreshPhase::Idle);
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

    struct NoDefaultTask;

    let operation: RefreshOperation<NoDefaultData, NoDefaultProblem, NoDefaultTask> =
        Default::default();
    assert_eq!(operation.phase(), RefreshPhase::Idle);
}

#[test]
fn runtime_accepts_non_clone_non_send_payloads_and_empty_data() {
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

    #[allow(dead_code)]
    struct LocalTask(Rc<()>);

    let value = Rc::new(Cell::new(42));
    let mut operation = RefreshOperation::<LocalData, LocalProblem, LocalTask>::new();
    operation.transition(Load(LocalTask(Rc::new(()))));
    operation.transition(Complete(Ok(LocalData(value.clone()))));
    assert_eq!(operation.data().unwrap().0.get(), 42);

    operation.transition(Refresh(LocalTask(Rc::new(()))));
    operation.transition(Complete(Err(LocalProblem(Rc::new(())))));
    assert_eq!(operation.phase(), RefreshPhase::Degraded);
    assert_eq!(operation.data().unwrap().0.get(), 42);
    assert!(Rc::strong_count(&operation.problem().unwrap().0) >= 1);

    let mut empty = RefreshOperation::<Vec<i32>, IntProblem, Task>::new();
    empty.transition(Load(Task));
    empty.transition(Complete(Ok(Vec::new())));
    assert_eq!(empty.phase(), RefreshPhase::Ready);
    assert!(empty.data().unwrap().is_empty());
}

#[test]
fn runtime_applies_domain_messages_only_while_ready() {
    let mut operation = runtime_ready(IntData(40));
    let RefreshOperation::Ready(ready) = &mut operation else {
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
fn runtime_complete_and_cancel_drop_task_after_installing_final_state() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let mut operation = RefreshOperation::<OrderedData, IntProblem, OrderedTask>::new();

    operation.transition(Load(OrderedTask {
        name: "load-task",
        order: order.clone(),
    }));
    operation.transition(Complete(Ok(OrderedData {
        name: "previous",
        order: order.clone(),
    })));
    assert_eq!(&*order.borrow(), &["load-task"]);

    order.borrow_mut().clear();
    operation.transition(Refresh(OrderedTask {
        name: "refresh-task",
        order: order.clone(),
    }));
    operation.transition(Complete(Ok(OrderedData {
        name: "next",
        order: order.clone(),
    })));
    assert_eq!(
        &*order.borrow(),
        &["refresh-task", "previous"],
        "the task must be dropped before obsolete data"
    );
    assert_eq!(operation.phase(), RefreshPhase::Ready);

    order.borrow_mut().clear();
    operation.transition(Refresh(OrderedTask {
        name: "cancel-task",
        order: order.clone(),
    }));
    operation.transition(Cancel);
    assert_eq!(&*order.borrow(), &["cancel-task"]);
    assert_eq!(operation.phase(), RefreshPhase::Ready);
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

    let mut completing = RefreshOperation::<IntData, IntProblem, MaybePanickingTask>::new();
    completing.transition(Load(MaybePanickingTask {
        panic_on_drop: true,
    }));

    let completion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        completing.transition(Complete(Ok(IntData(42))));
    }));
    assert!(completion.is_err());
    assert_eq!(completing.phase(), RefreshPhase::Ready);
    assert_eq!(completing.data(), Some(&IntData(42)));

    let mut cancelling = RefreshOperation::<IntData, IntProblem, MaybePanickingTask>::new();
    cancelling.transition(Load(MaybePanickingTask {
        panic_on_drop: true,
    }));

    let cancellation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cancelling.transition(Cancel);
    }));
    assert!(cancellation.is_err());
    assert_eq!(cancelling.phase(), RefreshPhase::Idle);
}
