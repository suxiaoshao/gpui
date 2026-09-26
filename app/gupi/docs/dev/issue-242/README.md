# 会话阅读、正文查找与信息查看

归属 [#242](https://github.com/suxiaoshao/gpui/issues/242)，父 Issue #217。插件持久消息与会话信息弹窗已实现并通过受影响回归；正文查找尚未实现，留待接入包含范围高亮、定位接口的兼容正式包。当前依赖保持不变，不提前实施正文搜索或修改其快捷键。

2026-09-26 对照 Gupi 主 Issue 基线 `ab5a19ae`、Pi v0.87.1 契约，以及上游合并提交 `aa2c3f77`（范围高亮）/ `80230b65`（范围定位）复核。当前 workspace 仍锁定 GPUI Kit 0.6.4；正式包的发布、API 与依赖兼容性须在执行依赖升级时核实，不因本计划采用已发布的假设就视为已经完成升级或集成验收。

## 目标与范围

合并完成三个面向当前会话的能力：

1. 插件 `custom_message display:true` 在聊天正文中正确展示，支持文字、图片、复制和已有图片预览；`display:false` 保持正文隐藏。
2. 会话信息弹窗集中展示已有身份、目录、文件、模型/思考等级、Token 与费用。
3. 当前查看分支正文查找，提供匹配导航、定位与高亮。用户已确认：按每轮对话，有最终输出只查最终输出；没有最终输出时只查 assistant 文字正文。用户消息、思考、工具内容、压缩/分支摘要及插件消息不纳入搜索。此决定取代立项时“包含可展示插件消息”的初始范围。

主窗口与临时窗口共用业务；不扩展跨会话全文索引、不新增数据库、不重新实现插件 TUI renderer。通知由 #241 承接；输入原子 Token、Markdown 资源标签和 Questionnaire 由 #243 承接；Pi/项目设置另归 #244。信息弹窗只读，不变成第二个设置页。

## 当前实现与剩余接入

### 插件消息：统一使用持久条目

Pi 存在两条正式路径：

- `sendCustomMessage` 产生 `role:"custom"` 消息，最终通过 `message_start/message_end` 发送；它保存的历史条目为 `type:"custom_message"`。
- 0.87 扩展边界也可以直接提交 `custom_message` 条目，经 `entry_appended` 告知客户端。

`DisplayMessage::from_entry` 接受 `custom_message display:true` 并保留 entry ID、customType、content 和 details；`display:false` 只在正文投影中隐藏。custom 的实时事件触发来源会话的历史回读，不进入普通消息 signature 合并，也不打断正在更新的 assistant 流。正文仍按完整 Run 关联工具和判定最终回答；展示时分为过程、插件消息和回答片段，插件消息位于折叠过程之外，按原顺序呈现 Markdown 与图片，复用复制和图片预览。

Pi 自定义消息保存时间由 appendCustomMessageEntry 新建，事件 timestamp 来自消息创建时刻，可能不同；历史 entry id 也不在普通 message 事件中。不能简单给历史 custom 补上 role 后假定现有 role+timestamp 的 signature 一定能去重。若同内容消息确实出现两次，也不能按文本去重吞掉一条。

### 信息弹窗：读取现有会话数据

- `Session.info` 已有 ID、名称/标题、cwd、文件路径和 parent_session；运行状态已保留当前模型和思考等级。
- `Session.stats` 已按事件定向读取 `get_session_stats`，输入区已有 Token/context/费用呈现；不需要为打开弹窗增加常驻轮询。
- Pi 的 stats 返回 sessionFile/sessionId、userMessages/assistantMessages/toolCalls/toolResults/totalMessages，Rust `SessionStats` 已补充这些可选字段，信息弹窗复用现有读取结果，不新增 RPC。
- **统计口径**：Pi `getSessionStats()` 遍历当前会话文件的全部 entries，包含各分支累计用量及结构条目的 usage；不是当前预览分支的可见消息计数。custom_message 不计入该实现的普通 message 数量。contextUsage 则来自当前执行上下文。弹窗不能把两者误标成“当前预览分支”。
- Pi TUI `/session` 还有模型费用分布、缓存预热/浪费等扩展统计。本 Issue 不为了照搬 TUI 再造统计计算；先显示当前已可取得的数据。

### 正文查找：组件契约已明确，应用接入尚未实现

- `Session.messages(preview)` 已统一当前执行分支/其他分支预览和 live 内容；`HomeView::sync_messages` 按 content_revision 投影 ChatRow，持有稳定行 ID 与位置映射。
- `ChatRow::reveal`、`MessageScrollerState::scroll_to_item` 已能展开必要过程/工具分组并定位到消息行。历史 `preview_node` 会改变预览分支，不能直接把它当搜索跳转来用。
- 工具与压缩摘要的全文在 #238 的 DetailsView 中；它们已明确排除在本次搜索之外，不增加详情弹窗内搜索或搜索触发弹窗。
- 临时窗口的 `InputEvent::Change` 已直接触发会话过滤；`ToggleInputFocus` 已实现搜索框与正文输入框之间的 Tab 切换，已有页面回归覆盖。当前 `secondary-f → FocusSearch` 只是额外聚焦入口，不负责开启搜索。用户确认无需保留这项重复入口；#242 移除旧绑定和搜索框上的对应键帽，Cmd/Ctrl+F 统一用于正文查找。

## 实现方案

### 1. 统一插件消息投影

- 在会话内容投影层统一判断 `display == true`，保留原始 content 块顺序、customType、details 和历史 entry 身份。隐藏只作用于正文/正文搜索，不删除 Pi 原始条目或现有“全部”历史数据。
- 插件消息作为时间顺序中的独立 Message，使用现有 MessageContent/图片预览/复制组件。用 customType 标识消息类别；RPC 没有可靠的插件显示名映射，不能把它伪称为安装包名称。不把插件内容当成用户输入或最终回答。
- 默认展开可展示正文；details 元数据保持原数据，不主动新增 JSON 详情面板、气泡或复制元数据功能。最后回答复制/临时回填仍只取 assistant 的最终回答。
- 会话是否为空的业务判定继续沿用当前 `empty_conversation`（已计入 custom_message）；不要因为 display:false 从正文过滤就改变会话复用、删除或持久化行为。
- 以 Pi entries 为持久消息的权威来源：custom message_end 与 entry_appended 驱动来源会话已有的合并历史回读，以 entry id 区分每条消息。custom 实时消息不进入普通 live 消息的 role + timestamp/text signature 合并路径；不另建按内容或时间猜身份的插件消息缓存。普通 assistant 增量链路保持原语义。回读按下述范围拆分，不再因普通输出增量丢弃有效持久条目。
- 插件消息在完整 Run 内作为独立 Message 呈现，按原 content 数组顺序组合现有 MessageContent、Markdown 和图片预览。当前用户消息渲染先集中图片、再合并文本，不能直接复用该排序来呈现交错的插件文字/图片。
- 独立展示不能改变 assistant 轮次语义。当前 `project_rows` 只向最后一个 Run 追加消息，`RunContent::project` 在传入消息中关联工具调用与结果；直接插入插件行可能截断关联。投影时保留原轮次和工具关联，最后回答选择及搜索范围也不能因插件行而改变；不要以新增的展示分段推断新的用户轮次。
- 避免在每个自定义 message_start/message_end 都重复刷新；使用现有请求合并与 history/content revision。只更新所属 session 和改变的消息行，不扫描目录或重连其他实例。

#### 运行中回读的已知接缝

`read_session` 在请求开始记录 binding、event_revision、model_revision 和历史 revision。响应期间发生普通增量时，仅在绑定有效且历史未被更新的情况下接纳 entries，不应用已过期的 get_state，也不清理较新的 live、工具和运行状态。已有排队的历史回读继续执行，最终 settled 回读统一整理运行状态；失败仍保留原内容并沿用现有重试。

`entry_appended` 自带完整 SessionEntry，但普通 custom message_end 没有对应 entry ID，前者到达时父链也可能尚未读取。因此两条事件都复用已有合并回读，不按内容或时间猜身份，不另建插件消息缓存。Pi 新会话只有 custom 消息时尚未创建会话文件，展示仍使用内存中的权威 entries；会话文件状态按实际存在性呈现。

### 2. 会话信息弹窗

从现有会话更多菜单、会话右键菜单与命令面板进入；临时窗口通过其操作面板提供同一入口，不额外加一排标题栏按钮，不新增默认快捷键。

| 分组 | 显示与来源 |
| --- | --- |
| 身份与位置 | 当前会话标题、Pi ID、cwd、实际会话文件；复制 ID/路径，已有文件可显示位置 |
| 运行配置 | 当前执行模型（含 provider）及思考等级，来源为已有 state；历史预览时不冒充预览节点的模型 |
| 用量 | Pi 累计输入/输出、缓存读写、费用；已有 contextUsage 单独标明为当前执行上下文 |
| 消息统计 | 反序列化 Pi 已返回的消息/工具计数，沿用其会话范围，不按当前可见气泡数推算 |

临时会话显示“未保存”的语义，工作目录仍可查看；文件未创建时不提供无效的打开文件操作。未取得数据展示未知/读取状态而非 0。优先使用已有缓存和读取生命周期，运行中的会话按已有来源事件更新弹窗字段，不新增后台轮询。单纯查看已有信息不启动闲置会话或切换模型。

Dialog 拦截已有上下文切换动作；不为打不开的操作设计额外的跨会话快照/兼容层。关闭只是结束查看，不影响运行、草稿或统计源。低频信息以普通分组字段呈现，路径可选择/复制；复用组件默认样式及现有复制反馈。

### 3. 查找范围、匹配与定位

搜索内容范围、原位置高亮目标与快捷键分工已经确认。原位高亮按现有组件可用能力分步接入，不改成结果摘要高亮。

- 在当前会话区域提供查找栏，含查询输入、匹配计数、上一处/下一处和关闭。首轮使用普通文本匹配，不增加替换、正则和全局索引；大小写不敏感，跨 Unicode 字符匹配保留正确的原始范围。
- 仅在当前查看分支内按每轮选取搜索文本：已有最终输出时只取最终输出，尚无最终输出时取 assistant 的文字正文；按消息/内容块顺序导航。沿用现有最终回答判定，不按“最后一条消息”或文本内容另造推断。用户消息、thinking、工具参数/输出、压缩/分支摘要、custom_message、图片、details 及内部元数据均不纳入。
- 搜索直接使用 `TextViewState::rendered_text()` 的实际阅读文字，例如 `hello **world**` 可以按 `hello world` 匹配。结果保存该文本中的 UTF-8 字节区间；大小写不敏感匹配必须返回原文本的区间，不把小写转换后的位置直接用于原字符串。复用同一组件的解析与映射，不另写 Markdown 去标记或源码到显示位置转换器。
- 命中最终输出直接定位；没有最终输出时，若命中的 assistant 正文位于过程区，只展开必要过程容器。复用行定位，不改变执行 leaf、不切换分支、不打开工具或摘要详情。
- 查询、当前匹配和结果属于视图的短期状态，不存入 Pi 或 state.toml，不复制一份会话正文。关闭或切换会话/预览分支时清理本次查找，原有滚动和折叠状态继续按既有规则；不追加自动恢复旧滚动/折叠的备份系统。
- 初次打开从已有内容计算；正文变化只使对应会话受影响文本失效。追加输出更新匹配数量；最终输出出现后按已确认规则收敛到最终输出，移除不再属于搜索范围的过程正文匹配，保留仍有效的当前目标，不自动跳到新匹配、抢焦点或强制跟随底部。关闭查找后停止维护结果，不为每个 token 重扫所有 session。
- 查找栏 Enter/Shift+Enter 导航、Esc 关闭并恢复焦点；结果按消息/内容单元和字节位置排序，上一处/下一处循环导航，零结果时禁用导航。首轮采用大小写不敏感的字面匹配，不增加去重音、正则或整词模式。有 Dialog/图片预览时先由现有最上层处理，不绑定全局键盘钩子。

## 已确认的上游契约与接入前提

[#3215](https://github.com/longbridge/gpui-kit/pull/3215) 与 [#3216](https://github.com/longbridge/gpui-kit/pull/3216) 已提供本轮需要的公共接口；范围坐标与方法签名不再是待确定项。执行前核实所选正式包确实包含这些 API，并对 workspace 共用的 kit/component/assets 依赖一起检查兼容性；不直接切 Git main、vendor 或修改 Cargo registry。

| API | 契约与 Gupi 用法 |
| --- | --- |
| `TextViewState::rendered_text() -> RenderedText` | 最近一次已提交解析的阅读文字；按 UTF-8 字节取范围，跨加粗/链接等样式搜索。新提交的 Markdown 尚未解析时不在快照中。快照相等性包含 view 与解析版本，可用来跳过未变正文 |
| `RangeHighlight::new(range, background)`、`set_range_highlights(highlights, cx)` | 在原文字布局上绘制背景；普通命中和当前命中用主题语义色区分，保留语法色、选择与复制。匹配范围应基于当前快照计算，并在同一次状态更新中设置；异步得到的结果先核对快照 |
| `clear_range_highlights(cx)` | 清除查找高亮，不修改消息原文、用户选择或复制文本 |
| `reveal_range(range, cx)` | 将命中起点所在行滚入视口。非独立滚动的 TextView 会交给最近的外层 `gpui::list`，前提是对应消息行已经参与布局 |
| `TextView::on_reveal(handler)` | 自定义滚动容器可接收命中行的窗口坐标。Gupi 当前 MessageScroller 使用 `gpui::list`，先使用原生传播，不预先新增手算滚动偏移的适配层 |

搜索范围、查询、大小写匹配、计数、当前命中与上下处导航由 Gupi 管理；组件负责渲染文本范围、高亮和布局内定位。旧的 `<mark>` 注入、替换整个段落为 InlineElement、覆盖代码块 highlighter 等方案不再采用，也不保留为后续实施步骤。

### 状态归属与离屏消息

当前 `MarkdownState` 只在 `Markdown::render` 的 `window.use_keyed_state` 中按需创建。只向已渲染组件询问文本会漏掉离屏消息和折叠过程中的 assistant 正文；需要把可搜索文本的组件状态变为应用可取得的既有消息呈现状态。

- `ConversationState` / Session 继续持有 Pi 消息、历史和实时内容；查询与高亮不写入这一层，不新建正文数据库或第二份会话记录。
- 对可搜索 assistant 文本，由现有会话视图按稳定消息/内容单元 ID 保留 `TextViewState`，Markdown 包装器与查找共用同一实体。用内容单元身份区分最终回答与过程文本，不用行号作为身份；其他非搜索内容不必一起改造。
- 平时继续按显示需要创建；打开查找时，为当前查看分支全部符合规则的文本补齐组件状态并解析，离屏内容不需要创建或挂载整行 UI。仅保存组件实体、必要的快照和命中区间，不再运行第二套 Markdown 解析器。
- 文本同步仍沿用追加 `push_str`、替换 `set_text`。搜索打开期间只处理正文或搜索范围发生变化的文本；最终输出出现后按已确认规则收敛范围。组件提交解析后的通知逐步更新结果与计数，不等待所有离屏文本解析结束才提供已知命中，也不宣称 `set_text` 返回时已经取得完整新快照。只按已提交快照匹配，不用固定延时猜解析进度。
- 查找关闭时清理查询、结果、待定位目标与高亮，停止搜索订阅；为搜索额外创建而未被消息呈现使用的状态可释放。正常呈现状态随所属会话视图管理，消息删除、预览切换或视图释放时清除失效项，不做跨会话长期搜索缓存。

### 避免高亮引起额外刷新

`set_range_highlights` 会通知 TextView；当前 Markdown 观察者收到任何通知都会重测所属消息行，因此接入时必须区分原因。

1. 比较 `RenderedText` 与上一次解析快照；正文未改变时不重搜、不重新测量消息行。
2. 正文解析改变时只更新该文本的命中，并继续对所属消息行执行已有重测；宽度、字体和主题变化保留各自的布局失效路径，不被搜索判断吞掉。
3. 上一处/下一处只调整旧、新命中的高亮和定位；不改 Markdown 字符串、不触发 Pi 回读、不刷新目录。切换高亮产生的通知不能再次发起搜索，避免反馈循环。

### 定位顺序与流式行为

1. 命中包含所属消息/内容单元、当前解析快照和 UTF-8 范围。沿用会话行位置映射查到目标行；不能调用会改变预览分支的 `preview_node`。
2. 最终输出通常已在过程区之外；没有最终输出时，命中可能在 assistant 过程文字中，只展开该文字所需容器。按同一投影中所有 assistant 文字单元筛选，不能仅搜索 `RunContent.answer_text` 而遗漏此前过程正文。
3. 对离屏目标先调用 `MessageScrollerState::scroll_to_item` 使消息行参与布局，再由目标 Markdown 呈现生命周期消费一次待定位目标并调用 `reveal_range`。新的导航替换旧目标；关闭查找、切换会话/分支取消目标，不添加固定时长轮询或无限重试。
4. 查询改变或显式导航才滚动。流式追加只更新对应文本和计数，不自动跳到新命中、不调用 `scroll_to_end`，不抢焦点。范围/内容改变时核对当前命中，失效则重新匹配，不把旧字节偏移用于新正文。
5. “最终输出优先，否则 assistant 文字”的筛选与现有呈现使用同一最终回答规则；暂定 answer 槽位并不代表全部待搜索正文只有那一段。用纯投影回归覆盖还在运行、已完成和中断的不同情况。

### 已知组件边界

- `reveal_range` 的成功返回表示范围合法且请求被接受，没有滚动完成回调；请求约一秒内未完成会被丢弃，只保留最新请求。普通查找按这一契约调用，不为缺少确认回调另造持久任务或将其列为新的上游前置条件。
- 定位保证的是起点所在行可见，不保证整个跨行匹配同时可见。横向滚动表格的自动横向定位暂未覆盖。
- 普通 Markdown 段落、标题、代码块和表格单元格有文字范围高亮；分隔符、HTML/自定义块及自定义内联对象不提供相同的逐字绘制。保留现有呈现，不替换内容来模拟高亮。若接入时常见内容因这些边界不能满足原位查找，按下文记录具体复现和影响，再决定该内容的处理，不提前扩大为自定义渲染器。

这些是已知契约和集成验证点，不代表需要用户重新确认搜索产品规则。通用 API 的源码适配已核对；Gupi 的实际排版、长消息定位和交互结果仍须在实现后验证。

## 已确认的搜索范围

D1 已确认：有最终输出只查最终输出，没有最终输出只查 assistant 文字正文。插件消息展示仍属于本 Issue 的独立功能，但不进入搜索范围。

D2 已确认：必须原位置高亮，不采用结果摘要高亮作为替代。使用 #3215 / #3216 的范围 API；搜索逻辑由应用实现。

## 已确认的快捷键分工与研究结论

D3 已确认：临时会话搜索框获得焦点后直接输入即过滤，不需要 Cmd/Ctrl+F 激活。Tab 继续在原有搜索框与正文输入框间切换；不按焦点让同一个 Cmd/Ctrl+F 承担两种搜索。

主窗口、临时窗口的 Cmd/Ctrl+F 统一打开/聚焦当前会话正文查找，操作面板提供同一动作入口。实施时移除 `gupi_temporary::FocusSearch` 的旧绑定、专属 action handler 和搜索框右侧键帽；保留 `focus_search` 方法供窗口显示、Tab 等既有流程调用。普通搜索的 Change 订阅和过滤实现无需重做。

`toggle_focus` 当前只在搜索框或普通 composer 真正有焦点时切换，其余输入和扩展表单继续走正常 Tab 导航；新正文查找输入也应保持自身正常焦点顺序。沿用 Dialog、图片预览和操作面板的已有作用域，不另加全局键盘钩子或上下文切换兼容层。

依据：`features/temporary.rs::init/new/toggle_focus` 及现有 `temporary_page_tab_search_and_recreation_preserve_the_draft` 回归。该测试已包含搜索输入 → Tab 正文输入 → Tab 返回搜索；本轮只读取源码和测试，没有重跑或原生交互验收。

## 待确定问题与开发中记录

当前没有需要用户先决定才能开发的产品问题。搜索范围、键位、信息弹窗内容和插件消息呈现方向均已确认；上游范围坐标、快照语义和方法签名已经明确，不再列作待定。

正式包是否含所需接口、离屏状态接入、解析通知与高亮重测分离、custom 消息权威条目同步，属于执行前核实或实现/回归工作，不因尚未编写代码就反复询问用户。

开发中若发现会改变已确认搜索范围、交互行为、数据权威来源或引入新依赖策略的问题，在本节记录具体场景、源码/复现证据、受影响步骤、建议处理及需要用户决定的点；未受影响的步骤继续。普通实现选择直接解决并更新相应方案，不预填假设性待办。目前没有新增条目。

## 实施顺序与归属

1. 依赖核实：在所选兼容正式包中确认 `rendered_text`、范围高亮、`reveal_range` 及组件 re-export，更新匹配的 workspace 依赖和 lockfile，并完成受影响构建。这是本计划的执行前提，不在文档修订阶段升级。
2. 插件消息：已接入历史/实时统一投影与 display 判断，在完整 Run 内接入独立 Message，覆盖事件转历史的身份、顺序与时序；更新必要历史定位/最后回答回归。
3. 信息弹窗：已补充 pi-rpc 的既有 stats 响应字段映射，复用 Session 数据、菜单/面板动作、字段与复制入口。
4. 正文查找：接通共用的 Markdown 状态与当前分支匹配，区分解析/高亮通知，再完成查找栏、原位高亮、离屏行及长段落定位和流式更新。主窗口与临时窗口复用同一路径，按 D3 修改快捷键。
5. 交付验证：完成下列关键回归、受影响构建/Clippy 和必要原生检查，交付试用；不把方案完成或依赖升级当作功能已经完成。插件消息和信息弹窗本身不依赖搜索 API，可独立实施。

代码归属仍为应用的 state / features/home；pi-rpc 只负责真实协议字段。只有通用 TextView 能力才属于上游组件，不把 Gupi 的会话/工具语义塞进基础组件。实现时按现有代码拆分局部模块，不提前建立跨应用 search crate。

## 必要验证

- 插件：实时消息与 entry_appended 两种路径、空闲不触发模型、运行中追加、恢复与其他分支预览、display true/false、交错文字/图片顺序、相同内容但不同 entry id 的消息各显示一次；覆盖插件消息插在工具调用、工具结果和最终回答之间，关联和搜索轮次不被切断，不把 custom 当最后 assistant 回答。
- 信息：Pi stats fixture 的真实字段、全部分支累计口径、未知值、未落盘/临时会话、数据更新、复制/打开位置；不增加多实例轮询或目录扫描。
- 查找：先验证原位高亮前后排版、换行、语法样式、链接与选择复制一致；覆盖有最终输出/无最终输出的范围选择、最终输出出现后的匹配收敛、非 assistant 内容排除、Unicode/大小写、Markdown 样式边界、回答中的代码/表格、重复匹配、过程正文定位、预览分支切换及流式追加；确认只更新所属会话。
- 增量与性能回归：离屏及折叠的合格正文均可命中；纯高亮/当前命中变化不重新解析或测量行，不引起循环通知；正文变化只更新所属文本，查找关闭后不再维护结果。等待解析和待定位过程中切换会话，旧目标不能滚动新会话。
- 原生界面：在现有最小主窗口和固定临时窗口检查焦点、Tab 双向切换、Cmd/Ctrl+F 统一正文查找且不再聚焦会话搜索、Dialog 关闭/复制、长路径与长消息定位。只有相关实现完成后进行受影响回归与必要构建，不把 #223 的完整发行验收搬到本 Issue。

## 本轮验证记录

- `cargo test -p gupi --locked`：259 项通过；覆盖 custom 两种事件触发、流式回读不覆盖运行/队列/模型、较新分支保护、重复条目身份、工具关联和分段定位、原始块顺序、复制/图片预览，以及临时操作面板打开信息、来源更新、复制和 Esc 关闭。思考等级改为复用已有翻译后，信息弹窗定向回归再次通过。
- `cargo test -p pi-rpc --locked`：12 项单元、13 项进程集成和 1 项文档示例通过；既有两项外部真实进程测试维持忽略，未将它们计为通过。
- 构建、受影响 crate 的 Clippy（含测试 target，`-D warnings`）、格式与差异检查通过。沿用依赖自身 `block 0.1.6` 的 future-incompatibility 提示，本轮未改依赖。
- 真实 Pi 隔离 RPC：`/gupi-ui custom-message` 生成三个不同 ID 的条目，display 为 true/false/true；两条可见消息保留 text/image/text 顺序，未启动模型 turn，新会话文件未提前创建。可通过 `script/gupi-ui-gallery` 重现此场景。
- macOS 原生检查已确认两条可见插件消息、图片预览及 Escape 关闭、会话菜单与命令面板入口、信息弹窗长路径换行、未保存状态、底部统计滚动、复制反馈及 Esc 关闭。临时窗口入口及动态复制由 GPUI 窗口交互回归覆盖；其他平台尚未原生目视检查。

本轮没有新增需要用户决定的问题。正文搜索、其快捷键调整和依赖升级仍未执行，以上验证不代表整个 #242 已完成。

## 源码入口

- Gupi：[历史投影](../../../src/state/history.rs)、[实时消息](../../../src/state/conversation/messages.rs)、[事件与回读](../../../src/state/conversation.rs)、[消息行](../../../src/features/home/messages.rs)、[过程投影](../../../src/features/home/messages/activity.rs)、[Markdown 增量呈现](../../../src/features/home/messages/markdown.rs)、[详情 Dialog](../../../src/features/home/messages/details.rs)、[消息列表与分支定位](../../../src/features/home.rs)、[现有统计呈现](../../../src/features/home/composer/metrics.rs)、[临时窗口键位](../../../src/features/temporary.rs)、[RPC 响应类型](../../../../../crates/pi-rpc/src/protocol.rs)。
- Pi v0.87.1：[AgentSession / sendCustomMessage / getSessionStats](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/agent-session.ts)、[SessionManager](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/core/session-manager.ts)、[TUI 通用插件消息](https://github.com/earendil-works/pi/blob/f07218c4d4bbc12bef056a7058c3dd49dfe41abe/packages/coding-agent/src/modes/interactive/components/custom-message.ts)。
- 组件正式源码：[0.6.4 TextView](https://docs.rs/crate/gpui-base/0.6.4/source/src/text/text_view.rs)、[TextViewState](https://docs.rs/crate/gpui-base/0.6.4/source/src/text/state.rs)、[0.6.6 TextView](https://docs.rs/crate/gpui-base/0.6.6/source/src/text/text_view.rs)、[Markdown 扩展](https://docs.rs/crate/gpui-base/0.6.6/source/src/text/markdown_ext.rs)。

- 已确认的组件 API：[TextViewState 契约](https://github.com/longbridge/gpui-kit/blob/80230b652dbc573b5811f46258c36cd184887177/crates/base/src/text/state.rs#L572)、[RenderedText / RangeHighlight](https://github.com/longbridge/gpui-kit/blob/80230b652dbc573b5811f46258c36cd184887177/crates/base/src/text/range_highlight.rs)、[官方 Markdown 查找示例](https://github.com/longbridge/gpui-kit/blob/80230b652dbc573b5811f46258c36cd184887177/examples/markdown/src/main.rs#L1263)。
