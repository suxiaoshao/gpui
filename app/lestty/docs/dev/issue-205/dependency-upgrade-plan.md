# Lestty 依赖升级计划

- 状态：`In progress`（本地依赖图与自动化通过；三平台 CI 待执行）
- Owner：`app/lestty`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)
- 本文是 Lestty 在 Issue #205 中的依赖升级实施规范；跨 workspace 的版本、Git SHA 与
  lockfile 决策仍由 root plan 统一拥有。

## Owner scope

本 owner 只负责空应用脚手架的最小依赖图和终端应用所需的 app-local 资源边界；按当前
Issue #205 范围，`src/main.rs` 保持空 `main`，不提前建立窗口或初始化 UI。终端核心、PTY、
配置、主题和首个 GPUI 窗口继续由
[选型草稿](terminal-backend-selection-draft.md)维护。

Lestty 是本次升级中**唯一**直接依赖 `gpui-base` 的应用。它不引入
`gpui-component`、`gpui-component-assets`、`app-assets` 或 `app-theme`；其他应用也不会因为
Lestty 而迁移到 `gpui-base`。

## 当前证据与目标依赖

| 位置 | 当前状态 | 目标 |
| --- | --- | --- |
| `app/lestty/Cargo.toml:11` | `[dependencies]` 为空 | 增加 workspace `gpui`、`gpui_platform` 与 `gpui-base` |
| `app/lestty/src/main.rs:1` | 空 `main` | 本批保持空 `main`；后续终端实施再建立启动链并调用 `gpui_base::init(cx)` |

目标 `gpui-base` 来自 `longbridge/gpui-component` 提交
`5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3`，其 manifest 版本是 `0.5.2` 且
`publish = false`，因此必须通过与完整组件同一 Git SHA 的 workspace 依赖获得。`gpui` 与
`gpui_platform` 必须使用 root plan 选定的同一个 Zed 提交，不能在本 crate 另行 pin。

`gpui-base` 已公开 `gpui_base::init`、行为/交互基础设施、语义主题 token、输入/textarea/editor、
tab、scrollbar 与 dialog 等基础 API；它不提供完整 `gpui-component` 的 styled component
设计系统和 assets。

新 `gpui_base::TextSelection` 可作为 terminal selection 的 mapping spike，复用 window gesture、range、copy 与
auto-scroll；它不能替代终端 core 对 grid/cell、wide character、wrapped line、block selection 和 alternate
screen 的 authority。本依赖升级只保证 API 可用，不在空 crate 中提前实现 terminal selection。

## 工作包

### LST-DEP-1：声明最小依赖图

- 在 root workspace dependencies 中注册与 `gpui-component` 同 SHA 的 `gpui-base`，再由
  `app/lestty/Cargo.toml` 继承。
- 保持 Lestty 直接依赖图仅含 `gpui`、`gpui_platform`、`gpui-base` 及经确认的终端依赖。
- 不通过 `app-theme` 或 `app-assets` 间接拉入完整组件；Lestty 的图标、字体和主题资源由
  app-local asset source 拥有。

### LST-DEP-2：冻结后续初始化边界

- 本批不创建 GPUI application/window，也不调用任何 UI init；空 crate 只验证目标依赖可解析。
- 后续建立启动链时只调用一次 `gpui_base::init(cx)`，不调用 `gpui_component::init`，也不同时
  调用两套 init。
- 后续首个 window 使用 GPUI/`gpui-base` API 组合，不依赖 `Root`、`TitleBar` 等完整组件类型。

### LST-DEP-3：锁定轻量性回归门

- 用反向依赖图确认 Lestty 没有拉入完整组件族。
- release 二进制尺寸和 cold-start 基线延后到首个可启动窗口；后续新增 UI dependency 必须解释
  其体积与初始化成本。
- 若后续引入 Tokio，Lestty 自己显式声明 `process/io/time` 等 features；不依赖 Jaco/HTTP Client 的 feature union。

## Focused verification

```text
cargo check -p lestty --locked
cargo test -p lestty --locked
cargo clippy -p lestty --all-targets --all-features --locked -- -D warnings
cargo tree -p lestty --locked
cargo tree -i gpui-base --locked
cargo tree -i gpui-component --locked
```

通过条件：`cargo tree -p lestty` 不含 `gpui-component`、`gpui-component-assets`、`app-theme`
和 `app-assets`；`cargo tree -i gpui-component` 不出现 `lestty`；空 crate 在目标依赖图上通过
check/test/clippy。窗口创建和 init 重复注册属于后续终端实施验收。

## 完成条件

- root plan 已锁定唯一的 Zed 与 longbridge Git 身份，Lestty 使用相同来源。
- 空 crate 的三平台构建与依赖图门禁通过；本批不实现最小窗口。
- 终端实现开始前，依赖升级批次已经独立可验证并可回滚。
