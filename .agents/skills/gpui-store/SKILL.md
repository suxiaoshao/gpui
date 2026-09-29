---
name: gpui-store
description: Implement or modify gpui-store and its deliberate application integrations.
---

# GPUI Store

Use the [README](../../../crates/gpui-store/README.md) and [guide](../../../crates/gpui-store/docs/guide.md) for current APIs, then inspect the affected code and consumers.

- Store owns authoritative in-memory state and publication. Applications own I/O, tasks, persistence and recovery; Form owns editable sessions.
- Selectors are pure read-only projections. Retain subscriptions in their owner. A whole-store observer holds a borrow; defer mutations that would re-enter that Store.
- `update_if` does not undo changes: returning `Unchanged` promises no observable mutation occurred.
- Catalog changes do not silently rewrite Form values. Keep persistence and optimistic-update policy in the application.

API changes cover affected publication/observation behavior and subscription lifetimes. Keep changed public English/Chinese docs aligned.
