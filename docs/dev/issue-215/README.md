# Issue #215：GPUI Kit 与依赖升级实施计划

## 状态与范围

- 状态：实施及本地 PR 验证完成；2026-09-07 用户授权提交全部改动并创建 PR。Windows/Linux 和远程 CI 待确认，手工 UI 边界见实施记录。
- Issue：[#215](https://github.com/suxiaoshao/gpui/issues/215)，已确认 OPEN。
- Plan ID：`issue-215`；分支：`codex/215-gpui-kit-dependency-upgrade`。
- 基线：`main@0dbe80e`；证据日期：2026-09-05。
- 本轮交付：按已确认范围实施依赖/API/上游复用与资料同步，记录实际验证和未验证平台边界。
- 依赖/API/资料实施与验证记录：[实施记录](implementation.md)。用户已授权提交和 PR；WP-07 本地检查通过，远程三平台 CI 待确认。
- [调查草稿](draft.md) 保存历史调查；本计划的候选与决定优先于旧调查条目。未决项以 Q-ID 为准。
- 2026-09-07 用户授权按计划实施；技术映射和实际结果见实施记录，平台未验证内容保留标记。

### 目标

统一 workspace GPUI 类型来源，采用发布包替代可移除的 Git 依赖与 patch；完成必要 API、feature、宏、
资源、开发资料与构建配置迁移。普通依赖按配套约束更新，保留有证据的限制。
同时复用目标版本已覆盖的通用交互，删除重复实现；必要功能与正确性优先，不要求复刻现有 UI/UX。
Message/Shimmer 归 WP-04，其余控件复用归 WP-03；具体删除边界见 D 表与 owner 计划。

### 非目标与已确认边界

- 保留 `app-assets`、`app-assets-macros` 的图标声明和运行时资源设计；本轮不新增 gpui-lucide。
- 不整分支合并 #205，不覆盖 main 后续保存中关闭保护等修复。
- 不重写 ComposerEditor、审批/附件安全访问、业务消息投影、Store/Operation/Form 核心架构。
- 不增加数据库兼容迁移，不清理用户数据库、凭据或真实附件；依赖升级本身不授权数据删除。
- 不自动启用 Web、Shell、WebView，不导入旧分支与升级无关的产品调整。
- 本次实施执行受影响验证；WP-07 的三平台、全部 release 和最终 CI 门禁留待用户要求最终验收或 PR。

## 待审阅问题与执行阻断

| ID | 问题与决定状态（未确认项显式标记） | 阻断范围 |
| --- | --- | --- |
| Q-01 | 已确认：应用使用 gpui-kit，由 app-assets 提供供宏使用的隐藏类型 re-export；生成代码统一经 ::app_assets::__private 引用所需 GPUI/组件类型，不要求应用另加底层依赖或逐模块 import。保持现有宏调用和图标资源设计 | 方案已确认，落实精确导出与宏展开测试 |
| Q-02 | 已确认：本轮保留本地 gpui-tokio，仅迁移 GPUI 来源；不采用 #205 的 Zed Git bridge | 已解除方案阻断 |
| Q-03 | 已确认：独立 MCP 测试服务不升级；与主 workspace 一起保留 RMCP 2.2.0，Rig 保留 0.42.0 | 不进行 RMCP 3 迁移或 2/3 跨版本 E2E |
| Q-04 | 已确认迁移 MessageScroller 并采用上游跟随行为：用户向上阅读时暂停自动跟随，滚回底部后恢复；不再要求等下一次提交才恢复 | 行为选择已确认 |
| Q-05 | 已确认使用组件库 0.6.0，不等待下一发布；未发布 TextView 修复仍作为已知风险，遇到阻断再报告，不默默改回 Git | 版本选择已确认 |
| Q-06 | 已确认保留 gpui 与 gpui-component-usage 的职责分离，按新版上游目录同步资料；不整体替换为上游统一 skill | 目录细化属于后续技术工作 |
| Q-07 | 已确认采用 Message、Bubble、Attachment、Marker，实际处理中使用 Shimmer；附件检查动画未额外确认，不作为必做 | 展示采用范围已确认 |
| Q-08 | 已授权同步 GitHub issue 的版本、范围和本轮决定 | 同步结果见完成记录 |
| Q-09 | 2026-09-06 用户要求按必要功能检查上游替代机会，并授权补充计划；接受上游 UI/UX 与默认行为差异。D-07–D-13 分别记录已选迁移和仍待设计的候选 | 2026-09-07 已另行授权实施；具体技术映射见 jaco/L-02–L-08 和实施记录 |

技术映射已在实施中按固定发布源码核实：依赖最终解析、Windows 生成器兼容限制、Form 状态与事件、
Composer 事务及动画生命周期均记录在 owner 文档。Windows/Linux 实机证据仍待执行。

## 变更影响摘要

| 面向 | 影响 |
| --- | --- |
| Workspace/依赖 | [Modify, Cross-owner] 四应用及下表共享 crate 的声明/feature，root lock 与独立工具 lock 分开处理 |
| 公共契约 | [Modify] 新 FormTextarea/FormEditor 适配器；[Breaking] Q-01 确认后的宏路径；GPUI 类型保持单一来源 |
| 状态与持久化 | [No change] 业务状态和数据库 schema 不迁移；编辑器只替换原生控制层 |
| 平台与发布 | [Modify] 包级优化名、Windows 生成方式与配套版本；检查单应用 release 和资源加载 |
| 同步资料 | [Modify, Move, Delete] 官方 skill/docs 集合、导航、来源/许可及受影响本地规则；按 Q-06 保留两类本地 skill 职责 |
| 产品交互 | [Modify] Message/Shimmer、MessageScroller、Command、列表 Picker、菜单/标题栏及局部编辑器通用行为；按 Q-04/Q-09 采用上游交互，保留业务与安全约束 |
| 安全/数据删除 | 无新增权限、真实数据清理或凭据迁移 |

## 文档地图与工作包顺序

根拥有 E/D/C/R/T/WP 公共编号；owner 使用以 crate 名限定的局部 F/L/ST/G 编号，不互相定义实现。
各 owner 只记录自己的迁移，公共版本表和问题只在根/调查证据中维护。

| Owner | 计划 | 工作包 |
| --- | --- | --- |
| app/jaco | [jaco](../../../app/jaco/docs/dev/issue-215/README.md) | WP-03、WP-04 |
| app/http-client | [http-client](../../../app/http-client/docs/dev/issue-215/README.md) | WP-03 |
| app/feiwen | [feiwen](../../../app/feiwen/docs/dev/issue-215/README.md) | WP-03、WP-05 |
| app/novel-download | [novel-download](../../../app/novel-download/docs/dev/issue-215/README.md) | WP-03、WP-05 |
| crates/app-assets | [app-assets](../../../crates/app-assets/docs/dev/issue-215/README.md) | WP-02 |
| crates/app-assets-macros | [app-assets-macros](../../../crates/app-assets-macros/docs/dev/issue-215/README.md) | WP-02、WP-05 |
| crates/app-theme | [app-theme](../../../crates/app-theme/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-form | [gpui-form](../../../crates/gpui-form/docs/dev/issue-215/README.md) | WP-02、WP-05 |
| crates/gpui-form-gpui-component | [gpui-form-gpui-component](../../../crates/gpui-form-gpui-component/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-form-macros | [gpui-form-macros](../../../crates/gpui-form-macros/docs/dev/issue-215/README.md) | WP-05 |
| crates/gpui-heatmap | [gpui-heatmap](../../../crates/gpui-heatmap/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-operation | [gpui-operation](../../../crates/gpui-operation/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-store | [gpui-store](../../../crates/gpui-store/docs/dev/issue-215/README.md) | WP-02 |
| crates/gpui-tokio | [gpui-tokio](../../../crates/gpui-tokio/docs/dev/issue-215/README.md) | WP-02 |
| crates/http-client-test-server | [http-client-test-server](../../../crates/http-client-test-server/docs/dev/issue-215/README.md) | WP-05 |
| crates/jaco-agent | [jaco-agent](../../../crates/jaco-agent/docs/dev/issue-215/README.md) | WP-05 |
| crates/jaco-conversation | [jaco-conversation](../../../crates/jaco-conversation/docs/dev/issue-215/README.md) | WP-05 |
| crates/jaco-core | [jaco-core](../../../crates/jaco-core/docs/dev/issue-215/README.md) | WP-05 |
| crates/jaco-db | [jaco-db](../../../crates/jaco-db/docs/dev/issue-215/README.md) | WP-05 |
| crates/platform-ext | [platform-ext](../../../crates/platform-ext/docs/dev/issue-215/README.md) | WP-05 |
| crates/window-ext | [window-ext](../../../crates/window-ext/docs/dev/issue-215/README.md) | WP-02、WP-05 |
| crates/xtask | [xtask](../../../crates/xtask/docs/dev/issue-215/README.md) | WP-05、WP-07 |
| tools/mcp-auth-test-server | [mcp-auth-test-server](../../../tools/mcp-auth-test-server/docs/dev/issue-215/README.md) | WP-05 |

执行顺序：

1. WP-00：已固定 Q 项与技术映射；以批准范围和固定版本作为实施依据。
2. WP-01：root GPUI 发布包与 feature/宏边界固定，依赖图迁移。
3. WP-02：共享 GPUI、资源、表单和运行时适配；先生产方后应用消费者。
4. WP-03：四应用必要迁移及 D-07–D-13 上游复用；保留必要功能与 main 后续修复，允许 Q-09 确认的交互变化。
5. WP-04：确认后的 Jaco Message/Shimmer 展示，TextView 与滚动独立受门控。
6. WP-05：普通依赖、SQLite/Windows/MCP 配套升级，可与不相交迁移独立推进。
7. WP-06：按实际目标同步 skill/docs 和 owner 文档，不提前混入未发布 API。
8. WP-07：用户要求 PR/最终验收时，执行一次最终门禁并记录平台/手工边界。

## 系统适用性

| ID | 状态 | 当前证据与本轮契约 |
| --- | --- | --- |
| S-01 Workspace/owner | Applicable | Cargo.toml 成员与各 owner manifests；只改受影响边界 |
| S-02 UI/无障碍 | Applicable | Jaco detail、HTTP 编辑器；R-03/R-04，展示控件保留键盘/可访问语义 |
| S-03 状态/身份 | Applicable | Form 绑定、TextViewState、时间线行；C-02/C-04，稳定身份不能变成下标 |
| S-04 焦点/窗口 | Applicable | app 初始化、prompt dialog、Editor；保存中关闭保护与 IME 保留 |
| S-05 异步/退出 | Applicable | gpui-tokio/src/lib.rs；C-03，不修改任务权限/业务取消策略 |
| S-06 数据获取/Operation | No change | gpui-operation 核心无运行时 GPUI 依赖，仅 dev-dependency 改来源；无新获取流程 |
| S-07 表单 | Applicable | FormInput → FormTextarea/FormEditor 控制层；Form 继续唯一业务值 owner |
| S-08 跨 crate/协议 | Applicable | C-01–C-05；GPUI、宏、Form、Tokio、MCP 配套 |
| S-09 错误 | No change | 不设计新错误分类；JoinError、表单 ResolveError、审批/文件错误通路保持；若 API 强制改变须先补契约 |
| S-10 数据库 | Applicable | Jaco 的 Diesel/SQLite 配套；Feiwen 保留 DuckDB；schema 与用户数据不改 |
| S-11 生成/复制 | Applicable | platform-ext/build.rs、官方 skill/docs；G-01/G-02 |
| S-12 资源 | Applicable | app-assets 宏与 assets fallback；保留图标与打包资源分层 |
| S-13 文案 | Applicable | 复用现有处理/审批/附件文案；新增 en-US/zh-CN `conversation-jump-to-latest`，用于 MessageScroller 按钮 |
| S-14 安全 | No change | Jaco 审批/文件访问仍由现有业务代码决定；不申请新运行时权限 |
| S-15 诊断 | No change | 不新增日志系统；保留当前错误传播和脱敏 |
| S-16 CI/打包 | Applicable | ci.yml、xtask/bundle.rs、bootstrap；R-06 |
| S-17 依赖 | Applicable | 下述依赖目标与来源约束 |
| S-18 文档 | Applicable | owner 文档、官方快照、根索引；不改写历史 #205/#199 结论 |
| S-19 验证 | Applicable | R/T 表；实施阶段按本轮受影响范围验证 |

## 证据与目标依赖

| E-ID | 分类 | 事实/证据 | 计划后果 |
| --- | --- | --- | --- |
| E-01 | Baseline | root Cargo.lock：Zed 1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba；组件 57a9903f48160845aabc8b92a1e2f5348c80d439 | 从当前 main 迁移 |
| E-02 | Upstream | [v0.6.0](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.0)，源码 94a313a72a2513aee2780240cd322d552b2395f0 | 固定组件/文档证据 |
| E-03 | Upstream | [gpui-pre 0.3.3 发布源码](https://docs.rs/crate/gpui-pre/0.3.3/source/Cargo.toml)，元数据 Zed 5b055fa789a8b8d38ac951a6e0cde272f66b4495；[#2929](https://github.com/longbridge/gpui-kit/pull/2929) 记录旧宏 release 问题 | 用 0.3.3 替代此前候选 0.3.1，解析时确认 macros 配套版本 |
| E-04 | Historical | draft §8 的 #205 三提交与 Windows 验证记录 | 只按改动借鉴，不视为本次测试结果 |
| E-05 | Release-gated | [#2945](https://github.com/longbridge/gpui-kit/pull/2945)、[#2946](https://github.com/longbridge/gpui-kit/pull/2946)、[#2947](https://github.com/longbridge/gpui-kit/pull/2947) 在 0.6.0 后 | Q-05 与 T-04 |
| E-06 | Baseline | app-assets-macros/src/lib.rs 的绝对 crate 路径；root profile.dev.package.gpui | 不能仅换 import 或依赖字符串 |

| 依赖 | 基线 → 实施目标 | 状态/迁移约束 |
| --- | --- | --- |
| GPUI/platform | Zed Git → gpui-pre / gpui-pre-platform 0.3.3 | 已解析；共享 crate 允许保留 Rust 别名 gpui，应用入口受 Q-01 |
| 组件/统一入口 | Git 0.5.2 → gpui-component / gpui-kit 0.6.0 | 已知 Editor/Textarea 迁移；不要因 umbrella 名称重命名本地所有 crate |
| assets | gpui-component-assets Git → gpui-kit-assets 0.6.0 | 保留 app-local Assets 与 fallback 顺序 |
| gpui / macros patch | 两项 Zed Git patch → 删除 | 必须先证明单一 GPUI 类型来源；不靠 patch 混接两种类型 |
| Tokio bridge | 本地 path → 保留 | Q-02；无 gpui-pre-tokio 发布包，不引入 Zed Git |
| Rig/RMCP | Rig 0.42.0 不变；主 RMCP 2.2.0 保留 | Rig 的 RMCP ^2 类型边界；独立工具按 Q-03 保留 2.2.0 |
| Diesel/SQLite | 2.3.11 / 0.37.0 → 2.3.13 / 0.38.2 | 一起处理，保留 bundled-windows 与唯一 SQLite links owner |
| Windows | bindgen 0.66.0、core 0.62.2、future 0.3.2 保留 | 已核实 0.100.0 移除 --no-allow/--no-comment、改变返回值且生成 RuntimeType::NAME 与保留 runtime 不配套；使用 0.66.0 已有 --no-allow 删除生成后字符串裁剪，Windows 实机验证仍待执行 |
| 普通直接依赖 | draft §6 的明确完整版本 | 候选表不等于逐项兼容已确认；变更 feature 与调用点留在各 owner |

普通依赖保留 draft §5–6 的唯一版本清单；不得使用 latest 作为可执行目标。不继承旧 arrayref Git patch：
当前 registry 的 0.3.9 未撤回，历史 patch 理由不能直接套用。完整间接 Git 来源以解析后的 lock/tree 为准，
不预先承诺归零。若有残留，记录包、引入链、必要性和退出条件。

## 公共契约与复用决定

| ID | 契约/owner | 迁移不变量 |
| --- | --- | --- |
| C-01 | root → 所有 GPUI 消费者 | 同一 GPUI package/source；dev 与运行依赖一致；宏别名与 feature 显式映射，profile 按真实 package 名 |
| C-02 | Form 适配器 → Jaco/HTTP Client | Form 持有 String 值；原生控制持有 IME/焦点/选择；ControlBinding 单一生命周期 owner，defer_set/defer_blur 路由不变 |
| C-03 | gpui-tokio → HTTP Client 等既有消费者 | init/init_from_handle、Tokio::spawn 返回 Task<Result<R, JoinError>>；Task drop abort、外部 runtime 不被关闭 |
| C-04 | Jaco 业务投影 → 展示组件 | Message 等不拥有审批、附件权限、请求用量、业务消息身份；滚动修改需 Q-04 |
| C-05 | jaco-agent → MCP 独立服务 | Rust 类型与跨进程协议兼容分开；保持认证、工具审批、取消/退出路径，Q-03 决定测试服务版本 |

| D-ID | 分类 | 删除/替换顺序与保留边界 |
| --- | --- | --- |
| D-01 | Adapt | 去旧 Git/patch → 统一发布包；不增加兼容版本层 |
| D-02 | Retain，用户决定 | 保留 app-assets/-macros；仅调整 Q-01 所需路径/依赖 |
| D-03 | Adapt | 旧 InputState 多行/code_editor 用法 → Textarea/Editor；先扩展已有适配器再迁移消费者 |
| D-04 | Retain，限必要功能 | Composer 的原子 skill token/IME/选区协作、快捷键录制/持久化、原生隐藏/窗口层级/Quick Look、复制失败反馈仍有具体功能缺口；不再整体保留这些控件的通用内部实现，局部替换见 D-11–D-13 |
| D-05 | Adapt，用户决定 | 采用 Message/Shimmer 和 MessageScroller；恢复跟随语义见 Q-04 |
| D-06 | Adapt | 官方资料全目标集合同步；本地规则独立维护，布局见 Q-06 |
| D-07 | Adapt | Jaco 会话搜索 → Command；删除 Input/List/delegate/键盘选择样板，保留 DB/Operation/错误重试/业务 ID；jaco/L-02，R-09/T-09 |
| D-08 | Adapt | Jaco 标题栏菜单 → AppMenuBar；删除本地菜单状态机与 popup 递归，只保留应用菜单定义与 leading 外壳；jaco/L-03，R-10/T-10 |
| D-09 | Adapt | 列表型 Picker → Combobox/SearchableListDelegate；删除通用选择/搜索/弹层样板，保留领域数据与可修改性策略；任意内容 Popover 分开处理；jaco/L-04，R-09/T-09 |
| D-10 | Reuse directly | 自绘 TitleBar 的 WindowOptions 以 TitleBar::window_options() 为基线，保留 owner-specific 参数；jaco/L-05、feiwen/L-01，R-10/T-10 |
| D-11 | Reuse directly，局部 | 快捷键显示 → Kbd::format/Kbd；不改变录制或序列化；jaco/L-06，R-10/T-10 |
| D-12 | Adapt | Composer 重复闪烁调度 → GPUI Animation；jaco/L-07 记录已实施的可见性/生命周期，R-11/T-11 |
| D-13 | Adapt | Composer 历史容器 → UndoHistory；jaco/L-08 记录已实施的事务和 IME 映射，R-11/T-11 |

复用依据以 E-02/E-03 固定发布源码为准。Command 为目标新增；AppMenuBar、Kbd、Combobox 在基线已存在，
应标记为既有重复实现清理，不能把目录迁移误认成新增组件。API 与必要功能缺口由 owner 的 L 表维护。
UI 外观、键盘细节和默认交互差异本身不构成 Retain/Defer 理由；业务身份、错误、权限、数据一致性仍须成立。

### WP-01：依赖图与构建配置

Root 文件：`Cargo.toml`、`Cargo.lock`；独立工具 lock 由工具 owner 维护。
批准 Q-01 后固定 Rust dependency keys、实际 package 名、features、默认 features 和宏路径。保持 Jaco
basic/full 语法集合、HTTP Client 全语言、平台 font-kit/x11/wayland/runtime_shaders、测试支持的实际需求。
使用 Cargo 更新锁文件（执行前按仓库规则提权），不手工改锁；将 profile.dev.package.gpui 对应到新真实包名。
检查发布宏配套、重复 GPUI 类型、SQLite links 和剩余 Git 引入链。无法解析时停止相关包，不加入临时 patch。

### WP-06：Skill、文档与生成资料

- G-01：上游固定提交 → 官方 skill references / website/docs/components → 本地快照；按 Q-06 保留职责分离，细化精确移动/删除。
- 目标包括新增、更新、重命名、删除正文；本地 index/rules 不算官方正文，不被覆盖。
- 更新 attribution 的仓库、路径、SHA、许可证副本；存在 hash 管理时先核实算法/入口，禁止伪造 hash。
- 同步受影响 skill 导航、AGENTS.md 依赖说明、owner README/guide；Form 公共文档保持中英文一致。
- 保留不受影响历史计划；不把未发布接口复制成当前依赖的能力。官方快照保持原文，本地适配差异另记。
- G-02：Windows winmd → platform-ext/build.rs → OUT_DIR/windows_ai_bindings.rs；具体生成处理归 platform-ext。
- 完成条件：批准目标的集合/正文一致、链接可达、来源可追溯、本地约束仍被保留；未执行时不可更新完成状态。

## 关键不变量与验证计划

下列为验证计划，实际执行结果以[实施记录](implementation.md)为准。开发阶段只选覆盖本轮改动的最小充分检查；同状态不重复跑
覆盖同范围的 build/test/clippy。缺失自动化覆盖时补关键回归，不按文件数新增测试。

| R/T | 不变量 | 最小证据 |
| --- | --- | --- |
| R-01/T-01 | 单一 GPUI 来源与正确 feature | 更新后 cargo tree -d、cargo tree -e features、lock 源检查；四应用独立默认 feature 路径 |
| R-02/T-02 | 宏展开/资源正确 | app-assets 既有宏测试及 Jaco/Feiwen 调用方编译；自有图标、provider SVG、组件 fallback 加载 |
| R-03/T-03 | 表单状态与 IME | adapters.rs 复用来源抑制/程序写回/退役测试；中文输入、撤销、保存中关闭保护手工场景 |
| R-04/T-04 | 消息/Markdown/滚动语义 | 同 block 数换成长内容、流式中文、硬软换行、详情展开高度；向上阅读时新消息不抢滚动，滚回底部后后续流式增长恢复跟随，无需再次提交 |
| R-05/T-05 | Tokio 取消/运行时所有权 | panic → JoinError、drop Task abort、外部 handle 可继续使用；复用或提取旧分支三类测试 |
| R-06/T-06 | release/平台/资源 | 四应用各执行 cargo run -p xtask -- bundle <app>（不加 --install）；此命令已内含单应用 release build，不重复构建 |
| R-07/T-07 | 配套库遵循已确认行为边界 | 各 owner 定向测试；SQLite 事务/映射既有测试，Similar diff 按 [jaco-agent 已确认的默认算法策略](../../../crates/jaco-agent/docs/dev/issue-215/README.md)核对既有样例，MCP E2E 受 Q-03 |
| R-08/T-08 | 同步资料一致 | 官方目标集合/正文对比，来源/许可/相对链接与旧路径残留检查，不格式化官方正文 |
| R-09/T-09 | 搜索/选择保留必要业务功能 | Command 正文/项目命中不被二次过滤、过期查询不覆盖新结果、确认定位正确会话；Picker 刷新/过滤不串值、只读不能修改、失败可恢复；详见 jaco/L-02、L-04 |
| R-10/T-10 | 菜单/标题栏/快捷键边界 | 菜单 action 路由与关闭后焦点、标题栏拖动/双击、窗口参数；Kbd 仅改变显示且录制值可回读；按受影响平台执行 |
| R-11/T-11 | Composer 局部替换保留编辑正确性 | 闪烁失焦停止、输入后光标可见、减少动态效果可见；撤销/重做同时恢复文本/token/选区，IME 提交与撤销后新编辑分支正确；不固定旧动画节拍或无必要的历史布局 |

WP-07 最终阶段对齐现有 CI：`cargo fmt --all -- --check`、`cargo build --workspace --locked`、
`cargo test --workspace --locked`；macOS 的
`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`。
Linux 原生依赖只在 script/bootstrap / script/install-linux.sh 维护；不散落到 workflow。
三平台 build/test、平台相关包/生成、单应用 release、人工 UI 与 MCP E2E 分别记录，不互相替代。
打包/人工验证使用隔离数据，不安装覆盖用户现有应用，不使用真实凭据或抓取入口。

## 实施状态与完成记录

用户已授权实施。WP-01–WP-06 的具体变更、技术映射和验证证据统一记录在[实施记录](implementation.md)。
本地 workspace 构建、测试和严格 Clippy 已通过；未将 macOS 检查等同 Windows/Linux 验证，也未将自动化测试等同手工 UI 验收。

| 项目 | 实际结果 |
| --- | --- |
| 代码/依赖/锁文件/skill | 已落地，受影响自动化验证与 Jaco 定向界面观察已完成；边界见实施记录 |
| 提交/PR | 2026-09-07 用户已要求提交、推送并创建面向 main 的普通 PR |
| 远端 issue 同步 | 已按此前用户决定更新 #215 并回读确认；本轮实施尚未发布 |
| 测试 | 本地 workspace 测试及严格 Clippy 通过；远程三平台 CI 待 PR 触发 |
| release/手工 UI/MCP E2E | 分别记录于实施记录，不互相替代 |
| 平台限制 | Windows/Linux 实机验证待执行；Windows bindgen 保留 0.66.0 的依据见版本表 |
