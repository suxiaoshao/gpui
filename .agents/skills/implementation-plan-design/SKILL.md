---
name: implementation-plan-design
description: Research, design, review, rewrite, and maintain implementation-ready development plans grounded in the current GPUI workspace and authoritative upstream APIs. Use for non-trivial features, fixes, refactors, dependency or framework upgrades, GPUI application work, cross-crate/provider/MCP contracts, error behavior, database changes, generated or synchronized artifacts, packaging, and coordinated work that needs a durable executable specification.
---

# Implementation Plan Design

Produce a durable implementation specification before production changes. Make it executable without rediscovering the repository, inventing contracts, or choosing unresolved product or architecture decisions.

## Workflow

1. Read repository instructions, nearest owner README files, manifests, entrypoints, relevant source and tests, schema/migrations, generators, existing plans, and current Git status.
2. Require a durable plan for a non-trivial change to behavior, ownership, public APIs, persistence, security, dependencies, generated artifacts, packaging, or several coordinated files. Keep a truly local behavior-preserving correction lightweight.
3. Create the durable plan set with [documentation-layout.md](references/documentation-layout.md): one root hub at `docs/dev/<plan-id>/README.md` and one same-ID owner plan under every affected app/crate. Keep the root and owner indexes synchronized.
4. Trace the current implementation end to end, then classify every canonical surface with [system-surfaces.md](references/system-surfaces.md).
5. Load every conditional reference whose trigger applies. Read it completely and keep its facts in its sole plan section rather than redefining them elsewhere.
6. Research current and upstream facts before persisting target design. Verify exact source paths, APIs, components, traits, methods, versions, features, configuration, schema behavior, and generation/synchronization entrypoints.
7. Surface material product and architecture choices before continuing. Ask the user in the conversation when alternatives affect behavior, public API, schema, ownership, security, compatibility, dependency policy, or long-term maintenance. Do not persist unresolved questions, recommendations, or assumed answers.
8. Instantiate the root-hub and owner-plan structures in [plan-template.md](references/plan-template.md). Name exact files, symbols, stable IDs, work packages, tests, validation evidence, deletions, and completion conditions.
9. Re-read the plan as an implementer. Remove vague verbs, duplicate facts, speculative APIs, broad research tasks, and decisions deferred to implementation.
10. If implementation is authorized, begin only after the root hub is `Ready`. Keep the plan set synchronized with material discoveries and record actual commits/PRs, diffs, validation, deviations, owner-document updates, unverified boundaries, and final `Done` evidence.

## Conditional References

Read selected references completely before using them.

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

## Rules

- Prefer cohesive, testable ownership and dependency direction over the fewest changed files.
- Keep each fact in one canonical artifact and representation. Reference stable IDs elsewhere instead of copying definitions or progress.
- Use annotated trees for hierarchy, language-tagged declarations for exact contracts, labeled per-ID blocks for heterogeneous ownership/lifecycle/runtime facts, pseudocode for behavioral rules, numbered steps for simple flow, Mermaid only for non-trivial topology/sequence/state, tables for homogeneous mappings, and prose for rationale or security.
- Separate `Current fact`, `Upstream fact`, `Decision`, `User decision`, `Release-gated`, and implementation evidence.
- Verify proposed upstream/local names and behavior before marking `Ready`. Do not leave broad upstream research to the implementer.
- Change handwritten sources first, use verified repository generation/synchronization entrypoints, and inspect derived additions, changes, and deletions.
- Create issues, branches, commits, pushes, or pull requests only when explicitly authorized.

A plan is `Ready` only when an implementer can execute every work package without choosing architecture, selecting an unspecified GPUI/runtime primitive, inventing a contract, rediscovering a dependency migration, or guessing acceptance criteria.
