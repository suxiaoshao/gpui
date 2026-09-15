# 运行中状态与过程展示对照

记录日期：2026-09-14。本文记录用户在 Runtime Gallery 中观察到的差异，对照本机 Codex **Electron**、Pi TUI 与 Gupi 源码；属于调研报告，不表示下面的行为调整已经实现。

本报告保留实施前快照的源码与样本证据。2026-09-15 的实际实现、验证结果、统一图标及 Windows shell/RPC 能力见[实施计划](runtime-display-plan.md)，下文描述的旧 Gupi 行为不代表当前实现。

## 用户观察与要求

| 项目 | 用户观察或明确要求 |
| --- | --- |
| 首字前反馈 | 第一个字输出之前没有“已处理 xxx（时间）”一类的状态与计时反馈。 |
| 最外层折叠 | 还没有最终回复时，第一级详情应保持展开，不能手动关闭，对齐 Codex Electron。 |
| 过程保留 | 第二轮开始后，前面的“我先查看目录和检查技能，再定位待处理内容。”、思考过程等消失，只留下最新内容。应查明原因。 |
| 计时文字动效 | “已处理 xxx”不需要 shimmer。这里指最外层状态/计时文字，不扩大为删除全部工具动效。 |
| 多调用汇总 | 调研同一步中有很多工具调用时，Codex 的外层标题、图标和内容组织方式。 |
| Skill 读取 | 调研 Codex 是否区分 Skill 读取与普通文件读取。以上各点也同时对照 Pi TUI。 |

为避免层级混淆，本文将一轮回复的整体过程区称为**第一级**，连续思考/工具形成的组称为**第二级**，单个工具详情称为**第三级**。这只是描述 Gupi 当前结构；Pi TUI 没有完全对应的三层结构。

## 证据范围

- Gupi：当前 `codex/229-gupi-runtime-loading` 工作区，基于提交 `8d984f6a`；另有未提交的 Runtime Gallery fixture 与启动脚本。
- Pi：本机源码 `/Users/sushao/Documents/code/pi`，提交 `71dca871b`，coding-agent 版本 `0.85.1`。实际 Gallery 会话也使用安装版 Pi 0.85.1。
- Codex Electron：已有本机解包产物，`package.json` 版本 `26.908.40834`。分析的是 Electron webview JavaScript，不用 Rust CLI 的呈现代替桌面实现；没有声称它是上游最新版本。
- 验证方式：用户提供的实际 UI 观察、Gupi/Pi 源码、Electron 静态 bundle，以及一次 Gallery 的实际 JSONL 记录。本轮没有自动操作三个应用的窗口，也没有创建或注册 `.app`。

Electron bundle 已压缩且没有本地 source map。文末保留文件名、函数名与部分字符偏移，便于在同一快照复核；这些函数名不是稳定公共 API。

## 1. 首字前状态与计时

### Gupi

当前存在两个独立限制：

1. `project_rows` 根据已有的非 user 消息形成 Run。只有 user 消息、还没收到 assistant 消息时，没有用于渲染运行标题的 Run。即使知道会话正在运行，也只是把已有的最后一个 Run 标记为 active，不会为当前 user 后面补一个空 Run。
2. `process_title(active = true)` 只返回“正在处理”的本地化文本，没有实时耗时。完成后的耗时从**第一条 assistant 的 message.timestamp**算到最后一条消息的 `completed_at`，并非从本轮开始处理计时。

因此，不能仅加一个文字动画就补齐首字前反馈；“何时存在运行区”与“计时从哪里开始”都需要明确。收到空 assistant 的 `message_start` 后可能出现工作占位，但这仍晚于只有 user 消息的阶段。

依据：[消息投影与渲染](../../../src/features/home/messages.rs)、[标题与时间](../../../src/features/home/messages/metadata.rs)。

### Codex Electron

计时是独立的 `worked-for` 展示项：

- 工作中：先显示 `Working`，达到 1 秒后显示 `Working for {time}`，每秒更新。
- 处理结束：显示 `Worked for {time}`；相应停止路径有 `You stopped after {time}`。
- 使用 `startedAtMs`、`completedAtMs`，不依赖正文已显示多少字符。正常本地 turn 的起点优先取 `turnStartedAtMs`，缺失时取首个 work item 的开始时间。

但不能据此写成“点击发送就无条件出现计时”：本快照的本地 turn 转换要求已经出现 work item，`firstTurnWorkItemStartedAtMs` 不能为空；`item/started` 就可以建立这个状态，不必等可见文本 delta。只有 user、尚无 work item 时，该转换路径不会生成 `worked-for`。sleep 等路径还有额外排除条件。

结论：Electron 的计时与可见正文解耦，能在首个可见字之前出现；**用户发送、RPC 接受、开始工作、首字输出是不同时间点**，不能把它们混成一个已确认起点。用户要补上首字前反馈已经记录；Gupi 的具体起点尚未在本轮替用户决定。

依据：Electron `gqt`/`mqt` 的 `item/started` 处理，`XMn`/`EAn` 生成 `worked-for`，`aHi`/`sHi` 渲染和计时。

### Pi TUI

`turn_start` 会在启用工作状态显示时启动 WorkingStatusIndicator，不依赖 assistant 正文输出。它使用 Loader 的旋转字符与工作提示；默认实现没有 Electron 式的累计秒数和最终 `Worked for` 汇总标题。

依据：[interactive-mode.ts](../../../../../../pi/packages/coding-agent/src/modes/interactive/interactive-mode.ts) 的 `turn_start`、`showWorkingStatusIndicator`；[status-indicator.ts](../../../../../../pi/packages/coding-agent/src/modes/interactive/components/status-indicator.ts)；[loader.ts](../../../../../../pi/packages/tui/src/components/loader.ts)。Pi 源码链接依赖本机同级 `pi` checkout。

## 2. 最外层什么时候可以折叠

### Gupi

active Run 默认展开，但 `process_open` 中保存的手动选择可以覆盖默认值。通用 `fold` 始终提供点击与键盘切换，没有针对“第一级且最终回答尚未出现”的限制。

所以当前行为是“默认展开”，并没有实现用户要求的“此阶段不允许关闭”。依据：[presentation.rs](../../../src/features/home/messages/presentation.rs) 的 `fold`，以及 [messages.rs](../../../src/features/home/messages.rs) 的 Run 默认展开条件。

### Codex Electron

外层 `LG` 的核心条件等价于：

```text
最终回答已经开始 && 本轮未取消 && 存在可展示过程项
    → 可以进入允许折叠的分支
其他情况
    → 不允许折叠，保持展开
```

随后还会叠加 `disableCollapse`、实际内容数量等限制。保存的折叠状态不能绕过“最终回答尚未开始”的限制。

这里的边界是 **hasFinalAssistantStarted，即最终回答开始**，不是最终回答全部输出完毕。内层工具 disclosure 是另一套规则：运行中默认展开，用户仍可以手动折叠。不能把第一级规则直接套到全部层级。

依据：`conversation-blocks` 的 `LG`、`zG`；`tool-activity-disclosure` 的 `S`。

### Pi TUI

没有包住一轮所有过程的同类第一级 disclosure。思考可隐藏，工具输出可切换展开程度，但它们不是 Electron 的最外层整体过程区，因此不存在可以直接照搬的同级开关规则。

2026-09-15 纠正：Responses 内部确实会在 final_answer/text_start 时修改 partial.stopReason，但当前 RPC 的 `toJsonEvent` 会移除 partial 和累计 message。这不能作为 RPC 提前确认最终阶段的依据。当前 Gupi 根据 message_end 的 stopReason 与 textSignature 确认和拆分最终文本；增量期间仍显示正文，整轮结束另由 agent_settled 决定。实际线上事件格式、接入修复与录制回归见[实施计划](runtime-display-plan.md)。

## 3. 为什么旧过程在第二轮消失

### 本次 Gallery 已找到具体原因

检查实际测试 session：

```text
/var/folders/rq/c4thf05d5zz26j9g3nf759w00000gn/T/gupi-runtime-gallery-c8snCJ/sessions/
2026-09-14T13-34-25-926Z_01a0a020-4a85-70d6-a1b0-6a6687966507.jsonl
```

其中完整场景的 **13 条 assistant 消息共用 `message.timestamp = 1789393114958`**，包含 12 条 `toolUse` 回答与 1 条最终 `stop` 回答；session entry 的 id 各不相同。前面另外两条报错尝试不计入这 13 条。

例如首条 `82f7e111` 保留了用户提到的“我先查看目录和检查技能……”及思考，末条 `9b62c25e` 是最终回答。它们在 JSONL 里都还存在，不能描述成 Pi 删除了历史消息。

形成覆盖的链路：

1. Gallery 在 `before_agent_start` 内同步构造全部 faux 回答，再 `setResponses`。Pi 的 `fauxAssistantMessage` 默认在**构造时**填入 `Date.now()`；实际样本中的 13 条恰好同毫秒。
2. Gupi 的 `DisplayMessage::signature()` 用 `role + timestamp` 匹配消息。
3. `message_start`、`message_update`、`message_end` 共用查找已有 signature 并替换 value 的处理。因此后续不同的 assistant 消息被当成同一条正在更新的消息。
4. live 与已有消息合并时也使用相同 signature，无法靠重新排列折叠内容解决这个身份冲突。

这是**测试插件的构造方式触发了 Gupi 的消息身份冲突**。之前加速跑两次检查了工具数量、思考事件和最终结果，没有检查时间戳碰撞或实际 Gupi 画面，所以那些检查通过不能证明消息展示正确。

证据足以解释本次样本的旧过程覆盖；尚未测得真实 provider 中此类碰撞的发生频率，也不据此宣称所有过程丢失都只有这一个原因。本轮没有改 fixture 或生产代码。

依据：[runtime-gallery.mjs](../../../tests/fixtures/runtime-gallery.mjs) 第 58、73 行附近；Pi [faux.ts](../../../../../../pi/packages/ai/src/providers/faux.ts) 第 76–97 行；Gupi [history.rs](../../../src/state/history.rs) 第 264 行、[conversation.rs](../../../src/state/conversation.rs) 第 238、1246 行附近。

### Pi TUI 如何保留

每次 assistant `message_start` 都新建 AssistantMessageComponent 并加入 chatContainer；后续 delta 更新当前 streamingComponent；`message_end` 完成当前组件并清空 streamingComponent 引用。下一条 assistant 再创建一个新组件，已完成的前一条仍在容器内。

因此这条渲染路径没有用“相同时间戳意味着同一条 assistant”的假设。组件内部重新绘制当前消息也不能误解成清空整段历史。

依据：`interactive-mode.ts` 的 `message_start`（3220 行附近）、`message_update`、`message_end`（3279 行附近）；`assistant-message.ts` 的消息更新与内容渲染。

### Codex Electron 如何保留

过程投影将 assistant 过程说明作为独立项，连续工具另行成组；稳定键优先取 source item 的 id、callId、requestId 等，必要时才回退到类型与位置。新的工具组不会因为开始运行就代替前面的独立过程说明。

需要保留一个差别：`agent-activity-item` 的普通工具活动投影会跳过 `reasoning`，当前思考另有状态/摘要路径。**不能据此声称 Electron 会逐项展示每一段原始 reasoning**。应分别讨论“已经出现的过程说明被覆盖”和“思考内容以何种方式呈现”，不能为模仿后者而容忍前者的数据覆盖。

依据：`agent-activity-item` 的 `nn`，`agent-activity-units` 的 `Z` 与键生成逻辑；`local-conversation-turn` 的 thinking fallback。

## 4. 最外层计时是否使用 shimmer

| 应用 | 当前实现 |
| --- | --- |
| Codex Electron | `aHi` 把 Working/Worked for 渲染成普通 span，使用弱化文字色与 tabular-nums；计时文字本身没有 shimmer。其他活动标签有自己的运行表现，不能混为一项。 |
| Pi TUI | 默认工作指示器是 Loader 旋转字符配合主题文字，没有该类计时文字 shimmer。 |
| Gupi | 所有层级共用的 fold 都配置 `MarkerLoadingStyle::Shimmer`，active Run 传入 loading，所以第一级也会应用该动效。 |

用户决定已经明确：第一级的状态/计时文字不使用 shimmer。具体工具内部动效不在这个决定的范围内。

## 5. 同一步很多工具调用时的外层显示

### Codex Electron

它会先组织活动，再决定标题，并非把全部工具调用永远合并成“运行 N 个工具”：

1. **连续项成组。** 可分组的 exec、文件修改、搜索、一般 MCP 调用等连续出现时归入一个组；独立 assistant 过程说明会截断分组，部分特殊工具/UI 也保持独立。
2. **正在运行的最近一组突出当前动作。** 最新可见组、turn 仍运行且组未关闭时，选择当前探索动作，或从组尾找尚未完成的活动来提供标签；没有合适活动时可回落到思考状态。例如读文件与读 Skill 有各自标签。并行多个调用时，这只是选出的状态摘要，不代表其他调用被删掉。
3. **已经结束或较早的组使用分类汇总。** 汇总部分可以包括 MCP 来源、加载的技能/工具、未命名 MCP 调用次数、文件修改数、文件探索、命令数、网页搜索等；不是每组都出现所有类别。图标也根据汇总部分和具体活动选择。
4. **详情仍保留各调用。** 外层摘要不替代内层明细。运行中的内层组可以由用户展开或收起，与第一级锁定展开的规则独立。

依据：`agent-activity-units` 的 `Z`（分组）、`qe`（运行态摘要选择）、`Se`（探索统计）、`Oe`（分类汇总）、`Me`（汇总图标）；`conversation-blocks` 的活动组渲染。

### Pi TUI

工具调用按 toolCallId 创建/更新各自 ToolExecutionComponent，并逐项加入会话容器。同一条 assistant 产生多个调用也会有多个组件；默认路径没有 Electron 的同类聚合外层标题。

### Gupi

当前已有“连续活动成组、assistant 过程说明分隔”的结构。第二级标题在任意工具运行时优先显示通用的运行计数；完成后再区分读取、搜索、命令、修改及混合探索，图标依据工具种类选择。

所以这里有可比较的具体差异：**Gupi 运行中主要提示数量，Electron 最近活动组更突出当前动作，较早/完成的组再做分类汇总**。用户已确认对齐该行为；实施仅映射 Gupi 已有活动，不照搬尚无对应数据的完整分类。

依据：Gupi `RunContent::blocks`、[presentation.rs](../../../src/features/home/messages/presentation.rs) 的组标题和图标生成。

## 6. Skill 读取和普通文件读取

| 应用 | 识别与呈现 | 边界 |
| --- | --- | --- |
| Codex Electron | 从读取目标路径解析技能元信息，判断 `isSkillDefinitionFile`。普通文件使用 `Reading {target}`，技能定义使用 `Reading {skillName} skill`；组统计把技能定义读取与普通文件探索分开。 | 识别包含路径约定，不是所有任意名为 SKILL.md 的文件都可据此断言被识别。普通文件与一般 Skill 的活动标签仍可用同一个 run-command 图标；不能声称它必然有专用 Skill 图标。 |
| Pi TUI | read 的紧凑 renderer 检查文件名 `SKILL.md`，显示 `[skill]` 加父目录名。普通文件显示 read 与路径；也另有 Pi 文档、AGENTS/CLAUDE 等资源分类。 | 这是未展开时的紧凑呈现；展开后回到普通读取调用信息。RPC 中仍然是 read 工具，不新增 Skill 消息类型。 |
| Gupi | 当前消息区主要按工具名 read 选择 BookOpen，并显示读取参数/路径。 | 尚未在这条呈现路径加入与上述相当的技能目标识别。 |

Electron 依据：`skill-exploration-labels` 的 `F`/`P`，`app-initial` 的 `bRn` 路径解析，`active-tool-activity-label` 的 read 分支，`agent-activity-units.Se` 的统计。

Pi 依据：[read.ts](../../../../../../pi/packages/coding-agent/src/core/tools/renderers/read.ts) 的 `getCompactReadClassification`、`formatCompactReadCall`、`readRenderers.renderCall`。这属于 TUI renderer，Gupi 接收 RPC 不会自动得到相同格式。

## 当前结论与尚未确定的边界

- 已确认的 Gupi 差异：首条 assistant 之前缺少对应 Run；运行标题没有实时耗时；第一级只是默认展开而非限制收起；第一级继承 shimmer。
- 已确认的本次覆盖原因：Gallery 的 13 条回答同时间戳，触发 Gupi 按角色与时间戳替换消息；源 session 没有丢记录。修复讨论应同时考虑 fixture 的构造方式和 Gupi 的消息匹配，不先做大规模展示重构。
- 用户已明确要求：首字前有状态/时间反馈；最终回复前第一级保持展开；前面的过程不应被新一轮覆盖；第一级状态/时间不使用 shimmer。
- 用户已确认计时包含最终回答输出，表示本轮总执行耗时；有明确最终阶段信号时允许收起，没有提前信号则保持展开，等回答结束确认后再允许收起。计时事件映射属于实施核对。Responses 路径已找到可在文本开始时取得的 stop 状态。真实模型的时间戳碰撞频率仍未验证。
- 用户已确认多工具外层行为对齐 Electron，并要求区分 Skill 与普通读取、统一消息区和命令面板的技能图标；具体图标建议与 Skill 识别边界见实施计划。

## Electron 快照复核索引

解包根目录：`/private/var/folders/rq/c4thf05d5zz26j9g3nf759w00000gn/T/codex-sidebar-source-ld9z8im5/`。下列文件均位于其 `webview/assets/`；临时目录将来可能被清理，上文已保存本轮需要的条件、差异和证据结论。

| 文件 | 复核入口 |
| --- | --- |
| `app-initial-9b95fa538c62.js` | `mqt`/`gqt`：work item 开始；`EAn`（字符偏移约 3082910）、`XMn`（约 3134760）：worked-for 数据；`aHi`（5745398）、`sHi`：文字与计时；`bRn`（3185637）：技能路径。偏移为 JavaScript 字符索引，不是行号。 |
| `conversation-blocks-b695a23bbff2.js` | `LG`（462504）、`zG`：最外层折叠；活动组渲染。 |
| `local-conversation-turn-e5f6ce31b82d.js` | `bi`、workedForItem、hasFinalAssistantStarted、thinkingFallbackMessage。 |
| `agent-activity-item-f72a0494a0e9.js` | `nn`：可分组项、独立 assistant 项与 reasoning 的投影。 |
| `agent-activity-units-7618c38ce0ab.js` | `Z`、`qe`、`Se`、`Oe`（6608）、`Me`：分组、活动选择、汇总与图标。 |
| `tool-activity-disclosure-be7e75617865.js` | `S`：运行中内层组的手动展开/收起。 |
| `skill-exploration-labels-99ed514c4616.js` | `F`、`P`：技能定义识别与标签元信息。 |
| `active-tool-activity-label-6bfe40e62b41.js` | read 分支：Reading target 与 Reading skillName skill。 |
