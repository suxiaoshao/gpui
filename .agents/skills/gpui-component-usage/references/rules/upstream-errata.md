# Upstream Documentation Errata

These corrections apply to the vendored documentation snapshot from
`longbridge/gpui-component@5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3`.
The copied documents remain unchanged; target source and tests are authoritative
when a row below conflicts with them.

## TitleBar AppMenuBar Example

`references/components/title-bar.md` uses a two-argument `AppMenuBar`
constructor. The target source at
`crates/ui/src/menu/app_menu_bar.rs` exposes:

```rust
pub fn new(cx: &mut App) -> Entity<AppMenuBar>;
```

Create and retain that Entity in the owning view, then render the Entity in the
title bar. Do not copy the menu bar's traversal or popup state into an app-local
fork.

Both `TitleBar::title_bar_options()` and `TitleBar::window_options()` are valid.
When constructing an entire `WindowOptions`, prefer `window_options()` so the
titlebar and `app_owns_titlebar_drag` requirements stay paired.

## Command External Filtering

`references/components/command.md` describes application/remote search but its
method table omits the target source method:

```rust
pub fn filterable(self, filterable: bool) -> Self;
```

Use `.filterable(false)` when an application, database, or remote service owns
matching. `.searchable(false)` removes the query UI and is not an equivalent
way to disable the second local label/keyword filter.

## Select Combobox Link

`references/components/select.md` links to an extensionless `combobox` target.
That link is preserved from the target documentation snapshot but does not
resolve as a local Markdown file. Follow the repository-authored component
index or open `references/components/combobox.md` directly.
