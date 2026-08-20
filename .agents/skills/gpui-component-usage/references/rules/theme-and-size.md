# Theme and Size Rules

Use this file when translating visual specs or building app-local controls.

## Theme Tokens

- Prefer `cx.theme()` via `ActiveTheme` over hard-coded colors.
- Use semantic tokens before raw palette values: foreground/background, primary/secondary, muted, border, ring, popover, tab, titlebar, danger, warning, success, and info.
- Use `Theme.tokens.<role>` for renderable backgrounds so JSON-theme gradients
  survive. If opacity is required, apply it to
  `Theme.tokens.<role>.background.opacity(...)`; calling color methods through
  `ThemeToken`'s representative `Hsla` discards the gradient.
- Continue using semantic `Hsla` fields for text, icons, borders, caret,
  low-level paint quads, and color calculations that cannot accept a
  `Background`.
- `Theme::change(...)` projects the selected component theme into
  `gpui_base::Theme`. After mutating public fields through
  `Theme::global_mut(cx)`, call `Theme::sync_base(cx)` so base-owned
  scrollbars, resize handles, and semantic tokens observe the same theme.
  Do not write a competing base theme in a full-component app because the next
  component-theme sync replaces it.
- Prefer component variants such as `.primary()`, `.secondary()`, `.danger()`, `.ghost()`, `.outline()`, or `.link()` before custom colors.
- Keep custom overrides local to the feature. If many screens need the same override, look for an existing token or component variant first.

## Editor and Markdown Code Colors

- Generated editor chrome and syntax palettes belong to the shared theme layer,
  not to each application.
- Input editors and rendered Markdown code blocks share the installed
  `HighlightThemeStyle` content palette. Their surfaces may differ because an
  editor and an inline Markdown code block are different contexts.
- At target `5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3`, rendered `CodeBlock`
  styles follow the current `HighlightTheme`. Its cache retains parsed
  highlighter state but recomputes styles when the palette changes. Do not add
  app-side theme subscriptions, same-value `set_text`, forced reparsing, or a
  second syntax palette.

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
