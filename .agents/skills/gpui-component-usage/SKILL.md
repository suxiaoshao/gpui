---
name: gpui-component-usage
description: Use gpui-component components in GPUI applications. Use when building, refactoring, reviewing, or debugging app UI that should consume existing gpui-component controls, compose component-library primitives, choose between available components, or translate shadcn/ui-inspired patterns into GPUI desktop UI. This is for consuming gpui-component from apps, not for contributing new components to gpui-component itself.
---

# GPUI Component Usage

## Workflow

1. Identify the UI job: action, form input, overlay, layout, navigation, feedback, data display, or advanced interaction.
2. Read `references/components/index.md` and choose the closest existing gpui-component component.
3. Read only the specific component reference files needed for the task.
4. Before implementing, inspect the chosen component's docs, story/demo usage, and source API when available.
5. If the UI needs custom app-local composition, read the relevant rules first:
   `references/rules/traits.md`, `references/rules/primitives.md`, `references/rules/theme-and-size.md`, or `references/rules/state-and-interaction.md`.
6. Compose existing components, traits, helpers, theme tokens, and delegate patterns before writing custom app-local UI.
7. If the bundled reference conflicts with the current dependency, inspect the app's actual `gpui-component` API from Cargo sources, docs.rs for the exact version, or the checked-out dependency source.
8. If a component reference is missing or stale, use the upstream Markdown docs fallback: `https://longbridge.github.io/gpui-component/docs/components/{component}.md` or the full `https://longbridge.github.io/gpui-component/llms-full.txt`.

## Principles

- Use `gpui-component` first for common controls such as buttons, inputs, selectors, dialogs, sheets, popovers, tables, scrollable areas, feedback, and navigation.
- If no component fits, implement only the app-local gap; do not copy or re-create generic controls already owned by gpui-component.
- Treat GPUI style methods as the desktop Rust analogue of Tailwind-style composition: build layout and visual hierarchy with small chained style methods, but use GPUI APIs and existing project patterns.
- Treat shadcn/ui as a design and composition reference for gpui-component behavior, visual states, sizing, and component structure.
- Keep desktop differences: buttons normally use the default cursor, keyboard focus matters, overlays belong to the GPUI window/root model, and macOS/Windows control expectations outrank Web DOM habits.
- Do not copy React, DOM, CSS, or Tailwind code into GPUI. Translate the intent into GPUI elements, `gpui-component` primitives, and the app's existing view patterns.
- If a shadcn component is not available in gpui-component, prefer composing available gpui-component pieces before creating app-local generic controls.
- If a need is just size, selected state, disabled state, variant, styling, theme tokens, overlay behavior, or list/select delegation, prefer existing gpui-component traits and helpers over new app-local generic abstractions.
- Treat bundled component docs as a portable snapshot. When precision matters, verify against the current checkout, Cargo dependency source, story/demo code, or docs.rs for the version in use.
- A component `State` may physically contain a selected/text value, dynamic
  options/delegate data, and focus/query/scroll state. Treat these as separate
  ownership channels: form/domain owns the value, app/catalog owns options, and
  the component owns interaction. Never make the whole state the submit source.

## Component Selection

| Need | Prefer |
| --- | --- |
| Primary, secondary, danger, icon, or loading action | `Button`, `ButtonGroup`, `Toggle` |
| Text entry | `Input`, `Editor`, `NumberInput`, `OtpInput` |
| Formatted text, Markdown, or HTML rendering | `TextView` |
| Boolean or option selection | `Checkbox`, `Switch`, `Radio`, `Select`, `Combobox`, `Slider`, `ColorPicker`, `DatePicker` |
| Forms and settings | `Form`, `Settings`, `GroupBox`, `Label`, `DescriptionList` |
| Modal or confirmation flow | `Dialog`, `AlertDialog` |
| Non-modal panel or floating content | `Sheet`, `Popover`, `HoverCard`, `Tooltip` |
| Menu actions | `Menu`, `DropdownButton` |
| Feedback and status | `Alert`, `Notification`, `Progress`, `Spinner`, `Skeleton`, `Badge`, `Tag`, `StatusBar` |
| Data display | `Table`, `DataTable`, `List`, `Tree`, `VirtualList`, `Chart`, `Plot` |
| Navigation and structure | `Sidebar`, `Tabs`, `Pagination`, `Accordion`, `Collapsible`, `Resizable`, `Scrollable` |
| Media and affordances | `Icon`, `Image`, `Avatar`, `Kbd`, `TitleBar` |

## References

- Component index: `references/components/index.md`
- Composition rules: `references/rules/composition.md`
- Styling rules: `references/rules/styling.md`
- Trait and extension rules: `references/rules/traits.md`
- Library primitives and helpers: `references/rules/primitives.md`
- Theme and size rules: `references/rules/theme-and-size.md`
- State and interaction rules: `references/rules/state-and-interaction.md`
- Component docs: `references/components/<component>.md`
- Online docs fallback: `https://longbridge.github.io/gpui-component/docs/components/{component}.md`
- Full online docs fallback: `https://longbridge.github.io/gpui-component/llms-full.txt`
- Third-party attribution: `references/third-party/gpui-component-docs.md`

The component docs are an English snapshot of gpui-component's Markdown documentation. They are bundled for portability and progressive loading; do not load every component file unless the task genuinely requires it.
