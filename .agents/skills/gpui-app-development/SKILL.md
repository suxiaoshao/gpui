---
name: gpui-app-development
description: Choose app structure, shared ownership, and relevant skills for GPUI workspace development.
---

# GPUI App Development

Choose boundaries by capability and ownership. `app/{name}/src` owns startup, windows, navigation and feature composition. App-specific feature crates live in `app/{name}/crates`; cross-app support lives in root `crates/`. Keep each capability's models, services, views, dialogs and tests together. A new crate should express a useful dependency boundary, not merely a technical role or file-size limit.

Feiwen query and fetch depend on feiwen-data; query emits navigation intents and receives a summary through the shell. HTTP request and response depend on http-client-core and have independent entities; the shell coordinates response events and save activity. Novel Download keeps one download feature with a backend interface. Features do not depend on shell types or another feature's private runtime.

Persistent immutable UI text uses SharedString at its owning boundary; edit buffers and pure transport data retain their appropriate types. Keep one result collection and separate display indexes when sorting; preserve domain row identity and selection. Fixed UI geometry uses rem/semantic helpers; native coordinates, measured and media geometry retain pixels. Recompute size-dependent projections when their font/rem/viewport inputs change.

Keep behavior-bearing fields private. Explicit data records may expose fields with an appropriate constructor and non-exhaustive contract; follow the Coding Guides rather than interpreting all public fields as forbidden.

Keep one authority per business fact. Derive cheap values rather than caching them; a necessary cache needs clear invalidation. Let a retained task or runtime variant express activity without parallel loading flags.

Applications enter through `gpui_kit::application()` and `gpui_kit::init(cx)`. Dependency paths and versions follow the root manifest.

Use the relevant focused skill:

- `gpui-kit`: framework APIs, components, state ownership and coding guides.
- `gpui-kit-design-guides`: component composition and visible UI design.
- `gpui-app-icon-usage`, `gpui-i18n`: resources and localized text.
- `gpui-store`, `gpui-operation`, `gpui-form`: implementation or deliberate integration of those crates. Ordinary state, async code or inputs do not require adopting them.
- `gpui-computer-use-debugging`: runtime behavior that needs direct observation.

## Workspace integration

Applications import `gpui_kit` and `gpui_kit::component`; shared crate aliases
follow the root manifest. Official skills may describe newer APIs: verify the
locked dependency source before using them.

- Shared `app-theme` owns editor and Markdown syntax colors. Do not force reparsing
  or create another app-specific syntax palette just to update colors.
- For gradient-capable surfaces use `Theme.tokens.<role>.background`; use semantic
  `Hsla` fields for text, icons, borders and low-level painting.
- Form/domain owns business values, catalog owns available options, and native
  component entities own focus, query, scroll and popup state. Replacing options
  must not become user input or choose a business fallback. After replacing a
  Combobox delegate, project the authoritative selection explicitly if required.
- Use existing gpui-form bindings for source suppression and deferred writes.
  Do not duplicate their feedback-loop routing in app render callbacks.

## Official skill ownership

`gpui-kit` and `gpui-kit-design-guides` are upstream copies managed by npx skills and root `skills-lock.json`. Update only the selected skills with `npx skills add longbridge/gpui-kit --skill gpui-kit gpui-kit-design-guides --agent codex --copy --yes`; do not patch their contents. Put project-specific rules in owned skills. When a summary differs from the website, verify the website and locked crate source. The installed gpui-kit summary's blanket public-field prohibition is narrower than the Coding Guides' record exception.
