# Feiwen 依赖升级计划

- 状态：`In progress`（本地自动化通过；三平台/人工 smoke 待执行）
- Owner：`app/feiwen`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)

## Owner scope

Feiwen 保留完整 `gpui-component`，不迁移到 `gpui-base`。本 owner 负责应用启动、表单/UI、
自绘标题栏及 system-accent 主题的消费侧验证；公共主题投影修复属于 `crates/app-theme`。

## 精确依赖与已知命中

`app/feiwen/Cargo.toml:24-32` 直接依赖 `gpui`、`gpui_platform`、`gpui-component`、
`gpui-form`、`gpui-form-gpui-component`、`gpui-operation/tracing`、`gpui-store`、
`app-assets` 与 `app-theme/system-accent`；61 行的 dev dependency 开启 `gpui/test-support`。
升级后继续继承 root 统一版本，不增加 `gpui-base`。

非 GPUI direct 更新只有 `thiserror 2.0.19 -> 2.0.20`。`reqwest 0.13.4`、
`duckdb 1.10505.0`、`scraper 0.27.0`、`nom 8.0.0`、`regex 1.13.1`、`url 2.5.8` 等在
2026-08-20 root audit 中保持 current；实施当天仍需重跑 audit，不能据此永久 pin。

- `app/feiwen/src/main.rs:29` 调用 `gpui_component::init(cx)`；该调用已覆盖 transitively 的
  `gpui_base::init`，不得重复初始化。
- `app/feiwen/src/main.rs:95,104` 使用 GPUI platform、app assets 与 `Root`；component TitleBar 窗口改以
  `TitleBar::window_options()` 为 base，再覆盖 bounds/background/traffic-light position，删除手写
  `app_owns_titlebar_drag = true`。
- `app/feiwen/src/app/workspace.rs:137` 调用
  `app_theme::apply_fixed_system_accent_theme(window, cx)`；需消费 `app-theme` owner 增加的
  `Theme::sync_base`，并验证 scrollbar/resize handle 跟随 accent。
- `features/fetch.rs` 与 `features/query/advanced/{controller,render}.rs` 仅使用单行
  `FormInput`/`InputState` 和 number input，不命中移除的 `.multi_line`/`.code_editor` builder。
- target 只让 `ListItem` 可直接挂 drag handlers，没有提供 typed reorder state machine；advanced sort 的多列
  form row、独立 handle、drag preview、`PathKey` 与 mutation 全部保留。本批不强行换 row shell。
- `Disableable`、`Selectable`、`IndexPath`、flex helpers 等目标组件仍 re-export，现有 import
  路径保持不变。

## 工作包

### FEIWEN-DEP-1：依赖与初始化

- 消费 root 的同一 Zed/longbridge Git 身份，保留完整组件、assets、form 和 theme 依赖。
- 保持 `gpui_component::init` 为唯一组件初始化入口；不直接依赖或初始化 `gpui-base`。

### FEIWEN-DEP-2：消费公共修复

- 在 `app-theme` theme projection 修复和 form adapter 升级完成后重新编译 Feiwen。
- 对 fetch 与 advanced query 的 FormInput、IntegerInput、select/combobox 做行为测试；只有
  上游确实破坏 re-export 时才调整 import。

### FEIWEN-DEP-3：平台/UI 回归

- 以 `TitleBar::window_options()` 统一窗口契约，检查 drag region、double-click、窗口按钮与 route title。
- 保留 advanced sort DnD；回归 handle-only drag、disabled form field、drop target 与 typed reorder。
- 切换 light/dark/system-accent，确认普通控件、scrollbar 与 resize handle 使用同一主题。

### FEIWEN-DEP-4：非 GPUI direct 回归

- 更新 `thiserror` 后运行 fetch/query/database error mapping tests，确认 source chain 与用户可见
  分类不变。
- 对 root audit 标记 current 的网络、parser 与 DuckDB direct dependencies 只做 lockfile
  cohort 审阅和现有 tests，不做无目标 manifest churn。

## Focused verification

```text
cargo check -p feiwen --locked
cargo test -p feiwen --locked
cargo clippy -p feiwen --all-targets --all-features --locked -- -D warnings
cargo tree -p feiwen --duplicates --locked
```

手工 smoke：启动应用，执行一次 fetch 表单和 advanced query 编辑，切换系统 accent，检查
Windows/macOS/Linux 的 titlebar 拖动与窗口 resize。

## 完成条件

- Feiwen 没有 direct `gpui-base`，启动时只初始化完整组件。
- system-accent 的 base theme 投影与表单行为回归通过。
- TitleBar helper 与 advanced sort DnD 的三平台行为通过，未把领域 reorder 误迁成通用 ListItem state。
- `thiserror 2.0.20` 的 fetch/query/database error paths 通过。
- 应用测试、clippy 和三平台 CI 通过。
