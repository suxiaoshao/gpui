# 输入框中的 Skill、模板与文件引用

阶段：共用输入外壳与粘贴接线纳入依赖升级；资源标签及相关交互继续等待上游正式版本，尚未实施。2026-09-20 更新。归属 #221 的输入能力，与 #226 命令入口联动；统一入口见[总待处理文档](../../../../../docs/dev/issue-217/follow-ups.md)。

## 当前决定：先复用已发布控件，资源交互继续等待

按用户 2026-09-20 要求，依赖升级已使用正式版 0.6.4 的 InputGroup 替换共用输入外壳，并已采用 on_paste 简化附件粘贴接线，详见[依赖更新接入计划](../../../../../docs/dev/dependency-refresh-2026-09/README.md#changelog-对照接入上游能力并删除重复实现)。Skill 填入正文、Skill/模板标签、模板附带文件引用及可选 `@` 文件入口仍沿用此前统一等待正式组件的决定；不自行改用 Git main。现有草稿、模型和附件业务归属保留。

| 上游能力 | Gupi 接入职责 |
| --- | --- |
| [Questionnaire #2878](https://github.com/longbridge/gpui-kit/pull/2878) | 为标准扩展问答提供统一宿主组件，与输入框改造同批恢复；Pi 仍只有原有标准 UI 协议，不自行推断多选/多题 schema。 |
| [InputGroup #3042](https://github.com/longbridge/gpui-kit/pull/3042) | 用上游组合输入容器承接主输入框与模板任务编辑器的共用外壳、附加控件和底部布局，保留现有草稿、模型选择和发送归属。 |
| [原子内联标签 #3113](https://github.com/longbridge/gpui-kit/pull/3113)（对应 [#3110](https://github.com/longbridge/gpui-kit/issues/3110)） | 在正文编辑层提供 Skill/模板标签、点击回调和可定制展示，以及原子编辑、历史和布局能力；资源身份与 Pi 命令语义仍由 Gupi 决定。 |

- 暂停应用侧自定义 token 编辑器路线，不移植 Jaco 的正文编辑内核，也不以拦截退格等局部处理代替完整能力。下文 Jaco/Zed 调研保留为行为参考。
- InputGroup 接入保留现有 TextareaState，替换 Composer 手写外壳；后续原子标签发布接入后，上游再承担对应文本范围、内联布局、整体编辑、IME、剪贴板和撤销重做；Gupi 仍负责资源查询、补全候选、Pi 命令语法及发送。
- 恢复时核对实际发布且可接入的 API：标签图标/文本与换行布局、光标边界、整体删除/替换/剪切、撤销恢复标签身份、Unicode/IME、禁用/只读以及复制/发送表示。仅有高亮、只读 TextView 内联内容或 tracked decorations 不等于满足这些要求。
- 原子标签与 Questionnaire 合并后仍需等待正式发布，不提前改用 Git/main 或本地补丁依赖；它们发布并与 InputGroup 兼容后再恢复其余输入资源交互。已发布的外壳替换不受这一等待限制。
- “Skill 选择后填入正文、不直接发送”的产品决定保留，当前仍未实现；按用户决定与其他输入交互一起暂缓，不单独提前实施。下文待确定项在恢复这项工作时再讨论。

2026-09-19 核对：三项均已合并，但正式版 v0.6.4 只包含其中的 InputGroup，尚不包含 Questionnaire 与原子内联标签；除已纳入升级的 InputGroup 外壳与粘贴接线外，其余交互继续等待兼容正式版本。版本证据与完整 RPC/TUI 待处理盘点统一见[总待处理文档](../../../../../docs/dev/issue-217/follow-ups.md)。

## 已确认方向

- Skill 和命令模板显示在正文开头的命令位置，可作为整体删除；后续文字是参数或补充要求。
- Pi Skill 调用为 `/skill:名称 参数`，命令模板为 `/模板名称 参数`，没有固定的 `/prompt:` 前缀。
- 普通文件与图片放在同一个附件区：文件图标和文件名、图片缩略图，均可独立移除。
- `@` 可作为文件搜索和添加入口，选中后进入同一个附件区；本阶段先调研，不自动扩大为完整文件搜索实施任务。
- 用户本次确认：选择 Skill 后应填入对话输入框供继续编辑，不能立即发送。这取代旧命令面板对 Skill 选中后直接执行的规则。
- 普通文件继续发送路径引用，图片通过 RPC `images` 传递，不默认读取文件全文。
- 模板展开后必须保留附带的文件引用，避免无参数模板忽略追加路径。

## 协议边界

Pi 在消息开头识别 Skill 和模板；它们不是可在正文任意位置放置多个的通用 mention。Pi CLI 的 `pi @文件` 会读取文件内容，但 RPC `prompt.message` 中的 `@路径` 只是文本引用。可视标签与附件区域不应改变这些协议语义。

## 源码调研

2026-09-17 核对本地源码：Zed `ba7da93e5c`、Pi `71dca871b`；Jaco 与 Gupi 使用当前工作区代码。本次没有拉取参考仓库，也没有执行参考应用的交互测试。以下是源码证据，不当作实机验收。

### Jaco：自己维护编辑器与 token

- `app/jaco/src/components/chat/input/composer_editor.rs` 的 `confirm_skill_completion` 只将候选替换为 `$名称 ` 并关闭候选。`on_submit` 先处理补全，命中后立即返回，不继续发出 SubmitRequested。这正好对应用户此次提出的“选择后继续输入”。
- `composer_editor/token.rs` 保存 token ID、UTF-8 文本范围和 Skill 身份；提供 token 前后边界、光标吸附、扩大编辑范围。删除、部分选区替换、从 token 内输入都扩大到整块处理。
- `composer_editor/element.rs` 把普通文本与 token 分片布局，自行测量、绘制标签并处理命中；不是用 TextareaState 的背景色模拟标签。
- `composer_editor/history.rs` 让文本、选区、输入法标记范围和 tokens 一起进入撤销历史。`composer_editor.rs` 实现 EntityInputHandler，承担 UTF-16 输入法接口与内部文本位置转换。
- `snapshot.rs` 输出 Jaco 的 ContentPart 和 SkillActivationRequest。Gupi 不能照搬此发送契约，也不能照搬任意位置多个 `$skill` 的解析规则。
- 文件和图片使用 `components/chat/form.rs::render_attachments` 的共同附件栏，卡片按类型分支；不要求普通文件成为正文内 token。

可借鉴：token 编辑边界、文字与标签布局、补全确认不发送、编辑历史一致性。不能只复制标签绘制函数而丢掉剪切、输入法和撤销规则。

### Zed：已有 Editor + 内联折叠

- `crates/agent_ui/src/message_editor.rs` 创建现有 `editor::Editor`，配置软换行与 completion provider。`insert_skill_crease` 先插入资源链接文本和空格，再登记 mention，不在此发送消息。
- `crates/agent_ui/src/mention_set.rs::insert_crease_for_mention` 使用文本 anchors 和 `Crease::Inline`，为范围设置 FoldPlaceholder，随后 fold；标签是折叠后的自定义展示，底层内容仍在文本 buffer 中。相邻标签设置 `merge_adjacent: false`。
- Zed Editor 的 Backspace 通过 display map 移动并映射回 buffer 范围，从而能跨过整个折叠区。`message_editor.rs` 有删除 mention 后不再发送资源链接的测试；不能把已删除标签的资源对象遗留到发送快照中。
- `build_chunks_from_creases` 按顺序输出正文与 ACP ContentBlock；MentionSet 跟踪资源身份并移除失效范围。此 ACP 表达能力不等于 Pi RPC 能力。

可借鉴：真实内容与显示标签分离、稳定范围、发送时从当前编辑内容生成快照。当前 Gupi 的 gpui-kit 0.6.0 没有公开的 Zed Editor/Crease/自定义内联标签等价接口；不能直接移植 Zed 的调用代码，也不建议为输入框引入 Zed 整套编辑器依赖。

### Gupi 现状与具体接入点

- `features/composer.rs` 只负责共用外壳与底部布局；正文仍为 TextareaState。共用外壳内部已换成上游 InputGroup，token 能力归正文编辑层，保留模型选择器和发送按钮的业务归属。
- `features/command_palette.rs::activate` 将 Skill、模板、插件统一当作 `Id::Pi(name)`，补全后在 execute=true 时直接 `send_text`，这是选中 Skill 就发送的原因。现有行分组虽来自 RPC source，但派发身份没有保留资源类型。
- Pi `get_commands` 已返回 `source: skill/prompt/extension` 和来源信息。应保留此类型用于派发，不仅靠名字前缀猜测；插件仍可有容易混淆的命令名称。
- Skill 确认应变成一次明确的“将选定命令写入目标会话草稿”，关闭面板后聚焦正文，并阻止同一次 Enter 继续触发发送。实际发送仍由主输入框 Enter/发送按钮执行。
- 保留命令面板和正文独立输入；此处是用户确认后的单次写入，不恢复双向同步。面板关闭时原有焦点恢复逻辑需要让位于本次明确的正文聚焦。
- 使用当前 session key、binding 和可编辑门禁确认目标仍有效；不能在切换会话或连接变更后向另一个草稿写入。完成写入后更新 slash 候选状态，防止插入 `/skill:…` 立即再弹回命令面板。
- 草稿内容仍由 ConversationState 持有。推荐保留文本作为持久化与 RPC 的依据、标签作为已知命令范围的展示；普通会话原有草稿规则与临时会话不落盘规则继续适用。

## 上游能力可用后的接入考虑（尚未实施）

1. **Skill 选择行为**：恢复工作后保留候选类型，增加一次性写入正文路径，覆盖点击和 Enter；本项随整体输入交互暂缓。命令模板是否同步采用这条规则见待确定项。
2. **接入输入容器与标签能力**：在共用 Composer 内采用 InputGroup，由上游输入状态管理编辑状态和 token 生命周期；Gupi 提供资源身份、图标、标签、点击行为与 Pi 文本表示，不移植或重写 Jaco 编辑器。具体 API 以正式发布版本为准，主窗口与临时窗口共享接入路径。
3. **token 最小范围**：消息开头最多一个已识别的 Skill/模板命令；后面是普通参数文本。展示图标与名称，复制/提交还原 `/skill:名称` 或 `/模板名称`，不把展开后的 Skill/模板正文塞进可编辑草稿。未知命令继续保留为普通文本，不臆造资源身份。
4. **保留附件模型**：文件和图片仍使用现有 Attachment 集合；增加 `@` 候选时选中后复用相同添加路径，取消候选不删除原始文字。不另建一套“文件 token + 附件”重复状态。
5. **单独解决模板与文件引用顺序**：普通 `/模板` 的参数追加不是可靠的附带上下文通道。不能简单在命令前面加路径，否则 Pi 不再识别开头的命令。需要明确选定模板的展开责任，见下节。

仅“按一次 Backspace 删除整段命令”可通过现有 TextareaState 的选区/replace 接口扩展；完整标签仍须统一处理鼠标选区、Delete、剪切、粘贴、输入法、撤销和重做，不能用输入 Change 后补删文本的方式拼凑。

## 待确定项

以下推荐尚未代替用户决定，实施前一起确认。

| 问题 | 建议与影响 |
| --- | --- |
| 命令模板选择后是否也只填入正文？ | 建议与 Skill 一致。用户此次明确要求的是 Skill；插件命令和应用操作继续按既有规则执行，不一并改成填入正文。 |
| 正文已有内容或已有命令时怎样插入？ | 建议只替换开头已识别的命令，保留参数/正文；普通正文前插入所选命令与分隔空格。面板内已输入参数与原正文同时存在时，需确认合并顺序，不能静默覆盖或重复两份文字。Tab 在命令面板内仍只补全，是否也转回正文尚未确认。 |
| 手工输入、粘贴的已知命令是否自动变成标签？ | 建议确认完整命令边界后统一识别，输入法组合期间不转化；未知命令保留原文。这样手工输入和候选选择不会成为两套语义。 |
| `@` 文件搜索是否本轮一起实现？ | 建议作为同一文档的后续独立步骤，第一阶段先用已有选择/粘贴/拖拽附件。若纳入，推荐只搜索当前会话 cwd，后台查询、取消过期结果，不做全磁盘检索。 |
| 模板带文件引用如何确保不丢失？ | 建议仅对用户明确选中的模板，由 Gupi 按 Pi 参数语义展开后追加文件引用；图片继续独立发送。代价是输入插件将看到展开后的正文，而不再是原 `/模板 参数`。必须核对同名扩展命令优先级、模板来源/启用状态及内容变化；不自行安装拦截插件或假设 RPC 有额外文件字段。是否接受这一行为差异需要确认。 |

## 必要验证

- 共用 InputGroup 下的附件栏、模型选择器、发送操作与模板任务编辑器保持原有行为；输入框聚焦、禁用、尺寸与换行布局正常。
- Skill 点击/Enter 确认只改目标草稿、不调用 RPC prompt；随后主输入框发送一次。旧正文、参数、面板取消和切换会话按确认规则处理。
- 已知命令标签的前后退格、Delete、部分选区替换、剪切、粘贴、中文/emoji、IME、撤销/重做；删除标签后发送快照不残留资源信息。
- 手工输入、粘贴、候选确认的命令序列化一致；模板参数空格、引号和换行保真；未知命令与普通 `/` 不被强制转换。
- 文件/图片添加与移除不重复提交；无占位符模板、有占位符模板都不丢文件引用。沿用发送确认后清空、失败原位保留的契约，不新增草稿备份队列。
- 实施后只复测受影响编辑与选择场景；本调研不产生新的全平台验收或打包任务。

## 参考入口

- [Jaco 编辑器](../../../../jaco/src/components/chat/input/composer_editor.rs)、[token](../../../../jaco/src/components/chat/input/composer_editor/token.rs)、[布局](../../../../jaco/src/components/chat/input/composer_editor/element.rs)、[历史](../../../../jaco/src/components/chat/input/composer_editor/history.rs)。
- Zed 本地：`/Users/sushao/Documents/code/zed/crates/agent_ui/src/{message_editor,mention_set}.rs`、`crates/editor/src/editor.rs`。
- Pi 本地：`/Users/sushao/Documents/code/pi/packages/coding-agent/src/core/{agent-session,prompt-templates}.ts`、`src/modes/rpc/rpc-mode.ts`、`src/cli/file-processor.ts`。
- [现有命令入口](../issue-226/command-palette.md)、[当前输入能力](README.md)。
