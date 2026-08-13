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
2. Stage only the recorded files while preserving the target-specific layout required by macOS codesign/rpath and
   Windows DLL/plugin discovery; Linux validates declared system dependencies instead of staging foreign ABI.
3. Verify staged file hash, plugin/element mapping, license/notice presence and fixture-compatible runtime.
4. Keep SDK verification separate from release verification. An SDK gate may establish build prerequisites only;
   a release verifier rejects incomplete package artifacts.
5. Cover malformed manifest, missing whitelist entry, hash mismatch, absent notice, missing plugin/element and
   target-layout failure with focused unit tests.

## Diagnostics and validation

Diagnostics contain target, manifest field name, artifact-relative path and plugin/element identifier only. They
must not emit HTTP response data, URLs, headers or user file paths.

Run focused xtask tests and strict Clippy, then exercise the verifier against every produced package. The root hub
records the three-platform package result. This plan remains `In progress` until `C-2001` is proven for all
targets.
