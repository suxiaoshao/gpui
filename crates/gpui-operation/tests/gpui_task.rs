use std::{
    cell::{Cell, RefCell},
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use gpui::{App, AppContext, Task, TestAppContext};
use gpui_operation::{Cancel, Complete, Load, Repair, Transition, refresh, repair};

// ── Fixtures ────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
struct IntData(i32);

#[derive(Debug, Clone, PartialEq)]
struct IntProblem(i32);

impl fmt::Display for IntProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "problem-{}", self.0)
    }
}

impl std::error::Error for IntProblem {}

type AttemptTask = Task<Result<IntData, IntProblem>>;
type RefreshOperation = refresh::Operation<IntData, IntProblem, AttemptTask>;

#[derive(Default)]
struct AttemptControl {
    started: Cell<bool>,
    dropped: Cell<bool>,
    ready: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl AttemptControl {
    fn complete(&self) {
        self.ready.set(true);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

struct ControlledAttempt {
    control: Rc<AttemptControl>,
    value: i32,
}

impl Future for ControlledAttempt {
    type Output = Result<IntData, IntProblem>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        self.control.started.set(true);
        if self.control.ready.get() {
            Poll::Ready(Ok(IntData(self.value)))
        } else {
            *self.control.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Drop for ControlledAttempt {
    fn drop(&mut self) {
        self.control.dropped.set(true);
        self.control.waker.borrow_mut().take();
    }
}

// ── OP-R5: Cancel drops GPUI task, completion is not routed ─────────────

#[gpui::test]
fn cancel_drops_pending_gpui_task(cx: &mut TestAppContext) {
    let control = Rc::new(AttemptControl::default());
    let task: AttemptTask = cx.update(|cx: &mut App| {
        let attempt = ControlledAttempt {
            control: control.clone(),
            value: 1,
        };
        cx.spawn(async move |_cx| attempt.await)
    });

    let mut operation = RefreshOperation::new();
    operation.transition(Load(task));
    cx.run_until_parked();
    assert!(control.started.get(), "attempt must have reached Pending");
    assert!(!control.dropped.get());

    operation.transition(Cancel);
    cx.run_until_parked();
    assert!(
        control.dropped.get(),
        "dropping the running state must cancel and drop its pending future"
    );
}

#[gpui::test]
fn cancelled_gpui_task_cannot_deliver_completion(cx: &mut TestAppContext) {
    let control = Rc::new(AttemptControl::default());
    let completion_delivered = Rc::new(Cell::new(false));
    let delivered = completion_delivered.clone();
    let task: AttemptTask = cx.update(|cx: &mut App| {
        let attempt = ControlledAttempt {
            control: control.clone(),
            value: 2,
        };
        cx.spawn(async move |_cx| {
            let result = attempt.await;
            delivered.set(true);
            result
        })
    });

    let mut operation = RefreshOperation::new();
    operation.transition(Load(task));
    cx.run_until_parked();
    assert!(control.started.get(), "attempt must have reached Pending");

    operation.transition(Cancel);
    control.complete();
    cx.run_until_parked();

    assert!(
        !completion_delivered.get(),
        "a cancelled attempt must not reach its completion route"
    );
}

// ── OP-R4: Task completion via self-replacement pattern ─────────────────

/// A minimal Entity owner that holds a running operation state.
struct Owner {
    operation: RefreshOperation,
    completed_flag: Rc<Cell<bool>>,
}

#[gpui::test]
fn task_completion_via_entity_self_replacement(cx: &mut TestAppContext) {
    let completed_flag = Rc::new(Cell::new(false));
    let completed_capture = completed_flag.clone();

    let (entity, weak) = cx.update(|cx: &mut App| {
        let entity = cx.new(move |_| Owner {
            operation: RefreshOperation::new(),
            completed_flag: completed_capture.clone(),
        });
        let weak = entity.downgrade();
        (entity, weak)
    });

    // Start a load with a task that completes and updates the owner.
    cx.update(|cx: &mut App| {
        let task: AttemptTask = cx.spawn({
            let weak = weak.clone();
            async move |cx: &mut gpui::AsyncApp| -> Result<IntData, IntProblem> {
                if let Some(entity) = weak.upgrade() {
                    entity.update(cx, |owner, cx| {
                        owner.operation.transition(Complete(Ok(IntData(42))));
                        owner.completed_flag.set(true);
                        cx.notify();
                    });
                }
                Ok(IntData(42))
            }
        });
        entity.update(cx, |owner, cx| {
            owner.operation.transition(Load(task));
            cx.notify();
        });
    });

    // Let the spawned task complete.
    cx.run_until_parked();

    let is_ready = cx.update(|cx: &mut App| {
        entity.read_with(cx, |owner, _| {
            owner.operation.phase() == refresh::Phase::Ready
        })
    });
    assert!(
        is_ready,
        "entity should be in Ready state after async completion"
    );

    let data = cx.update(|cx: &mut App| {
        entity.read_with(cx, |owner, _| owner.operation.data().map(|data| data.0))
    });
    assert_eq!(data, Some(42));
    assert!(completed_flag.get(), "completion flag should have been set");
}

// ── OP-R2: Repair cancel restores problem with real GPUI task ───────────

struct RepairProbe {
    dropped: Rc<Cell<bool>>,
}

impl Drop for RepairProbe {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

#[gpui::test]
fn repair_cancel_drops_pending_task_and_repair_then_restores_problem(cx: &mut TestAppContext) {
    let mut operation: repair::Operation<IntData, IntProblem, RepairProbe, AttemptTask> =
        repair::Operation::new();
    let load_task: AttemptTask = cx.update(|cx: &mut App| {
        cx.spawn(async move |_cx| -> Result<IntData, IntProblem> { Err(IntProblem(7)) })
    });
    operation.transition(Load(load_task));
    operation.transition(Complete(Err(IntProblem(7))));

    let control = Rc::new(AttemptControl::default());
    let repair_task: AttemptTask = cx.update(|cx: &mut App| {
        let attempt = ControlledAttempt {
            control: control.clone(),
            value: 8,
        };
        cx.spawn(async move |_cx| attempt.await)
    });
    let repair_dropped = Rc::new(Cell::new(false));

    operation.transition(Repair {
        repair: RepairProbe {
            dropped: repair_dropped.clone(),
        },
        task: repair_task,
    });
    cx.run_until_parked();
    assert!(
        control.started.get(),
        "repair task must have reached Pending"
    );

    operation.transition(Cancel);
    cx.run_until_parked();
    assert!(control.dropped.get(), "repair task must be dropped");
    assert!(repair_dropped.get(), "selected repair must be dropped");
    assert_eq!(operation.problem(), Some(&IntProblem(7)));
}
