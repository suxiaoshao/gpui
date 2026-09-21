# 运行状态与过程展示实施计划

状态：Done（2026-09-15，本轮代码实现与自动验证完成，界面效果交付用户试用）。没有待确认的产品问题。未自动操作或启动 Gupi 窗口，未创建/注册测试 .app；不把自动验证当成视觉验收。

实施前的源码与样本证据见[运行展示对照报告](runtime-display-research.md)。本工作归属 Gupi #229，不扩展目录读取、搜索、连接管理或 Pi RPC 协议。

## 已确认行为与当前实现

| 项目 | 当前行为 |
| --- | --- |
| 过程区出现条件 | 初始等待与纯思考阶段只显示“正在思考”，不显示耗时标题或可展开详情。出现工具调用或已确认的非最终 assistant 正文后，显示带累计耗时的过程区，包含此前思考；不按思考条数或经过秒数决定。思考后直接最终回答不生成过程区。占位与后续 Run 使用同一行标识。 |
| 总耗时 | 思考、工具及最终回答输出期间按秒更新；最终回答开始或收起第一级不会停表，整轮执行结束后显示固定耗时。第一级文字没有 shimmer。 |
| 第一级折叠 | 未确认最终阶段时强制展开，鼠标、键盘和无障碍点击均不能收起；明确最终阶段后允许操作。没有提前信号的 provider 等回答结束确认。中断/错误过程保持展开。 |
| 内层折叠 | 与第一级独立。两个 assistant 正文之间的活动段只有一个工具或思考项时直接显示该项，不增加二级分组折叠；至少两个活动项时才显示二级分组，默认收起，包括当前运行组。单个工具与思考详情仍默认收起；允许用户切换并保留用户选择。 |
| 工具组摘要 | 二级分组标题不显示类型图标。最近运行组显示实际运行工具的“正在…”动作与目标，或实际思考项对应的“正在思考”；没有活动项正在运行时显示分类汇总，不因没有运行工具就推断正在思考。具体工具和思考项仍显示各自图标。 |
| 思考项 | 保留 Pi thinking_start 的空文本块并显示“正在思考”，尚无正文时不提供详情展开。最新未完成 assistant 的末尾 pending 思考块显示运行状态；开始后续文本/工具或消息结束后恢复“思考过程”。历史内容不会标成正在思考。 |
| 过程保留 | 新步骤不主动删除旧过程。Responses 同一消息中的前置说明与最终文本分开显示，最终回答旁的复制只复制该处显示的文本。 |
| Skill | 读取目标文件名为 SKILL.md 时识别为技能，显示父目录名；没有可用父目录时保留文件名。兼容正反斜杠与 path/file_path 参数，不读取正文猜类型。 |
| 图标 | 消息区、命令面板与历史树采用相同的语义映射。多调用的历史聚合节点保留通用工具图标。 |

运行时间与折叠不是同一个生命周期条件。第一级允许收起不表示 RPC 执行完成，也不改变发送、取消或重试行为。

消息块的普通间距为 2rem，与 MessageScroller 默认值一致；压缩记录与相邻过程维持 0.75rem。应用仍按相邻行类型控制间距，因此关闭 scroller 的默认行底部间距，避免叠加。列表首尾和左右留白由 scroller 默认样式负责，不在渲染行内再次添加。折叠标题使用 Marker 默认高度，标题与箭头保留已确认的 gap_1。

## 计时来源

当前计时优先从 **Pi 用户消息的 timestamp** 开始。它属于 Pi 会话记录，不是 Gupi 按下发送或开始连接的时间；实时消息与恢复历史都能取得同一来源，无需另外持久化计时记录。

- 首条消息尚未到达、只有 agent_start 时，使用本机接收该事件的时间临时显示。
- 缺少用户时间的历史片段回退到第一条 assistant 的 timestamp。
- 工具回合的 turn_start 不重置同一消息段的计时。
- 过程区出现时显示从原起点累计的耗时，不从首次工具调用重新计时；纯思考后直接回答不单独展示耗时标题。
- 当前 Run 仍 active 时取当前时间；Run 结束后取最后一条消息的 completed_at。实时 completed_at 来自 message_end 的接收时间，历史来自 entry 外层 timestamp。
- 现有 Run 结束仍由 agent_settled 决定，不用提前的 stop 状态替代。已完成时间表示消息执行结束时间，不额外计入随后 UI 刷新时间。
- 计时刷新由独立 `ProcessClock` 的 GPUI Task 驱动，只更新当前运行过程标题；停止或切到非运行会话时释放，不重新投影或重测整段消息，不触发目录扫描或 RPC 轮询。正文与 Markdown 的局部测量见[事件同步与局部刷新](../issue-236/README.md#局部刷新实现与边界)。

原实现从第一条 assistant 开始计算，运行中没有累计显示。当前将可取得的起点提前到 Pi 用户消息，包含等待模型首字与最终回答输出；实时和恢复历史使用同一提取规则。

## 最终回答语义

Pi 不需要与 Electron 使用同名顶层字段，但必须区分内部事件和 RPC 线上事件：

1. Responses 在内部 final_answer/text_start 时修改 partial.stopReason；当前 RPC 序列化会移除 partial 和累计 message，因此 Gupi 无法从该增量取得提前的最终阶段信号。此前把内部行为当成 RPC 能力的结论已纠正。
2. 当前 RPC 在 message_end 提供完整消息，Gupi 根据 stopReason 确认最终阶段。pending 正文仍实时显示，但不会仅凭没有 toolCall 就解锁第一级折叠；不必等 agent_settled 才展示消息结果。
   尚无法判定用途的最新正文先独立显示，并结束初始思考占位；随后出现工具调用、后续 assistant 消息或可用的最终分段信息时，已确认的中间正文进入过程区，不凭正文开始就假定它是非最终回复。
3. 完整消息中的 textSignature V1 JSON 可包含 phase=final_answer，实时结束校准与恢复历史均据此分开前置说明和最终回答。保留已有完整快照事件的处理路径，但不依赖当前 RPC 增量里不存在的字段。
4. message_end 不等于整轮执行完成。length/error/toolUse 等按实际事件处理；Run 仍由 agent_settled 结束，error/aborted 保持过程展开。

源码入口：`state/conversation.rs` 的消息事件处理、`state/history.rs::DisplayMessage::final_part`、`features/home/messages/activity.rs::RunContent::project`。

## RPC 增量接入

当前安装的 Pi 与本地 `packages/coding-agent/src/modes/json-event.ts` 一致：message_update 只有 usage 和 assistantMessageEvent，不携带累计 message 或 partial。旧接入要求 raw.message 存在，导致全部增量被跳过；Markdown 增量组件和运行标记因收不到数据而只能等待 message_end。

- `message_start` 建立 live 消息，Session 仅保留当前 assistant 的对应标识和尚未拼完的工具参数。
- text/thinking 的 start 按 contentIndex 建立空块，delta 追加文本，end 以完整块内容校准；usage 使用事件中的累计值。
- toolcall_start 取得工具 id/name，delta 累加参数字符串，JSON 完整时更新参数；toolcall_end 的完整 toolCall 为准，不新增不完整 JSON 解析器。
- `message_end` 用完整消息校准原来的流式消息并释放组装状态。即使 faux 中止消息改变 timestamp，也替换当前 assistant，而非遗留旧 pending 消息。
- 组装后的消息继续经过原有 Session.messages、RunContent 和 Markdown 呈现，不另建 UI 消息来源。agent_start/agent_settled 清理当前流关联。

回归样本 `tests/fixtures/rpc-message-stream.jsonl` 来自 2026-09-15 本机安装版 Pi 的隔离 Gallery RPC 录制，仅保留消息事件：三条 assistant、18 次思考增量、6 次正文增量、2 次工具调用开始，并在第三条的思考期间中止。测试逐条回放并检查 message_end 之前的消息列表投影，不仅校验最终文件。录制没有启动 Gupi 窗口或请求外部模型。

## 图标与 shell

| 对象 | 图标 |
| --- | --- |
| 普通文件读取 | FileText |
| Skill 读取与命令面板技能 | BookOpen |
| grep/find 搜索 | Search |
| ls 列目录 | Folder |
| 插件命令入口 | Puzzle |
| Bash / PowerShell 执行 | Terminal，文字保留 Bash / PowerShell 名称 |
| 写入 / 编辑 | FilePlus / FilePenLine |
| 未知工具 / 多工具历史聚合 | Wrench |

共享分类位于 `foundation/tool_presentation.rs`。历史从原始调用参数提取分类并沿 toolCallId 关联结果，不根据已截断的标题猜测 Skill。命令面板复用相同的 Skill 图标。

插件命令入口与实际 shell 执行是不同对象。插件内部运行 shell 不改变它的入口图标，也不为 RPC 未报告的内部动作虚构工具行。

本机 Pi 0.85.1（源码提交 71dca871b）已核对：

- 模型工具集合有独立 bash/powershell，RPC 工具事件保留 toolName，Gupi 可直接区分。默认 coding tools 仍是 read/bash/edit/write，PowerShell 不因此自动启用。
- Windows Bash 优先指定 shellPath，然后 Git Bash、PATH 上的 bash.exe；PowerShell 独立查找 pwsh.exe/powershell.exe。
- 顶层主动执行 RPC 只有 type=bash，没有 PowerShell 请求或 shell 选择字段。本次没有增加此类入口。
- Pi TUI 共用 shell renderer，分别显示 `$` 与 `PS>`。
- 当前实现接入 PowerShell 的 command 提取、命令分类和图标；没有进行 Windows 实机执行验证。

## 临时 Gallery

`tests/fixtures/runtime-gallery.mjs` 的队列现在保存 FauxResponseFactory，在每次实际请求时创建回答和时间戳，最终回答同样处理；保留慢速输出和真实工具等待，不手工递增时间戳。

工厂修复针对原样本中 13 条回答批量构造、共用时间戳的问题。新消息仍沿用角色+时间戳匹配；本次 RPC 接入另外按当前流关联 assistant 的结束消息，处理 faux 中止时改变时间戳的情况。这不宣称解决全部可能的生产碰撞。

试用：仓库根目录执行 `node script/gupi-runtime-gallery --no-build`。使用刚构建的 target/debug/gupi，发送任意文字体验两轮过程；不创建新的 .app。

## 实现范围与验证

消息呈现沿用 gpui-kit 0.6.0：整轮 assistant 与连续过程内容用 `MessageGroup`，单条消息用 `Message` / `MessageContent` / `MessageFooter`，分支摘要标题使用 `MessageHeader`；过程折叠继续使用 `Marker` + `Collapsible`，保持既定折叠规则。

Markdown 接入 `TextViewState` 与 `TextView::new`。呈现层按文本块保存最后提交的快照，末尾增长调用 `push_str`，相同内容不更新，替换或缩短调用 `set_text`。比较基准为已提交快照，避免异步解析尚未完成时重复追加。状态随可见元素存在；折叠或虚拟化后重新挂载时，以当前完整快照初始化，不修改 RPC 数据模型。解析完成通知 `MessageScroller` 重新测量高度，继续沿用列表的阅读位置与跟随规则。适配层直接返回 `TextView`，不增加 padding、margin 或布局容器。

实现位于消息投影/呈现、历史分类、共享图标、两种语言文本和临时 fixture。没有新增依赖、RPC 字段、数据库或计时持久化格式。

已完成：

- 消息回归：首字前稳定占位、纯思考后直接回答不生成过程区、工具/中间正文生成过程区并保留思考、pending 正文不解锁、已确认最终阶段解锁、同一消息保留前置说明、历史 final signature、错误状态、时间包含最终回答输出与结束后固定。
- 历史回归：Windows Skill 路径、普通读取、PowerShell 名称与分类，以及现有历史分支/折叠相关测试。
- `cargo test -p gupi messages --locked --offline`（21 项，包含过程区出现条件、安装版 RPC 录制逐条投影、思考/文字/工具参数增量、结束校准与中止时间戳变化，以及原有消息、Markdown 与相关历史/加载回归）。
- `cargo build -p gupi --locked --offline`、`cargo clippy -p gupi --all-targets --locked --offline -- -D warnings`、`cargo fmt --all -- --check`。
- 临时插件 JavaScript 语法检查；新增 Fluent 键与变量中英文一致性检查。

未进行自动窗口交互验证，也未重新自动跑完整慢速 Gallery。思考占位到过程区的切换、鼠标/键盘折叠、组摘要和图标实际观感交付用户体验；这些不记为已通过的视觉测试。
