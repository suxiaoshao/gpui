# Gupi 未完成项与能力边界索引

GUI 附加限制的移除与预览同步见[职责边界收敛](gui-boundary.md)，该记录不增加输入框上游依赖之外的待办。

归属：[#217](https://github.com/suxiaoshao/gpui/issues/217)。更新日期：2026-09-21。

本页集中查看各阶段留下的依赖阻塞、后续工作和未验证边界。详细设计、源码依据及验证仍归原文档；已有独立文件只链接，不在这里重写方案。表中的“待确定范围”不代表已经授权实现，“未验证”也不等于已发现缺陷。

## Gupi 合入后的 Jaco 清理

用户已确认 Jaco 停止维护，Gupi 合入后清理。专用源码、旧图标库/Lucide 子模块、MCP 测试工具、CI、打包、依赖与文档的删除和调整范围统一见 [Jaco 退役与关联清理](../jaco-retirement/README.md)。当前不执行删除，不删除本机用户数据；共享能力的保留边界也由该文档记录。

## 等待上游或依赖更新

全项目版本盘点和官方 skill 替换方案见 [2026-09 依赖更新计划](../dependency-refresh-2026-09/README.md)。依赖基础升级与下方输入组件功能接入分别判断；升级版本本身不代表这些等待项已经完成。

| 项目 | 当前边界与恢复条件 | 归属与详细记录 |
| --- | --- | --- |
| 设置搜索无结果反馈 | 已升级 v0.6.4，原过滤索引错位不再复现；macOS 实测跨页定位、无结果、清空恢复均对应正确分类。但无结果时正文仍为空白，缺少提示，后续检查上游空状态能力 | [#231 设置计划](../../../app/gupi/docs/dev/issue-231/README.md)、[本轮验证](../dependency-refresh-2026-09/README.md#最终验证与限制) |
| 输入框组件与资源交互（部分等待） | 用户 2026-09-20 要求升级时复用上游能力：已发布的 InputGroup 外壳、on_paste 接线和 Markdown 流式呈现已完成接入，见[依赖更新计划](../dependency-refresh-2026-09/README.md#changelog-对照接入上游能力并删除重复实现)。Skill 选择后填入正文、Skill/模板标签、模板附带文件引用及可选 `@` 入口仍等待下表三项能力进入兼容正式版本；不提前使用 Git 依赖或自建编辑器 | [输入框接入计划](../../../app/gupi/docs/dev/issue-221/composer-resources.md) |
| Questionnaire 与扩展 UI 完整体验 | Questionnaire 尚未正式发布，继续等待兼容正式版本。恢复后映射现有标准 select/confirm/input/editor，验证取消/超时、连续请求和来源会话；控件发布不代表 Pi 新增了多选/多题组合协议 | [#222 扩展 UI 与体验环境](../../../app/gupi/docs/dev/issue-222/README.md) |
| Pi RPC 能力缺口 | 插件参数/子命令补全、自定义快捷键、任意 TUI UI/渲染、组合问卷、输入回读、显示控制等受协议限制；同文件树节点续聊和进程内 reload 也缺直接 RPC。逐项依据、现有降级、社区方案及调查时点统一见原文；后续恢复时重新核对上游，不自动采用私有桥接或社区 fork | [RPC 能力缺口与社区调研](../../../app/gupi/docs/dev/pi-rpc-gaps.md)；[树导航边界](../../../app/gupi/docs/dev/issue-220/history.md)、[刷新重连](../../../app/gupi/docs/dev/issue-226/reconnect.md) |

### 三项组件的统一恢复条件

2026-09-19 通过官方 PR 元数据、正式 release 标签和提交祖先关系核对（日期为 UTC）：

| 能力 | 合并日期 | 最新正式版 v0.6.4 是否包含 |
| --- | --- | --- |
| [Questionnaire #2878](https://github.com/longbridge/gpui-kit/pull/2878) | 2026-09-19 | 否 |
| [InputGroup #3042](https://github.com/longbridge/gpui-kit/pull/3042) | 2026-09-17 | 是 |
| [Input / Textarea 原子内联标签 #3113](https://github.com/longbridge/gpui-kit/pull/3113) | 2026-09-18 | 否 |

[正式版 v0.6.4](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.4) 于 2026-09-18 发布，但发布时间晚于某个 PR 不代表该版本包含它。#3113 的合并提交与此 tag 分叉，#2878 也不在该 tag 中；不能据“都已 merged”提前恢复未发布功能。2026-09-20 已完成正式发布的 InputGroup 外壳与粘贴接线升级；其余资源交互仍等待。正式版本条件满足后，还需确认 API/依赖兼容并完成 Gupi 接入与验证，不意味着下面的 Pi 协议缺口会随组件升级自动消失。

## Pi RPC 全量接入盘点

核对日期：2026-09-19。Gupi 基线为 `be4e7dc6`（已包含本轮快捷键与临时窗口实现）；Pi 官方主线固定为 [`36b60d2e8985899743c4cf5bd5f8929832a3f05d`](https://github.com/earendil-works/pi/tree/36b60d2e8985899743c4cf5bd5f8929832a3f05d)，版本仍为 `0.85.1`。本机安装包也是 `0.85.1`；本地 Pi checkout 为 `71dca871b`，其 `rpc-types.ts`、`rpc-mode.ts` 与此次官方快照相同。这里是源码和调用点审计，没有重新运行所有 RPC，也没有升级依赖或启用新功能。

统计覆盖官方 `RpcCommand` 的 **33 个命令**、转发的 Agent/Session 事件与 `extension_error`、**9 类扩展 UI 请求及回复**，并核对可选参数。`pi-rpc` 暴露 `request_raw` 只代表传输能力，不能算 Gupi 已接入；测试 fixture 的调用不计为产品入口。未调用同名 RPC 也不必然缺功能，已有多实例管理和本地历史投影需要单独标明。

### 33 个命令：16 个直接调用，10 个有替代或主要能力，7 个未接入

下表按命令名计数，每个名字只归入一类。替代并不表示逐项行为完全相同，差异写在相应行；未接入不自动等于必做。

| RPC 命令 | Gupi 当前接入 | 未覆盖内容／处理判断 |
| --- | --- | --- |
| `prompt` | 直接调用；文字、图片及 `streamingBehavior` | Skill/模板的可视编辑与选择后填入正文等待组件版本；不是 RPC 不支持 |
| `abort` | 直接调用；停止当前会话 | Pi 的 `abort()` 同时取消重试、压缩和分支摘要；不能把缺专用停止按钮算作完全不能停止 |
| `get_state` | 直接调用；握手、模型与会话状态、排队数量 | `steeringMode`、`followUpMode`、`messageCount` 留在 extra 中，未提供配置/统计展示；自动压缩状态已有上下文 tooltip |
| `get_commands` | 直接调用；插件命令、模板、Skill 候选 | 参数补全和插件键位不在返回值内，见 RPC 缺口；不返回 TUI 内置命令 |
| `get_available_models`、`set_model` | 直接调用；模型选择器及模板任务覆盖 | 模型范围管理不等同于模型选择器 |
| `get_available_thinking_levels`、`set_thinking_level` | 直接调用；模型对应思考等级及模板任务覆盖 | TUI 保存默认思考等级是另一项配置行为，不由当前会话选择自动完成 |
| `compact` | 直接调用；手动压缩及停止 | 未暴露可选 `customInstructions`；自动压缩失败详情另见事件表 |
| `get_entries` | 直接调用；完整条目、历史树、消息投影 | 未用可选 `since` 增量读取；这是优化候选，不是缺少历史功能 |
| `get_fork_messages`、`fork` | 直接调用；指定用户消息分叉 | 不等同于在原文件任意树节点继续，见 RPC 缺口 |
| `clone` | 直接调用；复制当前会话 | 已有，不重复列为待实现 |
| `export_html` | 直接调用；系统保存窗口指定 `outputPath` | 未用 Pi 自动选默认输出路径；无需为此另加入口。JSONL 导出属于文件能力 |
| `set_session_name` | 直接调用；离线会话也先连接 Pi，再执行改名 | 已有，不重复列为待实现 |
| `get_session_stats` | 直接调用；token、费用、context 用量 | typed 结果只保留这三类；身份、用户/assistant/工具/总消息计数等未用于统一会话信息页，范围待定 |
| `steer`、`follow_up` | 未直接调用；通过 `prompt.streamingBehavior` 接入运行中 steer/follow-up | 已有主要提交能力，不能因缺两个 typed 方法认定缺功能；独立命令的始终入队语义不与 prompt 的空闲直接执行混同 |
| `new_session`、`switch_session` | 未直接调用；Gupi 新会话/恢复使用独立或复用实例，以及 `--session` | 已有多会话新建/恢复；不为用满 API 而强切一个 runtime。`parentSession` 也未作为普通新建参数提供 |
| `cycle_model`、`cycle_thinking_level` | 未直接调用；已有显式模型/思考选择器 | 无循环切换动作；是否需要快捷操作待选，不影响现有选择能力 |
| `get_tree` | 未直接调用；由 `get_entries` 在本地构建历史树 | 已有历史预览，不重复新增读取；树导航语义缺口仍存在 |
| `get_messages` | 未直接调用；消息界面由 entries 和实时事件构建 | 已有消息展示；若要查看 Pi 精确模型上下文再评估此接口，历史视图不等同于模型当前上下文 |
| `get_last_assistant_text` | 未直接调用；从实际执行分支提取最后回答 | 已有复制/回填，避免增加重复请求 |
| `abort_retry` | 未直接调用；通用 `abort` 已会取消重试 | 仅缺“只取消重试”的专用动作，不能列为无法停止重试 |
| `clear_queue` | 已接入清空全部、全部文字取回草稿 | 返回值仅含两类文字数组，排队图片无法恢复；逐条操作继续延后 |
| `set_steering_mode`、`set_follow_up_mode` | 未接入 typed 方法和设置 UI | `all` / `one-at-a-time` 策略待选。Pi setter 会写其设置，接入前须明确保存归属，不能随意当作临时会话字段 |
| `set_auto_compaction` | 未接入开关；只展示已有状态 | Pi 自动压缩仍正常工作；缺的是修改设置的入口，setter 会写 Pi 设置 |
| `set_auto_retry` | 未接入开关 | Pi 自动重试仍工作；缺修改设置入口，setter 会写 Pi 设置 |
| `bash`、`abort_bash` | 未接入用户命令执行及独立中止 | TUI 的 `!` / `!!`、`excludeFromContext`、输出流和取消需要一起定范围。已有模型工具的 bash 卡片不是此能力；不能通过普通 prompt 冒充执行 |

### 实时事件：未使用的不能漏算

`pi-rpc::Event::Agent` 会保留原始 JSON，但 Gupi 只对部分 kind 更新状态。下表覆盖 24 种事件名（含 `extension_error`），不把原样传输当作界面已消费。

| 事件 | 当前行为 | 剩余内容／判断 |
| --- | --- | --- |
| `agent_start`、`agent_end`、`agent_settled` | start 更新运行状态；settled 定向校准并收尾；end 不单独触发回读 | 已有；不把 prompt 接受当作任务结束 |
| `turn_start`、`turn_end` | 没有单独 handler；消息和工具使用更细粒度事件 | 暂无独立轮次 UI 需求，不仅为了消费事件增加界面 |
| `message_start`、`message_update`、`message_end` | 接入消息增量、思考、文字、工具调用及结束校准 | 已有；特定消息种类的展示不能因此一概视为完整 |
| `tool_execution_start`、`tool_execution_update`、`tool_execution_end` | 接入工具执行状态、部分结果及结果卡片 | 已有，不继承插件 TUI renderer |
| `compaction_start`、`compaction_end` | 更新压缩状态并刷新历史 | `reason`、`errorMessage`、`aborted`、`willRetry` 没有完整反馈；手动 RPC 失败已有报错；本轮同步状态，新增自动压缩失败提示延后至通知设计 |
| `auto_retry_start`、`auto_retry_end` | 已更新 retrying、尝试次数、等待倒计时及原因 | 重试等待不回读正文；已有最终错误处理保留，统一错误提醒延后 |
| `queue_update` | 已消费，实时同步两类文字和 pending 数量 | 队列可折叠查看；状态快照仅能补充数量，逐条操作和完整附件仍受协议限制 |
| `entry_appended` | 已接入来源会话的历史/用量同步 | Pi 0.86.0 的 custom、缓存预热 usage 及其他条目定向校准；不作为所有消息的通用追加通知，不新增专用缓存预热卡片 |
| `session_info_changed`、`thinking_level_changed` | 已接入名称、思考等级与定向状态校准 | 保留清空名称语义及模型设置请求与事件的竞态保护，不因此扫描全部目录 |
| `summarization_retry_scheduled`、`summarization_retry_attempt_start`、`summarization_retry_finished` | 已接入独立的摘要重试等待阶段 | 本地倒计时；实际尝试开始/结束清理本阶段，不误清普通模型重试或将其当作整个任务结束 |
| `bash_execution_update` | 未消费 | 与用户 bash 功能共同评估，非模型 `tool_execution_update` |
| `extension_error` | 未消费 | 专门错误展示未接入；用户已确认延后至通知设计，不再作为 #236 本轮必做项 |

#### 事件同步、错误提示与局部刷新

上述接入盘点继续作为能力索引；事件契约、局部刷新实现和验证边界统一见 [#236 实现说明](../../../app/gupi/docs/dev/issue-236/README.md)。名称/思考等级/entry 同步、压缩状态、两类重试进度及过宽刷新修正已完成受影响验证，不再列为待处理项。外部会话由用户手动刷新；首次新项目定向发现、后台删除保留选择、设置资源首次按需加载均已落实。

错误、插件提示和回答完成通知的新增及统一改造继续延后，见下方“通知与用户注意力管理”。`queue_update` 已接入，逐条操作仍受协议限制；turn 事件不另建 UI，直接 Bash 仍另定范围。状态通知 `cx.notify` 与应用内/系统用户提醒是不同层次，不据通知次数推断所有 Pi 实例重新加载。

### 9 类扩展 UI 与参数细节

| 请求／回复 | 当前行为 | 剩余内容 |
| --- | --- | --- |
| `select`、`confirm`、`input`、`editor` | 都已接入，回复带请求 id，使用 value/confirmed/cancelled；editor 支持 prefill，前三者处理 timeout | 统一问卷组件、焦点/连续请求/取消/真实插件验收仍按 #222；不能记成四类功能都没实现 |
| `notify` | 已通知，error 映射错误样式 | warning 与普通 info 目前未区分；分级、来源和通知路由统一延后，保留既有通知 |
| `setStatus`、`setWidget` | 已展示/更新/清理文本状态及上下方文字 widget | 只支持 RPC 提供的字符串数组；组件工厂不是应用漏接 |
| `setTitle`、`set_editor_text` | 已更新扩展标题和会话输入文字 | token 内容怎样与插件纯文本替换共存，留在新输入组件接入范围内 |
| `extension_ui_response` | 已支持三种回复形态 | 它是 UI 应答，不额外计入 33 个命令 |

### 队列边界修正

- **已经提供**：运行中提交 steer/follow-up、`queue_update` 两类排队文本、`clear_queue` 清空并返回文本、两个队列模式 setter。用户已确认逐条返回草稿、编辑和删除的设计目标，详见[队列交互草稿](../../../app/gupi/docs/dev/issue-222/queue-composer.md)；现有接口尚不足以直接实现完整目标。
- **没有专用 RPC**：无副作用主动读取队列的 `get_queue`、按消息 ID 修改/删除某一项、包含图片等附件的完整队列快照。事件和清空结果只有字符串数组，不能声称能够无损恢复排队图片。
- TUI 的“编辑排队消息”实际是将全部队列取出、拼接进编辑器，不是任意逐条在线编辑 API。不要把候选功能写成上游已经提供。
- 继续调研发现本地 Pi 新 Harness 已有 entryId、完整消息与 cancelQueued，但 coding-agent RPC 尚未接入；公开扩展 API 也不能操作已排队单条。具体可行性、消费竞态和推荐契约见[队列交互草稿](../../../app/gupi/docs/dev/issue-222/queue-composer.md#逐条操作的可行性与实现边界)，不据此维护第二套客户端队列。
- TUI 在压缩期间另有界面层 `compactionQueuedMessages`；Gupi 当前压缩时限制发送。若要同等体验，属于新增客户端队列策略，尚未授权实现，也不恢复先前删除的失败输入恢复队列。

## Pi TUI 有、原生 RPC 没有直接提供的能力

以下是客户端接入边界，不等同于全部必须补齐。已有替代实现和明确不做的范围继续保留。更详细的源码与既有社区调查链接见 [RPC 缺口文档](../../../app/gupi/docs/dev/pi-rpc-gaps.md)，社区 PR 状态仍是原调查日期的快照。

| TUI 能力 | 原生 RPC 缺什么 | Gupi 状态／下一步归类 |
| --- | --- | --- |
| 插件子命令/参数补全、自定义补全提供器 | `get_commands` 仅名称/说明/来源，缺参数候选查询；`addAutocompleteProvider` 为空 | 已有命令名候选；参数补全等待 Pi 协议能力，Input token 不能解决 |
| 插件注册快捷键和终端原始输入 | 不传 `registerShortcut` 配置/执行接口，`onTerminalInput` 无效 | Gupi 自有快捷键已实现；不能把 Pi 插件按键自动视为可用 |
| `ctx.ui.custom`、自定义 header/footer/editor、组件 widget | 无跨进程组件契约；custom 返回 undefined，工厂不序列化 | 标准 9 类 UI 已接入，任意插件 TUI 面板需插件降级或另定协议，不自建私有桥接 |
| 自定义消息及工具 renderer | TUI Component/渲染函数不通过 RPC 传输 | 已有通用工具详情/图片/错误显示；不能自动得到 Fleet 等专用可视面板 |
| 多选、多题复核、附加说明等组合问卷 | select 只有字符串数组及单值，缺统一组合 schema | Questionnaire 解决宿主控件，不会自动生成 Pi 问卷协议；现有插件 fallback 仍按顺序 select/input 处理 |
| 插件读取编辑器、插入式粘贴、请求撤销通知 | `getEditorText()` 返回空，`pasteToEditor` 退化成替换；AbortSignal 取消对话框不发专门的撤销事件 | Gupi 可处理已传 timeout 和来源会话关闭；插件主动取消的即时撤销并非现成 RPC。token 接入不能补这些协议信息 |
| 原文件任意树节点继续、节点标签与导航总结 | 有 entries/tree/fork，无公开 `navigate_tree` / 标签编辑 RPC | 已有树预览/消息 fork；原分支切换不能假称由 fork 覆盖。扩展命令上下文可调用核心，不等于公开 RPC 已支持 |
| `/reload` 进程内资源重载 | 无直接 reload RPC | Gupi 通过重启当前连接实现已确认替代行为；不保持原插件内存 |
| 模型范围管理、完整 Pi settings、默认思考级别保存 | 只有部分设置 RPC，无通用配置 CRUD 或范围/顺序管理接口 | 选择模型/思考已实现；范围、默认值等管理另定，不把 Gupi 通用设置等同完整 Pi settings |
| `/login`、`/logout`、`/trust`、`/share` | 无对应认证、信任、gist 发布工作流 | 沿用外部 Pi 管理和原范围决定，不为覆盖命令表自动新增凭据/发布功能 |
| `/import`、JSONL 导出 | switch_session 仅切换现有文件，不负责导入管理；export_html 只导出 HTML | HTML 导出已做；JSONL 文件入口与复制/归档归客户端文件管理，待选 |
| 插件控制 working 动画/隐藏思考标签、主题、工具展开 | 对应 UI setter 多为空或返回不支持；主题 getter 也不是 Gupi 当前主题 | Gupi 自有主题、思考和工具折叠已有；缺的是插件驱动与 TUI 完全一致的效果 |

### TUI 已有，但属于客户端交互而非 RPC 阻塞

| 功能 | Gupi 当前情况 | 判断 |
| --- | --- | --- |
| 输入历史、外部编辑器、原生文件路径补全 | 未提供同等完整入口；已有文字编辑与文件/图片附件 | 不要误列为必须新增 RPC。`@` 入口及标签等仍随输入组件接入统一讨论，外部编辑器/历史不自动加入本轮 |
| 工具统一展开、思考显示切换、历史树筛选 | 已有逐项折叠、三级历史详情与树预览；TUI 的特定全局动作/仅用户/仅标签筛选不完全对应 | 现有能力可用，是否增加这些操作待选，不因快捷键名字不同判缺失 |
| 会话选择器命名筛选、排序切换、路径显示 | 已有目录分组、搜索、时间排序、路径操作 | 不需新 RPC；额外筛选/排序选项属于体验候选 |
| 正文搜索、会话信息页、`hotkeys` 别名、Pi changelog 入口 | 正文搜索与统一信息页未做，快捷键设置已有；别名与 changelog 入口未接 | 见下方原有候选和完整 23 项内置命令对照，不重复创建任务 |
| 终端挂起、终端主题/按键协议/全屏与滚屏控制 | 原生桌面窗口已有自身的窗口、主题和剪贴板机制 | TUI 专属机制，不列为桌面缺陷 |

### 本次核对的来源

- Pi 官方固定快照：[RPC 类型](https://github.com/earendil-works/pi/blob/36b60d2e8985899743c4cf5bd5f8929832a3f05d/packages/coding-agent/src/modes/rpc/rpc-types.ts)、[RPC 分发与 UI 降级](https://github.com/earendil-works/pi/blob/36b60d2e8985899743c4cf5bd5f8929832a3f05d/packages/coding-agent/src/modes/rpc/rpc-mode.ts)、[会话事件/队列/设置语义](https://github.com/earendil-works/pi/blob/36b60d2e8985899743c4cf5bd5f8929832a3f05d/packages/coding-agent/src/core/agent-session.ts)、[Agent 事件](https://github.com/earendil-works/pi/blob/36b60d2e8985899743c4cf5bd5f8929832a3f05d/packages/agent/src/types.ts)、[TUI 行为](https://github.com/earendil-works/pi/blob/36b60d2e8985899743c4cf5bd5f8929832a3f05d/packages/coding-agent/src/modes/interactive/interactive-mode.ts)、[TUI 动作表](https://github.com/earendil-works/pi/blob/36b60d2e8985899743c4cf5bd5f8929832a3f05d/packages/coding-agent/src/core/keybindings.ts)。公开 slash 表仍为 23 项，既有逐项对照见[内置命令文档](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)。
- Gupi：[typed RPC 命令与响应](../../../crates/pi-rpc/src/protocol.rs)、[RPC Client](../../../crates/pi-rpc/src/client.rs)、[会话状态/事件/发送](../../../app/gupi/src/state/conversation.rs)、[数据读取](../../../app/gupi/src/state/conversation/reads.rs)、[输入与扩展交互](../../../app/gupi/src/features/home/composer.rs)、[全局模板任务](../../../app/gupi/src/app/shortcuts.rs)。本轮只把非测试调用计为接入。

## 应用侧后续工作与待定范围

| 项目 | 当前已有内容与剩余范围 | 归属与详细记录 |
| --- | --- | --- |
| 错误、插件提示与回答完成通知 | 用户确认暂缓新增与统一改造；已有提示保留。应用内/系统通知及其他注意力场景的候选分类见下节，尚未授权实现 | [通知与用户注意力管理](#通知与用户注意力管理延后) |
| 队列交互 | 已接入实时文字队列、折叠展示、清空全部、全部文字取回及运行中发送入口。未完成：逐条撤回/编辑/删除、完整附件和原命令恢复、原位置重新入队；Pi 0.86.0 的标准 RPC 仍不提供所需契约。两个队列模式 setter 尚未提供设置入口。不自建调度或失败输入恢复队列 | [#222 队列交互草稿](../../../app/gupi/docs/dev/issue-222/queue-composer.md)、[RPC 取消契约](../issue-219/README.md) |
| Cmd/Ctrl+F 当前分支正文查找 | 尚未实现、尚未单独建 Issue。范围、高亮、折叠展开、结果定位与流式更新规则待独立讨论；不与现有会话目录搜索或跨会话全文索引混为一项 | [正文查找范围草案](../../../app/gupi/docs/dev/issue-226/README.md#独立于-226-的正文查找) |
| 额外 Pi 内置命令能力 | 统一会话信息页、模型范围管理、JSONL 导入/导出等仍需选择范围。快捷键设置已经可查看/修改/恢复，不能再列成缺失功能；`hotkeys` 的命令搜索别名尚未接入。分享、认证、信任管理等仅保留原有边界，不自动列为必做任务 | [完整内置命令对照](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)、[D-14 范围决定](../../../app/gupi/docs/dev/issue-226/decisions.md#待确定的产品范围) |
| 项目级设置 | #231 只做个人级设置。若后续接入项目级设置，应从 session 中独立对话框进入，具体范围尚未设计；不在全局设置中添加项目选择器 | [设置范围与生效边界](../../../app/gupi/docs/dev/issue-231/README.md#已确认方向) |

### 通知与用户注意力管理（延后）

2026-09-21 用户决定：错误、插件提示、回答完毕提示暂不新增或统一改造；同时盘点其他通知场景并区分应用内通知与系统通知。以下为后续设计建议，未授权实现。保留已经存在的错误反馈、插件 `notify` 和交互请求，不把“暂不做”解释成删除或屏蔽它们。重试倒计时属于原位置的运行状态展示，已确认在 #236 中实施。

**源码现状：** [ConversationEvent 与事件消费](../../../app/gupi/src/state/conversation.rs) 的 `Notify` 只有 message/error，没有来源 session key 或完整严重等级；[HomeView](../../../app/gupi/src/features/home.rs) 将它转成窗口内 Notification。插件 notify 的 warning 目前与 info 同样处理。select/confirm/input/editor 保存在各 session 的 pending_ui，当前会话在[输入区域](../../../app/gupi/src/features/home/composer.rs)显示交互；现有超时会移除待答请求。[设置资源操作](../../../app/gupi/src/features/settings/resources.rs)、[导出](../../../app/gupi/src/state/conversation/export.rs)、[临时窗口回填失败](../../../app/gupi/src/app/temporary.rs)已有局部或应用内反馈。本次对 Gupi 源码及 manifest 的检索未发现桌面系统通知发送接入；GPUI 的 `cx.notify` 和 macOS 外观变更 Notification 均不属于系统用户通知。

| 场景 | 用户正在看来源界面 | Gupi 在前台，但用户在其他会话/页面 | 应用在后台或没有可见窗口 | 建议范围 |
| --- | --- | --- | --- | --- |
| 需要用户确认、选择、输入或编辑（包括插件通过标准 UI 发出的确认） | 直接显示现有交互，不重复弹通知 | 应用内提醒来源会话正在等待操作 | 系统通知候选；点击回到仍有效的请求 | 除已延后三类外，优先考虑的提醒；不能仅凭 confirm 推断为权限审批 |
| 任务最终失败、Pi 意外断连、保存失败等错误 | 来源界面显示错误与已有恢复操作 | 对影响任务或用户数据的失败考虑一次应用内提醒 | 需要用户处理的失败考虑系统通知；可自动恢复的中间失败不逐次推送 | 已延后；区分业务失败、连接和存储故障，不将所有 error 都升级为系统通知 |
| 插件主动 notify | 应用内显示并区分 info/warning/error | 应用内显示来源 | 普通插件提示不默认转系统通知；将来需明确用户控制与升级规则 | 已延后；插件级别本身不能决定是否打断用户 |
| 回答完成/临时模板任务完成 | 更新完成状态即可 | 轻量应用内提醒候选 | 用户开启后才考虑系统通知 | 已延后；以 agent_settled 等最终状态判断，不能每次 agent_end/工具结束都通知；失败、取消不报完成 |
| 用户发起的导出、安装/更新/删除资源、清理等操作结果 | 局部反馈或已有应用内提示 | 必要时用带对象名称的应用内通知 | 暂不增加通用系统成功通知；失败按错误规则评估 | 已有反馈保留；未来确有长耗时任务再讨论，不凭空增加任务中心 |
| 复制、保存设置、模型切换、改名、队列变更 | 控件或列表反馈；有必要才简短提示 | 同步对应界面 | 不发系统通知 | 避免每项成功操作都弹提示；失败仍保留既有反馈 |
| 重试等待、压缩过程、工具进度、用量更新 | 更新所属状态区域 | 更新已有必要状态 | 不发系统通知 | 倒计时不每秒通知；阶段进展不等于需要用户注意 |
| 系统权限缺失、附件无法读取、配置校验失败 | 在触发操作的位置说明问题及可用操作 | 无来源操作时不凭空提示 | 不用桌面通知代替权限对话框或表单校验 | 保留既有交互；统一错误提示也随错误项延后 |

后续设计要点：

- 根据**来源界面是否正被用户查看**选择交互位置；根据**应用是否在前台**决定是否适合系统通知，不能把“后台 session”直接等同于“应用在后台”。系统通知与应用内弹出提醒避免重复，来源界面的状态仍保留。
- 通知携带稳定来源会话/操作或请求标识。临时窗口可销毁，未来系统提醒不能依赖窗口 entity 存活；点击时定位已有会话，不自动新建或发送内容。
- 同一等待请求只提醒一次；答复、超时、取消、断连后不再显示过期操作。点击陈旧通知时核对请求，不延长 Pi 的超时，也不自动确认。
- 锁屏通知建议默认只显示应用、通用事件类型，不包含正文、插件报错详情、完整路径或敏感会话名；详情在应用内查看。平台权限被拒绝时保持应用内状态可见，不反复索权。
- 不为提醒增加历史数据库、独立错误中心或另一套 session 状态。当前手动压缩可能同时收到 compaction_end 和 RPC 失败，后续按实际操作去重；不以任意时间窗口过滤所有相似报错。非致命插件错误不能直接改变任务结果，也不把提示写入模型上下文。

**后续待确定：** 各类别开关及默认值、系统通知授权时机、锁屏内容、点击后的主/临时窗口定位，以及连续等待请求的合并体验。先确定产品范围，再选择 macOS/Windows 系统接入；本轮没有实现或验证系统通知，也不为这些候选另建 Issue。

## 发行与验证边界

| 项目 | 已有证据与尚未覆盖部分 | 归属与详细记录 |
| --- | --- | --- |
| 临时窗口与全局快捷任务原生验收 | #221 当前已确认的应用侧实现已完成，输入框后续改造见上方依赖项。macOS 窗口启动、会话隔离、操作面板、停止/隐藏切换及快捷键设置已验证；真实系统热键、外部应用取词与自动粘贴成功路径、权限允许/拒绝、托盘、原生附件入口及跨屏体验仍未完整验证，Windows 尚无实机结果。保留这些验收边界，不再把已实现的面板、双栏布局与 600 秒回收列为缺失功能 | [#221 验证记录](../../../app/gupi/docs/dev/issue-221/README.md#验证记录) |
| 完整发行与性能验收 | macOS 打包、签名及重点原生功能已验证；Windows/Linux 实机体验、Windows Pi 启动脚本行为，以及长对话、冷启动、多实例占用的完整发行验收仍由发行阶段承接。既有 CI/单元测试不能替代这些结果 | [#223](https://github.com/suxiaoshao/gpui/issues/223)、[第一阶段平台边界](../issue-218/README.md)、[运行入口](../../../app/gupi/docs/dev/issue-218/README.md) |
| 开发期 Logo | 现有记录为临时使用 Pi 官方 logo，授权尚未确认。收到回复或准备发布时确定最终方案；不将开发期选择当作正式授权 | [D-04 Logo 决定](../issue-218/README.md#d-04开发期-logo)、[#223](https://github.com/suxiaoshao/gpui/issues/223) |
| 旧阶段原生验证缺口 | 原文仍记录 IME、真实插件交互、真实模型压缩等未覆盖场景，以及早期无 Dock 图标/不可点击的启动反馈。当前打包启动成功不证明早期反馈根因已定位；按具体复现或所属阶段处理，不把每条历史“未验证”都升级成新的阻塞任务 | [命令入口验证记录](../../../app/gupi/docs/dev/issue-226/validation.md#未验证与后续问题)、[扩展 UI 验收](../../../app/gupi/docs/dev/issue-222/README.md) |

2026-09-16 的设置补测：关闭 macOS 台前调度后，本轮不再出现 ScreenCaptureKit `-3811/-3812`；模板启停、编辑放弃、快捷键清除/取消/确认和全部恢复默认已通过控件状态与磁盘结果核对。截图仍有刷新滞后。这是测试工具的捕获边界，不列为 Gupi 待修复功能，也不推断所有 macOS 27 捕获问题都已解决。

## 已有结论，不重新列为未完成项

- 目录读取已完成顺序字节读取与 sonic-rs 按字段解析，用户选择保留原有搜索。分批加载、头尾并发、Rayon/Tokio fs 迁移、全文索引和数据库不是遗留实施任务。见[读取优化方案](../../../app/gupi/docs/dev/issue-229/README.md)；[前期调研](../../../app/gupi/docs/dev/session-catalog-scan-draft.md)仅作依据。
- 手动压缩、HTML 导出、复制会话、消息 fork、会话删除，以及个人级插件/Skill/提示词管理已有实现。不能因早期 RPC 调研或旧命令表写过“缺管理接口”而重复列为缺失。见[命令能力对照](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)、[设置计划](../../../app/gupi/docs/dev/issue-231/README.md)。
- 运行计时、流式 Markdown、工具详情与卡片组织已按后续反馈实施；早期 loading/消息展示反馈已有专门工作承接，不继续保留为“尚待描述”的空白问题。见[运行展示](../../../app/gupi/docs/dev/issue-229/runtime-display-plan.md)、[工具详情](../../../app/gupi/docs/dev/issue-229/tool-details-research.md)、[卡片组织](../../../app/gupi/docs/dev/issue-229/tool-card-layout-research.md)。新问题需要具体复现。

## 维护方式

后续发现已有功能受阻、暂停或尚未覆盖时，在本页增加简短入口，并把详细说明写回所属文档。解决后同步原文并从待办表移除；不通过 Issue 是否关闭、PR 是否合并推断功能完成，不在此维护提交和 PR 流水账。
