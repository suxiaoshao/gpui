---
name: gpui-app-icon-usage
description: Choose or update UI icons, runtime assets, or packaged app icons in GPUI workspace apps.
---

# GPUI App Icon Usage

## UI icons and runtime assets

- Jaco and Feiwen use app-local `IconName` in `app/{name}/src/foundation/assets.rs`; Feiwen declares its set with `app_assets::define_lucide_icons!`.
- Jaco provider logos use `ProviderLogoName` / `ProviderLogoAssets`, `app_assets::define_svg_icons!` and `app/jaco/assets/provider-icons/`.
- HTTP Client and Novel Download register `gpui_kit::assets::Assets`. Use component icons for generic component affordances and app-local variants for app-owned additions.
- Lucide slugs must exist at `third_party/lucide/icons/<slug>.svg`. App-owned SVG sets use `define_svg_icons!`; feature code uses typed names instead of raw paths or scattered `include_bytes!` calls.
- Runtime images belong to the app's `assets/` tree and `with_assets(...)` source. Shared loading helpers belong in `crates/app-assets`.

## Bundle assets

- The base app icon is `app/{name}/build-assets/icon/app-icon.png`.
- xtask owns platform icon lists; do not add `icon = [...]` to `[package.metadata.bundle]`.
- `xtask bundle <app>` derives `.iconset`, `.ico` and Liquid Glass layer PNGs in temporary staging. Keep the base PNG and `.icon/icon.json` in Git, not derived outputs; `Assets/app-icon-liquid-glass.png` is staged from the base PNG.
- Keep packaging resources separate from runtime assets, except when the app intentionally displays its packaged icon.
