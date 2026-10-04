---
name: gpui-i18n
description: Implement or review user-visible text, Fluent locales, language settings, or macOS bundle localization in GPUI workspace apps.
---

# GPUI I18n

## Runtime text

- The shared `crates/app-i18n` owns locale detection, Fluent bundles and the `I18n` runtime. Each app shell registers that global with its own locale resources; feature crates import `app_i18n::I18n`. Form validation message mapping belongs to its feature. UI uses `cx.global::<I18n>().t(...)` or `t_with_args(...)`; language settings rebuild that global where supported.
- Runtime locale files are `app/{name}/locales/{en-US,zh-CN}/main.ftl`. Keep changed keys and interpolation variables aligned across both languages.
- Convert immutable retained presentation text to SharedString at the UI ownership boundary; retain String for editing/formatting and avoid speculative translation caches.
- Use semantic keys and the app's naming convention. Use `FluentArgs` for interpolation; avoid composing sentences from translated fragments with `format!`.
- User-facing Rust literals are reserved for intentionally unlocalized text; debug/test strings are separate. The current missing-key fallback returns the key itself and is not acceptable shipped copy.

## macOS bundle text

- Bundle strings live under `app/{name}/locales/macos/{en-US,zh-Hans}.lproj/InfoPlist.strings`.
- `crates/xtask/src/bundle/settings.rs` maps these resources; `bundle/macos.rs` sets `CFBundleAllowMixedLocalizations` and `CFBundleLocalizations`. New apps need both runtime and bundle locale files.
- Text-only edits use key/variable parity checks. Bundle localization logic changes use affected xtask coverage; text-only edits do not require bundling.
