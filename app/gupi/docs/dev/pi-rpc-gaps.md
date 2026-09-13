# Pi RPC 能力缺口与社区调研

核对日期：2026-09-14。本地基线：Pi `71dca871bc80b6bc97be37f0ca3189399d651fff`；Gupi 当前工作区。本文记录能力边界和公开社区证据，不把调查结果自动转为实现计划。

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
| 插件／包管理 | Pi CLI 已有 install/remove/list/update；无需把无 RPC 等同为无法管理 |
| Skill、模板、全局提示词管理 | 已有资源文件、配置和 SYSTEM.md/APPEND_SYSTEM.md；需确定文件管理和重载范围 |
| Pi 设置、模型范围管理 | 部分设置有专用 RPC；其他配置需明确读写归属，没有通用设置编辑 RPC |
| 登录／退出登录、信任管理 | 无对应 RPC 工作流，需要单独接入认证／信任机制 |
| JSONL 导入导出、分享 | 可由应用提供文件管理或发布能力；不属于复用一个现成 RPC 的工作 |
| 参数表单、必填项及忙碌可用性 | get_commands 缺少结构化元数据，无法自动生成可靠 UI；不自行猜测 |

## 不应归为 Pi 上游阻塞

- 手动压缩、HTML 导出、复制会话、用户消息 fork：RPC 已支持，Gupi 已接入。
- 删除会话：没有专用 RPC，Gupi 已通过本地文件管理实现。
- 模型／思考选择、会话统计、队列操作：有 RPC；具体界面完善归 Gupi。
- select/confirm/input/editor、通知、状态、文字 widget、标题、设置输入文本：九类标准 UI 已支持。
- question 组件等待属于组件库和界面工作，不改变 Pi 问卷协议。当前锁定 gpui-kit/gpui-component 0.6.0 中未发现 question 入口；不能据此判断上游最新分支状态。

已有设计参见 [命令能力对照](issue-226/builtin-commands.md)、[命令面板](issue-226/command-palette.md)、[扩展 UI](issue-222/README.md)。

## 社区调研

调查范围和公开证据见下文。关闭不等于已合入；fork 最近更新不等于对应功能仍在开发；新协议不等于已有 stdio RPC 得到兼容增强。

### 调查范围与状态解释

2026-09-14 通过 GitHub API 核对官方仓库 issue、PR（包括 merged_at）、评论、公开分支及提交日期；检查维护者 badlogic、mitsuhiko、christianklotz、cristinaponcela 的相关公开工作，以及由相关 PR 指向的社区 fork。搜索词覆盖 rpc、argument completions、navigate_tree、reload、protocol/server、shortcut、custom UI、getEditorText、multi-select。分支时间均使用 UTC 提交时间，不以 PR 评论时间或仓库 pushed_at 代替功能进展。

部分 issue 有 `no-action` / `not_planned` 标签，但评论只有贡献者准入机器人的自动关闭通知；这不能当作维护者已技术评审并否决方案。以下状态是调查时的快照，不承诺更新，也未设置自动监控。

### 与现有缺口直接对应的讨论和 PR

| 需求 | 社区证据 | 核实结果 |
| --- | --- | --- |
| 插件参数补全 | [Issue #8214](https://github.com/earendil-works/pi/issues/8214)、[PR #7621](https://github.com/earendil-works/pi/pull/7621) | 与 Gupi 问题一致。PR 增加 get_argument_completions，并有插件补全测试；2026-08-04 被准入机器人关闭，未合入。Issue 也被自动关闭，无维护者技术回复 |
| 原 session 内树导航 | [Issue #8645](https://github.com/earendil-works/pi/issues/8645)、[PR #1762](https://github.com/earendil-works/pi/pull/1762) | Issue 明确区分 navigate_tree 与另建文件的 fork，当前关闭。PR 包含导航、标签、会话列表和导航总结取消，未合入 |
| 较早的一组 RPC 扩展 | [PR #1522](https://github.com/earendil-works/pi/pull/1522) | dnouri 提出树导航、会话管理、工作提示转发等，2026-02-16 关闭，未合入；不能作为可用上游 API |
| 进程内 reload | [Issue #6173](https://github.com/earendil-works/pi/issues/6173) | 报告缺 reload handler，2026-06-30 关闭。报告者关于旧版本曾有接口的说法未另行验证；当前本地源码确实没有直接 reload 命令 |
| RPC 登录／认证 | [Issue #8451](https://github.com/earendil-works/pi/issues/8451)、[#8095](https://github.com/earendil-works/pi/issues/8095) | 请求将 provider 认证流程暴露给客户端；当前均关闭。#8451 作者提到自己维护补丁，不构成上游接入完成 |
| RPC 能力发现 | [Issue #6345](https://github.com/earendil-works/pi/issues/6345) | 提议机器可读的 RPC 命令、字段与事件元数据；2026-07-06 关闭，未找到已合入实现 |
| RPC 与 TUI 能力一致 | [Issue #885](https://github.com/earendil-works/pi/issues/885)、[#2737](https://github.com/earendil-works/pi/issues/2737) | 维护者明确讨论 server 方向；issue 关闭为 completed 不表示旧 RPC 已达到全部功能一致 |
| 插件消费 prompt 后的结果说明 | [Issue #9098](https://github.com/earendil-works/pi/issues/9098) | 仍开放，最近更新 2026-09-12。提议返回 handled/queued/started；作者称已有本地补丁并申请提交 PR，本轮未发现对应开放 PR。不据此恢复 Gupi 已删除的“Pi 已接受”常驻状态 |

没有找到旧 stdio RPC 下插件快捷键调用、getEditorText 回读、任意 TUI 组件跨端渲染或组合多选 schema 的直接可用、正在审查的官方实现。检索中的近似 UI/TUI 议题不算协议补齐；例如 TUI Tab 接续补全问题与 RPC 根本没有补全查询接口是不同问题。该结论限定于上述公开检索范围。

已完成的上游工作也需区分：

- [PR #6078](https://github.com/earendil-works/pi/pull/6078) 于 2026-06-28 合入 get_entries/get_tree，只补历史读取，没有补 navigate_tree。
- [PR #8355](https://github.com/earendil-works/pi/pull/8355) 于 2026-08-27 合入 ui_prompt_start/ui_prompt_end 生命周期事件；没有把 custom Component 变成可传输 UI。

### 官方与主要维护者的路线

badlogic 在 #1762 的 2026-03-25 评论中表示正在重构，RPC 可能建立在新 server 上；在 #2737 的 2026-04-01 评论中再次说明 server 方向。其当时预计时间已过去，不作为当前交付承诺。

现在有实际合入的基础设施：christianklotz 的 [PR #7344](https://github.com/earendil-works/pi/pull/7344)（2026-07-30，远程协议）与 [PR #7386](https://github.com/earendil-works/pi/pull/7386)（2026-07-31，server）。但 [#7396](https://github.com/earendil-works/pi/pull/7396) 显示关闭且 merged_at 为空，不能因标题相关就视为该 PR 已合入。

后续主要工作在官方 main，不能只查个人 fork：

| 工作 | 最近可核对的实质证据 | 对 Gupi 的意义 |
| --- | --- | --- |
| Chord 服务与 server/client | mitsuhiko 于 2026-08-31 [把 lane RPC 改成 Chord service](https://github.com/earendil-works/pi/commit/ae2cc5116f5adc2ed9d2c88c3d6093fae2f960b7)，随后[把服务语义移入 Chord](https://github.com/earendil-works/pi/commit/1a7bc80e7cf57822726aa8cd9554b2f986976eb7)；09-01 继续清理兼容层 | 是近期真实推进的另一套协议／运行架构；不是在旧 JSONL RPC 上追加几个命令 |
| 实验远程 harness | 2026-09-05 [明确隔离开发入口并从发行包排除远程 harness](https://github.com/earendil-works/pi/commit/1382777ed8000e8a84f81053d66f6bb713dccd92) | 不能承诺升级稳定 Pi 即可使用；新协议 README 仍声明 experimental、无兼容保证 |
| `experiment/client-capability-bindings` | christianklotz [2026-07-24 提交](https://github.com/earendil-works/pi/commit/fec371a33f8b1168a75f1177531397eadf407cab)，分支头仍停留此处 | 演示服务端扩展调用 TUI/浏览器绑定，有 HTTP/SSE 和 Cap'n Web 实验；明确是本地 spike，不能作为近期活跃、完整插件 UI 桥接实现 |
| `bigrefactor`、`harness-v2/j4`、`switchable-tui` | 公开分支头分别为 2026-05-08、08-07、08-02 | 分支仍存在不表示最近继续开发；相关方向部分已转到 main |

当前实现说明见 [pi-protocol README](https://github.com/earendil-works/pi/blob/71dca871bc80b6bc97be37f0ca3189399d651fff/packages/protocol/README.md) 与 [pi-server README](https://github.com/earendil-works/pi/blob/71dca871bc80b6bc97be37f0ca3189399d651fff/packages/server/README.md)。它们描述 CBOR、服务路由、多客户端挂接和状态订阅，不能据此推出旧插件 UI／补全全部可用。

个人 fork 核对：

- [mitsuhiko/pi-mono](https://github.com/mitsuhiko/pi-mono) 的 `hot-reload-in-vm` 最后提交 2026-01-09，`custom-commands` 为 2025-12-29，`session-metadata-change-event` 为 2026-01-27；本轮未见个人 fork 上新的、独立活跃的 RPC 补齐路线。他最近相关提交主要在官方 main。
- [cristinaponcela/pi-mono](https://github.com/cristinaponcela/pi-mono) 的 `feat/ui-prompt-hooks` 更新于 2026-08-27，对应已经合入的 #8355；`feat/thinking-model-rpc` 为 2026-07-20，当前思考档位读取已在上游。不能当成仍待接入的完整 RPC 增强分支。
- christianklotz 的相关分支和 PR 直接位于官方仓库；本轮公开仓库检索未找到需要另查的 Pi 个人 fork。

### 仍有近期活动的社区实现

| fork／分支 | 已核实范围 | 活动与限制 |
| --- | --- | --- |
| [fan92rus/pi:livecraft](https://github.com/fan92rus/pi/tree/livecraft) | 当前 rpc-mode.ts 仍有 get_argument_completions handler，调用插件补全函数；对应 #7621 | 参数补全提交为 2026-08-04；分支 2026-08-30、09-05 继续同步上游，09-05 已同步 v0.85.1。补全代码在近期维护分支中保留；不宣称补全本身 9 月有新功能开发 |
| [btvn-nghia-tnh/pi:feat/pi-web-gui](https://github.com/btvn-nghia-tnh/pi/tree/feat/pi-web-gui) | [PR #8840](https://github.com/earendil-works/pi/pull/8840) 描述共享 RpcCore、22 个新增命令，包括会话列表、树导航、设置、主题、认证、文件搜索及 Web UI | PR 2026-08-30 被准入机器人关闭且未合入；分支仍在 09-05 修插件事件归属，09-08 [修自定义消息内容渲染](https://github.com/btvn-nghia-tnh/pi/commit/4676c5987788ef4e812cf28a7c5fa716c692cb68)。作者虽称 full parity，但说明终端专用 API 仍按 RPC 降级，不能理解为任意 TUI 组件均支持 |
| [dnouri/pi-mono:rpc-browsing-surface](https://github.com/dnouri/pi-mono/tree/rpc-browsing-surface) | 旧 #1762 的树导航方案 | 分支头停在 2026-03-25；作者 07-18 表示转向 Emacs 客户端自己的会话处理，08-27 链接其客户端 [PR #266](https://github.com/dnouri/pi-coding-agent/pull/266)。这是历史参考，不列为当前活跃上游 RPC 补丁 |

本轮没有发现上述关键缺口对应、仍开放等待合入的官方 PR。找到的是：已有历史补丁、近期维护的社区 fork，以及仍在变化的官方实验协议。未运行这些 fork、未验证其全部功能、未替换 Gupi 依赖；不根据 PR 作者自述宣称可直接投入使用。
