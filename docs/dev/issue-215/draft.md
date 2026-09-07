# Issue #215：依赖升级调查草稿

调查日期：2026-09-05。入口：[README.md](README.md)。

本文保存创建 issue 前及后续讨论的只读调查结果。版本来自当时的 crates.io 查询；上游可能继续发布，实施前刷新受影响证据。候选方案尚未经过锁文件解析、编译或运行验证。实施安排与待审阅问题已整理到 [实施计划](README.md)，本文保留调查背景，不作为独立执行指令。

## 1. 当前基线

- 仓库基线：`main` / `0dbe80e`。
- [根 Cargo.toml](../../../Cargo.toml) 声明 4 项直接 Git 依赖，以及 2 项指向 Zed 的 `[patch.crates-io]`。
- [Cargo.lock](../../../Cargo.lock) 锁定 Zed 为 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`，组件库为 `57a9903f48160845aabc8b92a1e2f5348c80d439`。
- 锁文件还有上游带入的字体、截图、输入法、proptest 等 Git 包。直接 Git 声明移除与完整依赖图 Git 来源归零需要分别验证。

## 2. 已核实的 GPUI Kit 发布事实

GPUI Kit 0.6.0 于 2026-09-03 发布。仓库已更名为 `longbridge/gpui-kit`；`gpui-component` 仍是带样式的组件层。新增 `gpui-kit` 统一入口，资源包更名为 `gpui-kit-assets`。

`gpui-kit 0.6.0` 声明 `gpui-pre-* 0.3.1` 系列兼容要求；后续查询确认配套发布包已到 `0.3.3`，本计划候选更新为 `0.3.3`。它们是 Longbridge 从 Zed 源码发布的快照，内部包使用匹配版本；不应将旧 `gpui 0.2.2` 的版本字符串视为同等源码或兼容性证据。旧宏 `0.3.1` 的非 inspector release 问题已在后续版本修复，解析时需要确认 macros 也使用正确配套版本，不能仅更新顶层包。

| 当前声明/锁定状态 | 已发布替代候选 |
| --- | --- |
| `gpui`：Zed Git，版本标识 `0.2.2` | `gpui-pre 0.3.3` |
| `gpui_platform`：Zed Git | `gpui-pre-platform 0.3.3` |
| `gpui-component`：Git，版本标识 `0.5.2` | `gpui-component 0.6.0` |
| `gpui-component-assets`：Git，版本标识 `0.5.1` | `gpui-kit-assets 0.6.0` |
| `gpui` / `gpui-macros` 的 Git patch | 统一来源后删除 |

来源：[0.6.0 发布说明](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.0)、[发布包](https://docs.rs/crate/gpui-kit/0.6.0)、[发布机制](https://github.com/longbridge/gpui-kit/blob/main/CONTRIBUTING.md)、[组件包依赖](https://crates.io/api/v1/crates/gpui-component/0.6.0/dependencies)、[GPUI 快照依赖](https://crates.io/api/v1/crates/gpui-pre/0.3.1/dependencies)、[平台包依赖](https://crates.io/api/v1/crates/gpui-pre-platform/0.3.1/dependencies)。

## 3. 候选方向，待确认

- 应用层使用 `gpui-kit 0.6.0` 统一入口。
- 仅需要 GPUI 的共享 crate 可直接依赖 `gpui-pre`，避免引入整个组件层。
- workspace 统一 GPUI 来源，移除旧 patch，避免 Git GPUI 与发布包 GPUI 类型并存。
- 先处理 GPUI Kit 迁移与 Git 来源替换，再处理普通依赖；MCP、Windows 和 SQLite 分别核对配套约束。

当前 4 项直接 Git 依赖和 2 项 Git patch 均有发布包替代路径。完整图中是否仍有 Git 来源，需要实际更新锁文件后确认；本轮没有执行依赖解析。

## 4. 已找到的本地迁移点

| 本地文件/调用 | 上游变化与待实施方向 |
| --- | --- |
| [HTTP 请求体编辑器](../../../app/http-client/src/features/request/body/http_text.rs)：`InputState.multi_line().code_editor()` | 迁到 `EditorState` / `Editor` |
| [HTTP 响应编辑器](../../../app/http-client/src/features/request/response.rs)：`code_editor()` | 迁到独立 Editor API |
| [Jaco 提示词输入](../../../app/jaco/src/features/settings/prompts/dialog.rs)：多行 `InputState` | 迁到 `TextareaState` / `Textarea`，同步相关交互与测试 |
| [共享资源](../../../crates/app-assets/src/lib.rs)、[Jaco 资源](../../../app/jaco/src/foundation/assets.rs)及应用入口 | 调整 `gpui_component_assets::Assets` 与初始化引用 |

发布说明还涉及 Dock、History、Dialog、主题、选择和其他 API 变化，尚未完成本地适用性调查。当前基线来自 Git 开发版本，已包含部分新 API；不能机械套用全部 0.5.0 → 0.6.0 迁移条目。正式计划需比较锁定提交到目标版本的实际差异。

## 5. 需要配套处理的版本候选

| 包 | 当前 → 最新稳定版 | 已知约束 |
| --- | --- | --- |
| `rmcp` | `2.2.0 → 3.2.0` | `rig-agent 0.42.0` 运行时依赖 `rmcp ^2`，需要协调跨 crate 类型与集成边界 |
| `libsqlite3-sys` | `0.37.0 → 0.38.2` | `diesel 2.3.13` 接受 `>=0.17.2, <0.39.0`；仍需核对整体图与 `bundled-windows` |
| `windows-bindgen` | `0.66.0 → 0.100.0` | 涉及 platform-ext 生成绑定；新版本声明 Rust 1.95 |
| `windows-core` | `0.62.2 → 0.100.0` | 最新 `windows 0.62.2` 仍要求 `windows-core ^0.62.2` |
| `windows-future` | `0.3.2 → 0.100.0` | 最新 `windows 0.62.2` 仍要求 `windows-future ^0.3.2` |

来源：[rig-agent 依赖](https://crates.io/api/v1/crates/rig-agent/0.42.0/dependencies)、[Diesel 依赖](https://crates.io/api/v1/crates/diesel/2.3.13/dependencies)、[Windows 依赖](https://crates.io/api/v1/crates/windows/0.62.2/dependencies)。

## 6. 其他落后的直接依赖

以下是 workspace 成员（含 target/dev/build 依赖）当时声明与锁定的直接依赖版本，和 crates.io 最新稳定版本的比较。未对所有间接包做最新版盘点，也未逐项完成 changelog 或兼容性审阅。

| 包 | 当前 → 最新稳定版 |
| --- | --- |
| `async-compression` | `0.4.42 → 0.4.44` |
| `async-trait` | `0.1.91 → 0.1.92` |
| `base64` | `0.23.0 → 0.23.1` |
| `bytemuck` | `1.25.0 → 1.25.2` |
| `bytes` | `1.12.0 → 1.12.1` |
| `clap` | `4.6.4 → 4.6.6` |
| `diesel` | `2.3.11 → 2.3.13` |
| `futures`、`futures-util` | `0.3.33 → 0.3.34` |
| `globset` | `0.4.19 → 0.4.20` |
| `http` | `1.4.2 → 1.5.0` |
| `http-body-util` | `0.1.3 → 0.1.5` |
| `hyper` | `1.10.1 → 1.11.1` |
| `ignore` | `0.4.31 → 0.4.33` |
| `similar` | `3.1.1 → 3.2.0` |
| `syn` | `3.0.3 → 3.0.5` |
| `thiserror` | `2.0.19 → 2.0.20` |
| `time` | `0.3.54 → 0.3.55` |
| `toml` | `1.1.4+spec-1.1.0 → 1.1.5+spec-1.1.0` |
| `trybuild` | `1.0.118 → 1.0.120` |
| `uuid` | `1.24.0 → 1.26.0` |
| `which` | `8.0.5 → 8.0.6` |
| `xcap` | `0.9.7 → 0.9.8` |

版本来源为 [crates.io API](https://crates.io/api/v1/crates/async-compression)，每项使用 `/api/v1/crates/<package>` 的 `max_stable_version` 查询，当前版本来自本地 manifests / lockfile。

`rig 0.42.0`、`reqwest 0.13.4`、`tokio 1.53.1`、`serde 1.0.229`、`serde_json 1.0.151`、`garde 0.23.0`、`duckdb 1.10505.0` 等在调查时已是最新稳定版本。

## 7. 后续形成计划所需内容

- 确认 facade 与共享 crate 的依赖边界、最终版本和 feature；补齐受影响 owner 文档。
- 核对从锁定提交到发布目标的 API 差异、迁移指南和已使用行为。
- 确认 MCP / Windows 的保留或迁移方案及 SQLite 图约束。
- 核对相关 skill、开发文档、生成绑定和资源的耦合范围。
- 拆出具体工作包与关键不变量验证，记录尚未验证的平台和 UI 边界。

本次调查与文档建立均未运行 `cargo upgrade`、`cargo update`、构建或测试。上述事项是后续计划待完善内容，不代表已经完成或当前已授权开始实施。

## 8. Issue #205 终端分支的已有升级工作

2026-09-05 已通过 GitHub API 确认远端分支 `codex/205-add-lestty-terminal` 的 HEAD 为
`9c8d1e5fae5a8f5857403dd22823d70ef4a1cc3f`。相对本轮 `main` 基线，该分支有 3 个独有提交，同时缺少 main
后来的 29 个提交。用户要求将其作为 #215 的参考资料；尚未合并或移植。

| 提交 | 内容 |
| --- | --- |
| `c6f7bf8` | `chore(deps): upgrade workspace dependencies`，依赖升级及配套源码迁移 |
| `493213a` | `docs: add application readmes`，应用文档 |
| `9c8d1e5` | `docs: add dependency upgrade plans`，依赖升级、skill 同步和上游复用计划 |

| 已有工作 | #215 的参考价值与边界 |
| --- | --- |
| Input / Textarea 拆分、`FormTextarea` 及回归测试 | 可提取输入控件与表单适配的迁移思路；仍需按 0.6.0 API 调整 |
| 本地 `gpui-tokio` 退役，改用上游 `gpui_tokio` | 提供减少自有实现的候选；旧方案新增 Zed Git edge，发布包是否覆盖同一能力尚未核实 |
| Windows 绑定生成使用 `--no-allow` | 替换生成后对 lint 属性的字符串裁剪；需匹配选定 bindgen 版本 |
| Diesel / SQLite 原子升级 | 参考 `bundled-windows`、唯一 SQLite links owner 约束 |
| 主 workspace RMCP 2 与独立工具 RMCP 3 | 区分 Rust 类型兼容与跨进程协议兼容；独立工具及其 lockfile也要纳入后续范围 |
| GPUI skill 镜像、组件文档快照、repo-local 规则与 provenance | 参考同步边界和方法，目标源码及文档目录须按新版本刷新 |
| Command、主题及其他上游复用 | 作为候选线索，逐项判断是否属于 #215；不自动继承旧分支所有产品调整 |

参考文档均固定在上述提交：

- [依赖升级计划](https://github.com/suxiaoshao/gpui/blob/9c8d1e5fae5a8f5857403dd22823d70ef4a1cc3f/docs/dev/issue-205/dependency-upgrade-plan.md)
- [Skill 同步计划](https://github.com/suxiaoshao/gpui/blob/9c8d1e5fae5a8f5857403dd22823d70ef4a1cc3f/docs/dev/issue-205/skill-sync-plan.md)
- [上游复用调查](https://github.com/suxiaoshao/gpui/blob/9c8d1e5fae5a8f5857403dd22823d70ef4a1cc3f/docs/dev/issue-205/upstream-reuse-audit.md)
- [a11y 与 Command 调查](https://github.com/suxiaoshao/gpui/blob/9c8d1e5fae5a8f5857403dd22823d70ef4a1cc3f/docs/dev/issue-205/accessibility-and-command-reuse-audit.md)

旧目标是 Zed `e0931d5a9dbf4f781b336fdf448739e74a2ac0b5` 与组件库
`5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3` 的 Git 组合，不能直接作为 #215 去 Git 化的最终依赖声明。
旧分支还包含历史 patch；是否保留必须按当前依赖证据重新判断。

移植以具体改动为单位，保留 main 后续修复。例如 Jaco 提示词对话框的直接分支对比中，旧分支缺少当前
main 的保存中关闭保护；提取 Textarea 迁移时不能整文件覆盖而丢掉该行为。

旧文档记录 Windows / Rust 1.97.1 的 workspace build、test、Clippy、独立工具检查及四应用 MSI 打包通过，
并明确 macOS/Linux、MCP 跨版本 E2E 和人工 UI/a11y 检查未完成。这些是旧分支的历史记录，本轮未重新执行，
不得写成 #215 的验证结果。

## 9. 内存 SVG 与 gpui-lucide 候选设计

### 当前决定

用户已确认：本轮保留 `app-assets` 与 `app-assets-macros` 的现有图标声明、路径转换和资源注册方式，暂不
实施 `gpui-lucide` 或删除这两个 crate。升级所必需的上游包名/API 调整仍按依赖迁移处理。

上游接口请求已提交为 [gpui-kit #2961](https://github.com/longbridge/gpui-kit/issues/2961)。下文保留调查
证据与未来候选方向；只有相关接口可用且再次确认迁移范围后，才推进图标体系替换。

### 用户提出的方向

利用 GPUI 的 SVG 字节输入，新增 `gpui-lucide`，提供类似 React 图标库的按名称导入体验；将 Lucide 图标
从路径注册切换为独立 Rust 项，利用编译器和链接器消除未使用内容，并评估删除 `app-assets` 与
`app-assets-macros`（实际 crate 名为复数）。本轮只确认可行性并记入草稿，尚未实施。

### 已核实接口与限制

- 已直接读取 crates.io 的 `gpui-pre 0.3.1` 发布归档中的 `src/elements/svg.rs`：存在
  `pub fn data(mut self, data: &[u8]) -> Self`，可使用 `svg().data(include_bytes!(...))`，无需该图标的
  `AssetSource` 路径注册。数据是 SVG 源文档的字节，仍由运行时解析/渲染。
- 该版本的 `.data()` 对内容计算 hash 并构建缓存标识，再通过 `Arc::from(data)` 保存数据。因此编译期嵌入
  不等于零拷贝，也不等于 SVG 已在编译期解析为绘制数据。
- `gpui-component 0.6.0` 的 `IconNamed` 只定义 `path()`，`Icon` 只公开路径设置；同时核对的上游 main
  `9db6ac7fd7c5f8d35dc850cd532f4d13d1142d33` 仍是该接口。
- `Button::icon` 经 `ButtonIconVariant` 接受 `Icon`、`Spinner`、`ProgressCircle`；任意 `Svg` /
  `IntoElement` 不能直接成为该插槽的图标。自有图标可以作为普通元素渲染，但要维持
  `.icon(...)` 的一致接入体验，需要组件 Icon 增加数据源能力或其他经确认的接口支持；这是当前配套缺口。

来源：[gpui-pre 0.3.1 发布源码](https://docs.rs/crate/gpui-pre/0.3.1/source/src/elements/svg.rs)、
[发布归档](https://static.crates.io/crates/gpui-pre/gpui-pre-0.3.1.crate)、
[Icon 接口](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/icon.rs)、
[ButtonIcon 接口](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/button/button_icon.rs)。

### 按需导入与裁剪的候选实现原则

- 每个图标导出独立类型、函数或常量，分别引用静态 SVG 数据；例如 `use gpui_lucide::{Search, Settings};`。
  这是候选 API 示意，具体构造与组件转换接口尚未确定。
- Lucide 源文件到 Rust 导出的生成逻辑可集中在新 crate 的生成流程中，不需要调用方先声明 app-local
  `IconName` 枚举或使用 proc macro；保留来源版本、图标名称映射和许可证。
- 不在正常渲染路径引入引用全量图标的注册表、字符串查找或全量枚举分发表；否则所有数据都可能保持可达，
  阻碍按使用情况裁剪。`use` 语句本身不决定资源保留，实际可达引用才是关键。
- Rust 默认启用死代码移除，当前 release 配置还有 thin LTO。独立引用结构有利于消除未使用图标，但不能把
  最终二进制体积、跨平台裁剪比例或编译时间收益作为语言保证；后续实现时用 release 样例验证仅使用少数
  图标时的产物。不将本轮未执行的大小检查写成已验证结论。

来源：[include_bytes!](https://doc.rust-lang.org/std/macro.include_bytes.html)、
[rustc link-dead-code](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-dead-code)、
[当前 profile](../../../Cargo.toml)。

### 删除旧 crate 的条件与资源边界

| 当前职责 | 候选落点 / 需要保留的能力 |
| --- | --- |
| Lucide 枚举、路径与 AssetSource 宏生成 | `gpui-lucide` 独立导出和静态 SVG 数据 |
| Jaco `ProviderLogoName` / `define_svg_icons!` | app-local provider logo 数据与映射；不放入 Lucide 专用库 |
| `SvgIconMetadata` 来源信息 | 在 app-local 元数据或资源来源文档中保留对应信息 |
| Feiwen 的 `AppAssets<LucideAssets>` 组合 | Lucide 无需注册后移除该组合；组件内置图标依赖仍需满足 |
| Jaco 主题 JSON、应用图像、组件默认图标 | 保留其所需 app-owned 资源加载；与 Lucide 图标的字节接入分别处理 |

现有 `app-assets-macros` 已用 `include_bytes!`，但通过运行时路径 `match` 和 `list()` 注册整组声明的图标。
新设计的主要简化是移除这层 app-local 声明和路径查找，而非首次实现资源编译期嵌入。

`app-assets` 与 `app-assets-macros` 都有删除路径，前提是全部调用方职责完成迁移且组件图标插槽问题得到
解决。`gpui-kit-assets` 中组件自身使用的默认图标不因新增 `gpui-lucide` 自动消失；不能据此承诺移除应用
全部 `AssetSource` 或默认资源。最终选择在实施计划中确定。

## 10. Message 系列组件采用建议

用户要求保存本轮选型结论。已对照 Jaco 消息展示源码，以及本地 gpui-kit `928c3eb7` 的组件文档、
接口和 Message story；核对的五个组件源码与 `v0.6.0` 无差异。以下为采用建议，尚未实施或进行 UI 验证。

| 上游组件 | 当前 Jaco 对应部分 | 建议与保留边界 |
| --- | --- | --- |
| `Message` / `MessageContent` / `MessageFooter` | 用户消息、助手回复的对齐、正文与操作区 | 采用通用布局；保留消息投影、运行分组、时间格式化、复制及用量逻辑 |
| `Bubble` | 用户消息背景、边框、宽度和正文容器 | 采用展示外壳；助手可使用 `Ghost` 保持无背景效果，具体尺寸与样式需匹配现有界面 |
| `Attachment` | 文件卡片、图片预览外壳、附件操作区 | 采用展示结构；保留附件身份、可用性检查、访问控制、预览和操作回调 |
| `Marker` | 运行状态与时间线提示 | 用于普通状态行；工具展开和审批操作保留自身交互语义 |
| `MessageScroller` | 虚拟列表、流式更新重测与滚动跟随 | 建议单独迁移；保持稳定行映射、流式增长、展开详情、向上阅读和定位消息行为 |

`MessageContent` 接受任意元素，可以承载现有 `TextView`、工具调用详情、审批按钮和用量详情。
`MessageScroller` 接受应用提供的行渲染器，提供 `splice`、局部重测、尾部跟随和跳转；应用继续拥有消息
数据、稳定 ID 与行索引映射，不把 Jaco 业务状态转移到组件内。

建议先迁消息与附件展示，再单独迁滚动层。此顺序用于隔离视觉调整和滚动行为变化，尚未拆为正式工作包。
不因采用新组件额外增加头像、反应按钮、未读状态或其他尚未要求的产品功能。

本地依据：[消息行](../../../app/jaco/src/components/chat/detail/message.rs)、
[消息投影](../../../app/jaco/src/components/chat/detail/timeline.rs)、
[附件展示](../../../app/jaco/src/components/chat/detail/attachments.rs)、
[会话详情与列表](../../../app/jaco/src/components/chat/detail.rs)。

上游依据：[Message](https://gpui-kit.com/docs/components/message)、
[Bubble](https://gpui-kit.com/docs/components/bubble)、
[Attachment](https://gpui-kit.com/docs/components/attachment)、
[Marker](https://gpui-kit.com/docs/components/marker)、
[MessageScroller](https://gpui-kit.com/docs/components/message-scroller)。

## 11. Shimmer 使用建议

用户提到的 `simmer` 按上游 `Shimmer` 理解。已核对上游文档和 Jaco 当前状态展示；用户同意将以下结论
保存进草稿，尚未实现。

| 场景 | 建议 |
| --- | --- |
| 助手实际执行中的“处理中 · 耗时” | 优先使用 `ShimmerText`；采用 `Marker` 时可启用 `MarkerLoadingStyle::Shimmer` |
| 附件“正在检查可用性” | 可使用轻量 Shimmer，但与当前 Spinner 二选一，避免同一状态重复动画 |
| 附件上传或处理中的标题 | 采用 `Attachment` 后可利用其 `Uploading` / `Processing` 状态的标题 Shimmer；只映射真实存在的业务状态 |
| 等待用户批准工具调用 | 保持静态提示并突出审批操作，不表现为系统正在执行 |
| 已完成、失败、取消和消息正文 | 保持静态；流式正文不做整段 Shimmer |

最明确的候选接入点是 `AgentTurnRow::render_status_row`，目前用普通 `Label` 展示
`conversation-agent-processing`。当前分支按 run 是否终止选择文案，接入动画前必须进一步区分实际执行与
等待审批；不能把所有未终止 run 都视为动画开启条件。状态、耗时和文案继续由 Jaco 拥有。

`ShimmerText` 支持主题适配，reduced motion 下保留静态文字。与 `Marker` 组合时优先通过
`MarkerContent::text(...)` 提供文字；组件本身不拥有运行状态，也不自动完成辅助技术的进度播报。

本地依据：[助手状态行](../../../app/jaco/src/components/chat/detail/message.rs)、
[附件检查状态](../../../app/jaco/src/components/chat/detail/attachments.rs)。
上游依据：[Shimmer 文档](https://gpui-kit.com/docs/components/shimmer)。

## 12. Skill 与本地文档配套升级

用户确认：本次依赖升级必须包含从 gpui-kit 仓库复制的官方 skill、组件文档，以及受影响的本地使用规则。
本节记录同步范围，当前尚未替换 skill 或文档快照；整体仍处于 Draft 阶段。

### 已核实的来源与所有权

| 本地资料 | 当前来源/所有权 | 新版上游对应位置 |
| --- | --- | --- |
| `.agents/skills/gpui/SKILL.md` 与 `references/` | 从上游 GPUI skill 引入的框架资料 | `skills/gpui-kit/SKILL.md` 与 `skills/gpui-kit/references/gpui/`；入口已整合，不能只照搬旧目录 |
| `.agents/skills/gpui-component-usage/references/components/` 中的组件正文 | 官方网站 Markdown 快照，当前 attribution 记录旧路径 `docs/docs/components/*.md` 与提交 `57a9903f48160845aabc8b92a1e2f5348c80d439` | `website/docs/components/*.md` |
| `gpui-component-usage/SKILL.md`、组件 `index.md`、`references/rules/` | 本仓库维护的组件选择与应用使用规则，独立于官方快照 | 按选定 API 和文档同步适配，保留本地所有权约束 |
| `references/third-party/gpui-component-docs.md` 与许可证副本 | 官方文档来源与许可记录 | 更新仓库名、上游路径、固定提交与对应许可信息 |

已在本地上游 checkout 核对 `v0.6.0`，对应提交
`94a313a72a2513aee2780240cd322d552b2395f0`，其官方 skill 目录为 `skills/gpui-kit` 与
`skills/gpui-kit-design-guides`。本地 skill 最终如何命名、拆分和路由，需要在实施计划中明确；本节不预先
把两个新 skill 全量安装视为已确认方案。

### 纳入本次升级的工作

1. 按最终依赖目标固定官方 skill 和文档的来源提交。区分已发布版本与上游 main；尚未发布的接口不能
   混入本地资料后被描述成当前依赖已具备的能力。
2. 同步官方 GPUI references 和组件文档的完整目标集合，包含新增、修改、重命名与删除；官方正文保持
   上游快照内容，本地适配规则单独维护。同步范围包括 Message、MessageScroller、Shimmer、Textarea、
   Editor 等新增或变化的组件资料。
3. 同步本地组件选择索引、skill 导航、在线文档地址与来源/许可记录；根据上游新入口布局处理相对链接，
   不保留指向旧目录或已删除文档的路由。
4. 只调整受此次 API/包名迁移影响的 repo-local skill、`AGENTS.md` 和应用/crate 文档：例如 GPUI 来源、
   组件导入、初始化、Editor/Textarea、资源注册。保留已确认的表单/状态所有权以及现有 assets 方案，
   不用上游通用示例覆盖本仓库的明确约束。
5. 实施时核对目标集合与官方正文一致性、来源提交、相对链接和旧 API/路径残留；若涉及已有同步工具或
   hash 记录，使用核实过的入口与算法，不手写推测值。文档/skill 同步本身不触发 Rust 全量门禁。

上游依据：[官方 skill 入口](https://github.com/longbridge/gpui-kit/tree/94a313a72a2513aee2780240cd322d552b2395f0/skills)、
[组件文档源](https://github.com/longbridge/gpui-kit/tree/94a313a72a2513aee2780240cd322d552b2395f0/website/docs/components)。

## 13. 后续版本与遗漏检查补充

以下调查已进入实施计划的证据、待审阅问题和工作包，不表示已实施。

- `gpui-pre 0.3.3` 发布元数据对应 Zed `5b055fa789a8b8d38ac951a6e0cde272f66b4495`；`0.3.1` 的 SVG 源码调查仍作为当时的历史证据保留。
- 终端分支目标之后新增 Message/Shimmer，TextView 移至 gpui-base，Dock/History 与官方 skill 布局变化；旧分支明确延后的滚动/复制等工作不应算作遗漏。SVG 字节接口在旧终端分支目标中已存在。
- `0.6.0` 后的 TextView 文档替换同 block 数高度重测、Markdown 硬换行和软换行修复尚不能算作 `0.6.0` 已发布能力；是否等待下一发布由根计划 Q-05 处理。
- 当前没有 `gpui-pre-tokio` 发布包；旧分支的 Git bridge 方案不能原样用于去 Git 化。旧分支已有 panic、Task drop abort、外部 handle 三类回归，可复用其不变量。
- 当前 registry 的 `arrayref 0.3.9` 未被撤回，不直接继承旧分支的 Git patch；不据此否定未经重新核实的历史状态。
- 保留 assets 设计仍需解决宏生成的 `::gpui` / `::gpui_component` 绝对路径；单一 `gpui-kit` 依赖不能自动保证这些路径解析。
- root Cargo 的包级优化应匹配真实 `gpui-pre` 包名；Jaco basic/full 与 HTTP Client 全语言 features、测试支持、平台 features 要分别保留。
- workspace debug/全 features 通过不代表单应用默认 features 的 release 打包通过；当前 CI 未覆盖 release bundle，独立 MCP 工具也不在 root workspace 检查范围内。
- `gpui-kit::init` 与本地主题初始化需保持顺序；`gpui_kit::*` 在 test-support 下引入的 test 属性不能混淆普通 Rust 测试。

来源与执行门统一见 [根计划](README.md) 的 E-02–E-06、Q-01–Q-08 和 R/T 表。
