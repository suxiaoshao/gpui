# gpui-component Component Index

Use this index to choose the smallest existing component set for an app UI task. Read only the referenced component docs needed for the current work.

## Action and Basic Controls

| Need | Component docs |
| --- | --- |
| Clickable actions, icon buttons, loading actions | [Button](button.md) |
| Button with dropdown actions | [DropdownButton](dropdown_button.md), [Menu](menu.md) |
| Binary pressed state or toolbar option | [Toggle](toggle.md) |
| Grouped disclosure | [Accordion](accordion.md), [Collapsible](collapsible.md) |
| Status labels and counters | [Badge](badge.md), [Tag](tag.md) |
| Keyboard shortcut display | [Kbd](kbd.md) |
| Icons and media | [Icon](icon.md), [Image](image.md), [Avatar](avatar.md) |
| Formatted text, Markdown, or HTML rendering | [TextView](text-view.md) |

## Forms and Inputs

| Need | Component docs |
| --- | --- |
| Form layout and field labels | [Form](form.md), [Label](label.md), [GroupBox](group-box.md) |
| Text input | [Input](input.md) |
| Multiline/code input | [Editor](editor.md) |
| Numeric, one-time-code, date, and color input | [NumberInput](number-input.md), [OtpInput](otp-input.md), [DatePicker](date-picker.md), [ColorPicker](color-picker.md) |
| Simple single choice | [Select](select.md), [Radio](radio.md) |
| Searchable, multi-select, custom trigger, or custom option rendering | [Combobox](combobox.md) |
| Boolean, range, rating, date, or color choices | [Checkbox](checkbox.md), [Switch](switch.md), [Slider](slider.md), [Rating](rating.md), [DatePicker](date-picker.md), [ColorPicker](color-picker.md) |
| Settings pages | [Settings](settings.md), [DescriptionList](description-list.md) |

## Overlays and Feedback

| Need | Component docs |
| --- | --- |
| Modal dialogs and destructive confirmations | [Dialog](dialog.md), [AlertDialog](alert-dialog.md) |
| Side panel or drawer-like content | [Sheet](sheet.md) |
| Anchored floating content | [Popover](popover.md), [HoverCard](hover-card.md), [Tooltip](tooltip.md) |
| Inline alerts and async status | [Alert](alert.md), [Notification](notification.md), [Progress](progress.md), [Spinner](spinner.md), [Skeleton](skeleton.md) |
| Clipboard operations | [Clipboard](clipboard.md) |

## Layout, Navigation, and Data

| Need | Component docs |
| --- | --- |
| App navigation | [Sidebar](sidebar.md), [Tabs](tabs.md), [Pagination](pagination.md) |
| Scroll and resize | [Scrollable](scrollable.md), [Resizable](resizable.md), [VirtualList](virtual-list.md) |
| Data presentation | [Table](table.md), [DataTable](data-table.md), [List](list.md), [Tree](tree.md), [DescriptionList](description-list.md) |
| Charts | [Chart](chart.md), [Plot](plot.md) |
| Calendar views | [Calendar](calendar.md), [DatePicker](date-picker.md) |
| Window chrome | [TitleBar](title-bar.md) |
| Status bars and contextual footer regions | [StatusBar](status-bar.md) |
| Focus management | [FocusTrap](focus-trap.md) |

## Library Primitives and Patterns

| Need | Reference |
| --- | --- |
| Shared traits for size, selected, disabled, variants, and delegates | [Trait rules](../rules/traits.md) |
| Layout helpers, element helpers, icons, root, window helpers, initialization | [Primitive rules](../rules/primitives.md) |
| Theme tokens, semantic colors, component sizes, typography, spacing | [Theme and size rules](../rules/theme-and-size.md) |
| Focus, overlay, menu, pointer, loading, selected, and delegated interactions | [State and interaction rules](../rules/state-and-interaction.md) |

## shadcn/ui Mental Mapping

| shadcn-style need | gpui-component reference |
| --- | --- |
| Button, Toggle, ToggleGroup | [Button](button.md), [Toggle](toggle.md) |
| Input, Textarea, InputOTP, Slider | [Input](input.md), [Editor](editor.md), [OtpInput](otp-input.md), [Slider](slider.md) |
| Checkbox, RadioGroup, Switch, Select, Combobox, Command-like searchable picker | [Checkbox](checkbox.md), [Radio](radio.md), [Switch](switch.md), [Select](select.md), [Combobox](combobox.md) |
| Dialog, AlertDialog, Sheet, Popover, Tooltip, HoverCard | [Dialog](dialog.md), [AlertDialog](alert-dialog.md), [Sheet](sheet.md), [Popover](popover.md), [Tooltip](tooltip.md), [HoverCard](hover-card.md) |
| Alert, Badge, Skeleton, Progress | [Alert](alert.md), [Badge](badge.md), [Skeleton](skeleton.md), [Progress](progress.md) |
| Tabs, Accordion, Collapsible, Sidebar, Pagination | [Tabs](tabs.md), [Accordion](accordion.md), [Collapsible](collapsible.md), [Sidebar](sidebar.md), [Pagination](pagination.md) |
| Table, DataTable, Chart | [Table](table.md), [DataTable](data-table.md), [Chart](chart.md) |
| Card-like grouping | [GroupBox](group-box.md), plain GPUI layout with existing typography and spacing |

If a shadcn component is not listed here, first search this index for the underlying interaction pattern instead of recreating the Web component literally.
