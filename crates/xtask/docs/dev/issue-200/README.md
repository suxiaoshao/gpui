# xtask：HTTP Client GStreamer staging 与 release verifier

## Root Hub 与 Owner 边界

- Plan ID：`issue-200`
- Root hub：[Issue #200 发行交付计划](../../../../../docs/dev/issue-200/README.md)
- Owner directory：`crates/xtask`
- Owner index：[xtask 开发文档](../README.md)
- Root-owned IDs consumed：`C-2001`、`D-2000`–`D-2001`、`WP-2000`、`WP-2003`
- Assigned work package：`WP-2002`
- Owns：app-local manifest 的解析、staging、release verifier、其单元测试与安全诊断。
- Does not own：manifest/notices/fixtures 的内容，也不拥有 HTTP Client 产品状态或媒体运行时代码。

## 文件与执行契约

```text
crates/xtask/
├── src/bundle.rs                 # F-2100 [Modify] dispatch only for the http-client release path
├── src/bundle/gstreamer.rs       # F-2101 [Modify/Add] manifest parser, whitelist staging and verifier
├── src/cli.rs                    # F-2102 [Modify] explicit SDK versus release verification commands
├── src/main.rs                   # F-2103 [Modify] command routing
└── docs/dev/issue-200/README.md  # F-2104 [This file] owner plan and completion record
```

The CLI accepts the app-local manifest path as its source of truth. It may inspect staged package contents but
must not embed HTTP Client codec/plugin lists, silently discover an arbitrary system runtime, or mutate the
manifest.

## WP-2002：manifest-driven staging 与 verifier

1. Parse and validate the `C-2001` manifest before copying any native payload.
2. Preserve target-specific private layouts: macOS Framework/symlink/rpath, Windows exe-root DLL plus sibling
   plugin tree, and Linux private prefix with per-ELF relative RUNPATH.
3. Verify producer source identity, required paths, plugin/element mapping, notice presence and private runtime.
4. Keep SDK verification separate from release verification. An SDK gate may establish build prerequisites only;
   a release verifier rejects incomplete package artifacts.
5. Cover malformed manifest, missing whitelist entry, hash mismatch, absent notice, missing plugin/element and
   target-layout failure with focused unit tests.

`GPUI_GSTREAMER_SDK_ROOT` 与 `GPUI_GSTREAMER_RUNTIME_DIR` 只用于显式覆盖。macOS release build 默认自动准备
同一官方 1.28.6 发行的 development SDK 与 private runtime，编译和运行使用一致的 GLib/GStreamer ABI；最终
bundle 把动态库收敛到安装包内 `@rpath`，并拒绝 Homebrew 或其他宿主 GStreamer 路径。Windows 与 Linux 使用
各自 producer 的固定 prefix；显式覆盖无效时直接失败，不回退到另一套安装。

## Diagnostics and validation

Diagnostics contain target, manifest field name, artifact-relative path and plugin/element identifier only. They
must not emit HTTP response data, URLs, headers or user file paths.

Run focused xtask tests and strict Clippy, then exercise the verifier against every produced package. The root hub
records the three-platform package result. This plan remains `In progress` until `C-2001` is proven for all
targets.

## 当前实现（2026-08-13）

- manifest format 1、三平台 private layouts、source SHA/revision、required paths 与 elements 已实现验证。
- macOS：匹配 runtime 的官方开发 SDK 自动准备、完整 Framework copy、symlink preservation、arm64 thin、
  主程序 load-command 与 rpath 验证、外部 Homebrew/GStreamer dependency 拒绝、私有 `gst-inspect`、所有
  修改后的最终 codesign。
- Windows：`bin/*.dll` 同时进入 app root 与 sibling prefix，plugins/scanner/data 进入 `gstreamer/` resources。
- Linux：Cerbero prefix 进入 Debian resources，主程序/lib/plugin/scanner 分别写相对 RUNPATH。
- `verify-gstreamer --inspect` 只运行 manifest 对应的 private inspector/plugin/scanner/registry，不再接受
  host PATH 中的 GStreamer 作为 release 证据；source marker 与 notices 也属于 fail-closed contract。
- bundle 命令会自动发现平台脚本安装的固定 SDK 与 private runtime；环境变量已收敛为可选覆盖，缺失产物时
  错误会给出对应的 producer 命令。
- release workflow 已增加 macOS ZIP、Windows MSI、Linux DEB 的独立解包 smoke；
  `cargo test -p xtask --locked`（46）与 strict Clippy 已通过。三平台 workflow 尚未运行，因此仍为
  `In progress`。
- 2026-08-13 本机未设置 `GPUI_GSTREAMER_*` 环境变量运行真实 macOS bundle 成功。xtask 自动选择匹配的
  官方 1.28.6 SDK/runtime，裁剪到可达 Mach-O/plugin 闭包，运行私有 `gst-inspect`，清除所有非系统绝对
  rpath，并在最后完成 deep ad-hoc codesign。成品约 102 MiB；主程序与内置 GLib compatibility version
  同为 8201，且未引用 SDK、Homebrew 或系统 GStreamer 路径。
