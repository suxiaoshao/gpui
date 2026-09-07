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

Use a durable plan only when the skill workflow requires one. Place a single-owner plan under that app/crate's `docs/dev/<plan-id>/README.md`; place a cross-owner or workspace plan under root `docs/dev/<plan-id>/README.md`.

Start with one document. Split out a same-ID owner plan only when independent design, implementation sequencing, or validation boundaries benefit from a separate document. A dependency or manifest change alone does not require a child plan. Keep brief owner-specific changes in the main plan.

Preserve existing plan paths and links when revising a plan; this rule does not require reorganizing historical documents.

## Name the Plan Set

Choose one `plan-id` and reuse it in any child plans:

- when a tracking issue exists, use `issue-<number>`;
- otherwise use a concise kebab-case task slug that describes the observable outcome.

The entrypoint is always `README.md`; do not place durable plans as flat Markdown files. Different issues and different tasks must use different plan directories. Put transient implementation details in the document, not in the plan ID.

## Separate Hub and Owner Responsibilities

The main plan (root hub for cross-owner work) owns:

- status, scope, non-goals, compatibility/rebuild policy, and release gates;
- affected surfaces, material no-change decisions, and shared evidence/decisions;
- cross-owner integration and error contracts;
- the owner-plan map, cross-owner sequencing, aggregate validation, and completion evidence.

When split out, each child owner plan owns only:

- its main-plan link, owner directory, shared IDs it consumes, local work packages, and explicit boundary;
- exact owner-local files, symbols, local contracts, state/data flow, tests, and validation;
- owner-local work packages and deviations that require root-hub synchronization.

The main plan owns shared IDs and any unsplit owner detail. When child plans are needed, the root hub owns S/C/ERR IDs plus shared/cross-owner E/D/R/T/WP IDs. An owner plan owns its local E/D/F/L/DB/G/ST/R/T/WP IDs; it consumes root-owned C/ERR IDs and defines only its local implementation of those contracts. Assign non-overlapping ranges when several owners author the same ID family. An owner plan must not define a sibling app/crate's implementation. Reference shared and cross-owner facts by their root-hub IDs instead of copying them. Do not duplicate the root goal, status, applicability matrix, shared evidence, decisions, contracts, aggregate progress, or completion evidence. Link every owner plan from the hub and every owner plan back to the hub.

## Maintain Indexes and Links

Use these index scopes:

- root `docs/dev/README.md` discovers workspace plans and relevant single-owner plan entrypoints;
- app/crate `docs/dev/README.md` discovers only that owner's plans;
- each root hub maps every affected owner plan;
- a plan index contains links and one-sentence purpose, not a second status/progress ledger.

Update indexes needed to discover the plan; do not create per-owner indexes when no owner plan exists. Use repository-relative Markdown links. Do not copy owner-local implementation details into an index or list an owner plan under an unrelated app/crate.

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

Fill the main plan's [completion evidence](plan-template.md#completion-evidence) from actual implementation and validation. Synchronize affected owner documentation and discovery links; indexes remain discovery lists rather than progress ledgers.

## Promote Durable Decisions to ADRs

Keep issue-specific choices and sequencing in the plan. Create an ADR only when a decision must constrain future work or establishes a long-lived architecture, protocol, ownership, persistence, security, or compatibility policy.

Place the ADR at the nearest common owner under its verified `docs/adr` convention. Link it to the originating plan. Do not use ADRs for work-package progress, temporary release gates, or facts already owned by source and README files.
