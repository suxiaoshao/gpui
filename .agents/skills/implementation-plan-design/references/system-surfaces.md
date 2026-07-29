# Product and System Surfaces

Use this checklist for every implementation plan. Persist an explicit decision for each applicable surface; write `No change` with evidence when appropriate.

## Files and Modules

- exact added, modified, moved, and deleted paths;
- module entrypoints and exports;
- crate/layer ownership;
- no `mod.rs` when repository conventions forbid it.

## UI Components

- existing library components by exact type/module;
- composition and state entities;
- custom component only when the library lacks a verified capability;
- custom component fields, traits (`Render`, `RenderOnce`, `EventEmitter`, focus traits), methods, events, and ownership;
- keyboard, focus, loading, empty, disabled, error, scrolling, resizing, and accessibility behavior.

## Data and State Flow

- event/request entrypoint;
- validation and policy derivation;
- data acquisition and transformation;
- background/foreground boundary;
- local/entity/shared/global state owner;
- persistence and notification path;
- UI projection;
- cancellation, retry, rollback, and shutdown.

Prefer one source of truth. Explain every cached or duplicated projection and its invalidation rule.

## Database

- final tables, columns, types, defaults, constraints, foreign keys, indexes;
- exact repository methods and query filters/order;
- transaction boundaries and atomicity;
- schema rebuild versus compatibility migration policy;
- fixtures, schema generation, query-plan and repository tests;
- behavior when provider success and DB persistence disagree.

## Data Acquisition

- exact local/provider/HTTP/SDK endpoint or repository query;
- authentication and secret boundary;
- pagination, streaming, freshness, cache and TTL;
- timeout, rate limit, offline and partial-response behavior;
- raw payload retention and typed projection.

## Icons and Assets

- exact icon enum variant or SVG path;
- library, app-local runtime asset, and bundle asset ownership;
- added/removed assets and fallback behavior.

## i18n

- exact Fluent/resource keys and locale paths;
- interpolation variables and plural/select behavior;
- fallback language and provider error localization boundary;
- accessibility labels and tooltips.

## Dependencies

- exact crate/package and complete version or release gate;
- features and default-feature policy;
- added and removed packages;
- native/TLS/runtime implications;
- proof that an existing dependency cannot already provide the capability.
