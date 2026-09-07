# Error Contracts

Use this reference whenever a plan adds, removes, renames, remaps, or changes handling of a validation, domain, database, provider, transport, MCP/tool, GPUI, cancellation, or shutdown failure. Treat error identity and recovery as an end-to-end contract independent of one crate or UI component.

**Contents:** [Canonical model](#canonical-model-and-ownership) · [End-to-end chain](#end-to-end-chain) · [Producer mapping](#producer-normalization) · [Boundary mapping](#boundary-adapters) · [GPUI recovery](#gpui-classification-and-recovery) · [Failure classes](#failure-classes-and-partial-success) · [Security](#compatibility-security-and-observability) · [Validation](#validation) · [Order](#synchronization-order)

## Canonical Model and Ownership

Give every materially distinct failure one stable ERR-ID in the plan:

| ERR-ID/runtime code | Category | Meaning and exact trigger | Safe details type | Retry/idempotency | Default recovery | Compatibility |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `ERR-01` | `<class>` | `<testable semantics>` | `<L-ID/type/None>` | `<policy>` | `<action>` | `<policy>` |

Name the exact typed producer variant or stable public code. Preserve cancellation distinctions and the safe-details unknown-field policy.

Provide Rust declarations for changed error enums, details types, `From`/mapping functions, parser/encoder functions, and recovery classification. The catalog owns meaning and safe details; operations, adapters, notifications, and tests reference ERR-IDs.

Do not create different public identities for identical semantics merely because producers differ. Do not collapse failures that require different user action.

## End-to-end Chain

Trace each affected failure:

```text
validation/domain/database/provider/MCP/platform failure
  -> owner-local typed error
  -> cross-crate/provider adapter and canonical ERR-ID
  -> persistence/transaction/partial-output consequence
  -> Operation or runtime state transition
  -> GPUI notification, inline state, retry/repair/stop action, and Fluent text
  -> structured diagnostic with redaction
```

Use numbered steps for one linear path. Use a sequence diagram when several crates/providers, partial output, rollback, cancellation, or exactly-once recovery make ordering material. Label nodes with F/L/C/ERR/ST IDs.

## Producer Normalization

Record every producer variant or condition that reaches another owner or the user:

| Mapping ID | Producer boundary | Variant/condition | ERR-ID | Details conversion | Internal cause/log policy | Exhaustiveness test |
| --- | --- | --- | --- | --- | --- | --- |
| `EM-01` | `<path:symbol>` | `<variant/trigger>` | `<ERR-ID>` | `<typed conversion>` | `<policy>` | `<T-ID>` |

Cover applicable validation, domain/service errors, database/pool/migration failures, provider/Rig errors, external HTTP, MCP/tool errors, platform failures, cancellation, owner disappearance, and shutdown.

Prefer structured upstream status, enum, or payload data. Never classify by localized, display, or debug-message matching. A new producer variant must not silently inherit an unrelated fallback.

## Boundary Adapters

For every affected C-ID, record only its ERR-ID mapping and exact representation:

| Adapter ID | C-ID | ERR-ID | Exact representation | Partial/output semantics | Unknown fallback | Compatibility |
| --- | --- | --- | --- | --- | --- | --- |
| `EA-01` | `<C-ID>` | `<ERR-ID>` | `<type/status/event/L-ID>` | `<behavior>` | `<fallback>` | `<policy>` |

Provide native declarations for public Rust errors, Rig/provider mappings, MCP error data, HTTP status/body, serialized configuration errors, and database-service responses as applicable.

Keep normal success shapes in `integration-contracts.md`. Boundary adapters cannot change ERR-ID meaning or expose internal causes.

## GPUI Classification and Recovery

For every affected ERR-ID, define:

| ERR-ID | Handling owner | Runtime/Operation transition | Persisted/visible side effects | Recovery action | Fluent key/variables | UI component/action | Diagnostic |
| --- | --- | --- | --- | --- | --- | --- | --- |

Specify:

- page/controller/Store/Operation/runtime owner;
- retry, repair, stop, cancel, edit-and-resubmit, reauthenticate, reopen, rebuild, or terminal behavior;
- whether settled/stale data or partial output remains visible;
- whether the error propagates, is swallowed, or emits exactly once;
- notification versus inline/field/dialog/resource-status presentation;
- exact Fluent key, variables, accessible behavior, and safe fallback;
- focus/navigation effect and disabled actions;
- structured log/span owner, fields, severity, and correlation.

Use typed classification. Keep Operation transition semantics in `gpui-application-contracts.md` and reference ERR-IDs instead of redefining failure meaning there.

## Failure Classes and Partial Success

Distinguish:

- validation/policy rejection;
- state conflict, including an operation disallowed while a resource is running;
- authentication/credential failure;
- provider or dependency failure;
- transport/protocol failure;
- database/persistence/migration failure;
- client parsing or generated-contract drift;
- cancellation and owner/window disappearance;
- shutdown;
- unknown internal failure.

Record occurrence-specific behavior:

| C-ID/operation/symbol | EM-IDs | Possible ERR-IDs | Side effects before failure | Rollback/partial behavior | UI call site | Override |
| --- | --- | --- | --- | --- | --- | --- |

Cover external success followed by local persistence failure, partial stream already shown, duplicate text/tool-call prevention, transaction rollback, retry safety, and cleanup. Do not repeat catalog meaning or default UI copy in this occurrence table.

## Compatibility, Security, and Observability

Treat variant/code renames, semantic changes, safe-details changes, adapter changes, and recovery changes as compatibility changes. Define old/new behavior, rollout order, temporary alias owner, removal condition, and fallback.

Never expose database query text, environment contents, transport debug output, stack traces, filesystem paths, internal type dumps, tokens, credentials, secrets, provider raw payloads, or `Debug` renderings through notifications, Fluent variables, public details, HTTP/MCP responses, or persisted snapshots.

Define public field allowlists, protected-resource disclosure, unknown/internal fallback, log location, correlation identifier, severity, and redaction.

## Validation

Map changed error-contract risks to sufficient existing or missing evidence:

| R-ID | Layer | Scenario | Fixture/producer | Expected ERR-ID/encoding | State/security/UI assertions |
| --- | --- | --- | --- | --- | --- |

Select checks for the affected failure, recovery or security contract. One scenario may cover several boundaries; this table is not a quota of tests per layer.

Compilation alone does not verify an error contract.

## Synchronization Order

Order affected WPs from canonical error identity through producer normalization, boundary encoding and consumer recovery. Include stale code/alias/translation removal when applicable. Use the contracts above as the field definitions and the plan's validation table for sufficient regression evidence; do not recreate the same inventory as a handoff checklist.
