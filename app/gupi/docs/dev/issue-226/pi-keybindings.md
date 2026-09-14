# Pi TUI 默认快捷键完整清单

来源：Pi upstream main `71dca871bc80b6bc97be37f0ca3189399d651fff`，2026-09-13 拉取并核对。由两份 keybindings.ts 的定义对象提取；上游 description 保留原文，中文作用域与 Gupi 判断见[调研正文](README.md)。

合并注册表共 **90 个动作**；macOS **80 个默认绑定、10 个默认未绑定**。基础 TUI 47 项，应用 43 项；应用层的四项 TUI 覆盖不重复计数。键位沿用 Pi 配置格式，`ctrl` 是 Control，`alt` 是 Option/Alt，不是 Cmd。

每行统计动作，不按别名重复统计；同一键可在不同上下文使用。未包含插件动态 registerShortcut、终端自身映射、slash command 文本和注册表之外的组合逻辑。

## 输入编辑与历史

| 动作 ID | macOS 默认键位 | 上游行为说明 |
| --- | --- | --- |
| `tui.editor.cursorUp` | `up` | Move cursor up |
| `tui.editor.cursorDown` | `down` | Move cursor down |
| `tui.editor.historyPrevious` | 未绑定 | Select previous prompt history entry |
| `tui.editor.historyNext` | 未绑定 | Select next prompt history entry |
| `tui.editor.cursorLeft` | `left` / `ctrl+b` | Move cursor left |
| `tui.editor.cursorRight` | `right` / `ctrl+f` | Move cursor right |
| `tui.editor.cursorWordLeft` | `alt+left` / `ctrl+left` / `alt+b` | Move cursor word left |
| `tui.editor.cursorWordRight` | `alt+right` / `ctrl+right` / `alt+f` | Move cursor word right |
| `tui.editor.cursorLineStart` | `home` / `ctrl+home` / `ctrl+a` | Move to line start |
| `tui.editor.cursorLineEnd` | `end` / `ctrl+end` / `ctrl+e` | Move to line end |
| `tui.editor.jumpForward` | `ctrl+]` | Jump forward to character |
| `tui.editor.jumpBackward` | `ctrl+alt+]` | Jump backward to character |
| `tui.editor.pageUp` | `pageUp` / `ctrl+pageUp` | Page up |
| `tui.editor.pageDown` | `pageDown` / `ctrl+pageDown` | Page down |
| `tui.editor.deleteCharBackward` | `backspace` | Delete character backward |
| `tui.editor.deleteCharForward` | `delete` / `ctrl+d` | Delete character forward |
| `tui.editor.deleteWordBackward` | `ctrl+w` / `alt+backspace` | Delete word backward |
| `tui.editor.deleteWordForward` | `alt+d` / `alt+delete` | Delete word forward |
| `tui.editor.deleteToLineStart` | `ctrl+u` | Delete to line start |
| `tui.editor.deleteToLineEnd` | `ctrl+k` | Delete to line end |
| `tui.editor.yank` | `ctrl+y` | Yank |
| `tui.editor.yankPop` | `alt+y` | Yank pop |
| `tui.editor.undo` | `ctrl+-` | Undo |

## 通用输入

| 动作 ID | macOS 默认键位 | 上游行为说明 |
| --- | --- | --- |
| `tui.input.newLine` | `shift+enter` / `ctrl+j` | Insert newline |
| `tui.input.submit` | `enter` | Submit input |
| `tui.input.tab` | `tab` | Tab / autocomplete |
| `tui.input.copy` | `ctrl+c` | Copy selection |

## 通用选择器

| 动作 ID | macOS 默认键位 | 上游行为说明 |
| --- | --- | --- |
| `tui.select.up` | `up` | Move selection up |
| `tui.select.down` | `down` | Move selection down |
| `tui.select.pageUp` | `pageUp` | Selection page up |
| `tui.select.pageDown` | `pageDown` | Selection page down |
| `tui.select.confirm` | `enter` | Confirm selection |
| `tui.select.cancel` | `escape` / `ctrl+c` | Cancel selection |

## 全屏 transcript

| 动作 ID | macOS 默认键位 | 上游行为说明 |
| --- | --- | --- |
| `tui.altScreen.pageUp` | `pageUp` | Scroll viewport up one page |
| `tui.altScreen.pageDown` | `pageDown` | Scroll viewport down one page |
| `tui.altScreen.halfPageUp` | 未绑定 | Scroll viewport up half a page |
| `tui.altScreen.halfPageDown` | 未绑定 | Scroll viewport down half a page |
| `tui.altScreen.lineUp` | 未绑定 | Scroll viewport up one line |
| `tui.altScreen.lineDown` | 未绑定 | Scroll viewport down one line |
| `tui.altScreen.previousPrompt` | `ctrl+shift+up` / `ctrl+up` | Jump to previous semantic prompt |
| `tui.altScreen.nextPrompt` | `ctrl+shift+down` / `ctrl+down` | Jump to next semantic prompt |
| `tui.altScreen.search` | `ctrl+shift+f` | Search the primary scroll view |
| `tui.altScreen.searchNext` | `enter` / `ctrl+g` | Select the next search match |
| `tui.altScreen.searchPrevious` | `shift+enter` / `ctrl+shift+g` | Select the previous search match |
| `tui.altScreen.searchClose` | `escape` | Close transcript search |
| `tui.altScreen.top` | `home` | Scroll viewport to top |
| `tui.altScreen.bottom` | `end` | Scroll viewport to bottom |

## 应用与专用选择器

| 动作 ID | macOS 默认键位 | 上游行为说明 |
| --- | --- | --- |
| `app.interrupt` | `escape` | Cancel or abort |
| `app.clear` | `ctrl+c` | Clear editor |
| `app.exit` | `ctrl+d` | Exit when editor is empty |
| `app.suspend` | `ctrl+z` | Suspend to background |
| `app.thinking.cycle` | `shift+tab` | Cycle thinking level |
| `app.thinking.save` | `ctrl+s` | Save thinking level |
| `app.model.cycleForward` | `ctrl+p` | Cycle to next model |
| `app.model.cycleBackward` | `shift+ctrl+p` | Cycle to previous model |
| `app.model.select` | `ctrl+l` | Open model selector |
| `app.tools.expand` | `ctrl+o` | Toggle tool output |
| `app.thinking.toggle` | `ctrl+t` | Toggle thinking blocks |
| `app.session.toggleNamedFilter` | `ctrl+n` | Toggle named session filter |
| `app.editor.external` | `ctrl+g` | Open external editor |
| `app.message.copy` | `ctrl+x` | Copy message to clipboard |
| `app.message.followUp` | `alt+enter` | Queue follow-up message |
| `app.message.dequeue` | `alt+up` | Restore queued messages |
| `app.clipboard.pasteImage` | `ctrl+v` | Paste image from clipboard (text fallback) |
| `app.session.new` | 未绑定 | Start a new session |
| `app.session.tree` | 未绑定 | Open session tree |
| `app.session.fork` | 未绑定 | Fork current session |
| `app.session.resume` | 未绑定 | Resume a session |
| `app.tree.foldOrUp` | `alt+left` / `ctrl+left` | Fold tree branch or move up |
| `app.tree.unfoldOrDown` | `alt+right` / `ctrl+right` | Unfold tree branch or move down |
| `app.tree.editLabel` | `shift+l` | Edit tree label |
| `app.tree.toggleLabelTimestamp` | `shift+t` | Toggle tree label timestamps |
| `app.session.togglePath` | `ctrl+p` | Toggle session path display |
| `app.session.toggleSort` | `ctrl+s` | Toggle session sort mode |
| `app.session.rename` | `ctrl+r` | Rename session |
| `app.session.delete` | `ctrl+d` | Delete session |
| `app.session.deleteNoninvasive` | `ctrl+backspace` | Delete session when query is empty |
| `app.models.save` | `ctrl+s` | Save model selection |
| `app.models.enableAll` | `ctrl+a` | Enable all models |
| `app.models.clearAll` | `ctrl+x` | Clear all models |
| `app.models.toggleProvider` | `ctrl+p` | Toggle all models for provider |
| `app.models.reorderUp` | `alt+up` | Move model up in order |
| `app.models.reorderDown` | `alt+down` | Move model down in order |
| `app.tree.filter.default` | `ctrl+d` | Tree filter: default view |
| `app.tree.filter.noTools` | `ctrl+t` | Tree filter: hide tool results |
| `app.tree.filter.userOnly` | `ctrl+u` | Tree filter: user messages only |
| `app.tree.filter.labeledOnly` | `ctrl+l` | Tree filter: labeled entries only |
| `app.tree.filter.all` | `ctrl+a` | Tree filter: show all entries |
| `app.tree.filter.cycleForward` | `ctrl+o` | Tree filter: cycle forward |
| `app.tree.filter.cycleBackward` | `shift+ctrl+o` | Tree filter: cycle backward |

## 平台差异

未列出的动作默认键位与 macOS 表一致。Linux 非 WSL 与 macOS 的树左右键仅数组顺序不同，包含的键相同；Windows/WSL 调整来自 Pi 的终端兼容处理，不应直接复制进 Gupi 的桌面键表。

| 动作 | macOS | Linux | Windows | WSL |
| --- | --- | --- | --- | --- |
| `tui.editor.undo` | `ctrl+-` | `ctrl+-` | `ctrl+z` | `alt+z` |
| `tui.altScreen.previousPrompt` | `ctrl+shift+up` / `ctrl+up` | `ctrl+shift+up` / `ctrl+up` | `ctrl+up` | `ctrl+up` |
| `tui.altScreen.nextPrompt` | `ctrl+shift+down` / `ctrl+down` | `ctrl+shift+down` / `ctrl+down` | `ctrl+down` | `ctrl+down` |
| `tui.altScreen.search` | `ctrl+shift+f` | `ctrl+shift+f` | `ctrl+f` | `ctrl+f` |
| `app.suspend` | `ctrl+z` | `ctrl+z` | 未绑定 | `ctrl+z` |
| `app.model.cycleBackward` | `shift+ctrl+p` | `shift+ctrl+p` | `alt+p` | `alt+p` |
| `app.message.followUp` | `alt+enter` | `alt+enter` | `ctrl+q` | `ctrl+q` |
| `app.message.dequeue` | `alt+up` | `alt+up` | `alt+q` | `alt+q` |
| `app.clipboard.pasteImage` | `ctrl+v` | `ctrl+v` | `alt+v` | `alt+v` |
| `app.tree.foldOrUp` | `alt+left` / `ctrl+left` | `ctrl+left` / `alt+left` | `ctrl+left` / `alt+left` | `ctrl+left` / `alt+left` |
| `app.tree.unfoldOrDown` | `alt+right` / `ctrl+right` | `ctrl+right` / `alt+right` | `ctrl+right` / `alt+right` | `ctrl+right` / `alt+right` |

## 注册表以外的入口与作用域

- 空输入时双 Esc：500ms 内按两次，按 doubleEscapeAction 配置打开 tree/fork 或不处理，默认 tree；运行中先处理 abort。
- 双 Ctrl+C：应用处理器中 500ms 内第二次退出；选择器和文本选择等上下文可能先消费该按键。
- `/` 触发 slash command 补全；Tab 补全，Enter 提交，不是独立“打开命令面板”快捷键。
- Ctrl+Shift+D：TUI 的调试回调，未列在 KEYBINDINGS 注册表，不建议作为产品默认操作。
- 默认未绑定的历史前后动作允许用户配置；配置后在主编辑器中优先于应用模型循环。默认 Up/Down 在编辑器首尾也可浏览输入历史。
- fullscreen 的 Home/End/PageUp/PageDown 优先作用于正文视口，Ctrl 变体仍可用于编辑器；普通模式路由不同。
- Ctrl+S 只在相应模型/思考/会话选择器里执行保存或排序，不是主输入区的通用保存。
- 插件可以另外注册快捷键；RPC get_commands 不提供这些快捷键，不把它们算入原生 90 个动作。
