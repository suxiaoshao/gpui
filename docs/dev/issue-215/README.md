# Issue #215：GPUI Kit 发布包迁移记录

本页保留 2026-09-07 从 Git 依赖迁移到 GPUI Kit 0.6.0 / GPUI pre 0.3.3 的共享结论。
当前版本和运行命令以根 manifest、README 及后续 [0.7.0 批次](../dependency-refresh-0.7.0/README.md)为准。
Jaco、旧资源宏、Quick Look、OCR 和独立 MCP 工具已退役，对应工作包与发布要求不再适用。

## 保留的公共契约

- 应用通过 `gpui_kit`、`gpui_kit::component`、`gpui_kit::assets` 接入，共享 crate 别名沿用根 manifest；GPUI 类型只保留一个来源。
- 本地 `gpui-tokio` 保留 owned/external runtime、JoinError 与 drop-to-abort 契约。
- Form 持有业务字符串和验证，原生 Input/Textarea/Editor 持有 IME、焦点与选择；`ControlBinding` 持有订阅和租约，`defer_set` / `defer_blur` 继续负责事件路由。
- 原生组件提供通用交互，应用保留数据、稳定 ID、领域约束与错误恢复；替换组件不隐式移动业务所有权。
- 语法高亮和 Markdown 主题统一由 `app-theme` 提供；应用不另建颜色缓存或强制重解析路径。
- 各平台继续使用同一个 Rust 工具链与锁图，按真实包名配置优化；单独构建应用以验证 feature 自足性。
- 当前图标使用 `gpui-lucide` 的 SVG bytes 和官方默认资源，旧宏的隐藏类型 re-export 与资源注册要求已删除。

## 保留的 owner 记录

| Owner | 计划 | 当时工作包 |
| --- | --- | --- |
| app/http-client | [http-client](../../../app/http-client/docs/dev/issue-215/README.md) | WP-03 |
| app/feiwen | [feiwen](../../../app/feiwen/docs/dev/issue-215/README.md) | WP-03、WP-05 |
| app/novel-download | [novel-download](../../../app/novel-download/docs/dev/issue-215/README.md) | WP-03、WP-05 |
| crates/app-theme | [app-theme](../../../crates/app-theme/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-form | [gpui-form](../../../crates/gpui-form/docs/dev/issue-215/README.md) | WP-02、WP-05 |
| crates/gpui-form-gpui-component | [gpui-form-gpui-component](../../../crates/gpui-form-gpui-component/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-form-macros | [gpui-form-macros](../../../crates/gpui-form-macros/docs/dev/issue-215/README.md) | WP-05 |
| crates/gpui-heatmap | [gpui-heatmap](../../../crates/gpui-heatmap/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-operation | [gpui-operation](../../../crates/gpui-operation/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-store | [gpui-store](../../../crates/gpui-store/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-tokio | [gpui-tokio](../../../crates/gpui-tokio/docs/dev/issue-215/README.md) | WP-02 |
| crates/http-client-test-server | [http-client-test-server](../../../crates/http-client-test-server/docs/dev/issue-215/README.md) | WP-05 |
| crates/platform-ext | [platform-ext](../../../crates/platform-ext/docs/dev/issue-215/README.md) | WP-05 |
| crates/window-ext | [window-ext](../../../crates/window-ext/docs/dev/issue-215/README.md) | WP-02、WP-05 |
| crates/xtask | [xtask](../../../crates/xtask/docs/dev/issue-215/README.md) | WP-05、WP-07 |

## 证据入口

- [调查结论](draft.md)：版本配套、组件边界与 SVG 选型。
- [历史实施与验证](implementation.md)：当时执行结果，不作为当前构建通过的证据。
- [Jaco 退役](../jaco-retirement/README.md)：专用 owner 和构建链的删除范围及实际验证。
