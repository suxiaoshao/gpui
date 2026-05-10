---
name: gpui-app-icon-usage
description: Use when adding, changing, reviewing, or debugging icons and app assets in gpui workspace apps. Covers app-local Lucide IconName enums, gpui-component icon fallback, runtime assets, and bundle app icons under app/*/build-assets/icon.
---

# GPUI App Icon Usage

Use this skill for icon and asset decisions in apps under `app/`. Use `gpui-component-usage` for component selection and this skill for choosing where icon assets live.

## Workflow

1. Identify whether the request is about a UI control icon, a runtime image/resource, or the packaged app icon.
2. Follow the existing asset pattern for the affected app before introducing a new one.
3. For Lucide icons, verify the slug exists under `third_party/lucide/icons/<slug>.svg`.
4. Keep feature code using typed icon names or component icons; do not scatter raw SVG paths or `include_bytes!` calls through feature modules.
5. Keep runtime assets and bundle assets separate.

## UI Icons

- `ai-chat`: use the app-local `IconName` in `app/ai-chat/src/foundation/assets.rs`. Add missing Lucide variants there before using them from UI code.
- `feiwen`: use the app-local `IconName` in `app/feiwen/src/foundation/assets.rs`, declared with `app_assets::define_lucide_icons!`.
- `http-client` and `novel-download`: these currently register `gpui_component_assets::Assets`; default to `gpui-component` icons unless the app needs a deliberate app-local Lucide set.
- When an app has both component icons and app-local icons, prefer app-local icons for app-owned Lucide additions and existing component icons for generic component-provided affordances.

## Runtime Assets

- Runtime assets are loaded by the app's `with_assets(...)` source and should live with that app's runtime asset tree or typed asset wrapper.
- Do not put runtime UI images into `build-assets/`; that directory is for packaging resources.
- Reusable cross-app asset loading helpers belong in shared crates such as `crates/app-assets`.

## Bundle Icons

- Packaged app icons use `app/{name}/build-assets/icon/app-icon.png` as the base image.
- Do not add `icon = [...]` entries to `[package.metadata.bundle]`; xtask owns the platform icon list.
- `xtask bundle <app>` derives platform icon outputs such as `.iconset`, `.ico`, and Liquid Glass layer PNGs in temporary staging; do not commit or manually maintain those derived files.
- Liquid Glass source directories keep `icon.json` in git, while the layer referenced by `Assets/app-icon-liquid-glass.png` is staged from the base `app-icon.png` during bundle.
- Keep app icons out of runtime `assets/` unless the app intentionally displays its own packaged icon at runtime.

## Review Checklist

- The app's current asset source is checked before adding a new pattern.
- Lucide additions use a declared `IconName` variant and an existing Lucide slug.
- Generic component affordances use `gpui-component` icons when no app-local icon set exists.
- Bundle app icons keep only `app-icon.png` and `.icon/icon.json` in git.
- Runtime assets and package-time `build-assets/icon` resources are not mixed.
