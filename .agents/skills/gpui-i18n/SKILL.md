---
name: gpui-i18n
description: Use when adding, changing, reviewing, or debugging user-facing text, Fluent locale files, language selection, or macOS bundle localization in gpui workspace apps.
---

# GPUI I18n

Use this skill for app-local localization work under `app/`.

## Current Pattern

- App runtime text lives in Fluent files:
  - `app/{name}/locales/en-US/main.ftl`
  - `app/{name}/locales/zh-CN/main.ftl`
- macOS bundle strings live in:
  - `app/{name}/locales/macos/en-US.lproj/InfoPlist.strings`
  - `app/{name}/locales/macos/zh-Hans.lproj/InfoPlist.strings`
- Each app owns an app-local `foundation::i18n` module that builds `FluentBundle`s and installs an `I18n` global.
- UI code reads localized text through the app's `I18n`, typically with `cx.global::<I18n>().t(...)` or `t_with_args(...)`.
- Apps that support language settings rebuild the `I18n` global after the language changes.

## Workflow

1. Identify the affected app and inspect its existing `foundation/i18n.rs`.
2. Add or update keys in both `en-US/main.ftl` and `zh-CN/main.ftl`.
3. Use existing naming style for keys in that app.
4. Use `FluentArgs` and `t_with_args` for interpolated values.
5. Keep user-facing strings out of Rust code unless they are debug-only, test-only, or intentionally not localized.
6. Update tests that assert required localization keys when the app already has key coverage for that feature surface.

## Fluent Rules

- Prefer semantic keys such as `dialog-delete-message-title` over text-shaped keys.
- Do not build localized sentences with `format!` around translated fragments when grammar may differ by language.
- Keep placeholders explicit and stable, for example `{ $name }` or `{ $path }`.
- Missing keys intentionally fall back to the key string in current app implementations; do not rely on that fallback for shipped UI.

## macOS Bundle Localization

- `crates/xtask/src/bundle/settings.rs` maps app `locales/macos` resources into bundle `.lproj` directories.
- `crates/xtask/src/bundle/macos.rs` sets `CFBundleAllowMixedLocalizations` and `CFBundleLocalizations`.
- Do not add manual bundle localization resources outside the existing `locales/macos` tree.
- If a new app is added, it needs both runtime Fluent locale files and macOS `InfoPlist.strings` files so `xtask bundle <app>` can package it.

## Validation

For i18n changes, run the most focused affected app tests/checks, for example:

```sh
cargo fmt
cargo test -p jaco i18n
cargo check -p jaco
git diff --check
```

If bundle localization logic changes, also run focused `xtask` tests.
