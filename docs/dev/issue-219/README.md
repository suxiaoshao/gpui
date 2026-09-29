# Gupi：Pi RPC 与进程生命周期开发计划

状态：Done。所属 [Gupi 总览](../issue-217/README.md)，范围对应 [Issue #219](https://github.com/suxiaoshao/gpui/issues/219)。本阶段代码与必要验证已完成；实际结果见下文。

## 目标与范围

新增不依赖 GPUI 的 `crates/pi-rpc`，将用户本机 Pi 接入 Gupi。一个 client 拥有一个 Pi 子进程及其通信资源；应用级 state 管理多个 client、各自事件消费任务和统一退出。交付可独立测试的 RPC 接入及应用生命周期集成。

本阶段包括命令探测迁移、启动就绪、请求关联、事件分发、标准扩展 UI 协议往返和资源收尾。使用用户已有 Pi 命令；安装、升级、登录、模型和扩展配置由用户在外部完成。以 Pi **0.85.1** 为验证基线，暂不做旧版适配，也不增加严格最低版本拦截。

主窗口对话、会话列表、临时翻译窗口、命令面板及扩展原生控件归后续阶段。本阶段不引入 Jaco 数据层、provider、Agent 循环、进程池、自动重启或请求重放，不新增持久化配置或迁移。

## 当前实现与职责

| 位置 | 当前事实与本阶段改动 |
| --- | --- |
| [app/gupi/src/pi.rs](../../../app/gupi/src/pi.rs) | 当前混合纯命令探测与 PiProbeController。底层命令定位、执行、版本解析、错误及相关测试迁入 pi-rpc；Operation、GPUI 任务接入、本地化映射留在应用 |
| [crates/pi-rpc](../../../crates/pi-rpc/README.md) | 单 client 拥有 Child、stdin/stdout/stderr、请求等待表、事件流、就绪和关闭结果；crate 不读 Gupi 配置，不返回本地化键 |
| [app/gupi/src/state.rs](../../../app/gupi/src/state.rs) 与[state/pi.rs](../../../app/gupi/src/state/pi.rs) | 建立应用级 PiState，持有多个 client 及消费任务；管理创建、查找、关闭、移除和事件投影，不复制 Child 或请求等待表 |
| [app/gupi/src/app.rs](../../../app/gupi/src/app.rs) | 初始化应用级 state，保持现有主窗口与菜单入口；不因本阶段接入就自动启动常驻 RPC 进程 |
| [app/gupi/src/features/startup.rs](../../../app/gupi/src/features/startup.rs) | 现有 quit 会停止探测、等待配置写入、保存窗口状态，然后调用 cx.quit；接入 PiState 的关闭流程，确保在 cx.quit 前执行 |
| [crates/gpui-tokio/src/lib.rs](../../../crates/gpui-tokio/src/lib.rs) | 复用现有桥接；GPUI Task 丢弃会取消 Tokio future，runtime Drop 使用 shutdown_background，不能依赖它代替显式收尾 |
| 根 Cargo.toml、Cargo.lock、app/gupi/Cargo.toml | 注册新 workspace 成员及路径依赖，迁移依赖归属并删除应用已不使用的直接依赖；不顺带升级其他成员 |

PiState 用应用内稳定运行实例标识管理连接，Pi session ID 用于会话关联，OS PID 仅用于诊断。状态由 client 的事件和退出结果推进；应用只维护需要的投影。视图隐藏或切换不自动停止连接，配置命令变更只影响之后创建的连接，不重启既有实例。

本阶段提供应用内部创建连接的入口，并通过测试建立实例。生产环境何时为对话创建连接，在后续 UI 接入时决定；不增加临时按钮或启动即运行的隐含产品行为。

## 接口与行为契约

类型与模块名称允许在实施时调整；以下描述所有权和可观察行为，不固定私有字段与方法。

| 边界 | 契约 |
| --- | --- |
| 探测 | 返回解析后的 executable 路径、版本及非本地化错误。保留现有 15 秒探测 deadline、16 KiB 输出限制、最多两个阻塞启动任务，以及取消/超时后的迟到 Child 处理；这些参数不直接套用到 RPC 消息 |
| 启动 | 接收 executable、cwd 和结构化启动选项。立即读取 stdout/stderr；调用方可在等待就绪期间取得事件流，避免扩展消息被藏在 Ready 之后。以首个带 ID 的 get_state 成功响应确认就绪；启动超时、提前退出或协议失败均为启动失败 |
| client 所有权 | client 内部唯一生命周期 owner 持有 Child；请求接口、事件接收端与关闭结果可分开使用。关闭开始后拒绝新请求，重复关闭不会重复发送信号或挂起等待者；事件消费任务取消时不使进程失去 owner |
| 请求 | client 分配 ID，先登记等待者再写入，按 ID 和 command 关联响应；支持乱序响应。Pi success:false 是单个请求失败；I/O、分帧或连接终止结算全部挂起请求 |
| 本阶段命令 | 类型化 get_state、get_commands、prompt、abort、clear_queue 和标准 extension_ui_response；request_raw 支持尚未类型化的命令并复用关联与容量限制，后续按需要补充历史、模型及完整类型 |
| 事件 | 区分 response、运行事件和 extension_ui_request。按 stdout 的可观察顺序分发事件；未知事件和新增字段保留原始 JSON。扩展 UI 回复使用自己的信封，不进入普通命令响应等待表 |
| 执行完成 | prompt 成功仅表示接受、排队或已由扩展处理；agent_end 后可能重试、压缩或续跑，0.85.1 的 agent_settled 才表示这些续跑已结束 |
| 本地取消 | 丢弃请求 future 只停止本地等待，不隐式发送 abort，不声称命令未执行；迟到响应仍按响应处理，不转成事件，不自动重发 |
| 停止执行 | 暴露 abort 和 clear_queue 的真实语义。abort 等待 idle，但不清空 steering/follow-up；“停止并清队列”的用户交互及文本处置留给后续应用功能 |
| 诊断与错误 | 区分命令不可用、启动未就绪、Pi 拒绝、协议/I/O 错误、连接关闭和资源收尾失败。stderr 只保留有界尾部，不记录完整 prompt、环境变量或凭据 |
| 内存与背压 | 单帧、待处理请求、写入队列和事件积压均有界。队列满时明确拒绝请求或报告连接错误，不能静默丢增量；响应和关闭控制不能被无人消费的事件流无限阻塞。当前容量与失败行为见 [crate README](../../../crates/pi-rpc/README.md#limits-and-failure-behavior) |

RPC 启动预算与模型执行时长分开。状态查询可有限等待；prompt/abort 等命令不能统一套用探测超时。启动选项必须支持隔离测试所需的显式扩展、禁用发现、环境目录和无持久会话模式，不把测试限制强加给正常连接。

### 关闭顺序

每个 client 在正常关闭时拒绝新业务请求并结算等待者，发送带保留 ID 的 abort；收到成功回应后停止 writer 并释放 stdin，让 Pi 执行 session_shutdown。继续读取结束事件并观察退出，整个优雅关闭共用 2 秒上限，完成即返回。

通信错误、启动失败或优雅关闭超时后，取消并等待自己的 pipe 任务结束，再释放 Child；kill_on_drop 与 Tokio 的后台回收负责进程资源。不额外发送 SIGTERM，不追加强杀/回收等待。探测 --version 的错误路径同样使用 Drop，不追加 2 秒清理等待。退出等待用于给 Pi 业务退出逻辑机会，不承担证明 RAII 有效性的职责。

应用先对所有 client 调用 `close()`，同步拒绝新请求并发送关闭信号，再等待各自报告。各进程 owner 并发执行上述关闭流程和各自的 2 秒截止时间，不因其他进程卡住而推迟关闭。终态为 ConnectionState::Closed(CloseReport)，表示连接和 I/O 资源已释放；status 仅在实际观察到退出时存在，reason 记录原始连接错误或 ShutdownTimeout。删除 forced/cleanup_error，不把释放 Child 说成已同步回收。关闭控制独立于普通写入队列；阻塞或半帧写入导致 abort 无法完成时由同一个截止时间结束等待。

Windows 使用相同协议与 Child Drop 行为，无独立 SIGTERM 路径；npm/pnpm shim 的结构化参数处理不变。直接 Child 的释放不承诺任意扩展或 shim 后代退出。

### 应用退出接入

PiState 停止接受新建连接，同时启动已有实例的关闭；每个消费任务保留到对应连接关闭。现有配置写入与窗口状态保存继续完成；在这些工作和 Pi 收尾流程结束后才调用 `cx.quit()`。退出协调任务由应用持有，不能因某个视图被隐藏或先丢弃消费任务而取消。保留重复退出防护，错误记录后按真实关闭结果结束流程。

## 自动测试边界

保留协议类型及命令/事件解析的内存测试。依赖真实子进程、安装版 Pi、Shell、文件系统和实际超时的集成测试及专用 fixture 已删除；当前测试不证明操作系统进程退出或真实 Pi 互操作行为。稳定使用契约见 [pi-rpc README](../../../crates/pi-rpc/README.md)。

## 实现依据与已知限制

### 来源

2026-09-08 核对本地 `/Users/sushao/Documents/code/pi`，HEAD `da840b6216578c2a571d0374ac6a2091a83f9d91`，源码包版本与本机安装命令版本均为 0.85.1；未证明源码与安装产物逐字节一致。本次只读源码并做隔离实验，没有修改 Pi 或用户配置。

- [Pi RPC 官方文档](https://pi.dev/docs/latest/rpc)、[扩展文档](https://pi.dev/docs/latest/extensions)：能力说明；latest 可变化，以下固定版本源码为本次依据。
- [rpc-types.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/rpc/rpc-types.ts)、[rpc-mode.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/rpc/rpc-mode.ts)、[jsonl.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/rpc/jsonl.ts)：自定义 JSONL 信封、并发命令分发、启动与扩展 UI。协议不套 JSON-RPC 2.0。
- [agent-session.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/core/agent-session.ts)、[agent-session-runtime.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/core/agent-session-runtime.ts)：单进程一个当前会话、abort/idle、会话替换和收尾语义。
- [pi-desktop process_manager.rs](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/process_manager.rs#L409)、[退出入口](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/lib.rs#L153)：早期参考的退出时序；当前实现已按上文精简，不沿用固定等待。本地克隆 `/tmp/gupi-reference-pi-desktop-20260907`，只读源码，未运行退出测试。参考实现直接取消 wait 任务，不能据此证明回收完成。
- [第一阶段进程证据](../../../app/gupi/docs/dev/issue-218/pi-evidence.md)：已实现探测与长期 RPC 的边界。

### 启动扩展对话

Pi 先等待扩展 session_start，再注册 stdin reader，没有独立 Ready 事件。官方 TypeScript client 的固定 100 ms 等待不能证明就绪。

已使用临时目录、无凭据环境和本机 Pi 0.85.1 做以下实验，未发送模型请求，临时目录已清理：

| 实验 | 观察 | 对实现的影响 |
| --- | --- | --- |
| 立即发送带 ID 的 get_state 与未知命令 | 约 216 ms 收到成功/失败响应，stdin EOF 后约 4 ms 正常退出 | 支持以响应证明就绪；时间是观察值，不是门限 |
| session_start 等待无 timeout 的 confirm，收到后立即回复 cancelled | 约 198 ms 收到对话，约 202 ms 以 0 提前退出，无 get_state 响应 | 未就绪退出必须报告失败 |
| 同一 confirm 设置 1000 ms timeout | 约 207 ms 收到对话并回复，约 1214 ms 才收到 get_state 响应 | 回复未提前解除初始化阻塞，需等扩展自身超时 |

Fixture 核心为在 `pi.on("session_start", ...)` 中 `await ctx.ui.confirm(...)`。这是已知上游限制；本阶段准确报告启动失败，不修复上游或默认取消用户尚未看到的问题。

### 扩展交互与后续 UI

| 能力 | 当前 RPC 边界 |
| --- | --- |
| select、confirm、input、editor | 标准请求和回复，本阶段完成类型与自动化往返；后续用 GPUI 控件呈现 |
| notify、setStatus、setTitle、set_editor_text | 通知类消息，不等待用户回复 |
| setWidget | 支持文本行、key 更新/清除和 aboveEditor/belowEditor 位置；组件工厂不传输 |
| custom、setEditorComponent、header/footer、working、原始终端按键与自定义补全 | TUI 组件或处理器不能经当前 RPC 搬入 GPUI；部分 API 不产生消息，宿主无法仅靠事件发现缺失 |
| get_commands 与 prompt | 发现并调用扩展命令、templates、skills；不代表覆盖所有终端内置命令 |

命令入口与补全规则见 [统一命令面板](../../../app/gupi/docs/dev/issue-226/command-palette.md)；本阶段只提供协议。

已核对本机 `@juicesharp/rpiv-ask-user-question` 2.9.0：终端使用 custom overlay 和独立 Editor；RPC 模式进入 rpc-fallback.ts，以 select/input 依次询问，多选输入 `1,3`，没有 tabs、提交复核、备注和并排预览。取消任一对话会把整个问卷报告为用户拒绝；宿主不支持不能自动伪装为用户取消。该源码实例证明标准协议可承接简化交互，不承诺完整 TUI 外观兼容。

### 资源与验证边界

Pi 的 EOF/SIGTERM 清理可能执行扩展 shutdown；内置 shell 在 Unix 使用 detached 子进程，并在 abort 路径清理进程树。只终止直接 Child 或 Pi 所在进程组不能保证全部工具后代、任意扩展自建进程都已回收。

Linux/Windows 实机、真实模型执行与任意扩展后代清理未验证。
