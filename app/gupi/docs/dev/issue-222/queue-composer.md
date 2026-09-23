# 队列交互与输入框布局

状态：当前 RPC 支持的队列交互已实现并通过构建与回归；原生点击/外观验证未完成，逐条操作延后。2026-09-20 更新。归属 #222。资源标签与 Skill/模板输入继续由[输入框资源接入计划](../issue-243/README.md)管理；扩展协议与体验环境见[扩展 UI 计划](README.md)。

## 本轮实施范围

用户确认先使用现有接口完成。已接入 `queue_update`，按本轮补充/后续任务展示实时队列，默认折叠并可展开。支持清空全部、全部文字取回草稿；取回依照 Pi TUI 顺序（steering、follow-up、当前草稿，空行分隔）。保留当前草稿已有附件，不自动发送，不从文本推断或恢复已发送附件。

取回操作明确标注“文字”，展开区域说明排队图片无法恢复。操作以 `clear_queue` 实际返回值为准，不使用点击前的缓存冒充返回内容；错误不改变草稿，响应应用到原会话并检查连接 binding。等待时允许继续改草稿，禁止重复操作与提交；回填使用响应时的最新草稿。队列事件负责内容，状态快照校准数量，晚到响应不清掉后续新入队内容。

运行中有草稿时同时提供发送与停止按钮，发送箭头执行本轮补充，相邻菜单可选择本轮补充/后续任务；原 Enter/Alt+Enter 语义保持。项目选择器位于新会话页面的欢迎语中，与输入框解耦，累计 Token 统计保留。

文件和图片使用 `AttachmentGroup`／`Attachment`，放入共用 InputGroup 的 `BlockStart` 插槽，多附件横向滚动；没有附件时不渲染插槽。文件名下通过 `AttachmentDescription` 显示格式与大小（如 `PDF · 128 KB`）；文件大小在添加时读取，图片按原始字节计算，单位采用十进制 B/KB/MB。保留文件打开、图片预览及独立删除，点击删除不触发预览。附件入口与空闲发送使用 `InputGroupButton`，运行中的发送和模式菜单使用 `DropdownButton`，停止按钮独立保留。

输入区保留原有 Attachment 横向卡片及 AttachmentMedia.src() 图片展示；点击图片打开窗口级预览，沿用组件弹层的窗口边距，按可用空间等比适配；提供缩放比例、放大/缩小（最高 800%）、Ctrl/Cmd＋滚轮及触控板捏合，放大后可滚动查看。关闭按钮、Esc 和图片外空白均可关闭；图片与工具栏操作不关闭预览。Gupi 自有预览模块沿用 Jaco 的缩放几何，不依赖 Jaco 包。预览与 RPC 使用同一份原始图片字节，不缩小或重编码。文件名使用浮动胶囊，关闭按钮独立放在右上角，底部使用 2.5rem 缩放按钮和比例读数，与 Jaco 的预览布局一致。GUI 与 Pi 的职责整理见[边界收敛记录](../../../../../docs/dev/issue-217/gui-boundary.md)。

本轮不新增逐条编辑/删除、排队图片恢复、客户端调度队列或原输入备份；扩展问答和资源标签的完整布局改造仍按原有依赖计划推进。以下逐条交互是后续目标，不能当成本轮已完成功能。

## 后续交互目标

### 队列按条目操作

队列应展示待执行内容，并提供以下逐条操作，不能以仅显示数量或清空全部代替。

| 操作 | 目标行为 |
| --- | --- |
| 返回草稿 | 从待执行队列撤回该条，将文字和附件放回输入框，不自动发送。已有草稿不能被直接覆盖，合并方式待确定。 |
| 编辑 | 编辑后更新该条，取消保留原内容。原草稿提出行内编辑；本次 Codex 调研发现它使用主输入框编辑，呈现方式需按下文研究结果再确认。 |
| 删除 | 仅移除选中条目，不影响其他排队内容。 |

每条右侧提供对应 icon 按钮和悬停提示。区分“本轮补充”（steering）与“后续任务”（follow-up），操作不应无意改变原来的类别或其他条目的顺序。已经被 Pi 消费的消息不再属于可编辑队列；如何可靠判定并处理操作期间出队，属于接口调研的一部分。

### 项目选择器只在新会话页面显示

- 放在新会话页面中央欢迎语中：“你想在 gpui⌄ 中做些什么？”。项目名使用原生按钮，点击选择工作目录，悬停显示完整路径，长名称截断。
- 页面负责项目选择；输入区只负责编辑、附件与发送。移除旧项目背景条、负 margin 重叠和标签宽度测量，InputGroup 保留组件默认外观。
- 已有会话和运行中不常驻项目切换控件；临时窗口不增加项目选择器。
- 进入正式会话后移除该区域，输入框内部布局保持一致。
- 当前实现已用“非临时会话、会话路径为空、无待答扩展 UI”限制显示，并在 busy/命令执行中禁用。后续布局调整保留新会话选择目录的语义，不扩大成已有会话切换项目。

设计图 E/F 中“项目放在输入框内部”的比较已被本节替代；A–D/H 中常驻项目控件的画法不作为实现依据。

### 保留累计用量

会话累计 Token 用量与上下文占用是不同信息，不能用上下文百分比替换现有输入、输出、缓存统计。保留详细提示中的累计输入/输出、缓存读取/写入、最近缓存命中率和 Pi 统计费用。

布局方向：统计放输入区底部左侧；模型、上下文占用、附件和发送/停止在右侧。只按应用实际允许的窗口和分栏宽度检查布局，不新增过窄窗口模式；具体控件尺寸不由草图直接决定。

## Pi 接口边界与当前实现

2026-09-20 核对本地 Pi 的 `packages/coding-agent/src/modes/rpc/rpc-types.ts`、`rpc-mode.ts` 和 `core/agent-session.ts`：

- 提供 `prompt`（含 streamingBehavior）、`steer`、`follow_up`，可提交文字与图片。
- `queue_update` 提供 steering/followUp 两类字符串数组；不含稳定条目 ID 或完整附件。
- `clear_queue` 清空两类队列并返回字符串数组；不是逐条撤回接口。
- 尚无按 ID 修改/删除单条的 RPC，也没有包含图片的完整队列快照。
- Pi 内部实际入队内容包含图片；文本事件不能代替完整消息。Skill/模板还会经过展开，不能假定队列文本等同于原输入。

Gupi 已接入 `clear_queue` 的整队操作和 `queue_update` 文字展示，产品界面尚未接入逐条操作。已发布的 InputGroup 已接入普通 Composer，但控件升级不会补齐 Pi 的队列契约。

不能直接用“清空全部，再重新提交其他条目”实现逐条编辑/删除：这会涉及消息消费时机、顺序、附件保留和再次展开的差异。也不因设计要求自动新增客户端离线队列或第二套草稿备份。完整方案需先调研再确定，不能宣称现有 RPC 已支持。

源码入口：

- [输入区](../../../src/features/home/composer.rs)、[新会话欢迎区与项目选择](../../../src/features/home/welcome.rs)
- [共用 Composer](../../../src/features/composer.rs)
- [会话状态](../../../src/state/conversation.rs)
- [RPC 客户端](../../../../../crates/pi-rpc/src/client.rs)

## 本轮确认的其他布局

- 运行中提供“本轮补充／后续任务”发送模式入口，停止按钮独立保留。
- 队列默认显示紧凑摘要，可展开查看与逐条操作。
- 插件提问使用同一输入区域，结束后恢复原草稿，不增加“草稿已保留”等重复说明。普通草稿继续由既有 Entity 持有，不因此新增备份存储。
- 不设计独立过窄窗口模式，也不照草图 H 增加新的临时窗口布局。检查已有最小尺寸和分栏边界即可。

## 参考实现调研

### Pi TUI：全部取回，排队文字在已有草稿前

2026-09-20 核对本地 Pi checkout `71dca871b`，`packages/coding-agent/src/modes/interactive/interactive-mode.ts` 的 `handleDequeue`、`clearAllQueues`、`restoreQueuedMessagesToEditor`（约 4157、4353、4388 行）：

1. 取出并清空全部队列，包含 TUI 自己的压缩期间队列。
2. 先拼接 steering，再拼接 follow-up，各消息之间使用两个换行。
3. 取现有编辑器文字；最终使用 `[queuedText, currentText].filter(t => t.trim()).join("\n\n")`。
4. 写回编辑器，不自动发送；普通 dequeue 调用没有要求中止当前运行。

例如，队列是 A、B，当前草稿是 C，取回结果为 `A\n\nB\n\nC`。这与此前建议的“追加在现有草稿后”不同。它没有逐条行内编辑，也没有把两种队列模式作为结构化草稿保留。该路径只处理字符串，不能据此声称附件无损回填。

Gupi 可参考的建议：逐条返回草稿时采用“撤回文字在前、原草稿在后、空行分隔”。这仍是建议；附件和开头命令的合并规则不能由 TUI 的文本拼接直接推导。

### 本地 Codex Electron：移出单条，在主输入框编辑

只读解包 `/Applications/ChatGPT.app/Contents/Resources/app.asar`，应用版本 `26.915.31945`，归档 SHA-256 为 `1f7939c1c781887c167043c4d1d307af3400d324685cfc315dfe2f80e634f483`。临时解包目录为 `/tmp/gupi-codex-queue-study-20260920`；未修改或运行解包代码，也未做原生交互验证。

关键证据位于解包产物的 `webview/assets/`：

| 文件与符号 | 源码行为 |
| --- | --- |
| `queued-message-list-538322429aba.js`，`Ce` / `Te` | 每条使用稳定消息 ID，展示单行摘要、可选图片预览；删除为 icon 操作，“编辑消息”在更多菜单中。编辑行并不渲染独立输入框。 |
| `app-initial-a498f911edeb.js`，`Dio` | `beginEdit` 映射为 `remove`，`restoreEdit` 映射为 `restore`。 |
| 同文件，`removeQueuedMessage` / `restoreQueuedMessage` | 移除返回原消息、index、前后相邻消息 ID；恢复按相邻 ID 和原 index 定位，而非只靠文本匹配。 |
| `app-primary-426732871368.js`，`T4e.handleEditMessage` | 先等待 `beginEdit` 成功，再把消息文字和附件上下文放入主 Composer 并聚焦。若等待期间正文或相关草稿状态改变，恢复该队列条目，放弃这次载入。 |
| 同文件，`T4e` 的 undo / redo | 撤销可恢复队列消息和编辑前的正文、附件等状态；这是应用撤销流程，不能当成已有的队列行内“取消”按钮。 |
| 同文件，`U3e` | 有编辑位置时采用 queue 模式，并将 `editPosition` 交给提交路径；接受提交后清除编辑标记。 |

结论：已移出的消息在编辑期间不再属于待执行队列。这条编辑路径没有暂停整个队列，也没有“占住队头等待编辑”的机制；其他排队内容仍受原有执行条件管理。消息已经不存在时，`beginEdit` 返回空，不继续把旧内容载入编辑器。

本次证据支持“先移出单条，再编辑和重新提交”的方案，不能支持此前未调研的“行内编辑且暂时冻结该条”推断。Codex 拥有完整消息上下文与逐条队列操作；Pi 当前字符串队列接口不能直接照搬。Codex 的拖拽排序、旁支任务等其他功能不因此纳入 Gupi 范围。

## 逐条操作的可行性与实现边界

继续核对同一本地 Pi checkout `71dca871b`，工作区无未提交改动。以下结论仅针对该源码版本，不推断远程新版本、后续迁移时间或本机安装包已经具备这些接口。

### 现有扩展接口不能直接补齐

`core/extensions/types.ts` 的 ExtensionContext 只暴露 `hasPendingMessages()` 等状态；`sendUserMessage` / `sendMessage` 可入队，但没有枚举、取回或删除单条的方法，也没有将 Agent 实例交给插件。`input` 钩子可以 continue/transform/handled，能改写输入或接管处理，不等于可以操作已经进入 Agent 的队列。

因此，普通插件无法仅用现有公开接口增加所需逐条操作。若拦截全部输入并自行维护队列，就已改变队列所有者和发送时机，不是一个小插件补丁。本次不安装插件、不使用私有字段或 monkey patch，也不写自定义 RPC wrapper。

### 旧 Agent 的消费边界比界面事件更早

`packages/agent/src/agent.ts::PendingMessageQueue` 仅有 enqueue/drain/clear；drain 会立即取出一条或全部消息。`agent-loop.ts::runLoop` 随后持有 `pendingMessages`，经过 prepareNextTurn 等步骤后才逐条发出 message_start。

coding-agent 的 `_handleAgentEvent` 到 message_start 时，才按文字 `indexOf` 从 `_steeringMessages` / `_followUpMessages` 移除并通知 UI。因此：

- 界面仍看见“排队”不保证消息尚在底层可撤回队列里。
- 先读文本快照再 clear/requeue 无法消除这个间隙；消息可能已经被底层取出，随后重复提交。
- 相同文字、不同附件的消息不能可靠地靠内容匹配；空文字图片消息也不能以文本作为身份。
- 可靠操作必须在真正消费队列的所有者处判定是否已消费，不能只从 message_start 推断。

### 新 Harness 已有可借鉴基础，但 RPC 尚未使用

同仓库 `packages/agent/src/harness/agent-harness.ts` 已定义：

| 能力 | 本地源码现状 |
| --- | --- |
| 入队返回身份 | QueueResult 返回 `entryId` |
| 完整队列消息 | LaneQueuedItem 包含 `entryId`、`kind` 和完整 AgentMessage；字符串入队会合并图片 content |
| 单条取消 | `cancelQueued(entryId)` 返回 `cancelled` / `already_consumed` / `not_found` |
| 状态与更新 | LaneSnapshot 和 queue_update 都包含结构化 queues |

`harness/runtime/lane.ts::cancelQueued` 在 lane command 提交中判断并删除 pending 项；找不到时查询已消费 entry，以区别已经消费和不存在。`test/harness/runtime/drive-public.test.ts` 已有三种结果的测试，本轮只阅读测试，未运行。

但 `coding-agent/src/core/sdk.ts` 仍创建旧 `new Agent(...)`，RPC 仍调用 AgentSession 的文字队列接口。不能从 Harness 有 cancelQueued 就声称 Gupi 可直接调用，也不为队列 UI 顺带迁移整个 Pi runtime。

Harness 仍未提供本次所需的全部语义：cancelQueued 只返回结果，不原子返回被取出的完整条目；未发现按原相邻位置恢复/插入的公开接口；队列中保存的执行消息也不自动等于用户的原始 Skill/模板输入。

### 最小需要的能力契约（建议，非现有 RPC 名称）

不要求先实现一个独立的“编辑队列”后端：编辑可拆为取出与重新入队。但以下能力需由真正的队列所有者提供：

1. **身份和快照**：稳定条目 ID、类别、顺序、完整文字/图片；入队应能关联提交与条目。快照读取不能靠清空队列实现。
2. **原子取出单条**：成功时返回完整消息与位置；已经消费/不存在时明确失败，不让 Gupi先载入旧副本或假装删除成功。
3. **带位置重新入队**：恢复时参考仍存在的前后条目，避免只使用会变化的数组下标；原相邻条目已执行时需要明确退化位置。取消编辑恢复原消息时不能重新触发输入插件或再次展开模板；编辑后提交新内容则走正常输入处理。
4. **同步消费结果**：队列变化和消费边界一致，客户端处理延迟响应时不得复活已消费条目；按真实协议选择序列或版本机制，不预先发明字段。

删除只需成功移除；返回草稿和编辑必须在取出成功后才改变编辑器。`already_consumed` 时更新显示并告知消息已开始处理，不重发、不打断当前执行。网络结果不明时重新获取权威状态，不将“超时”当成“删除成功”。

### 原始输入与执行消息不能混为一份

Pi 在入队前执行 input handler，再展开 Skill 和模板；输入插件还可改变图片。Gupi 文件附件在提交时转成 `@路径`，图片进入 RPC images；提交成功后会清理已发送的编辑器内容。因此现有队列文字无法无损还原命令标签、文件附件卡片和原始草稿。

应区分：

- **原输入呈现**：用户输入的命令、参数、文件与图片选择，供编辑/返回草稿。
- **实际执行内容**：Pi 经输入处理后的消息，供未修改的撤回恢复，避免重复触发插件。

如果后续接口只返回完整执行内容，那么最多能编辑展开后的内容，不能承诺还原原命令。这一差异需要明确。若为已成功排队条目保留原输入元数据，它必须关联权威队列 ID、随消费/删除释放；它不是额外持久化临时窗口草稿，也不是失败输入恢复队列。在可靠 ID 契约确定前不新增这类存储。

### 方案比较与建议

| 路径 | 能否覆盖目标 | 结论 |
| --- | --- | --- |
| 不改 Pi，只消费当前 RPC | 可显示两类文字和数量、全部取回文字；不能可靠逐条撤回/编辑附件 | 可做有限界面工作，不能宣称队列交互完成 |
| 清空全部后重新排队 | 存在已被 drain 的消息、附件缺失、身份混淆、重复输入处理等问题 | 不采用 |
| 普通 Pi 扩展 | 缺少读取/取出已排队消息的公开能力 | 无直接可用方案 |
| Gupi 自己持有全部待发内容 | 可控制本地编辑，但需重做调度；仅在 agent_end 发送会把 steer 变成 follow-up，也无法统一插件入队 | 不作为本轮默认方案 |
| Pi 队列能力经正式 RPC 暴露 | 能保持 Pi 执行语义，Harness 已有部分基础 | 推荐方向；先与上游能力边界对齐，再接入 Gupi |

当前范围：先交付现有 RPC 支持的文字展示和整队操作。逐条操作继续等待正式 RPC 契约，不维护平行队列，不在本轮修改 Pi 或创建上游 Issue/PR。另核对正式 v0.85.0、v0.85.1、[v0.86.0 SDK](https://github.com/earendil-works/pi/blob/v0.86.0/packages/coding-agent/src/core/sdk.ts) 和 [RPC 类型](https://github.com/earendil-works/pi/blob/v0.86.0/packages/coding-agent/src/modes/rpc/rpc-types.ts)：coding-agent 仍使用旧 Agent，2026-09-23 再次核对 [v0.87.1 SDK](https://github.com/earendil-works/pi/blob/v0.87.1/packages/coding-agent/src/core/sdk.ts) 与 [RPC 类型](https://github.com/earendil-works/pi/blob/v0.87.1/packages/coding-agent/src/modes/rpc/rpc-types.ts)：仍为 new Agent，rpc-types.ts/rpc-mode.ts 与 v0.86.0 完全相同。升级到 0.87.0 仍不会解除逐条队列限制；新的 context_edit、finishTurn 和插件边界钩子不构成队列 ID/单条操作接口。

## 窗口与分栏的实际约束

以下为当前源码值，单位为 GPUI 逻辑像素；这是源码核对，不是此次实机测量。

| 区域 | 已有约束 | 证据 |
| --- | --- | --- |
| 主窗口 | 默认 960×740，最小 800×600 | `state/layout.rs::minimum_window_size/default_window_size`；`app.rs` 将最小尺寸传入 WindowOptions |
| 主窗口正文栏 | 分栏算法保留至少 360px | `features/home/panes.rs::CONTENT_MIN` 与 fit/resize |
| 主窗口左栏 | 拖动范围 160–480px，并受正文可用空间限制 | 同文件 LEFT_MIN / LEFT_MAX |
| 主窗口历史栏 | 240–520px；窗口宽度小于 1100px 时改为覆盖层 | 同文件 RIGHT_MIN / RIGHT_MAX、overlay |
| 临时窗口 | 默认固定 960×620，`is_resizable: false` | `app/temporary.rs::WINDOW_SIZE/window_options` |
| 临时窗口内部左栏 | 默认 280px，可调 220–420px | `features/temporary.rs` 的 resizable_panel |
| 临时窗口正文区域 | 由剩余宽度决定，右侧没有显式 size_range；按左栏上限估算约 540px，尚未扣分隔线和内容内边距 | 同上；`features/home.rs` 的空会话区域还使用 px_8 两侧内边距 |

已有最小尺寸并不表示输入框底部所有控件必然能排成一行：主窗口正文允许缩到 360px。实现时只需让控件适配这些现有有效边界；不增设新的过窄窗口状态，也不在本次研究中擅自提高窗口最小尺寸。

## 延后功能的待确定项

1. 完整附件与原始 Skill/模板命令的恢复规则。本轮文字顺序已采用 TUI 行为，不再列为待定。
2. 是否采用 Codex 的主输入框编辑流程，替代原提案的队列行内编辑；采用后“返回草稿”和“编辑后回原队列位置”的区别，以及取消/撤销呈现需一起定清楚。
3. 实现边界已查明：现有 RPC 与扩展 API 不足，新 Harness 尚未接入 coding-agent RPC。本轮先做现有接口支持的交互，后续逐条操作需补齐正式 RPC。
4. 返回编辑器时要求恢复原命令/附件选择，还是接受 Pi 展开后的执行消息；现有字符串事件不足以两者兼得，需与接口契约一起确定。

## 后续实施与必要验证

先完成接口调研并解决上述行为边界，再确定队列状态归属和数据契约，随后接入事件与逐条交互，最后调整共用输入区布局。该顺序适用于后续逐条交互；本轮整队操作已获用户授权实施。

实施后重点验证：单条操作不影响其他条目；相同文字的不同消息不混淆；图片/文件引用不丢失；保存、取消和返回草稿不覆盖现有输入；操作与消息出队重叠时结果明确；新页面之外不出现项目选择器；累计用量与上下文占用分别展示。沿用受影响构建与关键回归，再检查实际窗口布局。

## 本轮验证结果

- 项目选择迁移到欢迎语后，Gupi 构建与全目标 Clippy 通过，`cargo test -p gupi composer --locked` 的 3 项回归通过；中英文欢迎语的 Fluent 参数一致。此次布局尚未原生视觉复核。

- `cargo check -p gupi --locked`、`cargo build -p gupi --locked` 通过。
- `cargo test -p gupi -p pi-rpc --locked`：Gupi 201 项、pi-rpc 11 项单元测试、12 项传输测试和 1 项文档测试通过。新增覆盖 TUI 合并顺序、重复文字保留、整队清除、失败不改草稿、回填最新草稿并保留已有附件、跨会话响应归属、晚到清空响应不抹掉后续入队，以及重复点击只发送一次 RPC。
- 附件组件接入时，`cargo test -p gupi --locked composer`：3 项通过。图片预览升级后，`cargo test -p gupi --locked image_preview` 的 6 项几何回归通过；`composer_attachments` 实际临时窗口集成测试通过，覆盖附件位于 InputGroup 内、预览范围与图片适配、放大/缩小、普通滚动不缩放、关闭按钮/Esc/空白关闭、点击图片不关闭、删除不触发预览且保留草稿、删除最后一项后移除附件插槽。
- `cargo clippy -p gupi --all-targets --locked -- -D warnings`、格式检查、diff 检查及队列中英文 key 一致性检查通过。
- macOS 使用临时配置和无模型调用的模拟 Pi 启动了本次构建。原生工具最初重新启动未继承隔离环境的实例，未向其发送测试消息；随后固定隔离启动入口，确认测试进程及模拟 Pi 启动，但工具持续连接超时，未完成队列按钮和外观实机验证。已结束本次测试进程。此结果不等于真实 Pi 端到端或原生视觉验收通过。
