# Development Documentation Layout

Use this reference to choose where a durable plan lives, which artifact owns each fact, and how plan lifecycle is recorded.

**Contents:** [Artifact roles](#separate-artifact-roles) · [Plan set](#create-the-root-hub-and-owner-plans) · [Plan IDs](#name-the-plan-set) · [Responsibilities](#separate-hub-and-owner-responsibilities) · [Indexes](#maintain-indexes-and-links) · [Lifecycle](#manage-lifecycle) · [Completion](#synchronize-completion) · [ADRs](#promote-durable-decisions-to-adrs)

## Separate Artifact Roles

Keep every fact in one durable owner:

| Artifact | Owns |
| --- | --- |
| Source, manifests, migrations, schemas, configuration, generator inputs | Executable behavior and runtime contracts |
| Nearest owner `README.md` | Current stable architecture, ownership, prerequisites, and workflows |
| Root plan hub | Shared scope, evidence, decisions, cross-owner contracts, sequencing, status, aggregate validation, and completion evidence |
| App/crate owner plan | Only that owner's files, contracts, state/data flow, work packages, tests, and focused validation |
| `docs/adr` record | Long-lived decisions that constrain future architecture, protocol, ownership, persistence, security, or compatibility |
| `docs/dev/README.md` index | Discovery within its documented scope; no duplicated status or progress |
| `AGENTS.md` | Stable repository policy and documentation routing |

Do not turn a plan into a second permanent runtime specification. When implementation changes stable behavior, update executable sources and the nearest owner README or ADR.

## Create the Root Hub and Owner Plans

First decide whether the task is non-trivial enough to require a durable plan under the skill workflow. A truly local behavior-preserving correction does not create a plan set. Once a durable plan is required, always create its root hub:

```text
docs/dev/<plan-id>/README.md
```

Inventory every affected app, crate, shared module, root configuration, database, generated source, and packaging boundary. For every affected app or crate, create a same-ID owner plan:

```text
app/<name>/docs/dev/<plan-id>/README.md
crates/<name>/docs/dev/<plan-id>/README.md
```

This topology also applies to a single-app or single-crate durable task. Do not create owner plans for untouched apps/crates. Root-owned workspace configuration, CI, packaging, or repository-wide tooling may stay in the root hub when no app/crate owns it.

## Name the Plan Set

Choose one `plan-id` and use it unchanged at the root and under every affected owner:

- when a tracking issue exists, use `issue-<number>`;
- otherwise use a concise kebab-case task slug that describes the observable outcome.

The entrypoint is always `README.md`; do not place durable plans as flat Markdown files. Different issues and different tasks must use different plan directories. Put transient implementation details in the document, not in the plan ID.

## Separate Hub and Owner Responsibilities

Let the root hub own:

- status, scope, non-goals, compatibility/rebuild policy, and release gates;
- the complete applicability matrix and shared evidence/decisions;
- cross-owner integration and error contracts;
- the owner-plan map, cross-owner sequencing, aggregate validation, and completion evidence.

Let each app/crate owner plan own only:

- its root-hub link, owner directory, root-owned IDs it consumes, owner-local ID ranges/work packages, and explicit boundary;
- exact owner-local files, symbols, local contracts, state/data flow, tests, and validation;
- owner-local work packages and deviations that require root-hub synchronization.

The root hub owns all S/C/ERR IDs plus shared/cross-owner E/D/R/T/WP IDs. An owner plan owns its local E/D/F/L/DB/G/ST/R/T/WP IDs; it consumes root-owned C/ERR IDs and defines only its local implementation of those contracts. An owner plan must not define a sibling app/crate's implementation. Reference shared and cross-owner facts by their root-hub IDs instead of copying them. Do not duplicate the root goal, status, applicability matrix, shared evidence, decisions, contracts, aggregate progress, or completion evidence. Link every owner plan from the hub and every owner plan back to the hub.

## Maintain Indexes and Links

Use these index scopes:

- root `docs/dev/README.md` discovers every durable root plan hub;
- app/crate `docs/dev/README.md` discovers only that owner's plans;
- each root hub maps every affected owner plan;
- a plan index contains links and one-sentence purpose, not a second status/progress ledger.

Create or update the root index and each affected owner index as part of the plan set. Use repository-relative Markdown links. Do not copy owner-local implementation details into an index or list an owner plan under an unrelated app/crate.

## Manage Lifecycle

Use only these canonical statuses:

| Status | Meaning |
| --- | --- |
| `Draft` | Evidence, user decisions, contracts, work packages, or acceptance criteria remain incomplete |
| `Ready` | Every work package is executable and all material user choices are confirmed |
| `In progress` | Authorized implementation has started |
| `Blocked` | A named external condition or required user decision prevents all meaningful progress |
| `Done` | Required implementation and validation are complete and recorded |
| `Superseded` | A linked successor replaces this plan |

Track a narrow release gate independently when unrelated work remains executable. Do not mark `Ready` while the plan contains unresolved questions, speculative APIs, hidden architecture choices, or unverified dependency behavior.

Keep completed plans at their original paths. When replacing one, mark it `Superseded` and link predecessor and successor bidirectionally.

## Synchronize Completion

Before `Done`, record in the root hub:

- actual commits and PR;
- actual added, modified, moved, deleted, generated, synchronized, and vendored files;
- delivered stable contract/DB/error/dependency/requirement IDs;
- commands and results actually run;
- manual, packaged-app, or real-API scenarios actually exercised;
- accepted deviations from target design;
- unverified boundaries and reason;
- owner README, index, and ADR synchronization.

Update stale root/owner indexes and plan headers when status changes. Do not leave a discovery index saying work is unimplemented after the root hub records completion.

## Promote Durable Decisions to ADRs

Keep issue-specific choices and sequencing in the plan. Create an ADR only when a decision must constrain future work or establishes a long-lived architecture, protocol, ownership, persistence, security, or compatibility policy.

Place the ADR at the nearest common owner under its verified `docs/adr` convention. Link it to the originating plan. Do not use ADRs for work-package progress, temporary release gates, or facts already owned by source and README files.
