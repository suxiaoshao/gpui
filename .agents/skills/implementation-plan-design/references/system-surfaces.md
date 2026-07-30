# Product and System Surfaces

Use this reference only to decide what the root hub must assess. Put semantics in the routed contract reference and concrete output in `plan-template.md`.

## Applicability Rules

Give every canonical row exactly one status:

- `Applicable`: the task changes, adds, deletes, migrates, or deliberately preserves behavior that needs an explicit target contract and work package.
- `No change`: the surface exists in the traced flow but the design intentionally leaves it unchanged; cite exact inspected evidence and the reason.
- `N/A`: the surface is absent from the traced flow; cite enough evidence to support that conclusion.

Copy every row into the root hub without merging or renaming IDs. Record each negative decision once. Owner plans reference assigned S-IDs and do not copy the matrix.

## Canonical Taxonomy

| ID | Surface | Apply when the task touches | Required target decision |
| --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Apps, crates, exports, shared code, manifests, moves/deletions, owner docs | Optimal owner, public/private boundary, dependency direction, exact paths and consumers |
| `S-02` | GPUI components, layout, interaction, and accessibility | Existing/custom components, dialogs, lists, loading/empty/error UI, keyboard, sizing, theme | Exact component composition, state/delegate/events, accessibility, layout, custom-gap evidence |
| `S-03` | Entity, Store, Global, identity, and projections | `Entity<T>`, `Store<S>`, `Global`, `EntityId`, `ElementId`, selectors, duplicated projections | One authority, identity/lifetime, readers/writers, publication, invalidation/reset |
| `S-04` | Actions, events, subscriptions, focus, and windows | Actions/keybindings, event emitters, observations, focus/IME, overlays, temporary/multiple windows | Registration/handler, subscription owner, focus/window lifecycle, reentrancy and cleanup |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Foreground/background tasks, streaming, long-lived resources, window/app shutdown | Task/resource owner, context/thread boundary, ordering, cancellation, partial completion, shutdown |
| `S-06` | Data acquisition and Operation state | DB/provider/HTTP/SDK/catalog reads, refresh/retry/repair, cache/freshness/offline/partial data | Exact source and types, Operation family or no-operation decision, transitions and phase-to-UI |
| `S-07` | Forms and editable state | Typed forms, bound controls, validation, dynamic options, save/rebase behavior | Value/control/catalog/save ownership, validation, submit and revision-safe rebase |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Public Rust APIs, adapters, protocols, tools/plugins, OS APIs, external services | Authoritative definition, producer/consumer, conversions, compatibility, rollout |
| `S-09` | Error identity, propagation, recovery, and error UI | New/changed failure, validation, mapping, retry/repair, notification/i18n/logging | Typed identity, producer mapping, boundary propagation, safe details, recovery/UI/logging/tests |
| `S-10` | Database, persistence, and migrations | Tables, schema, models, queries, transactions, indexes, rebuild/backfill/rollback | Final schema/query, atomicity, existing-data policy, consumers, generated schema and tests |
| `S-11` | Generated, synchronized, copied, or vendored content | Generators, schemas, registry/docs/skills copied upstream, submodules, vendored assets/code | Handwritten/canonical source, verified entrypoint, add/change/delete diff, drift/manual-edit policy |
| `S-12` | Icons and assets | Typed icons, app SVGs, runtime assets, bundle icons, asset generators | Exact enum/path, ownership, generation, fallback, runtime versus bundle placement |
| `S-13` | Fluent i18n and bundle localization | User-visible text, validation/error/accessibility copy, formatting, macOS bundle strings | Exact keys/files/variables, caller/UI state, fallback, locale parity and tests |
| `S-14` | Security, privacy, and credentials | Authentication, secret storage, provider/tool input, filesystem/database access, sensitive data | Trust boundary, validation, least exposure, lifecycle, redaction, failure behavior |
| `S-15` | Observability and diagnostics | Requests/tasks, dependency/storage failures, tracing/logging, user diagnostics | Span/event owner, fields, severity, correlation, redaction, cancellation/shutdown evidence |
| `S-16` | Packaging, platform behavior, and CI/release | xtask, bundles, manifests/entitlements, native dependencies, platform branches, workflows | Artifact owner, platform matrix, prerequisites, packaging/CI changes, rollback/release gate |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | Crate add/remove/update, features, pins, Git SHA, submodule, runtime/generator, lockfile | Evidence-backed target, compatibility class, migration, coupled artifacts, reuse and stop condition |
| `S-18` | Owner documentation, indexes, and ADRs | Stable architecture/ownership/workflow or long-lived decision changes | Exact README/index/ADR owner, links, status synchronization, facts retained in source |
| `S-19` | Validation and completion evidence | Every durable plan | Requirements-to-tests map, focused/aggregate/manual checks, implementation references, deviations |

## Expansion Routing

- Use `implementation-contracts.md` for owner-local portions of `S-01`, `S-05`, `S-10` through `S-16`.
- Use `gpui-application-contracts.md` for `S-02` through `S-07`.
- Use `integration-contracts.md` for `S-08`.
- Use `error-contracts.md` for `S-09`; other sections reference ERR-IDs instead of restating error semantics.
- Use `dependency-changes.md` and `upstream-reuse-audit.md` for `S-17`.
- Use `documentation-layout.md` for `S-18` and `plan-template.md` for `S-19`.

Do not add competing taxonomy rows to make a plan appear comprehensive. Use exact paths, symbols, configuration owners, focused-skill evidence, and stable IDs when expanding an applicable row.
