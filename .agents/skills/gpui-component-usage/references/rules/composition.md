# Composition Rules

Use this file when choosing how to assemble app UI from gpui-component pieces.

## Use Existing Components First

- Use `Button` for actions before styling a raw `div` as a button.
- Use `Input`, `Select`, `Checkbox`, `Radio`, `Switch`, `Slider`, `DatePicker`, `ColorPicker`, or `OtpInput` for form controls before custom input markup.
- Use `Dialog`, `AlertDialog`, `Sheet`, `Popover`, `HoverCard`, and `Tooltip` for overlay behavior before manually positioning floating containers.
- Use `Table`, `DataTable`, `List`, `Tree`, and `VirtualList` for structured data instead of app-local list/table frameworks.
- Use `Alert`, `Notification`, `Progress`, `Spinner`, `Skeleton`, `Badge`, and `Tag` for status and feedback.

## Compose by Role

- Settings and preference pages usually compose `Settings`, `Form`, `GroupBox`, `Label`, `DescriptionList`, and existing form controls.
- Command or menu surfaces usually compose `Button`, `DropdownButton`, `Menu`, `Popover`, and keyboard focus patterns already used by the app.
- Empty or lightweight card-like areas should start with ordinary GPUI layout plus `GroupBox`, `Alert`, `Skeleton`, `Icon`, `Button`, or `Tag` as needed. Do not introduce a generic app-local card component only because a Web example uses Card.
- Data-heavy views should choose `DataTable`, `Table`, `List`, `Tree`, or `VirtualList` based on size and interaction needs.

## Translate shadcn Patterns, Do Not Copy Them

gpui-component intentionally follows many shadcn/ui component ideas, but app code should translate the design intent into GPUI:

- shadcn variants map to gpui-component builder methods such as `primary()`, `danger()`, `ghost()`, `outline()`, `small()`, and `large()` when available.
- shadcn composition maps to nested GPUI elements and component builders, not React child components or `className`.
- shadcn state styling maps to GPUI state, builder flags, and theme colors, not CSS selectors.
- shadcn overlay primitives map to gpui-component overlay components that participate in GPUI window/root behavior.

## Avoid App-Local Generic Controls

Only build app-local UI when no gpui-component primitive covers the interaction. If custom UI is necessary, keep it feature-specific and compose gpui-component parts around it. Do not create reusable generic controls in the app that duplicate gpui-component ownership.
