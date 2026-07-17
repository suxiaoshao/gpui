---
name: gpui-store
description: Use when implementing, reviewing, debugging, or integrating the gpui-store crate. Covers LocalStore vs SharedStore ownership, StoreSelection and StoreBinding, StoreBackend synchronization and commit semantics, and GPUI notification behavior.
---

# GPUI Store

Use this skill for `crates/gpui-store` itself and for app code that is deliberately adopting `gpui-store`.

Do not use this skill just because code has ordinary GPUI entity state. Use `gpui` for entity/global framework guidance, or the app's existing state pattern, unless the task is specifically about `gpui-store`.

## Required Reading

Before changing `gpui-store`, read:

1. `crates/gpui-store/README.md`
2. `crates/gpui-store/docs/development-plan.md`
3. The affected implementation files under `crates/gpui-store/src/`

Before integrating `gpui-store` into an app, also read the current app state flow first. Do not replace app state with `gpui-store` unless the task explicitly calls for that integration.

## Core Model

- `StoreState` is a marker trait. Domain state stays a normal Rust type.
- Memory-only users update state directly with `set`, `update`, `update_if`, or memory `StoreBinding`.
- Committed backend users update state through `try_set`, `try_update`, `try_update_if`,
  `try_update_field`, or committed `StoreBinding`.
- The store owns equality checks, revision bumps, `cx.notify()`, selector refresh, and commit coordination.
- `StoreSelection<T>` is a read-only subscribed snapshot.
- `StoreBinding<T, Error = Infallible>` is a writable subscribed snapshot for lens-like fields.
- `StoreBackend<S>` is the external synchronization extension point. Users implement it for files, databases, S3, HTTP, keychain, repository projections, or app-specific backends.
- `StoreCommitBackend<S>` is the optional committed write capability. Only implement it when generic local drafts can be safely committed through the backend.
- Backend type is orthogonal to ownership: both `LocalStore` and `SharedStore` can be memory-only or backend-backed.

## Source of Truth and Form Integration

- A store state must contain the actual typed committed snapshot needed by its consumers. A
  revision/event-only entity is metadata, not a catalog source; do not make each page query and cache
  its own rows, labels, or capabilities.
- `StoreSelection<T>` is a one-way read projection. It may cache render data, but it has no business
  setter and must not be read by submit/validation as a replacement for the committed store or form
  draft.
- `StoreBinding<T>` is only for an intentional writable lens whose setter writes back to the same
  backing store. Do not use it to create a form mirror or a second persistence owner.
- There is no implicit form↔store synchronization. A committed domain value may
  be explicitly installed as a new form baseline, but catalog/options snapshots
  are dependencies and must never hydrate/rebase the form. Form submit commits
  through the app, then the store reconciles the committed snapshot.
- Backend/selection observers read and compute, then replace their own snapshot or notify. They must
  not recursively update the source entity while it is already in `Entity::update`; schedule an
  explicit cross-entity command/task when another entity must change.
- For the Jaco #175 ownership migration, read the app state flow first and follow
  `app/jaco/docs/dev/issue-175/state-ownership-sync-plan.md`; do not add a generic cross-store
  selector or a `gpui-store` dependency on `gpui-form` as a shortcut.

## Ownership Choice

Use `LocalStore<S, Backend>` when:

- One component owns the state.
- Other components do not need to subscribe to it directly.
- The owner's `cx.notify()` is the correct render invalidation boundary.
- Backend lifecycle should be tied to the owner component.

Use `SharedStore<S, Backend>` when:

- Multiple components need to read or write the same state.
- The store needs its own GPUI entity lifetime.
- The store may be installed as an app global.
- Backend lifecycle should outlive any single consumer component.

Do not create a `SharedStore` only because state is persisted. Persistence is a `StoreBackend` / `StoreCommitBackend` concern, not an ownership concern.

## Selection And Binding

- Use `StoreSelection<T>` for derived values with no meaningful inverse write: filtered rows, counts, booleans, labels, and computed view models.
- Use `StoreBinding<T>` for writable fields where the setter updates the same value that the getter observes.
- Memory `StoreBinding::set` writes a requested value into the backing store. Committed bindings use `try_set` / `try_update` and must handle backend errors.
- If a setter normalizes, clamps, or maps the requested value, the binding is valid as long as the getter-observable value changes when the update should count.
- If an update changes unrelated state while the binding getter returns the same value, use store-level `update_if` / `try_update_if` instead of `StoreBinding`.
- `StoreSelection<T>` and `StoreBinding<T>` are owner-bound handles that keep subscriptions. They should feel like read smart pointers, but they should not expose mutable references.

## StoreBackend Rules

- Implement `StoreBackend<S>` for backend synchronization. Do not add built-in file, database, S3, or HTTP store kinds unless there is a reusable adapter with clear value.
- `load` hydrates initial state.
- `subscribe` registers external change notifications when the backend can push updates.
- `load_after_event` converts an external event into a snapshot.
- `reconcile` mutates store state from a snapshot and returns whether it really changed.
- Implement `StoreCommitBackend<S>` only when local draft updates should be committed generically.
- Committed writes must clone a draft, commit it, then update the store after success. A commit error must leave the store unchanged and must not notify.
- Projection backends should usually implement only `StoreBackend<S>` and refresh with `refresh_from_backend` or `sync_snapshot` after domain repository commands.
- `StoreBackendBuilder` is a convenience adapter. Prefer `reconcile_replace` and
  `reconcile_field` before writing hand-rolled equality/replacement closures.

Current `StoreBackendFuture<T, Error>` is a synchronous `Result<T, Error>` alias. Do not assume async backend I/O without first updating the crate design and tests.

## GPUI Semantics

- Only call `cx.notify()` when the store or selected snapshot really changed.
- Keep store mutations inside GPUI contexts.
- Avoid render-time reads from unrelated entities. Use `StoreSelection` snapshots for subscribed render data.
- Use `observe_select` / `observe_select_in` for selected side effects instead of observing whole store entities when only one field matters.
- Keep subscriptions and backend handles owned by the store handle or owner component so they are dropped with the correct lifecycle.

## Validation

For changes to `crates/gpui-store`, run:

```sh
cargo fmt
cargo test -p gpui-store
cargo check -p gpui-store
cargo clippy -p gpui-store --all-targets -- -D warnings
git diff --check
```

For app integration, also run the focused app tests/checks for the touched app.
