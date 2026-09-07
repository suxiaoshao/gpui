---
name: implementation-plan-design
description: Create or revise implementation-ready plans for substantial gpui changes requiring durable coordination; review existing plans read-only when requested. Excludes routine local fixes and ordinary code review.
---

# Implementation Plan Design

Substantial changes need a durable specification of ownership, exact contracts, work packages and completion evidence. Routine local fixes without material contract, ownership or dependency changes do not need a plan. A plan-only request delivers the plan; a review delivers findings.

## Plan contract

- Reuse the plan that already owns the change. Revisions affect changed work packages, decisions, dependencies and indexes while preserving settled structure and evidence.
- Start with one plan at the nearest owner: app/crate for single-owner work, workspace root for cross-owner work. Split child plans only for independent design, sequencing or validation needs; follow the layout reference. Implementation starts when the main plan is `Ready`.
- Identify affected surfaces with the applicability reference. Specify affected files, symbols, stable IDs, interfaces, lifecycle, dependencies, verification evidence, deletions and acceptance criteria; keep each fact in one canonical location.
- Separate current facts, verified upstream facts, design decisions, user decisions, release-gated assumptions and implementation evidence. Verify proposed API names, versions, feature flags, configuration and generation/migration entrypoints before marking `Ready`.
- `Ready` means work packages can be implemented without inventing missing contracts or resolving material product/architecture choices. Material discoveries update the plan; completion records actual validation, deviations, implementation references, owner-document updates, unverified boundaries and `Done` evidence.

## Reference routing

| Trigger | Reference | Sole responsibility |
| --- | --- | --- |
| Creating, moving, splitting, completing, blocking, or superseding a durable plan | [documentation-layout.md](references/documentation-layout.md) | Artifact ownership, root-hub/owner-plan topology, plan IDs, indexes, lifecycle, ADR boundary |
| Assessing plan scope | [system-surfaces.md](references/system-surfaces.md) | Canonical `S-xx` applicability taxonomy only |
| Designing owner-local implementation | [implementation-contracts.md](references/implementation-contracts.md) | Files/modules, Rust types/traits/methods, persistence owner, lifecycle, lineage, icons/i18n, security/diagnostics |
| Designing a GPUI app or shared app-support runtime/UI | [gpui-application-contracts.md](references/gpui-application-contracts.md) | Entity/Store/Global, identity, components, actions/events/focus/window, tasks, Operation, Form, GPUI tests |
| Changing a cross-crate, app/agent, Rig/provider, MCP, platform, database-service, or external API boundary | [integration-contracts.md](references/integration-contracts.md) | Boundary authority, exact contract, producer/consumer, compatibility, rollout |
| Adding or changing failures, recovery, error UI/i18n, or diagnostic propagation | [error-contracts.md](references/error-contracts.md) | Typed error identity and end-to-end producer-to-UI/recovery/logging behavior |
| Changing dependencies, Git sources, submodules, toolchains, generators, manifests, or lockfiles | [dependency-changes.md](references/dependency-changes.md) | Baseline, release evidence, compatibility, migration, coupled artifacts, stop conditions |
| Evaluating whether upstream can replace local code or copied content | [upstream-reuse-audit.md](references/upstream-reuse-audit.md) | Reuse/adapt/retain/defer and deletion-first decisions |
| Writing, reviewing, handing off, or completing a plan | [plan-template.md](references/plan-template.md) | Representation rules, stable IDs, root-hub/owner-plan skeletons, work packages, validation, completion evidence |

Representation and handoff consistency belong to `plan-template.md`; location, indexes and lifecycle belong to `documentation-layout.md`.
