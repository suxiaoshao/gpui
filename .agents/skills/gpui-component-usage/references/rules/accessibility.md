# Accessibility Rules

Use this reference when a component is interactive, renders a custom child, or
is expected to work with VoiceOver, NVDA, or AT-SPI.

## Responsibility Layers

- GPUI creates the platform AccessKit adapter for ordinary native windows.
- Each gpui-component control must explicitly expose its role, accessible name,
  state, value, and actions where applicable.
- The application still owns semantics for custom rows, app-drawn surfaces,
  icon-only actions, domain errors, and focus transitions.

An upgraded GPUI or a call to `gpui_component::init` does not make arbitrary
application elements accessible. Inspect the target component source before
claiming coverage.

## Accessible Names and Custom Content

- Every interactive control needs a programmatic name. A tooltip is help text,
  not a substitute for an accessible name.
- When a component's public builder cannot supply the needed name or state,
  do not hide the gap behind a styled `div`; add an app-level semantic wrapper
  only if the target GPUI API supports it, otherwise report or fix the upstream
  component contract.
- Lazy custom row factories may run more than once. Keep rendering
  side-effect-free and ensure the row's visible text is also represented in the
  accessibility tree.

## Command Palettes

Target `Command` exposes `ListBox` and `ListBoxOption` roles, but source audit
does not establish a complete active-descendant or accessible-name contract for
every custom/default row. Treat this as partial infrastructure, not completion.

For a migrated palette, validate at minimum:

1. the search field has a useful accessible name;
2. Up/Down changes are announced while focus remains on the search field;
3. disabled items are conveyed and skipped;
4. loading, empty, error, and result-count changes are understandable;
5. Escape clearing versus dismissal and focus return match the product flow.

Use focused GPUI tests for roles/actions that the API exposes, then perform a
real assistive-technology smoke test on every supported platform that is being
claimed. Do not describe untested platform behavior as supported.
