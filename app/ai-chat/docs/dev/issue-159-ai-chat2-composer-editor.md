# Issue #159 ai-chat2 ComposerEditor 自研输入进度板

本文档是 `app/ai-chat2` 真实输入框 `ComposerEditor` 的专项进度板。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档只跟踪 composer 输入内核、token
输入、completion 和 agent loop payload 组装，不跟踪 ChatForm 外框、toolbar 或 picker 视觉预览。

最后同步时间：2026-05-29。

当前状态：第一版输入内核已实现。`app/ai-chat2` 已接入 app-local `ComposerEditor`，替换
ChatForm 的非 input 占位，并完成文本输入、IME range 映射、选择/光标、编辑快捷键、plain text
剪贴板、Enter 发送、Shift+Enter 换行、`$skill-name` token 解析和 `ComposerSnapshot` 输出。

本轮仍不接真实 agent loop、附件存储、provider/model 数据源或 `$` completion UI。当前
`gpui-component::Input` / `InputState` 只作为参考材料，不作为最终 composer 输入框。

## 当前决策

- `ChatForm` 继续负责 Codex 风格外框、toolbar、picker 和按钮；输入区已由 app-local `ComposerEditor` 承接。
- 真实输入能力由 `ComposerEditor` 承接，第一版已支持普通文本、`$skill-name` token metadata 和 snapshot payload；completion UI 和 agent loop 接线后续继续做。
- 第一版实现优先匹配 Zed agent input 的基础编辑体验：IME、选择、全选、双击选词、光标快捷键、选择快捷键、剪贴板和 undo/redo。
- 不为了短期接线把 `gpui-component::Input` 作为最终输入框；如果后续组件库提供合适的 inline token/decoration API，再单独评估是否收敛。

## Zed Agent Input 对照

本轮本地参考代码：

- `/Users/sushao/Documents/code/zed/crates/agent_ui/src/message_editor.rs`
- `/Users/sushao/Documents/code/zed/crates/agent_ui/src/mention_set.rs`
- `/Users/sushao/Documents/code/zed/crates/editor/src/input.rs`
- `/Users/sushao/Documents/code/zed/crates/editor/src/selection.rs`
- `/Users/sushao/Documents/code/zed/crates/editor/src/element.rs`
- `/Users/sushao/Documents/code/zed/assets/keymaps/default-macos.json`
- `/Users/sushao/Documents/code/zed/assets/keymaps/default-linux.json`
- `/Users/sushao/Documents/code/zed/assets/keymaps/default-windows.json`

Zed 的 `MessageEditor` 不是普通 input。它创建 `editor::Editor`，配置 placeholder、soft wrap、
completion provider、context menu、focus 事件、agent send/cancel 事件，并使用 editor 内核提供 IME、
selection、undo/redo、keyboard action 和 mouse selection。Zed 的 skill/context mention 不是语法高亮；
它先插入 `MentionUri::as_link()`，再通过 `Crease::Inline` 把该范围折叠成可渲染 mention chip，发送时从
crease snapshot 还原 content blocks。

`ai-chat2` 不需要完整代码编辑器能力，但需要复刻 agent input 的基础交互质量。

## 目标边界

专项长期目标：

- 单一文本 buffer，带 UTF-8 / UTF-16 range 映射，供 IME 和 GPUI input handler 使用。
- IME composition、marked range、候选框定位和 composition undo grouping。
- 鼠标定位、拖拽选择、双击选词、三击选行、全选、shift 扩展选择。
- 键盘光标移动、word/line/document movement、shift selection、删除到词/行。
- `cmd-z` / `cmd-shift-z`，非 macOS `ctrl-z` / `ctrl-y` / `ctrl-shift-z`。
- copy/cut/paste、paste as plain text、选区序列化。
- multiline composer 的 soft wrap、auto-grow、placeholder、scroll 和 send/newline 策略；第一版已完成 auto-grow、placeholder 和 send/newline，soft wrap/scroll 后置。
- `$skill-name` token 的 range 追踪、特殊样式、无效 token 清理、删除/复制/payload 还原。
- `$` skill completion；后续 `@` context picker 和 slash command 可以分阶段加入。本轮先完成 `$skill-name` token 解析和 snapshot，不实现 completion UI。

不做或后置：

- 语法高亮、Markdown/code diagnostics、code folding、line number、LSP、go-to-definition。
- 多 cursor、column selection、Vim/modal editing、kill ring、复杂 editor inlay hints。
- Zed 的完整 file/thread/selection/pasted-image mention 类型；第一阶段只实现 `$skill-name`。
- 与 agent loop 的真实发送本轮不在输入内核文档内完成，但 payload shape 必须为后续接线预留。

## 功能矩阵

| 能力 | Zed 对照 | ai-chat2 目标 | 状态 |
| --- | --- | --- | --- |
| 文本 buffer | `Buffer::local` + `MultiBuffer::singleton` | app-local 单 buffer；保存 UTF-8 文本和 selection | 已完成 |
| UTF-16 range 映射 | `EntityInputHandler` | IME API 使用 UTF-16，内部统一转 UTF-8 offset | 已完成 |
| IME marked range | `replace_and_mark_text_in_range` / `marked_text_range` | composition 文本带 marked range 和下划线渲染 | 已完成 |
| IME 候选框定位 | `bounds_for_range` | 基于当前 layout 返回候选框 bounds | 已完成，待手动 IME 验证 |
| composition undo grouping | editor tests 覆盖 composition undo/redo | composition 更新合并为一个 history group；composition 中 undo 取消 marked range | 已完成 |
| 单击定位 | `EditorElement` mouse down | 根据 hit test 设置 cursor，清除 selection | 已完成 |
| 拖拽选择 | `selection.rs` begin/extend selection | 鼠标拖拽扩展 selection，支持越界 autoscroll 可后置 | 已完成；越界 autoscroll 后置 |
| 双击选词 | click count 2 | 按 word boundary 选择 `$skill-name` 或普通词 | 已完成 |
| 三击选行 | click count 3 | 选择当前 visual/text line | 已完成 |
| 全选 | `SelectAll` | macOS `cmd-a`，其他平台 `ctrl-a` | 已完成 |
| shift-click 扩选 | selection extension | 从 anchor 到点击位置扩展 selection | 已完成 |
| 左右移动 | `MoveLeft` / `MoveRight` | caret 左右移动，清除或保持 selection | 已完成 |
| 上下移动 | `MoveUp` / `MoveDown` | visual line 上下移动，保持 preferred column | 已完成 |
| word movement | macOS `alt-left/right`，其他平台 `ctrl-left/right` | 按词边界移动；token 作为独立词段 | 已完成 |
| line movement | macOS `cmd-left/right`、`ctrl-a/e`，Home/End | 移动到行首/行尾 | 已完成 |
| document movement | macOS `cmd-up/down` | 移动到 buffer 开头/结尾 | 已完成 |
| shift selection movement | Zed keymap shift variants | 对应 movement 的选择版本 | 已完成 |
| Backspace/Delete | editor actions | 删除 selection、前/后字符，token range 被破坏时清理 token metadata | 已完成 |
| 删除到词/行 | `DeleteToPreviousWordStart` 等 | macOS alt/cmd delete，其他平台 ctrl delete/backspace | 已完成 |
| Enter send | `AcpThread > Editor && !use_modifier_to_send` | 默认 Enter 发送；Shift+Enter 换行 | 已完成 |
| modifier send | `use_modifier_to_send` key context | 后续设置项：macOS cmd-enter 发送，其他平台 ctrl-enter 发送，Enter 换行 | 后置 |
| undo/redo | `editor::Undo` / `Redo` | 文本、selection、token metadata 一起回滚 | 已完成 |
| copy/cut | `MessageEditor::copy/cut` | 序列化 selection；token 可复制为显示文本或 payload link，策略待实现时固定 | 已完成，plain text |
| paste | `MessageEditor::paste` | plain text paste；后续图片/文件 paste 交给附件入口 | 已完成，plain text |
| context menu | Zed custom Cut/Copy/Paste/Paste Raw | 基础编辑菜单；是否接 app menu action 后续实现 | 后置 |
| placeholder | `set_placeholder_text` | 空文本显示 muted placeholder | 已完成 |
| soft wrap | `set_soft_wrap` | 按 composer 宽度换行，不横向滚动 | 后置；当前按显式换行布局 |
| auto-grow | `EditorMode::auto_height` 类场景 | 高度随内容增长，超过上限后内部滚动 | 已完成 2-8 行自动高度；内部滚动后置 |
| scroll | editor scroll state | 支持长输入内部滚动和光标可见 | 后置 |
| `$skill-name` token parse | Zed skill mention completion + crease | 插入 skill token，保存 range + skill id/path/source snapshot | 已完成 |
| token 特殊样式 | Zed `Crease::Inline` chip | 第一版可以是 inline styled token；chip/replacement 可后续增强 | 已完成，inline styled token |
| token 删除策略 | Zed crease invalidation | partial edit 破坏 token 时清理 metadata；删除整个 token 时删除对应 metadata | 已完成，变更后重解析清理 |
| token payload | `build_chunks_from_creases` | 发送时输出 plain text + skill snapshot blocks，不能只依赖渲染文本 | 已完成，输出 `ComposerSnapshot` |
| `$` skill completion | `PromptCompletionProvider` | 输入 `$` 或 token prefix 时打开 skill picker | 未开始 |
| `@` context completion | Zed context mention | 后续接 project/file/context picker | 后置 |
| slash command | Zed slash command validation/hint | 后续接 MCP prompts 或 app commands；不阻塞第一版 ComposerEditor | 后置 |
| read-only input attempted | Zed `InputAttempted` | agent running/read-only 时可记录输入尝试，用于后续 queue/提示 | 后置 |

## 实现阶段

1. 输入内核骨架：已完成 `ComposerEditor` 模块、文本 buffer、selection、history、focus handle、基础 render。
2. GPUI input handler：已完成 IME 所需 range 映射、marked range、replace/unmark 和 candidate bounds。
3. 光标与选择：已完成 hit test、caret layout、mouse selection、double/triple click、keyboard movement。
4. 编辑动作：已完成插入、删除、剪贴板、undo/redo、composition grouping 和跨平台 key bindings。
5. Composer 布局：已接入 ChatForm 输入区，保留现有外框和 toolbar；auto-grow 已完成，soft wrap/scroll 后置。
6. Token 模型：已完成 `$skill-name` token range、样式、invalid cleanup、selection/copy/delete 行为。
7. Completion：先接 `$` skill picker，再接 `@` context picker；slash command 后置。
8. Payload：已定义 `ComposerSnapshot`，包含 plain text、token ranges、skill snapshots、attachments 占位和 send policy。
9. Agent loop 接线：由后续 UI/runtime 任务把 snapshot 转成 `ai-chat-agent` input，不在输入内核第一阶段完成。

## 验收与测试

第一阶段必须有代码级测试覆盖：

- UTF-8 / UTF-16 range roundtrip，包含中文、emoji 和混合文本。
- IME composition replace、finalize、cancel、undo/redo grouping。
- word boundary、double-click selection、triple-click line selection。
- keyboard movement 和 shift selection 的核心路径。
- undo/redo 同步文本、selection 和 token metadata。
- token partial edit invalidation、whole token delete、payload snapshot。

UI 行为验证在可运行后补充：

- macOS 中文输入法 composition 和候选框位置。
- 鼠标拖拽、双击、三击、全选、copy/cut/paste。
- `cmd-z` / `cmd-shift-z` 和非 macOS `ctrl-z` / `ctrl-y` / `ctrl-shift-z`。
- Enter 发送、Shift+Enter 换行、后续 modifier-to-send 设置。

## 当前记录

- 2026-05-29：已完成 ChatForm 视觉预览，输入区仍是非 input 占位。
- 2026-05-29：确认最终 `ComposerEditor` 先走 app-local 自研路线；`gpui-component::InputState`
  只作为参考，不作为最终输入能力承诺。
- 2026-05-29：已完成 `ComposerEditor` 第一版实现并接入 ChatForm；已运行 `cargo fmt`、
  `cargo test -p ai-chat2 composer_editor`、`cargo test -p ai-chat2 chat_form`、
  `cargo test -p ai-chat2`、`cargo check -p ai-chat2`、
  `cargo clippy -p ai-chat2 --all-targets --all-features -- -D warnings` 和 `git diff --check`。
  未做手动 macOS 中文输入法、候选框、双击/拖拽和 bundle GUI 验证。
