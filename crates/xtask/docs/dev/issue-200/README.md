# xtask：删除 HTTP Client GStreamer bundle 链路

## Root Hub 与 owner 边界

- Plan ID：`issue-200`
- Root hub：[Issue #200](../../../../../docs/dev/issue-200/README.md)
- Owner directory：`crates/xtask`
- Owner index：[xtask 开发文档](../README.md)
- Root-owned IDs consumed：`D-2000`、`D-2002`、`C-2001`、`R-2000`
- Owner-authored IDs：`F-2100`–`F-2108`、`T-2100`
- Assigned WP：`WP-2002`
- Owns：删除 HTTP Client GStreamer bundle CLI、staging/verifier modules 与其测试入口。
- Does not own：Rodio audio backend、Linux ALSA package、CI workflow 删除或 PDF viewer。

Rodio 通过 CPAL 调用 OS audio API，不需要提供、复制或验证独立 native runtime。完成此工作后，xtask 对
HTTP Client 音频没有新运行时责任。

## 文件边界

```text
crates/xtask/
├── src/bundle.rs                      # F-2100 [Modify, handwritten] remove GStreamer submodule/dispatch
├── src/bundle/gstreamer.rs             # F-2101 [Delete, handwritten] manifest staging/verifier
├── src/bundle/gstreamer/linux.rs       # F-2102 [Delete, handwritten] Linux private prefix staging
├── src/bundle/gstreamer/macos.rs       # F-2103 [Delete, handwritten] Framework/rpath/codesign staging
├── src/bundle/gstreamer/windows.rs     # F-2104 [Delete, handwritten] Windows DLL/prefix staging
├── src/cli.rs                          # F-2105 [Modify, handwritten] remove GStreamer CLI options/subcommands
├── src/main.rs                         # F-2106 [Modify, handwritten] remove GStreamer command routing/error text
├── docs/dev/issue-200/README.md        # F-2107 [Modify, handwritten] this owner plan
└── src/bundle/settings.rs              # F-2108 [Modify, handwritten] map generic bundle.deb.depends metadata
```

The app-local `build-assets/gstreamer/*` inputs belong to `app/http-client` and are deleted by its owner plan.
No replacement manifest, runtime copy, `GPUI_GSTREAMER_*` compatibility variable, or xtask audio command is added.

### 当前实施状态（2026-08-14）

GStreamer bundle/staging/verifier/CLI 的 active implementation 已删除。此事实不等同于三平台 build 或 package
验证通过；对应结果仍由 root `C-2001` 后续记录。

## WP-2002：删除 staging/verifier CLI

1. Remove F-2101–F-2104 and the module declarations/exports in F-2100.
2. Remove every GStreamer-specific bundle command, option, environment-variable diagnostic and unit test from
   F-2100/F-2105/F-2106. Keep unrelated app bundle behavior untouched.
3. Search `crates/xtask` for `gstreamer`, `GST_`, `GPUI_GSTREAMER`, `verify-gstreamer` and the deleted module paths;
   any remaining executable reference is a failure.
4. Do not replace this with an xtask Rodio check: dependency resolution and runtime audio behavior are owned by
   HTTP Client/Cargo and root platform verification respectively.
5. Preserve generic Deb metadata parsing and map HTTP Client's `depends = ["libasound2"]` into
   `BundleSettings::deb.depends`; test the exact mapping without adding audio-specific xtask behavior.

| T-ID | Focused validation | Expected evidence |
| --- | --- | --- |
| `T-2100` | `cargo test -p xtask --locked --no-fail-fast` | remaining xtask bundle commands/tests pass; actual result is recorded in root hub |
| `T-2101` | `cargo clippy -p xtask --all-targets --all-features --locked -- -D warnings` | no lint regression |
| `T-2102` | scoped residual scan | no executable GStreamer manifest/staging/verifier/CLI surface under `crates/xtask` |

`WP-2002` 已完成：root plan 记录了实际删除路径、12/12 xtask tests、strict Clippy 与 active residual scan。
