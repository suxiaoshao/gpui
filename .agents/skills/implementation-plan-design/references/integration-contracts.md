# Integration Contracts

Use this reference when two ownership boundaries must agree on data, behavior, lifecycle, compatibility, or rollout. Typical boundaries in this workspace include app ↔ shared crate, Jaco UI ↔ `jaco-core`/`jaco-agent`/`jaco-db`, agent ↔ Rig/provider, MCP/tool/plugin, database service, platform API, and external HTTP/SDK.

Keep owner-internal implementation in `implementation-contracts.md`, GPUI projection in `gpui-application-contracts.md`, and error semantics in `error-contracts.md`.

**Contents:** [Ownership and IDs](#contract-ownership-and-ids) · [Exact bodies](#exact-boundary-bodies) · [Common mechanisms](#workspace-boundary-mechanisms) · [Lifecycle](#ordering-streaming-and-lifecycle) · [Compatibility](#compatibility-and-rollout) · [Validation](#validation)

## Contract Ownership and IDs

Assign one stable C-ID to every affected boundary:

| Contract ID | Direction | Mechanism | Authoritative definition | Producer/owner | Consumers | Compatibility | ERR-IDs | Body/WPs |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `C-01` | `<owner A -> owner B>` | `<Rust API/trait/event/channel/Rig/MCP/HTTP/DB/platform>` | `<path:symbol>` | `<owner>` | `<all consumers>` | `<class>` | `<ERR-IDs>` | `<section/WPs>` |

For every row define:

- exact direction and mechanism;
- sole authoritative handwritten or generated definition;
- producer and all consumers;
- serialization, generation, adapter, and projection chain;
- compatibility class and rollout owner;
- referenced ERR-IDs, F/G/ST IDs, and work packages.

Do not put local-only types in the registry. Do not duplicate normal success shapes in the error catalog or error meaning in transport tables.

## Exact Boundary Bodies

For every C-ID, provide the exact target contract in the boundary's native representation:

- Rust structs/enums/traits/associated types and public method signatures;
- Rig/provider request, response, streaming, tool-call, reasoning, or session adapter types;
- MCP capability/tool/resource/event schemas and runtime ownership;
- HTTP method/path, body mode, request/response, status, headers, authentication, content type, timeout/size/redirect policy;
- database service/repository boundary and transaction ownership;
- platform API, window/hotkey/screenshot/file-system boundary and platform conditions;
- configuration or serialized file schema and unknown-field/version policy.

Then define validation, normalization, conversions, side effects, ordering, authorization, idempotency, cancellation, partial output, and R/T IDs below the declaration. Keep persistence/domain/view types separate unless their invariants intentionally match.

Generated bindings or schemas remain derived from their verified source. Name the G-ID and generation/synchronization entrypoint; do not create parallel handwritten wire types.

## Workspace Boundary Mechanisms

### Cross-crate Rust APIs

Name the owning crate/module, public export, feature gates, downstream manifests, callers, and dependency direction. Provide exact target traits/types/methods and conversions. Include all consumers and remove stale exports, adapters, feature flags, and duplicate local types.

### Rig and Provider Adapters

Treat the verified Rig version and public adapter API as authoritative. Define which behavior Rig owns and which policy remains app/agent-owned. Cover blocking/streaming paths, response/session identifiers, reasoning/config mapping, tool calls, error conversion, cancellation, transport selection, and provider capability differences only when applicable.

Do not add provider-specific special logic without proving Rig cannot own it or the application must enforce a product policy outside Rig.

### MCP, Tools, and Plugins

Define capability discovery, transport/runtime owner, tool/resource schema, approval/security boundary, event ordering, cancellation, shutdown, and UI projection. Name the source of truth for generated schemas or copied skills and all consumers.

### External HTTP/SDK and Platform APIs

Define exact input validation, authentication, SSRF/path/privacy boundaries, timeout/retry/redirect/size limits, streamed-body ownership, platform availability, and cancellation. Use a sequence diagram only when several participants or branching lifecycle make ordering non-obvious.

## Ordering, Streaming, and Lifecycle

Record:

- startup/registration order and readiness gates;
- request/event ordering and backpressure;
- streaming item ownership, aggregation, partial output, deduplication, and finalization;
- cancellation and timeout propagation;
- owner disappearance, window/app shutdown, and resource cleanup;
- external success followed by local persistence/publication failure;
- retry and idempotency boundary.

Use numbered steps for a simple flow, `sequenceDiagram` for several participants, and `stateDiagram-v2` for recurring lifecycle. Label participants/messages with C/ST/ERR/F IDs.

## Compatibility and Rollout

Classify each changed C-ID as additive, behavior-compatible, deprecated, breaking, release-gated, or intentionally incompatible.

When version skew or staged migration is possible, record:

| C-ID | Old producer/new consumer | New producer/old consumer | Rollout order | Temporary compatibility | Removal condition | Rollback |
| --- | --- | --- | --- | --- | --- | --- |

Put executable migration/rollout steps outside the table. Give every temporary adapter one owner and exit condition. Record deletions as explicitly as additions.

For an unpublished app, use the user-selected rebuild/incompatibility policy instead of introducing unnecessary compatibility layers.

## Validation

Map every C-ID to producer tests, consumer tests, conversion/serialization tests, generated/snapshot diffs, integration tests, streaming/cancellation tests, and mixed-version checks when compatibility requires them.

A plan is incomplete when implementation must infer the authoritative boundary, invent a consumer, duplicate a contract, discover an undocumented generation step, or choose compatibility behavior.
