# Architecture and Dependency Boundary

The authority for this workspace upgrade is
`longbridge/gpui-component@5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3`.
Its `gpui-base 0.5.2` crate is unpublished and must come from the same Git
source as the complete component library. That target in turn pins Zed GPUI to
`e0931d5a9dbf4f781b336fdf448739e74a2ac0b5`.

## Dependency Direction

```text
base-only app
├─ gpui
├─ gpui_platform
└─ gpui-base

gpui-component -> gpui-base
```

A base-only app does not depend on the right-hand complete layer. Check shared
theme, asset, and helper crates as well as direct manifest entries; a transitive
component edge violates the same boundary.

## Initialization

Call `gpui_base::init(cx)` once during application startup before windows are
opened. It installs base theme state and shared behavior infrastructure for
dialogs, focus traps, popovers, sheets, comboboxes, selects, inputs, and trees.
It does not install a ready-made root view, titlebar, design theme, or app icon.

If an application uses full `gpui-component`, that library initializes base
internally. Do not add a second direct base dependency or duplicate init call
to existing full-component apps merely because base appears in their
transitive graph.

## Application Ownership

The base-only app owns:

- semantic theme tokens and their projection into base theme state;
- spacing, radius, type, control sizing, focus visuals, and motion policy;
- window root, titlebar/tabs, overlay composition, icons, and assets;
- domain values, commands, persistence, errors, and notifications;
- accessibility semantics not explicitly provided by the selected primitive.

Before sharing any of those across apps, prove a second consumer and ensure the
shared crate does not reintroduce `gpui-component`.
