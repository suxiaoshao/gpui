---
name: gpui-component-usage
description: Use gpui-component components in GPUI applications. Use when building, refactoring, reviewing, or debugging app UI that should consume existing gpui-component controls, compose component-library primitives, choose between available components, or translate shadcn/ui-inspired patterns into GPUI desktop UI. This is for consuming gpui-component from apps, not for contributing new components to gpui-component itself.
---

# GPUI Component Usage

## Workflow

1. Identify the UI job: action, form input, overlay, layout, navigation, feedback, data display, or advanced interaction.
2. Read `references/components/index.md` and choose the closest existing gpui-component component.
3. Read only the specific component reference files needed for the task.
4. Compose existing components before writing custom app-local UI.
5. If the bundled reference conflicts with the current dependency, inspect the app's actual `gpui-component` API from Cargo sources, docs.rs for the exact version, or the checked-out dependency source.

## Principles

- Use `gpui-component` first for common controls such as buttons, inputs, selectors, dialogs, sheets, popovers, tables, scrollable areas, feedback, and navigation.
- Treat GPUI style methods as the desktop Rust analogue of Tailwind-style composition: build layout and visual hierarchy with small chained style methods, but use GPUI APIs and existing project patterns.
- Treat shadcn/ui as a design and composition reference for gpui-component behavior, visual states, sizing, and component structure.
- Keep desktop differences: buttons normally use the default cursor, keyboard focus matters, overlays belong to the GPUI window/root model, and macOS/Windows control expectations outrank Web DOM habits.
- Do not copy React, DOM, CSS, or Tailwind code into GPUI. Translate the intent into GPUI elements, `gpui-component` primitives, and the app's existing view patterns.
- If a shadcn component is not available in gpui-component, prefer composing available gpui-component pieces before creating app-local generic controls.

## Component Selection

| Need | Prefer |
| --- | --- |
| Primary, secondary, danger, icon, or loading action | `Button`, `ButtonGroup`, `Toggle` |
| Text entry | `Input`, `Editor`, `NumberInput`, `OtpInput` |
| Boolean or option selection | `Checkbox`, `Switch`, `Radio`, `Select`, `Slider`, `ColorPicker`, `DatePicker` |
| Forms and settings | `Form`, `Settings`, `GroupBox`, `Label`, `DescriptionList` |
| Modal or confirmation flow | `Dialog`, `AlertDialog` |
| Non-modal panel or floating content | `Sheet`, `Popover`, `HoverCard`, `Tooltip` |
| Menu actions | `Menu`, `DropdownButton` |
| Feedback and status | `Alert`, `Notification`, `Progress`, `Spinner`, `Skeleton`, `Badge`, `Tag` |
| Data display | `Table`, `DataTable`, `List`, `Tree`, `VirtualList`, `Chart`, `Plot` |
| Navigation and structure | `Sidebar`, `Tabs`, `Pagination`, `Accordion`, `Collapsible`, `Resizable`, `Scrollable` |
| Media and affordances | `Icon`, `Image`, `Avatar`, `Kbd`, `TitleBar` |

## References

- Component index: `references/components/index.md`
- Composition rules: `references/rules/composition.md`
- Styling rules: `references/rules/styling.md`
- Component docs: `references/components/<component>.md`
- Third-party attribution: `references/third-party/gpui-component-docs.md`

The component docs are an English snapshot of gpui-component's Markdown documentation. They are bundled for portability and progressive loading; do not load every component file unless the task genuinely requires it.
