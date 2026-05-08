# Theme and Size Rules

Use this file when translating visual specs or building app-local controls.

## Theme Tokens

- Prefer `cx.theme()` via `ActiveTheme` over hard-coded colors.
- Use semantic tokens before raw palette values: foreground/background, primary/secondary, muted, border, ring, popover, tab, titlebar, danger, warning, success, and info.
- Prefer component variants such as `.primary()`, `.secondary()`, `.danger()`, `.ghost()`, `.outline()`, or `.link()` before custom colors.
- Keep custom overrides local to the feature. If many screens need the same override, look for an existing token or component variant first.

## Size System

`gpui-component` uses `Size` and `Sizable` to coordinate height, text, padding, icon size, and spacing. Do not only change an outer container height when the component has a built-in size.

Preferred order:

1. Use component builders: `.xsmall()`, `.small()`, `.large()`.
2. Use `with_size(Size::...)` or `with_size(px(...))` when custom size is supported.
3. Use `StyleSized` for raw GPUI elements that must match input, list, table, or button sizing.
4. Only hand-write height/padding/text size when no component size model exists.

Example:

```rust
TabBar::new("routes")
    .underline()
    .large()
    .selected_index(active)
```

This is better than a hand-written tab with `h(px(44.))`: the large underline size also controls text, inner height, margins, gap, and indicator placement.

## Typography and Spacing

- Match text size to the component size. A large control should not keep `text_xs()` unless the design intentionally needs compact metadata.
- Let component spacing win when available; for example, `TabBar` manages underline tab gaps per size.
- Use `cx.theme().radius` and component radius helpers instead of arbitrary rounded values.

