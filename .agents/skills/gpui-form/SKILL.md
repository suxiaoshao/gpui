---
name: gpui-form
description: Implement typed forms, control bindings, validation and submission using the gpui-form crates.
---

# GPUI Form

Start with the [README](../../../crates/gpui-form/README.md) or [guide](../../../crates/gpui-form/docs/guide.md) and affected implementation. Macros use the [macro guide](../../../crates/gpui-form-macros/docs/guide.md); component adapters use the [binding guide](../../../crates/gpui-form-gpui-component/docs/guide.md).

One `Entity<Form<M>>` owns an editing session. Native controls own interaction state; applications own catalogs, persistence, operations and product policy. A component cache is not the submission source. Catalog changes do not silently replace draft values.

Use existing bindings and typed paths. Apply async save results with the prepared version so a stale result cannot replace newer edits. Current exported APIs and public guides define implemented behavior.

Read [internal invariants](references/contracts.md) only for core, custom binding or lifecycle changes. Keep changed public English/Chinese docs aligned; record new development designs in Chinese.
