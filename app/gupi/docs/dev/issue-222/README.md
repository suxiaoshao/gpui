# Pi 扩展 UI：交互测试环境与验收

状态：Blocked。按用户 2026-09-13 的决定，等待上游 gpui-kit 合入 question 组件后，再继续本部分的 UI 接入与调整。临时插件和隔离启动脚本保留，完整插件 UI 验收尚未完成。

归属：[#222](https://github.com/suxiaoshao/gpui/issues/222) 的扩展 UI 部分。目标是让开发者和用户随时在真实 Gupi 界面触发不同插件 UI，观察布局并完成提交、取消和连续交互。本部分可基于已有主窗口独立推进，无须等待临时窗口。队列等其他工作不在本文展开。

当前实现不修改生产界面、不安装插件、不修改用户 Pi 配置。插件/包、Skill 和提示词管理另行规划。任意 TUI 组件与同文件树节点续聊继续保持现有边界。

## 当前结论与恢复条件

2026-09-13 确认：

- Pi 原生 RPC 提供九类扩展 UI 消息；上下方 widget 是位置区别，不是额外类型。没有通用自定义 dialog、原生多选问卷或“选项加补充文字一起提交”的组合 schema。
- 本机 ask_user_question 插件 2.9.0 的 RPC 单选流程是先 select；选普通项立即返回，选“自行输入”再打开 input。多选直接使用 input：`1,3` 解析为选项，普通文字为自定义回答；`1,3 补充说明` 整体作为自定义文字，不同时返回选项和补充说明。依据为该插件 `rpc-fallback.ts` 的 askSingleSelect/askMultiSelect。
- 本地 `draft show` 在 TUI 中使用 custom 阅读浮层；RPC 分支将标题、路径和正文降级为 notify，不传输原阅读弹窗、滚动或打开文件操作。`draft edit` 使用标准 editor，属于 RPC 已支持的类型。依据为本地 `extensions/draft/viewer.ts` 的 showDraftViewer 及 `index.ts` 的调用。
- 命令入口已由 #226 接入，使用 / 或 Cmd/Ctrl+Shift+P；Cmd/Ctrl+P 用于会话快速打开。详见 [统一命令面板](../issue-226/command-palette.md)。
- 临时插件只用于用户自行体验。保留已有检查结果，不继续自动操作体验窗口、扩展验收或按猜测修改生产 UI。

恢复工作时，先核对上游 question 组件的实际 API，以及当前 gpui-kit 依赖是否已包含它，再决定如何映射 RPC 的 select/confirm/input 等消息。组件提供的视觉能力不改变 Pi 协议，不能由此推断 RPC 新增了组合问卷或 custom 支持。本轮未查询上游合并或发布状态，不设自动监控；由用户后续恢复工作。

## 体验入口

在仓库根目录运行：

```sh
python3 script/gupi-ui-gallery
```

默认先构建 Gupi，再打开独立环境；已有当前构建可加 `--no-build`。`--pi /absolute/path/to/pi` 指定已有 Pi，`--keep` 保留退出后的临时目录，`--prepare-only` 仅准备环境。macOS 使用临时的 `Gupi UI Gallery.app` 标识窗口，不是发行包。

在新窗口输入 `/gupi-ui` 打开场景菜单；直接输入 `/gupi-ui select`、`/gupi-ui confirm`、`/gupi-ui input`、`/gupi-ui editor` 可以跳过菜单。`/gupi-ui sequence` 连续展示四种交互。退出待答界面用其取消按钮，结束体验用 Cmd+Q。未匹配的普通输入会被临时插件拦截，不调用模型。

实现位置：[临时插件](../../../tests/fixtures/ui-gallery.mjs)、[隔离启动脚本](../../../../../script/gupi-ui-gallery)。插件只通过本次 wrapper 的 `--extension` 加载，不注册到个人配置。用户已明确自行测试，后续不自动接管体验窗口或扩展测试范围。

## 1. Pi RPC 究竟有多少种扩展 UI

按 `RpcExtensionUIRequest.method` 计算，共 **9 种**：4 种需要回执的交互，5 种无需回执的内容更新。此处统计扩展 UI 子协议，不包含普通消息、工具结果、扩展错误事件或模型输出。

| 分类 | 扩展 API → RPC method | 内容与结果 | 需要体验的场景 |
| --- | --- | --- | --- |
| 交互 | `ctx.ui.select` → `select` | 标题、字符串选项、可选 timeout；返回选项字符串或取消 | 少量/大量选项、长文本、中文、点击、键盘、取消 |
| 交互 | `ctx.ui.confirm` → `confirm` | 标题、说明、可选 timeout；返回 confirmed 或取消 | 确认、否、取消、长说明 |
| 交互 | `ctx.ui.input` → `input` | 标题、placeholder、可选 timeout；返回字符串或取消 | 占位文字、空文本、中文输入、提交、取消 |
| 交互 | `ctx.ui.editor` → `editor` | 标题、可选 prefill；返回字符串或取消 | 多行预填、换行、长文本、滚动、提交、取消 |
| 展示 | `ctx.ui.notify` → `notify` | message；info / warning / error | 三种通知、长文本、连续通知 |
| 展示 | `ctx.ui.setStatus` → `setStatus` | statusKey、statusText；省略文本表示清除 | 多 key、同 key 更新、清除 |
| 展示 | `ctx.ui.setWidget` → `setWidget` | widgetKey、字符串数组；可选 aboveEditor / belowEditor；省略内容表示清除 | 上/下位置、默认位置、多行、多 key、更新、清除 |
| 展示 | `ctx.ui.setTitle` → `setTitle` | title 字符串 | 标题提示、连续更新、与会话名称区分 |
| 编辑器控制 | `ctx.ui.setEditorText` → `set_editor_text` | text 字符串 | 普通草稿回填、替换、空字符串清除 |

注意：

- widget 上方和下方是同一种 UI 的位置选项；三种通知级别也不算三种消息。
- `input` 的第二个参数是 placeholder，预填内容属于 `editor` 的 prefill；不能把两者混用。
- `select` 仅提供 `string[]` 和单个字符串返回值；没有多选、分组选项、描述字段或任意表单 schema。可搜索列表可以是宿主呈现方式，但不是另一种 RPC method。
- 多题问卷可由插件顺序调用 select/input 等接口组成。多选若通过文本编号实现，是插件的解释逻辑，Gupi 不推断题型或伪造多选控件。
- `pasteToEditor` 在 RPC 中退化为 `setEditorText`，不新增类型，也没有 TUI 粘贴行为。
- select/confirm/input 支持 timeout；editor 的 RPC 请求没有 timeout。取消回执为 `cancelled: true`；Pi 将 confirm 取消解释为 false，其他三类解释为 undefined。

### 没有对应原生 RPC UI 的接口

Pi RPC 的 `custom()` 直接返回 undefined；`setHeader`、`setFooter`、`setEditorComponent`、widget 的组件工厂不传输 TUI 组件。`onTerminalInput`、`addAutocompleteProvider`、工作指示器/隐藏思考标签定制、工具展开控制也没有对应 UI 消息。主题查询/切换在该 RPC 上下文中提供空结果或不支持结果；`getEditorText()` 返回空字符串。

这些不能通过发送新的私有 method 就变成“支持 Pi 插件”。测试时应区分插件是否实现了 RPC fallback；不把 TUI 截图中的任意窗口都算作 Gupi 待实现界面。

## 2. 现有基础与缺口

- [现有协议 fixture](../../../../../crates/pi-rpc/tests/fixtures/extension.mjs) 注册 `/gupi-rpc-test`，依次调用 select、confirm、input、editor，再设置 status、下方 widget，并通知结果。
- [真实 Pi 协议测试](../../../../../crates/pi-rpc/tests/installed_pi.rs) 在临时目录中加载该扩展，自动发送回执并核对结果；这证明的是协议链路，不是用户操作或视觉效果。
- [Gupi 输入区](../../../src/features/home/composer.rs) 已渲染四类交互：select 为选项按钮，confirm 为确认/否按钮，input/editor 使用扩展专用文本输入；共同提供取消入口。
- [会话状态](../../../src/state/conversation.rs) 已接收九类消息、排队待答请求、按来源会话回传，以及清理断线后的扩展状态。

新增 gallery 提供随时启动、单独选场景、重复体验的原生环境。原有四题串行测试保持不变；自动回执无法代替焦点、中文输入法、滚动和鼠标操作体验。

## 3. 推荐测试环境：真实 Pi + 独立测试扩展

```text
独立配置启动的 Gupi
  → prompt("/gupi-ui select")
  → 真实 Pi 执行测试扩展命令
  → ctx.ui.select(...) 发出 extension_ui_request
  → Gupi 现有输入区显示选项
  → 用户操作，Gupi 回传 extension_ui_response（同一 id）
  → 扩展得到真实返回值，并显示结果
```

测试扩展只调用 UI API，不调用模型，不注册执行真实任务的工具。Pi 在模型与凭据校验前执行已注册的扩展命令，因此无需为了展示选择器启动一次模型回答。

### 开发入口

应用归属的 fixture 位于 `app/gupi/tests/fixtures/ui-gallery.mjs`，配合开发启动脚本。保留现有协议测试的固定序列，避免 UI 场景调整影响传输层测试。

预期操作流程：

1. 运行开发启动脚本，创建并打印本次临时根目录，启动独立 Gupi 窗口。
2. 在该窗口中选择脚本创建的测试项目目录。
3. 输入 `/gupi-ui` 查看场景选择菜单，或输入 `/gupi-ui select` 直接打开选择器。
4. 完成操作后看见扩展返回结果；再次输入同一命令可重复体验。
5. 退出独立应用，确认它持有的 Pi 进程收尾；脚本只清理本次临时目录。需要保留日志时保留该目录，不混入个人会话目录。

不新增面向用户的“测试模式”设置或生产 UI 分支。Gupi 已有 `pi_command` 可执行路径配置，用一个临时 wrapper 加载测试扩展即可。`pi_command` 不是 shell 命令字符串，不能把带参数的 `pi --extension ...` 直接写入该字段。

### 启动隔离与 wrapper 契约

- Gupi 的 `GUPI_CONFIG_DIR`、`GUPI_LOG_DIR` 指向临时目录；生成最小 config.toml，将 pi_command 指向 wrapper 的绝对路径。
- Pi 使用独立 `PI_CODING_AGENT_DIR` 和 `PI_CODING_AGENT_SESSION_DIR`；让 Gupi 同样继承这些路径，以保持历史发现与 Pi 写入一致。
- 使用空测试 cwd，禁用自动扩展、skills、prompt templates、themes、context files，并只显式加载 fixture；参数参考已有 installed_pi 测试：`--offline --no-extensions --no-skills --no-prompt-templates --no-themes --no-context-files --no-approve --extension <fixture>`。
- wrapper 原样处理 `--version`，调用真实 Pi 的版本探测；RPC 启动保留 Gupi 传入的参数，只补测试参数。真实 Pi 路径在生成 wrapper 前解析，避免 wrapper 递归调用自身。
- wrapper 使用 exec 交接进程，日志写 stderr 或独立文件，不向 RPC stdout 输出调试文本。
- 原生测试保留临时目录内的 session 持久化，方便切换会话和重启检查；无需使用协议测试中的 `--no-session`。
- 测试进程环境参考现有 installed_pi 测试采用必要环境变量白名单；本机 UI 启动所需的平台变量在脚本中按需保留。不复制用户认证或扩展配置。
- `--offline` 本身不等于禁止模型调用。fixture 不发起模型请求，并通过 input hook 对未匹配的普通输入返回 handled、显示帮助，避免命令拼错后落入普通 prompt。测试脚本不需要模型凭据。

当前脚本遵循以上契约。第一轮以本机 macOS 的可交互环境交付，不扩大为多平台启动器工程。

## 4. 场景菜单与选择器如何构造

fixture 注册一个 `/gupi-ui` 命令，根据参数调度场景。无参数时用 select 展示菜单，同时保留直接命令，避免选择器自身有问题时无法测试其他 UI。每次只运行选中的场景，结束后恢复普通输入；不自动无限循环弹菜单。

| 命令参数 | 场景 |
| --- | --- |
| `select` / `select-long` | 3 个普通选项；大量长文本选项，检查尺寸和滚动 |
| `confirm` | 一段说明，分别确认、否、取消 |
| `input` | 带占位文字的输入，检查中文、空文本和取消 |
| `editor` | 含多段中文与代码的多行预填文本 |
| `notify` | 依次显示 info、warning、error |
| `status` / `status-clear` | 多状态项、同 key 更新和清除 |
| `widget-above` / `widget-below` / `widget-clear` | 输入区上下多行文本，重复运行更新原 key，按 key 清除 |
| `title` | 修改扩展标题提示，观察会话名是否保持独立 |
| `editor-text` | 用固定内容设置普通输入；提示用户此场景会替换测试草稿 |
| `sequence` | select → confirm → input → editor，观察每一步与最终回传结果 |
| `timeout` | select/confirm/input 的有限超时，观察到期后的恢复 |

选择器不需要假造会话 JSONL 或直接注入 Gupi 状态。下面是 fixture 的核心逻辑示意，尚未保存为可执行测试文件：

```js
export default function (pi) {
  pi.registerCommand("gupi-ui", {
    description: "Gupi extension UI gallery (no model calls)",
    handler: async (args, ctx) => {
      const scene = args.trim() || await ctx.ui.select("选择 UI 场景", [
        "select", "confirm", "input", "editor",
      ]);
      if (scene === undefined) return;
      let value;
      switch (scene) {
        case "select":
          value = await ctx.ui.select("选择处理方式", ["翻译", "润色", "解释"]);
          break;
        case "confirm":
          value = await ctx.ui.confirm("确认操作", "这里只演示交互，不会修改文件。");
          break;
        case "input":
          value = await ctx.ui.input("输入名称", "例如：测试项目");
          break;
        case "editor":
          value = await ctx.ui.editor("编辑文本", "第一段预填内容\n\n第二段预填内容");
          break;
        default:
          ctx.ui.notify("未知场景，请输入 /gupi-ui", "warning");
          return;
      }
      ctx.ui.notify(JSON.stringify({ scene, value: value ?? null }), "info");
    },
  });
  pi.on("input", async (_event, ctx) => {
    ctx.ui.notify("此环境只演示 UI，请输入 /gupi-ui", "info");
    return { action: "handled" };
  });
}
```

完整 fixture 的结果同时使用专用下方 widget 显示，方便通知消失后继续核对。空字符串、false 和取消需要保留区别；confirm 的取消在 Pi 返回值层面与 false 相同，不虚构额外结果。结果展示不得使用 setEditorText 覆盖原草稿。

## 5. 从可看见到可验收

第一轮先覆盖九类消息和 widget 两个位置，让用户能实际点击、输入并提出布局意见。重点观察选择器的选项数量、长文本、输入区高度、取消入口、焦点与中文输入法。界面修改以这些实际结果为依据。

随后按已实现边界完成必要交互检查：

- 每种交互提交/取消后，扩展收到对应值，普通输入恢复；保留普通草稿和扩展输入的区别。
- 连续请求按顺序显示；回执不能变成普通聊天 prompt；命令结束后不残留待答界面。
- 在测试项目 A 留一个待答请求，切到测试项目 B；返回 A 后继续作答，确认结果属于 A。延迟请求场景可用于先输入一段普通草稿再显示交互。
- timeout 按真实 Pi 请求验证，不给 editor 人为增加 timeout。Pi 内部 AbortSignal 取消未提供独立撤回 UI 消息，这属于协议观察边界；不据此直接新增生产取消机制。
- 状态和 widget 更新/清除，以及连接结束后的扩展 UI 清理，沿用已有回归并做必要原生检查。

真实插件另做一次端到端验收：使用用户已安装的问卷插件，在独立环境显式加载确认过的入口，核对它实际采用的 RPC fallback。若其入口依赖模型工具调用，不能把手工调用相似 select/input 当成真实插件验收；先确定可用触发方式，再单独运行并记录。fixture 覆盖与真实插件通过分别记录。

## 6. 实施顺序与证据

1. 增加 UI gallery fixture 和隔离启动脚本，验证 Pi 探测、加载命令与普通文本拦截。
2. 在 Gupi 窗口跑四类交互，核对真实返回值；展示其他五类消息，提供可重复操作的入口。
3. 根据用户体验和可复现问题修改布局/行为；仅补具体缺陷的必要回归，并验证受影响构建。
4. 跑来源隔离、连续请求等必要场景和真实插件流程；记录实际平台、Pi 版本、插件版本、结果与未验证项。

当前证据：2026-09-12 核对本地 Pi checkout `71dca871b` 的 `packages/coding-agent/src/modes/rpc/rpc-types.ts`、`rpc-mode.ts`，以及 `core/agent-session.ts` 的扩展命令优先执行路径；并对照本机安装的 `@earendil-works/pi-coding-agent` 0.85.1 的 `dist/modes/rpc/rpc-mode.js`，九类 UI 映射一致。版本号相同不保证 checkout 与安装产物完全相同，后续运行继续记录实际版本与来源。

当前实现已完成 Gupi 构建；真实 Pi 的临时回执检查经过九类消息、连续问答和普通文本拦截，统计为零模型消息、零 Token。macOS 原生窗口已显示单选并完成一次点击回传，随后停在 sequence 第一题交由用户体验。timeout 场景的协议检查提前作答，未验证自然到期；其他原生场景和真实插件未验收。用户要求自行测试后停止原生操作，不追加自动检查。

## 7. 本机其他插件的 UI 源码盘点

以下读取 2026-09-12 本机全局 settings.json 中声明的十个包及自动发现的本地扩展，只检查 UI 调用与降级分支，没有运行这些插件、执行它们的命令或修改配置。表中“可传输”表示 API 在 RPC 中存在，不代表当前 Gupi 视觉或功能验收通过。

| 插件 / 本机版本 | UI 形式 | RPC 边界 |
| --- | --- | --- |
| pi-approval-guardian 0.8.0 | 通知、下方警示文本 widget | 使用可传输的 notify/setWidget |
| pi-web-access 0.22.0 | 选择、通知、搜索活动文本 widget | 标准消息可传输；活动面板的一个入口是 registerShortcut，RPC UI 消息本身不接入该 TUI 快捷键 |
| pi-codex-image-gen 0.1.12 | 图像生成工具与结果 | 本次没有发现新增的独立对话框 UI 类型；图片/工具结果的呈现另属消息内容能力，不算九类扩展 UI |
| pi-xai-oauth 1.5.0 | 选择、通知、状态 | 标准 UI 可传输；登录/认证流程未运行 |
| pi-subagents 0.49.0 | 标准选择/确认/编辑器/状态，以及任务树 widget、Fleet 面板和自定义消息渲染 | 任务 widget 传的是组件工厂，Pi RPC 忽略；Fleet 使用 custom，无法直接显示；注册的 TUI 消息 renderer 不作为 UI 组件传输 |
| @narumitw/pi-plan-mode 0.49.3 | 选择、编辑器、草稿回填、通知、计划状态与文本 widget | 使用标准消息；源码明确允许 RPC 查看已保存计划 |
| @narumitw/pi-usage 0.60.7 | 状态、通知、选择及自定义设置列表 | 设置列表仅在 TUI 使用 custom；非 TUI 通知用户手工编辑配置，不会出现同款设置面板 |
| @juicesharp/rpiv-ask-user-question 2.9.0 | TUI 标签页问卷、自定义浮层、RPC 连续问答 | 明确提供 RPC fallback：单选走 select，自由回答/多选编号走 input；并排预览和多题复核页不保留 |
| @narumitw/pi-goal 0.51.0 | 确认、输入、编辑器、状态 | 使用标准消息；未运行目标任务 |
| pi-intercom 0.10.1 | 确认、通知 | 标准消息可传输；未连接或发送真实通信 |
| 本地 change-prompt | 提示词选择、通知、状态 | 使用标准消息 |
| 本地 draft | 选择、输入、编辑器、确认、状态，以及阅读浮层 | 管理对话框支持标准 RPC；阅读浮层仅 TUI 使用 custom，其他模式降级为文本通知 |
| 本地 system-theme-sync.ts | 跟随系统切换 TUI 主题 | session_start 明确跳过 RPC，不应在 Gupi 期待其主题切换 |

本地 `extensions/subagent` 只有配置文件、`pi-codex-image-gen-install.json` 为安装记录，未当成额外可执行 UI 扩展计数。未枚举个人每一个项目目录的扩展配置。

主要证据位置（相对 `~/.pi/agent/`）：

- `npm/node_modules/pi-subagents/src/tui/render.ts` 的 renderWidget、`src/tui/fleet.ts` 的 custom 调用、`src/extension/index.ts` 的 registerMessageRenderer。
- `npm/node_modules/@narumitw/pi-usage/dist/index.ts` 的 showUsageSettings，在非 TUI 时直接通知并返回。
- `npm/node_modules/@juicesharp/rpiv-ask-user-question/ask-user-question.ts` 和 `rpc-fallback.ts`，检查 mode、顺序问答以及多选文本处理。
- `npm/node_modules/pi-web-access/index.ts` 的 updateWidget 与 activityKey 快捷键处理。
- `extensions/draft/viewer.ts` 的 showDraftViewer，以及 `extensions/system-theme-sync.ts` 的 RPC 提前返回。

因此，当前临时插件覆盖的是 RPC 可表达的 UI。子代理任务面板、问卷标签页、usage 设置列表属于额外 TUI 呈现；要在 Gupi 获得相应能力，需要上游 RPC 降级或独立产品设计，不能通过添加一种测试 JSON 消息冒充支持。
