# gpui-operation contracts

Consult the section relevant to the task and any invariants it depends on. Current public docs and exported code remain the implemented authority; report conflicts rather than silently replacing the contract.

## Choose the family

Use `refresh` for recovery that repeats the same read; use `repair` when the caller must choose a repair value. Do not model absence of repair as `Repair = ()` or make the library choose recovery policy.

The [guide](../../../../crates/gpui-operation/docs/guide.md) owns the family variants and legal message matrix. `Ready` owns valid data; `Unavailable` has a problem without valid data; `Degraded` retains last-known-good data and the latest problem. Empty data is successful data. Running states retain the exact settled state and task, plus the selected repair where applicable.

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
- Use the guide for constructors, accessors and legal message combinations. Keep runtime messages explicit; do not add command-style wrappers, `can_*` methods or ownership-return rejection APIs.

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

See the [guide](../../../../crates/gpui-operation/docs/guide.md) for the Ready business-message example.

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
