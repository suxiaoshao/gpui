# gpui-operation user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

This guide documents the public API implemented by `gpui-operation` 0.1.
Examples use application-defined Data, Problem, Repair, and task sources to
keep the ownership and transition patterns concrete.

## 1. Purpose

`gpui-operation` describes safe state transitions for fallible asynchronous
work. It is a transition library, not an executor or owner:

```text
current state + message -> next state
```

The caller is responsible for:

- constructing and starting the task;
- delivering its eventual result as a message;
- choosing an Entity, ordinary Global, `gpui-store::Store`, local
  variable, or custom owner;
- publishing owner notifications; and
- deciding which product actions are available for each Problem.

The library provides one complete runtime `Operation` enum for each family.
It does not define `OperationSource`, await a task, install an owner, route a
completion, or persist data. It deliberately provides no universal
cross-family `Operation<S>`.

## 2. Two operation families

Failure has two materially different product meanings:

| Family | Failure recovery | Typical examples |
| --- | --- | --- |
| `refresh` | Run the same read again | Database query, catalog, remote metadata |
| `repair` | Select an explicit recovery | Malformed config, database open or migration failure |

The distinction is part of the public type system. A refresh-only operation
has no Repair type and no repair transition. A repair-capable operation does
not pretend that `Repair = ()` means “no repair”.

Both families preserve the same facts:

- `Ready` contains valid Data.
- `Unavailable` contains a Problem and no valid Data.
- `Degraded` contains last-known-good Data and the latest Problem.
- a running state owns both its previous settled state and its Task.

## 3. Messages and transitions

Both API layers use the same transition trait:

```rust
pub trait Transition<Message> {
    type Output;

    fn transition(self, message: Message) -> Self::Output;
}
```

For a named state, `self` is the owned state and the transition consumes it.
For a complete runtime enum, the receiver is `&mut Operation`, the transition
replaces its current variant in place, and `Output = ()`.

The public messages are:

```rust
pub struct Load<Task>(pub Task);
pub struct Refresh<Task>(pub Task);
pub struct Retry<Task>(pub Task);

pub struct Repair<Kind, Task> {
    pub repair: Kind,
    pub task: Task,
}

pub struct Complete<Data, Problem: std::error::Error>(
    pub Result<Data, Problem>,
);

pub struct Cancel;
```

Messages own their payloads. Transitions move those payloads into the next
state; they do not require Data, Problem, Repair, or Task to implement
`Clone`, `PartialEq`, `Send`, or `Sync`. Problem must implement
`std::error::Error`.

The complete runtime enums accept the messages valid for their family:

| Runtime family | In-place messages |
| --- | --- |
| `refresh::Operation` | `Load`, `Refresh`, `Retry`, `Complete`, `Cancel` |
| `repair::Operation` | `Load`, `Refresh`, `Repair`, `Complete`, `Cancel` |

A message that is not valid for the current runtime variant leaves the
operation unchanged and drops the message and its owned payload. There is no
alternate return path; runtime delivery returns `()`. Applications should
match the current variant before constructing work whose payload must not be
dropped. The optional `tracing` feature emits a debug event containing the
family, phase, and message type when a runtime message is ignored.

`Fetching<Previous, Task>` represents an active load, refresh, or retry.
`Repairing<Previous, Repair, Task>` represents an active repair. `Load`,
`Refresh`, `Retry`, and `Repair` are the messages that enter those running
states.

Named states expose borrowed payloads. The table uses both families' exact
generic forms:

| State | Borrowed API |
| --- | --- |
| `refresh::Ready<Data>` / `repair::Ready<Data, Repair>` | `data() -> &Data` |
| `refresh::Unavailable<Problem>` / `repair::Unavailable<Problem, Repair>` | `problem() -> &Problem` |
| `refresh::Degraded<Data, Problem>` / `repair::Degraded<Data, Problem, Repair>` | `data() -> &Data`, `problem() -> &Problem` |
| either family's `Fetching<Previous, Task>` | `previous() -> &Previous` |
| `repair::Repairing<Previous, Repair, Task>` | `previous() -> &Previous`, `repair() -> &Repair` |

These projections do not clone payloads or expose mutable references.
Section 7 shows how an exact `Ready` state accepts application-defined domain
messages without exposing its Data mutably.

## 4. Refresh-only operations

Use `gpui_operation::refresh` when retrying means running the same read again.

### 4.1 State graph

The settled states are:

```rust
refresh::Idle
refresh::Ready<Data>
refresh::Unavailable<Problem>
refresh::Degraded<Data, Problem>
```

Every running state has one representation:

```rust
refresh::Fetching<Previous, Task>
```

`Previous` is the exact settled state from which the task started:

```text
Idle
  + Load<Task>
  -> Fetching<Idle, Task>

Ready<Data>
  + Refresh<Task>
  -> Fetching<Ready<Data>, Task>

Unavailable<Problem>
  + Retry<Task>
  -> Fetching<Unavailable<Problem>, Task>

Degraded<Data, Problem>
  + Refresh<Task>
  -> Fetching<Degraded<Data, Problem>, Task>
```

This representation makes cancellation uniform:

```rust
impl<Previous, Task> Transition<Cancel>
    for refresh::Fetching<Previous, Task>
{
    type Output = Previous;
}
```

The transition consumes and drops Task, then returns `Previous`. It does not
reconstruct the previous state from a status flag.

### 4.2 Completion

When no previous Data exists, completion returns:

```rust
pub enum FetchCompleted<Data, Problem: std::error::Error> {
    Ready(refresh::Ready<Data>),
    Unavailable(refresh::Unavailable<Problem>),
}
```

This applies to first load and retry from `Unavailable`:

```text
Fetching<Idle, Task> + Complete(Ok(new data))
  -> Ready(new data)

Fetching<Idle, Task> + Complete(Err(new problem))
  -> Unavailable(new problem)

Fetching<Unavailable(old problem), Task> + Complete(Ok(new data))
  -> Ready(new data)

Fetching<Unavailable(old problem), Task> + Complete(Err(new problem))
  -> Unavailable(new problem)
```

When previous Data exists, completion returns:

```rust
pub enum RefreshCompleted<Data, Problem: std::error::Error> {
    Ready(refresh::Ready<Data>),
    Degraded(refresh::Degraded<Data, Problem>),
}
```

A successful refresh replaces old Data. A failed refresh moves old Data into
`Degraded` with the new Problem.

### 4.3 Direct use

```rust
use gpui_operation::{Complete, Load, Refresh, Transition};
use gpui_operation::refresh::{
    FetchCompleted, Idle, RefreshCompleted,
};

let loading = Idle::new().transition(Load(load_task));

let ready = match loading.transition(Complete(Ok(initial_data))) {
    FetchCompleted::Ready(ready) => ready,
    FetchCompleted::Unavailable(_) => unreachable!(),
};

let refreshing = ready.transition(Refresh(refresh_task));

let settled = match refreshing.transition(Complete(refresh_result)) {
    RefreshCompleted::Ready(ready) => CatalogState::Ready(ready),
    RefreshCompleted::Degraded(degraded) => {
        CatalogState::Degraded(degraded)
    }
};
```

## 5. Repair-capable operations

Use `gpui_operation::repair` when a Problem may require an explicit recovery
chosen by the caller.

### 5.1 State graph

The settled states are:

```rust
repair::Idle<Repair>
repair::Ready<Data, Repair>
repair::Unavailable<Problem, Repair>
repair::Degraded<Data, Problem, Repair>
```

Normal first load and refresh use:

```rust
repair::Fetching<Previous, Task>
```

An explicit repair uses:

```rust
repair::Repairing<Previous, Repair, Task>
```

Legal starts are:

```text
Idle<Repair>
  + Load<Task>
  -> Fetching<Idle<Repair>, Task>

Ready<Data, Repair>
  + Refresh<Task>
  -> Fetching<Ready<Data, Repair>, Task>

Unavailable<Problem, Repair>
  + Repair<Repair, Task>
  -> Repairing<Unavailable<Problem, Repair>, Repair, Task>

Degraded<Data, Problem, Repair>
  + Repair<Repair, Task>
  -> Repairing<Degraded<Data, Problem, Repair>, Repair, Task>
```

The Repair type is part of this family, so a runtime owner cannot accidentally
mix repair values belonging to different resources.

### 5.2 Completion and cancellation

First load and normal refresh use family-specific result types:

```rust
pub enum FetchCompleted<Data, Problem: std::error::Error, Repair> {
    Ready(repair::Ready<Data, Repair>),
    Unavailable(repair::Unavailable<Problem, Repair>),
}

pub enum RefreshCompleted<Data, Problem: std::error::Error, Repair> {
    Ready(repair::Ready<Data, Repair>),
    Degraded(repair::Degraded<Data, Problem, Repair>),
}
```

A normal refresh from `Ready` succeeds with new Data or fails with old Data
preserved in `Degraded`.

Repair completion preserves the exact set of legal destinations. Repairing
without previous Data returns:

```rust
pub enum RepairWithoutDataCompleted<
    Data,
    Problem: std::error::Error,
    Repair,
> {
    Ready(repair::Ready<Data, Repair>),
    Unavailable(repair::Unavailable<Problem, Repair>),
}
```

Repairing with previous Data returns:

```rust
pub enum RepairWithDataCompleted<
    Data,
    Problem: std::error::Error,
    Repair,
> {
    Ready(repair::Ready<Data, Repair>),
    Degraded(repair::Degraded<Data, Problem, Repair>),
}
```

The possible failure variant is determined by `Previous`:

```text
Repairing<Unavailable(old problem), repair, task>
  + Complete(Err(new problem))
  -> Unavailable(new problem)

Repairing<Degraded(old data, old problem), repair, task>
  + Complete(Err(new problem))
  -> Degraded(old data, new problem)
```

Success always produces `Ready(new data)`.

`Cancel` drops Task and Repair and returns the exact `Previous` state:

```text
Repairing<Unavailable(problem), repair, task> + Cancel
  -> Unavailable(problem)

Repairing<Degraded(data, problem), repair, task> + Cancel
  -> Degraded(data, problem)
```

### 5.3 Direct use

```rust
use gpui_operation::{Complete, Repair, Transition};
use gpui_operation::repair::RepairWithoutDataCompleted;

enum DatabaseRepair {
    RetryOpen,
    RestoreBackup,
    Recreate,
}

let repairing = unavailable.transition(Repair {
    repair: DatabaseRepair::RestoreBackup,
    task,
});

let settled = match repairing.transition(Complete(result)) {
    RepairWithoutDataCompleted::Ready(ready) => {
        DatabaseState::Ready(ready)
    }
    RepairWithoutDataCompleted::Unavailable(unavailable) => {
        DatabaseState::Unavailable(unavailable)
    }
};
```

## 6. Task and cancellation contract

The library treats Task as an owned generic value. It does not poll, abort, or
inspect it. Entering a running state moves Task into that state; completing or
cancelling consumes the running state and drops Task.

For GPUI `Task`, dropping the handle cancels the task. Therefore the intended
UI contract is:

1. a running state is the only owner of its task;
2. the application matches the current runtime variant before it constructs
   the attempt and Task;
3. constructing Task must not synchronously re-enter the owner or deliver
   `Complete` before the start message stores it;
4. the owner delivers exactly one legal `Load`, `Refresh`, `Retry`, or
   `Repair` message without yielding between the check and transition;
5. cancellation restores the settled state, drops the current task, and then
   returns;
6. only after that transition may the caller start another task; and
7. a cancelled task cannot later deliver `Complete`.

Under this contract there is no stale-completion state, attempt identifier,
generation counter, opaque Completion, or acceptance check.

If a custom runtime handle does not cancel its work when dropped, the caller
must wrap it with the required abort-on-drop behavior. Detached producers that
can deliver after cancellation are outside this contract and must perform
their own generation check.

External side effects already performed by a file, database, network service,
or repair action are not rolled back by dropping Task.

## 7. Two predefined runtime enums

The named states provide compile-time-safe `Transition<Message>` calls when
owned directly. An Entity, Global, Store, or ordinary field that needs
long-term state uses the two complete enums provided by the library:

```rust
use gpui::Task;
use gpui_operation::{refresh, repair};

type CatalogOperation =
    refresh::Operation<CatalogData, CatalogProblem, Task<()>>;

type DatabaseOperation = repair::Operation<
    Database,
    DatabaseProblem,
    DatabaseRepair,
    Task<()>,
>;
```

`refresh::Operation` has eight public variants:

```text
Idle / Loading
Ready / Refreshing
Unavailable / Retrying
Degraded / RefreshingDegraded
```

`repair::Operation` also has eight public variants:

```text
Idle / Loading
Ready / Refreshing
Unavailable / RepairingUnavailable
Degraded / RepairingDegraded
```

Both enums provide:

- `new` / `default`, which starts at `Idle` without requiring any payload to
  implement `Default`;
- `phase`, which returns a comparable, copyable family-specific `Phase`;
- `data` / `problem`, which borrow current valid Data or the latest Problem;
- `is_running`; and
- `active_repair` on the repair family.

The enums implement `Transition<Message>` for `&mut Operation`. The
refresh-only runtime routes explicit messages as follows:

```text
Idle + Load<Task> -> Loading
Ready + Refresh<Task> -> Refreshing
Unavailable + Retry<Task> -> Retrying
Degraded + Refresh<Task> -> RefreshingDegraded
running + Complete<Result<Data, Problem>> -> settled
running + Cancel -> exact previous settled state
```

The repair-capable runtime uses `Load` from `Idle`, `Refresh` from `Ready`,
and `Repair { repair, task }` from `Unavailable` or `Degraded`. `Complete`
and `Cancel` apply to all of its running variants.

Runtime delivery is intentionally one-way:

```rust
use gpui_operation::{Complete, Load, Transition, refresh};

let mut operation =
    refresh::Operation::<CatalogData, CatalogProblem, CatalogTask>::new();

operation.transition(Load(task));
operation.transition(Complete(result));
```

An invalid runtime message preserves the current variant and drops the
message. This makes a delivered Task, Repair, or completion result owned by
the state machine even when application code sends it in the wrong phase.
Match first when that would be a product error; enable `tracing` when ignored
message diagnostics are useful.

Accepted completion and cancellation install the final valid state before
dropping Task, Repair, or obsolete previous payloads. A re-entrant or
panicking generic destructor therefore cannot observe a temporary `Idle`.

### 7.1 Domain messages for exact Ready data

Committed domain changes sometimes need to update in-memory Data without
cloning the complete catalog or exposing `&mut Data`. Define a domain message
on the Data type:

```rust
use gpui_operation::Transition;

struct ReplaceRecord(Record);

impl Transition<ReplaceRecord> for &mut CatalogData {
    type Output = ();

    fn transition(self, message: ReplaceRecord) {
        self.insert_or_replace(message.0);
    }
}
```

Both families implement the corresponding delegation for
`&mut Ready<Data>`. The caller must first match the exact runtime variant:

```rust
if let refresh::Operation::Ready(ready) = &mut operation {
    ready.transition(ReplaceRecord(committed_record));
}
```

There is no mutable Data accessor. Data retained by `Refreshing`, `Degraded`,
or a degraded repair remains read-only, so an application cannot accidentally
publish a committed mutation into a non-Ready lifecycle phase.

The following examples only define domain wrappers, not runtime enums:

```rust
struct CatalogResource {
    operation: CatalogOperation,
    repository: CatalogRepository,
}

struct DatabaseResource {
    operation: DatabaseOperation,
    database: DatabaseService,
}
```

## 8. Using an Entity

Use an Entity when the resource belongs to a component, document, window, or
another independently bounded object.

### 8.1 Refresh-only Entity

```rust
use gpui::{Context, Entity};

impl CatalogResource {
    fn load(&mut self, cx: &mut Context<Self>) {
        let refresh::Operation::Idle(_) = &self.operation else {
            return;
        };

        let attempt = self.repository.fetch();
        let task = cx.spawn(async move |owner, cx| {
            let result = attempt.await;

            let _ = owner.update(cx, |owner, cx| {
                let refresh::Operation::Loading(_) =
                    &owner.operation
                else {
                    return;
                };
                owner.operation.transition(Complete(result));
                cx.notify();
            });
        });

        self.operation.transition(Load(task));
        cx.notify();
    }
}

let catalog: Entity<CatalogResource> =
    cx.new(|_| CatalogResource::new(repository));

catalog.update(cx, |catalog, cx| catalog.load(cx));

let has_data = catalog.read(cx).operation.data().is_some();
```

This command matches `Idle` before constructing work, then sends `Load`
directly. A refresh or retry command follows the same shape but matches
`Ready`/`Degraded` or `Unavailable` and sends `Refresh` or `Retry`.

Components observe the Entity and store the returned subscription:

```rust
let subscription = cx.observe(&catalog, |_view, _catalog, cx| {
    cx.notify();
});
```

### 8.2 Repair-capable Entity

```rust
impl DatabaseResource {
    fn repair(
        &mut self,
        repair: DatabaseRepair,
        cx: &mut Context<Self>,
    ) {
        let problem = match &self.operation {
            repair::Operation::Unavailable(state) => state.problem(),
            repair::Operation::Degraded(state) => state.problem(),
            _ => return,
        };

        // The application constructs an owned future from borrowed inputs.
        // DatabaseRepair itself remains available for the running state.
        let attempt = self.database.repair_attempt(problem, &repair);

        let task = cx.spawn(async move |owner, cx| {
            let result = attempt.await;

            let _ = owner.update(cx, |owner, cx| {
                match &owner.operation {
                    repair::Operation::RepairingUnavailable(_)
                    | repair::Operation::RepairingDegraded(_) => {}
                    _ => return,
                }
                owner.operation.transition(Complete(result));
                cx.notify();
            });
        });

        self.operation.transition(Repair { repair, task });
        cx.notify();
    }
}
```

The transition API does not require Data, Problem, or Repair to implement
`Clone`. `repair_attempt` is application code that returns an owned future;
it may extract owned request values, clone selected inputs, or share them with
`Arc` as its own runtime requires. It is not a trait hook from this crate.
Normal first load and refresh use the same Entity pattern as the catalog.

To expose cancellation, the Entity matches one of its exact running variants,
then delivers `Cancel` and calls `cx.notify()`. Delivering `Cancel` to a
settled operation would leave it unchanged, but the match avoids a
meaningless notification.

## 9. Using an ordinary Global

Use an ordinary GPUI Global when the resource has application lifetime and
there is naturally one instance for the process.

### 9.1 Refresh-only Global

```rust
use gpui::{
    App, AppContext as _, BorrowAppContext as _, Global,
};

struct CatalogGlobal(CatalogResource);
impl Global for CatalogGlobal {}

fn install_catalog(repository: CatalogRepository, cx: &mut App) {
    cx.set_global(CatalogGlobal(CatalogResource::new(repository)));
}

fn refresh_catalog(cx: &mut App) {
    let catalog = cx.global::<CatalogGlobal>();
    let refresh::Operation::Ready(_) = &catalog.0.operation else {
        return;
    };

    let attempt = catalog.0.repository.fetch();
    let task = cx.spawn(async move |cx| {
        let result = attempt.await;

        cx.update_global::<CatalogGlobal, _>(|catalog, _| {
            let refresh::Operation::Refreshing(_) =
                &catalog.0.operation
            else {
                return;
            };
            catalog.0.operation.transition(Complete(result));
        });
    });

    cx.update_global::<CatalogGlobal, _>(|catalog, _| {
        catalog.0.operation.transition(Refresh(task));
    });
}
```

`update_global` publishes to Global observers. The read-only match avoids
constructing refresh work outside `Ready`, and the mutation sends `Refresh`
directly.

A component observes the Global:

```rust
let subscription = cx.observe_global::<CatalogGlobal>(|_view, cx| {
    cx.notify();
});
```

### 9.2 Repair-capable Global

```rust
struct DatabaseGlobal(DatabaseResource);
impl Global for DatabaseGlobal {}

fn repair_database(repair: DatabaseRepair, cx: &mut App) {
    let Some(attempt) = cx.read_global::<DatabaseGlobal, _>(
        |database, _| {
            let problem = match &database.0.operation {
                repair::Operation::Unavailable(state) => {
                    state.problem()
                }
                repair::Operation::Degraded(state) => state.problem(),
                _ => return None,
            };
            Some(database.0.database.repair_attempt(problem, &repair))
        },
    ) else {
        return;
    };

    let task = cx.spawn(async move |cx| {
        let result = attempt.await;

        cx.update_global::<DatabaseGlobal, _>(|database, _| {
            match &database.0.operation {
                repair::Operation::RepairingUnavailable(_)
                | repair::Operation::RepairingDegraded(_) => {}
                _ => return,
            }
            database.0.operation.transition(Complete(result));
        });
    });

    cx.update_global::<DatabaseGlobal, _>(|database, _| {
        database
            .0
            .operation
            .transition(Repair { repair, task });
    });
}
```

The task does not capture the Global. It finds the one application-lifetime
owner through the GPUI context when it completes.

## 10. Using gpui-store

This section uses the current `gpui-store` API: `Store<S>`,
`Store::install_global`, `Store::global`, `read`, `update`, `update_if`,
`select`, and `observe`.

Use Store when several consumers need the same operation state and benefit
from store-native reads, selections, or observation. The operation remains one
field in the authoritative Store state; do not copy Data into another Store.

### 10.1 Refresh-only Store

Install one typed global Store:

```rust
use gpui_store::Store;

Store::install_global(
    cx,
    CatalogResource::new(repository),
);
```

Start a retry:

```rust
fn retry_catalog_store(cx: &mut App) {
    let catalog = Store::<CatalogResource>::global(cx);

    let Some(attempt) = catalog.read(cx, |resource| {
        let refresh::Operation::Unavailable(_) =
            &resource.operation
        else {
            return None;
        };

        Some(resource.repository.fetch())
    }) else {
        return;
    };

    let task = cx.spawn(async move |cx| {
        let result = attempt.await;
        let catalog = Store::<CatalogResource>::global(cx);

        catalog.update(cx, |resource| {
            let refresh::Operation::Retrying(_) =
                &resource.operation
            else {
                return;
            };
            resource.operation.transition(Complete(result));
        });
    });

    catalog.update(cx, |resource| {
        resource.operation.transition(Retry(task));
    });
}
```

The task does not capture a strong Store handle. It retrieves the typed global
Store when completion is ready. The read matches `Unavailable` before
constructing work, and Store `update` sends `Retry` directly. Load, refresh,
and cancellation use the same owner route with their explicit messages.

Consumers may select only the render state they need:

```rust
let status = catalog.select(
    cx,
    |resource: &CatalogResource| resource.operation.phase(),
);
```

### 10.2 Repair-capable Store

```rust
Store::install_global(
    cx,
    DatabaseResource::new(database_service),
);

fn repair_database_store(
    repair: DatabaseRepair,
    cx: &mut App,
) {
    let database = Store::<DatabaseResource>::global(cx);

    let Some(attempt) = database.read(cx, |resource| {
        let problem = match &resource.operation {
            repair::Operation::Unavailable(state) => state.problem(),
            repair::Operation::Degraded(state) => state.problem(),
            _ => return None,
        };
        Some(resource.database.repair_attempt(problem, &repair))
    }) else {
        return;
    };

    let task = cx.spawn(async move |cx| {
        let result = attempt.await;
        let database = Store::<DatabaseResource>::global(cx);

        database.update(cx, |resource| {
            match &resource.operation {
                repair::Operation::RepairingUnavailable(_)
                | repair::Operation::RepairingDegraded(_) => {}
                _ => return,
            }
            resource.operation.transition(Complete(result));
        });
    });

    database.update(cx, |resource| {
        resource
            .operation
            .transition(Repair { repair, task });
    });
}
```

The Store does not load the database or choose a Repair. Application commands
construct work and mutate the Store with pure state transitions. Store remains
responsible only for shared in-memory ownership and publication.

A non-global Store needs an application-chosen completion route to the same
Store instance. `gpui-store` does not expose a weak Store handle, so the
example uses its typed-global lookup. An application can instead route
completion through another owner that already holds the Store.

## 11. Choosing an owner

| Need | Owner |
| --- | --- |
| Component-, document-, or window-scoped lifetime | Entity |
| Exactly one process-wide resource | Ordinary Global |
| Shared reads, selections, and observation | `Store<S>`; the examples above use a typed-global Store |
| No observation and short lexical lifetime | Local variable |

Owner choice does not change the state machine:

- the owner matches a legal start variant before constructing the task;
- the running state owns Task;
- task completion returns to the same owner;
- the owner applies `Complete(result)` synchronously; and
- the owner publishes after each message it deliberately delivers.

## 12. Dependencies and runtime choice

Dependencies are application inputs to task construction:

```rust
let attempt = repository.fetch_catalog();
let task = cx.spawn(async move |owner, cx| {
    let result = attempt.await;
    // Deliver Complete(result) to the chosen owner.
});
```

The transition crate has no dependency graph or `Waiting` state. If a
dependency is unavailable, the application does not construct or send the
start message.

Task is generic. A caller may use:

- a GPUI `Task<()>`;
- a GPUI task bridging work from `gpui-tokio`;
- another abort-on-drop handle that satisfies section 6.

Runtime-specific `Send` or `Sync` requirements remain local to task
construction and are not imposed on every state payload.

## 13. Product policy

The state machine reports facts:

- `Ready` has valid Data.
- `Unavailable` has no valid Data.
- `Degraded` has valid previous Data and a newer Problem.
- `Fetching` or `Repairing` is running and owns a cancellable task.

The application decides:

- whether degraded Data remains usable;
- which Problems are shown to the user;
- whether cancellation is exposed;
- which Repair choices require confirmation;
- how repair side effects are explained; and
- whether a resource is required for application startup.

The library never fabricates default Data after failure.

## 14. Non-goals

This crate does not provide:

- `OperationSource` or Source hooks;
- a universal cross-family `Operation<S>` enum;
- command-style `start_*`, `complete`, `cancel`, or `can_*` runtime methods;
- task spawning, awaiting, routing, or runtime selection;
- attempt identifiers or stale-completion reconciliation;
- automatic startup, retry, refresh, repair, or cancellation;
- Entity, Global, or Store adapters;
- persistence, transactions, or rollback;
- a generic dependency graph or `Waiting` state;
- observation, selections, or notifications; or
- blanket payload `Clone`, `PartialEq`, `Send`, or `Sync` bounds.

## Related documentation

- [`gpui-operation` README](../README.md)
- [中文 README](../README.zh-CN.md)
- [Target `gpui-store` guide](../../gpui-store/docs/guide.md)
- [`gpui-form` guide](../../gpui-form/docs/guide.md)
