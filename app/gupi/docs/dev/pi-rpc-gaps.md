# Pi RPC 能力缺口与接入边界

核对日期：2026-09-23。Gupi 基线 `4e160e4f`；Pi 正式版 **v0.87.1**：`f07218c4d4bbc12bef056a7058c3dd49dfe41abe`；本地最新 main：`898ab804050730e9dcefb4443875d5a932aa6a32`。官方 latest 发布和标签查询没有 v0.97.0，本次不宣称核验了该版本。本机安装 Pi 仍为 0.87.0，未升级。

本文维护协议缺口、现有替代及源码依据。完整 **33 个 RPC 命令、事件、9 类扩展 UI 与 Gupi 接入状态**集中维护在[统一待处理文档](../../../../docs/dev/issue-217/follow-ups.md#pi-rpc-全量接入盘点)，不在这里重复一份接入表。已立项的后续功能见该文档的[工作归属](../../../../docs/dev/issue-217/follow-ups.md#已确认的后续工作归属)。

## 正式版和 main 复核

| 检查 | 结论 |
| --- | --- |
| v0.86.0 → v0.87.0 → v0.87.1 → 本次 main | `rpc-types.ts`、`rpc-mode.ts` 无差异，仍为 33 个命令、9 类扩展 UI。下方补全、插件键位、自定义 UI、树导航、reload 和完整队列缺口继续成立 |
| SDK 与 Harness | `sdk.ts` 仍构造 `new Agent(...)`，未切到实验 Harness。Harness 的队列 ID、完整消息与 cancelQueued 不能作为标准 RPC 可用接口 |
| main 新增 durable JSONL storage / node environment | 位于 `packages/durable`；SDK、RPC 类型和分发未改变。这些底层/实验设施不等于 Gupi 所用会话协议增加命令，也不要求 Gupi迁移存储 |
| 0.87.1 发行变化 | 新模型/思考能力、split-turn 压缩提示修复、无效 --mode 报错，以及部分 OpenAI-compatible provider 的纯图片消息修复。无新的 GUI 队列或扩展 UI 接口；动态模型读取和图片提交继续由 Pi 处理 |
| 已有运行证据 | 两项隔离 installed_pi 集成测试在本机 **0.87.0** 通过，无真实模型请求。0.87.1/main 本次仅做源码比较，不能将旧测试写成新版本运行验收 |

## 0.87 引入的语义仍需正确理解

- `context_edit` 改变模型上下文贡献，不重写原始历史。`get_messages` / `get_last_assistant_text` 的结果可能与界面可见历史不同；Gupi 复制可见回答，不为模拟模型上下文删改正文。
- `agent_before_settle`、`context_with_system` 是扩展钩子；扩展 `turn_end` 的 boundary 字段不是同名基础 AgentEvent 的新增 RPC 字段。Gupi 沿用 `agent_settled` 判定收尾。
- `entry_appended` 可来自 custom/custom_message/context_edit/compaction。Gupi 已定向回读历史；display:true 插件消息已按原块顺序展示在正文，display:false 保留原历史但不显示。
- retain-none compaction 的 firstKeptEntryId 可以指向压缩记录自身。GUI 展示原始历史与摘要，不复制 Pi 的上下文裁剪规则。
- RPC 图片由 Pi 按 `inputLimits.images.resize` 和配置处理；GUI 保留本地原图预览并发送原始内容，恢复历史的图像可能已经由 Pi 处理。见[职责边界](../../../../docs/dev/issue-217/gui-boundary.md)。

## 原生 RPC 的主要缺口

| 能力 | 源码事实 | Gupi 的影响 |
| --- | --- | --- |
| 插件子命令／参数补全 | `get_commands` 仅返回名称、描述、来源；没有 `getArgumentCompletions` 查询命令，`addAutocompleteProvider` 为空实现 | 已能补全命令名，参数仍需手动输入 |
| 插件自定义快捷键 | RPC 不枚举、不触发 `registerShortcut` 的回调；`onTerminalInput` 为空实现 | 无法自动继承插件快捷键；Gupi 自己的 action/keybinding 不受影响 |
| 插件自定义 UI | `custom` 返回 undefined；header/footer/editor 和 widget 组件工厂不传输 | 无法直接复用任意 TUI 弹层、阅读器和组件；插件需提供标准 RPC 降级路径 |
| 插件自定义消息／工具渲染 | `renderCall`、`renderResult`、message renderer 返回本地 TUI Component，RPC 传输事件数据 | 可显示文本、工具数据，无法自动继承插件渲染函数 |
| 组合问卷 | `select` 是字符串选项和单字符串返回，没有原生多选、附加说明、多字段 schema | 不能通用推断问卷结构；只能由插件顺序调用标准交互或解析输入文本 |
| 插件读取输入、粘贴语义 | `getEditorText` 返回空字符串，`pasteToEditor` 降级到 `setEditorText` | 插件读不到宿主正文；粘贴退化为替换文字 |
| 插件控制显示细节 | 工作提示／动画、隐藏思考标签、主题查询／切换、工具展开 API 在 RPC 下为空或不支持 | 插件设置无法驱动宿主；Gupi 自己实现对应显示能力仍可行 |
| 同文件树节点续聊 | 有 get_entries/get_tree/fork，没有直接 navigate_tree RPC | 历史预览和用户消息 fork 已有；任意节点原地续聊及导航总结／标签未接入 |
| 进程内资源重载 | 没有直接 reload RPC | Gupi 采用重启连接，不能完整保留原进程插件内存及生命周期 |

树导航和 reload 已在 RPC 模式的扩展命令上下文中绑定到 Pi 核心。因此“缺直接 RPC”不等于核心不支持；额外桥接扩展可以调用，但目前未采用私有桥接。

源码依据（相对于 Pi 仓库根）：

- `packages/coding-agent/src/modes/rpc/rpc-types.ts`：客户端命令及九类扩展 UI 消息。
- `packages/coding-agent/src/modes/rpc/rpc-mode.ts`：空实现、get_commands，以及扩展上下文的 navigateTree/reload 绑定。
- `packages/coding-agent/src/core/extensions/types.ts`：插件补全、快捷键和 TUI 渲染函数契约。
- `packages/coding-agent/src/modes/interactive/interactive-mode.ts`：TUI 补全与快捷键接入。

## 缺管理接口，但可以由应用另行接入

| 能力 | 可用基础与边界 |
| --- | --- |
| 插件／包管理 | 已由 #231 使用 Pi CLI 接入个人包安装、更新、移除及资源启停；无需把无 RPC 等同为无法管理 |
| Skill、模板、全局提示词管理 | #231 已接入个人资源文件、配置及 SYSTEM.md/APPEND_SYSTEM.md；包内资源只读，修改后由用户手动刷新会话 |
| Pi 设置、模型范围管理 | 部分设置有专用 RPC；其余读写由独立后续 #244 确定，没有通用设置编辑 RPC |
| 登录／退出登录、信任管理 | 无对应 RPC 工作流，需要单独接入认证／信任机制 |
| JSONL 导入导出、分享 | 可由应用提供文件管理或发布能力；不属于复用一个现成 RPC 的工作 |
| 参数表单、必填项及忙碌可用性 | get_commands 缺少结构化元数据，无法自动生成可靠 UI；不自行猜测 |

## 不应归为 Pi 上游阻塞

- 手动压缩、HTML 导出、复制会话、用户消息 fork：RPC 已支持，Gupi 已接入。
- 删除会话：没有专用 RPC，Gupi 已通过本地文件管理实现。
- 模型／思考选择、会话统计：已有 RPC 与应用接入。队列已有 `queue_update` 文本事件、`clear_queue` 和模式设置，但没有主动读取完整队列、按 ID 逐项修改/删除或附件无损恢复接口；不能笼统称为完整队列管理已被 RPC 覆盖。
- select/confirm/input/editor、通知、状态、文字 widget、标题、设置输入文本：九类标准 UI 已支持。
- InputGroup 已接入；#243 承接 Questionnaire 和原子内联标签，等待兼容正式版本，v0.6.6 尚未包含。Markdown 内联插件已发布；它不提供输入编辑能力，组件发布也不改变 Pi 问卷协议。
- `custom_message display:true`：已通过原生历史/事件接入正文投影，支持 Markdown、图片、复制和预览；与 #241 的短期插件通知分别处理。

已有设计参见 [命令能力对照](issue-226/builtin-commands.md)、[命令面板](issue-226/command-palette.md)、[扩展 UI](issue-222/README.md)。

## 有保留价值的上游参考

当前可用性以以上固定源码为准。删除旧的逐人 fork 活动时间、已失效预计日期和“近期活跃”表述；它们不再影响当前实现决定。以下只保留与具体缺口直接相关的历史方案入口，不承诺当前审查状态或上线时间。

| 缺口 | 历史方案／讨论 | 使用边界 |
| --- | --- | --- |
| 参数补全 | [#7621](https://github.com/earendil-works/pi/pull/7621) | get_argument_completions 的实现参考，正式 0.87.1 和本次 main 仍无该命令 |
| 同文件树导航 | [#1762](https://github.com/earendil-works/pi/pull/1762) | 区分导航与另建文件 fork；当前没有公开 navigate_tree RPC |
| 更广的 Web GUI/RPC 接口 | [#8840](https://github.com/earendil-works/pi/pull/8840) | 社区私有协议参考，不作为官方能力或替换依赖依据 |
| 输入 disposition 与队列关联 | [#9098](https://github.com/earendil-works/pi/issues/9098)、[#9832](https://github.com/earendil-works/pi/pull/9832) | 正式 RPC 未提供逐项身份和完整队列，不能据方案名称恢复客户端调度或“已接受”状态 |

## 实验协议的边界

Pi 的 `packages/protocol`、`packages/server`、`packages/durable` 和实验 Harness 是另一层架构。标准 coding-agent stdio 仍走 RPC mode；研究这些设施时应检查实际 SDK 接线、发行入口与协议契约，不以目录存在、包版本号或底层 JSONL 存储支持推断 Gupi 已可使用。当前继续按用户确认使用标准 RPC，不采用社区 fork 或自建桥接。

## 固定源码依据与后续复核方法

- 正式 [v0.87.1 release](https://github.com/earendil-works/pi/releases/tag/v0.87.1) 与 [CHANGELOG](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/CHANGELOG.md)。
- [RPC 类型](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/modes/rpc/rpc-types.ts)、[分发/扩展降级](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/modes/rpc/rpc-mode.ts)、[SDK](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/sdk.ts)。
- [AgentSession](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/agent-session.ts)、[SessionManager](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/session-manager.ts)、[扩展 API](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/extensions/types.ts)。
- [本次 main](https://github.com/earendil-works/pi/tree/898ab804050730e9dcefb4443875d5a932aa6a32)；新版文档已拆为 `docs/rpc-commands.md` 与 `docs/rpc-extension-ui.md`，拆分本身不表示协议增加。

升级复核时先比较正式标签与 main 的 rpc-types/rpc-mode、SDK、事件及历史格式，再核对 Gupi 实际调用与消息投影。版本号、PR 合并日期和社区自述不能替代发布源码和针对性运行证据。
