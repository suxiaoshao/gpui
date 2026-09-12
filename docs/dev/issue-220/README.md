# Gupi 第三阶段开发计划：会话导航与主窗口对话

- 状态：已实现，进入用户试用；当前验证范围见文末。
- Issue：[#220](https://github.com/suxiaoshao/gpui/issues/220)
- 上级：[Gupi 总览](../issue-217/README.md)
- 应用设计：[功能要求与页面设计](../../../app/gupi/docs/dev/issue-220/README.md)

下文确定数据、接口、职责、实施顺序和必要验证；已确定的默认双栏、按需展开历史面板、图标工具栏及可调整宽度由应用设计文档维护。页面实现遵循其中已定决定，局部呈现通过试用调整。

## 交付目标与边界

先建立会话入口，再完成选中会话中的真实对话。用户可以按项目或最近活动寻找会话、恢复聊天、查看分支树，并使用 Pi 原生 fork 从支持的历史位置另开会话。

2026-09-09 已确定：Gupi 的会话内容与会话操作使用 Pi 已有 RPC 命令，不通过自带扩展、私有通知协议或自建 SDK 宿主补充缺失的 RPC 能力。Rust 类型化封装可以覆盖 Pi 已有命令，不改变上游协议。删除会话是文件管理例外：RPC 没有删除命令时，只对已落盘且空闲的会话文件执行可恢复的系统“移到废纸篓”，等待 Gupi 自身实例实际关闭并重新核对文件身份，不修改 JSONL 内容。

据此，分支树提供查看、预览和定位；本阶段不提供同一会话内的“从这里继续”、树导航分支总结、导航附加指令或导航标签写入，也不自动重放历史节点导航。Pi 原生 fork/clone 与同文件树导航的含义保持区分。

保留原 Issue 的模型与 thinking 选择、文本输入、流式回答、工具执行状态、失败与停止，以及标准扩展 UI 交互。生成期间支持 Pi 原生 steer/follow-up。按接口承接用户扩展产生的标准 RPC UI 消息，不按特定提问插件设计；不自带补协议能力的扩展。

本阶段包含会话发现、恢复和必要 fork；第五阶段保留完整队列交互、工具详情及更完整的扩展 UI。临时窗口、全局快捷键和发行工作仍归各自阶段。

设置页重设计与手动会话目录管理延后，不纳入 #220 的设计、实现或完成条件。本轮只连接现有设置入口；默认目录和已知 Pi 配置目录的发现不依赖设置页改造。

## 当前基础与归属

| 归属 | 当前基础 | 本阶段工作 |
| --- | --- | --- |
| `crates/pi-rpc` | 单进程 Client、请求关联、事件流、标准扩展 UI 信封及关闭 | 为实际使用的现有会话、历史、模型命令补充类型化接口 |
| `app/gupi` | 主页占位；PiState 管理运行连接 | 会话目录、项目分组、连接与会话绑定、对话及历史视图、原生交互 |
| Pi | JSONL 会话、entry 树、模型执行、会话恢复与 fork | 保持内容和会话操作的权威来源 |

实现沿用应用现有 `foundation`、`state`、`features` 边界：

- `foundation` 承担目录与元数据读取、应用自身文件的读写；复用现有 paths/persistence 支持，不建立会话索引数据库。会话删除由 `state` 协调实例关闭与目录刷新，文件身份核对及移入系统废纸篓沿用可恢复的文件管理路径。
- `state` 承担目录结果、会话与实例绑定、entries 投影、输入草稿及操作状态。复用 PiState 的 start/client/close 和事件入口，不另建进程管理器。
- `features` 组合原生组件并调用状态层动作；当前 home 占位由主会话页面接替，现有 Pi 检测、配置与启动入口继续复用。
- 非空的未发送草稿使用独立的应用状态文件，避免输入时重写全部设置；采用现有序列化与持久化方式，不另引入数据库或通用框架。本轮不为手动会话目录扩展 AppConfig。具体私有模块名在实现时决定。
- 左右侧边栏宽度扩展现有 `state/layout.rs` 的 LayoutState，保存到 `state.toml`。保留现有 main_window 字段及读写入口，加载旧状态时为缺失宽度提供默认值；窗口 capture/save 与宽度保存共用完整布局状态，避免相互覆盖。

SessionCatalog 描述可发现的会话文件；PiState 描述运行中的连接。目录项不对应常驻进程。RPC crate 不负责文件扫描、项目分类或视图状态。

本地目录扫描只提取列表所需元数据，用于实现“所有会话”入口，不向 Pi 新增 list_sessions 命令。选中会话后启动或复用 Pi，通过原生 RPC 获取正文、entries 和执行状态；不提供自行解析离线正文的第二条展示路径。目录覆盖范围见后文。

## 已有能力与数据边界

调研基线为 Pi 0.85.1，源码提交 `da840b6216578c2a571d0374ac6a2091a83f9d91`；以下结论针对该版本。

| 能力 | 已核实的 Pi 行为 | 本阶段使用方式 |
| --- | --- | --- |
| 会话发现 | RPC 没有 list_sessions；Pi 使用文件目录保存会话 | 应用只读扫描默认目录及已知配置目录 |
| 历史 entries | get_entries 返回所有分支 entries 与 leafId；since 返回某 id 之后的全部 entries，没有 limit | 作为历史视图来源，不声称支持分页 |
| 分支树 | get_tree 返回真实树与 leafId | 查看已有分支；也可从已获取 entries 构建相同父子关系，避免重复加载 |
| 模型上下文 | get_messages 返回当前 session.messages，受分支与压缩影响 | 不能替代完整历史 |
| 会话操作 | new_session、switch_session、fork、clone、get_fork_messages、set_session_name 已存在 | 按 UI 实际需要封装；fork 与 clone 区分来源位置 |
| 会话删除 | RPC 没有删除会话命令；会话内容保存在 JSONL 文件中 | 对已落盘且空闲的目标文件等待自身实例关闭后核对并移入系统废纸篓，不修改 JSONL 内容 |
| 同文件树导航 | 原生 RPC 没有 navigate_tree | 无对应操作入口，不绕过协议补齐 |

所有历史视图共享 entries 投影，Pi JSONL 为内容来源，不建立第二套权威 transcript 数据库。活动连接的 leafId 取自 Pi；浏览节点、滚动和输入草稿不改变执行节点。重启后以 Pi 实际恢复结果为准。

会话恢复与 fork 的关键事实：

- 默认会话可能到出现 assistant 消息才落盘，目录需要合并运行中尚未保存的会话。
- fork 会在同一个进程内切换 sessionId/file，返回所选用户文本；新文件记录 parentSession。此路径已用隔离 Pi 0.85.1 实测，无模型请求。
- InstanceId 不等于 sessionId；fork/switch 后须更新会话绑定，事件和 UI 回复不能按当前选中项路由。
- Pi 恢复旧格式会话时负责迁移，可能改写文件；Gupi 不复制迁移或离线正文解析实现。
- switch_session 使用目标 header.cwd 重建运行服务。原生 RPC 没有 cwdOverride，项目目录缺失时按实际错误呈现，不悄悄换目录执行。
- fork/switch 返回取消或失败时，不能直接把目标会话显示成已恢复；按实际 RPC 状态同步。运行时替换失败不保证旧运行时仍可用。

## 已确定的读取与恢复方式

- 历史内容统一走原生 RPC，选中会话后启动或复用 Pi，与 TUI 需要启动 Pi 才显示对话的使用方式一致。目录扫描只承担发现及元数据，不另建离线正文模式。
- 跨重启保存各会话非空的未发送文本草稿，不保存会话选择或滚动位置；启动进入空白新对话，项目默认收起。执行分支遵循 Pi 实际恢复结果，不重放节点导航。fork 返回文本放入新会话，原会话草稿保留。
- 持久化选择以真实会话文件路径为恢复入口；尚未落盘的新会话以本地草稿身份及 cwd 保留未发送文本，取得 Pi 会话身份后归并。重启时不伪造不存在的 Pi 历史；恢复失败保留草稿并呈现实际错误。

当前传输限额是客户端自身策略，不是 Pi 协议规则；不预建基于文件大小猜测的回退或分页机制，实际超限时报告错误并针对证据处理。

## 会话发现方案与参考依据

2026-09-09 用户确认采用简单的目录发现方案：

- 覆盖 Pi 默认会话根及已知配置的 sessionDir。已知配置包括全局配置、当前项目及已发现项目的 `.pi/settings.json`，不搜索未知项目或全盘配置。
- 日常入口为“新建会话 → 选择工作目录 → 启动 Pi”，项目按 cwd 自动形成分组，不要求单独登记项目；已有项目中新建可沿用 cwd。运行中尚未落盘的会话也立即显示。
- 默认根遍历项目子目录；已知自定义 sessionDir 读取其中的 JSONL。手动添加会话目录随设置页工作延后。
- 按 header.cwd 分组，按最近消息活动倒序；保留真实文件路径并去重。采用后台扫描和内存元数据列表，不引入专门的索引数据库、全文搜索或文件监听服务。
- 启动、手动刷新及应用自身会话操作后更新列表。正文仍由选中会话对应的 Pi 通过原生 RPC 提供。

本机只读扫描已确认此方式可从默认目录发现 10 个项目、226 个会话；当时未发现额外 sessionDir，所有项目 cwd 均存在。这是方案可用性证据，不作为固定数据或实现验收数量。

Pi 选择目录的优先级是 `--session-dir`、`PI_CODING_AGENT_SESSION_DIR`、启动目录对应的 settings.sessionDir，再回退默认目录。未知项目的私有 sessionDir 本轮不主动发现。项目 cwd 已不存在时保留目录项，打开后显示 Pi 实际错误。

检查此前克隆的 `StarkInternationalAI/pi-desktop`，固定提交 `7ffbc1606475a22bfbcec4252ab0577b821305ff`，以下为源码结论，未启动该应用验收。

| 路径 | 实际实现 | 对 Gupi 的参考价值 |
| --- | --- | --- |
| 项目列表 | ProjectManager 读取自身 `~/.pi-desktop/projects/*.json` 项目配置 | 以应用登记项目为入口，没有从全部 Pi session header 自动发现项目 |
| 会话列表 | 前端调用 session_list，从 SQLite 按 project_id 查询，按 last_modified_at 倒序；进程启动时登记 session | 可参考“列表索引与运行进程分开”，不能据此声称已覆盖外部 TUI 会话 |
| 重建索引 | session_reindex 固定使用 `~/.pi/agent/sessions`；reindex_project 只读第一层 `.jsonl`，以文件 stem 为 id、当前时间为创建时间，全部归入传入 project_id | 未遍历 Pi 默认项目子目录，未读取 header.cwd，也未在此路径处理 Pi 目录覆盖配置；前端源码未找到该命令的调用 |
| 打开会话 | session-view 拼接 `~/.pi/agent/sessions/{sessionId}`，再启动 `pi --mode rpc --session …`；没有从列表拿到原文件路径 | 适用于该应用自己约定的路径；发现已有 Pi 文件时应保留扫描所得完整路径 |

该版本还将 message_end 摘要写入 SQLite，entry id 由自己生成且 parent_id 为空；这不是可复用的 Pi 完整历史树索引。Gupi 仍通过原生 RPC 获取真实 entries。

## 同一会话的多个 Pi 进程

Pi 0.85.1 普通 TUI 与原生 RPC 共用 SessionManager 和运行时。TUI 恢复会话调用 runtimeHost.switchSession，后者使用 SessionManager.open；所查普通会话路径没有文件占用锁或外部文件变更监听。认证、设置文件使用的锁不等于会话锁。

- 每次打开把文件加载到本进程 fileEntries / byId / leafId。get_entries 及当前对话读取这些内存状态，不在每次请求或写入前重新读磁盘。
- 对已落盘会话，通常按本进程 leafId 生成 parentId，然后追加 JSONL；因此两个进程可以各写各的分支，运行中的双方不会自动看到对方的新记录。重新打开时才重新加载文件；加载以文件最后一个 entry 建立初始 leaf，启动本身还可能追加设置 entry。
- 这不是同一实时对话的多端协作保证。旧格式迁移等路径会重写文件，也没有由普通会话锁保护，不能从“通常追加”推导出所有并发操作都安全。
- 同一源码另有 experimental/session-worker 的 ownership 文件锁，以及 mini server 复用 worker 的方案；普通 main.ts 的 TUI/RPC 路径不接入它们。本阶段不接入实验性宿主。

2026-09-09 使用安装的 Pi 0.85.1 做了隔离复现：A、B 两个原生 RPC 进程打开同一临时会话，分别调用 set_session_name，再各取 get_entries。A 只含名称 `from A`，B 只含 `from B`；文件包含双方记录，各自 parentId 对应本进程操作前的 leaf。新进程打开后读到两个名称。实验只用了原生命令，没有扩展、凭据、模型请求或用户会话；验证的是共用会话实现，未操作两个 TUI 窗口，也未验证并发迁移或高负载写入。

Gupi 的边界继续保持：应用内同一文件复用一个运行连接；与外部 TUI 共用文件遵循 Pi 自身行为，不承诺实时同步，不追加自建跨进程锁、自动暂停或自动重载。目录刷新只更新元数据，不把它解释为活动 Pi 历史已同步。该事实不再列为未知技术问题。

## 原生 RPC 接入顺序

按实际功能补充 Command、结果类型和 Client 方法；沿用现有请求关联、错误和未知字段兼容方式，不一次性封装全部上游命令。

| 工作 | 使用的原生命令/入口 | 应用处理 |
| --- | --- | --- |
| 创建与恢复 | LaunchOptions 的 cwd / `--session` 参数、get_state、get_entries；需要进程内切换时使用 new_session / switch_session | 文件路径与 sessionId 绑定到运行实例；恢复结果成功后才更新活动会话 |
| 会话重命名 | set_session_name | 未运行时按正常路径恢复；成功后刷新目录标题，保留原活动排序 |
| 对话控制 | prompt（streamingBehavior 为 steer / followUp）、abort、get_available_models、set_model、set_thinking_level | Enter 发送/steer，Option/Alt+Enter 发送/follow-up；接受/排队与执行结束分开，遵循 Pi 队列和扩展命令语义 |
| 用量与上下文 | get_session_stats、get_state、已有 entries | 上下文圆环及 hover 详情；累计 tokens/cost 与当前上下文区分，CH 从最近 assistant usage 计算；未知值不当作零，不推测订阅标记 |
| 扩展 UI | 现有 extension_ui_request / reply | 按接口承接 select/confirm/input/editor、notify、set_editor_text、setTitle、setStatus 和字符串数组 setWidget；请求绑定来源实例与请求 id，status/widget 按来源会话与 key 维护 |
| 树与 fork | get_entries、get_fork_messages、fork | entries 构造分支树；只在上游支持的用户消息位置 fork，完成后重新查询身份和历史 |

get_tree 可按实际需要调用；已有 entries 足够时不重复获取全树。clone 不纳入本阶段基础菜单，不为覆盖协议目录额外增加工作。

启动、恢复和 fork 的状态变化由状态层统一处理。不同会话的事件按来源实例及当前有效会话绑定进入各自状态；异步完成结果检查绑定代次，切换显示不会将旧结果或扩展回复送给别的会话。具体归并算法和私有类型通过对应回归确定。

## 四个实现提交与完成条件

每个提交包含所属实现、关键回归和对应稳定文档更新。依赖顺序为 1 → 2 → 3 → 4；页面部分依据已确定的应用设计实现。计划拆分不要求另开协议、索引或页面基础设施项目。

| 顺序 | 提交主题 | 内容与依赖 | 关键验证 |
| --- | --- | --- | --- |
| 1 | `feat(gupi): discover and group Pi sessions` | 只读 SessionCatalog、项目/最近活动列表、搜索和空态 | 元数据、活动排序、同名项目；只浏览目录不启动 Pi |
| 2 | `feat(gupi): open and restore session conversations` | 新建/恢复、运行实例与会话绑定、RPC 历史 entries 和当前分支投影、非空草稿持久化；所需原生 RPC 类型，依赖 1 | 应用内单一写入连接、恢复及切换后的事件归属、压缩历史、重启空白页与旧草稿保留 |
| 3 | `feat(gupi): complete the main conversation loop` | 输入及分会话草稿持久化、模型/thinking、流式回复、工具摘要、停止、错误与标准扩展 UI，依赖 2 | 真实 Pi 连续对话/停止；草稿恢复；事件收尾和扩展回复关联 |
| 4 | `feat(gupi): browse session history and fork conversations` | 只读分支树、预览定位、原生 fork 与来源关系，依赖 2、3 | 预览不改变执行节点；fork 返回文本、新文件与绑定同步；取消或失败不伪造成功 |

提交 2 后可试用会话入口与历史，提交 3 后可试用主对话闭环；本阶段没有独立的协议扩展工作包。

### 1. 会话发现与项目分组

- 在 Gupi 中实现只读元数据扫描：默认根及已知配置目录，文件去重、cwd 分组、最近活动排序及简单搜索。复用 Pi 的元数据语义，不解析正文视图。
- 搜索使用现有 Command/CommandState 与 Dialog 组合的弹出搜索框，复用查询及键盘导航；确认结果后打开对应会话，覆盖范围不受侧边栏最近 5 条限制。
- 接入会话目录状态与页面数据，合并运行中未落盘项目；列表位置、密度与控件形式由页面设计确定。
- 按已确定的两级 Sidebar 实现左侧导航及可拖动分隔边，宽度写入 state.toml；拖动完成时保存，启动恢复并适配当前窗口。
- 接入项目右键的新建会话、定位目录和复制路径；右键本身不启动 Pi，动作绑定被点击项目。
- 完成条件：能列出配置范围内的真实会话；浏览目录不启动 Pi；目录或文件读取失败时本轮扫描整体失败，不发布部分结果；刷新失败时保留上次完整目录。
- 最小回归：默认/已知自定义目录布局与去重、同名 cwd、消息活动排序、坏文件导致本轮失败及刷新失败保留完整旧目录；扩展现有布局状态覆盖缺失/无效宽度默认及窗口状态与宽度共同保存。原生检查拖动调宽、重启恢复及缩窄适配。首轮用本机目录只读验证实际分组，数量随数据变化。

### 2. 新建、恢复与历史数据

- 从选择的工作目录创建 Pi 会话；从真实文件路径恢复会话，复用应用内同文件连接。沿用已有启动和退出契约。
- 订阅 PiState 事件，建立运行实例、会话身份及历史状态的绑定。获取 entries / leaf 后投影当前分支和压缩前历史，保留一份共享历史数据。
- 保留非空草稿，启动进入空白新对话；用户主动打开历史，加载失败保留目录入口和实际错误。Pi 负责旧格式恢复，Gupi 不迁移 JSONL。
- 接入 session 右键的原生重命名、复制路径、定位文件、空闲实例“结束运行”及“移到废纸篓”。释放进程后保留目录项和草稿；删除前等待自身实例实际关闭，重新核对普通文件、session ID 和 cwd，再通过系统废纸篓 API 移除；路径操作和删除只对已落盘文件可用。
- 完成条件：新项目自然进入目录；现有会话能加载、切换和重启恢复；切换展示不停止后台会话。
- 最小回归：同文件连接复用、切换后迟到结果归属、分叉及压缩历史投影、选中会话恢复、删除等待实例退出及目录扫描旧结果失效。隔离真实 Pi 验证新建、已有文件恢复和重命名后重新读取；原生交互检查覆盖右键目标、结束运行后的恢复以及删除失败时保留选择与草稿。

### 3. 主对话闭环

- 接入文本输入、模型/thinking 选择、流式消息、工具执行摘要、错误及 abort。Enter 空闲发送、生成期间 steer；Option/Alt+Enter 空闲发送、生成期间 follow-up；Shift+Enter 与 Super+Enter 换行，正确处理输入法组词。输入使用 prompt 的 streamingBehavior，保留 Pi 扩展命令和输入处理语义；首版不增加附件编辑和完整队列管理界面。
- 新建与 session 共用输入组件，仅新建显示项目选择行；无权限/语音入口。接入上下文占用圆环与 hover/聚焦统计详情，复用 ProgressCircle；按 RPC 数据刷新，处理未知上下文值，避免历史预览改变统计归属。
- 复用 Message/Bubble/TextView/MessageScroller/Collapsible；运行中展开过程但折叠详细信息，正常完成后保留最终回答并收起过程入口，用户可重新展开。Gupi 负责运行分组与完成判定，保留错误/中止及无最终回答内容，滚动与测量复用组件能力。
- session 右键在生成中提供“停止生成”，复用主对话 abort 动作及验证；空闲时才提供“结束运行”。
- 输入草稿按会话持久化，尚未落盘的草稿保留 cwd；清空草稿遵循实际发送接受结果，失败时可恢复输入。持久化复用应用支持，采用普通合并写入策略即可。
- 按原生 RPC 方法接入扩展 UI：select/confirm/input/editor 临时替换来源会话主输入区，提交/取消后恢复普通输入与草稿；同时承接 notify、set_editor_text、setTitle，以及 setStatus 和字符串数组 setWidget 的更新/清除与上下位置。区分临时交互与普通草稿文本替换，扩展回复不走聊天 prompt；连续请求按实际 RPC 顺序处理。处理取消、超时及来源关联；后台只显示侧边栏状态，点击来源会话显示其待处理交互，不追加通知或自动切换。setEditorComponent/custom/setFooter/setHeader 无原生 RPC 呈现，不增加自建桥接。
- 完成条件：连续发送两轮消息、看到工具过程、停止生成并继续使用；切换或重启后未发送草稿仍在。
- 最小回归：流式消息与最终 entries 不重复、停止/失败后状态可继续、跨会话草稿隔离及重启恢复、扩展请求回复不串会话。真实 Pi 验证连续对话、一次工具执行及停止；扩展 UI 用隔离 fixture 覆盖必要回复。
- 覆盖 steer/follow-up 路由与接受/排队反馈，确认 streamingBehavior 序列化为 `followUp`（已有 camelCase 序列化保留，并增加回归断言），不混同独立命令名 `follow_up`；原生验证换行、输入法确认和生成期间编辑/提交。扩展 fixture 覆盖 select→input 连续请求、临时交互的提交/取消与普通草稿恢复、editor 回执、set_editor_text 替换及 setTitle 来源；沿用现有回归，不要求为特定插件新增宿主。
- 统计与扩展展示覆盖未知上下文、CH 分母为零，以及跨会话 status/widget 更新和清除；原生检查圆环 hover/聚焦详情及输入框上下文本区域。
- 关键回归覆盖正常完成折叠、失败/中止内容保留、用户展开不被刷新覆盖和运行分组；侧边栏验证等待输入/失败/运行优先级及请求失效后的状态清理。原生检查展开过程、折叠详情、流式尾随和向上阅读不抢滚动位置。

### 4. 历史浏览与原生 fork

- 从 entries 提供分支树，按 Pi leaf 标识当前执行路径；浏览任意分支保持原执行节点。
- 右侧历史面板默认关闭，用带 tooltip 的纯图标按钮切换；可拖动调宽并复用 state.toml 的布局持久化，窄窗口覆盖显示。与左侧共用宽度处理方式，不重复引入一套持久化逻辑。
- 从 get_fork_messages 支持的位置调用 fork，结果确认后更新会话绑定、目录及来源关系；返回文本进入新会话输入，原草稿保留。
- 历史面板分列表和树两种展示，默认选择列表，共用简略、详细、全部三级内容；列表另有全部分支和仅此分支范围，树始终显示全部分支。树详细模式保留用户消息与助手最终回答，其他任意长度的过程段可展开和收起，并保护根、分叉、叶子及当前定位节点。其他分支预览保留草稿并暂停普通发送，提示条提供返回当前分支；当前分支内定位不禁用发送，扩展回执仍绑定来源请求。支持 fork 的用户消息操作菜单提供另开会话。
- 按应用设计的[会话历史文档](../../../app/gupi/docs/dev/issue-220/history.md)生成共享可见投影：列表使用 `List` / `ListItem` 逐条呈现，树使用独立画布布局绘制全部分支、连线和过程折叠；不预设固定图形列。树的折叠、定位点保护、相机与展开意图以该文档为准，实现归 Gupi，不抽取通用 Git 图或扩展协议。
- 完成条件：可查看其他分支及历史并返回当前会话；可从用户消息另开会话继续，原文件保留。
- 最小回归：长单链不持续缩进，分叉及折叠后的连线正确，追加/折叠后按 entry ID 保留有效选择；预览不发变更命令、fork 的身份/文本/来源更新，以及取消或失败不伪造成功。隔离真实 Pi 验证 fork；原生检查跨视口连线、点击预览与展开分离，以及返回当前分支。

## 两份独立设计文档

本文维护跨 pi-rpc / Gupi 的范围、原生接口、依赖与必要验证；[应用设计](../../../app/gupi/docs/dev/issue-220/README.md)维护已确定的功能、交互和应用侧实现选择。保留两份独立文档；应用与 crate 的稳定 README 随实现更新。

## 必要验证与交付

应用继续使用 Gupi 名称、现有 bundle 标识及运行入口。此次按迭代交付：完成必要检查后由用户试用，不为验证另起应用名称或追加完整平台验收。

此次按用户要求采用最小充分验证：受影响构建、既有回归与新增投影/协议/布局回归，再检查必要原生路径。隔离真实 Pi 验证恢复、重命名、fork 和标准扩展回复；真实模型的连续对话、工具执行、停止及输入法体验由用户试用继续验证。检查使用临时会话。

受影响代码的基础检查：

```sh
cargo fmt --all -- --check
cargo build -p pi-rpc -p gupi --locked
cargo test -p pi-rpc -p gupi --locked
cargo clippy -p pi-rpc -p gupi --all-targets --all-features --locked -- -D warnings
```

实际执行按改动范围缩小；修复后只复测受影响部分。提交和集成遵循实际 hooks 与 CI，完整 workspace 检查按其要求运行。记录通过项及影响试用的限制，不把研究实验当作新实现已通过验证。

普通实例关闭沿用[第二阶段当前关闭契约](../issue-219/README.md)，不增加 RAII 清理或固定 sleep；删除操作单独等待自身实例报告实际关闭后再执行文件身份核对和移入系统废纸篓。只复测被改变的部分，完成必要验证即交付试用；完整平台发行矩阵归发行阶段。

## 当前验证与边界（2026-09-12）

- `cargo test -p pi-rpc -p gupi --locked`：Gupi 83 项、pi-rpc 21 项测试和 1 项文档测试通过；2 项需要本机 Pi 的集成测试默认忽略，本次未重跑。
- `cargo build -p pi-rpc -p gupi --locked`、上述严格 Clippy 命令、格式和 diff 检查通过。
- 删除功能已完成隔离原生检查：移入系统废纸篓后返回新建会话，保留项目目录、清除目录项与该会话草稿；未调用模型。历史画布、加载状态、模型控件和消息呈现的局部原生验证见应用设计及其专题文档。
- 主窗口快捷键尚未统一接入，由 [#226](https://github.com/suxiaoshao/gpui/issues/226) 承接；全局翻译热键仍归 #221。标准扩展已有实现和早期 fixture 验证，当前界面与真实插件的完整交互验收归 [#222](https://github.com/suxiaoshao/gpui/issues/222)。
- 真实模型的流式多轮、工具/停止及输入法体验仍待试用；本次未重跑完整 workspace 或跨平台发行验收。

## 实现与历史验证快照（2026-09-09）

以下是 2026-09-09 已完成的历史验证快照；之后的历史面板、加载状态和会话删除等改进以应用设计文档为准。本节保留当时真实做过的证据，不替代当前提交的验证记录。

实现位于 `state/conversation.rs`（实例绑定、命令/事件、草稿保存）、`state/history.rs`（共享历史投影）、`foundation/session_catalog.rs`（目录发现）及 `features/home/`（Sidebar、消息、输入、历史）。继续沿用 PiState 的连接与退出所有权。接收 `agent_settled` 后才完成已观察运行的收尾，避免把单次消息或 agent_end 的到达当作可安全切换执行身份。

- 受影响构建通过；Gupi / pi-rpc 39 项测试及 1 项文档示例编译通过。新增覆盖会话目录去重和排序、历史分叉/预览、steer 插入后的活动过程、压缩历史保留、局部无效面板宽度及 followUp 序列化。
- 隔离 Pi 0.85.1 的 2 项集成测试通过：类型化查询、双实例与标准扩展回复；临时文件恢复、原生重命名及 fork 后来源文件保持不变。未使用模型凭据或发送模型请求。
- 原生 Gupi 检查通过：新建/恢复、Markdown 与用户气泡、历史覆盖层关闭、其他分支预览、搜索选择、多行中文草稿切换/重启恢复、select/confirm/input/editor 连续交互和正常退出。保留 Gupi 名称及 bundle 标识。检查中修正了气泡颜色、覆盖层点击穿透和弹层挂载。
- 实际模型的流式多轮、工具/停止、输入法组词及更大历史的交互体验尚待用户试用；未扩展为完整平台或发行包验收。


## 依据

- 当前实现：[主页](../../../app/gupi/src/features/home.rs)、[PiState](../../../app/gupi/src/state/pi.rs)、[协议类型](../../../crates/pi-rpc/src/protocol.rs)、[pi-rpc 使用与限额](../../../crates/pi-rpc/README.md)。
- Pi 官方文档：[Sessions](https://pi.dev/docs/latest/sessions)、[RPC](https://pi.dev/docs/latest/rpc)、[Session format](https://pi.dev/docs/latest/session-format)、[Compaction](https://pi.dev/docs/latest/compaction)。latest 可变化，具体行为以固定源码与实测范围为准。
- 固定源码：[session-manager.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/core/session-manager.ts)（目录、元数据、entries、迁移）、[rpc-mode.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/rpc/rpc-mode.ts)（命令与结果）、[agent-session-runtime.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/core/agent-session-runtime.ts)（恢复与 fork）、[main.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/main.ts)（目录配置优先级）。
- Tauri 固定源码：[会话命令](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/commands/session.rs#L58)、[索引扫描](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/session_index.rs#L267)、[项目列表](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/project_manager.rs#L25)、[打开会话](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src/views/session-view.ts#L171)、[进程与索引](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/process_manager.rs#L268)。
- Pi 普通路径：[TUI 恢复](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/interactive/interactive-mode.ts#L5348)、[会话加载与写入](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/core/session-manager.ts#L898)、[模式入口](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/main.ts#L930)。独立实验性路径：[worker 锁](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/experimental/session-worker.ts#L533)。
