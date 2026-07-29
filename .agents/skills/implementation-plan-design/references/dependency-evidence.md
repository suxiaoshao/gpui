# Dependency Evidence and Migration

Use this reference whenever a plan changes crates, Git dependencies, submodules, frameworks, SDKs, build tools, or lockfiles.

## Research Sources

For each changed direct dependency, inspect:

1. current manifest declaration and resolved lockfile source;
2. target release/tag/commit and published package contents;
3. official release notes and changelog for the complete version interval;
4. migration or upgrade guide;
5. compare range and relevant merged PRs when release notes are incomplete;
6. public API source and examples for APIs used by the repository;
7. default and enabled feature changes, MSRV, platform/native/TLS requirements;
8. dependency-tree changes and high-risk transitive upgrades.

For a Git dependency without complete releases or changelog, treat the pinned-to-target commit range, merged PRs, manifests, source, stories, and docs as the migration record. State that the upstream lacks a complete changelog instead of pretending one was read.

## Required Evidence Table

Record one row per changed direct dependency and important transitive dependency:

| Dependency | Current source/version | Target source/version | Evidence | Breaking/behavior changes | Affected local call sites | Features/MSRV/platform | Migration decision |
| --- | --- | --- | --- | --- | --- | --- | --- |

Use exact URLs, tags, full Git SHAs, changelog headings, PR numbers, and local file paths. Do not write only “read changelog”.

## Breaking-change Mapping

For every relevant change, record:

| Upstream change | Old local API/use | Exact call sites | New API/behavior | Required edit/delete | Tests |
| --- | --- | --- | --- | --- | --- |

Include semantic changes even when code still compiles: retry budgets, defaults, ordering, serialization, error variants, caching, persistence, feature defaults, and platform behavior.

## Unreleased Targets

Split the evidence:

- `Known migration`: current repository version to the latest published version; complete now.
- `Release-gated delta`: latest published version to the awaited release; list the exact gate and API verification commands.

Do not postpone known migration work because a later target is unreleased.

## Transitive Scope

Read full release material for:

- changed direct dependencies;
- native, database, TLS/network, runtime, proc-macro, serialization, and framework transitives with material changes;
- transitives that introduce duplicate `links`, runtime, TLS, or major-version API surfaces.

For ordinary transitive churn, record its owning direct dependency and lockfile reason. Do not require exhaustive changelog research for every leaf package.

## Dependency Decision

Specify:

- exact version and source policy;
- enabled/default features and why;
- added and removed dependencies;
- duplicate-version policy;
- lockfile expectations;
- platform bootstrap changes;
- commands for source verification, tree inspection, focused checks, and full validation;
- stop conditions for incompatible or missing upstream APIs.
