# gpui-store user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

This guide documents the public API implemented by `gpui-store` 0.1.
Examples use application-defined domain types to show how the Store fits into
an actual GPUI owner.

## 1. Purpose

`gpui-store` provides one abstraction:

```text
Store<S>
  owns one authoritative in-memory S
  exposes controlled reads and mutations
  publishes explicit changes
  supports read-only derived selections
```

It is useful when multiple GPUI components or services need the same runtime
state and need to react when it changes.

A store does not assign domain meaning to the value it contains. It only owns
the in-memory `S` and publishes mutations according to the Store API.

## 2. Responsibility model

The application should have one mutable source of truth for a value:

```text
commands ──mutate──> Store<S> ──publishes──> components and observers
                         │
                         └──derives──> StoreSelection<T>
```

`StoreSelection<T>` is disposable derived state. It can be recomputed from
`S`, has no setter, and must not become another authoritative value.

The crate is intentionally limited to in-memory ownership and observation:

| Concern | Owner |
| --- | --- |
| Shared in-memory state | `gpui-store` |
| Editable form value, validation, and submit preparation | `gpui-form` |
| File/database/network writes and transactions | Application service or repository |
| UI-local interaction state | Component |

## 3. State types

Any `'static` Rust type can be stored. There is no marker trait:

```rust
struct WorkspaceState {
    active_project: Option<ProjectId>,
    sidebar_open: bool,
    pending_navigation: Option<Route>,
}
```

`S` does not need to implement `Clone`, `PartialEq`, `Default`, `Send`, or
`Sync`. Individual operations add constraints only to the values they
actually need:

- a read can return a copied, cloned, or newly computed value;
- a selection output needs `PartialEq` so unchanged outputs can be filtered;
- `StoreSelection::cloned` is available when its output implements `Clone`.

Keep state domain-shaped. Store the typed value consumers need, rather than
only a revision number or an invalidation flag that forces each consumer to
rebuild its own cache.

## 4. Creating and sharing a store

Create a store with an initial valid in-memory value:

```rust
use gpui_store::Store;

let workspace = Store::new(
    cx,
    WorkspaceState {
        active_project: None,
        sidebar_open: true,
        pending_navigation: None,
    },
);
```

`Store::new` does not perform I/O. The caller must already have a value that is
valid for the in-memory model.

`Store<S>` is a shared handle. Clone the handle to give another component or
service access to the same state:

```rust
struct Sidebar {
    workspace: Store<WorkspaceState>,
}

let sidebar = Sidebar {
    workspace: workspace.clone(),
};
```

Cloning the handle does not clone `S` and does not create another source of
truth.

## 5. Typed global stores

State that is application-wide can be installed once as a typed global:

```rust
Store::install_global(
    cx,
    WorkspaceState {
        active_project: None,
        sidebar_open: true,
        pending_navigation: None,
    },
);
```

Retrieve it after installation:

```rust
let workspace = Store::<WorkspaceState>::global(cx);
```

The state type is the global key. Install at most one global `Store<S>` for a
given `S`, and make installation order explicit during application startup.
Prefer passing a store handle directly when state is only shared by a bounded
part of the application. `Store::global` follows GPUI typed-global semantics
and panics if that `Store<S>` has not been installed.

## 6. Reading state

All reads happen through a closure:

```rust
let active_project = workspace.read(cx, |state| state.active_project);

let route_label = workspace.read(cx, |state| {
    state
        .pending_navigation
        .as_ref()
        .map(Route::label)
        .unwrap_or("No pending navigation")
        .to_owned()
});
```

The closure may return:

- a copied scalar or identifier;
- an owned clone of a field;
- a newly computed view model;
- any other result that does not retain the borrowed `S`.

Keeping the borrow inside the closure makes the lifetime of a read explicit
and prevents a caller from holding a reference across a later mutation.

## 7. Mutating state

All mutations go through the store. The API has three mutation shapes:

| Method | Meaning | Notification |
| --- | --- | --- |
| `set` | Replace the complete `S` | Always |
| `update` | Mutate the current `S` and return a business value | Always |
| `update_if` | Mutate and return a business value together with the change decision | Only for `StoreChange::Changed` |

The conditional result is explicit:

```rust
pub enum StoreChange<R> {
    Changed(R),
    Unchanged(R),
}
```

`StoreChange<R>` keeps the business result and notification decision in the
same mutation. The store returns that outcome to the caller; it does not
require a side channel or a second read.

### 7.1 Replacing the complete state

Use `set` when the caller already has the next complete in-memory value:

```rust
workspace.set(
    cx,
    WorkspaceState {
        active_project: Some(project_id),
        sidebar_open: true,
        pending_navigation: None,
    },
);
```

`set` always publishes. It does not compare old and new values and therefore
does not require `S: PartialEq`. The new value is installed before the old
value is dropped, and the old value is dropped outside the Store Entity lease.
If the old value's destructor panics, the panic propagates, the new value
remains installed, and notification may already have been queued.

### 7.2 Updating with a guaranteed change

Use `update` when the operation is known to change observable state. Its
closure may return a business value:

```rust
let sidebar_open = workspace.update(cx, |state| {
    state.sidebar_open = !state.sidebar_open;
    state.sidebar_open
});
```

`update` always publishes after the closure completes. Do not use it for an
operation that may be a no-op.

### 7.3 Updating conditionally

Use `update_if` when the caller knows how to decide whether a notification is
necessary:

```rust
use gpui_store::StoreChange;

let outcome = workspace.update_if(cx, |state| {
    let previous = state.active_project;

    if previous == Some(project_id) {
        return StoreChange::unchanged(previous);
    }

    state.active_project = Some(project_id);
    StoreChange::changed(previous)
});

let changed = outcome.is_changed();
let previous = outcome.into_result();
```

The `Changed` or `Unchanged` variant is the same decision used for
notification, while `R` remains available to the caller. `update_if` does not
clone `S`, compare the whole state, or roll back mutations. The closure must
return `Unchanged` only when it leaves the observable state unchanged.

This keeps equality local to the operation. A large state can compare one
field, a collection can compare a stable key, and a domain type can use its own
semantic equality without imposing a whole-state trait bound.

### 7.4 Notification is part of the mutation contract

Store mutation and notification are one operation. Callers cannot obtain a
mutable reference and change `S` without choosing one of the notification
semantics above.

Mutation closures and destructors are not caught or wrapped. A panic
propagates to the caller. `update` and `update_if` only notify after their
closures return normally.

The store does not expose revisions, mutation origins, actions, reducers, or
change sets. If the domain needs those concepts, model them explicitly as part
of `S` or in the command layer.

## 8. Reusable selectors

A selector computes a value from borrowed state:

```rust
pub trait Select<S: ?Sized> {
    type Output;

    fn select(&self, source: &S) -> Self::Output;
}
```

Functions and `Fn` closures implement `Select` automatically:

```rust
let sidebar_open = workspace.select(
    cx,
    |state: &WorkspaceState| state.sidebar_open,
);
```

Use a named selector when the same projection is shared by several consumers:

```rust
use gpui_store::Select;

#[derive(Clone, Copy)]
struct IsProjectActive(ProjectId);

impl Select<WorkspaceState> for IsProjectActive {
    type Output = bool;

    fn select(&self, state: &WorkspaceState) -> Self::Output {
        state.active_project == Some(self.0)
    }
}

let is_active = workspace.select(cx, IsProjectActive(project_id));
```

`Select` itself does not require `Clone`, `PartialEq`, or `'static`. An API that
stores a selector or its output adds only the bounds required for that use.

Selectors should be:

- pure and free of side effects;
- cheap enough to run after relevant store notifications;
- deterministic for the same `S`;
- independent of file, database, network, or unrelated entity reads.

If computing an output requires asynchronous work or can fail, perform that
work outside the selector and publish its result through an explicit store
mutation.

## 9. StoreSelection

`Store::select` creates a read-only `StoreSelection<T>` owned by the calling
component:

```rust
use gpui_store::{Store, StoreSelection};

struct WorkspaceHeader {
    workspace: Store<WorkspaceState>,
    active_project: StoreSelection<Option<ProjectId>>,
}

impl WorkspaceHeader {
    fn new(workspace: Store<WorkspaceState>, cx: &mut Context<Self>) -> Self {
        let active_project =
            workspace.select(cx, |state: &WorkspaceState| state.active_project);

        Self {
            workspace,
            active_project,
        }
    }
}
```

The selection:

1. computes its initial output when it is created;
2. recomputes after the source store publishes;
3. compares the new output with the previous output;
4. notifies its owner only when the output changed.

This means an unrelated store update does not redraw a component that only
uses the selection.

Read a selected output through a closure:

```rust
let has_project = self
    .active_project
    .read(|project_id| project_id.is_some());
```

Clone the output only when an owned value is needed:

```rust
let active_project = self.active_project.cloned();
```

`StoreSelection<T>` requires `T: PartialEq + 'static` for change filtering.
`cloned` additionally requires `T: Clone`.

A selection has no `set`, `update`, or mutable reference. Commands always
write through the source `Store<S>`.

## 10. Observing changes

Selections are for values a component reads while rendering. Observation is
for synchronizing a side effect with the current value and later changes.

Keep every returned `Subscription` in the owner for as long as the observation
must remain active.

The observation contract schedules one initial delivery after
registration, then delivers later publications or selected-value changes. The
first callback receives the current value at delivery time, is never called
reentrantly during registration, and runs before any later change callback.
Changes before that first callback may coalesce into its current value. This
lets a controller establish its derived state without a separate bootstrap
read.

### 10.1 Whole-store observation

Use `observe` when every published store change matters:

```rust
struct WorkspaceController {
    workspace: Store<WorkspaceState>,
    subscriptions: Vec<Subscription>,
}

let subscription = workspace.observe(cx, |this, state, cx| {
    this.rebuild_commands(state);
    cx.notify();
});

self.subscriptions.push(subscription);
```

The callback first receives the current `S` in the scheduled initial delivery,
then runs for every notification, including a `set` or `update` whose result
happens to compare equal by domain rules. Use selected observation when only
one projection matters.

### 10.2 Selected-value observation

Use `observe_select` to run a side effect only when a selected output changes:

```rust
let subscription = workspace.observe_select(
    cx,
    |state: &WorkspaceState| state.active_project,
    |this, active_project, cx| {
        this.rebuild_project_actions(*active_project);
        cx.notify();
    },
);
```

The selector output must implement `PartialEq + 'static`. The callback receives
the new selected output.

The observer schedules one callback with the current output after
registration. After that, it invokes the callback only when `PartialEq`
reports that the selected output changed.

### 10.3 Observation with a window

Use `observe_select_in` when the side effect also requires `Window`:

```rust
let subscription = workspace.observe_select_in(
    cx,
    window,
    |state: &WorkspaceState| state.sidebar_open,
    |this, sidebar_open, window, cx| {
        this.sync_sidebar_focus(*sidebar_open, window, cx);
    },
);
```

An observer callback decides whether to call `cx.notify()`. The observation
API does not assume that every side effect changes the owner's rendered state.

Dropping the returned `Subscription` inside its own callback is supported.
The current callback finishes, and no later callback is delivered. Closing the
target Window similarly cancels `observe_select_in`, whether initial delivery
is still pending or the observation is already active.

A whole-store callback holds the `&S` borrow it receives, so synchronously
writing to that same Store from the callback is a programmer error and
panics. Defer that command until after the callback. A selected callback runs
after the source `S` borrow is released and may issue an explicit command, but
must still avoid feedback loops.

Avoid feedback loops. If an observer must issue another command, make the
command boundary explicit and ensure it cannot continuously republish the same
selected value.

## 11. Choosing selection or observation

| Need | API |
| --- | --- |
| Read any part of state once | `Store::read` |
| Render from a derived value and skip unrelated redraws | `Store::select` |
| React to every store notification | `Store::observe` |
| Run a side effect only when one derived value changes | `Store::observe_select` |
| Run that selected side effect with `Window` | `Store::observe_select_in` |

Do not create a selection only to call a side effect, and do not observe a
whole store when a small stable selector expresses the dependency.

## 12. Persistence and commands

Store mutation is not persistence. `set`, `update`, and `update_if` only change
memory and cannot fail because a file or database write failed.

For a durable write, execute the domain command first and publish the committed
result afterward:

```rust
let saved_project = repository.save_project(input).await?;

projects.update(cx, |state| {
    state.replace(saved_project);
});
```

The application decides whether optimistic updates are appropriate and how to
roll them back. `gpui-store` does not provide backend, commit, reconcile, ack,
or transaction abstractions.

## 13. Forms

`gpui-form` owns the current editable model, its validation state, baseline,
and submit preparation. A store may own the last committed application value,
but it must not silently mirror every keystroke.

The explicit integration flow is:

```text
committed Store value
  -> form.rebase(committed value)

form.prepare_submit()
  -> application command or repository
  -> committed result
  -> Store::set/update
```

Catalog selections can supply options to a control. They must not replace the
form's selected value or rebase the form merely because the catalog store
changed.

There is no writable `StoreBinding` in the public API. Use typed form fields
for editable values and explicit store commands for application state.

## 14. Ownership and lifetime

- Clone `Store<S>` when several owners need the same state.
- Keep a non-global store handle in the application object or component that
  defines its lifetime.
- Install a global store only for genuinely application-wide state.
- Keep `StoreSelection<T>` in the component that renders it.
- Keep returned `Subscription` values in the observing owner.
- Dropping a selection or subscription stops that derived observation.
- Dropping one store handle does not affect other handles to the same store.

Avoid hidden mirrors. A component may cache interaction-local UI state, but
shared domain or application state should still have one authoritative owner.

## 15. Public API summary

The public surface is intentionally small.

### `Store<S>`

| API | Purpose |
| --- | --- |
| `Store::new` | Create a shared store from an existing valid `S` |
| `Store::install_global` | Create and install a typed global store |
| `Store::global` | Retrieve the typed global store |
| `Clone` | Share the handle without cloning `S` |
| `read` | Borrow `S` for one closure |
| `set` | Replace `S` and publish |
| `update` | Mutate `S`, return the closure result, and publish |
| `update_if` | Mutate and atomically return the result plus publish decision |
| `select` | Create an owner-bound read-only derived selection |
| `observe` | Observe every store notification |
| `observe_select` | Observe changes to a selected output |
| `observe_select_in` | Observe a selected output with `Window` |

### `Select<S>`

| API | Purpose |
| --- | --- |
| `type Output` | The derived output type |
| `select(&self, &S)` | Compute an output from borrowed state |
| Blanket `Fn` implementation | Use functions and closures without adapters |

### `StoreChange<R>`

| API | Purpose |
| --- | --- |
| `changed` | Return a business result and publish the mutation |
| `unchanged` | Return a business result without publishing |
| `is_changed` | Inspect the notification decision |
| `into_result` | Consume the outcome and return its business result |

### `StoreSelection<T>`

| API | Purpose |
| --- | --- |
| `read` | Borrow the current selected output for one closure |
| `cloned` | Clone the current output when `T: Clone` |

## 16. Non-goals

This crate does not provide:

- local and shared store variants;
- marker traits for state;
- external backends or source adapters;
- persistence, commit, transactions, reconciliation, or write acknowledgements;
- revisions, mutation origins, reducers, actions, middleware, or deltas;
- writable selections, bindings, or automatic form synchronization;
- render-time implicit dependency collection.

These exclusions keep `Store<S>` a predictable in-memory primitive that other
libraries can build on without competing for ownership of the same data.
