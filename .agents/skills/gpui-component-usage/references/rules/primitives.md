# Library Primitives and Helpers

Use this file when custom app UI is necessary but should still feel native to gpui-component.

## Layout Helpers

- Use `h_flex()` and `v_flex()` for common row/column containers before adding app-local helper functions.
- Use `StyledExt::paddings()` and `StyledExt::margins()` when applying `Edges` values from components or size helpers.
- Use `StyledExt::refine_style()` when a component exposes `StyleRefinement` and needs caller-provided style composition.

## Element Helpers

- Use `ElementExt::on_prepaint()` when a component needs measured bounds, animated indicators, or overlay positioning.
- Use `ChildElement` / `AnyChildElement` when children must inherit an index and shared size from a parent control.
- Use `InteractiveElementExt` before inventing small event wrappers; it contains component-library interaction extensions.

## Window and Root Helpers

- Use `Root` for app roots that should participate in gpui-component theme and global behavior.
- Use `WindowExt` for window-level helpers exposed by the library.
- For window chrome, inspect `TitleBar` and its compatibility handling before implementing custom titlebar behavior.

## Icons and Media

- Use `Icon` and `IconName` for iconography rather than app-local SVG drawing.
- Prefer component-owned icon sizing through `Sizable` or component builders such as `.small()` and `.large()`.

## Initialization

Application entrypoints should call `gpui_component::init(cx)` once before using components. This initializes theme, global state, root behavior, focus trap, overlays, menus, table, text, tree, tooltip, and other component systems.

When diagnosing missing overlay, tooltip, menu, theme, or focus behavior, check initialization before changing component code.

