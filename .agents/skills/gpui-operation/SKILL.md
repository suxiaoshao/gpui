---
name: gpui-operation
description: Use when implementing, reviewing, debugging, documenting, or deliberately integrating crates/gpui-operation or gpui_operation::{refresh, repair}. Covers family selection, message-driven complete runtime and named-state transitions, Task ownership, Ready business messages, cancellation, completion routing, optional tracing, and Entity, Global, and gpui-store owner boundaries. Do not use for ordinary GPUI async state unless the code deliberately adopts this crate.
---

# GPUI Operation

Use this repo-owned skill for the transition crate and application code that
deliberately stores one of its operation families.

The crate models caller-controlled fallible work. It owns state matching,
payload movement, cancellation, and restoration; the application owns the
runtime, task construction, completion route, notifications, and product policy.

## Source of truth and required reading

Before changing the public API or implementation, read:

1. `crates/gpui-operation/README.md`
2. `crates/gpui-operation/docs/guide.md`
3. `crates/gpui-operation/src/lib.rs`
4. the affected family implementation:
   `src/refresh.rs` or `src/repair.rs`
5. the matching tests under `crates/gpui-operation/tests/`

Read `tests/gpui_task.rs` for GPUI Task ownership, cancellation, or Entity
completion work. Read
`crates/gpui-operation/dev/message-driven-transitions.md` when changing the
architecture or its implementation plan. The English public documentation and
current exported code are the contract; keep the Chinese README and guide
semantically aligned when public documentation changes.

Before application integration, inspect the existing owner, state flow,
consumers, runtime, and UI. Use the `gpui` skill for Entity, Global, async, and
notification APIs. Use the `gpui-store` skill when an application deliberately
stores an Operation inside a Store, while keeping both crate responsibilities
separate.

## Choose the family

Use `refresh::Operation<Data, Problem, Task>` when recovery means running the
same read again: queries, catalogs, and remote metadata.

Its phases are:

```text
Idle / Loading
Ready / Refreshing
Unavailable / Retrying
Degraded / RefreshingDegraded
```

Deliver an explicit message matching the settled state:

```text
Idle        + Load(task)
Ready       + Refresh(task)
Unavailable + Retry(task)
Degraded    + Refresh(task)
```

The library does not combine these meanings into a `start_fetch` command.

Use `repair::Operation<Data, Problem, Repair, Task>` when a Problem requires an
explicit caller-selected recovery, such as restoring or resetting malformed
configuration or repairing a database.

Its phases are:

```text
Idle / Loading
Ready / Refreshing
Unavailable / RepairingUnavailable
Degraded / RepairingDegraded
```

Deliver `Load(task)` from `Idle`, `Refresh(task)` from `Ready`, and a
caller-selected `Repair { repair, task }` from `Unavailable` or `Degraded`.
Do not model “no repair” as `Repair = ()`, send a normal retry from a
problem-bearing repair state, or make the library choose a Repair.

Both families preserve the same facts:

- `Ready` contains valid Data.
- `Unavailable` contains a Problem and no valid Data.
- `Degraded` contains last-known-good Data and the latest Problem.
- A running state owns its exact previous settled state and Task.
- A repair running state additionally owns the caller-selected Repair.

Successful completion produces `Ready` with new Data. Failure without previous
Data produces `Unavailable`; failure with previous Data preserves it in
`Degraded` and replaces the Problem. Empty Data is still successful Data.

## Runtime and named-state APIs

- Store the family-provided complete `Operation` enum in any long-lived Entity,
  Global, Store, ordinary field, or local owner. Do not define a second
  isomorphic runtime enum.
- Complete enums implement `Transition<Message>` for `&mut Operation`. Calling
  `operation.transition(message)` mutates the retained enum in place and
  returns `()`, so application owners never use `mem::take` or a temporary
  `Idle`.
- Named states keep consuming `Transition<Message>` implementations. Use those
  when code directly owns one exact state and benefits from a precise output
  type and compile-time rejection of illegal transitions.
- Do not implement consuming `Transition<Message> for Operation`: moving a
  complete enum out of a long-lived owner would push replacement complexity
  back into the application.
- `new` and `default` start at `Idle`.
- `phase()` returns the family-specific comparable, copyable `Phase`.
- `data()` and `problem()` borrow valid current payloads, including the previous
  settled payload retained by applicable running states.
- Both families expose `is_running()`. The repair family additionally exposes
  `active_repair()`, which borrows the selected Repair while one is running.
- There are no command-style runtime methods, `can_*` methods, `Rejected`, or
  compatibility wrappers. Use `Load`, `Refresh`, `Retry`, `Repair`, `Complete`,
  and `Cancel` directly through `Transition`.
- Only `Problem` must implement `std::error::Error`. Do not add blanket
  `Clone`, `PartialEq`, `Default`, `Send`, or `Sync` requirements to Data,
  Problem, Repair, Task, or the complete Operation.

## Ready business messages

Business updates that preserve `Ready` use the same Transition trait without a
mutable Data getter or Clone:

1. The application implements `Transition<BusinessMessage>` for `&mut Data`.
2. Match the complete enum and obtain its exact `Ready` state.
3. Deliver the message to `&mut Ready`; the library delegates it to
   `&mut Data`, and the complete Operation remains `Ready`.

```rust
impl Transition<SelectModel> for &mut CatalogData {
    type Output = ();

    fn transition(self, message: SelectModel) {
        self.select(message.model_id);
    }
}

match &mut resource.operation {
    refresh::Operation::Ready(ready) => ready.transition(message),
    _ => {
        // Render the appropriate loading, problem, or read-only stale state.
    }
}
```

Do not lift arbitrary business messages to the complete Operation. Requiring
the exact Ready match keeps product behavior for every non-Ready state in the
application. Retained data in refreshing and degraded states remains
read-only.

## Task, ignored messages, and cancellation

- Task is an opaque owned handle. The library does not spawn, poll, await,
  abort, inspect, or route it.
- Make the running Operation variant the only owner of the lifecycle-critical
  Task. Do not detach an operation-owned GPUI Task.
- Match the current variant or otherwise establish that the message is legal
  before constructing the owned attempt and driver Task. Construct and install
  the start transition without yielding. Task construction must not
  synchronously re-enter the owner or deliver completion before installation.
- Dropping a GPUI Task cancels it. A custom handle must provide equivalent
  abort-on-drop behavior. A detached producer that can deliver after
  cancellation is outside the crate contract and needs an application-owned
  generation check.
- Under the normal abort-on-drop contract, cancellation drops the only
  completion route, so do not add attempt IDs or stale-completion state.
- An illegal runtime message restores the exact original Operation, optionally
  emits a debug event when the `tracing` feature is enabled, and then drops the
  owned message. This is a programmer-error diagnostic, not normal control
  flow and not an ownership-return API.
- Jaco integrations enable the `gpui-operation/tracing` feature. Tracing occurs
  only after a stable state has been restored and does not format payloads or
  add `Debug` bounds.
- Legal completion and cancellation install the final legal state before
  dropping Task, Repair, or obsolete payloads. Do not duplicate temporary-state
  or destructor-order workarounds in application code.
- Complete runtime implementations must not delegate `Complete` or `Cancel`
  directly to named-state consuming transitions: those transitions drop
  retired values before returning. Runtime transitions install the final enum
  first, then perform every potentially user-defined drop.
- Cancellation does not roll back file, database, network, or repair side
  effects already performed outside the state machine.

## Owner integration

Use the same message shape for every owner:

1. Establish the exact current variant and choose its legal message.
2. Construct an owned attempt using application dependencies.
3. Spawn a driver that awaits the attempt and routes `Complete(result)` back to
   the same owner through `Transition`.
4. Install the driver by delivering `Load`, `Refresh`, `Retry`, or `Repair`.
5. Publish the legal start, completion, cancellation, or Ready business
   transition.

Owner-specific rules:

- Entity: let the GPUI owner-aware task route completion through its weak owner;
  call `cx.notify()` after delivering the legal transition.
- Ordinary Global: look up and update the same typed Global when completion is
  ready.
- `Store<S>`: keep Operation as an ordinary authoritative field of `S`.
  Inspect with `read`, publish legal transitions with `update`, and let the
  application choose the completion route. A typed-global Store can be looked
  up again on completion so the stored Task does not capture a strong Store
  handle and form a cycle.
- Non-global Store: route completion through an application owner that already
  has the correct Store instance; the operation crate provides no locator.
- Dependencies are inputs to task construction. If a dependency is unavailable,
  do not start. Do not add a generic dependency graph or `Waiting` state.

The application may define a narrow source, repository, or service, but the
crate has no `OperationSource` hook. Do not mirror Operation Data, Problem,
phase, loading booleans, or Task in another mutable owner.

## Product and UI policy

The state machine reports facts; the application decides how to use them.
Explicitly map every relevant Phase to product behavior:

- distinguish `Ready(empty)` from `Unavailable`;
- show loading or refreshing while running;
- show Problem and a retry or Repair action when unavailable;
- keep last-known-good Data visible with a degraded warning when the product
  permits degraded use;
- decide whether cancellation is exposed and which Repair choices need
  confirmation.

The library never fabricates default Data, hides a Problem, chooses a Repair,
starts automatically, or publishes owner notifications.

## Non-goals and removed designs

Do not add or reintroduce:

- a universal cross-family `Operation<S>`;
- `OperationSource`, source hooks, or task runtime selection;
- Entity, Global, or Store adapters;
- command-style runtime methods, `can_*`, `Rejected`, or compatibility wrappers;
- consuming `Transition<Message> for Operation`;
- complete-Operation delegation for arbitrary Ready business messages;
- automatic startup, retry, refresh, repair, or cancellation;
- attempt identity or stale-completion reconciliation under the normal Task
  contract;
- persistence, transactions, side-effect rollback, dependency graphs, or
  `Waiting`;
- observation, selections, notifications, or a second application-defined
  runtime state machine.

## Validation

For implementation changes run:

```sh
cargo fmt
cargo test -p gpui-operation
cargo check -p gpui-operation
cargo clippy -p gpui-operation --all-targets --all-features -- -D warnings
git diff --check
```

For application integration, also run focused tests and checks for the touched
app, including state-to-UI coverage for every reachable Phase. For docs- or
skill-only changes, validate links, English/Chinese semantic parity, removed
terms, skill structure, and `git diff --check`; crate tests are not required.
