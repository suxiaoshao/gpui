# Gupi 剩余工作与能力边界

本页是 Gupi 跨阶段剩余工作的统一入口，放在应用开发文档根目录，不隶属于某个 Issue。按已确认范围区分应用收尾、上游依赖、独立后续、能力候选与验证边界；详细契约和证据链接到对应文档，不再维护第二份清单。

整理日期：2026-09-29。应用基线为 `5ffc4c6f`，锁定 GPUI Kit 0.7.0 / gpui-pre 0.3.7；Pi 协议基线为正式 v0.87.1。接口源码比对和运行验证各有自己的日期与范围，见 [RPC/TUI 接入盘点](pi-rpc-gaps.md)。Issue 是否开放不代表功能是否完成；未验证场景不自动成为新的开发任务。

## 当前收尾

| 工作 | 当前事实与处理边界 | 依据 |
| --- | --- | --- |
| GUI 启动时的环境与代理来源 | Pi 继承 Gupi 已有环境，登录 Shell 捕获目前只补 `PATH`；macOS 系统代理不会自动转换为 Pi 子进程的代理变量。已实测系统设置了代理但 Gupi/Pi 启动环境未包含代理变量；后续明确 Shell 环境、系统代理与 Pi 自有配置的职责和优先级，再实施接入 | [环境快照](../../src/state/environment.rs)、[Shell 捕获](../../src/foundation/shell_path.rs)、[Pi 启动](../../src/state/pi.rs) |
| 会话失败的诊断日志 | 会话错误可显示在界面，但关键失败路径未写入诊断日志；临时会话使用 `--no-session`，无法从持久历史回查。后续补充可关联到会话和失败阶段的诊断记录，避免记录凭据及完整会话正文；不因此持久化临时会话 | [会话错误处理](../../src/state/conversation.rs)、[日志入口](../../src/app/logging.rs) |
| 设置搜索无结果提示 | 搜索、分类定位和清空恢复已正常；零结果时仍为空白。锁定的 Settings 未提供空态入口，沿用正式组件，不复制整套搜索实现；接入方式需先核对组件可用能力 | [设置计划](issue-231/README.md) |
| 最终集成验证 | 按最终改动完成受影响构建、测试、Clippy 和必要的打包启动检查；已有单项验证继续有效，不重新安排全部功能审计。发行平台的缺口保留在下方，不以测试数量替代原生结果 | [运行入口](../../README.md)、[原生体验与验收](issue-223/README.md)、[依赖升级验证](../../../../docs/dev/dependency-refresh-0.7.0/README.md) |

## 等待上游的工作

| 工作 | 当前限制 | 恢复条件与入口 |
| --- | --- | --- |
| 长回答深处定位的无障碍滚动 | gpui-pre 0.3.7 列表二次 prepaint 未回滚无障碍节点；无障碍激活时调试版可能重复节点断言，发行版会丢弃重复节点 | 保持正式依赖，等包含事务回滚修复的版本后升级并复测；不关闭无障碍、不改应用元素 ID 掩盖问题。[上游复现 #3295](https://github.com/longbridge/gpui-kit/issues/3295)、[应用验证](../../../../docs/dev/dependency-refresh-0.7.0/README.md#原生验收发现长消息定位与无障碍树) |
| Markdown 中文与行内代码混排的行末裁切 | 应用根宽度约束已修复；0.7.0 的逐字估宽仍可能与 macOS 整行排版不同 | 升级到包含修复的正式版本后复测。[修复实现 #3293](https://github.com/longbridge/gpui-kit/pull/3293)、[宽度实测](../../../../docs/dev/dependency-refresh-0.7.0/README.md#消息正文宽度) |
| 队列逐条返回草稿、编辑、删除及完整附件恢复 | 标准 RPC 仅给文字数组与整队清空结果，没有稳定条目 ID、完整快照和原子单条操作；实验 Harness 的能力尚未接到 coding-agent RPC | 等正式契约提供权威身份、消费结果、完整消息与需要的位置语义，再确定编辑/取消交互。不使用 clear/requeue 模拟，不维护第二套客户端队列。[队列协议与建议](issue-222/queue-composer.md#逐条操作的可行性与实现边界) |

正文搜索的匹配、上下项导航、原位高亮、离屏定位、用户消息搜索都已接入。原子 Token、Skill/模板选择填入输入框、文件/目录引用、图片附件和标准 Questionnaire 也已接入；它们不再属于等待组件发布的任务。

## RPC 与 TUI 功能差距

完整盘点统一保存在 [pi-rpc-gaps.md](pi-rpc-gaps.md)：**33 个 RPC 命令、24 种事件名、9 类扩展 UI**，以及 TUI 能力与宿主职责。逐项 TUI 内置命令见 [24 项命令对照](issue-226/builtin-commands.md)。不能把“没有调用同名 RPC”直接等同于缺功能，也不能把传输层保留 JSON 算作界面已接入。

| 分类 | 仍需关注的内容 | 后续归属 |
| --- | --- | --- |
| RPC 已有，Gupi 没有入口 | `set_steering_mode`、`set_follow_up_mode`、`set_auto_compaction`、`set_auto_retry`；直接 `bash` / `abort_bash` 与 `bash_execution_update` | 前四项归 Pi 配置 #244 的字段选择；直接 Bash 尚未选择实施 |
| RPC 已有，可选参数或专用动作未接 | `compact.customInstructions`、`get_entries.since`、循环模型/思考、仅取消重试、普通新建的 `parentSession` | 可选能力或优化；不影响已有压缩、历史、显式选择、停止与新建，不自动增加任务 |
| TUI 有，标准 RPC 缺契约 | 插件参数/子命令补全、插件键位、任意 TUI 组件及消息/工具 renderer、组合问卷、输入回读/插入式粘贴/主动撤销通知、插件显示控制、同文件任意节点续聊、进程内 reload | 保留协议差距；标准问答、通用工具详情、历史预览/fork、重连替代已经可用。不自建私有协议或终端组件桥接 |
| TUI 交互，应用可以独立实现但尚未选定 | 输入历史、外部编辑器、全局展开/思考显示开关、额外树筛选、模型循环动作、`hotkeys` 搜索别名、Pi changelog 入口、JSONL 导入/导出 | 保留为能力候选，不冒充上游阻塞；`@` 文件/目录候选和会话信息弹窗已有 |
| 需要独立产品范围 | 分享、诊断上传、本地诊断包、认证/信任界面、压缩期间的客户端输入队列、模型上下文编辑标识 | 当前未纳入；Pi 安装、升级、登录和模型提供方凭据继续由用户在 Pi 中管理 |

`turn_start` / `turn_end` 暂无独立界面需要；已有更细粒度消息、工具与 `agent_settled` 收尾。复制/回填继续取实际执行分支的可见原始回答，`get_last_assistant_text` 可能受 Pi `context_edit` 影响，不以替换 RPC 调用本身作为优化目标。

## 独立后续工作

这些是已确认的后续范围，不因当前收尾而自动提前实施，也不作为 Gupi 本轮新增交付条件。

| 工作 | 范围与前提 | 统一入口 |
| --- | --- | --- |
| Pi 配置图形化与项目级覆盖 | 管理 Pi 自身配置，显示继承、来源与生效范围；先选择默认模型/思考、范围、自动压缩、重试、队列模式、图片处理等实际字段。保留未知字段，不另存 Gupi 同义配置；现有会话仍沿用用户手动刷新规则 | [#244](https://github.com/suxiaoshao/gpui/issues/244) |
| 插件包市场与资源发现 | 包搜索、详情与资源发现；复用现有个人包安装、更新、移除和资源管理入口 | [#245](https://github.com/suxiaoshao/gpui/issues/245) |
| Jaco 退役及关联清理 | Gupi 合入 `main` 后执行。删除 Jaco 专用源码、旧 assets 宏、Lucide 子模块、MCP 工具及 CI/打包/文档引用；Quick Look、OCR 与 OCR 专用绑定生成链一并删除。保留仍有消费者的共享能力，不删除用户数据 | [#240](https://github.com/suxiaoshao/gpui/issues/240)、[清理清单](../../../../docs/dev/jaco-retirement/README.md) |
| 自研共享库契约修复与精简 | 窗口句柄生命周期、Windows 显示器/坐标契约、Form 校验依赖与错误归属、阻塞写入的退出收尾；再按消费者价值评估 Store/Operation 等精简。已有结论含静态风险，开始时重核当前源码，不能写成已复现崩溃或数据丢失 | [#248](https://github.com/suxiaoshao/gpui/issues/248) |
| app-theme 分类配色 | 原首个场景是 Jaco 模型用量图表；应结合退役重新确认仍维护应用的消费者，再决定联合配色评估和实现范围 | [#234](https://github.com/suxiaoshao/gpui/issues/234) |

其他 workspace 开放任务只保留入口，不放进 Gupi 的交付清单：[Lestty 原生终端 #205](https://github.com/suxiaoshao/gpui/issues/205)；[HTTP Client #200 当前计划](../../../../docs/dev/issue-200/README.md)为音频后端迁移后的三平台交付，已移除 GStreamer 和视频，不沿用旧 Issue 描述恢复这些范围。

## 发行与验证边界

以下是尚未获得充分运行证据的场景。按目标平台和本轮改动选择必要验收；历史记录中的每个“未验证”不自动成为重新开发或全量测试的要求。

| 项目 | 已有证据 | 尚未覆盖部分与入口 |
| --- | --- | --- |
| macOS 全局快捷任务 | 窗口启动/隐藏/重建、会话隔离、停止、录制键位和落盘已验证 | 实体键盘系统级派发、重复热键、外部应用取词与自动回填、权限允许/拒绝及跨屏场景未完整验收。[临时窗口验证](issue-221/README.md#验证记录)、[本机验收](issue-223/README.md#2026-09-25-本机验收) |
| 通知、Dock 与 Tray | 用户已确认完成通知和 Dock/Tray 数字正常；未读、后台投递与回源有自动化覆盖 | 原生通知点击回源、注意力跳动及部分权限路径仍需实机证据；Dock Liquid Glass 材质/系统外观响应和 Tray 小尺寸动态菜单需要目视确认。[通知文档](issue-241/README.md)、[图标与桌面体验](issue-223/README.md) |
| 原生输入、插件与队列 | 资源标签、图片、标准 select/confirm/input/editor 已通过隔离原生或对应 UI 集成验证；问卷焦点环、提交和取消已验证 | 真实中文输入法组词、现有整队取回/清空的完整原生操作、真实模型压缩与 retain-none 压缩形态未完整覆盖；任意 TUI 插件不属于通用兼容承诺。[资源验证](issue-243/README.md#实现与验证)、[测试入口](issue-222/README.md)、[队列](issue-222/queue-composer.md) |
| 规模与资源占用 | 已验证 2000 条消息、普通/临时四任务重叠运行、隐藏恢复和受管退出；已有 CPU/RSS 采样 | 未测精确冷启动、帧率或长期高负载，不能声称排除所有泄漏；两项文本上游缺陷单列等待。[测量范围与结果](issue-223/README.md#2026-09-25-本机验收) |
| Windows / Linux / 其他 macOS | 已有构建配置、平台代码和部分跨平台回归，macOS Apple Silicon 包已启动验证 | Windows MSI 安装/升级/卸载、Pi shim、原生菜单/热键/通知和混合 DPI；Linux 包的运行体验；Intel/旧 macOS 尚需对应环境。Linux Tray/全局热键后端按当前明确范围不新增。[平台边界](issue-223/README.md) |
| 正式分发 | macOS bundle 与 ad-hoc 签名已验证 | Developer ID 公证未验证；开发期 Pi Logo 的正式使用授权尚未确认，发布前需得到明确结论或采用已获授权方案。[图标决定](../../../../docs/dev/issue-218/README.md#d-04开发期-logo)、[发行范围](issue-223/README.md) |

## 已有能力的实现入口

用于判断剩余范围，不按 Issue 关闭与否重复立项：

- [应用骨架与恢复入口](issue-218/README.md)、[RPC 生命周期](../../../../docs/dev/issue-219/README.md)、[主会话与历史](issue-220/README.md)、[临时窗口](issue-221/README.md)。
- [动作与命令面板](issue-226/README.md)、[运行展示](issue-229/runtime-display-plan.md)、[目录读取](issue-229/README.md)、[个人资源与快捷键设置](issue-231/README.md)。
- [事件同步与局部刷新](issue-236/README.md)、[摘要/工具详情 Dialog 与复制](issue-238/README.md)、[用户通知](issue-241/README.md)。
- [插件持久消息、信息弹窗和当前分支查找](issue-242/README.md)、[原子资源标签与标准扩展问答](issue-243/README.md)、[图标/菜单/引导/本地化/打包](issue-223/README.md)。

## 维护规则

- 剩余工作只在本页保留摘要、归属和恢复条件；RPC 调用/事件表只在 `pi-rpc-gaps.md` 维护，专门交互设计由原文档负责。
- 解决后更新实现文档并从待办表移除；删除已经失效的等待条件，不靠累计 PR、提交或 Issue 状态维护完成记录。
- 升级先核对正式接口和实际消费者，再复测受影响场景。不得把实验 Harness、社区 fork 或 TUI 函数当作已发布 RPC。
- 新发现的可选能力、静态风险和未验证场景分别标明；文档条目本身不扩大用户授权或当前实施范围。
