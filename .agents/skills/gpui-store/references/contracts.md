# gpui-store contracts

Consult the section relevant to the task and any invariants it depends on. Current public docs and exported code remain the implemented authority; report conflicts rather than silently replacing the contract.

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

Use the [guide](../../../../crates/gpui-store/docs/guide.md) for construction and read/mutation signatures.

- A typed-global Store is keyed by `S`; install once before retrieval. Object-local stores are passed by shared handle.
- Reads are synchronous borrows; no reference to `S` escapes.
- `set` and `update` always publish. `update_if` publishes only `StoreChange::Changed`; it neither compares nor rolls back state. Returning `Unchanged` promises no observable mutation occurred.
- Mutation and notification form one Store operation; do not expose an external mutable reference or separate publication step.

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
