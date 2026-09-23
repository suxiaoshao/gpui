# Gupi 未完成项与能力边界索引

GUI 附加限制的移除与预览同步见[职责边界收敛](gui-boundary.md)，该记录不增加输入框上游依赖之外的待办。

归属：[#217](https://github.com/suxiaoshao/gpui/issues/217)。更新日期：2026-09-23。

本页集中查看各阶段留下的依赖阻塞、后续工作和未验证边界。详细设计、源码依据及验证仍归原文档；已有独立文件只链接，不在这里重写方案。表中的“待确定范围”不代表已经授权实现，“未验证”也不等于已发现缺陷。

## Gupi 合入后的 Jaco 清理

用户已确认 Jaco 停止维护，Gupi 合入后按 [#240](https://github.com/suxiaoshao/gpui/issues/240) 清理。专用源码、旧图标库/Lucide 子模块、MCP 测试工具、CI、打包、依赖与文档的删除和调整范围统一见 [Jaco 退役与关联清理](../jaco-retirement/README.md)。当前不执行删除，不删除本机用户数据；共享能力的保留边界也由该文档记录。

## 等待上游或依赖更新

全项目版本盘点和官方 skill 替换方案见 [2026-09 依赖更新计划](../dependency-refresh-2026-09/README.md)。依赖基础升级与下方输入组件功能接入分别判断；升级版本本身不代表这些等待项已经完成。

| 项目 | 当前边界与恢复条件 | 归属与详细记录 |
| --- | --- | --- |
| 设置搜索无结果反馈 | 已升级 v0.6.4，原过滤索引错位不再复现；macOS 实测跨页定位、无结果、清空恢复均对应正确分类。但无结果时正文仍为空白，缺少提示，后续检查上游空状态能力 | [#231 设置计划](../../../app/gupi/docs/dev/issue-231/README.md)、[本轮验证](../dependency-refresh-2026-09/README.md#最终验证与限制) |
| 输入框组件与资源交互（部分等待） | 用户 2026-09-20 要求升级时复用上游能力：已发布的 InputGroup 外壳、on_paste 接线和 Markdown 流式呈现已完成接入，见[依赖更新计划](../dependency-refresh-2026-09/README.md#changelog-对照接入上游能力并删除重复实现)。Skill 选择后填入正文、Skill/模板标签、模板附带文件引用及可选 `@` 入口仍等待下表三项能力进入兼容正式版本；不提前使用 Git 依赖或自建编辑器 | [输入框接入计划](../../../app/gupi/docs/dev/issue-243/README.md) |
| Questionnaire 与扩展 UI 完整体验 | Questionnaire 尚未正式发布，继续等待兼容正式版本。恢复后映射现有标准 select/confirm/input/editor，验证取消/超时、连续请求和来源会话；控件发布不代表 Pi 新增了多选/多题组合协议 | [#222 扩展 UI 与体验环境](../../../app/gupi/docs/dev/issue-222/README.md) |
| Pi RPC 能力缺口 | 插件参数/子命令补全、自定义快捷键、任意 TUI UI/渲染、组合问卷、输入回读、显示控制等受协议限制；同文件树节点续聊和进程内 reload 也缺直接 RPC。逐项依据、现有降级、社区方案及调查时点统一见原文；后续恢复时重新核对上游，不自动采用私有桥接或社区 fork | [RPC 能力缺口与社区调研](../../../app/gupi/docs/dev/pi-rpc-gaps.md)；[树导航边界](../../../app/gupi/docs/dev/issue-220/history.md)、[刷新重连](../../../app/gupi/docs/dev/issue-226/reconnect.md) |

### 三项组件的统一恢复条件

2026-09-23 通过官方 PR 元数据、最新正式 release v0.6.6 和提交祖先关系重新核对（日期为 UTC）：

| 能力 | 合并日期 | 最新正式版 v0.6.6 是否包含 |
| --- | --- | --- |
| [Questionnaire #2878](https://github.com/longbridge/gpui-kit/pull/2878) | 2026-09-19 | 否 |
| [InputGroup #3042](https://github.com/longbridge/gpui-kit/pull/3042) | 2026-09-17 | 是 |
| [Input / Textarea 原子内联标签 #3113](https://github.com/longbridge/gpui-kit/pull/3113) | 2026-09-18 | 否 |

[正式版 v0.6.6](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.6) 于 2026-09-21 发布。GitHub compare 核对：#3113 的 `7f6d9232`、#2878 的 `f698b4bc` 与该 tag 均为 diverged；#3042 的 `142e4016` 是其祖先。0.6.6 主要是 Label 遮罩下高亮和 gpui-pre 精确版本修复，没有解除标签/问卷等待条件。Gupi 当前仍锁定 0.6.4，InputGroup 外壳与粘贴接线已接入；本次没有升级 Cargo 依赖。正式版本条件满足后，还需确认 API/依赖兼容并完成 Gupi 接入与验证，不意味着下面的 Pi 协议缺口会随组件升级自动消失。

同日直接复核源码：本地 `gpui-component` checkout 为 `f698b4bc`，确有 `InputToken` / `InlineToken` / `InputContent` 与 Questionnaire，但组件 manifest 仍标为 0.6.4，不能据工作目录内容推定正式包能力。另下载 crates.io 的 `gpui-component 0.6.6`、`gpui-base 0.6.6` 解包检查，两包 `.cargo_vcs_info.json` 均指向发布提交 `9765ae2c9a5eccfa13891248a445991e6f6a09d8`：组件有 InputGroup，没有 token 模块和 Questionnaire 导出；base 也没有 InlineToken / InputContent 及 Questionnaire 实现。因此“尚未发布”的结论有正式包源码支持，不仅依据提交祖先关系。

## Pi RPC 全量接入盘点

核对日期：2026-09-23。Gupi 基线 `4e160e4f`；最新正式版 [v0.87.1](https://github.com/earendil-works/pi/tree/f07218c4d4bbc12bef056a7058c3dd49dfe41abe) 固定为 `f07218c4d4bbc12bef056a7058c3dd49dfe41abe`，本地 Pi main 已快进至 `898ab804050730e9dcefb4443875d5a932aa6a32`。本次官方版本查询未找到用户提及的 v0.97.0，不能将其当作已核对版本；main 中发布后的 durable JSONL storage 等变化与标准 stdio RPC 分开判断。本机 `pi --version` 已为 0.87.0，本轮没有安装或升级 Pi。

逐文件比较 v0.86.0 → v0.87.0 → v0.87.1 → 当前 main：`rpc-types.ts` 与 `rpc-mode.ts` 均无变化，仍为 33 个命令及 9 类扩展 UI。已使用安装版 0.87.0 运行两项隔离集成测试，覆盖握手、模型/状态读取、标准扩展问答、多进程隔离、恢复/改名/导出/复制/分叉；没有真实模型请求，也不等于全部事件和原生 UI 已验收。

统计覆盖官方 `RpcCommand` 的 **33 个命令**、转发的 Agent/Session 事件与 `extension_error`、**9 类扩展 UI 请求及回复**，并核对可选参数。`pi-rpc` 暴露 `request_raw` 只代表传输能力，不能算 Gupi 已接入；测试 fixture 的调用不计为产品入口。未调用同名 RPC 也不必然缺功能，已有多实例管理和本地历史投影需要单独标明。

0.87.1 本次仅做源码复核：包含新模型支持、压缩提示及部分 provider 纯图片消息修复；没有改变标准 RPC。上一次两项运行测试仍对应本机 0.87.0，不能视为 0.87.1 或 main 的运行验收。

### 33 个命令：17 个直接调用，10 个有替代或主要能力，6 个未接入

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
| `get_last_assistant_text` | 未直接调用；从实际执行分支的原始历史提取最后回答 | 0.87 的此 RPC 从 Pi 的上下文投影取值，受 context_edit 省略/替换影响，不再假定与界面最后可见回答始终一致。现有复制/回填继续以用户看到的回答为准 |
| `abort_retry` | 未直接调用；通用 `abort` 已会取消重试 | 仅缺“只取消重试”的专用动作，不能列为无法停止重试 |
| `clear_queue` | 直接调用；清空全部、全部文字取回草稿 | 返回值仅含两类文字数组，排队图片无法恢复；逐条操作继续延后 |
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
| `entry_appended` | 已接入来源会话的历史/用量同步 | Pi 0.87.0 的边界钩子还可追加 custom_message、context_edit、compaction；已有定向校准保留。display:true 插件消息的正文展示缺口见下节，不将所有结构条目都当成聊天消息 |
| `session_info_changed`、`thinking_level_changed` | 已接入名称、思考等级与定向状态校准 | 保留清空名称语义及模型设置请求与事件的竞态保护，不因此扫描全部目录 |
| `summarization_retry_scheduled`、`summarization_retry_attempt_start`、`summarization_retry_finished` | 已接入独立的摘要重试等待阶段 | 本地倒计时；实际尝试开始/结束清理本阶段，不误清普通模型重试或将其当作整个任务结束 |
| `bash_execution_update` | 未消费 | 与用户 bash 功能共同评估，非模型 `tool_execution_update` |
| `extension_error` | 未消费 | 专门错误展示未接入；用户已确认延后至[#241 通知设计](../../../app/gupi/docs/dev/issue-241/README.md)，不再作为 #236 本轮必做项 |

#### 事件同步、错误提示与局部刷新

上述接入盘点继续作为能力索引；事件契约、局部刷新实现和验证边界统一见 [#236 实现说明](../../../app/gupi/docs/dev/issue-236/README.md)。名称/思考等级/entry 同步、压缩状态、两类重试进度及过宽刷新修正已完成受影响验证，不再列为待处理项。外部会话由用户手动刷新；首次新项目定向发现、后台删除保留选择、设置资源首次按需加载均已落实。

错误、插件提示和回答完成通知的新增及统一改造继续延后，见 [#241 通知设计](../../../app/gupi/docs/dev/issue-241/README.md)。`queue_update` 已接入，逐条操作仍受协议限制；turn 事件不另建 UI，直接 Bash 仍另定范围。状态通知 `cx.notify` 与应用内/系统用户提醒是不同层次，不据通知次数推断所有 Pi 实例重新加载。

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

## Pi 0.87 语义变化与当前接入

| 变化 | 当前代码核对与结论 |
| --- | --- |
| `context_edit` / canonical session context | Pi 用 append-only 记录省略或替换模型上下文贡献，原始历史不改。Gupi 的开放 SessionEntry 类型保留字段，历史“全部”中作为通用事件，正文不应用这些编辑，符合原始对话阅读语义。不要把它误当成删除聊天消息，也不另造上下文调度器；专门展示“对模型已隐藏/替换”属于可选产品范围。 |
| 可执行插件边界 | `turn_end` 新字段、`agent_before_settle` 和 `context_with_system` 属于扩展层；后两者不是新增的 stdio RPC 客户端事件。RPC 继续转发 AgentSessionEvent，不能因为 release 的 ExtensionEvent union 增加成员就给客户端虚构 handler。Gupi 继续以 agent_settled 收尾，符合插件可能延续运行的语义。 |
| `entry_appended` 的更多来源 | 边界提交可能包含 custom、custom_message、context_edit、compaction，失败恢复也会追加 context_edit。Gupi 已为任意 entry_appended 定向回读历史和用量；需区分“数据已同步”与“正文是否显示”。当前明确剩余为 display:true custom_message 的正文展示；普通结构元数据不应一律变成气泡。 |
| retain-none 压缩 | Pi 将压缩记录自身 ID 作为 firstKeptEntryId。Gupi 展示原始历史和压缩摘要，不在 GUI 复刻 Pi 上下文裁剪；不据此删除压缩前消息。此新形态尚未单独进行运行中兼容验收。 |
| 按模型图片处理 | AgentSession 已根据当前模型 inputLimits.images.resize 和 images.autoResize 归一化 RPC prompt 图片，之后才构建持久用户消息；CLI/read/工具图片也复用 Pi 的策略。Gupi 继续发送原始字节、保留原图预览，不复制这些限制。恢复历史中的图像可能已由 Pi 处理，不能承诺一定等于导入原图。 |
| TUI `/bug` | 内置命令从 23 增至 24，没有对应 RPC。诊断上传/本地 ZIP 导出属于独立产品与数据范围，不自动给 Gupi 增加上报功能。 |

当前推荐先推进 [#223 原生体验与发行验收](https://github.com/suxiaoshao/gpui/issues/223)，在已有功能上验证 Pi 0.87 与桌面行为；会话阅读功能已由 #242 承接。输入资源标签/Questionnaire 继续等待正式版，逐条队列继续等待 Pi 正式 RPC，通知后续由 #241 承接，不作为当前验收新增前置。

## Pi TUI 有、原生 RPC 没有直接提供的能力

以下是客户端接入边界，不等同于全部必须补齐。已有替代实现和明确不做的范围继续保留。更详细的源码与社区调查链接见 [RPC 缺口文档](../../../app/gupi/docs/dev/pi-rpc-gaps.md)；其中核心社区历史补丁仅保留少量直接相关入口，不将旧分支活动记录当作当前可用能力。

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
| 正文搜索、会话信息页、`hotkeys` 别名、Pi changelog 入口 | 正文搜索与统一信息页未做，快捷键设置已有；别名与 changelog 入口未接 | 见已立项的 #242 和完整 24 项内置命令对照，不重复创建任务 |
| 终端挂起、终端主题/按键协议/全屏与滚屏控制 | 原生桌面窗口已有自身的窗口、主题和剪贴板机制 | TUI 专属机制，不列为桌面缺陷 |

### 本次核对的来源

- Pi 官方固定快照：[RPC 类型](https://github.com/earendil-works/pi/blob/16787ad5b2dc748047f314ca1bfe7708f30f54f3/packages/coding-agent/src/modes/rpc/rpc-types.ts)、[RPC 分发与 UI 降级](https://github.com/earendil-works/pi/blob/16787ad5b2dc748047f314ca1bfe7708f30f54f3/packages/coding-agent/src/modes/rpc/rpc-mode.ts)、[会话事件/队列/设置语义](https://github.com/earendil-works/pi/blob/16787ad5b2dc748047f314ca1bfe7708f30f54f3/packages/coding-agent/src/core/agent-session.ts)、[Agent 事件](https://github.com/earendil-works/pi/blob/16787ad5b2dc748047f314ca1bfe7708f30f54f3/packages/agent/src/types.ts)、[TUI 行为](https://github.com/earendil-works/pi/blob/16787ad5b2dc748047f314ca1bfe7708f30f54f3/packages/coding-agent/src/modes/interactive/interactive-mode.ts)、[TUI 动作表](https://github.com/earendil-works/pi/blob/16787ad5b2dc748047f314ca1bfe7708f30f54f3/packages/coding-agent/src/core/keybindings.ts)。公开 slash 表现为 24 项（含 /bug），既有逐项对照见[内置命令文档](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)。
- Gupi：[typed RPC 命令与响应](../../../crates/pi-rpc/src/protocol.rs)、[RPC Client](../../../crates/pi-rpc/src/client.rs)、[会话状态/事件/发送](../../../app/gupi/src/state/conversation.rs)、[数据读取](../../../app/gupi/src/state/conversation/reads.rs)、[输入与扩展交互](../../../app/gupi/src/features/home/composer.rs)、[全局模板任务](../../../app/gupi/src/app/shortcuts.rs)。本轮只把非测试调用计为接入。

## 已确认的后续工作归属

下面的功能已立项，不再作为“是否需要做”的待确定项重复讨论。Issue 确定范围和排期，设计文档保留真实未决细节与验证边界；立项不等于实现完成。

| Issue | 已确认范围 | 归属与实施条件 |
| --- | --- | --- |
| [#223](https://github.com/suxiaoshao/gpui/issues/223) | 原生体验、打包验收、正式图标和 Welcome / 首次启动引导 | 当前主 Issue；复用现有设置并预填，必填/可跳过项在实施时确定 |
| [#241](https://github.com/suxiaoshao/gpui/issues/241) | 错误、插件提示、完成及待用户操作的应用内/系统提醒 | 主 Issue 下的后续工作，不作为 #223 新增前置；[通知设计](../../../app/gupi/docs/dev/issue-241/README.md) |
| [#242](https://github.com/suxiaoshao/gpui/issues/242) | display:true 插件持久消息正文、会话信息弹窗、当前分支正文查找 | 主 Issue；复用现有消息/历史与统计，不扩为跨会话索引 |
| [#243](https://github.com/suxiaoshao/gpui/issues/243) | 原子资源标签、Skill/模板输入、Markdown 资源展示、Questionnaire | 主 Issue；[资源接入计划](../../../app/gupi/docs/dev/issue-243/README.md)，输入原子标签/问卷等待正式组件；Markdown 内联插件已在正式包提供 |
| [#240](https://github.com/suxiaoshao/gpui/issues/240) | Jaco 与关联源码、资源、CI、打包、依赖和文档清理 | 独立后续任务，Gupi 合入后执行；[统一清单](../jaco-retirement/README.md)，不删除用户数据 |
| [#244](https://github.com/suxiaoshao/gpui/issues/244) | Pi 配置图形化与项目级覆盖 | 独立后续任务，不属于 #217 子 Issue；不另存 Gupi 同义配置 |
| [#245](https://github.com/suxiaoshao/gpui/issues/245) | 包市场、搜索/详情与资源发现 | 独立后续任务，不属于 #217 子 Issue；各类资源共用市场和已有安装器 |

逐条队列仍归 [#222](../../../app/gupi/docs/dev/issue-222/queue-composer.md)，受 Pi 契约限制。未选择的额外能力（JSONL 导入/导出、hotkeys 搜索别名、分享/认证等）保留在[命令能力对照](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)，不是上述 Issue 的隐含实施任务。

## 发行与验证边界

| 项目 | 已有证据与尚未覆盖部分 | 归属与详细记录 |
| --- | --- | --- |
| 临时窗口与全局快捷任务原生验收 | #221 当前已确认的应用侧实现已完成，输入框后续改造见上方依赖项。macOS 窗口启动、会话隔离、操作面板、停止/隐藏切换及快捷键设置已验证；真实系统热键、外部应用取词与自动粘贴成功路径、权限允许/拒绝、托盘、原生附件入口及跨屏体验仍未完整验证，Windows 尚无实机结果。保留这些验收边界，不再把已实现的面板、双栏布局与 600 秒回收列为缺失功能 | [#221 验证记录](../../../app/gupi/docs/dev/issue-221/README.md#验证记录) |
| 完整发行与性能验收 | macOS 打包、签名及重点原生功能已验证；Windows/Linux 实机体验、Windows Pi 启动脚本行为，以及长对话、冷启动、多实例占用的完整发行验收仍由发行阶段承接。既有 CI/单元测试不能替代这些结果 | [#223](https://github.com/suxiaoshao/gpui/issues/223)、[第一阶段平台边界](../issue-218/README.md)、[运行入口](../../../app/gupi/docs/dev/issue-218/README.md) |
| 开发期 Logo | 现有记录为临时使用 Pi 官方 logo，授权尚未确认。收到回复或准备发布时确定最终方案；不将开发期选择当作正式授权 | [D-04 Logo 决定](../issue-218/README.md#d-04开发期-logo)、[#223](https://github.com/suxiaoshao/gpui/issues/223) |
| 旧阶段原生验证缺口 | 原文仍记录 IME、真实插件交互、真实模型压缩等未覆盖场景，以及早期无 Dock 图标/不可点击的启动反馈。当前打包启动成功不证明早期反馈根因已定位；按具体复现或所属阶段处理，不把每条历史“未验证”都升级成新的阻塞任务 | [命令入口验证记录](../../../app/gupi/docs/dev/issue-226/validation.md#未验证与后续问题)、[扩展 UI 验收](../../../app/gupi/docs/dev/issue-222/README.md) |

2026-09-16 的设置补测：关闭 macOS 台前调度后，本轮不再出现 ScreenCaptureKit `-3811/-3812`；模板启停、编辑放弃、快捷键清除/取消/确认和全部恢复默认已通过控件状态与磁盘结果核对。截图仍有刷新滞后。这是测试工具的捕获边界，不列为 Gupi 待修复功能，也不推断所有 macOS 27 捕获问题都已解决。

## 已有结论，不重新列为未完成项

- 目录读取已完成顺序字节读取与 sonic-rs 按字段解析，用户选择保留原有搜索。分批加载、头尾并发、Rayon/Tokio fs 迁移、全文索引和数据库不是遗留实施任务。见[读取优化方案](../../../app/gupi/docs/dev/issue-229/README.md)。
- 手动压缩、HTML 导出、复制会话、消息 fork、会话删除，以及个人级插件/Skill/提示词管理已有实现。不能因早期 RPC 调研或旧命令表写过“缺管理接口”而重复列为缺失。见[命令能力对照](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)、[设置计划](../../../app/gupi/docs/dev/issue-231/README.md)。
- 摘要与工具详情 Dialog、原始摘要/分区复制、定向工具更新及侧边栏拖动修复已完成，见[详情方案](../../../app/gupi/docs/dev/issue-238/README.md)。不再列为下一项实施任务。
- 运行计时、流式 Markdown、工具详情与卡片组织已按后续反馈实施；早期 loading/消息展示反馈已有专门工作承接，不继续保留为“尚待描述”的空白问题。见[运行展示](../../../app/gupi/docs/dev/issue-229/runtime-display-plan.md)、[摘要与工具详情](../../../app/gupi/docs/dev/issue-238/README.md)。新问题需要具体复现。

## 维护方式

后续发现已有功能受阻、暂停或尚未覆盖时，在本页增加简短入口，并把详细说明写回所属文档。解决后同步原文并从待办表移除；不通过 Issue 是否关闭、PR 是否合并推断功能完成，不在此维护提交和 PR 流水账。
