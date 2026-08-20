# Library Primitives and Helpers

Use this file when custom app UI is necessary but should still feel native to gpui-component.

## Layout Helpers

- Use `h_flex()` and `v_flex()` for common row/column containers before adding app-local helper functions.
- Use `StyledExt::paddings()` and `StyledExt::margins()` when applying `Edges` values from components or size helpers.
- Use `StyledExt::refine_style()` when a component exposes `StyleRefinement` and needs caller-provided style composition.
- Let a `Scrollable` source element own its gap, padding, and size. Keep an
  outer wrapper only when it has a real clipping, overlay, or independent
  layout responsibility.

## Element Helpers

- Use `ElementExt::on_prepaint()` when a component needs measured bounds, animated indicators, or overlay positioning.
- Use `ChildElement` / `AnyChildElement` when children must inherit an index and shared size from a parent control.
- Use `InteractiveElementExt` before inventing small event wrappers; it contains component-library interaction extensions.

## Window and Root Helpers

- Use `Root` for app roots that should participate in gpui-component theme and global behavior.
- Use `WindowExt` for window-level helpers exposed by the library.
- For window chrome, use `TitleBar::window_options()` as the base when the
  component owns the title bar. Use `menu::AppMenuBar` instead of copying its
  menu traversal, focus, and popup behavior; see
  [upstream errata](upstream-errata.md) before following the bundled title-bar
  example.
- Prefer `ListItem` for ordinary selectable rows and `Progress` for determinate
  progress before duplicating their state visuals in app-local elements.

## Icons and Media

- Use `Icon` and `IconName` for iconography rather than app-local SVG drawing.
- Prefer component-owned icon sizing through `Sizable` or component builders such as `.small()` and `.large()`.

## Initialization

Application entrypoints that use the complete library should call
`gpui_component::init(cx)` once before using components. It initializes
`gpui-base` and the styled component systems. A `gpui-base`-only application
instead calls `gpui_base::init(cx)` and must not route through this skill for
presentation components.

When diagnosing missing overlay, tooltip, menu, theme, or focus behavior, check initialization before changing component code.
