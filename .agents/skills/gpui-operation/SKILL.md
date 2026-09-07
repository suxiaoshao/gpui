---
name: gpui-operation
description: Implement or review crates/gpui-operation and deliberate integrations. Ordinary GPUI async work alone does not trigger this skill.
---

# GPUI Operation

Use for `crates/gpui-operation` and deliberate application integrations. Ordinary GPUI asynchronous code does not require adopting this crate.

## Essential boundaries

- The crate owns state transitions, payload movement, cancellation/restoration semantics. The application owns task construction, completion routing, notifications, and product policy.
- Store one family-provided runtime in the chosen owner. Its running variant owns the lifecycle-critical task; do not mirror phase/data/task state or detach that task.
- Use `refresh` for repeating the same read and `repair` for caller-selected recovery. Cancellation cannot roll back already-performed external side effects.
- Current exported code and public documentation define the implemented contract. Keep public English/Chinese README/guide changes aligned.

## Source routing

For API/implementation work, locate the relevant contract in `crates/gpui-operation/README.md`, `docs/guide.md`, or `src/lib.rs`, then inspect the affected family and tests. Read `tests/gpui_task.rs` for GPUI task ownership/cancellation; architecture or plan work may require `dev/message-driven-transitions.md`. Paths after the crate name are relative to that crate.

For application work, inspect the affected owner, consumers, and runtime flow. Consult `gpui` for framework APIs, or `gpui-store` only when Store integration is part of the task.

| Task | Contract section |
| --- | --- |
| Family and recovery selection | [Families](references/contracts.md#choose-the-family) |
| Runtime enum, messages, named states | [Runtime APIs](references/contracts.md#runtime-and-named-state-apis) |
| Ready-state business changes | [Business messages](references/contracts.md#ready-business-messages) |
| Task lifetime, ignored messages, cancellation, drop order | [Task contract](references/contracts.md#task-ignored-messages-and-cancellation) |
| Entity, Global, Store completion routing | [Owner integration](references/contracts.md#owner-integration) |
| Phase-to-UI behavior | [Product policy](references/contracts.md#product-and-ui-policy) |
| Migration or proposed new abstraction | [Non-goals](references/contracts.md#non-goals-and-removed-designs) |

## Integration coverage

Changes to operation behavior cover affected consumer phase-to-UI paths.
