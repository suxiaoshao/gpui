# Trait Rules

Use this file before creating app-local generic traits or builder methods. Many gpui-component concepts are expressed as small reusable traits; custom app UI should usually adopt those names and semantics.

## Core Traits

| Need | Prefer |
| --- | --- |
| Shared component sizes | `Size`, `Sizable` |
| Size-derived text, padding, and row metrics | `StyleSized` |
| Selected/checked/active state | `Selectable` |
| Disabled state | `Disableable` |
| Collapse/expand behavior | `Collapsible` |
| Extra GPUI style helpers | `StyledExt` |
| Prepaint bounds capture | `ElementExt::on_prepaint()` |
| Child elements that inherit index and size | `ChildElement` |

Prefer implementing `Sizable`, `Selectable`, or `Disableable` on app-local components when the component exposes the same concept. This keeps local builders compatible with existing calls such as `.small()`, `.large()`, `.selected(...)`, and `.disabled(...)`.

## Variant Traits

Some components expose variants through local extension traits. Import and use those traits instead of hard-coding visual variants.

| Component family | Trait examples |
| --- | --- |
| Buttons | `ButtonVariants` for `.primary()`, `.secondary()`, `.danger()`, `.ghost()`, `.link()`, `.text()` |
| Toggles | `ToggleVariants` for `.ghost()` and `.outline()` |
| Group boxes | `GroupBoxVariants` for available group styles |

When adding an app-local button-like control, first ask whether the existing `Button`, `Toggle`, or variant trait already expresses the desired state.

## Delegate and Item Traits

Use library delegate traits when building data-backed controls.

| Need | Prefer |
| --- | --- |
| Selectable item display/matching | `SelectItem` |
| Custom select behavior | `SelectDelegate` |
| Virtualized or navigable list behavior | `ListDelegate` |

Avoid creating a parallel app-local delegate abstraction unless the gpui-component delegate cannot express the interaction.

## Import Pattern

Bring the trait into scope where its builder methods are used:

```rust
use gpui_component::{Disableable, Selectable, Sizable, StyledExt};
use gpui_component::button::{Button, ButtonVariants};
```

Missing imports often look like a component API does not exist. Before reimplementing a helper, check whether the trait is simply not in scope.

