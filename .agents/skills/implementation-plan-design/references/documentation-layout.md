# Development Documentation Layout

Place a plan at its nearest owner: `app/<name>/docs/dev/<plan-id>/README.md`, `crates/<name>/docs/dev/<plan-id>/README.md`, or root `docs/dev/<plan-id>/README.md` for cross-owner work. Use `issue-<number>` when an issue exists, otherwise a descriptive name.

Start with one document. Split only when independent design or execution is easier to understand separately. Link split documents from their common entrypoint; add a discovery link to the relevant `docs/dev/README.md`. Indexes contain purpose and links, without duplicating progress.

Source defines implemented behavior; README/guide explains stable use and architecture; a plan records the change being made. Keep each fact where readers need it. Update current conclusions directly and remove superseded discussion. Reorganize or remove obsolete plans when useful, updating affected links; retain historical evidence only when it has a concrete use.

Plan status reflects code and necessary validation:

- `Draft`: material decisions remain unresolved.
- `Ready`: the affected work can proceed with ordinary implementation judgment.
- `In progress`: implementation or necessary validation remains.
- `Blocked`: an external condition or required decision prevents progress; name it.
- `Done`: the scoped code work and necessary validation are complete.
- `Superseded`: another plan replaces this one, if retaining a pointer is useful.

A usable iteration can be delivered while the larger plan remains in progress. Record results and relevant limitations once. PR merge and Issue closure do not affect these statuses.

Use an ADR only for a lasting decision whose rationale future work needs; ordinary implementation details and progress do not need one.
