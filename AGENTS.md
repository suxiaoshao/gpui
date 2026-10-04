# 项目约定

- Rust 多应用 workspace，成员和版本以根 `Cargo.toml` 为准。应用入口在 `app/{name}/src/main.rs`；应用专属业务能力放在 `app/{name}/crates/`，入口负责窗口和功能组合；跨应用能力放在根 `crates/`。
- Rust 模块使用 `{module}.rs`，新增依赖写完整版本号；遵循项目格式配置。
- 应用通过 `gpui_kit`、`gpui_kit::component`、`gpui_kit::assets` 接入；共享 crate 的别名以根 manifest 为准。

## 文档与技能

[README](README.md) 提供运行入口，[开发索引](docs/dev/README.md) 导航计划。稳定行为归属应用或 crate 的 README/指南。

按实际任务使用 `.agents/skills/`：

| 工作 | skill |
| --- | --- |
| 应用结构与职责选择 | `gpui-app-development` |
| GPUI API、组件 | `gpui-kit`、`gpui-kit-design-guides` |
| 图标、资源、本地化 | `gpui-app-icon-usage`、`gpui-i18n` |
| Store、Operation、Form 实现或接入 | 对应 `gpui-store`、`gpui-operation`、`gpui-form` |
| 原生界面调试 | `gpui-computer-use-debugging` |
| 需要持久记录的复杂设计 | `implementation-plan-design` |

## 验证与协作

- 根据改动选择受影响 crate 的验证；沿用已有覆盖，仅为未覆盖的具体回归风险补测试。删除实现时同步删除其专用测试。
- Issue、PR 使用 `.github/` 模板，标题说明应用或 crate；PR 描述覆盖分支相对远程最新 `main` 的整体差异，默认普通 PR。
- 提交和集成遵循实际 hooks 与 `.github/workflows/ci.yml`。macOS / Linux 的工具链与原生依赖集中维护在 `flake.nix` / `flake.lock`，Rust 版本由 `rust-toolchain.toml` 统一声明。
