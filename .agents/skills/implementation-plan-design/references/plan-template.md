# Implementation-ready Plan Template

Instantiate the root-hub template once for every durable plan set at `docs/dev/<plan-id>/README.md`. Instantiate the owner-plan template once under every affected app/crate using the same `plan-id`. Delete instructional text and inapplicable conditional sections when instantiating documents, but always retain the complete applicability matrix in the root hub.

Do not begin implementation while the root hub remains `Draft`.

**Contents:** [Representation](#representation-rules) · [Stable IDs](#stable-id-families) · [Root hub](#root-hub-template) · [Target design](#target-design) · [Work packages](#work-packages) · [Validation](#validation) · [Completion](#completion-evidence) · [Handoff](#execution-handoff-audit) · [Owner plan](#owner-plan-template)

## Representation Rules

Choose the smallest representation that fixes the design:

- use annotated text trees for file/module ownership and other hierarchy;
- use language-tagged declarations for exact Rust types, traits, associated types, impl/method signatures, SQL/schema, HTTP/JSON, configuration, and protocol contracts;
- use labeled per-ID contract blocks for heterogeneous facts whose fields answer different questions, including ownership, lifecycle, data-source, and runtime decisions;
- use concise pseudocode for algorithms, migrations, projection/reset rules, retry/rollback, and state transitions that declarations cannot express;
- use numbered steps for simple linear flow;
- use Mermaid `flowchart`, `sequenceDiagram`, or `stateDiagram-v2` only for non-trivial topology/projection, multi-participant ordering, branching/error propagation, or recurring lifecycle;
- use tables for homogeneous mappings and comparisons such as applicability, evidence, compatibility, errors, dependencies, icons/i18n, tests, and validation;
- use short prose/lists for rationale, invariants, non-responsibilities, security, and exceptional behavior.

Keep each fact in one canonical representation and reference its stable ID elsewhere. Exact declaration blocks define target contracts, not complete method bodies. Do not force heterogeneous contracts into wide tables or add a diagram or table that merely restates a clearer declaration, tree, block, or sequence.

## Stable ID Families

Enable only families needed by the applicable surfaces. Once assigned, do not renumber an ID because another item is removed; mark the old item removed/superseded and preserve traceability.

| Family | Owns |
| --- | --- |
| `S-xx` | Canonical applicability surface from `system-surfaces.md` |
| `E-xx` | Current/upstream facts, user decisions, release-gated evidence |
| `D-xx` | Material target decisions |
| `F-xx` | Added/modified/moved/deleted/generated/synchronized/vendored paths |
| `L-xx` | Owner-local types, traits, methods, repositories, components, and operations |
| `C-xx` | Cross-owner/crate/provider/MCP/platform/external contracts |
| `ERR-xx` | Typed error identity and end-to-end recovery meaning |
| `DB-xx` | Schema objects, queries, transactions, and migration units |
| `G-xx` | Generated/synchronized/copied/submodule/vendored lineage |
| `ST-xx` | State authority, data flow, lifecycle, and projections |
| `R-xx` | Observable requirement or invariant |
| `T-xx` | Proposed automated/manual test or validation scenario |
| `WP-xx` | Ordered implementation work package |

Work packages reference IDs instead of pasting their definitions. Keep ID authority explicit:

- the root hub owns all S/C/ERR IDs plus shared/cross-owner E/D/R/T/WP IDs;
- each owner plan owns its owner-local E/D/F/L/DB/G/ST/R/T/WP IDs;
- C/ERR IDs remain root-owned; owner plans list them as consumed root IDs and define only their local producer, adapter, persistence, runtime, or UI implementation;
- a genuinely root-owned artifact may use the applicable local ID families directly in the root hub.

Assign non-overlapping ranges where a family is authored by several owner plans. Reference IDs instead of redefining their authority or contract elsewhere.

## Root Hub Template

# <Issue or outcome>: <Observable result>

## Status and Scope

- Status: `Draft`
- Tracking issue: `<link or None>`
- Plan ID: `<issue-N or kebab-case task slug>`
- Root hub: `docs/dev/<plan-id>/README.md`
- Root index: `docs/dev/README.md`
- Branch: `<branch or Not created>`
- Affected owners: `<exact app/crate/config/schema/packaging owners>`
- Release gates: `<named gate plus exact future verification or None>`
- Last evidence refresh: `<YYYY-MM-DD>`
- Implementation references: `Pending`

### High-impact Change Summary

Complete every audit gate. Write `None` when unchanged; otherwise keep the result to one sentence beginning with one or more of `[Add]`, `[Modify]`, `[Move]`, `[Delete]`, `[Cross-owner]`, `[Breaking]`, `[Destructive]`, `[Security-sensitive]`, or `[Release-gated]`. Name affected owners/consumers and reference canonical IDs instead of duplicating their definitions.

| Audit gate | Result | Canonical IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | `<None or tagged summary>` | `<D/F/C/WP IDs>` |
| Public or cross-owner contracts | `<None or tagged summary>` | `<D/L/C/WP IDs>` |
| Global/shared authority | `<None or tagged summary>` | `<D/L/ST/WP IDs>` |
| Persistence, data, configuration, or credentials | `<None or tagged summary>` | `<D/DB/C/ERR/WP IDs>` |
| Runtime, concurrency, performance, or shutdown | `<None or tagged summary>` | `<D/L/ST/ERR/WP IDs>` |
| Security, privacy, or external access | `<None or tagged summary>` | `<D/C/ERR/R/WP IDs>` |
| Dependencies, toolchains, generated, or vendored artifacts | `<None or tagged summary>` | `<D/F/G/WP IDs>` |
| Platform, packaging, CI, or release | `<None or tagged summary>` | `<D/F/C/R/WP IDs>` |
| User-visible compatibility, defaults, or removals | `<None or tagged summary>` | `<D/R/C/DB/WP IDs>` |

This summary is a discovery index, not a second contract registry. Before marking the plan `Ready`, fill every row and resolve the required user decision, compatibility/data policy, migration/rollback, or release gate for every material result.

### Goal

State the observable product or engineering outcome.

### Non-goals

List behavior, migrations, compatibility, cleanup, and follow-up work intentionally excluded.

### User Decisions

Record only decisions explicitly confirmed by the user. Do not persist unanswered questions, inferred preferences, recommendations, or assumed answers.

### Compatibility and Migration Policy

State compatibility for existing APIs, data, configuration, providers, tools, and packaged apps. Define rebuild, backfill, rollout, rollback, and intentional incompatibility policy. If the application is unpublished and the user permits rebuilding state, record the final clean design rather than unnecessary compatibility code.

### Plan Map

List the root hub and every affected app/crate owner plan. A root-only task may contain only the root row; any affected app/crate requires a same-ID owner-plan row.

| Scope | Document | Owns | Assigned IDs/WPs |
| --- | --- | --- | --- |
| Root hub | This document | Shared evidence, decisions, cross-owner contracts, sequencing, status, aggregate validation/completion | `<IDs>` |
| `<app-or-crate owner>` | [`<owner plan>`](path) | `<only this owner's exact local responsibility>` | `<IDs/WPs>` |

## Applicability

Copy all rows from `system-surfaces.md` without merging or renaming IDs. Fill each exactly once with `Applicable`, `No change`, or `N/A`. Negative decisions require exact current evidence.

| S-ID | Canonical surface | Status | Current evidence | Target decision or negative reason | Owning section/WP |
| --- | --- | --- | --- | --- | --- |

## Evidence

### Current Flow

Trace the exact current path from entrypoint through validation/policy, state/data acquisition, transformation/persistence, integration boundaries, GPUI owner/projection, UI/i18n, failures, tasks/cancellation, shutdown, and packaging consumers. Use numbered steps when linear and Mermaid only when relationships or ordering are non-trivial. Label steps/nodes with exact paths/symbols and stable IDs.

### Evidence Registry

| E-ID | Classification | Claim | Evidence | Plan consequence |
| --- | --- | --- | --- | --- |
| `E-01` | `Current fact` | `<verified claim>` | `<path:symbol/config/test/command>` | `<consequence>` |
| `E-02` | `Upstream fact` | `<verified claim>` | `<official release/tag/PR/commit/docs/source>` | `<consequence>` |
| `E-03` | `User decision` | `<confirmed decision>` | `<conversation>` | `<consequence>` |
| `E-04` | `Release-gated` | `<unavailable artifact>` | `<current evidence and future check>` | `<blocked scope only>` |

Do not present proposed names or unverified APIs as current facts. Cite stable architecture from executable sources and owner documentation instead of copying it into several plans.

### Conditional Evidence

- For `S-17`, insert the dependency inventory, upstream-change mapping, and coupled-artifact tables from `dependency-changes.md`.
- When upstream/repository capability may replace local code/content, insert the decision table from `upstream-reuse-audit.md`.

Keep each table in the root hub or its owning owner plan once and reference its row D/F/G/R/T IDs.

## Decisions

| D-ID | Decision | Evidence | Material rejected alternative | Consequence/owner |
| --- | --- | --- | --- | --- |

Before `Ready`, obtain user confirmation for every material choice not uniquely fixed by repository policy or authoritative APIs.

## Target Design

Keep only shared, root-owned, and cross-owner facts in the root hub. App/crate-local files, declarations, state, database design, GPUI behavior, icons/i18n, work-package steps, and focused validation belong in the corresponding owner plan.

### Root-owned Files and Workspace Topology

For root-owned manifests, configuration, CI, packaging, repository tooling, indexes, and documentation, show an annotated F-ID tree with action, artifact kind, responsibility, source of truth, and consumers. Do not inventory an app/crate's local files here; link its owner plan.

Use an annotated tree or Mermaid `flowchart` for non-trivial cross-owner dependency direction, crate/app additions or removals, public boundaries, generated inputs/consumers, and rollout topology. Reference owner-local F/L/DB/G/ST IDs without redefining them.

### Shared State and Data Flow

Describe only flows that cross owners or are genuinely root-owned. Define the authoritative owner, participating owner plans, conversions, publication, persistence/cache boundary, invalidation/reset, cancellation, partial behavior, and referenced ST/L/C/DB/ERR IDs. Keep each owner-internal projection in its owner plan.

### Integration Contracts

For `S-08`, insert the C-ID registry, exact native contract declarations, participant flow when needed, compatibility/rollout, and tests from `integration-contracts.md`. Reference ERR-IDs instead of duplicating error semantics.

### Error Contracts

For `S-09`, insert the ERR catalog, exact error/detail declarations, producer EM mappings, boundary EA mappings, GPUI recovery/i18n/diagnostic mapping, occurrence flow, compatibility, and tests from `error-contracts.md`.

### Shared Dependency, Persistence, Security, and Release Policy

Keep cross-owner dependency targets and release gates, compatibility/data-loss/rebuild/rollback policy, trust boundaries, redaction policy, aggregate diagnostics, platform/packaging order, and release/rollback sequencing here. Reference the authoritative dependency, C/ERR, and owner-local F/DB/G/ST/R/T IDs; keep exact manifest, schema, adapter, asset, and platform implementation in owner plans.

## Work Packages

In the root hub, keep the cross-owner WP dependency/sequence map and define shared or root-owned WPs in full. For an app/crate-owned WP, record only its ID, owner, observable outcome, dependencies, and owner-plan link here; keep its implementation steps and focused done condition in that owner plan.

Order WPs by dependency. Give each one owner and one observable outcome. Keep research and architecture decisions out of implementation steps. Reuse the following shape for full root-owned WPs and for assigned WPs inside owner plans.

### WP-<N>: <Outcome>

**Owner**

`<workspace/app/crate/config owner>`

**Prerequisites and contracts**

- `<prior WP, D/S/L/C/ERR/DB/G/ST/R ID, migration/release gate>`

**File IDs**

- `<F-IDs>`

**Implementation sequence**

1. `<source-of-truth edit and state transition>`
2. `<consumer, adapter, migration, generation/sync, packaging update>`
3. `<legacy path, workaround, compatibility, obsolete skill/doc deletion>`

**Failure and lifecycle behavior**

Reference shared IDs, then describe only WP-specific atomicity, partial progress, cancellation, retry/repair, rollback, reentrancy, window close, or shutdown.

**Tests**

| R-ID | T-ID/file | Proposed scenario | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |

**Focused validation**

| Command/manual scenario | Purpose | Required environment | Expected evidence |
| --- | --- | --- | --- |

**Done condition**

State observable result, expected source/generated/schema/dependency diff, removed paths, evidence, and stop conditions.

## Validation

Map each requirement to evidence. Use the union of applicable scopes and repository policy; focused checks precede aggregate checks.

| R-ID/requirement | Owner/WP | Automated/manual evidence | Expected result | External prerequisite |
| --- | --- | --- | --- | --- |

Discover exact commands from `AGENTS.md`, manifests, CLI help, owner docs, workflows, and source. Record unavailable network/provider/platform/packaged-app boundaries honestly. Compilation alone is not end-to-end validation.

## Completion Evidence

Keep pending until authorized implementation begins, then update continuously:

| Evidence | Actual result |
| --- | --- |
| Implementation PR and commits | `Pending` |
| Actual added, modified, moved, deleted, generated, synchronized, submodule, and vendored files | `Pending` |
| Delivered D/F/L/C/ERR/DB/G/ST/R/T/WP IDs | `Pending` |
| Automated commands and results | `Pending` |
| Manual, packaged-app, or real-API scenarios and environment | `Pending` |
| Schema/migration/dependency/generated/vendored diff | `Pending` |
| Owner README, index, and ADR updates | `Pending` |
| Accepted deviations and approving decision | `None / Pending` |
| Unverified boundaries and reason | `None / Pending` |

Set `Done` only when every required item is complete or accurately recorded as an accepted scoped limitation.

## Execution Handoff Audit

- [ ] The root hub exists at `docs/dev/<plan-id>/README.md`; every affected app/crate has a same-ID owner plan, and all documents link bidirectionally.
- [ ] The root hub owns status, applicability, shared decisions, cross-owner sequencing, and aggregate completion; owner plans do not duplicate them or define sibling implementation.
- [ ] Root-owned consumed IDs and owner-authored local ID ranges are explicit; C/ERR meanings are not redefined in owner plans.
- [ ] Every S-row has `Applicable`, `No change`, or `N/A` with evidence.
- [ ] Every material choice has a D-ID and required user confirmation.
- [ ] Every affected path has an F-ID, action, artifact kind, owner, source-of-truth role, consumer, and deletion/generated lineage where relevant.
- [ ] Every L/C/ERR/DB contract contains exact native declarations and methods, not only fields or prose.
- [ ] GPUI plans fix Entity/Store/Global/Form/Operation/window/task/subscription ownership, identities, methods, transitions, phase-to-UI, focus, i18n, and tests.
- [ ] Every mutable value has one authority; projections have synchronization/reset rules.
- [ ] Every C-ID names authoritative definition, producer, consumers, compatibility, rollout, and tests.
- [ ] Every ERR-ID connects producer, boundary, persistence/partial effects, runtime/Operation recovery, GPUI/i18n, diagnostics, and tests.
- [ ] Database atomicity/rebuild/rollback, generated lineage, dependency evidence, coupled artifacts, and upstream reuse are complete when applicable.
- [ ] Every R-ID maps to T-ID/validation evidence.
- [ ] Broad research tasks, duplicated definitions, vague verbs, speculative APIs, and compatibility layers without exit conditions are removed.
- [ ] Every release gate has an exact future verification procedure and blocks only dependent WPs.
- [ ] Every high-impact audit gate is `None` or references its canonical IDs; no material change appears only inside a target-design section, owner plan, or work package.
- [ ] Every `[Breaking]`, `[Destructive]`, `[Security-sensitive]`, `[Cross-owner]`, or `[Release-gated]` result names affected consumers and the required decision, migration/rollback, data policy, or gate owner.
- [ ] Implementation requires no invented architecture, GPUI primitive, contract, migration, or acceptance criterion.

## Owner Plan Template

Instantiate once for every affected app/crate at its same-ID path. Do not add status, full applicability, shared decisions/contracts, aggregate validation, or completion evidence.

# <Owner>: <Owner-local outcome>

## Root Hub and Ownership

- Plan ID: `<same plan-id as root hub>`
- Root hub: `<repository-relative link>`
- Owner directory: `<exact app/crate directory>`
- Owner plan: `<app/<name>/docs/dev/<plan-id>/README.md or crates/<name>/docs/dev/<plan-id>/README.md>`
- Owner index: `<app/<name>/docs/dev/README.md or crates/<name>/docs/dev/README.md>`
- Root-owned IDs consumed: `<S/C/ERR and shared E/D/R/T/WP IDs>`
- Owner-authored local IDs/ranges: `<E/D/F/L/DB/G/ST/R/T/WP IDs>`
- Assigned WPs: `<WP IDs>`
- Owns: `<exact owner-local responsibility>`
- Does not own: `<root-shared or sibling responsibility retained elsewhere>`

## Owner-local Evidence

Record only new E-ID evidence needed for this owner's design. Reference root-hub E/D/S IDs for shared facts and decisions; do not copy the root evidence registry.

## Owner-local Decisions

Record only D-IDs whose consequences stay inside this owner. Move a product choice, cross-owner architecture decision, compatibility/data policy, or release gate to the root hub and obtain required user confirmation there.

## Owner-local Target Design

Instantiate only applicable subsections below. Reference root-owned C/ERR/dependency contracts instead of copying them. Do not describe a sibling app/crate's implementation.

### File and Ownership Tree

Show this owner's complete affected paths once:

```text
<owner root>/
├── <path>  # F-01 [Modify, handwritten] <responsibility/source-of-truth>
├── <path>  # F-02 [Add, generated from G-01] <consumer>
├── <old> -> <new>  # F-03 [Move] <ownership change>
└── <path>  # F-04 [Delete] <removed responsibility/consumer>
```

Include source, consumers, tests, manifests, migrations, snapshots, generated/synchronized/vendored files, submodules, assets, documentation, and deletions. Record non-responsibilities and dependency direction below the tree when needed.

### Owner-local Contracts

Apply `implementation-contracts.md`. For every L-ID include:

- F-ID and exact path/symbol;
- visibility, owner, callers, and consumers;
- referenced root-owned C/ERR IDs and local DB/ST/R/T IDs;
- language-tagged target declarations for structs/enums/traits/associated types/impls/methods/components/repositories;
- invariants, validation, conversion, side effects, persistence, lifecycle, concurrency, and failure behavior not expressible in declarations.

Do not replace a known declaration with prose or write full ordinary method bodies.

### Boundary Implementations

For every consumed root C/ERR ID, define only this owner's producer, consumer, adapter, persistence, runtime, recovery, UI, or diagnostic implementation through local F/L/DB/ST/R/T IDs. Do not redefine the shared contract or error meaning.

### GPUI Application Contracts

Apply `gpui-application-contracts.md` for assigned `S-02` through `S-07`.

#### State and Ownership Contracts

Write one labeled block per mutable fact:

```markdown
##### ST-<N>: <Mutable fact>

- **Authority:** `<exact type and owner: local field, Entity<T>, Store<S>, Global, form, native control, Operation, page/controller, Window, database, or provider>`
- **Initialization and lifetime:** `<creator, initialization point, destruction/drop condition, strong/weak handles when relevant>`
- **Readers:** `<exact L-IDs/types/methods>`
- **Mutation:** `<sole writers and exact mutation entrypoints>`
- **Publication and projections:** `<notify/observe/subscribe path, selectors, derived state, invalidation>`
- **Persistence boundary:** `<DB/C-ID and write/read path, or None with reason>`
- **Reset and cancellation:** `<trigger, task/subscription behavior, resulting state>`
```

Use a Mermaid `flowchart` only when one authority feeds several readers/projections or crosses owners. Reference root IDs at the boundary; do not duplicate their contracts.

#### Interaction and Runtime Flows

Use numbered steps for a simple trigger-to-result path. Use a Mermaid `sequenceDiagram` only when ordering, branching, cancellation, or an async boundary makes the flow non-trivial. Name the R-ID and trigger, exact action/event and handler L-ID, context/task owner, ST transitions, UI/focus/notification/Fluent result, ERR-IDs, and T-IDs.

#### Data-source and Operation Contracts

Write one labeled block per changed resource:

```markdown
##### ST-<N>: <Resource>

**Data source**

- **Source and boundary:** `<repository/API/C-ID/DB-ID>`
- **Inputs and authentication:** `<exact input types, credentials, validation/policy>`
- **Acquisition semantics:** `<pagination, streaming, cache, freshness, deduplication>`
- **Data and failures:** `<exact Data L-ID and ERR-IDs>`
- **Publication destination:** `<authoritative owner and projection path>`

**Runtime decision**

- **Repeatable read/retry semantics:** `<whether retry repeats the same read>`
- **Caller-selected repair:** `<Repair value and selection path, or None>`
- **Selected model:** `<refresh::Operation, repair::Operation, Task/controller state, verified domain state machine, or No operation>`
- **Exact runtime types:** `<Data/Problem/Repair/Task L- and ERR-IDs>`
- **Owner and task retention:** `<exact type/field>`
- **Reason:** `<why this model matches the verified lifecycle>`
- **Entrypoints and transitions:** `<load/refresh/retry/repair/cancel/completion>`
- **Retry, repair, and cancellation policy:** `<observable behavior, stale-data policy, duplicate-trigger behavior>`
- **Phase-to-UI contract:** `<lifecycle/phase subsection below>`
- **Tests:** `<R/T IDs>`
```

Show a linear acquisition path with numbered steps. Use a Mermaid `flowchart` only when it branches through caches, fallbacks, projections, or several owners.

#### Operation Lifecycle and Phase-to-UI

Use Mermaid `stateDiagram-v2` when legal transitions, repeated triggers, repair, cancellation, or stale/degraded states are non-trivial. Keep UI projection as a narrow homogeneous mapping:

| Phase | UI projection: visible data and exact component/message | Actions and control state | Fluent key/variables | R/T IDs |
| --- | --- | --- | --- | --- |

Distinguish `Ready(empty)`, initial loading, refreshing settled data, unavailable, and degraded/stale usable data. Add applicable form/control, focus, task/subscription, window, dialog, overlay, and accessibility detail as L/ST contracts.

### State and Data Flow

Use ST-IDs for owner-local authorities and projections. Define writers/readers, conversion, persistence/cache, publication, invalidation/reset, stale behavior, cancellation, and the reason for every duplicated projection.

### Database and Migration Design

For each DB-ID, provide exact target SQL, Diesel schema/model, repository query/command signatures, constraints/indexes, transaction/atomicity, existing-data policy, backfill/rebuild/rollback, generated schema, consumers, and R/T IDs.

### Generated and Synchronized Lineage

Give every chain a G-ID. Use an annotated chain when linear or a flowchart when one source branches:

```text
F-<source> handwritten/canonical source
  -> F-<snapshot> optional maintained input
  -> F-<output> generated/synchronized/copied/submodule/vendored output
  -> F-<consumers>
```

Record verified entrypoint/provenance, manual-edit policy, expected additions/changes/renames/deletions, and drift checks.

### Icons and Assets

| UI role | Exact typed icon/Lucide slug/SVG path | Owner and F/G IDs | Runtime/bundle placement | Fallback | R/T IDs |
| --- | --- | --- | --- | --- | --- |

### Fluent i18n and Bundle Localization

| Key | Locale files | Meaning | Variables/plural/select | Caller/UI state | Fallback | R/T IDs |
| --- | --- | --- | --- | --- | --- | --- |

Keep runtime Fluent and bundle localization ownership distinct.

### Security, Observability, Packaging, and Platform

Define this owner's exact credential/config owners, trust validation, secret lifecycle/redaction, span/event fields, severity/correlation, runtime/bundle assets, xtask/packaging changes, platform branches, validation, and rollback. Reference shared policy and root release gates instead of copying them.

## Owner-local Work Packages

Use the root-hub WP shape for assigned packages only. Reference local F/L/DB/G/ST/R/T and root-hub shared IDs.

## Focused Validation and Handoff

Map every owner-local R-ID to T-ID/validation evidence, with exact command or manual scenario, required environment, and expected result. Record deviations that require a root-hub update; keep aggregate status/completion in the root hub.
