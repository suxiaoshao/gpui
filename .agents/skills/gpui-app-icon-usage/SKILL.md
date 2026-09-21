---
name: gpui-app-icon-usage
description: Choose or update UI icons, runtime assets, or packaged app icons in GPUI workspace apps.
---

# GPUI App Icon Usage

## UI icons and runtime assets

- Gupi and Feiwen use `gpui_lucide::IconName` constants. Each embeds its own SVG bytes and converts into the upstream `Icon`; use it directly in `Button::icon`, `Icon::new`, or as an element. No icon selection macro or asset path registration is needed. See `crates/gpui-lucide/README.md`.
- The complete catalog comes from the pinned `gpui-kit-assets` package through its supported Cargo icons-dir metadata. Do not copy SVGs or maintain an application-wide byte lookup table for Lucide icons.
- Custom/provider SVGs use `gpui_lucide::SvgIcon::new(include_bytes!(...))`; keep those files application-owned. Gupi provider logos live under `app/gupi/assets/provider-icons/`.
- Continue registering `gpui_kit::assets::Assets` for the component library's default icons. Gupi additionally supplies its own black/white brand logos. HTTP Client and Novel Download retain their existing default asset registration.
- The old `app-assets`, `app-assets-macros` and `third_party/lucide` are retained only for Jaco, which is no longer maintained. Remove them with Jaco after Gupi is merged; do not migrate Jaco or add new consumers to the old system.
- Runtime images and branded assets remain in the application's `assets/` tree. Shared SVG rendering, sizing and transformations come from the upstream `Icon` implementation.

## Bundle assets

- The base app icon is `app/{name}/build-assets/icon/app-icon.png`.
- xtask owns platform icon lists; do not add `icon = [...]` to `[package.metadata.bundle]`.
- `xtask bundle <app>` derives `.iconset`, `.ico` and Liquid Glass layer PNGs in temporary staging. Keep the base PNG and `.icon/icon.json` in Git, not derived outputs; `Assets/app-icon-liquid-glass.png` is staged from the base PNG.
- Keep packaging resources separate from runtime assets, except when the app intentionally displays its packaged icon.
