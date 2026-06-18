---
name: gpui-store
description: Use when implementing, reviewing, debugging, or integrating the gpui-store crate. Covers LocalStore vs SharedStore ownership, StoreSelection and StoreBinding, StoreSource external synchronization, and GPUI notification semantics.
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
- Users update state directly with `set`, `update`, `update_if`, or `StoreBinding`.
- The store owns equality checks, revision bumps, `cx.notify()`, selector refresh, and write-back coordination.
- `StoreSelection<T>` is a read-only subscribed snapshot.
- `StoreBinding<T>` is a writable subscribed snapshot for lens-like fields.
- `StoreSource<S>` is the external synchronization extension point. Users implement it for files, databases, S3, HTTP, keychain, repository projections, or app-specific backends.
- Source type is orthogonal to ownership: both `LocalStore` and `SharedStore` can be memory-only or source-backed.

## Ownership Choice

Use `LocalStore<S, Source>` when:

- One component owns the state.
- Other components do not need to subscribe to it directly.
- The owner's `cx.notify()` is the correct render invalidation boundary.
- Source lifecycle should be tied to the owner component.

Use `SharedStore<S, Source>` when:

- Multiple components need to read or write the same state.
- The store needs its own GPUI entity lifetime.
- The store may be installed as an app global.
- Source lifecycle should outlive any single consumer component.

Do not create a `SharedStore` only because state is persisted. Persistence is a `StoreSource` concern, not an ownership concern.

## Selection And Binding

- Use `StoreSelection<T>` for derived values with no meaningful inverse write: filtered rows, counts, booleans, labels, and computed view models.
- Use `StoreBinding<T>` for writable fields where the setter updates the same value that the getter observes.
- `StoreBinding::set` writes a requested value into the backing store. The final truth is the getter result after the setter runs.
- If a setter normalizes, clamps, or maps the requested value, the binding is valid as long as the getter-observable value changes when the update should count.
- If an update changes unrelated state while the binding getter returns the same value, use store-level `update_if` instead of `StoreBinding`.
- `StoreSelection<T>` and `StoreBinding<T>` are owner-bound handles that keep subscriptions. They should feel like read smart pointers, but they should not expose mutable references.

## StoreSource Rules

- Implement `StoreSource<S>` for backend synchronization. Do not add built-in file, database, S3, or HTTP store kinds unless there is a reusable adapter with clear value.
- `load` hydrates initial state.
- `subscribe` registers external change notifications when the backend can push updates.
- `load_after_event` converts an external event into a snapshot.
- `reconcile` mutates store state from a snapshot and returns whether it really changed.
- `write_snapshot` persists local changed updates.
- `StoreSourcePolicy` describes synchronization semantics, not backend type.

Current `StoreSourceFuture<T, Error>` is a synchronous `Result<T, Error>` alias. Do not assume async source I/O without first updating the crate design and tests.

## GPUI Semantics

- Only call `cx.notify()` when the store or selected snapshot really changed.
- Keep store mutations inside GPUI contexts.
- Avoid render-time reads from unrelated entities. Use `StoreSelection` snapshots for subscribed render data.
- Keep subscriptions and source handles owned by the store handle or owner component so they are dropped with the correct lifecycle.

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
