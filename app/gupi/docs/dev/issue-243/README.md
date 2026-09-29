# 输入资源、Markdown 展示与扩展问答

归属 [#243](https://github.com/suxiaoshao/gpui/issues/243)，父 Issue #217。资源输入、项目文件候选、消息资源展示和标准扩展问答已接入正式 GPUI Kit 0.7.0；受影响验证记录见文末。

## 目标与已确认范围

让用户选择 Skill 后先编辑再发送；用上游原子标签表达命令，保留完整删除、选择、复制与撤销；消息中的明确资源引用使用一致的名称、图标和点击含义；用合适的组件改善标准 Pi 扩展问答。

已确认且不重新询问：

- 主窗口和临时窗口共用输入路径，继续使用 InputGroup、Textarea 和 Attachment；普通文件在正文光标位置插入 InputToken；`@` 选择图片也保留路径引用。主动添加或粘贴的图片保留 AttachmentGroup 和 RPC `images`。
- Skill 和模板选择后只填入输入框，关闭候选面板并聚焦正文，不直接调用 `prompt`。用户再次按发送键或点击发送才提交。Tab 延续已确认的面板内补全行为，不发送；不重新将此项列为待定。
- 文件搜索纳入本轮，复用命令面板作为选择入口；参考 Zed 的同一候选体系，具体模式见下文。文件标签保存在同一份 InputContent 中，不增加独立文件窗口或另一套 token 存储。
- Skill 使用 `/skill:名称 参数`，模板使用 `/模板名称 参数`。只将消息开头的一个已识别命令作为资源标签，参数保持普通文本；插件命令按 Pi 来源区分。
- Skill/模板模式只选择资源，参数统一回 Composer 编辑；保留原草稿和附件，不维护另一段面板正文再合并。
- 选择候选时创建原子标签，普通手输/粘贴保留可发送原文；标签恢复只在会话草稿恢复边界处理，不在每次 Change 后强制转换。
- 模板继续交给 Pi 展开；对没有采用附带文件参数的组合给出明确提示，不在 GUI 另写模板展开引擎。
- 已发送的 Skill 显示资源标签与用户正文，完整 Skill 指令默认收起、可展开；搜索命中时展开并原位定位。文件使用明确的引用/链接，模板历史缺少身份时显示原正文。
- 采用正式 GPUI Kit，不移植 Jaco/Zed 编辑器，不实现自己的退格、IME、光标或撤销引擎，不增加第二套附件、持久消息或客户端队列。
- Pi 仍负责模板/Skill 执行、插件输入钩子和模型行为。宿主组件不产生协议中不存在的多题、多选或复合问卷。
- 开发覆盖本文完整范围，包括资源输入、文件候选、消息展示与标准扩展问答；按实施顺序完成受影响验证后交付。

## 核对基线与证据

2026-09-28 核对：应用基线为主 Issue 分支 `9fd511fb`；workspace 锁定 `gpui-kit = "=0.7.0"`。实际读取 Cargo registry 中的 `gpui-base-0.7.0`、`gpui-component-0.7.0`，不将本地上游 main 当作发布 API。

Pi 最新正式发布为 [v0.87.1](https://github.com/earendil-works/pi/releases/tag/v0.87.1)，commit `f07218c4d4bbc12bef056a7058c3dd49dfe41abe`。本地参考 main 为 `898ab8040`；已比较该 tag 与本地 main 的 `agent-session.ts`、`prompt-templates.ts`、`rpc-types.ts`、`rpc-mode.ts`，这些文件无差异。本次没有升级用户安装的 Pi，也没有发送模型请求。

参考客户端：Zed 本地 main 已快进至 `244023605536a412ab6b8d5b658466b89fb15401`；Codex 读取本机已解包的 Electron **26.924.22138**。下文描述这两个代码版本中核实的路径，不将静态阅读当成当前运行客户端的交互验收。

| 已核实能力 | 正式 API / 实现 | 对应用的要求 |
| --- | --- | --- |
| 原子编辑 | `InlineToken::new(id, text).with_label(label)`；`replace_with_token`、`replace_range_with_token` | ID 表示资源，text 保留真实 Pi 命令。范围为 UTF-8 字节且落在字素边界；组件负责整体编辑与历史。标签本体不含换行，参数可多行 |
| 连贯输入快照 | `InputContent::new(text).with_token(range, token)`；`TextareaState::content()`、`tokens()` | 文本与标签一起取快照；不维护应用自己的独立偏移数组 |
| 恢复与用户编辑不同 | `set_value(InputContent)` 不发 Change，但清空撤销历史、重置选区；范围 token 替换是可撤销操作 | 候选插入必须走编辑 API，不能靠每次 Change 后 `set_value` 重建。`set_value` 仅用于真实的会话/外部草稿恢复 |
| 输入外观与点击 | `Textarea::token`、`on_token_click`；现成 `InputToken::new(context).icon(...)` | 使用组件的标签皮肤，应用补图标/资源说明；不自行做点击与选区冲突处理 |
| Markdown 原子显示 | `MarkdownExtensions::plugin`、`MarkdownPlugin::parse/render_inline`、`MarkdownNode::text/markdown`、`InlineElement` | 可以替换已识别的 AST 节点。parse 一次接收整个 AST 节点，并非任意子字符串扫描器；不能把整段普通文字吞成一个 token |
| Markdown 与查找 | `TextView::markdown_extensions`；现有 `MarkdownState` / `Registry` / `RenderedText` | 扩展由 TextView 渲染时传给 state，`TextViewState::set_markdown_extensions` 在 0.7.0 是 crate 私有方法。不能假设离屏 state 有可直接调用的设置 API；见下方技术验证点 |
| 问答组件 | `QuestionnaireState`、Item/Choice/Input definitions、`QuestionnaireEvent::Submit` | Gupi 映射请求/回复，组件拥有选择、导航、焦点和验证；只在明确提交时回复，不能把 `AnswerChanged` 或 `Completed` 当提交 |
| 问答输入限制 | `QuestionnaireInputDefinition` 要求 `Entity<InputState>`；freeform 的空白值被视为无答案 | 不能直接替代 Pi 的多行 editor，也不能借组件验证禁止 Pi 允许的空字符串 |

组件代码入口（相对于对应 0.7.0 crate）：

- Base：`src/input/base/inline_tokens.rs`、`src/input/base/state.rs`、`src/input/textarea/mod.rs`。
- Component：`src/input/token.rs`、`src/input/textarea.rs`、`src/questionnaire/components.rs`。
- Base Markdown：`src/text/markdown_ext.rs`、`src/text/format/markdown.rs`、`src/text/text_view.rs`。
- Base 问答：`src/questionnaire/types.rs`、`src/questionnaire/state.rs`。

## Codex / Zed 对照：选择、粘贴与消息展示

| 交互 | Codex Electron 26.924.22138 | Zed `2440236055` | Gupi 采用的方案 |
| --- | --- | --- | --- |
| 选择 Skill / 文件 | Composer 候选按触发范围调用 `insertMention` / `insertAtMention`，最终以编辑事务 `replaceRangeWith` 插入节点、补空格并聚焦；选择资源与 submit 是分开的路径 | `PromptCompletionProvider` 同时处理 slash 和 mention；Skill/文件候选返回 `replace_range`、链接文本与确认回调，回调注册 MentionSet 折叠标签，不发送消息 | 保留同一份 Composer 正文；资源选择只改资源所在范围，参数回正文编辑，避免两份草稿合并 |
| 手输与粘贴 | `/`、`@`、`$` 的候选触发由配置控制；确认候选才插入对应资源节点。粘贴分支另行识别资源链接/结构化 HTML；普通文字有原样粘贴路径。恢复时可从序列化链接还原资源节点 | 普通 Edited 路径清理失效 mention，不扫描整段文字建立标签；粘贴明确的 `[@名称](URI)` 时解析资源 URI、注册并折叠。文件/图片剪贴板另走上下文导入 | 区分候选确认、普通文字编辑、明确资源恢复；不以每次 Change 扫描全文代替编辑事务。两者的资源链接不等于 Pi 的 `/skill:名称` 语法 |
| 文件搜索入口 | 文件与其他上下文类型共享 Composer 候选入口，使用当前会话的搜索 roots | `PromptCompletion::{SlashCommand,Mention}` 共用编辑器补全，`@` 还可选择资源分类；不是另开全局命令面板编辑第二段正文 | 复用 Gupi 已有 Command 面板，增加文件模式，默认搜索当前会话 cwd；复用组件与候选逻辑，无需照搬 Zed 的完整编辑器 |
| 已发送的用户消息 | 用户文本走 Markdown 渲染及 composer-link 扩展；明确 Skill/文件链接使用名称、图标和点击目标。Skill 可用性与路径满足条件时打开文件/侧面板，无法解析时仍能显示标签 | `EntryViewState` 对用户消息复用 MessageEditor，根据能力设为只读；从 ACP Text、Resource、ResourceLink、Image 内容块恢复文字与 mention。MentionCrease 显示名称/图标，文件打开文件、Skill 打开 Skill 文件 | 借鉴“资源标签＋可查看内容”，不要求输入与消息共用同一个编辑实体。Gupi 输入用 InputToken，消息用内容投影和 Markdown plugin；身份只能来自 Pi 实际保存的数据 |

Zed 源码入口：`crates/agent_ui/src/completion_provider.rs` 的 `completion_for_skill`、`completion_for_path`、`confirm_completion_callback`、`PromptCompletion`；`message_editor.rs` 的 Edited 订阅、`paste_item`、`parse_mention_links`、`insert_message_blocks`；`entry_view_state.rs` 的 UserMessage 分支；`ui/mention_crease.rs`。Zed 明确资源粘贴会解析/加载上下文，是其 ACP 接入的一部分；Gupi 不据此在 GUI 复制 Pi 的文件读取与 Skill 执行。

Codex 解包目录的 `webview/assets` 中：`app-initial-d817715f10a0.js` 的 `insertMentionNodeInRange`、`handlePaste`、`bHn` / `kHn`；`app-primary-cca0c1a58f0f.js` 的 `setSuggestionTriggers` 与资源链接呈现分支；`user-formatted-text-f992f332bf4c.js`、`user-message-markdown-af13d68dcbfa.js`。上述入口分别核实选择范围替换、链接还原、普通文字回退和用户消息渲染；不从“没看到一个函数”推断整个客户端绝无其他自动转换，也不把 Codex 的 `$Skill` / 资源链接照搬为 Pi 命令。

## Pi 语义：输入标签与历史展示需要分别处理

### 命令来源与优先级

`get_commands` 依次返回扩展、模板、Skill，并提供 `source` 与 `sourceInfo`。命令面板使用 `Id::Resource` 区分 Skill/模板选择，`Id::Pi` 保留插件执行，`Id::File` 确认文件选择；同名去重继续尊重 Pi 顺序，不能先按视觉分组排序再改变优先级。

`AgentSession.prompt` 先尝试扩展命令，再调用插件 input 钩子，然后展开 Skill/模板。发送保留原始命令字符串才能保持这个顺序。未知命令和普通 `/` 保持普通文本，不依据外观猜资源。

分隔符也以实际解析器为准：此版本模板命令接受空白分隔，`_expandSkillCommand` 则使用第一个普通空格分开名称与参数。候选插入命令后补空格；命令后直接换行再附加文件的场景需要纳入验证，不能假定 Skill 与模板的参数解析完全相同。

### 历史中真正保留了什么

- Skill：Pi 展开为完整的 `<skill name="..." location="...">…</skill>`，其后是用户参数；Pi 自己提供 `parseSkillBlock` 解析这种完整结构。历史包含名称、路径与当次 Skill 正文，可以在 GUI 中识别展示。
- 模板：历史保存展开后的正文，通常没有原模板名称、参数或来源。不能通过正文比对当前模板倒推出身份，不能仅为标签另存一份消息映射。没有身份时展示原正文是协议边界，不是待补缓存。
- 文件：当前 RPC 里的 `@路径` 是文字引用，不是 CLI `pi @文件` 的读取语义。消息也没有独立的文件附件字段。只有明确的文件链接或完整引用行适合单独展示，不能从普通句子猜文件。
- 图片：保持原有 RPC 图片块和预览入口，不因资源标签改造重复显示或改写图片。

上述结论也适用于重启后读取的历史，不只适用于本次 Gupi 发送的消息。

### 文件引用与模板参数

文件引用按正文中的实际位置发送，不再统一追加末尾。例如 `参考 @旧方案.md，修改 @新方案.md` 保留文字与路径的对应关系。路径含空格或引号时使用 Pi 参数解析兼容的引用方式；GUI 不读取文件正文，不执行模板。

路径引用同时支持文件和目录，不以 `is_file()` 作为插入条件。项目候选、系统选择器、拖入与粘贴均可选择目录；候选、输入标签和明确的消息资源链接使用 Folder / FileText 区分。类型在扫描、插入或解析时取得，不在标签每帧渲染时读取磁盘。只有实际文件且扩展名为支持的图片格式时才进入图片附件读取；名为 `pictures.png` 的文件夹仍是路径引用。快捷任务读取剪贴板路径时也保留目录引用。Pi TUI 的 `packages/tui/src/autocomplete.ts` 同样保留目录候选，并以尾部 `/` 标识；GUI 不据旧附件流程追加“只能引用普通文件”的限制。

如果正文是 `/模板 参数`，文件引用同样属于模板参数；不使用该位置参数的模板可能丢弃路径。例如 `/总结 重点 @report.txt 结尾` 中，文件是第二个参数，仅使用 `$1` 的模板不会保留文件。校验按已选文件 Token 与当前 quote-aware 参数范围的交集计算实际位置，支持位置参数、全参数与切片。图片使用独立的 RPC `images` 字段，不受模板文字参数替换影响。

现有临时任务的 `shortcuts::template_message` 是另一条既定路径：有参数时构造模板调用，无参数时组合模板正文与输入。它不是完整的 Pi 模板展开接口，不能直接作为普通 Composer 的通用展开器。

已确认继续发送原始模板调用，由 Pi 展开。模板没有采用附带文件参数时给出明确、可操作的提示，草稿与附件保留，用户可以调整模板或这次组合；不静默丢弃文件，也不自动改成 GUI 展开正文。提示判定要核对当前模板参数用法，包括位置参数、全参数与切片，不能仅以“有没有 `$ARGUMENTS`”作判断。GUI 不另写模板执行引擎，也不改变插件 input 钩子看到的原始输入或同名扩展命令优先级。

## 接入位置、状态归属与生命周期

| 归属 | 当前实现 |
| --- | --- |
| `features/command_palette.rs` | 候选保留 source/sourceInfo，分开“执行应用/插件命令”与“选择输入资源”；复用已有目标 session/binding 检查、关闭面板和焦点处理 |
| `features/home.rs`、`home/slash.rs` | 统一资源写入入口，接回同一个 Textarea；明确候选提交与真正发送的两次操作，插入后同步 slash 状态避免立即再次弹出 |
| `features/composer.rs`、`home/composer.rs` | 保留 InputGroup、附件、模型选择和底部操作布局；使用 InputToken 展示命令，扩展请求仍在当前输入区域中处理 |
| `state/conversation.rs` 的 Session 草稿 | 继续作为唯一会话草稿来源。内存草稿使用 InputContent 快照，保留切换/临时窗口重建后的已选标签，不另加 token/参数/草稿表；Textarea 仍拥有编辑历史与 IME |
| `WorkspaceFile::DraftFile` | 普通草稿继续保存命令原文，临时草稿继续不落盘。纯文本草稿恢复时在恢复边界按当前命令目录识别完整开头命令；不另建会话 sidecar 或保存展开后的 Skill 正文 |
| `home/messages/markdown.rs`、消息内容投影 | 资源是原始 Pi 消息的可重复推导展示；共用现有解析状态、复制、消息 ID 和查找定位。不得额外保留一份会话全文 |
| Session `pending_ui` / `reply` | 继续拥有请求顺序、来源、回复、deadline 和失效清理；Questionnaire 只替换适用交互的控件状态，不复制请求队列或造问卷组 ID |

同步检查要涵盖文本和标签元数据：只改变标签元数据时不能被现有“文字相等”的检查漏掉；普通 RPC 更新也不能把相同正文的 token 和撤销历史清空。普通草稿文件的文字字段保持现有契约，标签是 GUI 编辑/展示信息。文件路径在重启及队列取回后仍保留原文；不从任意 `@文字` 猜测资源身份。取回队列文字时按原规则前置，同时平移当前草稿已有标签的范围，保留其原子编辑能力。

已有 `set_editor_text` 是外部纯文本替换：按当前协议替换正文，清除不再匹配的标签，附件仍由原 owner 管理。发送成功只清理本次提交的草稿修订和附件；失败保持原位，继续使用当前实现，不增加额外备份与恢复队列。

## 已确认交互方案

### 选择与编辑

保留现有命令面板作为选择入口，InputGroup 中的 Textarea 承接最终编辑。点击/Enter 选中 Skill 或模板后回输入框，标签包含开头命令及 Pi 所需的一个普通空格分隔符，选择与补空格是一次原子编辑，撤销一次即可恢复原稿；参数保持可编辑文本；选区、退格、Delete、剪切、复制、IME 和撤销由组件完成。

输入标签默认使用 InputToken 与现有图标，完整命令/来源可查看；点击只查看资源信息，不触发发送。普通文件使用相同 InputToken，点击通过系统打开文件；图片附件仍使用 AttachmentGroup。文件标签与必要的分隔空格是一次可撤销编辑；撤销恢复 `@` 查询时不自动抢走正文焦点。Tab 延续面板内补全。普通手输/粘贴保留可发送原文；候选选择通过可撤销编辑事务创建标签，会话草稿按上文恢复，不增加 Change 扫描转换循环。

Skill/模板候选模式只选择资源，不编辑另一段请求正文；参数只在 Composer 编辑。选择 `review` 后，原来的“检查这段代码”成为 `/skill:review 检查这段代码`。已有已识别开头命令时只换命令，保留参数与附件；未知命令不擅自删除。从正文触发时使用原输入范围与原文，不把候选搜索词或草稿副本再拼进去。应用/插件操作保留既有执行路径。

在同一 Command 面板用 `@` 或“添加文件”进入文件候选，显示文件名及相对路径，默认范围为当前会话 cwd；外部文件继续用系统文件选择器、粘贴和拖拽。选中后在正文选区插入文件路径标签，关闭面板并回到正文；若由正文里的 `@查询` 触发，用标签替换该片段，保留前后文字。`@` 选择的图片按文件引用处理，不自动读取图片或加入附件区。系统选择器、拖入和粘贴文件时，普通文件同样就地插入；识别的图片仍作为图片附件。文件候选在专用模式显示，复用当前命令面板外观。路径含换行等 InputToken 不支持的字符时，保留为可编辑的原始路径文字，不丢弃引用。Pi RPC 仍接收 `message` 与独立的 `images`，不能保证图片数据块与文字交错排列。

### 消息资源

支持能够从原始消息确定的范围：

- Skill 的名称/路径显示为资源标签，用户自己输入的参数正常显示；完整 Skill 指令默认收起、可展开查看。查看当次内容使用消息里保存的正文；“打开 Skill 文件”才读取磁盘当前文件，两者不能混淆。
- 完整文件引用行在消息内容投影中识别为资源项；已有明确文件 Markdown 链接可用 inline plugin 渲染。普通段落内的任意 `@文字`、代码块、未知命令保持原样。
- 无法还原身份的模板继续显示展开正文。整条消息的复制保留原始文本；资源自身的复制给出命令或路径，不把装饰性标签名替换进 RPC/复制内容。
- 输入 token 与 Markdown 原子节点是两个组件契约。不能直接复用一个控件充当另一个。收起的 Skill 正文仍属现有用户消息查找范围：命中后先展开对应正文，再使用同一文本投影原位定位/高亮；不能出现计数命中但页面无位置，也不能静默缩小搜索范围。实现必须验证隐藏到展开时的文本与布局更新。

本轮不引入新的消息格式或通用 mention 语法。

T-01 已验证：Registry 离屏解析后配置文件资源 plugin，渲染前后的 RenderedText 和匹配坐标保持一致。MarkdownNode 保留与普通链接相同的 text 及原始 markdown；完整 Skill 结构与文件引用行在应用投影处理，不吞掉指令正文。插件配置等价时上游保留已解析文档，无需额外缓存或挂载全部消息。

### 标准扩展问答

继续在当前会话输入区呈现待答内容；主/临时窗口复用，保持当前会话可定位、可取消。每个 RPC 请求独立处理，不拼接连续请求推断问卷。

| Pi 请求 | 组件与回复规则 |
| --- | --- |
| `select` | Questionnaire 的 Title、单选 Choices、Error、Actions 与 Submit，使用默认尺寸，先选中再明确提交；选项值映射回 Pi 原字符串，不增加 RPC 没提供的“其他”、多选或补充说明。控件内部可用请求内序号区别同文选项 |
| `confirm` | 保留语义直接的确认/否/取消按钮；分别发 `confirmed:true`、`confirmed:false`、`cancelled`，不将“否”当成取消，不为两按钮确认强造问卷 |
| `input` | 使用 QuestionnaireTitle、QuestionnaireInput、QuestionnaireActions 和 QuestionnaireSubmit，placeholder 来自 RPC；提交读取原始 InputState 值，不采用组件整理后的答案。组件将空值视为未作答，因此仅空字符串/纯空白使用应用提交按钮直接回复，保留与取消的区别 |
| `editor` | 继续使用多行 Textarea，保留 prefill、换行和现有提交快捷键；不降为 QuestionnaireInput 的单行输入 |

QuestionnaireState 随请求身份保留，不在 render 中重建；select 的选择状态由 PendingUi 持有，input 的原文由 PendingUi.text 持有，视图按当前请求建立 Questionnaire 适配状态，重建窗口时恢复输入。完成、取消、超时、断线后释放相应控件。来源与期限检查复用现有 `reply`。焦点转到当前待答控件，结束后回 Composer；不改变 #241 的待答提醒、计数或系统通知策略。

四类问答统一由 GroupBox::outline() 包裹，使用组件默认的细边框、主题圆角与 p_4 四周留白，不叠加旧 Composer 的背景或阴影。外层不重复标题，内部不再添加 py_4，以免纵向留白累加。select/input 使用 Questionnaire 自身的标题与间距；confirm/editor 使用标准 Form/Field，editor 保留 Textarea 的输入边框。动作区统一左侧取消、右侧提交或确认；输入从空白变为有内容时按钮位置不变。长选项允许换行，选项区独立滚动。

选项滚动区内部使用 p_1，为组件向外绘制的焦点环留出空间；配对的负 margin 保持选项与标题的原有对齐和间距。GroupBox 的外层留白不能替代滚动区内部留白，因为裁剪发生在滚动容器边缘。

焦点环回归覆盖滚动视口与首项的顶部、左右间距；受影响的问卷测试和构建通过。macOS 原生窗口已检查首项聚焦的完整轮廓，以及 24 项列表滚动到底后末项聚焦的底部轮廓，均无裁剪；测试实例与临时目录已清理。

## 实现与验证

没有新增需要用户决定的产品问题，也没有更改依赖来源。主要接入点：

- `composer_resources` 只识别命令身份和参数引用位置，不执行模板；原命令、参数和独立图片仍交给 Pi。模板文件检查在后台执行；不可读、未采用文件参数、未闭合引号时停止此次发送并保留草稿/附件。模板源码使用 get_commands 提供的路径，Pi RPC 本身不返回展开预览。
- `CommandPalette` 复用列表与选择状态，资源和文件模式不会把查询作为消息发送。项目文件后台发现后在当前目录内匹配，遵循 ignore/隐藏文件规则，不跟随符号链接；每次面板展示前 200 个匹配项，继续输入可缩小范围。文件候选的元素身份使用完整路径，同名文件经过筛选占据原位置时不会复用另一条候选的提示状态；其他命令使用操作身份，不依赖翻译后的标题。
- `InputContent` 替换内存字符串草稿；无额外 token 表或磁盘映射。选择和分隔符合为一次可撤销编辑，删除标签后撤销不会重新打开面板。插件纯文本替换清除标签。
- `ResourceLinks` 只呈现完整路径引用及明确本地文件链接；识别输入标签生成的连续引号片段，路径外的空格分隔符保留在原文中，不参与 AST 范围检查或文件 URL。路径内的空格、引号保留，代码及普通叙述不转换。Skill 指令复用现有折叠与查找链路，发送内容和原文复制不变。
- `PendingUi` 持有 select 的 QuestionnaireState，选项使用请求内序号作为控件值，提交映射回 Pi 原字符串。使用 Questionnaire / Item / Title / Choices / Choice / Error / Actions / Submit，单行问答复用 QuestionnaireInput；只订阅 Submit 事件，不因 Completed 再次回复。取消与协议允许的空白提交使用标准 Button。editor 继续 Enter 提交、Shift+Enter 换行。
- QuestionnaireSubmit 使用组件默认按钮名称和无障碍名称。0.7.0 的问卷按钮文案提供英文、简中与繁中，其余应用语言沿用组件的英文回退；应用的取消和空白提交等按钮继续使用现有 Fluent 翻译。

已完成的关键回归：资源选择不发送、Tab 后继续编辑查询不误发送；原子删除/撤销/重做与中文/emoji 草稿保留；项目文件候选及取消、原触发范围替换、原位顺序与撤销；模板位置参数、全参数、切片、路径引号/换行与失败保留；Questionnaire 单选、重复标签、显式提交及窗口重建；input 空值/空白、editor 多行原值；Skill 收起正文搜索展开及整条原文复制；离屏文件链接的匹配坐标保持。

原生隔离检查使用当前构建、独立配置和临时项目，真实 Pi 的测试扩展拦截普通输入，不调用模型。已核对 Skill 标签、选择后保留草稿、文件引用选择、Skill 指令查找展开及原位高亮、真实 select 的先选后提交、24 项列表滚动、长选项换行、必答提示、键盘换项及带名称的提交/取消操作。

当前 Gupi 检查：`cargo test -p gupi --locked` 295 项通过、2 项既有忽略；`cargo clippy -p gupi --all-targets --all-features --locked -- -D warnings`、构建与格式检查通过。macOS 原生窗口走通 select → confirm → input → editor，确认单行首尾空格和多行换行原样回传；检查默认组件尺寸、长选项换行和独立滚动，以及取消/提交按钮位置。复测了单行输入清空后 Enter 提交空字符串，和输入有内容时标准 Submit 仍位于右侧。统一 GroupBox outline 后，在原生窗口核对四类问答的容器边框、四周留白、按钮对齐和长列表滚动。使用隔离测试扩展，没有调用模型；测试进程与临时目录已清理。

文件内联改造验证：会话测试 86 项、资源引用测试 3 项、队列合并测试 2 项通过；`cargo clippy -p gupi --locked --all-targets --all-features -- -D warnings`、构建与格式检查通过。真实 macOS 隔离窗口验证了中文文件名标签、正文与多个引用的排列、`@` 图片路径保持为引用、系统选择器添加同一图片为附件，以及整项删除、撤销/重做且不重新弹出候选。测试没有调用模型。

目录引用验证：回归测试覆盖目录候选、图片扩展名目录、文件/目录/图片混合粘贴的顺序与 RPC 图片分流；原生窗口确认 `sample-file.txt` 和 `sample-folder.png` 分别显示文件与文件夹图标，选择后目录保留为输入框内的文件夹标签。隔离测试进程与临时目录已清理。同名候选身份与路径序列化回归先在旧实现失败、修复后通过；覆盖标签分隔空格、文件名内空格与组合引号、换行及代码排除。真实 Markdown 组件测试确认带引号的路径生成资源按钮，且离屏/渲染后的查找文本与定位坐标一致。

已有正式依赖的中文混排换行、长文定位无障碍滚动问题仍见统一待处理文档，不因本轮新增资源展示重复创建任务。Windows 原生与真实中文输入法组词不在本机自动化验证结果内。

## 关联入口

- [统一待处理文档](../follow-ups.md)、[RPC 边界](../pi-rpc-gaps.md)。
- [命令面板](../issue-226/command-palette.md)、[现有扩展 UI 与体验入口](../issue-222/README.md)、[正文查找](../issue-242/README.md)。
- Pi 正式源码：[Skill/模板调度与 Skill 结构](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/agent-session.ts)、[模板参数替换](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/prompt-templates.ts)、[RPC 实现](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/modes/rpc/rpc-mode.ts)。
- Zed 固定版本源码：[候选选择](https://github.com/zed-industries/zed/blob/244023605536a412ab6b8d5b658466b89fb15401/crates/agent_ui/src/completion_provider.rs)、[输入、粘贴与资源恢复](https://github.com/zed-industries/zed/blob/244023605536a412ab6b8d5b658466b89fb15401/crates/agent_ui/src/message_editor.rs)、[用户消息复用编辑器](https://github.com/zed-industries/zed/blob/244023605536a412ab6b8d5b658466b89fb15401/crates/agent_ui/src/entry_view_state.rs)、[资源标签与点击](https://github.com/zed-industries/zed/blob/244023605536a412ab6b8d5b658466b89fb15401/crates/agent_ui/src/ui/mention_crease.rs)。
