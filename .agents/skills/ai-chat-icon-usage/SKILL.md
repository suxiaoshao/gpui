---
name: ai-chat-icon-usage
description: Use when adding or changing icons in the ai-chat app. App UI code must use crate::assets::IconName. Always check app/ai-chat/src/assets.rs first; if the icon is missing, add the Lucide icon there before using it from feature code.
---

# ai-chat icon usage

`app/ai-chat/src/assets.rs` is the app-level icon source of truth.

## Rules

- In `ai-chat` app UI code, prefer `crate::assets::IconName`.
- Do not introduce direct `gpui_component::IconName` usage in `app/ai-chat/src`.
- Before adding a new icon, inspect `app/ai-chat/src/assets.rs`.
- If the icon is missing, add it to `define_icon_assets!` and then use the new variant from app code.
- Stored conversation and template icons are user-facing emoji strings; keep rendering those as text unless a task explicitly changes the persisted icon model.

## Workflow

1. Check whether `crate::assets::IconName` already exposes the icon you need.
2. If it exists, use it directly.
3. If it does not exist, find the matching Lucide SVG slug and add it to `define_icon_assets!` in `app/ai-chat/src/assets.rs`.
4. After adding it to `assets.rs`, use the new `IconName` variant from the calling code.

## Example

```rust
define_icon_assets!(
    Search => "search",
    Send => "send",
    X => "x",
);

Icon::new(crate::assets::IconName::Send)
```

## Review Checklist

- Final app code uses `crate::assets::IconName` for UI controls.
- No unnecessary direct app-level use of `gpui_component::IconName` was added.
- Any missing Lucide icon was first added in `app/ai-chat/src/assets.rs`.
- The chosen variant name and Lucide slug match.
