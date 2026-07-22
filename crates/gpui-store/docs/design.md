# gpui-store design

[English](design.md) | [简体中文](design.zh-CN.md)

## 1. Role

`gpui-store` owns typed committed state inside a GPUI application. It standardizes snapshot reads, meaningful change notification, observation, projection, and optional backend reconciliation while leaving domain commands and persistence policy to the application.

It is not a form draft store, database repository, or generic two-way binding framework.

## 2. Store kinds

### `LocalStore<S, B>`

A local store is directly owned by one component or controller. It is appropriate when state and lifetime belong to that owner and application-wide access is unnecessary.

### `SharedStore<S, B>`

A shared store is a GPUI entity used for application-wide committed state or catalogs. Components observe it through GPUI subscriptions and read owned snapshots.

Both store kinds expose the same state semantics; they differ in ownership and observation lifetime, not in domain meaning.

## 3. Snapshot semantics

A store read produces an owned `S` snapshot (or another explicitly owned projection). Callers do not retain internal references across GPUI updates. This makes state boundaries visible and prevents long-lived borrows from leaking out of the store.

A snapshot is a point-in-time value. Consumers that need consistency across validation, resolution, or command creation capture it once and use that same value throughout the operation.

## 4. Mutation and change detection

Store writes are explicit commands such as replace, set, or update. Notification occurs only when the resulting committed state is meaningfully different according to the store's change policy.

Failed persistence or backend refresh does not replace the last valid committed snapshot. A successful external load/reconcile installs one new snapshot atomically.

## 5. Selections and bindings

`StoreSelection<T>` is a one-way read projection derived from store state. It may cache a row, label, or capability for rendering, but it has no business setter and is never a second source of truth for submit.

`StoreBinding<T>` is narrower: it is valid only when the presented field intentionally writes back to the same backing store. It must not be used to mirror component-local edit state or a form snapshot.

Derived component options are presentation configuration. Updating them does not update the form-owned current selection.

## 6. Backend boundary

Backends provide reusable load, save, subscribe, or reconcile mechanics. They do not decide domain commands, validation, fallback selection, or UI behavior.

`MemoryBackend` is sufficient when an application repository/service already owns persistence. A custom backend is justified only when its external lifecycle is genuinely reusable across store users.

The in-memory store remains the committed application source of truth regardless of backend choice.

## 7. Catalogs

Provider, model, project, and capability catalogs are typical `SharedStore` values. A repository refresh queries external storage and atomically replaces the catalog only after success.

Consumers derive component options from one catalog snapshot. If an existing component selection is no longer available, the application reports or validates that mismatch explicitly. The catalog never silently selects a fallback.

## 8. Form integration

Store and form ownership are independent:

| Concern | Owner |
| --- | --- |
| Committed domain state/catalog | `gpui-store` store |
| Current editable typed value | generated `gpui-form` store |
| Validation baseline/report/submit runtime | generated `gpui-form` store |
| Focus, popup state, and blur history | bound component instance |
| Persistence command | application service/repository/store command |

The application coordinates explicit transitions:

```text
load committed state
  -> form.rebase(committed value)
  -> bound components reproject from the form

user submit
  -> form.prepare_submit()
  -> validate/transform the same form-owned model
  -> execute application command
  -> reconcile committed store
  -> form.rebase(saved value)
```

There is no implicit form-to-store or store-to-form synchronization.

## 9. GPUI lifecycle

Store observers read and compute during the source callback. Cross-entity changes are sent as explicit owner commands or deferred tasks; observers do not recursively update the same source entity while it is already being updated.

Subscriptions are retained by the entity that owns the observation. Dropping that owner ends the observation lifetime.

## 10. Public invariants

- A store represents committed application state, not transient component editing.
- Reads produce owned point-in-time snapshots.
- Failed external work preserves the last valid snapshot.
- Change notification reflects meaningful committed changes.
- A selection is a read projection, not a second source of truth.
- Catalog refresh does not silently change component selection.
- Backend mechanics do not absorb domain or form policy.
- Store/form/component synchronization is always an explicit application command.

## 11. Non-goals

`gpui-store` does not provide database schema mapping, application repositories, form validation, component focus, error presentation, undo history, or automatic conflict resolution between arbitrary state owners.
