# Primitive Ownership and Composition

Use the target `crates/base/src/lib.rs` exports and `website/base/primitives`
pages as the catalog. Base includes behavior primitives for actions, overlays,
controlled controls, tables/tabs/trees, scrolling/resizing, virtual lists,
motion, theme tokens, and text selection. It intentionally does not include
the complete styled `Command`, app menu bar, titlebar, notification facade, or
component assets.

## Selection Rules

- Use a base primitive when it owns the interaction state you need and the app
  is prepared to supply every visual treatment.
- Keep `ElementId` values stable across renders. Do not key reorderable or
  filtered rows by their current index.
- For controlled primitives such as Checkbox, Switch, Radio, and Toggle, keep
  the authoritative value in the app and feed the callback's next value back
  on the next render.
- For stateful editors and other Entity-backed behavior, retain the Entity for
  the semantic lifetime of the control; do not reconstruct it in `render`.
- Prefer exported base parts and styles over copying their event/focus logic
  into a custom element. App styling may wrap or compose those parts without
  becoming a second interaction owner.

## Theme and Presentation

Base controls do not provide product padding, colors, radius, typography, or
focus visuals. Use ordinary GPUI styling and the app's semantic tokens. Keep
theme mutation and base projection atomic from the app's perspective so a
render cannot observe a mixed palette.

Full-component examples are useful for behavior evidence, but their
`ActiveTheme`, `Root`, `IconName`, `TitleBar`, or styled slots are not available
to a base-only app unless independently provided by the app.
