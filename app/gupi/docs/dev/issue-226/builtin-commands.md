# Pi TUI 内置命令能力对照

状态：调研完成；HTML 导出、复制会话与手动压缩已接入，其余新增能力范围待确定。按本地 Pi `71dca871bc80b6bc97be37f0ca3189399d651fff` 核对全部 **23 项**公开 TUI 内置命令。本表用于比较能力与选择接入范围，不预设某个命令的实施顺序，不展开单项功能开发方案。

## 统计与判断口径

- 统计来源为 `packages/coding-agent/src/core/slash-commands.ts` 的 BUILTIN_SLASH_COMMANDS。扩展注册的命令、内部调试与彩蛋不计入这 23 项。
- Pi RPC 的 get_commands 仅枚举 extension、prompt、skill；TUI 内置命令由交互层另行分发。RPC 有对应接口时，Gupi 可以接入该接口；仅发送 /xxx 文本不能假定执行 TUI 内置操作。
- 下表分别记录 **Pi RPC 支持、Gupi 当前实现、接入建议及理由**。Gupi 缺少 typed API/界面不代表 Pi RPC 缺少接口。
- 建议是待选择的能力范围，不等于已批准实现。已有 Gupi 替代入口保留多会话归属，产品行为不必为复刻命令名字而改变。

## 全部 23 项

| TUI 命令 | Pi RPC 支持 | Gupi 当前实现 | 接入建议与理由 |
| --- | --- | --- | --- |
| `/settings` | 无通用 Pi 设置界面接口 | 有 Gupi 自身设置 | Pi 设置管理另定范围；不能把 Gupi 设置当作等价实现 |
| `/model` | get_available_models、set_model | 已有模型选择器与动作 | 复用已有能力；可作为命令模式入口 |
| `/tree` | get_tree/get_entries 可读；无 navigate_tree | 有历史树预览，无同文件切分支续聊 | tree、fork 作为会话历史候选的搜索别名，打开现有面板；不冒充节点导航 |
| `/thinking` | get_available_thinking_levels、set_thinking_level | 已有思考等级选择 | 复用已有能力；修改会话等级与保存默认值区分 |
| `/scoped-models` | 无范围配置编辑接口 | 已读取、过滤范围，无管理界面 | 归模型配置管理；本次命令入口不附带实现 |
| `/export` | export_html；没有完整复制 TUI HTML/JSONL 导出交互的单一接口 | 标题栏右上角导出按钮，系统保存窗口选择路径后调用 export_html | 已接入 HTML 导出；JSONL 导出未接入 |
| `/import` | 无通用 import；switch_session 仅切换已有文件 | 无外部 JSONL 导入入口 | 独立评估导入、复制及目录归属，不能把 switch_session 直接当作 import |
| `/share` | 无 TUI GitHub gist 分享接口 | 无分享动作 | 暂不接入；需新增认证和外部发布能力 |
| `/copy` | get_last_assistant_text；也可读取当前分支文本 | 已有逐条消息复制及面板“复制最后回答” | copy 搜索映射最后回答动作，来源为实际执行分支 |
| `/name` | set_session_name | 已有在线/离线改名 | 复用统一 RPC 改名；离线会话先建立连接，再由 Pi 写入 |
| `/session` | get_state、get_session_stats 等 | 已有 token/context 统计提示，缺统一信息页 | 值得完善会话信息入口，集中展示身份、路径、模型与统计 |
| `/changelog` | 无专用接口 | 无 Pi 更新日志入口 | 无需照搬；Gupi 自身更新说明属于应用帮助 |
| `/hotkeys` | 无 TUI 键位表查询接口 | #231 已有 Gupi 快捷键查看、修改、清除及恢复默认；面板已有键位提示 | `hotkeys` 搜索别名尚未映射；不再将快捷键总览列为缺失能力，不展示未接入的 Pi TUI 键位冒充可用 |
| `/fork` | get_fork_messages、fork | 已有用户消息按钮和历史消息 fork | fork 搜索打开历史面板，由用户选择明确源消息 |
| `/clone` | clone（复制当前执行位置，并切换 runtime） | 会话上下文菜单“复制会话”，成功后转入新会话并定向更新目录项，不扫描全部目录 | 已接入；沿用 fork 的连接转交与历史读取，新会话输入框为空 |
| `/trust` | 无信任管理接口 | 无 Pi 信任配置界面 | 归独立安全/项目配置管理，本次不直接写配置替代 RPC |
| `/login` | 无认证流程接口 | Pi 认证由外部管理 | 归独立认证管理；不在命令模式里模拟 TUI 登录 |
| `/logout` | 无认证移除接口 | Pi 认证由外部管理 | 与登录统一考虑，不单独添加删除凭据入口 |
| `/new` | new_session | 已有 Cmd+N、多实例新建 | 复用 Gupi 新建，保留其他会话运行；不强切旧连接 |
| `/compact` | **compact**，支持可选 customInstructions；abort 可中止压缩 | 已接入 typed compact 与当前会话“压缩上下文”候选，复用压缩事件/历史及停止入口 | 已接入默认手动压缩；成功刷新历史与统计，失败显示 Pi 错误 |
| `/resume` | switch_session | 已有 Cmd+P、会话目录打开 | 复用既有会话选择与连接管理 |
| `/reload` | 无直接 reload RPC | Cmd+R 重启当前 Pi 并恢复会话 | 复用已确认方案；资源重载目标一致，插件内存和生命周期事件与 TUI 进程内 reload 不完全相同 |
| `/quit` | 无 quit slash RPC；由宿主管理进程退出 | 已有应用退出及受控收尾 | 复用应用菜单与平台退出入口 |

## 综合建议（尚未确定交付批次）

| 判断 | 命令 | 理由 |
| --- | --- | --- |
| 复用已有能力 | model、thinking、name、fork、new、resume、reload、quit | 已有业务入口，主要确定是否在命令模式展示及如何路由 |
| 已新增能力 | export（HTML）、clone | 分别位于标题栏右上角和会话上下文菜单；连接就绪且空闲时可用，复制还要求存在当前历史节点 |
| 已新增能力 | compact | 当前会话组发起默认手动压缩，停止沿用 abort |
| 值得完善入口 | session；hotkeys 搜索别名 | 统一会话信息页仍待确定；快捷键设置已由 #231 接入，仅命令搜索别名尚未映射 |
| 独立管理范围或暂不映射 | settings、tree、scoped-models、import、share、changelog、trust、login、logout | TUI 专属行为、RPC 缺口或涉及独立配置/认证/数据管理；具体理由见逐项表 |

统一面板已确定原始命令名搜索与现有 UI 映射，具体清单见 [command-palette.md](command-palette.md)。其他新增/完善项仍需确定交付范围。统一待确定项见 [decisions.md](decisions.md)。

## 接入边界

本地动作与 Pi 资源命令保留各自身份。按 [统一命令面板](command-palette.md)，仅显式选择本地候选派发 action；直接输入 /文字继续交给 Pi，不新增文本别名解析。同名本地动作与插件不互相去重，不因显示名称相同改变 Pi 分发。

已有新建和恢复使用 Gupi 多实例管理；已确认的刷新重连细节归 [reconnect.md](reconnect.md)，不在此重复设计。其他命令确定实现范围后，再按实际需要补充交互与关键验证。

## 源码依据

命令清单按本地源码核对，未拉取远程。HTML 导出与复制接入后，已通过应用层取消/失败/成功及连接归属回归，并使用隔离临时会话验证真实 Pi RPC 的 HTML 输出、复制后历史保留和原文件不变；未进行原生界面点击验收。

- Pi `packages/coding-agent/src/core/slash-commands.ts`：23 项公开内置注册表。
- Pi `packages/coding-agent/src/modes/interactive/interactive-mode.ts`：内置命令的参数解析与 TUI 分发。
- Pi `packages/coding-agent/src/modes/rpc/rpc-mode.ts`：compact、export_html、clone、get_last_assistant_text、get_commands 等支持情况。
- Pi `packages/coding-agent/src/core/agent-session.ts`：各会话操作语义。
- Gupi `crates/pi-rpc/src/protocol.rs`、`client.rs`：当前 typed API；`app/gupi/src/state/conversation.rs` 和 `features/home/`：已接入动作与界面。
