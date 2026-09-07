# Upstream Reuse and Local-code Audit

Use this reference after dependency research or whenever a verified upstream/repository capability may replace local code, copied content, or a planned custom implementation.

## Inventory

Start from the local implementation or upstream change relevant to the task. Follow its consumers and semantic dependencies far enough to establish whether replacement is viable. Broader repository inventory requires a task that calls for it.

Compare behavior, state ownership, error handling, serialization, accessibility and platform requirements where relevant. Verify source/API behavior when release notes or component-document changes do not establish it.

## Decision Table

Record:

| D-ID | Local implementation/F-L-G IDs | Upstream capability and evidence | Semantic differences | Decision | Files deleted/changed | R/T IDs |
| --- | --- | --- | --- | --- | --- | --- |

Use one of these decisions:

- `Reuse directly`: remove the local implementation.
- `Adapt`: keep a thin repository-specific adapter and delegate the general behavior upstream.
- `Retain`: upstream capability does not meet verified requirements; state the gap.
- `Defer`: replacement is outside scope; state why and create no hidden migration work.

## Audit Questions

- Does the dependency now expose the protocol/session/collector/error API we planned to write?
- Can blocking and streaming paths share an upstream result or driver?
- Did upstream add a component, binding, state method, accessibility behavior, or layout primitive that removes a local workaround?
- Did upstream add, rename, modify, or delete component documentation or a copied skill that changes the repository-owned adaptation?
- Did upstream fix the original reason a wrapper or fallback existed?
- Is a same-named upstream component only visual while the local implementation owns domain state or validation?
- Can local serialization, retry, pagination, caching, or native-platform code be deleted?
- Does a breaking change make the local abstraction invalid rather than merely uncompilable?

## Deletion-first Output

List removals before additions. For retained custom code/content, narrow its responsibility and name the upstream API/source it delegates to. Reference the decision's D-ID from work packages. Resolve the selected candidates and their migration dependencies before their work packages become executable.
