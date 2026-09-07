---
name: gpui-app-development
description: Choose app structure, shared ownership, and relevant skills for GPUI workspace development.
---

# GPUI App Development

Follow the app's existing `app`, `foundation`, `features`, and `state` boundaries. Product policy belongs in its app; reusable support belongs in a matching crate such as `window-ext`, `platform-ext`, `app-theme`, or `app-assets`.

Keep one authority per business fact. Derive cheap values rather than caching them; a necessary cache needs clear invalidation. Let a retained task or runtime variant express activity without parallel loading flags.

Applications enter through `gpui_kit::application()` and `gpui_kit::init(cx)`. Dependency paths and versions follow the root manifest.

Use the relevant focused skill:

- `gpui`: contexts, entities, tasks, events, focus and rendering.
- `gpui-component-usage`: controls and app UI composition.
- `gpui-app-icon-usage`, `gpui-i18n`: resources and localized text.
- `gpui-store`, `gpui-operation`, `gpui-form`: implementation or deliberate integration of those crates. Ordinary state, async code or inputs do not require adopting them.
- `gpui-computer-use-debugging`: runtime behavior that needs direct observation.
