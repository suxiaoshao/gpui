---
name: gpui-store
description: Implement or review crates/gpui-store and application code deliberately adopting Store. Ordinary GPUI entity state alone does not trigger this skill.
---

# GPUI Store

Use for `crates/gpui-store` and application code deliberately adopting `Store<S>`. Ordinary Entity state does not trigger a migration to Store.

## Essential boundaries

- Store owns one authoritative in-memory state and publication. It does not own I/O, persistence, tasks, retry, transactions, or rollback.
- Selectors are pure; selections are read-only projections, not second authorities. Retain subscriptions in their owner and avoid synchronous mutation from an active Store observation borrow.
- Applications own persistence and operation policy; Form owns editable sessions. Catalog updates do not silently rewrite Form values.
- Current public docs/exported code define the implemented contract. Keep public English/Chinese README/guide changes aligned.

## Source routing

For API/implementation changes, locate the affected contract in `crates/gpui-store/README.md`, `docs/guide.md`, or `src/lib.rs`, then inspect the corresponding implementation and tests. Read `crates/gpui-store/dev/in-memory-store.md` when changing architecture or executing that plan.

Before application integration, trace the affected state flow, consumers, and ownership lifetime. Do not migrate unrelated app state.

| Task | Contract section |
| --- | --- |
| State ownership and trait constraints | [Core model](references/contracts.md#core-model) |
| Construction, globals, mutation/publication | [Mutation](references/contracts.md#construction-and-mutation) |
| Selection, observation, subscription lifetime | [Observation](references/contracts.md#selection-and-observation) |
| Persistence, forms, catalogs, application state | [Application boundaries](references/contracts.md#application-boundaries) |
| Migration or proposed new abstraction | [Removed concepts](references/contracts.md#removed-concepts) |

## Integration coverage

Changes to publication or observation cover affected consumer contracts and subscription lifetimes.
