# State and Interaction Rules

Use this file when building custom interactions or wiring component state.

## Common State Builders

- Use `Selectable::selected(...)` for selected, checked, or active item state when the component implements it.
- Use `Disableable::disabled(...)` for disabled controls.
- Use component-specific loading APIs, such as `Button::loading(...)`, before rendering custom spinners around controls.
- Keep state names aligned with component semantics: selected, disabled, loading, checked, collapsed, active.

## Focus and Keyboard Behavior

- Preserve `FocusHandle`, `Focusable`, focus traps, and existing key bindings when composing inputs, dialogs, popovers, and menus.
- Use `FocusTrap` for modal or trapped-focus surfaces before writing custom escape/tab handling.
- Do not make a visual port that breaks keyboard navigation.

## Pointer and Window Behavior

- Buttons and desktop controls normally keep desktop cursor behavior unless the component is link-like.
- For titlebar or draggable regions, inspect `TitleBar` behavior first: drag area, double-click zoom, platform system buttons, and Linux window menu are platform-specific.
- In interactive child regions inside draggable areas, stop propagation intentionally so clicks do not start window drag.

## Overlays and Menus

- Use `Dialog`, `AlertDialog`, `Sheet`, `Popover`, `HoverCard`, `Tooltip`, and menu components before manually positioning floating content.
- Use `DropdownMenu` and `ContextMenuExt` for menu-like interactions.
- If overlay positioning or animation depends on bounds, use `ElementExt::on_prepaint()` rather than ad hoc state updates.

## Delegated Data Controls

- For select/list-like controls, prefer `SelectDelegate`, `SelectItem`, or `ListDelegate`.
- Keep filtering, selection, disabled state, and rendering in the delegate shape when possible; avoid creating a second app-local list framework.

