# 工具与思考详情展示调研

调研日期：2026-09-15。用户已确认本文推荐方向与下方 6 项交互选择；实现集中在现有工具详情中。源码对照保留调研时的基准，当前实现另见“实现与验证”。

范围：Gupi 中每条思考、读取、搜索、修改、Shell 和其他工具调用展开后的内容。沿用现有消息、一级过程、二级聚合与单项折叠关系，不重新设计聚合图标或增加其他功能。

## 结论

推荐保留统一的折叠入口，按工具语义提供不同的详情内容：

- 思考继续显示收到的 Markdown 内容。
- 文件读取、Skill 读取和写入显示带语言高亮的内容，参数转为简洁的路径、范围说明。
- 编辑优先显示工具返回的实际 diff。
- Shell 分开显示命令与输出，保留错误和截断提示。
- 搜索、查找文件和列目录先显示清楚的查询条件与实际返回文本，不把非结构化文本强行转换为结果树。
- 未识别的插件工具保留通用参数与结果展示。

无需把每项都做成独立卡片，也无需增加新的详情弹窗。最有价值的变化是让用户展开后直接看到“改了什么、执行了什么、找到了什么”，减少阅读 JSON 的负担。

Pi RPC 已能传递工具结果的 `details` 和图片内容。调研时的主要限制在 Gupi 的展示投影：它把结果提取成了文本，没有继续使用这些信息；本轮实现已接入这些结果数据。另一方面，Pi 插件的自定义 TUI 渲染函数不会通过 RPC 传来，不能直接复用其终端界面。

## 调研依据与边界

| 对象 | 本次依据 | 验证边界 |
| --- | --- | --- |
| Gupi | 当前分支提交 `93f22022` | 读取现有渲染、消息投影、RPC 事件与 Markdown 管理代码 |
| Pi | 本地源码提交 `71dca871b` | 对照内置工具、TUI renderer 与 RPC 事件输出 |
| Codex Electron | 本地解包快照 `26.908.40834` | 对照静态 Webview 源码；不代表所有模式都展示相同详情 |
| gpui-kit 依赖 | 当前使用的 gpui-component / gpui-base `0.6.0` | 核对 TextView、高亮与已有组件能力 |

调研没有启动应用、执行模型或工具，也没有进行视觉与性能验收。下文的 Electron 行为来自该快照中的渲染分支；特别是读取、搜索摘要不能等同于完整结果查看器。后续实现检查记录在“实现与验证”。

## Gupi 调研时如何展示

`messages/presentation.rs` 已按语义区分标题、图标、运行状态和失败颜色，但工具展开后的主体基本相同：

1. 把全部参数格式化为 JSON 代码块。
2. 把结果文本放进 `text` 代码块。

思考主体已经使用 Markdown，不需要随工具详情一起改成其他格式。Markdown 状态也已接入增量 `push_str`，不是每次都创建静态正文。

数据流中还有一个具体缺口：`messages/activity.rs` 的 `Tool` 只保留 `args`、`output: String` 和状态。历史工具结果通过 `DisplayMessage::text()`，运行中的结果通过 `text_content()` 提取文本；后者只选取 `content` 中的文本块。因此，`details.diff`、截断信息和图片没有进入当前工具详情的展示模型。底层历史消息与运行结果仍保留原始数据，并非 Pi 没有发送。

## 分类型对照

| 类型 | Codex Electron | Pi TUI | Gupi 推荐方向 |
| --- | --- | --- | --- |
| 思考 | Markdown 内容；运行与完成状态不同，可展开性取决于可展示内容；不同活动模式的入口不同 | 连续 thinking 块合并为 Markdown，弱化颜色与斜体，可切换隐藏 | 保留 Markdown 与现有运行 Marker；只展示实际收到的内容，不生成额外摘要 |
| 普通读取 | 识别读取命令后显示 `Read 路径` 等摘要，路径可关联文件；该分支没有展示文件正文 | 标题显示路径与行范围；成功时收起可只保留标题，展开后显示语法高亮内容 | 标题保留路径，详情显示本次返回内容；将 offset/limit 转为范围说明 |
| Skill 读取 | 能识别技能读取并使用专门摘要 | 对 `SKILL.md` 有专门分类，使用父目录名作为 Skill 名称；展开仍是读取内容 | 与普通读取复用主体，使用统一的 Skill 图标与名称；不另做一套 Skill 详情组件 |
| 内容搜索 | 识别后显示查询词和路径等简洁摘要；这里没有完整搜索命中列表 | 标题显示 pattern/path/glob/limit；结果为文本，收起预览 15 行，展开全部，并提示限制 | 查询条件用可读文本显示，结果保留真实返回格式；先不做需要猜测结构的命中列表 |
| 查找文件 / 列目录 | 显示查找或列目录的动作摘要 | 分别显示模式、路径与文本结果；收起预览 20 行，提示结果数量限制 | 按行展示路径或目录输出；不推断未返回的文件类型、大小或其他元数据 |
| 编辑 | 专门的文件变更入口、增删统计和 diff；详情是否显示受活动模式控制 | 专门的带行号、增删色和词级变化的 diff；完成后以实际工具结果更新 | 使用实际 `details.patch` / `details.diff` 显示变化，避免用户从 oldText/newText JSON 自己比较 |
| 写入 | patch 路径可区分新增、修改、删除；不能直接等同于 Pi 的 write 工具 | 从 `args.content` 展示带语法高亮的内容，收起预览 10 行；成功提示不重复正文 | 显示路径与写入内容，失败信息另行展示；没有旧内容时不伪造修改前后的 diff |
| Bash / PowerShell | 命令与输出分区、输出滚动、ANSI 颜色、复制命令与完成状态；有结构化退出码等数据 | `$` / `PS>` 区分入口；展示 timeout，运行中更新输出，收起看最后 5 个视觉行，另有计时和截断提示 | 明确区分命令和输出，运行时能看最新返回内容；错误与截断提示单独展示 |
| 自定义工具 / MCP 类工具 | 有通用调用详情，也有按内容类型展示图片、结构化结果及应用内容的路径 | 插件可提供 `renderCall` / `renderResult`；缺失或失败时回退通用渲染 | 未知插件工具使用通用详情；根据明确的内容类型展示文本、JSON、图片，不猜测专用协议 |

Pi 的预览行数是对照事实，不是建议 Gupi 照抄的配置。我们的工具详情已经有折叠入口，无需默认在每条详情内部再增加一层“展开全部”。

## 关键差异与推荐方式

### 1. 编辑：先使用已经返回的实际 diff

Pi 内置 edit 的结果包含：

| 字段 | 含义 | 使用建议 |
| --- | --- | --- |
| `details.diff` | Pi 为显示生成的差异，包含其自有行号格式 | 若使用它，需要明确按该格式处理，不能当标准 patch 解析 |
| `details.patch` | 标准 unified diff | 优先作为通用差异内容来源 |
| `details.firstChangedLine` | 首个修改行 | 可用于定位提示；没有对应交互前不必增加按钮 |

Pi TUI 还会在参数完整后读取本地文件，提前计算编辑预览。这个预览是 TUI 自己执行的逻辑，不是 RPC 保证返回的阶段数据。建议 Gupi 第一版只呈现实际结果，不为对齐预览而额外读取文件。工具尚未返回时保留运行状态；失败时显示失败结果。

历史详情尤其应以记录中的 diff 为准。当前磁盘文件可能已经变化，重新读取它不能还原当时的修改。

视觉上建议在现有展开区域内显示文件路径和 unified diff，增加、删除、上下文容易区分即可。先复用 TextView 代码块能力，再核对其 diff 高亮效果；如果需要行号或更准确的增删配色，增加局部 diff 内容渲染即可，不引入完整编辑器。

### 2. 读取、Skill 与写入：内容优先，参数降为说明

读取详情中，路径和请求范围应成为可读的说明，正文使用路径对应语言的代码块。不能把 `limit` 当作实际返回行数：文件可能提前结束，输出也可能被截断。

Skill 与普通读取共用内容展示，区别放在标题、图标和名称上。沿用已确认的 Skill 图标，并与命令面板保持一致。无需新增技能管理、专用面板或内容摘要。

写入工具的正文来自 `args.content`，成功结果通常只是确认信息。可以展示实际提交的内容，并明确失败状态。Pi 的 write 也可能覆盖现有文件，因此不能一律呈现为全绿色的“新增文件”。

读取返回图片时应按实际图片块展示，而不是把 base64 放入代码块。是否同时覆盖所有自定义工具的图片结果，属于后续实施范围选择。

### 3. 搜索：改善阅读，不虚构结构

Pi grep、find、ls 返回的主要内容仍是文本，不是统一的结构化命中数组。第一步可把参数从 JSON 改成“搜索词、目录、文件模式、限制”，结果继续保留原文。

不建议仅靠分割冒号等简单规则构造可点击的文件和行号。路径、平台、工具输出模式以及上下文行都会影响解析，错误链接比原始文本更难判断。若后续要做命中列表，应另有可靠的结果契约或明确受支持的解析范围。

“没有匹配”是有效结果，不应显示为失败。命中数量达到限制、单行过长、输出截断则应显示对应提示，避免用户把可见内容误认为全部结果。

### 4. Shell：命令、输出与状态分开

推荐保持一个工具折叠项，展开后依次展示命令、实际输出和必要状态。命令不再藏在 JSON 内；Bash 和 PowerShell 按真实工具类型区分，不根据命令字符串猜测。

运行中应更新同一个输出区域。Pi 的部分结果包含累计输出快照，不能把每次 `partialResult` 都当增量直接拼接，否则会重复。是否自动跟随底部，应与用户已经滚动查看旧输出的情况分开考虑。

Electron 的 Shell 结果有结构化退出码；Pi 内置 Bash 的 `details` 主要提供截断和完整输出路径，非零退出等错误常体现在错误文本中。Gupi 应使用 `isError` 和实际错误内容，不能假设存在同名 `exitCode` 或通过不可靠的文本提取制造结构化状态。

Pi TUI 的耗时计时由 renderer 本地维护。增加每个工具的耗时将是额外行为，本次不把它视为工具详情改进的必要条件，也不补造历史耗时。完整终端、后台终端控制等 Electron 功能不在本次建议范围内。

### 5. 通用回退：让插件始终可查看

已知内置工具采用明确的专用展示，其他工具保留参数 JSON 与结果内容。结果本身是可解析 JSON 时可以格式化；普通文本照常显示。图片按内容类型识别，不依赖工具名称。

Pi 插件的 `renderCall` / `renderResult` 返回 TUI 组件，不能通过现有 RPC 转成 GPUI 组件。扩展 UI 请求与这类工具结果 renderer 也不是同一接口。不能承诺安装任意 Pi 插件后，其 TUI 专用详情会自动出现在 Gupi。

## RPC 已有数据与应用侧缺口

| 数据 | Pi 来源 | 调研时的 Gupi 展示缺口 |
| --- | --- | --- |
| 工具名称、参数、调用 ID | `tool_execution_start` / 工具调用消息 | 已使用，可转为语义化说明 |
| 运行中结果 | `tool_execution_update.partialResult` | 已保留原始结果，但展示时只取文本 |
| 完成结果与错误标志 | `tool_execution_end.result / isError` | 已展示文本与失败状态，可继续使用其他结果字段 |
| 历史结果内容与元信息 | `toolResult.content / details` | 当前 Tool 投影未携带 details 与图片块 |
| 编辑 diff / patch | edit 的 `details` | 已在协议数据中，尚未使用 |
| 截断、结果上限、完整输出路径 | 各内置工具自己的 `details` | 应按工具类型解释，不能假设所有工具字段一致 |
| 插件 TUI 自定义渲染函数 | 插件本地 JavaScript renderer | 不经 RPC 传输，保留通用回退 |

`rpc-mode.ts` 输出 `toJsonEvent(event)`；该转换主要精简 `message_update`，其他事件直接返回。agent loop 同时把结果的 `content` 与 `details` 写入历史 `toolResult`，因此这些改进不需要新增 Pi RPC 命令。

需要区分两种“没有显示全部”：界面主动收起的内容可以直接展开；上游已经截断的内容不在当前消息里。后者只能说明实际限制与已返回的完整输出路径，不能把“展开”做成一个没有数据来源的承诺，也不应自动读取新的文件替代原始结果。

## 组件与实现方向建议

继续由现有 Message、Marker、Collapsible 和消息滚动容器负责外层布局。详情仅选择不同的主体内容，不叠加另一套卡片、默认 padding 或新的模态面板。

- Markdown 与带语言标记的代码块：复用现有受管理的 TextView 和增量更新路径。当前 gpui-component 已接入语法高亮，但具体语言和 diff 效果需要实施时验证。
- 图片：使用实际返回的图片内容与 GPUI 图片显示能力，不将图片编码成可见文本。
- Diff：先验证已有代码块的展示是否足够，再决定是否需要局部行渲染。本次没有找到可直接接入的完整 diff 或 terminal 组件。
- 复制等操作：优先沿用组件已有能力，避免同一内容出现重复按钮。
- 长输出：沿用现有消息视口，确有必要时才给特定输出增加局部滚动；不要给每一项都叠加滚动区域。

建议实施顺序为：先让展示模型保留必要结果数据，再做编辑 diff、读取/写入内容、Shell 命令与输出，随后完善搜索条件、截断说明和图片。思考现有 Markdown 展示可保持，未知工具始终有通用回退。

## 已确认的交互选择

用户确认以下选择，本轮按此实施：

- 展开后完整展示收到的内容，不再新增“展开全部”层级；上游截断单独提示。
- 错误不自动展开，沿用用户控制的折叠状态与标题失败标记。
- 路径可选择复制，不新增点击打开文件的行为。
- 按 RPC 内容类型统一显示图片，覆盖内置 read 与插件工具。
- 使用实际返回的 unified diff，由现有代码块区分增删与上下文；不增加词级高亮、增删统计或独立编辑器。
- Shell 分开展示命令和输出，沿用现有滚动方式，不新增 ANSI 配色与独立滚动。

## 实现与验证

- `Tool` 展示投影保留原始结果，运行中、完成及历史共用内容与 `details` 的读取方式。
- `tool_details.rs` 按工具语义选择正文，未知工具继续展示参数、文本/JSON、图片和扩展 details；同一次调用的混合内容保持顺序。
- 编辑优先展示 `details.patch`；仅有旧版 `details.diff` 时原样显示 Pi 格式。没有结果 diff 时仍可查看请求修改，不读取当前磁盘文件重算历史。
- 使用 gpui-kit 的 `tree-sitter-languages` feature 启用代码块高亮，未识别语言回退纯文本；PowerShell 命令保留自身语言标记，不当作 Bash 高亮。
- 图片复用 GPUI `Image` / `img`，base64 解码按可见元素状态复用，使用固定高度、保持比例且不裁剪的预览区域，避免解码完成改变虚拟行高度；无效图片显示失败提示。
- 一级过程、二级聚合与图标保持；工具单项已按后续确认的[卡片布局](tool-card-layout-research.md)整合参数、输入/结果和状态，思考继续使用受管理的 Markdown。

已完成受影响验证：

- `cargo build -p gupi --locked --offline`。
- `cargo test -p gupi features::home::messages --locked --offline`：25 项通过，包含实际结果优先、运行/历史与孤立结果投影、搜索限制、累计输出、图片内容顺序、base64 解码回退和明暗主题下的 diff 增删高亮。
- `cargo clippy -p gupi --all-targets --locked --offline -- -D warnings`。
- 格式与差异检查；21 个工具详情提示键在中英文中均存在。

没有新增自动模型调用、窗口交互或视觉验收。图片真实显示与各类详情的界面观感交付用户试用。

## 源码索引

Gupi（相对应用目录）：

- [详情与折叠渲染](../../../src/features/home/messages/presentation.rs)：`render_activity`、`render_tool`。
- [分类工具详情](../../../src/features/home/messages/tool_details.rs)：正文投影、内容渲染与图片适配。
- [活动投影](../../../src/features/home/messages/activity.rs)：`Tool`、`RunContent::project`。
- [Markdown 状态](../../../src/features/home/messages/markdown.rs)：TextView 状态复用与增量更新。
- [结果文本提取](../../../src/foundation/session_catalog.rs)：`text_content`。
- [RPC 会话状态](../../../src/state/conversation.rs)：工具执行事件处理。

Pi（本机源码根目录 `/Users/sushao/Documents/code/pi`）：

- `packages/coding-agent/src/core/tools/renderers/{read,edit,write,bash,grep,find,ls}.ts`：各工具的 call/result 展示。
- `packages/coding-agent/src/core/tools/{read,edit,write,bash,grep,find,ls}.ts`：参数、结果与 details 契约。
- `packages/coding-agent/src/modes/interactive/components/tool-execution.ts`：折叠、插件 renderer 与回退、图片处理。
- `packages/coding-agent/src/modes/interactive/components/{assistant-message,diff}.ts`：思考 Markdown、差异行与词级高亮。
- `packages/coding-agent/src/modes/{json-event.ts,rpc/rpc-mode.ts}`：RPC 事件转换与输出。
- `packages/agent/src/agent-loop.ts`：`emitToolExecutionEnd`、`createToolResultMessage`。

Codex Electron 解包根目录：`/private/var/folders/rq/c4thf05d5zz26j9g3nf759w00000gn/T/codex-sidebar-source-ld9z8im5/webview/assets`。该目录是本地临时快照，可能被清理；以下函数标识仅对本次版本有效：

- `conversation-blocks-b695a23bbff2.js`：`Uz` 分派读取/搜索/目录/Shell，`oB` 读取，`Jz` 搜索，`tB` 列目录，`Gz` Shell，`UV` / `GV` patch 与文件 diff，`CH` 思考。`NW` 根据显示模式控制原始命令与 diff 详情可见性。
- `exec-shell-container-948e64261748.js`：命令头、ANSI 输出、滚动与状态 footer。
- `mcp-tool-item-content-fdee2352aaf0.js`：文本、JSON、图片及结构化结果，调用原始详情。

组件依据：本机 Cargo registry 中 `gpui-component-0.6.0/src/text/mod.rs`、`gpui-base-0.6.0/src/text/text_view.rs`；核对 TextView 的代码块高亮、选择与内容展示能力。
