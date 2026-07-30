# Dependency Changes

Use this reference for additions, removals, upgrades, downgrades, source changes, pins, Git revisions, submodules, framework/toolchain/generator changes, manifest changes, and lockfile-only resolution changes. Treat them as source and behavior migrations, not version-string churn.

**Contents:** [Baseline](#baseline-and-target-inventory) · [Evidence](#evidence-scope) · [Compatibility](#compatibility-and-release-gates) · [Migration](#breaking-and-behavioral-migration) · [Coupled artifacts](#coupled-skills-generated-artifacts-submodules-and-vendored-source) · [Reuse](#upstream-reuse) · [Completion](#stop-conditions-and-completion-evidence)

## Baseline and Target Inventory

Capture the baseline before mutating manifests, submodules, generated outputs, or lockfiles. Inspect current manifests, resolved sources, Git/submodule SHAs, toolchain/runtime pins, features, generators, CI, packaging, and native bootstrap inputs.

Include one row for every changed direct dependency and material transitive dependency:

| Dependency | Scope/kind | Current declaration/resolution | Target source/version | Authoritative evidence | Local uses/coupled artifacts | Runtime/platform constraints | Classification/migration |
| --- | --- | --- | --- | --- | --- | --- | --- |

Define:

- direct/transitive and runtime/dev/build/proc-macro/generator/framework/toolchain kind;
- exact manifest plus resolved registry/Git/path/submodule source;
- exact target complete version, pin/range policy, tag, or full Git SHA;
- authoritative evidence across the complete crossed interval;
- exact imports, APIs, features, config, generators, skills/docs, CI/packaging, submodules, and vendored consumers;
- duplicate-major/`links`, MSRV/runtime, platform, native, TLS, serialization, and feature constraints;
- compatibility class and exact migration, pin, rejection, or release-gate action.

Use Cargo/Git/managing tools to update lockfiles and submodule state. Never hand-edit a lockfile or invent a recorded SHA.

## Evidence Scope

For each changed direct dependency, read and record:

1. current declaration and resolved source;
2. target release/tag/commit and published contents;
3. official release notes and changelog for the complete interval;
4. applicable migration/upgrade guides;
5. compare range and relevant merged PRs when release records are incomplete;
6. current public API/source/examples/tests for locally used behavior;
7. default/enabled features, MSRV/runtime, platform, native, and TLS changes;
8. dependency-tree changes and material transitives.

Use primary sources and exact URLs, tags, full SHAs, changelog headings, PRs, and local paths. Do not write only “review changelog”.

For Git dependencies without complete releases, treat the pinned-to-target compare range, merged PRs, manifests, source, tests, stories, component docs, and skills as the migration record, and state the release-record limitation.

Research material native/database/TLS/network/runtime/proc-macro/serialization/framework/generator transitives and conflicts that introduce duplicate `links`, runtimes, TLS stacks, or major API surfaces. Ordinary leaf churn needs only its owning direct dependency and resolution reason.

## Compatibility and Release Gates

Assign every proposed target one state:

- `Compatible`: migrate and verify in the current batch.
- `Incompatible`: name the exact blocker and stop only dependent work packages.
- `Pinned`: retain an exact source/version with constraint and removal condition.
- `Release-gated`: name the unavailable artifact and exact future verification procedure.
- `Rejected`: record why the target is unsuitable and leave no exploratory manifest churn.

Do not infer compatibility from a successful build.

For an unreleased final target, split `Known migration` from current to latest published and `Release-gated delta` from latest published to the awaited artifact. Complete known research now. If upstream changes after research, refresh only affected evidence and decisions.

## Breaking and Behavioral Migration

Map every relevant change:

| Upstream change | Old local API/use | Exact call sites | New API/config/behavior | Required edit/deletion | R/T IDs |
| --- | --- | --- | --- | --- | --- |

Include semantic changes that can compile while altering defaults, ordering, serialization, error variants, retry, caching, persistence, component behavior/accessibility, code generation, features, or platform behavior.

Search all affected imports, configuration keys, features, builder/method names, CLI calls, generated artifacts, tests, CI, packaging, docs, and copied skills. Apply verified recommended APIs/configuration, regenerate through real entrypoints, list obsolete adapters/workarounds for deletion, and define residual searches for old paths.

Compilation does not prove recommended-API or migration completeness.

## Coupled Skills, Generated Artifacts, Submodules, and Vendored Source

Include:

| Dependency/target | Coupled artifact | Ownership/provenance | Required synchronization | Expected add/change/delete | Adaptation/deletion | Evidence/check |
| --- | --- | --- | --- | --- | --- | --- |

Cover:

- repo-local skills coupled to GPUI, gpui-component, gpui-form, Rig, or other changed APIs;
- component usage docs copied or adapted from upstream, including newly added, modified, renamed, and removed component documents;
- generated schema/code/assets and registry snapshots;
- Lucide or other submodules and generated typed icon/assets;
- vendored/copy-preserved source and provenance/hash manifests.

For official copied content, synchronize the complete verified upstream set, preserve required bytes, refresh provenance/hash through the verified algorithm, compare directories, and run its focused validator. Do not blindly overwrite repository-owned adaptations such as `gpui-component-usage`; compare semantics and reapply only intentional local ownership.

If a managing tool fails after retrieval, use manual synchronization only when source and hash/provenance algorithms are independently verified. Record the failed command and actual fallback; never claim the manager completed.

## Upstream Reuse

After selecting a target, use `upstream-reuse-audit.md` for every affected wrapper, adapter, custom component, state projection, parser, retry helper, workaround, copied subsystem, and compatibility layer.

A dependency plan is not `Ready` until each affected subsystem has `Reuse directly`, `Adapt`, `Retain`, or `Defer`, with removals listed before additions.

## Stop Conditions and Completion Evidence

Stop only the affected dependency/work packages when the API is unavailable, incompatible, product-sensitive, or unsupported by required runtime/platform constraints. Record blocker, call sites, retained target, retry/removal condition, and future verification. Continue independent compatible work.

Completion evidence must show:

- manifests, Git/submodule pointers, and lockfile resolution match decisions;
- direct/material-transitive inventories and release evidence are complete;
- features, duplicates, MSRV/runtime, platform, native, and TLS constraints are resolved;
- upstream-change rows map to exact code/config edits, deletions, and tests;
- deprecated imports/config/features/APIs/workarounds were searched;
- coupled skills, docs, generated output, submodules, and vendored content were synchronized or evidenced unchanged;
- upstream-reuse decisions were executed;
- pins, rejections, release gates, and stop conditions remain accurate.

Keep aggregate commands, implementation references, owner-doc updates, deviations, and final status in the root hub's validation/completion sections.
