---
name: gpui-form
description: Implement or review gpui-form crates and their explicit application integrations; use for typed forms, bindings, validation, or Form API migration.
---

# GPUI Form

Use for the Form crates and their deliberate application consumers.

## Essential boundaries

- One `Entity<Form<M>>` owns the editable session. Native controls own interaction state; applications own catalogs, persistence, operations, and product policy.
- Descriptors and typed paths carry schema/location data without retaining Form or native entities. Preserve total versus dynamic path semantics and runtime-owned occurrence identity.
- Keep stateful projection routing in the binding, async validation tied to its snapshot/lifetime, and saves guarded by the prepared version. Do not duplicate owners or revive removed APIs.
- Preserve historical documents. Public README/guide edits retain English/Chinese semantic parity; new development documents remain Chinese.

## Source routing

Start with the relevant section of `crates/gpui-form/README.md` or `crates/gpui-form/docs/guide.md` and the affected implementation/tests. Macro changes use `crates/gpui-form-macros/docs/guide.md`; component bindings use `crates/gpui-form-gpui-component/docs/guide.md`.

For explicit Issue #199 migration/design work, consult `crates/gpui-form/docs/dev/issue-199/design-draft.md` and the affected consumer plan at `app/{jaco,feiwen}/docs/dev/issue-199/form-breaking-api-remigration-plan.md`. Its confirmed architecture guides that migration; current exported code/public documentation establish what is implemented. An unrelated task does not reopen the migration.

| Task | Contract section |
| --- | --- |
| Session and control ownership | [Ownership](references/contracts.md#ownership) |
| Schema, total/dynamic paths, identity, topology | [Typed paths](references/contracts.md#public-schema-and-typed-paths), [identity](references/contracts.md#runtime-owned-identity-and-topology) |
| Change impact and notifications | [Mutation/events](references/contracts.md#mutation-and-events) |
| Native stateful controls and adapters | [Control bindings](references/contracts.md#stateful-control-binding) |
| Async validation, prepare, saved-model rebase | [Validation/submission](references/contracts.md#validation-and-submission) |
| Internal mutation transaction order | [Private transitions](references/contracts.md#private-transitions-and-transaction-order) |
| Jaco/Feiwen integration or API migration | [Application integration](references/contracts.md#application-integration), [removed surfaces](references/contracts.md#removed-surfaces) |

## Documentation and validation

Public docs explain construction, controls, validation, submission, nested paths, and custom adapters. Internal identity and transition mechanics belong in development docs.

API changes require affected crate and consumer contract coverage; UI behavior changes may also need runtime evidence. Documentation changes preserve relevant English/Chinese parity and terminology.
