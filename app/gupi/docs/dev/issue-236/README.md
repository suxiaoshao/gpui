# 事件同步、重试进度与会话局部刷新

状态：Done。归属 [#236](https://github.com/suxiaoshao/gpui/issues/236)，父 Issue #217。2026-09-21 首轮事件接入及追加的刷新范围修正已实现；用户确认 Q-01～Q-03 按建议执行，均已落实。受影响回归和验证边界见文末。错误、插件提示和回答完成通知的新增及统一改造继续延后，统一记录在[待处理总文档](../../../../../docs/dev/issue-217/follow-ups.md#通知与用户注意力管理延后)。

## 目标与边界

单个 session 的变化只更新其业务数据和真正受影响的界面；同步名称、模型设置、历史和重试进度，其他会话和前台草稿、焦点、滚动位置保持稳定。需要区分三种“刷新”：

| 层次 | 实际工作 | 本轮目标 |
| --- | --- | --- |
| 会话目录 | 发现文件、读取所有会话元数据、排序及合并 | 普通会话内容变化不触发全目录扫描；保留必要的目录发现 |
| session 回读 | 请求该实例的 state、entries、stats、fork options 等 | 按数据变化选择请求，不以一个字段变化触发全部回读 |
| 界面更新 | 控件同步、列表投影、重绘、消息尺寸测量 | 只使相关区域失效；正常 render 不等同于重新请求数据 |

不为事件覆盖率新增功能；不实现直接 Bash、任意 TUI 组件、逐条队列管理或新错误中心；不新增数据库、文件监听器、轮询发现机制或第二套 session/队列数据。本轮不需要升级 Pi 或 GPUI Kit。新增错误展示、插件提示分级/路由、回答完成通知及系统通知均延后；现有报错、插件 notify、交互请求和操作反馈保持，不借延后决定删除已实现行为。本文的状态变更通知（GPUI observe/emit/notify）与面向用户的提醒是两个概念，前者仍在本轮优化范围内。

## 依据与已确认事实

Gupi 基线 `df623fdadbbe99e83c423c9e007a0dc316a12b92`，分支 `codex/236-session-event-sync`，GPUI Kit 0.6.4。Pi 参考本地干净 checkout `d1230ea2000d876b479a69b8b061f9d670f262f5`（package 0.86.0），并核对对应官方源码；这是固定版本调查，不宣称覆盖未来 Pi 版本。

- Pi [事件定义](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/core/agent-session.ts#L149)、[RPC 转发](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/rpc/rpc-mode.ts#L349)、[TUI 消费](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/interactive/interactive-mode.ts#L3273)。RPC 转发 session 事件，扩展错误另以 `extension_error` 输出。
- `entry_appended` **不只包含 custom**：0.86.0 的 cache warmer 会发出 `usage / cache_warm` 条目，TUI 分别处理两类；见 [cache warmer 回调](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/core/agent-session.ts#L403)。旧盘点中“只由插件 appendEntry 触发”的结论不能用于本轮实现。
- `session_info_changed` 只有 name，可为 undefined；JSON 中 name 缺失可以表达清空名称。`thinking_level_changed` 只有 level，没有模型身份或完整 state。
- `summarization_retry_scheduled` 有 attempt/maxAttempts/delayMs/errorMessage，但没有 source；source 只在 attempt_start 出现。finished 不带成功/失败信息或任务 ID。不能凭空给首次等待提示认定来源，也不能将 finished 当作整个任务结束。
- `agent_end` 不是完整运行结束；原有 `agent_settled` 收尾语义保持。TUI 与核心同进程，直接读内存数据的方式不能照搬为 GUI 全量 RPC 请求。
- [`get_entries(since)`](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/rpc/rpc-mode.ts#L638) 返回给定 ID 之后的条目及当前 leafId；ID 不存在报错。这不是通用差量补丁或会话身份校验。当前实现缩小请求触发范围，保留全量历史读取，未接入增量历史协议。

## 实现设计

### 所有权和通知范围

保留 `ConversationState` 管理 session 集合、选择和连接归属；`Session` 继续拥有事实状态、历史、草稿和读取任务，Pi 实例生命周期仍由 `PiState` 管理。输入组件 entity 继续保留焦点、编辑和滚动状态。

业务变更通过 `ConversationEvent::Changed(Changes)` 在当前 GPUI 更新结束时合并发布。批次区分来源 session 的导航/控件、正文、选择、目录结果及扫描进度；没有固定毫秒节流。正文事件不驱动命令候选和模型控件，扫描进度不重新分组导航；同次操作多个内部步骤对同一来源的通知合并。Pi 连接生命周期携带实例 ID，只更新持有该实例的 owner。

Session 仍由 ConversationState 持有，输入 entity 保留编辑、焦点和滚动状态。主侧边栏复用项目视图及导航投影，展开/查看更多只调整已有组，不重读全部 infos；选择更新既有组选择态。临时列表按来源比较并原地更新行，不再复制完整 rows 后比较。正文仍以现有消息投影为基础，变化行、折叠操作和 Markdown 异步解析使用 `remeasure_items`；行位置映射随投影更新，异步解析完成按稳定行 ID 查找当前 index。正常布局、绘制和必要的完整历史投影保留，不承诺零遍历或零父视图 render，也不使用造成原生 AX 丢失的绘制缓存。

### 事件处理表

| 事件 | 写入与界面影响 | 回读范围 |
| --- | --- | --- |
| session_info_changed | 来源 session 名称、state 中的 session_name、对应导航摘要；空名称恢复既有回退标题 | 同步名称本身不发 RPC；若需要展示新增 session_info 历史记录，定向使该会话历史失效，按当前可见历史需要回读 |
| thinking_level_changed | 更新当前模型设置中的 level 和选择器；未改变则不重绘正文 | 模型设置正在执行时沿用其最终 get_state 校准；其他来源不能仅凭 level 确认模型身份，可定向 get_state 校准 model/level，不读历史或目录 |
| entry_appended/custom | 来源历史失效，不直接当成聊天消息；沿用已有历史条目呈现规则 | 合并该会话的 entries 回读，不读其他实例/全目录 |
| entry_appended/usage | 来源历史及累计用量失效；不新增缓存预热卡片 | 定向 entries/stats；不复制 Pi 费用计算 |
| entry_appended/其他类型 | 保留来源会话的历史同步能力，不猜测其聊天语义 | 需要时回读该会话历史，不按已知两类把未来条目丢弃 |
| extension_error | 专门错误展示延后；不能因未消费事件使正文失效，也不改变运行结束状态 | 无回读；后续通知设计见总文档 |
| compaction_start/end | 同步压缩状态，取消与失败分开；成功压缩改变历史/上下文，保留已有命令报错，新增错误提示延后 | 手动压缩由命令完成回调进行 History 校准，其他来源由 compaction_end 校准；历史真变化才刷新 stats/fork，不扫描目录 |
| auto_retry_start/end | 保存本次模型重试展示信息；成功清理本次重试提示，保留已有最终失败处理，不新增通知 | 等待不回读历史；最终收尾按运行/消息变化处理 |
| summarization_retry_* | 独立的摘要重试展示阶段；scheduled 可先显示泛称，attempt_start 才有来源；finished 只结束重试等待展示 | 不因为重试提示变化读取历史或目录；结果由对应压缩/运行事件负责 |
| queue_update | 更新现有文字队列与数量；影响队列区域、发送模式及必要运行状态，不影响未变化正文 | 事件足够时不额外请求；清空后的兼容校准最多读取所需 state 字段 |
| message/tool 事件 | 正文或工具内容更新；运行状态转换可影响当前过程块 | 流式期间不逐 token 请求历史；结束后定向校准 |
| agent_end / agent_settled | 保持完整运行结束语义；收敛相邻结束事件的重复回读 | 以 settled 为普通最终校准点；仍需覆盖无 settled 的手动压缩/命令等独立路径，不一刀切删除回读 |
| turn_start/end、无业务处理事件 | 不因为收到事件就使正文失效 | 不自动回读；不增加轮次 UI |

条目事件没有完整 entries 快照及可靠的最终 leaf，初次实现不直接凭 entry 推断整个分支结构。已有历史加载采用全量 `get_entries` 仍可保留；目标是只在需要时读正确 session，而非本轮同时实现所有增量加载优化。

### 读取与并发契约

1. **拆请求，不复用过宽的副作用。** `ReadScope::State` 只取 state；共用的 `apply_snapshot` 仅在携带 entries 时替换历史或清理 live/tools。History 附带 state 校验身份，Full 用于初次连接和显式完整刷新；历史、stats、fork 列表按实际变化分别校准。
2. **来源固定。** 任务捕获 key、实例 binding 和必要请求版本；切换 selected 不改变结果目标，关闭/重连后的旧结果不得写回。插件触发的会话切换也需核对返回 session 身份，不把其他会话状态填进原目标。
3. **区分 UI 版本与回读保护。** content_revision 只表示正文投影真正失效；历史有独立 revision。并发保护针对请求所读数据，不能因为无关通知无止境废弃有效结果，也不能为减少重绘直接删除保护。
4. **设置命令与事件分别保护。** `model_revision` 标识设置命令，`settings_event_revision` 标识后续思考等级事件；事件不会使原命令失去收尾资格。命令最终通过 get_state 校准身份、模型和思考等级，若期间又收到设置事件则继续校准，解除未确认状态后再更新控件。
5. **连续请求合并。** 同一 session/资源保留一个在途读取及必要的待补读。首轮已实现 core 范围合并和 stats/fork 待补读；stats 使用 usage/model 版本，不再因无关队列事件丢弃有效结果。合并仍可能产生一次不必要的后续读取，不能替代对上游触发原因的收敛。
6. **无变化不使投影失效。** 比较实际条目/leaf、字段或既有权威版本；不能只比较历史条目数，也不引入一份完整历史副本用于比较。已有内容刷新失败时保持旧内容和局部错误，不退回整个页面 skeleton。

### 目录更新与外部变化

`infos()` 已以已加载 SessionInfo 覆盖目录中同路径记录，并保留本地草稿 key。因此单会话改名、活动时间、首条消息和新文件路径通常可以直接更新已知信息，无须重新扫描全部文件。

- 普通完成/改名不请求目录扫描；只修正该 session 及对应路径的目录元数据。多个本地别名指向同一文件时仍一致更新。
- 首次获得文件路径、fork/clone 后利用 Pi 返回和该实例的定向回读补充已知项；删除延用现有定向移除与扫描竞态保护。只有确实缺少发现信息时才触发发现过程。
- 扫描在途时发生改名/删除/新建，要防旧扫描结果覆写新元数据或复活已删除项。现有删除会取消扫描；更新也需明确合并顺序，不额外维护永久的目录覆盖数据库。
- 用户已确认：外部 Pi/TUI 的会话变化由用户主动刷新目录发现，保留启动时初次发现。普通任务结束不再顺带全目录扫描，也不在激活窗口时自动扫描，不新增文件监听/轮询；Gupi 自己管理的会话即时定向更新。
- 主题/语言切换、用户显式刷新全部目录是真实全局变化。写入草稿存储文件使用既有延迟合并任务和原子替换，这不等同会话列表全量刷新，本轮不顺带拆存储格式。

### 重试与计时

错误、插件提示和回答完成通知的统一设计已移至[待处理总文档](../../../../../docs/dev/issue-217/follow-ups.md#通知与用户注意力管理延后)。本轮保留既有错误处理和交互，不新增会话错误区域、后台提醒、系统通知或持久错误历史；非致命插件错误仍不能改变任务完成语义。

普通模型重试与摘要重试分开记录生命周期；只保留当前运行展示所需信息，不新增持久错误历史。结束、停止、断连和重连按所属阶段清理，不能用 summarization_retry_finished 清掉另一种重试或直接解锁全部 busy 行为。

运行耗时由独立 `ProcessClock` 更新当前过程标题；普通和摘要重试由独立提示视图按截止时间更新。计时不重新投影正文、重测整段消息、读取 RPC 或扫描目录；切换至非运行会话或视图销毁后停止旧计时任务。

### Pi TUI 错误与重试展示对照

2026-09-21 核对同一 Pi 0.86.0 固定源码：

- [showExtensionError](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/interactive/interactive-mode.ts#L2892) 将插件路径和错误作为错误色文本加入 chatContainer；有 stack 时还显示弱化堆栈。RPC extension_error 只发送 extensionPath/event/error，Gupi 不假设能收到 stack。
- [showError](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/interactive/interactive-mode.ts#L4379) 和 compaction_end 的错误分支也在当前会话输出区域追加文本。这些 UI 提示不通过此路径写入 Pi 会话记录或模型上下文；不是另开错误弹窗。该消费路径没有 Gupi 多 session 后台提醒的对应策略。
- [RetryStatusIndicator](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/interactive/components/status-indicator.ts#L51) 显示 attempt/maxAttempts、剩余秒数及取消键提示。[CountdownTimer](https://github.com/earendil-works/pi/blob/d1230ea2000d876b479a69b8b061f9d670f262f5/packages/coding-agent/src/modes/interactive/components/countdown-timer.ts) 每秒本地更新，结束或销毁时停止。TUI 会请求自身重绘，这不构成 Gupi 整页 notify/remeasure 的理由。

本轮仅采用 TUI 的重试进度展示：Pi 已提供 delayMs，界面无需额外请求即可显示实时倒计时。错误展示对照作为后续通知设计的依据，本轮不接入新增错误提示。

倒计时由提示视图维护本地截止时间，避免简单每秒减一累积漂移；后台会话不需持续驱动不可见视图，重新显示时从截止时间计算。到零不推断 Pi 已开始重试，只有实际消息/attempt_start/结束事件才能切换阶段；视图销毁或重试结束后停止计时。保留 Pi 的执行控制，不把本地计时器变成调度器。

## 已确认决定

2026-09-21 用户确认：

| 编号 | 决定 | 范围 |
| --- | --- | --- |
| D-01 | 外部会话变化由用户自己刷新 | 保留启动时初次发现；Gupi 内部变化按 session 定向更新，不增加激活扫描、监听或轮询 |
| D-02 | 错误、插件提示、回答完成通知暂不做新增或统一改造 | 保留现有处理；后续应用内/系统通知场景及待定项统一在[待处理总文档](../../../../../docs/dev/issue-217/follow-ups.md#通知与用户注意力管理延后)，不阻塞本轮事件同步 |
| D-03 | 参考 TUI 显示实时重试倒计时 | 显示第 n/N 次重试、约 x 秒后重试及简短原因；每秒只更新提示区域，不发 RPC、不扫描目录、不重建正文或重测整段消息。到零等待 Pi 的实际事件，停止沿用现有操作，不新增客户端重试策略 |

以上首轮范围的产品决定保持；追加审查的 Q-01～Q-03 已由用户全部确认按建议执行。通知方案在后续独立讨论，未授权提前实施。

实现复用现有 session 和组件状态，保留版本/实例保护；未接 `get_entries(since)`，不为 cache_warm 新增专用卡片，仅同步历史和已有用量。

## 局部刷新实现与边界

2026-09-21 的审查覆盖 Gupi 会话状态、主/临时窗口、搜索/命令面板、设置资源、配置应用和草稿持久化。修复结果按 R-01～R-13 汇总如下。

### GPUI 失效语义

- `gpui-pre 0.3.5` 的 `App::notify` 合并尚未处理的同一 entity 通知，窗口也归并 dirty 状态；`Context::emit` 则逐个追加业务事件。因此当前先汇总 Gupi 的变更范围，再发布一次批次，不能仅依赖绘制层合并来减少订阅回调。
- `Window::mark_view_dirty` 会标记祖先视图，render 中读取共享 entity 也会建立窗口依赖。按来源通知不代表父视图绝不 render；实际减少的是无关扫描、RPC、数据投影、列表复制和尺寸失效。
- `gpui-component 0.6.4` 提供 `MessageScrollerState::remeasure_items(range)`。消息异步解析完成后按稳定行 ID 查找当前位置，防止插入、删除后测量旧 index；保留正常布局、全局几何变化及必要的会话正文投影。
- 草稿、会话事实和 Pi 生命周期仍由原 owner 管理，组件保留焦点、输入、滚动及查询状态。不新增第二份业务历史、永久覆盖数据库、文件监听或生产监控框架。

### 已确认的追加行为

用户已确认全部按建议执行，本轮无剩余待确定项：

| 编号 | 已落实的规则 |
| --- | --- |
| Q-01 | 切换已知项目不扫描；首次选择未知项目只发现该项目的默认/自定义目录，在共享目录内按 cwd 过滤，不递归发现其他项目。发现结果合并到已有目录，启动和用户主动全量刷新保留。扫描期间排队的新项目在当前扫描结束后处理 |
| Q-02 | 删除后台普通会话保留当前选择、输入与滚动；删除当前会话才创建原有空白页面。同文件别名一并移除，保留关闭进程确认、废纸篓和取消旧扫描的防复活语义 |
| Q-03 | 资源页面第一次显示时加载目录；Pi/插件页面首次需要时检测 Pi，后续复用该命令的检测结果。重开设置不强制检测/扫描；用户显式刷新发现外部资源变化，Gupi 自己的资源操作自动校准结果 |

### 修复范围与当前行为

- **R-01～R-04、R-08：** 改名、发送/停止、导出、压缩、重连、扩展回复/超时按 key 通知；同文件改名同步别名。实例状态只路由所属 session，保留 Starting/Closing、就绪、失败和退出阶段。变更先在 GPUI 当前更新中汇总，再按区域发布；正文不再触发临时列表和命令面板的元数据重算，所选 Home 也跳过模型/草稿控件同步。重复插件 title/status/editor 值及未移除任何请求的超时不发布无效更新。
- **R-05：** 发送/停止/手动压缩的校准缩为 History，不固定刷新所有辅助资源；发送确认时若运行仍活跃，直接由 settled 收尾。没有 settled 的独立命令、停止失败/不确定状态仍保留校准。History 真变化才更新 stats/fork 等资源；必要的在途补读和版本保护保持。
- **R-06～R-07：** 新建和选择分离通知，项目切换按 Q-01；fork/clone 定向回读新实例，不再扫描全部目录；删除按 Q-02。扫描进度与目录结果分开，导航展开/查看更多复用已有数据。
- **R-09：** 消息内容变化只使差异行及活跃过程行的尺寸缓存失效；Markdown 解析、过程折叠和历史定位按行重测。仍保留会话正文投影及真实全局几何变化，不额外实现增量 Pi 历史协议。
- **R-10：** 删除通用 changed 与持久化绑定。选择、改名、操作忙闲不再请求草稿保存；实际文本、带非空草稿的身份/路径、删除持久草稿及 fork 返回文本才保存。保存任务合并和原子替换保持。
- **R-11：** 相同有效语言不重建 Fluent/global locale；相同模式/主题不重复应用或刷新窗口，系统强调色变化仍可强制更新。菜单与托盘文字按语言变化更新，正常主题/语言预览与取消仍通过已有配置流恢复。
- **R-12：** 搜索目录投影移到来源/目录事件回调，候选有效性用 key 集合检查，render 不再遍历 infos 并做嵌套线性匹配。命令面板订阅目标的控件/选择变化，不订阅正文；保留目标/binding 失效保护。
- **R-13：** 设置资源按 Q-03 加载；既有文件保存复用原解析器重读该文件，成功启停/删除定向更新对应项。新增/注册资源以及包操作仍进行资源发现，保留来源优先级与去重规则；操作失败可能部分改盘时也重新核实目录。只有 Catalog 真变化才清理 previews/expanded_packages，移除设置父页对资源 controller 的重复直接观察。

### 验证边界

新增回归检查后台改名的通知来源、保存版本/扫描计数，验证同次草稿通知合并及流式正文独立范围、新项目发现不会读取其他项目内容，相同语言不重复通知，以及编辑单个 prompt 不重读其他资源。既有删除/复制测试已按 Q-01/Q-02 更新断言，继续保留退出确认、草稿、文件和旧结果保护验证。没有新增生产监控框架，也未声称量化全应用性能收益。

## 验证结果与限制

- 回读分为 State / History / Full：状态读取不替换历史或清除 live；历史读取附带 get_state 用于会话身份和生命周期校准；首次连接和用户显式刷新保留完整读取。core 请求合并待补读范围，stats/fork 合并重复请求；模型读回用独立设置事件版本识别竞态，保持命令正常收尾。
- agent_settled 定向校准，agent_end 不重复读取；改名不扫描目录。名称/思考事件同步字段，历史页可见时校准对应 metadata 条目；entry_appended 同步 custom、usage 及其他历史条目。历史内容/leaf 相同时不重建索引。扫描完成时保留扫描期间已变更的本地元数据。
- 普通/摘要重试各自保存 Pi 公布的等待期限；独立视图本地倒计时，不产生 RPC/目录扫描/正文重新测量。切换会话或销毁窗口停止旧视图计时；停止、断连、重连和相应阶段结束清理提示。
- 回归覆盖状态类事件不重建正文、不读 entries；结束不扫描目录；旧模型读回与新事件交错；排队事件不丢弃用量结果；追加 custom/usage 同步；连续用量刷新合并；新增 A 会话保留 B 分组实体，后台正文变化保留前台消息和历史投影。
- `cargo test -p gupi -p pi-rpc`：Gupi 218 项通过；Pi RPC 11 项单元、13 项协议/进程回归及 1 项文档测试通过，2 项既有外部条件测试保持 ignored。`cargo clippy -p gupi -p pi-rpc --all-targets -- -D warnings`、格式检查及 debug 构建通过。
- 首轮 macOS 隔离原生测试使用当时的 debug 构建、临时配置、两个各 7 条会话的项目及本地 fauxProvider。已观察：展开/查看更多保持、向 A 新增会话时 B 不收起、侧栏滚动可用、切换回来草稿保留、运行过程标题正常计时，本地模拟任务完成后显示最终回答和冻结的耗时（3m 5s）；后台运行时前台所选会话保持。没有调用真实模型或改动用户配置。
- 原生检查曾发现 GPUI cached view 重用时 AX 控件丢失；已撤掉该缓存，并复测展开/新增/切换。保留项目 entity 及数据投影的局部更新，不把绘制缓存作为本轮前提。
- 本轮不宣称已有真实网络失败/所有重试来源的原生截图验证，或全应用性能基准；事件/并发语义由受影响回归验证。系统通知及新增错误展示按已确认决定继续延后。
- 追加修正使用隔离 `GUPI_CONFIG_DIR` / `GUPI_DATA_DIR` / `GUPI_LOG_DIR` 做原生启动检查。沙盒内 LaunchServices 受限，改为沙盒外启动后进程保持运行、配置加载成功、AX 已激活，日志未见应用错误；本轮原生检查限于启动和日志，未完成逐项原生点击/滚动复验。功能变化由以上受影响回归和 UI 集成测试验证，外观未改。
