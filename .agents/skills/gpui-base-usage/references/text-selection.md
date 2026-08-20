# Text Selection for Custom Renderers

Use `gpui-base` text selection when the app paints selectable text itself with
`StyledText`, `TextLayout`, a virtualized document, or a custom `Element`.

## Ownership

Base owns window gesture coordination, Shift/multi-click behavior, selection
range projection, scopes, copy orchestration, and auto-scroll events. The app
still owns source text, parser/grid/document identity, layout, hitboxes, scroll
coordinates, highlight painting, virtualized copy, and selection policy.

## Required Lifetime

1. Mount one `TextSelectionLayer` as the first child of the custom window root.
2. Retain one stable `TextSelectionHandle` per semantic selectable participant.
3. Register current hitbox, bounds, scroll offset, scope, and document order in
   prepaint through `TextSelectionRegistration`.
4. During paint, pass the exact laid-out UTF-8 text and `TextLayout` values as
   `TextSelectionRun`s to `update_runs`.
5. Paint returned ranges behind glyphs and retain any refresh/event
   subscription for the participant lifetime.

Do not create handles each frame or derive semantic document order from a
`HashMap`, current viewport index, or accidental paint order. Handles not
registered in the current frame stop participating.

For a terminal, base selection may own gestures and projection, but the
terminal grid/parser remains authoritative for cell, block, wrapped-line,
scrollback, and copied-text semantics. Use an adapter spike and tests before
replacing terminal-engine selection behavior.
