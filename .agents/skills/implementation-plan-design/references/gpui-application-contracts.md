# GPUI Application Planning Contracts

Use this reference when `S-02` through `S-07` are applicable to `app/*`, a GPUI Entity/View/window, `gpui-component`, or a shared app-support crate. It solely defines GPUI owner/runtime/UI contracts; keep general owner-local declarations, cross-boundary contracts, and error meaning in their routed references.

**Contents:** [Focused-skill routing](#focused-skill-routing) · [Applicability](#applicability-rules) · [State and ownership](#state-and-ownership-contract) · [View, component, and identity](#view-component-and-identity-contract) · [Actions, events, subscriptions, and focus](#actions-events-subscriptions-and-focus) · [Data source and operation](#data-source-and-operation-contract) · [Form and bound controls](#form-and-bound-control-contract) · [Tasks, contexts, and windows](#task-context-reentrancy-and-window-contract) · [Layout, theme, and UI state](#layout-theme-and-ui-state-contract) · [Tests and runtime evidence](#gpui-tests-and-runtime-evidence) · [Handoff audit](#gpui-handoff-audit)

## Focused-skill Routing

Load only the focused skills required by the affected surfaces:

| Surface | Skill |
| --- | --- |
| Repository app structure and owner placement | `gpui-app-development` |
| GPUI contexts, Entity, events, actions, focus, windows, elements, and tests | `gpui` |
| Fallible resource loading, refresh, retry, repair, cancellation, and stale data | `gpui-operation` |
| Shared authoritative in-memory state and projections | `gpui-store` |
| Typed editable values, validation, bindings, rebasing, and submit preparation | `gpui-form` |
| Existing components, delegates, state types, events, layout, and custom-component gaps | `gpui-component-usage` |
| Runtime and bundle icons/assets | `gpui-app-icon-usage` |
| Fluent runtime text and bundle localization | `gpui-i18n` |
| Desktop interaction and window validation | `gpui-computer-use-debugging` |

Verify exact APIs in the focused skill and current source before writing type paths, methods, phases, or component builders into the plan.

## Applicability Rules

- Give every mutable fact one authoritative owner. Do not mirror business state, operation phase, loading flags, tasks, selected values, or derived labels without an explicit projection and invalidation rule.
- Do not force every state into `Entity`, `Store`, or `Global`; every visual type into an Entity; or every async request into an operation. Record the selected primitive and the rejected primitive only when the distinction is material.
- Expand only canonical S-IDs marked `Applicable`. Keep `No change` and `N/A` evidence in the canonical applicability matrix; do not create a competing GPUI matrix.
- Assign L-IDs to exact GPUI type/method contracts, ST-IDs to authority and lifecycle flows, ERR-IDs to failures, and R/T IDs to requirements and tests.

## State and Ownership Contract

Provide one labeled contract block for every mutable business fact, editable value, interaction state, task, and cached projection:

```markdown
### ST-<N>: <Mutable fact>

- **Authority:** `<exact type and owner>`
- **Initialization and lifetime:** `<creator, initialization point, destruction/drop condition, strong/weak handles when relevant>`
- **Readers:** `<exact L-IDs/types/methods>`
- **Mutation:** `<sole writers and exact mutation entrypoints>`
- **Publication and projections:** `<notify/observe/subscribe path, selectors, derived state, invalidation>`
- **Persistence boundary:** `<DB/C-ID and write/read path, or None with reason>`
- **Reset and cancellation:** `<trigger, task/subscription behavior, resulting state>`
```

Classify the owner precisely:

- plain struct field for owner-local non-reactive state;
- `Entity<T>` for independently updated GPUI-owned state or views;
- `Store<S>` for shared authoritative in-memory business state;
- typed `Global` only for genuinely app-wide services or state;
- generated form store for typed editable values and validation;
- native control Entity for focus, IME, selection, popup, and incomplete editor state;
- operation for fallible resource acquisition and recovery phase;
- page/controller/service for commands, persistence tasks, notifications, and product policy;
- database or external provider for persisted/external source of truth.

For every `Entity<T>`, specify its creator, strong and weak handles, destruction condition, and whether callbacks may outlive the owner. For every `Store<S>`, specify whether it is local or typed-global, its initialization/retrieval point, selectors and output equality, observers, and retained subscriptions. A Store does not perform I/O, persistence, retry, or task ownership.

Use a Mermaid `flowchart` only when one authority feeds several readers/projections or crosses several owners. Label nodes and edges with ST/L/C/DB IDs and keep the heterogeneous lifetime details in the contract block rather than duplicating them in the diagram.

## View, Component, and Identity Contract

For every new or materially changed GPUI type, provide:

- its L-ID and F-ID;
- complete fields and constructor/method signatures;
- exact trait implementations such as `Render`, `RenderOnce`, `View`, `EventEmitter<E>`, and `Focusable`;
- emitted event enum, public methods, callers, and ownership boundary;
- the existing `gpui-component` type/module, state/delegate type, events, size/variant traits, and source/story evidence used;
- the verified component-library gap before introducing a custom component.

Distinguish identity types:

- explain when `View::entity_id() -> Option<EntityId>` is `Some` because the view is backed by a stable Entity, or `None` because no Entity identity is exposed;
- explain when `Element::id() -> Option<ElementId>` or `.id(...)` is required for keyed element state or interaction;
- state the `ElementId` parent scope and stable-key source;
- do not use a list index as a stable key when insertion, deletion, filtering, or reordering can change identity;
- state whether an Entity identity and element identity intentionally map to each other or remain independent.

If a low-level `Element` is necessary, specify its associated request-layout and prepaint state, child layout, hitboxes, input handling, paint path, and why `Render`/`RenderOnce` plus existing components cannot meet the requirement.

## Actions, Events, Subscriptions, and Focus

Persist the interaction contract rather than only naming the visible control:

- action type and namespace, registration module, default keybinding, `key_context`, handler owner, enable/disable condition, propagation, and menu/button reuse;
- event enum and payload, emitter, subscriber, emission edge, and product consequence;
- `observe`, `subscribe`, or `subscribe_in` selection and why window access is or is not required;
- subscription retention in an owner field or deliberate detached lifetime, plus cancellation/drop point;
- feedback-loop and reentrancy prevention, including update ordering across entities and stores;
- `FocusHandle` owner, initial focus, focus return after dismissal, Tab order, Enter/Escape behavior, modal focus boundary, and disabled behavior;
- IME, selection, composition, and native editor ownership when text input is involved;
- keyboard-equivalent operation, accessible name/description, focus indication, error announcement, and tooltip behavior.

Use numbered steps for a simple trigger-to-result path. Use a Mermaid `sequenceDiagram` only when ordering spans several participants, branches on failure/cancellation, or crosses an async boundary. Name the R-ID trigger, action/event, handler and task owners, ST transitions, UI/focus/notification/i18n result, ERR-IDs, and T-IDs; do not restate exact action or event declarations in the diagram.

## Data Source and Operation Contract

Design the data source before selecting an operation. Write one labeled contract block for every changed resource:

```markdown
### ST-<N>: <Resource>

**Data source**

- **Source and boundary:** `<repository/API/C-ID/DB-ID>`
- **Inputs and authentication:** `<exact input types, credentials, validation/policy>`
- **Acquisition semantics:** `<pagination, streaming, cache, freshness, deduplication>`
- **Data and failures:** `<exact Data L-ID and ERR-IDs>`
- **Publication destination:** `<authoritative owner and projection path>`

**Runtime decision**

- **Repeatable read/retry semantics:** `<whether retry repeats the same read>`
- **Caller-selected repair:** `<Repair value and selection path, or None>`
- **Selected model:** `<operation family, Task/controller state, domain state machine, or No operation>`
- **Exact runtime types:** `<Data/Problem/Repair/Task L- and ERR-IDs>`
- **Owner and task retention:** `<exact type/field>`
- **Reason:** `<why this model matches the verified lifecycle>`
```

Use these selection rules:

- select `refresh::Operation<Data, Problem, Task>` when retry repeats the same read, including initial load, refresh, stale data, and retry;
- select `repair::Operation<Data, Problem, Repair, Task>` when recovery requires an explicit caller-selected repair value;
- keep save, submit, delete, transaction, and other commands in the page/controller/service unless verified operation semantics match them;
- use an explicitly designed domain state machine for streaming or multi-stage execution when a resource operation cannot represent its lifecycle;
- record `No operation` with the concrete reason when work is synchronous, infallible, one-shot, or has no user-visible resource phase.

For a selected operation, specify:

- complete `Data`, `Problem`, `Repair`, and `Task` types and their invariants;
- authoritative owner and exact task-retention location;
- initial load, refresh, retry, repair, cancel, and completion entrypoints;
- foreground/background boundary and completion routing back to the same owner;
- legal transitions and behavior for duplicate triggers or owner disappearance;
- retry/repair product policy and whether stale/degraded data remains usable;
- every operation variant exposed by the chosen family and its UI projection.

Show a linear acquisition path with numbered steps. Use a Mermaid `flowchart` only when it branches through caches, fallbacks, projections, or several owners. Label nodes and edges with ST/L/C/DB/ERR IDs and do not duplicate the contract block.

Use Mermaid `stateDiagram-v2` when legal transitions, repeated triggers, repair, cancellation, or stale/degraded states are non-trivial. Keep phase-to-UI projection as a narrow homogeneous mapping:

| Phase | UI projection: visible data and exact component/message | Actions and control state | Fluent key/variables | R/T IDs |
| --- | --- | --- | --- | --- |

Treat `Ready(empty)` as a successful empty state, not as `Unavailable`. Distinguish initial loading from refresh with settled data and distinguish terminal unavailability from degraded/stale usable data. Keep transition edges in the state diagram and UI projection in the table; do not duplicate either.

## Form and Bound-control Contract

Separate four ownership channels:

1. typed value, baseline/revision, validation, and submit transformation in the form;
2. focus, IME, selection, popup, and incomplete editor state in the bound native control;
3. options/catalog/capabilities, disabled state, placeholder, and non-blocking hints in the app store or controller;
4. save task, loading/retry, persistence, notification, and navigation in the page/controller/service.

Specify the L-IDs for generated form/input types, fields, validation triggers, submit transformation, subscriptions, and bound controls. Give the value/control/catalog/save flow an ST-ID. For dynamic options, define the ordered flow:

1. update the authoritative catalog;
2. update native control options;
3. reproject the existing typed form value;
4. run dynamic validation.

Retain an unavailable typed value for correction. Do not silently choose a fallback, rebase, or persist because options changed. For submission, specify preparation, persistence, and revision-safe rebase behavior when edits occur while save is in flight.

## Task, Context, Reentrancy, and Window Contract

For every task, record:

- spawning context (`App`, `Context<T>`, window-aware context, or background executor);
- foreground/background boundary and any required `Send`/`Sync` data;
- sole `Task` owner, drop/cancel behavior, one-in-flight or ordering rule, and deduplication;
- completion route, weak-owner handling, state publication, user notification, and ERR-ID mapping;
- behavior when the owning Entity or window closes;
- application-shutdown draining or cancellation requirements.

Do not detach lifecycle-critical work. Record reentrancy boundaries explicitly: do not synchronously re-enter an Entity already being updated or rendered, and do not create observer/event feedback loops. When work needs a window after an await, specify the verified window-aware spawning/update path.

For every affected window, dialog, sheet, popover, menu, overlay, or notification, specify:

- root Entity/view and owner;
- window kind/options, platform branches, activation behavior, and initial focus;
- open, dismiss, hide, close, and focus-return paths;
- task and subscription lifetime across close or deactivation;
- interaction with application shutdown and persisted state.

## Layout, Theme, and UI-state Contract

Specify:

- component composition and ownership tree;
- containers, scroll owner, virtualization, clipping, truncation, and stable list identity;
- minimum/maximum/fill sizing, resizing behavior, display scale, and overflow;
- exact theme tokens, component size/variant traits, focus/hover/disabled styles, and contrast behavior;
- loading, empty, ready, refreshing, degraded, unavailable, validation-error, and command-in-flight presentation;
- exact icon and Fluent contracts through the icon and i18n focused skills.

Do not write only “adjust layout” or “show an error”. Name the component, state owner, trigger, visible result, interaction, and localized text.

## GPUI Tests and Runtime Evidence

Choose the narrowest correct test surface:

- use a plain Rust test for pure logic without GPUI entities or windows;
- use `#[gpui::test]` and the appropriate test context for Entity, Store, operation, event, subscription, action, focus, or task behavior;
- use a visual/window test when layout, rendering, focus routing, overlays, or window behavior requires it;
- use desktop interaction validation when runtime window behavior cannot be established by focused tests.

Persist R/T IDs, proposed test names, fixtures, trigger sequence, state/phase assertions, cancellation behavior, focus/action assertions, and cleanup. For desktop validation, name the built app artifact, isolated test data, exact interaction sequence, and observable result; screenshots are supporting evidence, not the sole acceptance criterion.

## GPUI Handoff Audit

Before finalizing a GPUI plan, verify:

- every mutable fact has one authoritative owner;
- every changed data source has an explicit operation-family or no-operation decision;
- every operation phase maps to UI, actions, i18n, and tests;
- every Entity, Store, form, control, task, and subscription has a lifetime owner;
- every action/event/focus/window path names its handler and state transition;
- every custom component or low-level Element has a verified upstream gap;
- no implementation work package asks the executor to choose among GPUI runtime primitives.
