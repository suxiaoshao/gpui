---
name: gpui-store
description: Use when implementing, reviewing, debugging, documenting, or integrating crates/gpui-store or application state deliberately stored in Store. Covers typed shared in-memory ownership, explicit mutation and publication, Select and StoreSelection, observation, typed globals, lifetime rules, and persistence/form boundaries.
---

# GPUI Store

Use this repo-owned skill for `crates/gpui-store` and for application code that
deliberately adopts `Store<S>`.

Do not use it merely because code contains ordinary GPUI entity state. Use the
`gpui` skill for framework APIs and the app's existing state conventions unless
the task specifically concerns `gpui-store`.

## Source of truth and required reading

Before changing the crate API or implementation, read:

1. `crates/gpui-store/README.md`
2. `crates/gpui-store/docs/guide.md`
3. `crates/gpui-store/src/lib.rs`
4. the affected implementation and tests under `crates/gpui-store/src/`

Read `crates/gpui-store/dev/in-memory-store.md` when changing architecture or
executing its implementation plan. The English public documentation and current
exported code are the contract; keep the Chinese README and guide semantically
aligned when public documentation changes.

Before integrating a Store into an app, inspect the complete current state flow,
all consumers, and the intended ownership lifetime. Do not migrate app state
opportunistically during an unrelated task.

## Core model

- `Store<S>` owns one authoritative in-memory `S`. Any `'static` Rust type can
  be stored; there is no state marker trait.
- `S` does not need `Clone`, `PartialEq`, `Default`, `Send`, or `Sync`.
  Individual APIs constrain only the values they actually retain or compare.
- Cloning `Store<S>` clones a cheap shared handle, not `S`; every handle refers
  to the same hidden GPUI Entity and state.
- Keep `S` domain-shaped. Store the typed value consumers need, not only a
  revision, event, or invalidation flag that makes every consumer rebuild a
  private cache.
- The crate owns in-memory state and publication only. It does not perform I/O,
  persistence, task execution, retry, repair, transactions, or rollback.
- Services and repositories execute domain commands. Publish the committed
  in-memory result through the Store afterward.

## Construction and mutation

- Use `Store::new(cx, state)` for a store whose lifetime is defined by an
  application object or component. Pass or clone its handle explicitly.
- Use `Store::install_global(cx, state)` only for genuinely application-wide
  state, then retrieve it with `Store::<S>::global(cx)`. The state type is the
  global key; install at most one global Store for each `S`. `global` panics
  when installation has not happened.
- Use `read(cx, |state| ...)` for one synchronous borrow. Return copied, cloned,
  or newly computed data; do not retain a reference to `S`.
- Use `set` to replace all of `S`; it always publishes.
- Use `update` only for a mutation known to change observable state; it always
  publishes and returns the closure's business result.
- Use `update_if` when the command decides whether publication is needed.
  Return `StoreChange::Changed(result)` or `Unchanged(result)`.
- `update_if` does not clone, compare, or roll back `S`. Returning `Unchanged`
  commits to having made no observable mutation.
- Mutation and notification are one Store operation. Do not expose an external
  mutable reference or publish separately.

## Selection and observation

- `Select<S>` is a pure deterministic projection. Functions and `Fn(&S) -> T`
  closures implement it automatically; use a named selector only when a
  projection is reused.
- Keep selectors free of I/O, fallible work, unrelated entity reads, and side
  effects. Perform that work in an application command and publish its result.
- `Store::select` creates an owner-bound, read-only
  `StoreSelection<Output>`. `Output` must implement `PartialEq`; the owner is
  notified only when the selected output changes. Creation computes the initial
  output synchronously without an initial owner notification.
- Read a selection through `StoreSelection::read`; call `cloned` only when the
  output implements `Clone` and an owned value is required. A selection has no
  setter and is never a second source of truth. It does not keep the source
  Store alive; if the source disappears first, the selection retains its last
  value and stops updating.
- Use `observe` when every Store publication matters. Use `observe_select` when
  only a projected value matters, and `observe_select_in` when the callback also
  needs a `Window`.
- Observation schedules one initial delivery with the current value, then later
  deliveries. Keep the returned `Subscription` in its owner. Observation
  callbacks decide whether to call the owner's `cx.notify()`; observation does
  not do that automatically.
- A whole-store callback holds a borrow of `S`; synchronously mutating the same
  Store from that callback panics. Defer the command. Selected callbacks run
  after the source borrow is released but must still avoid feedback loops.
- Dropping a selection or subscription stops future delivery.

Choose by intent:

| Need | API |
| --- | --- |
| Read once | `Store::read` |
| Render one derived value and skip unrelated redraws | `Store::select` |
| React to every publication | `Store::observe` |
| Run a side effect when one projection changes | `Store::observe_select` |
| Run that selected side effect with a Window | `Store::observe_select_in` |

## Application boundaries

- Keep shared application/domain state authoritative in one Store. A component
  may retain interaction-local UI state, but do not mirror shared data in
  multiple mutable caches.
- Store mutation is not persistence. Complete file, database, or network work
  first, then publish the committed result. The application owns optimistic
  update and rollback policy.
- `gpui-form` owns editable values, validation, baseline, and submit
  preparation. Rebase a form explicitly from committed Store data; submit
  through the application service, update the Store, then rebase the saved
  value. There is no automatic form synchronization.
- Catalog selections may supply control options but must not replace a form
  value, choose a fallback, or rebase a form merely because the catalog changed.
- Loading, error, retry, task, or other domain semantics belong to the stored
  state and application layer. Do not add those concepts to Store itself.
- A typed-global Store still publishes through its private Store Entity.
  Mutating it is not a GPUI Global replacement; consumers use Store selection
  or observation.

## Removed concepts

Do not reintroduce the previous architecture or compatibility aliases:

- `LocalStore`, `SharedStore`, `StoreState`, or separate local/shared ownership
  variants;
- `StoreBackend`, `StoreCommitBackend`, `StoreBackendBuilder`,
  `StoreBackendFuture`, backend IDs, snapshots, reconciliation, or refresh
  methods;
- `StoreBinding`, writable selections, `try_set`, `try_update`,
  `try_update_if`, or committed field APIs;
- `read_cloned`, `select_cloned`, `refresh_from_backend`, `sync_snapshot`,
  `reconcile_replace`, or `reconcile_field`;
- revisions, deltas, actions, reducers, middleware, mutation origins, commit
  acknowledgements, transactions, or automatic persistence.

## Validation

For implementation changes run:

```sh
cargo fmt
cargo test -p gpui-store
cargo check -p gpui-store
cargo clippy -p gpui-store --all-targets -- -D warnings
git diff --check
```

For app integration, also run focused tests and checks for the touched app. For
docs- or skill-only changes, validate links, English/Chinese semantic parity,
removed-term residue, skill structure, and `git diff --check`; crate tests are
not required.
