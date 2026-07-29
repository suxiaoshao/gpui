# Implementation-ready Plan Template

Use this structure for durable plans. Combine sections only when the task is genuinely small.

## 1. Status and Scope

- issue/branch/document location;
- current phase and release gates;
- goals, non-goals, user decisions;
- compatibility/rebuild policy.

## 2. Evidence Snapshot

- current repository flow with exact files;
- upstream facts with release/tag/PR/commit/docs;
- dependency evidence table;
- confirmed constraints and explicit no-change decisions.

## 3. Decisions

For each decision record evidence, selected design, rejected alternative only when material, and consequences. Do not persist unresolved questions.

## 4. Target Architecture

- module/file tree;
- component composition;
- type, trait, method and error contracts;
- ownership/global-state map;
- database model;
- end-to-end data and control flow;
- failure/cancellation/shutdown sequences.

## 5. Upstream Reuse Audit

- reuse/adapt/retain/delete table;
- deleted workarounds and wrappers;
- remaining custom responsibility.

## 6. Work Packages

Give each package a stable ID and dependency order.

### WP-N: Outcome

**Prerequisites**

- prior WP or release gate.

**Evidence**

- exact local and upstream sources already researched.

**Files**

- added/modified/deleted paths and responsibility.

**API contract**

- concrete types, traits, associated types, methods and visibility.

**Implementation flow**

- ordered edits and state transitions.

**Errors and lifecycle**

- cancellation, retry, partial progress, persistence, shutdown.

**UI/data/database/icons/i18n/dependencies**

- applicable changes or explicit `No change` decisions.

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |

**Validation**

- exact focused commands and expected evidence.

**Done condition**

- observable result, expected diff, removed old APIs, and stop conditions.

## 7. Cross-package Validation

- formatting, build, tests, clippy/lint, dependency tree, schema/query plan, platform CI, UI/manual/API smoke tests;
- commands actually required by repository instructions.

## 8. Execution Handoff Audit

Before finalizing, verify:

- no work package asks the implementer to choose architecture;
- no broad “research/check/handle breaking changes” task remains;
- every proposed upstream API was verified;
- every new type has behavior contracts, not fields alone;
- all system surfaces are addressed;
- upstream replacement/deletion was considered;
- requirements map to tests and done evidence;
- any remaining release gate is narrow and has an exact verification procedure.
