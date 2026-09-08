# Gupi：应用骨架、启动引导与恢复入口

- 阶段 issue：[#218](https://github.com/suxiaoshao/gpui/issues/218)
- 总 issue：[#217](https://github.com/suxiaoshao/gpui/issues/217)
- 当前分支：`codex/218-gupi-app-scaffold`
- 父计划：[Gupi 总览](../issue-217/README.md)
- 打包工具计划：[xtask](../../../crates/xtask/docs/dev/issue-218/README.md)
- 应用设计：[app/gupi](../../../app/gupi/docs/dev/issue-218/README.md)

本文件保存第一阶段的设计与运行时契约。应用、配置状态机、Pi 版本探测和 xtask 接入均已实现；实际验证范围见 [应用 README](../../../app/gupi/README.md)。

## 已确认方向

### D-01：独立的 Pi 原生客户端

新建 `app/gupi`，暂定名称 Gupi。应用通过用户本机的 `pi` 命令接入 RPC，Pi 负责模型与 Agent 能力，Gupi 负责原生 GUI 和桌面交互。用户自行安装、升级、登录及配置 Pi；Gupi 不提供安装、升级、账号或扩展包管理器。

### D-02：分阶段交付

六个阶段已建立原生 GitHub 父子关系：

1. [#218 应用骨架与运行入口](https://github.com/suxiaoshao/gpui/issues/218)
2. [#219 Pi RPC 与进程生命周期](https://github.com/suxiaoshao/gpui/issues/219)
3. [#220 主窗口基础对话闭环](https://github.com/suxiaoshao/gpui/issues/220)
4. [#221 临时窗口与全局快捷翻译](https://github.com/suxiaoshao/gpui/issues/221)
5. [#222 历史会话与完整 RPC 交互](https://github.com/suxiaoshao/gpui/issues/222)
6. [#223 原生体验、打包与验收](https://github.com/suxiaoshao/gpui/issues/223)

本阶段分支 `codex/218-gupi-app-scaffold` 通过 PR 汇入总 Issue 分支 `codex/217-gupi-pi-rpc-client`。其余阶段不在本阶段实现范围内。

### D-03：加入 welcome / startup 入口

第一阶段从配置读取结果和 Pi 检查状态呈现启动界面：配置不存在时提供初始设置，有效配置与环境检查通过后直达主界面。不同类型的故障进入对应恢复页面，按恢复任务划分，不为每个底层错误码建立页面。页面通过组件或函数共享结构、诊断展示和操作区；不把所有错误塞进同一个通用页面。

核心配置不可用时先进入配置恢复页；配置恢复后检查 Pi。Pi 不可用时留在 Pi 环境设置页，允许打开应用设置，不跳过进入业务主界面。多项问题按依赖顺序处理，每次修复后重新检查，全部启动条件满足后继续。布局缺失或不可用时使用默认窗口继续启动；不可用的 `state.toml` 直接尝试删除，退出时保存当前窗口数据，不提供布局恢复页。

后续模块可以接入故障与恢复状态；不为尚未存在的数据库预建恢复实现。运行中已有有效配置时，重新读取失败保留内存配置并在对应设置/恢复界面处理，不将应用当作首次启动。

### D-04：开发期 Logo

按用户决定，开发期间临时使用 Pi 官方 logo。已发邮件向官方询问直接使用及基于原设计做明显修改后的使用许可，目前没有收到已确认的授权。收到回复或准备发布时再确定最终方案；临时使用不表示许可已获得。

### D-05：平台、组件与 Pi 检查范围

第一阶段以 macOS 为首个完整验收平台，保持跨平台职责边界并明确记录其他平台未验项。先使用 gpui-component；以后遇到具体自定义需求再考虑引入 base，不预先建立双套控件。依赖统一入口按实施时已合入的 gpui-kit 与仓库约定核对。

第一阶段只检查 Pi 命令可用性和版本。登录、模型列表和实际 RPC 能力在第二、三阶段接入；启动检查不发送模型请求。

### D-06：窗口生命周期参考 Jaco

参考已核对的 Jaco：macOS/Windows 主窗口关闭时隐藏，应用继续运行；菜单和重新唤起入口恢复主窗口。明确退出走统一退出流程，保存必要状态并收尾本阶段持有的任务。第一阶段不移植 Jaco 的数据库、会话或 MCP 退出链。

Jaco 在 Linux 分支允许窗口实际关闭，不能仅凭该回调推断进程随后必然退出。Gupi 保持这一平台分支意图，具体平台生命周期在实施时核对；不宣称已完成 Windows/Linux 原生验收，也不因后台驻留自动增加托盘功能。

### D-07：设置保存、重新读取与恢复

- 设置采用明确保存动作；成功后统一应用语言、主题和 Pi 路径，失败不显示已保存。配置操作运行期间禁用表单编辑及其他配置操作，事件入口同样校验状态；通过 gpui-operation 统一协调读写与恢复。内存写回成功保留此前已有的未保存草稿。
- 设置页提供“重新读取”：主动从磁盘读取并完整校验，成功后才替换权威内存配置并发布变化。不做自动文件监听；外部修改可通过此操作生效，无需重启。
- 读取/解析/校验失败时保留当前有效内存配置及用户编辑内容，展示对应问题，允许修正后重试。
- 用户可以在设置页重新设置配置，也可以明确选择将当前有效内存配置保存到磁盘。写回来源是已应用的内存配置；尚未保存的表单修改通过通常的保存入口处理。
- 重新读取不得静默丢弃未保存的表单修改，写回不得静默覆盖已知磁盘改动。实现时提供明确的替换/舍弃确认并核对写入前状态，失败时保留双方数据。
- 替换损坏文件先保留备份并说明范围。目录或权限错误不能靠重写配置内容修复，保存失败后允许用户修正文件系统条件并重试，不虚报成功。
- 首次启动没有有效内存配置时，不提供虚假的“保存当前内存配置”操作；提供重新设置、定位文件、重试和明确确认后的备份重置。

### D-08：主题与语言参考 Jaco

默认跟随系统，提供浅色/深色/系统以及中文/英文/系统选择。配置生效后更新当前窗口与菜单，重启恢复选择；系统外观变化能传播到主题。语言检测参考 Jaco：识别中文系统语言，其余使用英文；不声称 Jaco 已支持运行中操作系统语言变更监听。

复用共享主题能力与应用本地 Fluent 方式，不依赖 jaco-core 的业务类型。第一阶段保持已讨论的基础切换范围，不据“参考 Jaco”扩展为复制全部主题预设、自定义颜色编辑器或 Jaco 设置项。

## 第一阶段范围

本节整理已确认的阶段范围，细节由上述决定约束。目标是完成可独立使用的应用外壳，让后续阶段接入业务能力。

| 范围 | 建议交付内容 | 本阶段不展开 |
| --- | --- | --- |
| 应用入口 | 独立成员、应用标识、薄 main、启动与退出编排、主窗口 | Agent 循环与模型适配 |
| 路径与日志 | Gupi 自有配置/状态/日志目录，隔离测试目录，错误诊断 | 采集模型对话全文、认证信息 |
| 用户配置 | Pi 路径覆盖、语言、外观；默认值、读取、校验、保存与失败反馈 | Pi 认证与模型配置管理 |
| 界面状态 | 主窗口位置、尺寸、最大化恢复，失效位置处理 | 会话与工具状态持久化 |
| 内存状态 | 明确配置、启动检查、窗口、任务各自所有者及销毁点 | 预建所有未来业务状态 |
| 设置页面 | 必要字段修改、保存、主动重新读取、有效内存配置写回与错误恢复 | 完整高级设置中心、自动文件监听 |
| Pi 检查 | 命令定位、可执行性、版本检查、可修正错误与重试 | 默认发送模型请求 |
| 主题 | 系统/浅色/深色，统一主题颜色，运行中变化与重启恢复 | 自定义主题编辑器与大量配色 |
| i18n | 中英文 Fluent、系统/手动语言，当前页面、菜单与错误文案 | 提前编写未来业务文案 |
| 资源与打包 | app-local 资源、必要图标、bundle 标识和本地化，基础打包启动 | 全平台发行验收 |
| welcome/startup | 首次引导、正常启动状态呈现与分类故障页面，共用结构和检查/恢复能力 | 一个页面承载全部故障、未来数据库恢复 |

主题、i18n 和基础打包入口放在第一阶段；第六阶段负责完整性、性能及发行验收。这项阶段细化尚未回写 GitHub issue 正文。

## 数据与状态边界

- **用户设置**：使用独立 `config.toml`，仅保存客户端自身的偏好与 Pi 路径覆盖。
- **界面恢复状态**：使用独立 `state.toml`，保存窗口布局；高频布局写入不改写用户设置。
- **内存运行状态**：检查过程、错误、窗口句柄、任务和订阅由对应运行时对象持有，不序列化到配置文件。
- 配置有一个权威修改入口，UI 与主题/i18n 消费其变更；可推导的状态不再创建并行的可修改字段。
- 文件位置、格式与应用私有目标类型见应用计划 L-100；设置提交与外部修改采用 D-07。
- Gupi 不保存或覆写 Pi 的认证、模型配置和会话内容。当前不引入数据库。
- 用户配置加载失败时保留原始文件；重置先备份、说明范围并由用户触发。重新读取失败保留有效内存配置，恢复操作遵循 D-03、D-07。

## 启动与恢复体验

| 场景 | 页面与体验 | 已定边界 |
| --- | --- | --- |
| 第一次使用 | welcome 展示简洁介绍、必要设置和检查结果 | 必要条件满足后继续，不跳过 Pi 环境要求 |
| 正常启动 | 检查通过直达主界面 | 避免快速检查造成页面闪烁，慢检查可见且不阻塞 UI |
| Pi 缺失/路径无效 | Pi 环境设置页，修正路径、重试和查看说明 | 设置仍可访问，不能进入业务主界面 |
| 启动时核心配置损坏 | 独立配置恢复页，提供基础主题和语言 | 修复后继续，不静默使用默认配置绕过；备份后重置需明确操作 |
| 布局状态不可用 | 使用默认窗口继续启动 | 尝试删除失效状态，退出时写入当前窗口数据；失败只记日志 |
| 运行中重新读取失败 | 对应设置/配置恢复界面 | 保留有效内存配置与草稿，可修正、重读或明确写回 |
| 多项启动故障 | 先处理配置等前置条件，再处理 Pi 环境 | 修复后重新检查，直到可以继续 |
| 后续模块失败 | 对应模块恢复页面，复用结构和操作组件 | 不因局部失败重复整个首次引导 |

不保存 onboarding_completed。实际读取配置并区分缺失、读取失败、解析/校验失败；首次明确保存前不自动创建配置。有效配置存在后继续环境检查，不推测用户是否曾完成引导。用户手动提供有效配置可直接使用，启动时删除配置则重新设置；运行中重读发现删除仍保留内存并进入对应恢复流程。布局文件只保存窗口信息，不参与首次设置判断。启动页本身需要在用户配置不可读时仍可显示。设置入口与启动引导应共用配置读写、校验和恢复能力，避免两套互相不同的行为。

## 产品问题决定记录

原 Q-01 至 Q-09 已由用户本轮答复解决，不再列为待确认。具体设计仍需代码调查；没有新增需要用户立即回答的产品问题。

| ID | 问题 | 已选定方向 | 决定 |
| --- | --- | --- | --- |
| Q-01 | 首个完整验收平台 | macOS，其他平台记录实际验证边界 | D-05 |
| Q-02 | Pi 不可用时是否跳过 | 不跳过，留在 Pi 环境设置页，可访问设置；采用最近一轮修订建议 | D-03 |
| Q-03 | 关闭与退出 | 参考 Jaco，区分隐藏主窗口和明确退出及平台差异 | D-06 |
| Q-04 | 设置保存方式 | 显式保存，成功后生效 | D-07 |
| Q-05 | 外部编辑如何生效 | 主动重新读取；失败保留内存，可明确写回或重新设置，不自动监听 | D-07 |
| Q-06 | 损坏配置如何恢复 | 配置分类恢复页、修复后继续、备份后重置；布局不可用时自动丢弃 | D-03、D-07 |
| Q-07 | Pi 检查范围 | 第一阶段命令与版本，不做登录/模型请求检查 | D-05 |
| Q-08 | 组件选型 | 先 component；具体自定义需求出现后再考虑 base | D-05 |
| Q-09 | 主题与语言 | 参考 Jaco，默认系统并支持手动选择和传播 | D-08 |

## 设计入口、依赖与契约

根计划拥有 S/C/ERR、D-01 至 D-08、WP-01 和聚合状态；应用与 xtask 的本地编号、类型、文件及测试各由其 owner 计划维护。

- [应用设计](../../../app/gupi/docs/dev/issue-218/README.md)：L-100 至 L-105、ST-100 至 ST-102，WP-100 至 WP-103。
- [基础打包](../../../crates/xtask/docs/dev/issue-218/README.md)：WP-200。
- 根拥有 F-01：实施时将 `app/gupi` 加入根 Cargo.toml workspace members；F-02：由 Cargo 更新 Cargo.lock，不手写锁条目。当前已增加 workspace member，Cargo.lock 仅新增 Gupi 包依赖条目。

### E-04：依赖与平台边界

应用采用根 manifest 锁定的 GPUI Kit 0.6.0，通过 `gpui_kit::application`、`init` 和 component/assets reexports 接入；不引入第二套 GPUI 类型。配置与表单接入使用仓库现有 gpui-store、gpui-operation 和 gpui-form。Windows/Linux 实机验证及完整发行验证尚未完成。

### 外部与打包契约

| ID | 权威来源与参与者 | 本阶段契约 | 兼容与验证 |
| --- | --- | --- | --- |
| C-01 | Pi CLI；Gupi `pi.rs` 调用 | 已应用命令加独立参数 `--version`，捕获输出和退出码；不执行 shell 拼接，不发 RPC/模型请求 | 本地 Pi 0.85.1 源码确认版本入口；本阶段不设置未经验证的最低版本、不声称 RPC 兼容；应用 T-104 |
| C-02 | 操作系统文件系统；Gupi persistence/controller | 配置与布局独立；配置使用快照冲突检查、显式备份覆盖、成功后发布；锁不保证外部编辑器参与；布局直接替换 | 无 Jaco 数据迁移；无数据库；应用 T-100 至 T-103 |
| C-03 | xtask CLI 和 app Cargo metadata | `bundle gupi` 定位 `app/gupi`；运行时与 bundle 资产分开；不打包 Pi | xtask T-200 与 Finder 启动 |

### 错误语义与恢复

错误的目标类型归属 Gupi；分类在根统一定义，应用计划实现 producer/controller/UI。错误携带有限的结构化上下文，不保存本地化后的句子。PendingConfig 的应用私有定义与修复准入见运行时 L-114；诊断不得打印其配置值或原始字节。

```rust
pub(crate) enum StartupProblem {
    ConfigRead { path: PathBuf, kind: std::io::ErrorKind }, // ERR-01
    ConfigInvalid { path: PathBuf, detail: String },       // ERR-02
    ConfigConflict { path: PathBuf, write_source: ConfigWriteSource }, // ERR-03
    ConfigWrite { path: PathBuf, outcome: WriteOutcome,
        stage: ConfigWriteStage, write_source: ConfigWriteSource,
        backup_path: Option<PathBuf>, kind: std::io::ErrorKind }, // ERR-04
    PiProbe { kind: ProbeFailure },                       // ERR-05
    Layout { path: PathBuf, detail: String },              // ERR-06
}
pub(crate) enum WriteOutcome { Unchanged, NeedsReconcile }
pub(crate) enum ConfigWriteStage { Lock, Backup, Stage, Commit, Sync }
pub(crate) enum ProbeFailure {
    NotFound, Spawn, Timeout, OutputLimit, Exit, InvalidVersion,
}
```

| ID | 产生与传播 | 用户恢复入口 | 部分效果与诊断 |
| --- | --- | --- | --- |
| ERR-01 | 文件读取失败 → config owner | 无有效配置进配置恢复；已有值在设置页定位文件/重试 | 保留内存和草稿；日志只含路径、错误码 |
| ERR-02 | TOML/字段校验 → config owner | 修正后重读、重新设置或明确备份重置 | 原文件不动；detail 必须清理配置内容和控制字符 |
| ERR-03 | 写入前磁盘快照不符 → config owner | 重读或明确备份覆盖 | 本次未写入；不循环自动重试 |
| ERR-04 | 写入/备份/提交失败 → config owner | 修正文件系统条件后，用原保存入口提交当前草稿 | 提交前失败保留旧值；提交后结果不明先重读协调，禁止假报成功；不重放旧草稿 |
| ERR-05 | 命令解析/进程/版本检查 → Pi owner | Pi 环境设置页修正命令并重试 | 回收探测进程；受限诊断，无凭据/环境全量日志 |
| ERR-06 | 布局读取/解析失败 | 使用默认窗口继续启动，尝试删除失效状态 | 不影响用户配置；删除、保存失败只记日志 |

取消属于生命周期事件，不包装成用户错误。正常检查使用低级别诊断；失败记录一次操作结果，UI 重绘不重复打印错误。

### 系统适用面

以下覆盖第一阶段目标范围；尚未完成的精确声明受 E-04 约束。

| ID | Surface | 状态 | 依据与归属 |
| --- | --- | --- | --- |
| S-01 | Workspace, files, modules, and owner boundaries | Applicable | 新增 app/gupi；F-01/02、应用 F-100 段 |
| S-02 | GPUI components, layout, interaction, and accessibility | Applicable | 应用 L-104 |
| S-03 | Entity, Store, Global, identity, and projections | Applicable | 应用 ST-100 至 ST-102 |
| S-04 | Actions, events, subscriptions, focus, and windows | Applicable | D-06、应用 L-104 |
| S-05 | Async tasks, concurrency, cancellation, and shutdown | Applicable | 应用 L-101 至 L-104 |
| S-06 | Data acquisition and Operation state | Applicable | 配置读取及 Pi 探测，应用 ST-100/101 |
| S-07 | Forms and editable state | Applicable | D-07、应用 L-101 |
| S-08 | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | C-01 至 C-03；无 provider/Rig/MCP 接入 |
| S-09 | Error identity, propagation, recovery, and error UI | Applicable | ERR-01 至 ERR-06 |
| S-10 | Database, persistence, and migrations | Applicable | C-02 文件持久化；D-01/第一阶段范围无数据库，故无 schema/migration |
| S-11 | Generated, synchronized, copied, or vendored content | Applicable | logo 与 bundle 图标溯源，E-04 第 4 项 |
| S-12 | Icons and assets | Applicable | D-04、应用 F-105 |
| S-13 | Fluent i18n and bundle localization | Applicable | D-08、应用 L-105 |
| S-14 | Security, privacy, and credentials | Applicable | C-01 命令执行、C-02 覆盖边界；不接触 Pi 凭据 |
| S-15 | Observability and diagnostics | Applicable | 错误目录、应用 L-105 |
| S-16 | Packaging, platform behavior, and CI/release | Applicable | C-03、WP-200；完整发行留 #223 |
| S-17 | Dependencies, frameworks, Git sources, and toolchains | Applicable | E-04、WP-01 |
| S-18 | Owner documentation, indexes, and ADRs | Applicable | 父/子/owner 计划和索引；当前没有独立 ADR 需要 |
| S-19 | Validation and completion evidence | Applicable | owner R/T 映射、下文交付记录 |

### 实施顺序

WP-01（根）：依赖门解决且设计 Ready、获得实施授权后，增加 workspace member 与应用 manifest 的依赖接入，Cargo 生成锁文件；检查无无关依赖升级，再进入应用 WP-100。随后 WP-101 配置恢复 → WP-102 Pi/状态呈现 → WP-103 桌面一致性 → xtask WP-200 基础打包。

各 owner 文档保存详细实施动作和最小充分验证；根不复制测试表。基础打包通过后按适用 `.github/workflows/ci.yml` 记录集成验证，未实际执行的平台和原生 UI 场景不能标为通过。

## 参考证据

以下来源为本轮本地只读调查，代表所查看代码，不代表所有应用都遵循同样完整的实现。

- [Jaco 应用编排](../../../app/jaco/src/app.rs)：配置、i18n、主题、窗口和退出初始化顺序。
- [Jaco 配置](../../../app/jaco/src/state/config.rs)与[布局状态](../../../app/jaco/src/state/layout.rs)：配置与恢复状态的不同职责。
- [Jaco 主题](../../../app/jaco/src/state/theme.rs)与[本地化](../../../app/jaco/src/foundation/i18n.rs)：设置变化、窗口外观与语言传播。
- [HTTP Client 入口](../../../app/http-client/src/main.rs)、[Feiwen 入口](../../../app/feiwen/src/main.rs)、[Novel Download 入口](../../../app/novel-download/src/main.rs)：较轻量的初始化和窗口入口。
- [共享主题](../../../crates/app-theme/src/lib.rs)与[打包 CLI](../../../crates/xtask/src/cli.rs)：实施前核对共享能力与新 app 接入位置。
- [Pi RPC 文档](https://github.com/earendil-works/pi/blob/da840b621/packages/coding-agent/docs/rpc.md)：此前核对的本机源码版本，尚未完成本项目真实 RPC 验证。

## 建议验收场景

按已确认的产品决定，在实施规格中将以下场景转为确切断言：

1. 配置缺失时进入初始设置且保存前不自动创建文件；手动提供有效配置直接继续检查。保存后退出重启能够继续 Pi 检查，未安装 Pi 也能看懂下一步。
2. 已配置环境启动时进入正确页面；启动检查不阻塞界面。
3. 修改语言、主题和 Pi 路径后，保存结果与实际行为一致；失败不假报成功。
4. 退出重启后设置和有效窗口布局恢复；关闭、重新唤起、明确退出符合选定策略。
5. 分别模拟配置损坏、布局损坏、Pi 路径失效：配置恢复保留原文件，布局失效自动丢弃且不阻塞启动，Pi 失败提供修正入口。
6. 基础打包应用可从桌面启动并定位 Pi，不能仅凭终端运行成功认定通过。
7. 外部编辑配置后主动重新读取；失败保留有效内存配置，成功才统一发布。存在未保存草稿时不会静默丢弃。
8. 验证当前内存配置写回、重新设置、备份重置及保存失败；磁盘冲突与权限问题不会导致虚报保存成功或静默丢失数据。

## 实现与验证

构建、测试、原生界面验证及未验证平台统一记录在 [应用 README](../../../app/gupi/README.md)。

### 按资源区分退出与失败（本轮细化）

| 具体操作 | 失败意味着什么 | 处理方向与当前状态 |
| --- | --- | --- |
| 用户点击保存配置 | 修改尚未可靠写入 config.toml | 已定：保留草稿/有效配置，显示具体写入错误；用户可修正后重试。用户随后明确退出时正常退出，不拦截、不弹二次确认、不自动重试，也不为未保存草稿增加退出恢复流程 |
| config.toml 提交已经开始但未返回 | 文件系统写入或同步尚未报告结果 | 已定：不把取消 future 当作回滚；保留任务和写入结果核对。不得套用 Pi 的强杀策略。极端文件系统阻塞的主动强退不在本轮承诺内 |
| 退出保存窗口布局 | 本次窗口位置/尺寸未保存 | 布局保存失败静默记录日志并允许退出；下次有效布局正常恢复，不可用时自动丢弃并使用默认窗口 |
| Pi --version 超时 | 环境探测进程未按期退出 | 从提交起计算 15 秒整体 deadline，覆盖排队、查找、启动和读取；超时后已持有子进程的 kill/wait 额外限 2 秒，随后 Complete(Timeout)。取消通过 Cancel/Drop 停止等待并触发终止，不承诺同步阻塞立即结束或已回收；属于环境探测结果 |
| Pi RPC 中模型/工具仍在执行 | 第二阶段运行资源需要停止 | 留 #219；Pi 返回的错误按原意呈现，不改写成配置错误。宿主进程退出/通信中断另标来源，不把单纯耗时长推断为 Pi 错误 |

### 配置重试需要的数据说明

这里的“提交数据”只是用户这次想保存的客户端设置。例如磁盘是 A，用户在表单改为 B，保存失败后点重试，应继续尝试 B，不能误写回 A。持有 B 的提交快照、目标文件与比较所需的原始字节，是 controller 内部职责，无需用户选择字段或结构。检测到外部把磁盘改成 C 时，重试不能自动覆盖 C；只有明确覆盖后才备份 C 并提交 B。不会因此保存 Pi 认证、对话或模型数据。P-02 的错误/修复 payload 已补入 ERR-03/04 与运行时 L-114，实施验证按相同不变量执行。
