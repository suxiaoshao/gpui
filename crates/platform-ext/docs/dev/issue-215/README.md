# platform-ext：依赖与平台边界

历史批次：[Issue #215](../../../../../docs/dev/issue-215/README.md)。当前保留 `app` 与 `appearance`，为现役应用提供前台应用交互、窗口菜单、图标、通知角标和系统颜色观察。

OCR 及 Windows AI 的 `build.rs` / `winmd` 生成链已随 [Jaco 退役](../../../../../docs/dev/jaco-retirement/README.md)删除。`windows-bindgen`、直接 `windows-future` 与 OCR 专用 features 不再由本 crate 引入。
`windows-core` 仍用于 `appearance` 的 `IInspectable`，`Foundation` / `UI_ViewManagement` 和 Win32 窗口、显示器、键盘接口继续保留；macOS 的 CoreGraphics 事件回填和系统外观能力继续使用原生依赖。

验证使用 `cargo check -p platform-ext --locked` 及上层应用的实际调用。macOS 编译不代表 Windows 已验证；各平台结果见退役文档。
