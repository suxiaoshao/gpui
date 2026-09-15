# 工具详情布局：Zed、Codex Electron 与 Gupi 对照

调研日期：2026-09-15。范围是工具详情的视觉组织。下方保留三端源码对比；当前设计按用户确认的“工具标题与展开按钮在卡片外，展开详情按工具类型组织”调整。

## 结论

**工具图标、标题与展开按钮保留在卡片外。展开后的详情才使用一张卡片，容纳本次调用的参数、结果、图片和错误。** 按工具类型安排头部、正文和尾部，沿用已有的单项展开/收起操作。

这里的“一张卡片”指一次调用，不是把多个工具、思考和中间 assistant 说明一起装进一张大卡片。现有的过程与多工具聚合仍负责组织调用顺序。

本次调整针对的问题：上一轮完成了按工具分类展示，但参数仍分别生成独立 TextView，结果又生成多个自带外观的代码块。外层只有纵向布局，没有共同的视觉边界，所以用户会看到“参数在外面、结果在另一块”的效果。把这些块原封不动套进一个框，仍会留下卡片内层层套框的问题。

两端值得借鉴的共同点是：**标题交代动作与对象，正文呈现内容，状态属于同一个工具容器**。不过，两端都没有给所有工具强制使用同一种卡片：

- Zed 对编辑、执行、待确认调用使用卡片；普通读取、搜索等仍可以是轻量标题与左侧引导线下的详情。
- Codex Electron 对 Shell 有完整的命令/输出容器，对文件修改有专门 diff 详情；读取、搜索、列目录在所核对的分支中主要显示简洁动作摘要。

因此，统一卡片是针对 Gupi 当前详情碎片化问题的建议，不应表述为“Zed/Codex 的所有工具本来就是这样”。

## 源码版本与更新结果

| 对象 | 本次依据 | 说明 |
| --- | --- | --- |
| Zed | 官方 `upstream/main`，`ba7da93e5ccc2b630077b2ae26c7581f3c21f984` | 本地 `/Users/sushao/Documents/code/zed` 的 `main` 从 `9d272b0363` 快进到该提交；更新前后均无本地未提交改动 |
| Codex Electron | 已解包的 `26.908.40834` | 使用本地静态源码快照，没有更新或启动 Electron；不能把该快照称为最新客户端 |
| Gupi | 当前工作区，包括上一轮尚未提交的工具详情实现 | 对照当前 `presentation.rs`、`tool_details.rs`、`markdown.rs` |
| gpui-kit | 当前依赖 `0.6.0` | 核对 Marker、GroupBox、TextView 的组合与样式能力 |

Zed 使用 `fetch upstream main` 后执行 `merge --ff-only upstream/main`，没有改写分支历史。

以下结论来自源码阅读，没有进行三端界面的运行、截图或视觉验收。

## 各类内容对比

| 类型 | Zed 最新本地源码 | Codex Electron 快照 | Gupi 调整前 | 调整方向 |
| --- | --- | --- | --- | --- |
| 普通文件读取 | 标题直接包含文件与行范围；普通展开区可显示 Raw Input 和 Output，正文有左侧引导线。原生 `read_file` 的特定输出可转为带行号内容 | 已识别的读取显示 `Read 路径` 等摘要，并能关联文件；这条分支没有展示读取正文 | 标题含路径，展开后再次显示“路径”、独立 offset/limit，再显示代码块 | 外部标题保持；详情头部显示路径和请求范围，正文紧接读取结果；参数不各占一个独立块 |
| Skill 读取 | 本次通用工具行根据 ACP kind/tool_name/location 渲染，没有在该分支找到类似 Gupi `SKILL.md` 的专用图标分类 | `oB` 能识别技能定义文件，转到 Skill 摘要 | 已区分 Skill，但展开后仍使用普通字段列表与代码块 | 外部保留 Skill 名称和统一图标，详情复用读取卡片；完整路径放在卡片头部 |
| grep 搜索 | 标题包含正则、大小写模式等；原生搜索还提供 ResourceLink、locations 和片段，通用展开仍可显示 Raw Input | 摘要表达“在路径中搜索某内容”，不是统一的结果表格 | pattern、path、glob、大小写、context、limit 分别占块，最后才出现结果 | 头部显示查询词；范围、文件模式和其他显式参数放在一个可换行的辅助区域；正文保留 Pi 返回文本 |
| find / ls | 按动作生成标题和输出，通常走非卡片的普通工具路径 | 主要为查找/列目录摘要 | 路径、模式、limit 与结果分开 | 同一张轻量卡片内展示查询范围与结果，避免形成表单式参数列表 |
| edit | 编辑调用使用带边框卡片，标题显示文件，正文为 diff editor；输入 JSON 不在普通参数区重复显示 | 按文件呈现修改摘要、状态和 diff；是否展示详情受活动显示模式控制 | 路径行在外，diff 自成代码块，成功确认文字还可能是另一块 | 文件路径归卡片头部，diff 是主要正文，错误/必要确认信息归卡片底部 |
| write | 原生创建与修改文件主要归入编辑流程，不能简单对应 Pi 独立 write 工具 | patch change 可以区分 add/update/delete，不能据此推断 Pi write 一定新增文件 | 路径、写入内容、确认结果各自展示 | 同一张文件卡片，正文展示提交内容，底部展示必要结果；仍不伪造不存在的旧内容 diff |
| Bash / PowerShell | 专用 TerminalToolHeader 组织 cwd、命令、运行/耗时/失败等；终端正文在同一个外框内，卡片失效状态可改变边框 | `exec-shell-container` 将命令、输出与 footer 组成一个容器；嵌入模式取消内部重复边框 | timeout 在外，“命令”标签、命令代码块、“输出”标签、输出代码块分别排列 | 卡片内部的命令区与输出区直接相接，timeout 为头部辅助信息，错误和截断归底部；不用额外的小卡片包每一区域 |
| 图片 | 内容按类型渲染，可受当前工具卡片/非卡片布局控制 | 工具内容 renderer 按 image 类型显示图片 | 已能显示图片，但它与参数、前后文字只有纵向间距关联 | 图片与前后文字继续保持顺序，全部位于同一次调用的卡片正文中 |
| 未知插件工具 | 非编辑/执行/图片的普通调用仍可能显示 Raw Input / Output；内容可含 Markdown、资源等 | 通用工具内容、结构化 JSON 与原始调用详情各有路径，部分相同结果做去重 | 参数 JSON、结果 JSON/文本、details 分成独立代码块 | 同一个容器内保留“输入 / 结果 / 附加信息”分区；只保留一套外框，不新增标签页或额外展开层级 |
| 思考 | 独立 Thinking 标题和 Markdown 折叠，不按工具输入输出卡片处理 | 有独立思考 Markdown 路径，展示入口随模式变化 | 已有 Marker 与 Markdown 折叠 | 本轮卡片建议限于工具调用，思考继续沿用已确认的展示方式 |

## Zed 的实际布局规则

`ThreadView::render_tool_call` 中决定卡片的条件明确为：

```rust
let use_card_layout = needs_confirmation || is_edit || is_terminal_tool;
let should_show_raw_input = !is_terminal_tool && !is_edit && !has_image_content;
```

因此，编辑与终端避免在顶部再列一遍通用 Raw Input；读取、搜索的展开区仍可能显示原始参数。不能用 Zed 的代码证明“参数一律隐藏”或“所有调用一律卡片化”。

对于卡片路径，标题和结果先组成同一个 `body`，外层统一设置边框、圆角、背景与裁切。`Embedded` / `Floating` 布局会避免再添加同一层外框。正文渲染函数接收 `card_layout`，按所处容器选择内部 padding/分隔线或非卡片左侧引导线。这种**外层统一拥有边界、内容按宿主调整样式**的做法值得借鉴。

Zed 的搜索结果链接来自原生工具提供的 ResourceLink 和 locations；读取行号转换也检查工具名和已知输出格式。Pi 返回普通文本时，没有理由照搬出看似结构化但可能错误的文件链接。

## Codex Electron 的实际布局规则

Shell 详情中，外层组件 `ke` 使用统一圆角、边框、背景，内部 Shell 组件 `M` 使用 `variant: embedded`，取消自己的重复边框。命令、输出和 footer 仍是不同内容区，但视觉上属于同一次调用。这正适合解决 Gupi 目前“每段内容像独立控件”的问题。

文件修改由 `UV` 按文件分派给 `GV`，文件路径、修改状态与 diff 关联，信息组织围绕文件变化。读取 `oB`、搜索 `Jz`、目录 `tB` 则更偏向简短摘要。不要为了统一样式，额外复制 Electron 的终端控制、文件编辑器或权限审批能力。

## 推荐的 Gupi 卡片结构

工具标题和展开按钮始终位于卡片外，展开不会改变标题内容或给标题加背景、边框和 padding。卡片内不再生成工具标题或第二个开关。

| 类型 | 详情头部 | 正文 | 尾部 |
| --- | --- | --- | --- |
| read | 完整路径，起始行与请求上限为辅助信息 | 返回内容按文件语言高亮，图片按内容类型显示 | 已有截断信息 |
| Skill | SKILL.md 完整路径与请求范围；Skill 图标留在外部标题 | 技能文件原文，复用读取展示 | 同 read |
| grep | 查询表达式优先；路径、glob、大小写、context、limit 等显式参数集中为辅助信息 | Pi 返回的匹配文本 | 已有结果上限、截断提示 |
| find | 文件匹配模式优先，搜索范围为辅助信息 | 返回的文件路径列表 | 已有上限、截断提示 |
| ls | 目录路径、显式限制 | 目录条目 | 已有上限、截断提示 |
| edit | 文件路径 | 优先使用返回 diff；无 diff 时保留请求修改内容 | 执行结果或错误 |
| write | 文件路径 | 提交写入的内容 | 执行结果或错误，不把覆盖写入推断为新增文件 |
| Bash / PowerShell | 完整命令，timeout 为辅助信息 | 命令输出 | 已有截断、完整输出路径；失败时保留完整返回内容作为错误区，不拆解或丢弃其中的部分输出 |
| 未知插件工具 | 有输入时显示“输入”及参数 JSON | “结果”区按顺序保留文字、JSON、图片 | 有 details 时显示附加信息 |

结构示意：

```text
文件图标  读取 src/main.rs  ▾

┌────────────────────────────────────────────────┐
│ src/main.rs                                    │
│ 起始行 4 · 请求最多 20 行                       │
├────────────────────────────────────────────────┤
│ pub fn greet(name: &str) -> String {            │
│     format!("你好，{name}！")                   │
│ }                                              │
└────────────────────────────────────────────────┘

搜索图标  搜索 TODO|FIXME  ▾

┌────────────────────────────────────────────────┐
│ TODO|FIXME                                     │
│ src/ · *.rs · 忽略大小写 · 上下文 1 行          │
├────────────────────────────────────────────────┤
│ src/main.rs:9: // TODO: 增加配置                 │
│ src/view.rs:42: // FIXME: 调整布局               │
└────────────────────────────────────────────────┘

终端图标  Bash cargo check  ▾

┌────────────────────────────────────────────────┐
│ cargo check                                    │
│ 超时 30 秒                                     │
├────────────────────────────────────────────────┤
│ Checking review-fixture v0.1.0                  │
│ Finished dev profile                           │
├────────────────────────────────────────────────┤
│ 截断 / 完整输出路径（仅有信息时出现）           │
└────────────────────────────────────────────────┘
```

布局规则：

1. **详情共同边界。** 参数、结果、图片和错误归属于同一次调用的展开详情。外部工具标题和折叠控制不纳入边框。
2. **根据内容组织。** 路径、查询表达式直接呈现，不加“路径”“搜索模式”标签；Shell 命令与输出由头部、正文位置区分，不再加同名标签。行数、超时、限制、错误和未知插件输入/结果保留必要说明。路径、查询表达式或命令优先；其余参数集中在辅助区域，可换行，不做逐字段卡片或徽章。
3. **正文连续。** 分区只使用必要分隔线，不分别套圆角和背景。代码仍可选择、复制和高亮；图片与前后文字保持返回顺序。
4. **按需生成区域。** 没有内容时不生成空头部、正文或尾部；没有任何详情时不显示空框。
5. **使用真实数据。** limit 是请求上限，不表示实际读取数量；不按普通文本行数推算匹配数量。当前没有通用、可靠的实际返回数量字段，因此不新增数量统计。已有截断与限制信息继续显示。
6. **运行中保持结构。** 输入先显示，结果到达后更新同一详情；失败不自动展开，错误内容保留在所属调用中，不添加新的 loading 卡片。
7. **不扩大功能。** 路径只选择复制；不新增展开全部、ANSI 配色、独立终端滚动或词级 diff。思考维持 Markdown 详情，多工具聚合仍在外层。

长路径、命令和辅助参数可在所属区域换行。未知插件结果不猜测结构，不按内置工具格式删除内容。上游返回的错误文本保留原样，不能用推测的状态或统计替代。

## 组件接入建议

当前 gpui-kit 组件索引把卡片式分组指向 `GroupBox` 或普通 GPUI 布局；没有在本次版本中找到名为 `Card` 的现成通用组件。因此不能直接宣称“换用 Card 组件即可”。

建议复用：

- Marker 与 Collapsible：继续负责工具标题、状态和现有折叠，不添加第二套开关。
- GroupBox 的 outline/fill 与样式接口，或与当前 Marker/Collapsible 直接组合的单层 GPUI 外框：选择能最少覆盖默认样式的方式。
- TextView：继续负责 Markdown、代码高亮和选择复制；正文在卡片内使用适合嵌入的局部样式，避免默认代码块表面形成重复外框。不要修改全局 TextView 样式影响 assistant 正文。
- 原生图片组件：继续展示 RPC 图片，保持与同一次调用的文字顺序。

`GroupBox.title` 是可选标题区，不应再重复生成一个与 Marker 相同的标题。工具标题只在卡片外渲染一次，详情外框只拥有一套圆角、边框和内容间距。具体样式接口需在实施时核对，不为这次布局创建通用工具渲染框架。

## 实现与验证

当前实现：

- Marker 和 Collapsible 保留原有标题与折叠行为；只有 `tool_details` 返回的详情拥有外框。展开时不再替换标题，不给标题增加卡片样式。
- 文件类、搜索类按上表区分主要信息和辅助参数；Shell 命令与插件输入置于详情头部。
- edit/write 的返回结果置于尾部；未知插件 details 独立为附加信息尾部。其他返回内容作为正文，图片维持原始顺序。
- 空区域不生成，也不为整个详情另加折叠层。
- `Markdown::embedded()` 仅用于详情内容，取消内部代码块重复背景、圆角与 padding；普通 assistant 正文、思考和全局 TextView 不变。

本轮已通过 Gupi 构建、25 项消息相关回归、Clippy（所有 target，warnings 视为错误）、格式与差异检查。没有自动启动窗口，视觉效果尚未运行验收，由用户通过已有“工具详情审阅 · 全场景样例”检查。

## 改动边界

主要调整 `tool_details.rs`、`presentation.rs` 与 Markdown 的局部样式：从逐项输出 `Field / Code / Notice` 的平铺布局，改为同一调用中的头部信息、正文内容和状态区。继续使用现有 RPC 结果、语言识别、图片解码和差异数据。

这项建议不需要修改 Pi RPC、不需要重新设计 operation 或 session 存储，也不需要重新计算文件 diff。本机已构造的“工具详情审阅 · 全场景样例”（13 组场景、34 次工具调用）仍可用于检查布局；具体样例保存在本机 Pi 会话目录，未作为仓库 fixture 提交。

实施后的重点应是确认：一项工具的展开详情只有一个视觉外框，唯一折叠入口保留在框外，参数全部归入详情卡片，长路径/命令可读，图片与错误不脱离所属调用，单工具和多工具聚合仍遵循原有层级。

## 源码索引

Zed，均固定到本轮更新的提交：

- [工具调用布局](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/conversation_view/thread_view.rs#L8199)：`render_tool_call`，card/raw input 条件、统一外框。
- [终端详情](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/conversation_view/thread_view.rs#L7862)：`render_terminal_tool_call`。
- [终端头部](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/ui/terminal_tool_header.rs)：`TerminalToolHeader`。
- [内容分派](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/conversation_view/thread_view.rs#L10243)：ContentBlock、Diff、Terminal 与卡片宿主布局。
- [Diff 正文](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/conversation_view/thread_view.rs#L10441)、[读取行号识别](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/conversation_view/thread_view.rs#L10538)、[思考](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent_ui/src/conversation_view/thread_view.rs#L7454)。
- [读取标题](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent/src/tools/read_file_tool.rs#L218)、[搜索标题与结果](https://github.com/zed-industries/zed/blob/ba7da93e5ccc2b630077b2ae26c7581f3c21f984/crates/agent/src/tools/grep_tool.rs#L90)。

Codex Electron 快照根目录与版本见[前一份调研的源码索引](tool-details-research.md#源码索引)：

- `conversation-blocks-b695a23bbff2.js`：`oB / Jz / tB` 摘要，`Gz` Shell，`UV / GV` 文件修改。字节位置分别可从 `function 函数名(` 定位。
- `exec-shell-container-948e64261748.js`：`ke` 外层容器与 footer，`M` 的 default/embedded 样式和命令/输出。
- `mcp-tool-item-content-fdee2352aaf0.js`：图片、结构化结果与原始调用详情。

Gupi：[分类详情](../../../src/features/home/messages/tool_details.rs)、[折叠与标题](../../../src/features/home/messages/presentation.rs)、[Markdown 适配](../../../src/features/home/messages/markdown.rs)。
