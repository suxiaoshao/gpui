---
name: gpui-app-development
description: GPUI application development conventions for this gpui workspace. Use when implementing, refactoring, reviewing, or debugging app code under app/ or shared app support crates, especially when deciding module placement, Render patterns, entity creation, resources, cross-app sharing, validation, or which GPUI-focused skill to load next.
---

# GPUI App Development

Use this skill for workspace-specific GPUI app decisions. Use lower-level GPUI skills for API details and `gpui-component-usage` for component selection.

## Workflow

1. Identify the app or shared crate affected by the change.
2. Follow the existing module and view patterns in that app before introducing new structure.
3. Use `gpui-component-usage` before building custom controls.
4. Use `gpui-app-icon-usage` when changing UI icons, runtime assets, or bundle app icons.
5. Load specific GPUI API skills only when needed:
   - Actions and keybindings: `gpui-action`
   - Async tasks: `gpui-async`
   - Context and windows: `gpui-context`
   - Entity state: `gpui-entity`
   - Events/subscriptions: `gpui-event`
   - Focus and keyboard navigation: `gpui-focus-handle`
   - Global state: `gpui-global`
   - Layout and styling: `gpui-layout-and-style`
   - Tests: `gpui-test`
6. Keep app behavior tied to local validation: run focused tests/checks for the touched app and use `gpui-computer-use-debugging` when runtime UI behavior matters.

## Workspace Rules

- App entrypoints live under `app/{name}/src/main.rs`; preserve each app's existing `app`, `foundation`, `features`, and state boundaries.
- Put reusable cross-app behavior in shared crates such as `crates/window-ext`, `crates/platform-ext`, `crates/app-theme`, or `crates/app-assets` instead of copying it across apps.
- Implement views with `Render` or the app's existing pattern. Use lower-level `Element` only when high-level rendering cannot express the behavior.
- Create context-managed state with `cx.new(...)`; do not bypass GPUI context construction for entities.
- Keep foreground UI updates and background work separated with GPUI spawn/background patterns.
- Use `Global`, `cx.global::<T>()`, and `cx.update_global(...)` for app-wide shared state.
- Keep runtime assets and package-time assets separate. Use `gpui-app-icon-usage` for app icons, Lucide declarations, and bundle icon resources.

## Product UI Guidance

- Build practical desktop app surfaces, not generic landing pages or decorative card grids.
- Use layout, alignment, spacing, type scale, contrast, and restrained motion to establish hierarchy.
- Avoid repeated metadata, excessive gradients, decorative color noise, and duplicating controls already provided by `gpui-component`.
- When borrowing Web, React, Tailwind, or shadcn/ui ideas, translate the intent into GPUI elements and local project patterns.

## Validation

- For code changes, run `cargo fmt` and the most focused app/crate checks available.
- For behavior changes, add or update a relevant test, or explicitly report the test gap.
- For visible UI/layout fixes, validate the actual desktop app with `gpui-computer-use-debugging` when feasible.
