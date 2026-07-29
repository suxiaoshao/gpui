---
name: implementation-plan-design
description: Design, review, or rewrite implementation-ready development plans grounded in the current repository and upstream APIs. Use for complex features, dependency or framework upgrades, architecture changes, database work, UI work, issue plans, and durable docs that another engineer or weaker coding model must execute without making unresolved product or architecture decisions.
---

# Implementation Plan Design

Produce an implementation specification, not a generic roadmap. Finish repository and upstream research before persisting decisions.

## Workflow

1. Read repository instructions, existing plan docs, relevant skills, source modules, manifests, schema, tests, and current Git state.
2. Trace the current implementation end to end: entrypoint, state owner, data acquisition, transformation, persistence, UI projection, errors, cancellation, and tests.
3. Research every changing direct dependency and high-risk transitive dependency. Read release notes, changelog, migration guide, compare range, relevant PRs, public API source, features, and MSRV. Use [dependency-evidence.md](references/dependency-evidence.md).
4. Compare new upstream capabilities with local custom code. Decide what to reuse, replace, delete, adapt, or retain using [upstream-reuse-audit.md](references/upstream-reuse-audit.md).
5. Resolve product and architecture decisions before writing the plan. If a material choice cannot be discovered from code or authoritative sources, stop and ask the user in the conversation. Do not persist the question, candidate options, or an assumed answer. Resume after the user decides and persist only the decision.
6. Specify modules, types, traits, methods, ownership, concurrency, errors, and lifecycle with [api-contracts.md](references/api-contracts.md).
7. Specify every applicable product and system surface with [system-surfaces.md](references/system-surfaces.md). Explicitly record `None` when a surface does not change.
8. Write the durable plan as ordered work packages using [plan-template.md](references/plan-template.md).
9. Re-read the plan as an implementer. Remove research tasks that should already have been completed, ambiguous verbs, speculative APIs presented as facts, duplicated sources of truth, and decisions deferred to implementation.

## Non-negotiable Output

Determine and persist:

- exact files and module structure, including added, modified, moved, and deleted files;
- existing components to use and any custom component required;
- concrete structs, enums, fields, trait implementations, associated types, method signatures, visibility, derives, and invariants;
- control flow and data flow, including success, empty, error, cancellation, retry, partial-output, and shutdown paths;
- local, entity, shared, and global state ownership and mutation paths;
- database tables, columns, indexes, constraints, queries, transactions, rebuild or migration policy, and schema tests;
- data acquisition APIs, source of truth, caching, freshness, pagination, authentication, and offline behavior;
- icons by exact enum/asset name and ownership location;
- i18n keys, locale files, interpolation variables, and fallback behavior;
- dependencies and features to add, remove, or change, with exact versions or release gates;
- upstream release evidence, breaking changes, affected call sites, migration action, and local code that becomes unnecessary;
- test files, proposed test names, fixtures/mocks, assertions, validation commands, and completion evidence.

Do not omit a surface because no change is expected. Write an explicit no-change decision and its evidence.

## Evidence Rules

- Distinguish `Current fact`, `Upstream fact`, `Decision`, `Release-gated`, and `User decision`.
- Cite local files and upstream release/tag/PR/commit or official documentation next to the decision they support.
- For an unreleased target, fully document the known current-to-latest migration now, then add a separate release-gated delta. Do not defer already-known breaking changes.
- Do not use method names, trait bounds, fields, components, or dependency features that have not been verified in source or official API documentation.
- Do not require the implementer to repeat broad upstream research. Leave only a narrow release-gate verification when the final artifact does not yet exist.

## Ambiguity Gate

Stop and ask the user when alternatives materially change product behavior, public API, schema, ownership, security, dependency choice, compatibility policy, or long-term maintenance. Present the evidence, the concrete alternatives, and a recommendation in the conversation.

Do not stop for choices that are directly determined by repository conventions, authoritative APIs, or an already recorded user decision.

## Completion Gate

A plan is ready only when an implementer can execute each work package without choosing architecture, inventing API contracts, rediscovering dependency migrations, or guessing acceptance criteria. If a weaker model could reasonably produce two incompatible implementations, the plan is incomplete.
