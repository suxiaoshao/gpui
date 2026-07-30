# Owner-local Implementation Contracts

Use this reference after tracing current behavior. Define only implementation owned within one app, crate, module, component, state owner, database owner, packaging owner, or generated-artifact owner. Put GPUI runtime semantics in `gpui-application-contracts.md`, cross-boundary contracts in `integration-contracts.md`, and error identity/propagation in `error-contracts.md`.

**Contents:** [Files and ownership](#files-modules-and-ownership) · [Types and methods](#types-traits-functions-and-methods) · [State authority](#state-and-data-authority) · [Database](#database-and-persistence) · [Async lifecycle](#async-lifecycle-and-concurrency) · [Generated lineage](#generated-and-synchronized-artifacts) · [Icons and i18n](#icons-assets-and-i18n) · [Security and diagnostics](#security-and-observability)

Verify exact current names and behavior from owner README files and executable sources. Label every proposed name and signature as target design.

## Files, Modules, and Ownership

For every added, modified, moved, generated, synchronized, vendored, or deleted path, define:

- owning app, crate, shared module, repository scope, or packaging boundary;
- one responsibility and explicit non-responsibilities;
- public exports, callers, consumers, manifests, configuration, and build inputs;
- dependency direction and why the type belongs at this boundary;
- handwritten, generated, synchronized, maintained-snapshot, submodule, or vendored status;
- legacy paths and consumers removed in the same work;
- owner README, index, or ADR updates required by the final architecture.

Show paths once in annotated owner trees with stable F-IDs, action, artifact kind, responsibility, and source-of-truth role. Put longer boundary reasoning below the tree. Work packages reference F-IDs instead of repeating inventories.

Choose boundaries for cohesion, dependency direction, testability, and long-term ownership. Require named consumers before creating shared code. A cross-owner refactor must coordinate contracts, consumers, manifests, tests, generated artifacts, packaging inputs, and documentation.

## Types, Traits, Functions, and Methods

Give every new or materially changed owner-local contract a stable L-ID and a language-tagged target declaration. For Rust, specify:

- exact structs/enums and fields/variants;
- visibility, derives, generics, bounds, associated types, and serde representation;
- required trait implementations and exact method signatures;
- inherent constructors, commands, queries, conversions, and shutdown methods;
- ownership/borrowing and `Send`/`Sync`/`Clone` requirements;
- identity, equality, ordering, hash, serialization, and invalid-state invariants.

For every method, function, handler, repository operation, or service command, define:

- callers and call frequency;
- inputs, validation, normalization, and authorization;
- output and side effects;
- runtime/thread/context/connection/transaction requirements;
- state, persistence, event, and notification changes;
- referenced ERR-IDs;
- idempotency, retry boundary, cancellation, partial progress, and R/T IDs.

Do not leave signatures, invariants, conversions, or failure boundaries to implementation. Use pseudocode only when declarations cannot fix the behavioral rule; do not write full ordinary method bodies.

When a type implements GPUI traits, owns an Entity/Store/Form/Operation, or needs Window/App contexts, expand it through `gpui-application-contracts.md` rather than duplicating those semantics here.

## State and Data Authority

Assign one ST-ID and one authoritative owner to every mutable business value, task, and cached projection. Record:

- writer and readers;
- local memory, GPUI owner, database, file, configuration, provider, or external authority;
- transformation and projection;
- persistence/cache behavior;
- publication/notification path;
- invalidation/reset and stale behavior;
- reason and synchronization rule for every duplicated projection.

Use numbered steps for a simple linear flow, Mermaid flow/sequence/state only when ownership branches, ordering matters, or lifecycle recurs, and a table only for homogeneous values.

Inspect actual cache policy instead of inventing TTL, offline, pagination, retry, or shutdown behavior. Mark irrelevant lifecycle behavior `N/A`.

## Database and Persistence

Give every changed schema object, repository query, or transactional command a DB-ID. Provide exact target SQL, Diesel schema/model declarations, and repository signatures as applicable.

Define:

- final table, column, type, default, constraint, foreign key, index, uniqueness, and ordering;
- exact query filters, pagination, count, projection, and consumers;
- transaction and atomicity boundary;
- existing-data conversion, loss policy, backfill, rollback, rebuild, and compatibility;
- behavior when an external action succeeds but local persistence or publication fails;
- migration and generated-schema entrypoints verified from this repository;
- fixtures, schema/repository/query-plan tests, and R/T IDs.

The application is not yet public only when the user has explicitly allowed a destructive rebuild policy. Do not add compatibility migrations that the selected product policy does not require.

## Async Lifecycle and Concurrency

Apply lifecycle detail only when asynchronous work or mutable resources require it. Define:

- task/resource owner and retention;
- foreground/background/runtime boundary;
- one-in-flight, ordering, stale-result, and reentrancy rules;
- timeout/retry/cancellation owner;
- partial completion and rollback;
- connection/resource use across awaits;
- explicit shutdown for long-lived resources.

Use the GPUI contract for `Task`, Entity, Window, Operation, subscription, and app-shutdown details. Do not rely on async work in `Drop`, detach lifecycle-critical work, or invent cancellation systems for synchronous operations.

## Generated and Synchronized Artifacts

Give every affected lineage a G-ID and define:

- handwritten or canonical upstream source;
- maintained snapshot/intermediate, if any;
- generated, synchronized, copied, submodule, or vendored output;
- verified owner-supported entrypoint or provenance/hash algorithm;
- expected additions, modifications, renames, and deletions;
- manual-edit and formatting policy;
- consumers and drift checks.

Change handwritten sources first. For copied upstream component docs or skills, compare complete directories and explicitly handle additions, deletions, renames, and repository-owned adaptations. Do not patch derived output around its source or run repository formatters over byte-preserved third-party content.

## Icons, Assets, and i18n

For icons and assets, define exact typed icon variant, Lucide/upstream slug or SVG path, ownership, additions/deletions, runtime versus bundle placement, generator/xtask input, and fallback. Use `gpui-app-icon-usage` for GPUI apps.

For every user-visible label, message, validation result, error, accessibility string, title, menu item, or formatted value, define exact Fluent key, all locale files, meaning, interpolation/plural/select variables, caller/UI state, fallback, and tests. Keep macOS bundle localization separate from runtime Fluent resources. Use `gpui-i18n`; error-specific mappings live in `error-contracts.md`.

## Security and Observability

For each owner-local trust boundary, define authentication/authorization inputs, credential and filesystem/database lifecycle, public-error allowlist, provider/tool input validation, and secret redaction.

For observable paths, define span/event owner, structured fields, severity, correlation identifiers, provider/storage/operation status, sampling when relevant, and cancellation/shutdown evidence. Inspect current logging before changing it.

Never persist or log real credentials, tokens, cookies, passwords, private keys, full environment contents, unredacted provider payloads, or internal causes exposed through user-visible errors.
