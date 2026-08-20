# Accessibility Responsibility

GPUI installs the platform AccessKit adapter for ordinary native windows, and
some base primitives expose explicit roles, state, and actions. Neither fact is
a blanket guarantee for every base control or custom application surface.

## Required Audit

For each interactive primitive or app-owned element, inspect the exact target
render implementation and verify:

- stable accessibility identity where automation or relationships require it;
- role, accessible name/description, value, selection, disabled/checked state;
- keyboard-equivalent action and focus order;
- error, loading, and dynamic-result announcements;
- behavior of custom child content and icon-only actions.

Base `Input`, `Textarea`, and `Editor` expose the editing engine, but the target
base wrapper does not by itself establish the complete styled Input's
`TextInput`/label/value/action contract. A base-only app must supply and test
the missing semantics or first improve the upstream base primitive. Do not
depend on a tooltip as the accessible name.

## Custom Renderers

A terminal, canvas, grid, or other custom `Element` does not gain semantics
from `gpui_base::init`. The app owns the semantic root, readable content,
selection/caret state, actions, and update granularity. Use target GPUI roles
only after verifying their platform behavior, and validate with the native
accessibility inspector plus VoiceOver, NVDA, or AT-SPI for every platform you
claim.

Component roles are useful evidence, but compilation and an AccessKit tree
snapshot do not replace keyboard and screen-reader smoke tests.
