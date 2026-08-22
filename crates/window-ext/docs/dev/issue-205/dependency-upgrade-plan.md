# window-ext 依赖升级计划

- 状态：`In progress`（本地自动化通过；macOS/Windows native smoke 与三平台 CI 待执行）
- Owner：`crates/window-ext`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)

## Owner scope

本 crate 拥有 GPUI `Window` 到 macOS/Windows native handle 的平台扩展、窗口可见性/层级/位置
操作与 macOS Quick Look。它直接依赖 GPUI，不消费 UI component；不增加 `gpui-base`。

## 精确依赖与已知命中

`crates/window-ext/Cargo.toml:7-18` 的完整 direct audit：

| Scope | Dependency | Current | Target / disposition |
| --- | --- | --- | --- |
| all | `gpui` | 0.2.2 @ `1a246efd...` | 0.2.2 @ `e0931d5a...` |
| all | `raw-window-handle` | 0.6.2 | current |
| all | `thiserror` | 2.0.19 | 2.0.20 |
| macOS | `objc2-app-kit` | 0.3.2 | current |
| macOS | `objc2` | 0.6.4 | current |
| macOS | `objc2-foundation` | 0.3.2 | current，保留 `objc2-core-foundation` feature |
| Windows | `windows` | 0.62.2 | current，保留现有 Win32 features |

所有 `current` 结论来自 2026-08-20 root audit，实施当天重审。

- `src/lib.rs:6` 导入 `Bounds`、`DisplayId`、`Pixels`、`Window`；86-254 行包装
  `RawWindowHandle` 并执行平台操作。
- `src/lib.rs:265-295` 声明 `WindowExt` 与 Quick Look；295-541 行为 GPUI `Window` 实现
  native handle、show/hide、floating、cursor rect、visibility 与 bounds。
- `src/lib.rs` 当前以 crate-wide deprecated allow 继续使用 `HasRawWindowHandle`。raw-window-handle 0.6 已提供
  `HasWindowHandle::window_handle(...).as_raw()`，本批删除旧 trait/allow；禁止用 unsafe pointer cast 绕过
  版本身份。target `TestWindow` raw handle 仍会 panic，测试不得调用 native path。
- `src/lib.rs:558+` 的纯转换 tests 覆盖 macOS window level 与 logical-to-device bounds，但
  真正 native calls 仍需各平台 smoke。
- Jaco 是当前直接消费者；Lestty 若以后需要窗口透明/材质，应在终端 issue 中明确授权后再
  消费此 crate，本升级计划不预先增加依赖。

## 工作包

### WINDOW-DEP-1：句柄/API 对齐

- 用 root GPUI pin 检查 `Window` native handle API、`DisplayId` 与 bounds 类型；同步
  raw-window-handle 的单一版本。
- 改用 `HasWindowHandle` 的安全公开 API，删除 crate-wide deprecated allow 和旧 trait import，不保留双 trait
  fallback；这只收回兼容层，不删除 window-ext 的 native capability。

### WINDOW-DEP-2：平台实现验证

- macOS：show/hide、window level、cursor rect、bounds 与 Quick Look。
- Windows：show/hide/show-without-activation、topmost、visibility、DPI bounds。
- Linux：无原生实现的分支保持明确的 unsupported/no-op contract，并确保可编译。

### WINDOW-DEP-3：消费侧回归

- Jaco 验证 temporary window hide/show、floating behavior 和 bounds restore。
- 三平台 CI 必须分别编译其 cfg 分支；不能只靠当前开发平台的 unit tests 验收。
- `thiserror 2.0.20` 更新后验证每个 `WindowExtError` variant 的 source/display contract，避免
  平台 API 错误被丢失为无来源字符串。

## Focused verification

```text
cargo check -p window-ext --locked
cargo test -p window-ext --locked
cargo clippy -p window-ext --all-targets --all-features --locked -- -D warnings
cargo tree -p window-ext --duplicates --locked
cargo check -p jaco --locked
```

另外由 CI 在 macOS、Windows、Linux 执行相同 package checks；macOS/Windows 各完成一次真实
native window smoke。

## 完成条件

- GPUI 与 raw-window-handle 只有兼容的一组类型身份，无 unsafe compatibility shim。
- active source 无 `HasRawWindowHandle` 或 crate-wide deprecated allow；真实 native smoke 不走 TestWindow。
- `thiserror 2.0.20` 的 native error mapping 与 display/source tests 通过。
- 平台 cfg 编译、纯转换 tests、Jaco smoke 与三平台 CI 通过。
- crate 没有 direct component/base 依赖。
