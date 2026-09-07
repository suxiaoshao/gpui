# Implementation-ready Plan Template

Use the [layout reference](documentation-layout.md) for placement, ownership, indexes and lifecycle. Use the main-plan skeleton for either a single owner or a root hub; use the child-owner skeleton only when splitting helps. Remove instructional text and inapplicable sections. Shared facts have one canonical owner.

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

ID ownership and non-overlapping owner ranges follow [documentation-layout.md](documentation-layout.md#separate-hub-and-owner-responsibilities).

## Root Hub Template

For a single-owner plan, keep its local design and full work packages here; omit the child-plan map and cross-owner-only sections.

# <Issue or outcome>: <Observable result>

## Status and scope

- Status: `Draft`
- Tracking issue: `<link or None>`
- Plan ID / canonical path: `<ID and repository-relative path>`
- Canonical owner: `<scope>`
- Branch: `<branch or Not created>`
- Affected owners: `<exact owners>`
- Release gates: `<artifact, dependent WPs, future verification or None>`
- Last evidence refresh: `<YYYY-MM-DD>`
- Implementation references: `Pending`

### Goal and non-goals

State the observable outcome and excluded work. Include material breaking, destructive, security-sensitive or cross-owner effects here, with affected consumers and canonical decision IDs.

### User decisions

Record explicitly confirmed choices; keep design decisions in D-IDs and unresolved consequential questions with their dependent work.

### Compatibility and migration policy

Specify applicable compatibility, existing-data, rollout, rollback or rebuild policy, using current user decisions.

### Plan map

List documents required by [layout and ownership](documentation-layout.md).

| Scope | Document | Owns | IDs/WPs |
| --- | --- | --- | --- |

## Applicability

Use [system-surfaces.md](system-surfaces.md) as the taxonomy. Include affected surfaces and material no-change decisions; omit unrelated rows. Child/owner plans reference assigned S-IDs.

| S-ID | Surface | Current evidence | Target decision or material no-change reason | Owner/WP |
| --- | --- | --- | --- | --- |

## Evidence

Trace the affected current flow with exact paths and symbols. Record facts needed for the design:

| E-ID | Classification | Claim | Evidence | Plan consequence |
| --- | --- | --- | --- | --- |

Classifications are current fact, upstream fact, user decision and release-gated evidence. Keep target design separate from verified facts. For dependency work, use [dependency-changes.md](dependency-changes.md); where a verified capability may replace local code, use [upstream-reuse-audit.md](upstream-reuse-audit.md).

## Decisions

| D-ID | Decision | Evidence | Material rejected alternative | Consequence/owner |
| --- | --- | --- | --- | --- |

## Target design

Include only applicable sections. Local detail belongs in its owning document; shared contracts are referenced by ID.

### Root-owned files and workspace topology

Use an annotated F-ID tree for root-owned artifacts and changed ownership or dependency edges. Link app/crate trees in their owner plans.

### Shared state and data flow

Define cross-owner or root-owned ST contracts using [implementation-contracts.md](implementation-contracts.md#state-and-data-authority). Reference owner-local projections.

### Integration contracts

For affected boundaries, use the C-ID registry and native declarations from [integration-contracts.md](integration-contracts.md).

### Error contracts

For changed failures or recovery, use [error-contracts.md](error-contracts.md). C-IDs and work packages reference ERR meaning once.

### Shared migration and release policy

Record cross-owner compatibility, data/rebuild/rollback, dependency gates, security policy and platform/release order. Exact local implementation belongs in owner plans.

## Work Packages

In the root hub, keep the cross-owner WP dependency/sequence map and define shared or root-owned WPs in full. For a WP with a child plan, record only its ID, owner, observable outcome, dependencies, and link here; keep its full steps and done condition in the child. Keep unsplit owner WPs in full in the main plan.

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

**Verification (reuse existing R/T entries)**

| R-ID | Existing evidence or gap | Selected check | Fixture if needed | Expected result |
| --- | --- | --- | --- | --- |

**Focused validation (commands may reference the shared validation table)**

| Command/manual scenario | Purpose | Required environment | Expected evidence |
| --- | --- | --- | --- |

**Done condition**

State observable result, expected source/generated/schema/dependency diff, removed paths, evidence, and stop conditions.

## Validation

Map requirements to sufficient evidence for the actual impact and current stage. Reuse existing coverage and remove redundant checks. Aggregate validation is required when cross-owner impact, CI/hooks, or the authorized acceptance scope requires it; do not automatically stack it after every focused check.

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

Status transitions follow [documentation-layout.md](documentation-layout.md#manage-lifecycle).

## Execution handoff audit

At an authorized handoff, check consistency across the affected parts: evidence supports decisions; declarations and owner boundaries agree; WPs cover those decisions in dependency order; requirements map to sufficient validation. Resolve material gaps before `Ready` under [the lifecycle rules](documentation-layout.md#manage-lifecycle). This is a consistency pass, not another copy of the field inventories.

## Owner Plan Template

# <Owner>: <Owner-local outcome>

## Root Hub and Ownership

- Plan ID / root hub: `<same ID and link>`
- Owner directory / plan / index: `<exact paths>`
- Root-owned IDs consumed: `<IDs>`
- Owner-authored local IDs/ranges: `<IDs>`
- Assigned WPs: `<IDs>`
- Owns / does not own: `<boundary>`

## Owner-local Evidence and Decisions

Record local E/D facts; reference shared root IDs. Cross-owner choices belong in the hub.

## Owner-local Target Design

Include only affected sections below. Field definitions remain in their linked reference.

| Section | Representation and definition |
| --- | --- |
| Files and ownership | Annotated F-ID tree; [files/modules](implementation-contracts.md#files-modules-and-ownership) |
| Local APIs | L-ID native declarations; [types/methods](implementation-contracts.md#types-traits-functions-and-methods) |
| Boundary implementations | Consumed root C/ERR IDs with local producer/adapter/consumer implementation |
| GPUI state and ownership | ST blocks from [state contract](gpui-application-contracts.md#state-and-ownership-contract) |
| Views, controls, identity, interaction and windows | Relevant sections of [GPUI contracts](gpui-application-contracts.md) |
| Data source and Operation | Resource block, transitions and phase-to-UI mapping from [runtime contract](gpui-application-contracts.md#data-source-and-operation-contract) |
| Form and bound controls | [Form ownership and save contract](gpui-application-contracts.md#form-and-bound-control-contract) |
| Other state/data flow | ST-IDs; [state authority](implementation-contracts.md#state-and-data-authority) |
| Database and migration | DB-ID declarations; [persistence](implementation-contracts.md#database-and-persistence) |
| Generated/synchronized lineage | G-ID source-to-output chain; [lineage](implementation-contracts.md#generated-and-synchronized-artifacts) |
| Icons and i18n | Mappings below; [field semantics](implementation-contracts.md#icons-assets-and-i18n) |
| Security, observability and platform | Local implementation of shared policy; [owner contract](implementation-contracts.md#security-and-observability) |

### Icons and assets

| UI role | Typed icon/slug/SVG path | Owner/F/G IDs | Runtime/bundle placement | Fallback | R/T IDs |
| --- | --- | --- | --- | --- | --- |

### Fluent i18n and bundle localization

| Key | Locale files | Meaning | Variables/plural/select | Caller/UI state | Fallback | R/T IDs |
| --- | --- | --- | --- | --- | --- | --- |

## Owner-local Work Packages

Use the [WP shape](#work-packages) for assigned packages. Keep their full implementation steps here and only the dependency/sequence map in the hub.

## Focused Validation and Handoff

Link owner-local R/T evidence and prerequisites; report deviations to the hub. Aggregate completion stays in the hub.
