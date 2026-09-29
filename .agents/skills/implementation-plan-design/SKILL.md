---
name: implementation-plan-design
description: Create or revise durable plans for substantial changes with material architecture, data, or cross-owner decisions; review plans when requested.
---

# Implementation Plan Design

Use a plan when a change needs decisions or coordination that cannot be conveyed clearly in the implementation itself. Routine fixes can proceed directly. Deliver the user's requested stage.

Write the outcome, affected ownership, material decisions, implementation sequence, and sufficient verification. Explain public interfaces, data changes, and lifecycle behavior only to the precision needed to resolve consequential ambiguity. Choose private methods, fields, routine library calls, and ordinary recovery mechanics during implementation.

Ground decisions in the current source and relevant upstream API. Investigate dependency changes where local behavior or compatibility depends on them; expand research when concrete uncertainty remains. Generated artifacts follow their actual source and generator. Reuse existing components and services where they fit the required behavior.

Use current accepted decisions to rewrite the plan. Remove superseded proposals, redundant inventories, and obsolete obligations. Historical wording and structure have no default priority. Keep rationale only when it explains a decision that still matters.

A development plan tracks code work and its necessary validation. PR/commit links are optional references; pushing, merging, and closing issues are managed by GitHub. They do not determine plan completion.

During iteration, finish the affected build, key regression checks and any startup check needed for a usable result, then deliver for user feedback. Actual failures require focused repair and retesting. Do not exhaust optional UI, packaging, platform or error scenarios before delivery, or automatically preserve them as future required work. Run broader acceptance when requested or required by the integration scope.

- [Document layout](references/documentation-layout.md): choose location and ownership when creating or reorganizing a plan.
- [Optional outline](references/plan-template.md): use when a starting structure would help.
