# platform-ext：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（Windows 本地自动化通过；macOS/Linux CI 与 native smoke 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)
- Owner directory：`crates/platform-ext`
- Root-owned surfaces consumed：`S-16`、`S-17`、`S-19`
- Owner-local IDs：`F-PE-01`–`F-PE-04`、`R-PE-01`–`R-PE-03`、`T-PE-01`–`T-PE-04`、`WP-PE-01`–`WP-PE-02`
- Owns：platform-ext registry targets、Windows binding build、macOS Objective-C/CG consumers 与三平台 focused validation。
- Does not own：GPUI window integration、workspace lockfile、winmd source updates 或新 platform capability。

目标 GPUI 的 `set_window_appearance` 只同步 native Light/Dark 外观，不能替代本 crate 的 macOS
control-accent/selected-text 与 Windows `UISettings` accent 观察；本 owner 明确保留，不做错误去重。

## 精确依赖目标

| Dependency | Current | Target | Features/kind | Classification |
| --- | --- | --- | --- | --- |
| `thiserror` | `2.0.19` | `2.0.20` | runtime | Compatible |
| `tracing` | `0.1.44` | 保留 `0.1.44` | runtime | Retained |
| `windows-bindgen` | `0.66.0` | 保留 `0.66.0` | build | Retained |
| `block2` | `0.6.2` | 保留 `0.6.2` | macOS | Retained |
| `objc2` | `0.6.4` | 保留 `0.6.4` | macOS | Retained |
| `objc2-app-kit` | `0.3.2` | 保留 `0.3.2` | macOS | Retained |
| `objc2-core-foundation` | `0.3.2` | 保留 `0.3.2` | macOS | Retained |
| `objc2-core-graphics` | `0.3.2` | 保留 `0.3.2` | `CGColorSpace`, `CGDataProvider`, `CGDirectDisplay`, `CGGeometry`, `CGImage`, `CGWindow` | Retained |
| `objc2-foundation` | `0.3.2` | 保留 `0.3.2` | `block2`, `NSUserDefaults`, `NSString` | Retained |
| `windows` | `0.62.2` | 保留 `0.62.2` | manifest 中 9 个 WinRT/Win32 features 原样保留 | Retained |
| `windows-core` | `0.62.2` | 保留 `0.62.2` | Windows | Retained |
| `windows-future` | `0.3.2` | 保留 `0.3.2` | Windows | Retained |

除 `thiserror` 外无 manifest target 变化；不得重生或替换 `winmd/*.winmd`。

## Owner-local 目标与文件

```text
crates/platform-ext/
├── Cargo.toml                                      # F-PE-01 [Modify] thiserror 2.0.19 -> 2.0.20
├── build.rs + winmd/*.winmd                        # F-PE-02 [Verify/retain] Windows binding input
├── src/{app,appearance,ocr}.rs + src/ocr/windows*  # F-PE-03 [Verify] Windows APIs/build output consumers
└── src/{app,appearance,ocr}.rs + src/ocr/macos.rs  # F-PE-04 [Verify] Objective-C/CoreGraphics consumers
```

- `R-PE-01`：Windows bindgen 仍只消费现有 winmd inputs，生成物留在 build output；tracked winmd 无 diff。
- `R-PE-02`：Windows OCR/appearance/app APIs 和 macOS OCR/appearance/app APIs 保持公开签名与行为。
- `R-PE-03`：Linux cfg path 继续可编译；不得为一次 error-derive patch 增加 platform fallback。

## Owner-local Work Packages

### WP-PE-01：更新 thiserror，冻结 native dependency set

1. 仅修改 `F-PE-01` 的 `thiserror` target；保留所有 target-specific versions/features。
2. 由 Cargo 更新 root lockfile；不运行 winmd 更新器、不编辑 binary inputs。
3. 当前平台编译若要求源码变化，先证明来自 thiserror 2.0.20；不顺带迁移 Windows/objc APIs。

### WP-PE-02：完成三平台 gate

1. 运行当前平台 tests/Clippy。
2. 在 root CI 分别执行 Windows、macOS、Linux build/test；Windows 必须执行 `build.rs` 和 generated bindings，macOS 必须类型检查 Objective-C/CoreGraphics paths。
3. 检查 tracked `winmd` 和公开 Rust signatures 无变化。

完成条件：只有 manifest/lockfile 的预期版本 diff，三平台均通过，`R-PE-01`–`R-PE-03` 成立。

## Focused Validation 与 handoff

| T-ID | Command/scenario | Expected evidence |
| --- | --- | --- |
| `T-PE-01` | `cargo test -p platform-ext --all-features --locked` | 当前平台 unit tests 与 cfg surface 通过 |
| `T-PE-02` | `cargo clippy -p platform-ext --all-targets --all-features --locked -- -D warnings` | 当前平台无 warning |
| `T-PE-03` | root Windows CI | bindgen、OCR/appearance Windows code 编译和 tests 通过 |
| `T-PE-04` | root macOS/Linux CI | macOS native consumers 与 Linux cfg path 分别通过 |

三平台结果和未执行边界只在 root completion evidence 汇总。
