# xtask

## Bundle localization

Apps may declare supported locale tags in the `package.metadata.bundle` table. The optional `localizations` array uses app locale tags such as en-US, zh-CN, zh-TW, ja, ko, de, fr, es, and pt-BR. The bundle command maps these locales to macOS `.lproj` resources and `CFBundleLocalizations`, and to WiX cultures with a matching `build-assets/locales/wix/<culture>.wxl` file. WiX produces one MSI per declared culture.

Apps without the declaration retain the existing macOS English and Simplified Chinese resources and the bundler's English-only MSI default. When `--install` sees localized MSI artifacts, xtask selects the en-US MSI; if it is absent, it falls back to the first MSI artifact.

All platforms resolve build and bundle output from `CARGO_TARGET_DIR` when set; relative paths are resolved from the workspace root. Otherwise output stays under `target/`.
