# Upstream Reuse and Local-code Audit

Use this reference after dependency research and before finalizing new custom code.

## Inventory

Search the repository for local components, adapters, wrappers, state machines, protocol parsers, retry logic, serialization helpers, and workarounds in the affected domain. Compare them with target upstream source, API docs, stories/examples, tests, and component documentation.

Do not limit UI-library review to Markdown file additions and deletions. Source can add methods or behavior without adding a component document.

## Decision Table

Record:

| Local implementation | Upstream capability and evidence | Semantic differences | Decision | Files deleted/changed | Regression tests |
| --- | --- | --- | --- | --- | --- |

Use one of these decisions:

- `Reuse directly`: remove the local implementation.
- `Adapt`: keep a thin repository-specific adapter and delegate the general behavior upstream.
- `Retain`: upstream capability does not meet verified requirements; state the gap.
- `Defer`: replacement is outside scope; state why and create no hidden migration work.

## Audit Questions

- Does the dependency now expose the protocol/session/collector/error API we planned to write?
- Can blocking and streaming paths share an upstream result or driver?
- Did upstream add a component, binding, state method, accessibility behavior, or layout primitive that removes a local workaround?
- Did upstream fix the original reason a wrapper or fallback existed?
- Is a same-named upstream component only visual while the local implementation owns domain state or validation?
- Can local serialization, retry, pagination, caching, or native-platform code be deleted?
- Does a breaking change make the local abstraction invalid rather than merely uncompilable?

## Deletion-first Output

List removals before additions. For retained custom code, narrow its responsibility and name the upstream APIs it must delegate to. A dependency update plan is incomplete until this table has been reviewed for every affected local subsystem.
