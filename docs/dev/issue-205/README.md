# Issue #205：Lestty 终端应用

## 状态与范围

- 状态：`Draft`
- 关联 issue：[#205](https://github.com/suxiaoshao/gpui/issues/205)
- Plan ID：`issue-205`
- Root hub：`docs/dev/issue-205/README.md`
- Root index：[Workspace development plans](../README.md)
- Branch：`codex/205-add-lestty-terminal`
- Primary feature owner：`app/lestty`
- Prerequisite owners：全 workspace member + `tools/mcp-auth-test-server`，见下方 owner 导航
- 最近证据刷新：`2026-08-21`
- 依赖升级前置：`In progress`（本地实现与自动化验证完成；外部 CI/E2E/人工 smoke 待执行）
- Implementation references：`Pending`

本 issue 当前只完成 Lestty crate 脚手架；`0.1.0` 已确认采用 `alacritty_terminal 0.26.0`、一个主窗口中的
tabs/splits、UI-independent `crates/lestty-terminal` 和 app-owned `state.toml`。终端引擎、PTY、GPUI 渲染、
输入与平台构建尚未进入实现；在其余协议、配置与外观选择得到确认之前，本计划保持 `Draft`。开始 Lestty 实现前，还必须先完成全 workspace
依赖升级计划，其中 Lestty 是唯一 normal/runtime app direct `gpui-base` consumer，现有应用继续使用完整
`gpui-component`；`app-theme` 仅有 projection-test dev edge。

## 目标

为快速、轻量的 Lestty 桌面终端确定可实施的终端核心、GPUI 集成边界、用户配置、终端优先主题格式、
平台透明材质与背景资源边界，并在同一 issue 文档中持续记录后续产品和架构讨论。

## 当前工作入口

| Scope | 文档 | 负责范围 |
| --- | --- | --- |
| Root hub | 本文 | Issue 状态、范围、适用性和 owner 导航 |
| 依赖升级前置计划 | [全 workspace 依赖升级计划](dependency-upgrade-plan.md) | 升级前 22/升级后 21 个 member + 独立工具、目标版本/SHA、本地 gpui-tokio 退役、lock 与三平台完成门 |
| Skill 同步前置计划 | [GPUI skill 同步计划](skill-sync-plan.md) | 上游 `gpui` 镜像、component docs 快照、repo-local component/base skill 与路由门禁 |
| 升级复用审计 | [a11y 与 Command 复用审计草稿](accessibility-and-command-reuse-audit.md) | GPUI/AccessKit 分层、Command 能力边界与 Jaco 搜索适配决定 |
| 全量上游复用 | [上游能力复用审计草稿](upstream-reuse-audit.md) | GPUI/component、Rig 与 registry 更新的 deletion-first 复用、保留和延后决定 |
| Lestty owner | [owner plan](../../../app/lestty/docs/dev/issue-205/README.md) | Lestty 本地文档入口与未来实施范围 |
| 讨论草稿 | [终端内核、配置与主题架构选型草稿](../../../app/lestty/docs/dev/issue-205/terminal-backend-selection-draft.md) | 选型证据、当前建议、待确认问题和讨论记录的唯一权威 |

## 前置升级 owner 导航

| Owner group | Canonical owner plans |
| --- | --- |
| Apps | [jaco](../../../app/jaco/docs/dev/issue-205/dependency-upgrade-plan.md)、[feiwen](../../../app/feiwen/docs/dev/issue-205/dependency-upgrade-plan.md)、[http-client](../../../app/http-client/docs/dev/issue-205/dependency-upgrade-plan.md)、[novel-download](../../../app/novel-download/docs/dev/issue-205/dependency-upgrade-plan.md)、[lestty](../../../app/lestty/docs/dev/issue-205/dependency-upgrade-plan.md) |
| Shared GPUI/UI crates | [app-assets](../../../crates/app-assets/docs/dev/issue-205/dependency-upgrade-plan.md)、[app-assets-macros](../../../crates/app-assets-macros/docs/dev/issue-205/dependency-upgrade-plan.md)、[app-theme](../../../crates/app-theme/docs/dev/issue-205/dependency-upgrade-plan.md)、[gpui-form](../../../crates/gpui-form/docs/dev/issue-205/dependency-upgrade-plan.md)、[gpui-form-gpui-component](../../../crates/gpui-form-gpui-component/docs/dev/issue-205/dependency-upgrade-plan.md)、[gpui-form-macros](../../../crates/gpui-form-macros/docs/dev/issue-205/dependency-upgrade-plan.md)、[gpui-operation](../../../crates/gpui-operation/docs/dev/issue-205/dependency-upgrade-plan.md)、[gpui-store](../../../crates/gpui-store/docs/dev/issue-205/dependency-upgrade-plan.md)、[gpui-tokio retirement](../../../crates/gpui-tokio/docs/dev/issue-205/dependency-upgrade-plan.md)、[platform-ext](../../../crates/platform-ext/docs/dev/issue-205/dependency-upgrade-plan.md)、[window-ext](../../../crates/window-ext/docs/dev/issue-205/dependency-upgrade-plan.md) |
| Product/runtime crates | [http-client-test-server](../../../crates/http-client-test-server/docs/dev/issue-205/dependency-upgrade-plan.md)、[jaco-agent](../../../crates/jaco-agent/docs/dev/issue-205/dependency-upgrade-plan.md)、[jaco-conversation](../../../crates/jaco-conversation/docs/dev/issue-205/dependency-upgrade-plan.md)、[jaco-core](../../../crates/jaco-core/docs/dev/issue-205/dependency-upgrade-plan.md)、[jaco-db](../../../crates/jaco-db/docs/dev/issue-205/dependency-upgrade-plan.md)、[xtask](../../../crates/xtask/docs/dev/issue-205/dependency-upgrade-plan.md) |
| Standalone tool | [mcp-auth-test-server](../../../tools/mcp-auth-test-server/docs/dev/issue-205/dependency-upgrade-plan.md) |

后续关于终端内核、PTY、渲染、输入、主题、窗口材质、背景资源和相关依赖的讨论统一更新到上述草稿；
本文不复制其结论。

## `0.1.0` 跨 owner 实施顺序（建议）

依赖升级前置不计入以下六个产品实施阶段。各阶段的详细范围、失败行为和验收门由
[终端选型草稿](../../../app/lestty/docs/dev/issue-205/terminal-backend-selection-draft.md#010-分阶段实施顺序)
维护；本表只拥有跨 owner 的顺序与交付结果。

| 阶段 | Owner | 可观察结果 | 依赖 |
| --- | --- | --- | --- |
| Phase 1 | `app/lestty` + `crates/xtask` | 可启动的 base-only app、正式 identity/icon、bundle metadata、当前平台首个 packaged app 与 README/docs 索引 | 依赖升级前置 |
| Phase 2 | `app/lestty` | 主题/配置/Settings、完整 state codec、默认 pane 与 configuration/themes 官方参考 | Phase 1 |
| Phase 3 | `app/lestty` | 基础 app menu、tabs/splits、placeholder panes、布局恢复与 shortcuts/session-restore 文档 | Phase 2 |
| Phase 4 | `crates/lestty-terminal` | UI-independent `alacritty_terminal 0.26.0` session/PTY/parser/snapshot/shutdown 与 crate README/rustdoc | Phase 3 |
| Phase 5 | `app/lestty` + `crates/lestty-terminal` | GPUI terminal surface 替换 placeholder，当前平台 packaged app 可运行真实 shell | Phase 4 |
| Phase 6 | root + 三个 owner | 协议/安全/错误/a11y/性能、官方文档及 macOS `.app`、Linux `.deb`、Windows `.msi` 达到 `0.1.0` 发布门 | Phase 5 |

Phase 1–3 不引入 Alacritty；Phase 4 不引入 GPUI。进入 Phase 4 前必须为新共享 crate 创建同 plan ID 的 owner
plan；Phase 1 必须把 Lestty packaging scope 加入 xtask owner plan，并在本 hub 的 plan map/owner 导航中登记。

## 适用性

| S-ID | Canonical surface | 状态 | 当前证据与草稿阶段决定 |
| --- | --- | --- | --- |
| `S-01` | Workspace、文件、模块与 owner | Applicable | `app/lestty` 拥有 GPUI/window/tabs/splits/config/state，新增 `crates/lestty-terminal` 拥有 UI-independent Alacritty adapter。 |
| `S-02` | GPUI 组件、布局与交互 | Applicable | 候选内核均不提供 GPUI renderer，Lestty 需要自有终端 `Element`；native theme 的 chrome 需投影到现有组件。 |
| `S-03` | Entity、Store、Global、identity 与 projections | Applicable | 终端状态、可绘制快照及窗口投影的 authority 尚待确定。 |
| `S-04` | Actions、events、subscriptions、focus 与 windows | Applicable | 键盘、鼠标、IME、焦点和窗口事件是终端输入边界。 |
| `S-05` | Async tasks、并发、取消与 shutdown | Applicable | PTY reader/writer、解析 event loop 和 shell shutdown 需要明确 owner。 |
| `S-06` | 数据获取与 Operation state | N/A | 当前范围不包含数据库、HTTP/provider 获取或 refresh/repair operation。 |
| `S-07` | Forms 与 editable state | Applicable | 配置草稿提出 Settings UI 与文本配置共享同一 typed authority；具体 controls/validation 尚待确认。 |
| `S-08` | 跨 crate、platform 与 external contracts | Applicable | 终端核心、PTY/ConPTY、shell、GPUI adapter，以及 Windows/macOS/Linux backdrop capability 构成边界。 |
| `S-09` | Error identity、传播、恢复与 error UI | Applicable | shell 启动、PTY、I/O、解析及 native build 失败尚待建模。 |
| `S-10` | 数据库、持久化与 migrations | Applicable | `config.toml` 是用户偏好 authority；独立 app-owned `state.toml` 原子保存窗口几何和 tabs/splits 恢复提示，不保存 live PTY/process/terminal content；没有数据库。 |
| `S-11` | Generated、synchronized、copied 或 vendored content | Applicable | Ghostty 候选会引入 Zig/native source 获取；`0.1.0` 已选 Alacritty，避免该 native 构建链。 |
| `S-12` | Icons 与 assets | Applicable | `0.1.0` 必须新增 canonical bundle icon 与可选 macOS `.icon/icon.json`，平台派生 icon 由 xtask 临时生成；`0.2.0` 候选背景图片和 native theme 文件另走 app-local/用户资源边界。 |
| `S-13` | Fluent i18n 与 bundle localization | Applicable | Settings UI、配置诊断与 apply-scope 提示将产生用户可见文案；精确 key 尚待实施计划。 |
| `S-14` | Security、privacy 与 credentials | Applicable | 剪贴板、OSC、shell 环境、theme 白名单、背景路径与拒绝运行时 URL 都需要固定信任边界。 |
| `S-15` | Observability 与 diagnostics | Applicable | PTY/shell 错误日志必须避免泄露命令、环境变量和终端内容。 |
| `S-16` | Packaging、platform behavior 与 CI/release | Applicable | 当前 CI 只有三平台 build/test、没有 bundle job；Phase 1 接入 `xtask bundle lestty` 并做宿主平台 smoke，Phase 6 必须实际验证 macOS `.app`、Linux `.deb`、Windows `.msi` 及签名范围。 |
| `S-17` | Dependencies、frameworks、Git sources 与 toolchains | Applicable | 先执行专门的全 workspace 依赖升级计划；Lestty 是唯一 normal/runtime app 直连 gpui-base，app-theme 仅保留测试 edge，现有应用保留 gpui-component。 |
| `S-18` | Owner documentation、indexes 与 ADRs | Applicable | dev plans 记录实施证据；`app/lestty/README.md`、官方 docs index/configuration/themes/shortcuts/session restore/platform/troubleshooting 及 shared-crate README/rustdoc 必须随阶段同步，不能由 dev 草稿替代。 |
| `S-19` | Validation 与 completion evidence | Applicable | 依赖确定后需要建立三平台构建与终端行为验证矩阵。 |

## Draft 退出条件

只有在依赖升级计划完成、讨论草稿中的材料选择得到用户确认，并补齐精确终端依赖、模块契约、工作包和
验收矩阵后，才能把本计划推进为 `Ready`。当前不得依据这份草稿开始生产实现。
