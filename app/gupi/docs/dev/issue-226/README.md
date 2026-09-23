# 主窗口快捷键与统一命令面板

状态：Implemented。已确认范围内的快捷键、快速打开、刷新重连与统一命令面板已实现；双入口、单行分类、Enter 执行/Tab 补全、无候选原文提交和独立输入已接入，按钮、侧栏和图标的快捷键提示已补齐。另已接入原始命令名搜索、HTML 导出、复制会话、用户消息 fork 入口与手动压缩。当前验证与覆盖边界见 validation.md；能力对照表中的其他建议不属于本次交付。

职责、状态转换与范围见各子文档，[决定清单](decisions.md)记录当前采用的行为。构建、回归与原生检查证据见[验证记录](validation.md)。

归属：[#226](https://github.com/suxiaoshao/gpui/issues/226)。插件 question UI 继续等待上游组件；全局热键与临时窗口属于 #221，插件/Skill/提示词管理不因本次快捷键盘点自动进入实现范围。

## 交付拆分

按可独立实现和检查的功能拆分，不按每个快捷键新建一份文档。各子文档维护自身工作项；本页负责范围、共同决定和调研索引，不重复维护完成记录。

| 顺序 | 开发文档 | 交付内容 |
| --- | --- | --- |
| 1 | [本地动作与快捷键路由](actions.md) | 原生设置、新建、焦点、侧栏、刷新等统一动作、键盘路由及可见入口的快捷键提示 |
| 1a | [刷新时重连 Pi](reconnect.md) | 手动刷新当前会话时关闭旧连接、重启 Pi 并恢复会话，重新加载 Pi 资源 |
| 2 | [会话快速打开](quick-open.md) | Cmd/Ctrl+P 搜索现有会话目录并打开 |
| 3 | [统一命令面板](command-palette.md) | Cmd/Ctrl+Shift+P 与 / 共用面板；按可见页面提供候选、单行分类、Enter 执行、Tab 补全与独立命令输入 |

参考资料：[Pi 完整快捷键](pi-keybindings.md)、[23 个 Pi 内置命令与 Gupi 对照](builtin-commands.md)。研究完成不代表上述功能已实现；测试入口也不等于原生体验验收。问题修复仅覆盖实际发现的行为，用户自行体验阶段不自动接管窗口。

### 完成记录与依赖

- 本页负责整体范围和导航；每份开发文档的状态、工作项、验证记录只描述其负责的实现。资料盘点完成不勾选实现条目；代码实现、构建/回归通过和用户体验分别记录。
- 两个命令入口的设计统一归 command-palette.md；共用候选、搜索、渲染和上下文路由。快速打开与内置命令能力对照继续独立维护。
- 快速打开与面板本身不依赖 Pi；Pi 候选按当前可见会话和连接状态提供，不读取隐藏或后台会话作为目标。/ 只是当前会话范围的便捷入口；新对话初始化仍由页面负责。
- 已按统一方案收敛已有入口，并实现 Enter 执行选中项、无候选原文提交、Tab 补全到同一输入框；不区分搜索词与命令文本，Tab 收起列表与 Esc 主动退出分开处理。新增内置能力范围 D-14 独立确定，不阻塞复用已有操作，也不从示例命令推导优先级。
- question 组件、正文查找、全局翻译均不是上述工作的前置依赖。没有新增数据库、用户配置版本或后台服务；命令候选与 UI 状态默认只存在本次运行。
- 本轮开发文档中的必要验证是实施时的最小检查计划，不授权自动操作用户的临时插件窗口。新增问题按实际影响补回归，不扩展成全量插件验收。

### 独立于 #226 的正文查找

Cmd/Ctrl+F 预留给“当前查看分支的正文查找”，不复用为目录搜索。已确认归 [#242](https://github.com/suxiaoshao/gpui/issues/242)，与会话信息弹窗和插件持久消息展示一起实施；开工时明确搜索行为。

它涉及稳定的消息/片段定位、折叠的工具与思考展开、Markdown 展示文本与源文本对应、长列表滚动、匹配高亮，以及流式增长后的当前命中保持，超过绑定一个快捷键的范围。建议首版只做当前查看分支的已加载文本：普通子串、不区分大小写、匹配计数、上/下一个、定位、Esc 关闭；不同时做跨会话全文检索、磁盘索引或正则。搜索是否包含工具/思考/压缩内容及高亮能力，需要在该 Issue 中确定。

## 1. 调研基线与结论

已检查两个仓库 main 的工作区均干净，fetch 对应上游 main 后进行 fast-forward 同步：

| 源码 | 远程 | 本次基线 | 同步结果 |
| --- | --- | --- | --- |
| Pi：`/Users/sushao/Documents/code/pi` | earendil-works/pi，origin/main | `71dca871bc80b6bc97be37f0ca3189399d651fff` | 已是最新；提交时间 2026-09-11 |
| Zed：`/Users/sushao/Documents/code/zed` | zed-industries/zed，upstream/main | `9d272b036335401f339d024ea94968fd51016c40` | 从 `480d81bf6a` 快进；提交时间 2026-09-12 |
| Gupi | 当前 #226 分支工作区 | 基于 #217 合并后的代码 | 既有 #222 文档和临时插件未提交改动保留 |

这是 upstream main 源码行为，不代表已发布的 Zed 或本机运行程序都已具备相同功能。没有运行 Pi/Zed/Gupi UI 测试；用户体验窗口不接管。未发现默认 Pi agent 目录下的 keybindings.json；下面以原生默认键位为准，不包含终端自身配置或插件动态注册键位。

关键结论：

1. Pi 注册了 **90 个动作**：TUI 基础 47 个、应用 43 个。macOS 默认有键位的动作 80 个，默认空绑定 10 个。这不是 90 个独立按键组合，也不是 90 个全局快捷键。
2. Pi 的 Ctrl+P 在主输入区切换模型；Zed 的 Cmd+P 打开文件搜索，Cmd+Shift+P 打开命令面板。Gupi 已确认借鉴 Zed 的入口划分，将快速打开的对象改为会话。
3. RPC get_commands 只枚举扩展命令、prompt templates、skills，**不枚举 Pi TUI 内置命令、应用本地动作或扩展快捷键**。
4. 原生编辑器的复制、剪切、撤销、Tab 焦点和输入法应优先保留；不能把终端 Ctrl 键整体替换为 Cmd 键。
5. 对“已有功能缺键位”和“功能本身尚未实现”分别处理。快捷键任务不顺带实现 TUI 专属功能或恢复先前暂停的交互设计。

完整上游动作及平台差异见 [Pi 默认快捷键清单](pi-keybindings.md)。

## 2. Pi TUI：按使用场景归纳

下表以 macOS/Linux 默认值为主；Windows/WSL 的差异见完整清单。Ctrl 为实际 Control 键，与 Cmd 不等价。

| 场景 | 默认键位与行为 | Gupi 迁移判断 |
| --- | --- | --- |
| 输入发送 | Enter 提交；运行中 steer；Alt+Enter follow-up；Shift+Enter / Ctrl+J 换行 | 保留 Gupi 已确认的 Enter/Alt+Enter 语义和现有 Shift+Enter、Cmd+Enter 换行，不按 Zed 的 Cmd+Enter 重定义 |
| 输入补全 | 输入 `/` 触发命令补全，Tab 补全；还支持 Skill/模板和文件路径等输入语法 | 命令列表可接 get_commands；文件补全不是此次必须实现的功能 |
| 取消与中止 | Esc 优先关闭输入补全；没有补全时中止生成、bash 等当前流程 | 应先消费弹层/局部取消，避免一次 Esc 同时取消弹层和停止会话 |
| 清空与退出 | Ctrl+C 清空，500ms 内再按退出；Ctrl+D 仅空输入时退出；非空时向前删除字符 | GUI 使用原生退出动作；不引入双击清空退出或覆盖 Cmd+C/D 的桌面习惯 |
| 空输入双 Esc | 500ms 内双按，按设置打开 tree、fork 或不处理；默认 tree | 不直接迁移；Pi tree 含执行位置导航，Gupi 历史预览不等同于它 |
| 模型与思考 | Ctrl+L 模型选择；Ctrl+P / Ctrl+Shift+P 循环模型；Shift+Tab 循环思考；选择器 Ctrl+S 保存默认 | Gupi 已有模型/思考弹层，可增加入口；保存 Pi 默认配置是另一能力，不能等同于会话内 set_model |
| 展示 | Ctrl+O 展开工具输出；Ctrl+T 展开思考；Ctrl+X 复制回答或树所选消息 | 先复用 Gupi 已有局部展开/复制；不要用 Cmd+X 替代原生剪切 |
| 队列 | Alt+Up 将排队内容恢复到编辑器 | 完整队列编辑仍属后续范围；不恢复已删除的“发送失败回复恢复”设计 |
| 会话入口 | new/tree/fork/resume 有动作 ID，但默认都没有快捷键 | 可为 Gupi 已有新建、历史、搜索和 fork 入口补桌面键位 |
| 会话选择器 | Ctrl+P 路径、Ctrl+S 排序、Ctrl+N 名称过滤、Ctrl+R 改名、Ctrl+D 删除、Ctrl+Backspace 空查询删除 | 都是选择器局部操作，不能当主窗口全局键位 |
| 树选择器 | 方向键、Ctrl/Alt+Left/Right 折叠与跳转；Shift+L 编辑标签、Shift+T 时间戳；Ctrl+D/T/U/L/A/O 等切过滤 | Gupi 已有树画布的方向键/Enter/E/缩放；不添加 RPC 未暴露的标签修改或节点续聊 |
| scoped models | Ctrl+A/X/P 全选/清空/供应商切换，Alt+Up/Down 调序，Ctrl+S 保存 | Gupi 当前仅投影读取模型范围，无范围管理 UI，不默认新增 |
| 编辑与选择 | 单词移动/删除、kill ring、方向键、Enter、Esc 等 | 原生文本编辑由组件负责；终端 kill ring 和外部编辑器入口不直接复制 |
| 全屏 transcript | PageUp/Down、Home/End；Ctrl+Shift+Up/Down 跳消息；Ctrl+Shift+F 搜索；Enter/Shift+Enter 搜索上下项 | 仅 Pi fullscreen 模式；GUI 应按焦点路由，不抢文本输入中的 Home/End 和上下键 |
| 终端能力 | Ctrl+Z suspend；Ctrl+Shift+D 调试；终端键盘协议、复制粘贴和图像路径插入 | 不迁移进桌面产品默认键表；附件能力单独规划 |

Pi 同一个键在不同组件中的含义不同。例如 Ctrl+P 可用于模型切换、会话路径显示、scoped models 的供应商切换；Ctrl+S 可保存模型、保存思考等级或切会话排序。统计必须保留上下文。

## 3. Zed：可借鉴的桌面快捷键

来源为默认 keymap，不枚举 Zed 的代码编辑、Git、调试等与 Gupi 无关动作。macOS 表里的 Cmd 是物理 Command 键；Windows/Linux 普通桌面动作通常用 Ctrl，具体采用各平台文件，不做字符串全量替换。

| 动作 | Zed macOS | Windows/Linux 参考 | 上下文与 Gupi 用途 |
| --- | --- | --- | --- |
| 打开设置 | Cmd+, | Ctrl+, | `!SettingsWindow`；Gupi 已有设置，应保留并统一菜单提示 |
| 打开设置文件 | Cmd+Alt+, | Ctrl+Alt+, | 高级入口；Gupi 无需为此增加设置文件编辑器 |
| 命令面板 | Cmd+Shift+P | Ctrl+Shift+P，Windows 另有 F1 | Workspace，本地 action 列表，不是服务端 slash commands |
| 快速打开 | Cmd+P | Ctrl+P | 文件搜索；Gupi 已确认用于会话快速打开 |
| 新建会话 | Cmd+N | Ctrl+N | AgentPanel / AcpThread；与其他上下文的新建文件不同 |
| 选择模型 | Cmd+Alt+/ | Shift+Alt+/ | AcpThread，不依赖将 Ctrl+L 改为全局快捷键 |
| 切换思考强度 | Ctrl+' | Ctrl+' | AcpThread > Editor；不能与“切模式”混为同一动作 |
| 切换模式 | Shift+Tab | 见各平台 AcpThread keymap | Zed 是模式、Pi 是思考；Gupi 不应同时绑定两种语义 |
| 左侧栏开关 | Cmd+B | Ctrl+B | Workspace；适合会话侧栏 |
| 右侧栏开关 | Cmd+Alt+B（另有 Cmd+R） | Ctrl+Alt+B | Workspace；适合历史面板；Gupi 若 Cmd+R 刷新就不能照搬其别名 |
| 全部 dock 开关 | Cmd+Alt+Y | 依平台表 | 可发现性较低，Gupi 首轮不必加入 |
| 聚焦项目侧栏 | Cmd+Shift+E | Ctrl+Shift+E | 区分“聚焦侧栏”与“开关侧栏” |
| 当前会话查找 | Cmd+F | Ctrl+F | AcpThread；Gupi 正文查找尚未实现，应与目录搜索区别 |
| 查找下一/上一项 | Cmd+G / Cmd+Shift+G | 按 search context 定义 | 查找打开后可用，不是始终操作会话列表 |
| 会话快速切换 | Ctrl+Tab / Ctrl+Shift+Tab | 按 AgentPanel 定义 | 有专门的 thread switcher；Gupi 尚无该控件 |
| 快捷键编辑器 | Cmd+K Cmd+S | Ctrl+K Ctrl+S | 有完整键位管理时再考虑；Gupi 本轮不实现编辑器 |
| 关闭窗口 | Cmd+Shift+W；设置窗口 Cmd+W/Esc | 按各平台窗口行为 | 主 workspace 的 Cmd+W 通常关闭当前 item，不应直接照搬为关闭会话 |
| 退出/隐藏/最小化 | Cmd+Q / Cmd+H / Cmd+M | 各平台原生行为 | 应用生命周期；关闭/隐藏不是 abort 或删除会话 |
| 输入区发送 | 默认 Enter；可配置 Cmd+Enter 发送；另有 Cmd+Enter ChatWithFollow 等上下文规则 | 可配置 modifier-to-send | Zed 发送模型不同于 Gupi 已确认规则，仅借鉴焦点划分 |

Zed 的有用结构：

- 菜单、按钮、快捷键复用 action。Zed command_palette 保存原焦点、按上下文枚举动作并按原焦点查询键位，供 Gupi 统一面板参考；Pi 资源由 Gupi 额外按可见会话接入，详见 command-palette.md，不假定 Zed 已合并 agent slash UI。
- Editor、AgentPanel、AcpThread、ThreadSearch、SettingsWindow 等上下文分别定义键位；子上下文优先，同层靠后定义优先。
- Tab/Shift+Tab 在菜单中选项移动，在输入中可能缩进/反缩进，在 agent 中还有模式切换。Gupi 需要明确作用域，不能用全局 Shift+Tab 覆盖正常焦点遍历。
- 多段 chord 有等待和冲突处理成本；首轮常用动作优先单段组合。

## 4. 调研时的 Gupi 入口（实现前基线）

| 位置 | 当前键盘能力 | 证据 |
| --- | --- | --- |
| 应用层 | `cmd-q` 退出、`cmd-,` 设置 | `src/app.rs` |
| 主输入 | Enter 发送；Alt+Enter follow-up；`super-enter` 转换为换行；输入组件处理 Shift+Enter | `features/home.rs`、`composer.rs` |
| 侧栏行 | 有焦点时 Enter/Space 激活 | `features/home/navigation.rs` |
| 历史画布 | 方向键选择；Enter 预览；E 展开；0 全图；+ / - 缩放；Space 配合平移 | `features/home/history/canvas.rs` |
| 控件局部 | 消息展开、思考滑杆和设置等存在局部按键处理 | 对应 presentation/pickers/preferences 模块 |
| Pi 命令 | pi-rpc 有 get_commands；主窗口没有调用，也没有命令面板或 Cmd+P | `crates/pi-rpc/src/client.rs` 与 app/gupi/src 搜索 |

不要误解 `super`：cmd/super/win 都视为 platform modifier，非 macOS 的 Super/Win 不等于 Ctrl；secondary 表示 macOS Cmd、其他平台 Ctrl。初次接入时已核对 gpui-kit 0.6.0 配套的 gpui-pre 0.3.3 支持 secondary；当前配套版本见根 manifest，该语义继续用于现有绑定，可用于这组可移植桌面动作。既有 super-enter 是已有行为，不在本次文档阶段悄悄改动。

## 5. Pi 之外应补的桌面动作与候选键位

以下保留候选盘点；首轮已确认的默认键以 [actions.md](actions.md) 为准，其他备选不自动进入实现。`主修饰键` 表示 macOS Cmd / Windows、Linux Ctrl，最终实现使用实际 GPUI API。

| 操作 | 候选默认键 | 所属/状态 | 边界 |
| --- | --- | --- | --- |
| 打开设置 | 主修饰键+, | #226，现有动作 | 复用已有设置入口；表单提交中沿用原启用条件 |
| 统一命令面板 | 主修饰键+Shift+P | #226，已统一 | 应用/当前会话动作及该会话 Pi 命令；/ 初始化同一面板的独立输入 |
| 会话快速打开 | 主修饰键+P | #226，已确认 | 搜索会话目录，不打开 Pi 命令菜单 |
| 新建会话 | 主修饰键+N | #226，已有鼠标入口 | 按现有项目规则新建，不隐式复用“关闭会话”语义 |
| 搜索历史会话 | 主修饰键+P | #226，已有目录搜索可复用 | 先统一现有搜索和快速打开，不默认再加一套重复入口 |
| 聚焦主输入 | 主修饰键+L | #226，缺少统一动作 | 先显示当前会话输入；扩展请求存在时聚焦其输入，不跳过待答交互 |
| 左侧会话栏开关 | 主修饰键+B | #226，已有鼠标入口 | 收起后若焦点原在侧栏，需要回到可见区域 |
| 右侧历史面板开关 | 主修饰键+Alt+B | #226，已有鼠标入口 | 区别于 Pi 原生 /tree 的执行位置导航 |
| 聚焦侧栏 | 主修饰键+Shift+E | #226，候选 | 不应再次触发时顺手关闭；与开关动作分开 |
| 刷新当前会话 | 主修饰键+R | #226，已确认改为重连 | 空闲时重启当前 Pi 并恢复会话，重载资源；保留正文/草稿，不影响其他会话 |
| 刷新会话目录 | 主修饰键+Shift+R | #226，已有入口 | 只刷新目录；不带出模型刷新 |
| 选择工作目录 | 主修饰键+O | #226，已有入口 | 打开现有目录选择器，不隐式导入会话或打开任意文件 |
| 模型与思考弹层 | 主修饰键+Alt+/；macOS 可另评估 Ctrl+L | #226，已有入口 | 不把“选当前模型”与“保存 Pi 默认模型”合为一步 |
| 重命名会话 | 侧栏焦点 F2，或先只在命令/菜单中提供 | #226，可复用现有动作 | 不抢输入框 Enter；重连/运行等门禁沿用现有逻辑 |
| 移到废纸篓 | 侧栏焦点下主修饰键+Backspace，或先不设默认 | #226，可复用现有动作 | 不能在普通/扩展输入中删除会话；完全沿用已有删除流程 |
| 停止当前运行 | Esc（无局部取消对象时） | #226，已有鼠标入口 | 弹层/补全/扩展取消先处理；不一键同时清空输入、中止和切会话 |
| 复制回答 | 先保留消息操作，不强占剪切键 | #226，已有复制入口 | 清楚区分所选文本、所选消息和最后回答 |
| 关闭/隐藏窗口 | macOS Cmd+W，其他平台原生关闭 | 生命周期候选 | 沿用当前平台关闭行为；不是结束运行；Windows 后台恢复入口属于 #221 |
| 退出应用 | macOS Cmd+Q；其他平台依菜单/明确键位 | 现有动作 | 等待当前既有退出收尾，不新增会话取消机制 |
| 隐藏/最小化/全屏 | macOS Cmd+H / Cmd+M / Ctrl+Cmd+F | 原生体验候选 | 先检查平台已提供能力，避免重复注册 |
| 文本复制/剪切/粘贴/撤销/全选 | 原生主修饰键+C/X/V/Z/A 等 | 组件行为 | 不另写一套文本操作；不接入图片附件作为快捷键附带工作 |
| 文本区域查找、上一/下一消息、跳到底部 | 先记录动作，再决定默认 | 功能缺口/体验后续 | 当前只有目录搜索，不能只绑定 Cmd+F 假装具备正文搜索 |
| 全局翻译热键、唤出临时窗口 | 在 #221 单独决定 | 后续阶段 | 不是窗口内 key binding；需系统热键注册和焦点恢复 |

## 6. RPC 约束：哪些键不能只换成 prompt

| 动作 | Pi 原生支持 | 当前 Gupi / 接入边界 |
| --- | --- | --- |
| 执行扩展/模板/skill | get_commands + prompt | 可用于命令入口；按当前会话加载，保留实例归属与失败重试 |
| Gupi 设置、侧栏、搜索、窗口操作 | 客户端本地动作 | 不向 Pi 发 `/settings` 等文字 |
| 模型/思考选择 | set_model、set_thinking_level 及查询 | Gupi 已接入，键盘复用已有处理 |
| 模型/思考循环 | cycle_model、cycle_thinking_level | 原生有；pi-rpc 当前 typed API 未接。cycle_model 没有方向参数，不能虚构向后循环 RPC；首次实施可先只开现有选择器 |
| 保存默认模型/思考、scoped models 管理 | TUI 修改设置 | 与会话内 set_* 不同，当前不能把 Ctrl+S 绑定到 set_* 冒充保存默认 |
| 中止 | abort；另有 abort_bash、abort_retry | 当前复用已支持的执行状态和中止入口，不臆测重写取消生命周期 |
| fork | get_fork_messages + fork | Gupi 已有；需要先选择源消息，不把当前整段文本当 fork 参数 |
| 新建/恢复 | 原生 new_session / switch_session；Gupi 自己持有多实例 | 复用 Gupi 当前会话管理，不能随意切当前 Pi 文件破坏实例归属 |
| 同文件树续聊 | 原生 RPC 无 navigate_tree 命令 | 不接键位；历史预览保持原有行为 |
| 队列恢复/编辑 | clear_queue 与队列状态等 | 完整队列 UI 另行处理；不能只复制 TUI 的 Alt+Up |
| 导出、复制会话 | export_html、clone | 已接入标题栏导出按钮与会话上下文菜单；其他内置能力见 [对照表](builtin-commands.md) |
| 手动压缩 | compact；abort 可中止 | 已接入当前会话组，支持 compact、/compact 搜索，完成后刷新历史与统计 |
| 插件 registerShortcut / TUI custom | 不通过 get_commands 暴露键位或组件 | 不把插件快捷键当 slash command，也不伪造 TUI UI |

RpcSlashCommand 只有 name、description、source、sourceInfo，没有参数 schema；客户端不猜测必填参数或生成表单。用户按 Enter 可明确执行当前选中项，按 Tab 则只补全命令并继续编辑；不因缺少 schema 强制二次确认。本地动作按各自语义执行，完整输入与草稿规则归 command-palette.md。

## 7. 已确认决定与剩余细节

1. **入口**：Cmd+P 会话快速打开；Cmd+Shift+P 与输入 / 使用统一命令面板，按当前可见页面显示适用命令，不提供范围切换；Windows/Linux 对应用 Ctrl 组合。
2. **补全与执行**：有选中项时 Enter 执行该项，无候选时提交完整文本给 Pi；本地候选仍执行 action，Tab 仅补全 Pi 命令。面板不区分搜索词和命令文本，手动输入与补全后编辑一致；Tab 收起列表后回改命令名可重新显示，Esc 主动退出才持续抑制重入。/ 来源保留完整原文；快捷键来源保留原正文。Enter 只提交一次，Tab 不发送。
3. **首轮范围**：设置、命令/搜索、新建、输入焦点、左右侧栏、刷新、现有模型选择与中止。低频操作先保证菜单/命令可达，不要求每个动作都有默认键。
4. **新建与刷新**：Cmd+N 复用 Gupi 新建入口；用户已确认 Cmd+R 与顶栏手动刷新改为重连当前 Pi，达到重新加载资源的效果。已连接空会话也允许重连，保留当前模型和思考等级，与 TUI 的资源刷新目标一致；自动数据回读继续使用轻量 refresh。详见[刷新重连](reconnect.md)。

既有约束继续成立：普通输入、扩展输入、弹层、侧栏、历史画布使用明确焦点作用域；中文 IME 组词优先；同一动作复用鼠标与键盘的启用条件；停止期间是否能提交遵循已确认的 Pi 行为，不因快捷键任务重新添加发送限制。扩展 question 控件改造等待上游，本轮不提前实现。

## 8. 源码证据

Pi（相对 `/Users/sushao/Documents/code/pi`）：

- `packages/coding-agent/src/core/keybindings.ts`：43 个应用动作与四项 TUI 平台覆盖。
- `packages/tui/src/keybindings.ts`：47 个基础动作。
- `packages/coding-agent/docs/keybindings.md`、`README.md`：作用域、fullscreen 和 slash command 说明。
- `packages/coding-agent/src/modes/interactive/components/custom-editor.ts`：补全 Esc、空输入 Ctrl+D、显式 history 绑定优先级。
- `packages/coding-agent/src/modes/interactive/interactive-mode.ts` 的 setupKeyHandlers、handleCtrlC、handleCtrlD：双 Esc/双 Ctrl+C 和运行中行为。
- `packages/coding-agent/src/modes/rpc/rpc-mode.ts` 的 get_commands/cycle_model 等分支、`rpc-types.ts` 的 RpcSlashCommand：RPC 命令列表与参数边界。
- `packages/tui/src/tui.ts`：独立于注册表的 Ctrl+Shift+D 调试入口。

Zed（相对 `/Users/sushao/Documents/code/zed`）：

- `assets/keymaps/default-macos.json`、`default-linux.json`、`default-windows.json`：默认按键及上下文。
- `docs/src/key-bindings.md`：上下文匹配、优先级、chord 行为。
- `crates/command_palette/src/command_palette.rs`：available_actions 构建命令列表、原焦点恢复与按原上下文显示键位。
- `crates/gpui/src/platform/keystroke.rs`：cmd/super/win 与 secondary 的语义。Gupi 接入时仍以自身依赖为准。

首轮实现与验证见各子文档及 validation.md。没有升级依赖、修改 GitHub Issue 或提交推送。
