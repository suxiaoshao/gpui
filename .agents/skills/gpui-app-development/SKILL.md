---
name: gpui-app-development
description: GPUI application development conventions for this gpui workspace. Use when implementing, refactoring, reviewing, or debugging app code under app/ or shared app support crates, especially when deciding module placement, Render patterns, entity creation, resources, cross-app sharing, validation, or which GPUI-focused skill to load next.
---

# GPUI App Development

Use this skill for workspace-specific GPUI app decisions. Use `gpui` for framework API details and `gpui-component-usage` for component selection.

## Workflow

1. Identify the app or shared crate affected by the change.
2. Follow the existing module and view patterns in that app before introducing new structure.
3. Use `gpui-component-usage` before building custom controls.
4. Use `gpui-app-icon-usage` when changing UI icons, runtime assets, or bundle app icons.
5. Use `gpui-i18n` when adding or changing user-facing text, Fluent locale files, language settings, or macOS bundle localization.
6. Use `gpui-store` only when changing `crates/gpui-store` or deliberately integrating it into app state.
7. Load `gpui` only when framework API details are needed, then use its Navigation to select the relevant reference file for actions/keybindings, async tasks, context/windows, entity state, events/subscriptions, focus, global state, layout/styling, elements, or tests.
8. Keep app behavior tied to local validation: run focused tests/checks for the touched app and use `gpui-computer-use-debugging` when runtime UI behavior matters.

## Workspace Rules

- App entrypoints live under `app/{name}/src/main.rs`; preserve each app's existing `app`, `foundation`, `features`, and state boundaries.
- Put reusable cross-app behavior in shared crates such as `crates/window-ext`, `crates/platform-ext`, `crates/app-theme`, `crates/app-assets`, or `crates/gpui-store` instead of copying it across apps.
- Implement views with `Render` or the app's existing pattern. Use lower-level `Element` only when high-level rendering cannot express the behavior.
- Create context-managed state with `cx.new(...)`; do not bypass GPUI context construction for entities.
- Keep foreground UI updates and background work separated with GPUI spawn/background patterns.
- Use `Global`, `cx.global::<T>()`, and `cx.update_global(...)` for app-wide shared state.
- Keep runtime assets and package-time assets separate. Use `gpui-app-icon-usage` for app icons, Lucide declarations, and bundle icon resources.
- Keep user-facing text localized through the app's `foundation::i18n` and Fluent files. Use `gpui-i18n` for locale and bundle localization details.

## State Modeling

- Prefer one source of truth. If a state can be represented by one field, do not add a second field that mirrors it.
- Use Rust data structures and type semantics to encode state: `Option<T>` for presence, enum variants with payloads for mutually exclusive states, newtypes for identity, and helper methods for derived predicates.
- Avoid ineffective synchronization data such as `is_loading` plus `load_task`, `selected_id` plus a duplicated selected record, or `status == Running` plus `task: Option<Task<_>>` when the extra field carries no independent meaning.
- Add a separate field only when it represents independent information, a durable business state, or user-visible history that cannot be derived from the existing field.
- For GPUI tasks, store the `Task` to keep it alive or cancel it on drop. If in-flight UI state has no extra semantics, derive loading/disabled guards from `task.is_some()`. Use an underscore prefix only for fields that are intentionally held but never read.
- For new `gpui-store` usage, choose `LocalStore` versus `SharedStore` by ownership and notification boundary. Choose `StoreBackend` separately by synchronization backend. Do not migrate an app to `gpui-store` opportunistically while doing unrelated work.

### Draft, committed state, projection, and ephemeral UI

Keep these layers separate when a screen edits data that also arrives from an external store:

1. committed store/repository state is the durable or shared source;
2. one generated form store owns the current editable typed model and baseline;
3. that form store owns validation report/generations and submit runtime;
4. bound controls project typed form fields and own subscriptions plus interaction-local state;
5. external component options/catalog/capabilities are app-owned configuration;
6. picker open/focus, query, highlight, scroll, preview, IME, and in-flight tasks are component-instance UI state.

Do not mirror a selected id with a selected record, a form value with a component business cache,
or a catalog revision with separately cached rows unless the extra value is explicitly a read-only
projection. Submit through `form.prepare_submit()` so validation, transform, and persistence use the
same typed value. Catalog/options updates only refresh component configuration and never choose a
fallback value or rebase the form. Focus is
owned by the concrete component/page because one data field may be represented by multiple components;
the form may return an error path but must not store a `FocusHandle`. For Jaco, the
durable target is `app/jaco/docs/dev/gpui-form-migration.md`; the form and store crate
contracts remain in their own docs.

## Product UI Guidance

- Build practical desktop app surfaces, not generic landing pages or decorative card grids.
- Use layout, alignment, spacing, type scale, contrast, and restrained motion to establish hierarchy.
- Avoid repeated metadata, excessive gradients, decorative color noise, and duplicating controls already provided by `gpui-component`.
- When borrowing Web, React, Tailwind, or shadcn/ui ideas, translate the intent into GPUI elements and local project patterns.

## Validation

- For code changes, run `cargo fmt` and the most focused app/crate checks available.
- For behavior changes, add or update a relevant test, or explicitly report the test gap.
- For visible UI/layout fixes, validate the actual desktop app with `gpui-computer-use-debugging` when feasible.
