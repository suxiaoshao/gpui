# HTTP Client：Response 媒体与 PDF 预览发行资产

## Root Hub 与 Owner 边界

- Plan ID：`issue-200`
- Root hub：[Issue #200 发行交付计划](../../../../../docs/dev/issue-200/README.md)
- Owner directory：`app/http-client`
- Owner index：[HTTP Client 开发文档](../README.md)
- Root-owned IDs consumed：`C-2001`、`E-2000`–`E-2002`、`D-2000`–`D-2001`、`WP-2000`、`WP-2003`
- Assigned work package：`WP-2001`
- Owns：app-local native runtime manifest、third-party notice、response-preview fixture corpus、app package
  smoke 和 owner-local completion evidence。
- Does not own：GStreamer runtime staging/verify CLI 的实现（`crates/xtask`），以及 root CI/脚本的三平台
  orchestration。

前序 [#199 media/PDF 计划](../issue-199/response-media-and-pdf-preview-plan.md) 已是 `Superseded` 历史基线。
其中的 Response asset、MediaSession、PdfPreview、viewer mode、i18n 与自动化代码不在本计划重写。

## 文件与工件边界

```text
app/http-client/
├── build-assets/gstreamer/runtime-manifest.toml       # F-2000 [Add] app-local runtime/plugin/license source of truth
├── build-assets/gstreamer/THIRD_PARTY_NOTICES.md      # F-2001 [Add] package notice and source-offer record
├── test-data/response-preview/README.md               # F-2002 [Add] fixture provenance/license/generation record
├── test-data/response-preview/*                       # F-2003 [Add] minimal licensed media/PDF smoke corpus
├── Cargo.toml                                         # F-2004 [Modify only if metadata needs the frozen release contract]
├── src/features/request/response/*                    # F-2005 [No behavioral rewrite] existing preview baseline; add only fixture-facing tests if needed
└── docs/dev/issue-200/README.md                       # F-2006 [This file] owner evidence and completion record
```

Runtime installers, frameworks, DLLs, plugins and generated package payloads never enter Git. The manifest is
the sole input for xtask; source code may not hard-code release paths or plugin lists.

## Owner-local requirements

### R-2000：可审计 native runtime

`runtime-manifest.toml` records every supported target's version, source URL, immutable SHA-256, deployment
mode, staged files, transitive native libraries, required element→plugin mapping and per-component license/notice
path. Placeholder values cannot be committed.

### R-2001：可审计 fixture corpus

Each fixture is small, local and offline-runnable. Its README records origin, exact license, any generation
command, checksum, MIME/codec/PDF purpose and the test that consumes it. Runtime installers and opaque
redistribution material are excluded.

### R-2002：app-level smoke preserves the existing contract

Fixture tests exercise only the already implemented viewer boundary: `audio/*`, `video/*`, `application/pdf`,
manual-mode failure and missing-plugin behavior. They must continue to preserve `RequestRuntime::Ready`, Headers
and Save behavior; response content and URLs stay out of logs/assertion snapshots.

## WP-2001：资产、fixture 与 app smoke

1. Add F-2000 through F-2003 from verified upstream/package evidence and make the manifest parseable by the
   xtask contract in `C-2001`.
2. Add fixture-backed tests at the existing response preview boundary; retain the existing pure/fake tests for
   lifecycle and stale-result behavior.
3. In every packaged target, run the manifest's required element/plugin and codec/PDF smoke through the app.
4. Record commands, fixture checksums, target triple, package artifact and actual viewer result here; do not
   record a platform as passed from another platform's result.

## Focused validation

| Scenario | Expected evidence |
| --- | --- |
| Manifest/notice/fixture provenance checks | Missing source, hash, license, plugin or fixture metadata fails. |
| HTTP Client focused/full tests and strict Clippy | Existing request/response behavior remains intact. |
| Installed app audio/video/PDF smoke on each target | Runtime loads from the documented deployment location; each fixture renders/plays as applicable. |

This owner plan is `Done` only after `C-2001` has its app-owned artifact evidence and all three package smokes
are recorded by the root hub.
