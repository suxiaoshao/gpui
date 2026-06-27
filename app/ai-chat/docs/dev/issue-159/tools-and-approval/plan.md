# Issue #159 ai-chat2 内置工具与审批计划

日期：2026-06-15
更新：2026-06-16

状态：实施中。V1.0 本地文件工具、审批 UI、流式输出和工具错误恢复已落地并完成本地验证；V1.1+ 能力仍待后续 issue。

关联文档：

- `app/ai-chat/docs/dev/issue-137/README.md`
- `app/ai-chat/docs/dev/issue-159/agent-conversation-page.md`
- `app/ai-chat/docs/dev/issue-159/composer-model-picker.md`

## 当前仓库状态

2026-06-16 实施进度：

- Core payload 已扩展 `ToolApprovalMode`、`ToolPermissionScopeSnapshot`、`ToolAccessRequestPayload`，并保持旧 JSON 兼容。
- `ai-chat-agent` 已新增 V1.0 本地内置工具：`read_file`、`list_directory`、`find_path`、`grep`、`write_file`、`edit_file`。
- `grep` 对模型仍暴露为 `grep`，底层使用 `ignore`、`globset`、`grep-searcher`、`grep-regex`、`grep-matcher` 等 ripgrep crates。
- path approval 已接入 tool call hook：project 内写默认允许，外部读默认允许，外部写在 `RequestApproval` 下暂停并创建 approval request。
- Approved resume 已接入：批准后先执行暂停的内置工具，写入 `ToolResult`，然后以 `AgentRunTriggerKind::Resume` 自动启动后续模型 run。
- `ai-chat2` ChatForm 已增加审批模式 selector，并把模式写入本次 run snapshot。
- conversation detail 已增加专门 tool/approval block UI，带工具 icon、二级折叠和 approve/deny button。
- agent run 主路径已改为真正流式输出：assistant text delta 和 reasoning delta 会增量写入 `conversation_items`，UI 通过 `ConversationItemUpdated` 实时刷新。
- 可恢复工具错误已接入：未知工具、内置工具参数解析失败、runtime tool 缺失等会持久化 failed tool call + error tool result，并把错误结果回传给模型继续下一轮。
- `ai-chat-agent` 大文件已完成结构拆分：`runtime.rs` 保留 run 主流程，streaming/finalization/approval_resume/tests 移入 `runtime/*`；`persistence.rs` 保留 `PersistenceContext` 门面，model/provider_step/conversation_items/tool_hook 移入 `persistence/*`。
- 已执行验证见“验证记录”。

仍未完成：

- `run_command` / 终端执行仍留 V1.1；尚未设计 shell AST、sandbox escalation、灾难命令硬拦截或命令 UI。
- LSP/code intelligence、Web search/fetch、task/sub-agent、plan/todo/question 不在 V1.0，本文件只保留后续范围说明。
- MCP config UI、connected server status、MCP tool approval 专门 UI 仍未实现；当前只复用 tool timeline fallback。
- Tool row 已有 V1 compact/rich-ish 展示和审批动作，但 structured output 的深度视图、attachment result、duration/progress、tool name collision display 仍需后续补齐。
- 审批模式不做跨重启的 conversation/project sticky preference；当前只在 ChatForm 本地状态和 run snapshot 中生效。

实施前基线如下，保留用于说明本 issue 的原始缺口。

- `crates/ai-chat-core/src/payloads.rs`
  - 已有 `ConversationItemPayload::{ToolCall, ToolResult, ApprovalRequest, ApprovalDecision}`。
  - 已有 `ToolSource::{Local, Mcp, ProviderHosted}`。
  - `ToolPolicySnapshot` 当前只有 `approval_policy`、`enabled_sources`、`max_steps`。
  - `RunSettingsSnapshot` 和 `ConversationSettingsSnapshot` 已经携带 `tool_policy`。
  - 这些 payload 使用 `deny_unknown_fields`，新增字段必须用 `#[serde(default)]` 保证旧数据可反序列化。
- `crates/ai-chat-db`
  - 已有 `tool_invocations`、`approval_decisions` 和 agent/provider step 持久化。
  - 首版应复用这些表，不新增 approval 专表。
- `crates/ai-chat-agent`
  - `tool_registry.rs` 已有 `LocalTool`、`ToolDefinition`、`ToolRunPolicy` 和 runtime tool name 生成。
  - `AgentRunRequest` 已携带 `tool_registry`、`provider_tools`、`project_root`、`settings_snapshot`。
  - `runtime.rs` 和 `persistence.rs` 已能持久化 tool call 和 approval request conversation item。
- `app/ai-chat2/src/state/conversations.rs`
  - `default_tool_policy()` 当前返回 `approval_policy: OnRequest`、`enabled_sources: Vec::new()`，正常聊天没有启用任何本地内置工具。
  - `build_run_request()` 已设置 `request.project_root = Some(PathBuf::from(&input.project.path))`。
- `app/ai-chat2/src/components/chat_form.rs`
  - ChatForm 已拥有 provider/model/reasoning 选择，并通过 `ChatFormSubmit` 返回。
  - 审批模式应放在这里，不放到 Settings。
- `app/ai-chat2/src/components/conversation_detail.rs`
  - Agent run 已有外层折叠状态。
  - 缺口是展开 run 以后，tool/approval 需要专门 UI，而不是只走通用 detail block。

## 落地文件和模块边界

首版应按下面的文件边界拆，不新增 `mod.rs`。

Core：

- `crates/ai-chat-core/src/payloads.rs`
  - 新增 `ToolApprovalMode`。
  - 扩展 `ToolPolicySnapshot`。
  - 新增结构化审批访问请求 payload，用于 UI 展示外部写入路径和访问类型。

Agent：

- `crates/ai-chat-agent/src/lib.rs`
  - 暴露 `builtin_tools` 需要给 app 或测试使用的类型时，只 re-export 稳定入口，不暴露内部 executor。
- `crates/ai-chat-agent/src/types.rs`
  - 首版继续使用 `AgentRunRequest.project_root`；不提前引入 multi-root。
- `crates/ai-chat-agent/src/tool_registry.rs`
  - 保持 `ToolRunPolicy` 作为静态 policy。
  - 本地内置工具的 path-based approval 不直接塞进 `ToolRunPolicy`，否则无法区分 project 内写和 external write。
- `crates/ai-chat-agent/src/persistence.rs`
  - 在 `PersistingPromptHook::on_tool_call` 创建 `tool_invocations` 后、真正执行 executor 前，调用动态 `ToolPermissionEvaluator`。
  - `Allow` 才执行 tool；`Ask` 创建 `ApprovalRequest` 并把 invocation 标为 `AwaitingApproval`；`Deny` 写入 error `ToolResult`。
- `crates/ai-chat-agent/src/runtime.rs`
  - 当前 `decide_approval(Approved)` 明确返回 `approved tool resume is not implemented in v1`；本 issue 必须补齐 approved resume。
  - 增加 async approval decision/resume 入口，供 `ai-chat2` runtime 调用。
- `crates/ai-chat-agent/src/builtin_tools.rs`
  - 新增模块入口文件。
- `crates/ai-chat-agent/src/builtin_tools/types.rs`
  - 工具 input/output 类型、`BuiltinToolName`、`PathAccessKind`、`PathAccessRequest`。
- `crates/ai-chat-agent/src/builtin_tools/approval.rs`
  - `ToolPermissionEvaluator` 和 path normalize/project containment。
- `crates/ai-chat-agent/src/builtin_tools/filesystem.rs`
  - `read_file`、`list_directory`、`write_file`、`edit_file`。
- `crates/ai-chat-agent/src/builtin_tools/search.rs`
  - `find_path`、`grep`。
- `crates/ai-chat-agent/src/builtin_tools/registry.rs`
  - 根据 run snapshot 和 project root 注册 V1.0 内置工具。
- `crates/ai-chat-agent/src/builtin_tools/command.rs`
  - 只在确认 V1.1 同 PR 实现 `run_command` 时新增。

DB：

- `crates/ai-chat-db/src/records.rs`
  - 不新增表；如 core payload 扩字段，只依赖 serde default 兼容历史 JSON。
- `crates/ai-chat-db/src/tests.rs`
  - 补旧 JSON round-trip 和 pending approval approved resume 的 repository 级测试。

App state：

- `app/ai-chat2/src/state/conversations.rs`
  - `default_tool_policy()` 默认启用 `ToolSource::Local`，并写入 approval mode defaults。
  - `build_run_request()` 把 ChatForm 的 approval mode 写入 run snapshot。
- `app/ai-chat2/src/state/conversation_runtime.rs`
  - 增加 approve/deny 方法。
  - approve 需要调用 agent runtime 的 approved resume 入口，并通过现有 `ConversationRuntimeEvent::ConversationChanged` 触发重新加载。

App UI：

- `app/ai-chat2/src/components/chat_form.rs`
  - `ChatFormSubmit` 增加 `approval_mode`。
  - footer 接入 approval selector。
- `app/ai-chat2/src/components/chat_form/approval_select.rs`
  - 新增 selector 组件，复用 picker 模式。
- `app/ai-chat2/src/components/conversation_detail.rs`
  - 分派 tool/approval row 到 `tool_blocks.rs`。
- `app/ai-chat2/src/components/conversation_detail/tool_blocks.rs`
  - tool row、approval row、icon/spec mapping、approve/deny button dispatch。
- `app/ai-chat2/src/foundation/assets.rs`
  - 新增 tool/approval icon variants。
- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`
  - 新增 ChatForm selector 和 tool row 文案。

## 横向调研结论

### Zed

Zed 把 tool permission 和 sandbox escalation 分开处理。

- `zed/crates/agent_settings/src/agent_settings.rs`
  - `ToolPermissions` 有默认模式和 per-tool 规则。
  - `SandboxPermissions` 保存 `allow_network`、`allow_fs_write_all`、`allow_unsandboxed`、额外 `write_paths`。
  - 命令规则使用 `regex` crate。
  - Zed 还有不可覆盖的灾难命令硬拦截规则。
- `zed/crates/agent/src/sandboxing.rs`
  - persistent grants 和 per-thread grants 会合并。
  - 额外写路径是规范化后的精确路径授权。
- `zed/crates/agent/src/tools/terminal_tool.rs`
  - project worktree 路径默认进入 writable sandbox paths。
  - 额外写路径、网络、全盘写、unsandboxed execution 都需要 sandbox authorization。
- `zed/crates/acp_thread/src/acp_thread.rs`
  - 审批结果包括 allow once、allow for thread、allow always、deny。
  - thread 会进入 waiting-for-confirmation 状态，UI 决策后继续。
- `zed/crates/agent_ui/src/conversation_view/thread_view.rs`
  - 审批按钮渲染在 conversation/tool-call row 中，不藏在设置页。

对 `ai-chat2` 的取舍：借鉴高层模型，不照搬完整 Zed 实现。首版只做 path-based approval 和 ChatForm 审批模式选择。

### Codex

Codex 有三个对我们有参考价值的权限 profile。

- `:read-only`
  - root 可读。
  - project 不可写。
- `:workspace`
  - root 可读。
  - project roots 和 temp dirs 可写。
  - 部分 metadata 路径默认 read-only。
- `:danger-full-access`
  - 没有外层 sandbox 限制。

Codex 也有 `on-request`、`never` 等审批模式。对当前计划最重要的是默认文件系统模型：外部读允许，项目内写允许，外部写才需要显式审批。

### opencode

opencode 对搜索、文件发现、diff、shell inspection 都优先复用现成库。

- `packages/core/src/ripgrep.ts`
  - 封装 ripgrep 做 file search 和 grep。
- `packages/core/src/filesystem/search.ts`
  - 可选使用 `@ff-labs/fff-bun` 做文件搜索，用 `fuzzysort` 做排序。
- `packages/opencode/src/tool/external-directory.ts`
  - tool 访问 workspace context 外路径时请求 `external_directory` 权限。
- `packages/opencode/src/tool/write.ts`
  - 外部目录先请求 external directory 权限，再带 diff 请求 edit 审批。
- `packages/opencode/src/tool/shell.ts`
  - 用 tree-sitter shell grammar 检查命令路径。

对 `ai-chat2` 的取舍：不要手写递归遍历、glob 匹配、grep 解析和 diff 生成。

## 内置工具能力对比

这一节对比的是“模型可调用或 agent runtime 可注册的内置工具能力”，不是 UI 上能手动完成的能力，也不是 MCP/plugin 外挂能力。

| 能力 | 当前 `ai-chat2` | Zed | Codex | opencode | `ai-chat2` 首版取舍 |
| --- | --- | --- | --- | --- | --- |
| 文件读取 | 有持久化 payload，但正常聊天未注册本地 tool | `read_file_tool.rs` | 主要通过 `exec` 读文件；另有 file-search crate 给内部搜索使用 | `read.ts` / `read-filesystem.ts` | 首版做 `read_file` |
| 目录列表 | 未注册 | `list_directory_tool.rs` | 主要通过 `exec` | `read` 可 list directory，另有 `glob` | 首版做 `list_directory` |
| 文件路径查找 | 未注册 | `find_path_tool.rs`，复用 project/worktree search | `file-search` crate 提供内部 fuzzy/path search；模型侧通常走 `exec` | `glob.ts`，core search 可用 ripgrep/fff/fuzzysort | 首版做 `find_path`，用成熟库，不手写遍历 |
| 内容搜索 | 未注册 | `grep_tool.rs`，复用 Zed project search | 通常通过 `exec` 调 `rg` / shell 命令 | `grep.ts`，底层复用 ripgrep adapter | 首版做 model-facing `grep`，但实现必须是 ripgrep-backed，不手写逐文件 `regex` 扫描 |
| 新建/覆盖文件 | 未注册 | `write_file_tool.rs` | `apply_patch` 或 shell 写入 | `write.ts` | 首版做 `write_file` |
| 精确编辑/补丁 | 未注册 | `edit_file_tool.rs` + `edit_session` fuzzy/streaming edit | `apply_patch` crate 和 runtime integration | `edit.ts` + `apply_patch.ts` | 首版做 `edit_file`；是否加 `apply_patch` 取决于实现复杂度 |
| 路径操作 | 未注册 | `create_directory`、`delete_path`、`copy_path`、`move_path` | shell / patch 间接完成 | 主要通过 shell 或 write/edit/patch 完成 | 首版不做独立 move/copy/delete；先用 write/edit 覆盖主要需求 |
| 命令/终端 | 未注册 | `terminal_tool.rs` / `SandboxedTerminalTool` | `exec` / unified exec / shell-command backend | `shell.ts` / core `bash.ts` | 先作为可选 V1.1；V1 不做复杂 shell AST |
| LSP/代码智能 | 未注册 | diagnostics、go to definition、find references、rename、get/apply code actions | 不作为核心本地 tool 组；更多靠 shell / MCP / IDE integration | `lsp.ts` | 首版不做；等文件工具和审批稳定后单独设计 |
| Web search/fetch | 未注册 | `web_search_tool.rs`、`fetch_tool.rs` | `web_search` tool spec | `websearch.ts`、`webfetch.ts` | 不归入本地文件工具首版；后续作为网络工具单独审批 |
| skill 加载 | 有 file-backed skills runtime 基础 | `skill_tool.rs` | `skills.rs`、plugin skills | `skill.ts` | 保留为 prompt/context 层，不当作文件执行工具 |
| 计划/todo/提问 | 无独立内置 tool UI | thread/agent 相关工具，非同一形态 | `update_plan`、`request_user_input` 等协议工具 | `plan.ts`、`todo.ts`、`question.ts` | 不作为这次本地内置工具范围；ChatForm/agent UI 另行处理 |
| 子 agent / thread | 无 | `create_thread_tool.rs`、`spawn_agent_tool.rs`、`list_agents_and_models_tool.rs` | multi-agent / spawn 相关核心能力 | `task.ts` background task | 不进首版；需要独立产品模型 |
| MCP/plugin/custom tools | `ai-chat-agent` 已有 registry/MCP 基础 | context server registry / ACP server | MCP、dynamic tools、plugin discovery | plugin tools / custom tool loading | 继续作为外部 tool source，权限 UI 复用同一 timeline 形态 |

结论：

- Zed 的内置工具最贴近 IDE：文件、路径、终端、LSP/code action、thread/spawn agent 都是一等工具。
- opencode 的内置工具最贴近 CLI agent：read/glob/grep/write/edit/patch/shell/web/task/todo/question 都明确建模。
- Codex 的内置能力更集中：`exec`、`apply_patch`、`web_search`、MCP/dynamic tools、plan/user-input 等高杠杆工具；文件读写搜索很多时候通过 shell/exec 或 patch 间接完成。
- `ai-chat2` 当前只有底层 tool/approval persistence 和 registry 抽象，没有 product-level built-in local tools。
- `ai-chat2` 首版应先补齐“文件系统 agent 最小闭环”：read/list/find/grep/write/edit + path approval + rich tool UI。
- LSP、Web、task/spawn agent、plan/todo/question 都不应混进首版，否则审批 UI、权限模型、数据流会同时扩张。

## `grep` 命名和 `rg` 实现决策

这里要区分 tool name 和 implementation。

- 对模型暴露的 tool name 保持 `grep`。
  - Zed 的 model-facing tool name 是 `grep`，opencode 的 tool name 也是 `grep`。
  - `grep` 表达的是“搜索文件内容”这类语义，不表示调用系统 `/usr/bin/grep`。
  - 直接把 tool name 叫 `rg` 会把实现细节暴露给模型，并且对非 CLI 用户不如 `grep` 直观。
- 实现不能手写“遍历文件 + `regex` crate 匹配每一行”。
  - 这会重新踩性能、ignore 规则、二进制文件、编码、分页、取消、超大行、上下文行等问题。
  - 当前计划原先只列 `regex = "1.12.3"`，这不足以作为内容搜索实现，应修正。
- Zed 的做法：
  - tool 叫 `grep`。
  - 不调用外部 `rg` binary。
  - 底层走 Zed 自己的 `Project::search(SearchQuery::regex(...))`，并复用 `PathMatcher`、`WorktreeSettings.file_scan_exclusions`、`private_files`。
  - 这很适合 Zed，因为 agent tool 可以直接依赖 Zed project/worktree runtime。
- opencode 的做法：
  - tool 叫 `grep`。
  - 底层 `Ripgrep.Service.grep(...)` 调用 ripgrep binary，并用 `rg --json --hidden --no-messages --glob ...` 解析结构化输出。
  - 这适合 TypeScript/Bun agent，因为复用外部 `rg` 比自己实现搜索更可靠。
- Codex 的做法：
  - 没有单独 model-facing `grep` tool。
  - 主要通过 `exec` 让模型调用 shell/`rg`，再通过 `apply_patch` 修改文件。
  - `codex-file-search` 是路径/文件 fuzzy search，使用 `ignore::WalkBuilder` + `nucleo`，不是内容 grep。

`ai-chat2` 的取舍：

- 保留 model-facing `grep` 名称。
- 默认不依赖系统里已有 `rg` binary，也不在 shell 中调用 `rg`。
- 用 ripgrep 拆出的 Rust crates 实现内容搜索：
  - `ignore::WalkBuilder` 负责遍历和 `.gitignore`。
  - `globset` 负责 include/exclude path filter。
  - `grep_searcher::Searcher` 负责高性能 line-oriented search、binary detection、context line 等。
  - `grep_regex::RegexMatcher` 负责 Rust regex matcher。
  - `grep_matcher::Matcher` 只在实现需要直接读取 match ranges 时作为 trait dependency。
- 只有在以后明确要求“完全复刻 `rg` CLI 行为”时，才考虑像 opencode 一样 bundle/resolve `rg` binary 并解析 `--json` 输出。

## `ai-chat2` 分阶段工具范围

### V1.0：本地文件系统闭环

必须实现：

- `read_file`
- `list_directory`
- `find_path`
- `grep`
- `write_file`
- `edit_file`

目标：

- 让模型能在 project 中读、查、改。
- project 目录内默认执行。
- external read 默认执行。
- external write 在 `RequestApproval` 下弹审批。
- 每个工具都有专门 timeline row。

### V1.1：命令执行

候选实现：

- `run_command`

约束：

- 默认 `cwd` 必须在 project root 内。
- 首版不解析 shell AST。
- 不推断命令可能写哪些外部路径，除非 tool input 显式声明 `write_paths`。
- 如果要做 Zed/opencode 级别的 shell path inference，需要单独设计 tree-sitter shell/powershell 解析、sandbox escalation、灾难命令硬拦截和 UI 文案。

### V2：IDE/code intelligence 工具

候选实现：

- diagnostics
- go to definition
- find references
- rename
- get/apply code actions

约束：

- 这类能力需要接 app/project/LSP runtime，不应放在 `crates/ai-chat-agent` 的纯本地文件工具里硬做。
- UI 上应和文件工具共用 tool timeline row，但数据源和错误模型独立。

### V3：网络、任务和多 agent 工具

候选实现：

- web search
- web fetch
- task/sub-agent
- todo/plan/question

约束：

- 网络工具需要独立 network approval。
- task/sub-agent 需要新的 run ownership、parent/child relationship 和 sidebar/timeline 展示。
- plan/todo/question 更像 conversation control tools，不应和本地文件工具混在同一个模块里。

## 首版产品决策

首版使用以下默认权限策略：

- project 目录内：所有本地内置工具默认可以执行。
- 外部目录：读操作不需要审批。
- 外部目录：写操作需要审批。
- 审批模式控制放在 ChatForm footer。
- 用户提交消息时，把审批模式写入本次 run settings snapshot。
- 本 issue 不新增全局 Settings 页面。

这和 Codex 对 `.git`、`.agents`、`.codex` 的 metadata carveout 不完全一致：按当前产品决策，project 目录内写入默认允许。如果以后要做 protected metadata 例外，应作为单独决策。

## Skills 和内置工具的优先级

Skills 和内置工具不是同一层。

- Skills 是加载进模型 prompt 的指令/上下文包。
- 内置工具是注册进 tool registry 的可执行能力。
- Skill activation 应发生在模型选择 tool 之前，这样模型能学会如何使用本地工具。
- 审批器包住内置工具执行。skill 不能绕过审批，也不能授予路径权限。
- 如果 skill 要求模型搜索或编辑文件，真正执行仍走内置工具，并使用同一套 path policy。
- 如果 MCP tool 和本地内置工具都能完成同一件文件系统/project 操作，优先用本地内置工具，因为它有 app 自己的持久化、preview 和审批 UI。

优先级顺序：

1. system/developer/user 指令。
2. runtime approval 和 path policy。
3. skill 提供的任务指导。
4. built-in、MCP 或 provider-hosted tool 的具体选择。

## 审批模式

ChatForm selector 暴露三个选项：

| UI 文案 | 内部枚举 | 行为 |
| --- | --- | --- |
| 替我审批 | `AutoApprove` | 使用默认 path policy，并自动批准该 policy 下本来会询问的操作。仍记录 `ApprovalDecision`，`decided_by = "auto"`。 |
| 请求批准 | `RequestApproval` | 默认值。project 目录内操作直接执行；外部读直接执行；外部写创建 approval request 并暂停 run。 |
| 完全访问权限 | `FullAccess` | 不因路径访问弹审批。tool call 仍正常产生 tool-call/tool-result UI 和持久化记录。 |

当前决策：V1.0 没有 `run_command`，因此不做 Zed 风格灾难命令硬拦截。以后实现命令执行时必须重新设计该规则。

## 数据模型变更

### `crates/ai-chat-core/src/payloads.rs`

扩展 `ToolPolicySnapshot`，不引入独立设置表。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalMode {
    AutoApprove,
    RequestApproval,
    FullAccess,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPermissionScopeSnapshot {
    pub project_roots: Vec<String>,
    pub external_read_requires_approval: bool,
    pub external_write_requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPolicySnapshot {
    pub approval_policy: ToolApprovalPolicy,
    pub enabled_sources: Vec<ToolSource>,
    pub max_steps: u32,
    #[serde(default = "default_tool_approval_mode")]
    pub approval_mode: ToolApprovalMode,
    #[serde(default)]
    pub permission_scope: Option<ToolPermissionScopeSnapshot>,
}
```

同时扩展审批 payload，让 UI 不需要解析 `arguments_preview`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAccessKind {
    Read,
    Write,
    Execute,
    Network,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolAccessRequestPayload {
    pub kind: ToolAccessKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<String>,
    #[serde(default)]
    pub within_project: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequestPayload {
    pub reason: String,
    pub tool_source: ToolSource,
    pub tool_name: String,
    pub arguments_preview: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub access_requests: Vec<ToolAccessRequestPayload>,
}
```

规则：

- `approval_policy` 保留给 provider/MCP compatibility。
- `approval_mode` 驱动产品级内置工具审批。
- `permission_scope` 是 run snapshot，不是可变全局状态。
- 新字段必须有 serde default，保证已存在的历史 row 能读取。
- `ApprovalRequestPayload.access_requests` 只用于结构化 UI 和审计；真正执行仍以 `tool_invocations.input.arguments` 作为 source of truth。

### `crates/ai-chat-agent/src/types.rs`

首版继续使用 `AgentRunRequest.project_root` 作为主 project root。

如果以后 app 真正支持 multi-root project，再新增：

```rust
pub project_roots: Vec<PathBuf>,
```

不要在 app 还不能提供多个 project root 前提前引入多 root 状态。

### `crates/ai-chat-agent/src/builtin_tools.rs`

新增模块入口文件，禁止使用 `mod.rs`。

```rust
pub mod approval;
pub mod command;
pub mod filesystem;
pub mod registry;
pub mod search;
pub mod types;
```

子模块结构：

- `builtin_tools/types.rs`
  - `BuiltinToolName`
  - `BuiltinToolInput`
  - `BuiltinToolOutput`
  - `PathAccessKind`
  - `PathAccessRequest`
- `builtin_tools/approval.rs`
  - `ToolPermissionEvaluator`
  - `ToolPermissionDecision::{Allow, Ask, Deny}`
  - path normalize 和 project-root containment 检查。
- `builtin_tools/filesystem.rs`
  - `read_file`
  - `list_directory`
  - `write_file`
  - `edit_file` 或 `apply_patch`
- `builtin_tools/search.rs`
  - `find_path`
  - `grep`
- `builtin_tools/command.rs`
  - V1.1 候选 `run_command`。
  - 不在 V1.0 创建该文件，除非确认命令执行也进入第一 PR。
  - V1.1 也不做 tree-sitter shell path inference。
- `builtin_tools/registry.rs`
  - 把启用的内置工具注册为 `ToolSource::Local`。

### 工具定义

V1.0 必做工具：

| Tool | 操作类型 | 审批行为 |
| --- | --- | --- |
| `read_file` | read | 始终允许，包括外部路径 |
| `list_directory` | read | 始终允许，包括外部路径 |
| `find_path` | read/search | 始终允许，包括外部路径 |
| `grep` | read/search | 始终允许，包括外部路径 |
| `write_file` | write | project root 内允许；外部路径在 `RequestApproval` 下询问，`AutoApprove` / `FullAccess` 下允许 |
| `edit_file` | write | project root 内允许；外部路径在 `RequestApproval` 下询问，`AutoApprove` / `FullAccess` 下允许 |

V1.1 候选工具：

| Tool | 操作类型 | 审批行为 |
| --- | --- | --- |
| `run_command` | exec | cwd 在 project root 内默认允许；外部写入推断先不做，除非 tool input 显式声明 write paths |

当前决策：`run_command` 不放进 V1.0，留给 V1.1 单独实现。

### V1 工具 input/output schema

所有 input/output 类型放在 `crates/ai-chat-agent/src/builtin_tools/types.rs`。`ToolDefinition.parameters` 使用这些类型对应的 JSON schema，字段名统一 `camelCase`，反序列化使用 `deny_unknown_fields`。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinToolName {
    ReadFile,
    ListDirectory,
    FindPath,
    Grep,
    WriteFile,
    EditFile,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadFileInput {
    pub path: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub max_lines: Option<u32>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileOutput {
    pub path: String,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub total_lines: u32,
    pub truncated: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListDirectoryInput {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub max_entries: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryOutput {
    pub path: String,
    pub kind: FileEntryKind,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDirectoryOutput {
    pub path: String,
    pub entries: Vec<DirectoryEntryOutput>,
    pub truncated: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindPathInput {
    pub query: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSearchMatchOutput {
    pub path: String,
    pub kind: FileEntryKind,
    pub score: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindPathOutput {
    pub query: String,
    pub matches: Vec<PathSearchMatchOutput>,
    pub truncated: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrepInput {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub context_lines: Option<u32>,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRangeOutput {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepMatchOutput {
    pub path: String,
    pub line_number: u32,
    pub line: String,
    pub ranges: Vec<TextRangeOutput>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepOutput {
    pub pattern: String,
    pub matches: Vec<GrepMatchOutput>,
    pub truncated: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub create_parent_dirs: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileOutput {
    pub path: String,
    pub created: bool,
    pub bytes_written: u64,
    pub diff: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditFileInput {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    #[serde(default)]
    pub replace_all: bool,
    #[serde(default)]
    pub expected_replacements: Option<u32>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditFileOutput {
    pub path: String,
    pub replacements: u32,
    pub diff: String,
}
```

`FileEntryKind` 放在同一文件：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}
```

工具输出写入方式：

- `ToolInvocationOutput.content` 放简短文本 summary，给模型继续推理使用。
- `ToolInvocationOutput.structured_output` 放完整 typed output JSON，给 UI 展开详情使用。
- `ToolInvocationOutput.raw_output` 首版不用。
- 超过 limit 的输出必须设置 `truncated = true`，不能把大文件或大搜索结果一次性塞进 timeline。

## Agent 数据流

1. ChatForm 渲染 provider/model/reasoning selector 和新的 approval mode selector。
2. 用户提交消息。
3. `ChatFormSubmit` 携带 `approval_mode`。
4. `app/ai-chat2/src/state/conversations.rs` 构造 `ToolPolicySnapshot`：
   - `enabled_sources = vec![ToolSource::Local]`
   - `approval_mode` 来自 ChatForm
   - `permission_scope.project_roots = [project.path]`
   - `permission_scope.external_read_requires_approval = false`
   - `permission_scope.external_write_requires_approval = true`
5. `AgentRunRequest` 收到 settings snapshot 和 `project_root`。
6. `crates/ai-chat-agent/src/runtime.rs` 通过 `builtin_tools::registry` 注册本地内置工具。
7. `PersistingPromptHook::on_tool_call` 收到模型 tool call 后先插入 `tool_invocations` 和 `ToolCall` conversation item。
8. 如果 tool 是 `ToolSource::Local` 且属于 `BuiltinToolName`，用 tool arguments 生成 `PathAccessRequest`，并先 normalize path。
9. `ToolPermissionEvaluator` 返回：
   - `Allow`：立即执行，并持久化 `ToolCall` 和 `ToolResult`。
   - `Ask`：持久化 `ApprovalRequest`，把 run 标为 waiting，并在 UI 暴露审批动作。
   - `Deny`：持久化 error `ToolResult`，带结构化 denial reason。
10. 如果不是本地内置工具，继续使用现有 `ToolRunPolicy.approval_policy` 静态审批逻辑。
11. conversation UI 处理 approve/deny。
12. 外部写入被批准后，执行 approved resume flow，并持久化 `ApprovalDecision`、`ToolResult`、后续 assistant output。

### 动态审批和 approved resume

静态 `ToolRunPolicy.approval_policy` 不足以表达本 issue 的规则。原因是 `write_file` / `edit_file` 只有 external write 需要审批，project root 内写入应直接执行；如果把整个 tool 静态设成 `OnRequest`，project 内写也会弹审批。

首版规则：

- V1 本地内置工具的 `ToolRunPolicy.approval_policy` 保持 `Never`。
- path-based approval 在 `PersistingPromptHook::on_tool_call` 中动态执行。
- MCP/provider-hosted/custom tools 继续走静态 `ToolRunPolicy`，不混入本地文件工具的 path policy。

动态审批伪代码：

```rust
let invocation = insert_tool_invocation(...);
append_tool_call_item(...);

if let Some(builtin) = BuiltinToolName::from_tool_name(&definition.tool_name) {
    let access = builtin.path_access_requests(&arguments)?;
    match evaluator.evaluate(&access, &run_settings.tool_policy) {
        ToolPermissionDecision::Allow => execute_tool_invocation(...).await,
        ToolPermissionDecision::Ask { reason, access_requests } => {
            mark_tool_invocation_awaiting_approval(&invocation.id);
            append_approval_request(invocation.id, reason, access_requests);
            update_agent_run_status(WaitingForApproval);
            return ToolCallHookAction::terminate("tool approval required");
        }
        ToolPermissionDecision::Deny { reason } => append_error_tool_result(...),
    }
}
```

approved resume 必须替换当前 `crates/ai-chat-agent/src/runtime.rs` 中 `decide_approval(Approved)` 的 unsupported 分支。

新增 agent runtime 入口：

```rust
pub async fn approve_and_resume_tool(
    &self,
    approval_decision_id: &str,
    decided_by: String,
    reason: Option<String>,
    observer: AgentRuntimeObserver,
) -> Result<ApprovalResumeOutcome>;

pub enum ApprovalResumeOutcome {
    ToolCompleted {
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
    ToolFailed {
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
        error: RunErrorPayload,
    },
}
```

approved resume 步骤：

1. 读取 `approval_decisions`、`tool_invocations`、`agent_runs`。
2. 校验 approval 仍是 `Pending`，tool invocation 仍是 `AwaitingApproval`。
3. 更新 approval 为 approved，并 append `ApprovalDecision` conversation item。
4. 只允许恢复 `ToolSource::Local` 且 `BuiltinToolName` 可识别的工具；非本地工具继续返回 explicit unsupported error，避免伪造 MCP/provider executor。
5. 从 `tool_invocations.input.arguments` 反序列化 typed input。
6. 用 `agent_runs.input.settings_snapshot.tool_policy` 和 project root 重新构造 `ToolPermissionEvaluator`，再次确认本次 approved 只覆盖同一 pending access。
7. 更新 tool invocation 为 `Running`，执行对应 built-in executor。
8. 成功时写入 `ToolResult`，更新 invocation 为 `Succeeded`。
9. 失败时写入 error `ToolResult`，更新 invocation 为 `Failed`。
10. 如果 tool 成功，创建一个 `AgentRunTriggerKind::Resume` 的后续 run，让模型读取已有 `ToolCall` + `ApprovalDecision` + `ToolResult` 后继续回答。

`app/ai-chat2/src/state/conversation_runtime.rs` 新增方法：

```rust
pub(crate) fn approve_tool_invocation(
    &mut self,
    conversation_id: ConversationId,
    approval_decision_id: ApprovalDecisionId,
    window: &mut Window,
    cx: &mut Context<Self>,
);

pub(crate) fn deny_tool_invocation(
    &mut self,
    conversation_id: ConversationId,
    approval_decision_id: ApprovalDecisionId,
    reason: Option<String>,
    cx: &mut Context<Self>,
);
```

UI 调用 approve 后，runtime 先执行 approved tool，再启动 resume run；期间发 `ConversationRuntimeEvent::ConversationChanged`。deny 继续用现有 `AgentRuntime::decide_approval(Denied)` 语义即可，但要补 UI action wiring。

## 全局状态和持久化

不新增全局 settings store。

- Runtime state：
  - `app/ai-chat2/src/state/conversation_runtime.rs` 拥有 active runs、cancel 和 pending approval wait state。
  - 审批模式从 run snapshot 读取，不读可变全局设置。
- ChatForm state：
  - `selected_approval_mode` 是本地 UI state。
  - 默认值是 `RequestApproval`。
  - submit 后，所选模式写进 run snapshot。
- Conversation state：
  - 为了 UX 连续性，可以在当前 mounted conversation 内用内存保存上一次选择。
  - 不在 app restart 后恢复该选择，除非另有产品决策要求 per conversation/project sticky。

## 数据获取方式

不新增独立 query/cache 层，继续复用当前 conversation snapshot 加载路径。

读取路径：

1. `ConversationDetailPage` 通过 `load_snapshot(&conversation_id, cx)` 读取 `ConversationLoadSnapshot`。
2. snapshot 已包含 conversation items、agent runs、tool invocations、approval decisions。
3. `timeline::ConversationTimelineRows` 只把 item 排成 timeline row，不主动查询数据库。
4. `tool_blocks.rs` 从 row 对应的 `ConversationItemRecord` 读取 payload，并通过 `tool_invocation_id` 在 snapshot 内找 `ToolInvocationRecord`。
5. tool output preview 优先读 `ToolInvocationRecord.output.structured_output`；没有结构化输出时退回 `ToolResultItem.content`。
6. approval row 优先读 `ApprovalDecisionRecord` 的 status/decision；payload 只作为渲染 fallback。

写入路径：

1. ChatForm submit 只创建 run request，不直接写 tool 状态。
2. tool call、tool result、approval request、approval decision 全部由 `ai-chat-agent` runtime/persistence 写入。
3. UI approve/deny button 只调用 `ConversationRuntimeStore` 方法，不直接写 DB。
4. `ConversationRuntimeEvent::ConversationChanged` 触发 `ConversationDetailPage` 重新 `load_snapshot`，保持单一数据来源。

不做：

- 不在 `tool_blocks.rs` 内单独查库。
- 不在 UI 里缓存 tool output 的反序列化结果作为长期状态。
- 不新增全局 approval store。

## 数据库变更

首版目标：不做 migration。

复用现有存储：

- `agent_runs.settings_snapshot` 存扩展后的 `RunSettingsSnapshot`。
- `conversations.settings_snapshot` 存扩展后的 `ConversationSettingsSnapshot`。
- `tool_invocations` 存 arguments/output/status。
- `approval_decisions` 存用户或自动决策。
- conversation items 存 `ApprovalRequest` 和 `ApprovalDecision` payload。

必须做的兼容工作：

- `ToolPolicySnapshot` 新字段都加 serde default。
- `ApprovalRequestPayload.access_requests` 加 serde default。
- 增加旧 JSON 缺少新字段时的 round-trip 测试。
- 增加 `approval_mode` 和 `permission_scope` 的新 JSON round-trip 测试。
- 增加旧 approval request JSON 缺少 `access_requests` 时的 round-trip 测试。

不做 migration 的前提：

- pending tool resume 能从 `tool_invocations.input.arguments`、`tool_invocations.input.tool_name`、`agent_runs.input.settings_snapshot` 和 `agent_runs.input.runtime_snapshot` 重构执行上下文。
- UI 展示需要的信息能从 `approval_decisions.request.access_requests` 或旧数据的 `arguments_preview` fallback 获得。
- 不需要把 ChatForm approval mode 作为跨重启 preference 保存。

只有在确认审批模式需要跨重启按 conversation/project sticky 时，才新增 migration。

## UI 计划

### ChatForm 审批 selector

新增文件：

- `app/ai-chat2/src/components/chat_form/approval_select.rs`

复用 `components/chat_form/effort_select.rs` 的 picker 模式：

- `PickerListDelegate`
- `PickerSection`
- `picker_trigger`
- `picker_popover`
- `SelectItem`

类型：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalModeChoice {
    AutoApprove,
    RequestApproval,
    FullAccess,
}

pub struct ApprovalModeOption {
    pub mode: ApprovalModeChoice,
    pub label_key: &'static str,
    pub description_key: &'static str,
    pub icon: IconName,
}
```

`approval_select.rs` 对外暴露：

```rust
pub(crate) fn approval_mode_options() -> Vec<ApprovalModeOption>;

pub(crate) fn approval_mode_picker_trigger(
    selected: ApprovalModeChoice,
    open: bool,
    cx: &mut App,
) -> impl IntoElement;

pub(crate) fn approval_mode_picker_popover(
    picker: Entity<PickerListDelegate<ApprovalModeOption>>,
    on_select: impl Fn(ApprovalModeChoice, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement;
```

使用组件：

- `PickerListDelegate` / `PickerSection` / `SelectItem`：选项列表。
- `picker_trigger` / `picker_popover`：触发器和弹层。
- `Icon`：`ShieldCheck`、`ShieldAlert`、`LockOpen`。
- `Label`：当前模式 label。

`ChatForm` 改动：

- 增加 `selected_approval_mode: ApprovalModeChoice`。
- 增加 `approval_mode_picker_open: bool`。
- 增加 `approval_mode_picker: Entity<PickerListDelegate<ApprovalModeOption>>`。
- `ChatFormSubmit` 增加 `approval_mode`。
- footer 中在 model/reasoning controls 旁边渲染 selector。

### Conversation tool UI

新增文件：

- `app/ai-chat2/src/components/conversation_detail/tool_blocks.rs`

同步改动：

- `app/ai-chat2/src/components/conversation_detail.rs`
  - 增加 `mod tool_blocks;`
  - 在 `render_timeline_row` 中把 `ConversationItemPayload::ToolCall`、`ToolResult`、`ApprovalRequest`、`ApprovalDecision` 分派到 `tool_blocks`，不再走通用 `message::detail_block`。
- `app/ai-chat2/src/components/conversation_detail/timeline.rs`
  - row metadata 保留 `ConversationItemId`、`AgentRunId`、tool/approval item payload。
  - 不在 timeline 层决定 icon；timeline 只负责分组和折叠状态。

为每个 tool/approval item 渲染专门 row，不继续只用通用 `detail_block`。

默认折叠层级：

- Agent run 外层 block：
  - 已有 final assistant answer 时默认折叠。
  - running、failed、waiting approval 时默认展开。
- 展开的 agent run 内部：
  - model output row 正常渲染。
  - 每个 tool call 作为一条 summary row 可见。
  - tool call arguments 和 output 默认折叠。
  - approval request 在等待用户时展开，决策后折叠。

这比 Zed 的 thread UI 更接近 Codex app 的交互模型，同时保留 Zed 的关键点：审批动作就在 tool row 内。

### Codex app 图标参考

已解包 `/Applications/Codex.app/Contents/Resources/app.asar` 并核验：

- Codex app 不是给所有工具调用使用一个 generic tool icon，而是按工具类别使用独立资源。
- bundle 中存在 `code-searching-icon-*`、`searching_animation-*`、`run_command_animation-*`、`run-command-*`、`terminal-*`、`file-diff-*`、`edit-*`、`edit_files_animation-*`、`document-search-*`、`web-search-icon-*`、`web-search-favicon-icon-*`、`mcp-tool-item-content-utils-*`、`patch-item-content-*`。
- 本地 `/Applications/Codex.app/Contents/Resources/rg` 是内置 Mach-O 可执行文件，说明 Codex 对代码搜索不是手写 regex 遍历；我们的 `grep` 工具名可以保留，但实现必须是 ripgrep-backed。
- Codex collapsed summary 的文案按工具动作聚合：`Loaded a tool`、`Searched code`、`Listed files`、`Created a file`、`Edited a file`、`Deleted a file`、`Running a command`。这些类别应该直接映射到我们的 `ToolTimelineKind`，否则 UI 无法稳定选择 icon 和文案。

因此 ai-chat2 的实现约束是：每个 model-facing tool kind 必须有稳定的 `ToolVisualSpec`，不能在 render 时用字符串临时匹配 icon。

Tool row 类型：

```rust
pub struct ToolTimelineBlock {
    pub item_id: ConversationItemId,
    pub invocation_id: Option<ToolInvocationId>,
    pub kind: ToolTimelineKind,
    pub status: ToolTimelineStatus,
    pub expanded: bool,
}

pub enum ToolTimelineKind {
    ReadFile,
    ListDirectory,
    FindPath,
    Grep,
    WriteFile,
    EditFile,
    ApplyPatch,
    DeleteFile,
    RunCommand,
    WebSearch,
    WebFetch,
    Skill,
    ApprovalRequest,
    ApprovalDecision,
    UnknownLocalTool,
    McpTool,
    ProviderHostedTool,
}

pub enum ToolTimelineStatus {
    Pending,
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Denied,
    Canceled,
}

pub enum ToolTimelineTone {
    Neutral,
    Search,
    Read,
    Write,
    Execute,
    Network,
    Approval,
    Danger,
}

pub struct ToolVisualSpec {
    pub icon: IconName,
    pub status_icon: Option<IconName>,
    pub label_key: &'static str,
    pub running_label_key: Option<&'static str>,
    pub tone: ToolTimelineTone,
    pub default_expanded: bool,
}
```

组件结构：

```rust
pub(crate) fn render_tool_timeline_block(
    block: ToolTimelineBlock,
    item: &ConversationItemRecord,
    invocation: Option<&ToolInvocationRecord>,
    approval: Option<&ApprovalDecisionRecord>,
    runtime: Entity<ConversationRuntimeStore>,
    window: &mut Window,
    cx: &mut Context<ConversationDetailPage>,
) -> impl IntoElement;

fn render_tool_summary_row(
    spec: ToolVisualSpec,
    title: SharedString,
    subtitle: Option<SharedString>,
    status_label: Option<SharedString>,
    expanded: bool,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement;

fn render_tool_body(
    item: &ConversationItemRecord,
    invocation: Option<&ToolInvocationRecord>,
    spec: ToolVisualSpec,
    cx: &mut App,
) -> impl IntoElement;

fn render_approval_actions(
    conversation_id: ConversationId,
    approval_decision_id: ApprovalDecisionId,
    runtime: Entity<ConversationRuntimeStore>,
) -> impl IntoElement;
```

使用组件：

- `gpui_component::button::Button`：approve / deny。
- `gpui_component::Icon`：summary icon、status icon、expanded body section icon。
- `gpui_component::label::Label`：summary title/status。
- `v_flex` / `h_flex` / `StyledExt`：布局。
- 继续使用现有 timeline list，不新增 nested card。

自定义 UI 结构：

- summary row 高度固定，左侧 icon 16px，右侧可选 status icon 12px。
- expanded body 分 section：`Arguments`、`Output`、`Diff`、`Approval`。
- `Arguments` 默认以 compact JSON preview 展示，超长时截断并保留完整 JSON 在 copy/action 后续能力。
- `Output` 优先展示 structured output 的 typed preview；没有 typed preview 时展示 text content。
- `Diff` 只对 `WriteFile`、`EditFile`、`ApplyPatch` 渲染。
- waiting approval 的 action buttons 只出现在 `ApprovalRequest` row expanded body 内。

`tool_blocks.rs` 必须提供：

```rust
fn tool_visual_spec(kind: ToolTimelineKind, status: ToolTimelineStatus) -> ToolVisualSpec;
fn summarize_tool_row(item: &ConversationItemRecord, locale: &I18n) -> SharedString;
```

`ToolTimelineKind` 来源：

- 本地内置工具：由 `ToolDefinition.name` 显式映射，不从自然语言 summary 反推。
- provider hosted tool：由 provider output item 的 tool type 映射。
- MCP/dynamic tool：默认 `McpTool`，允许未来 tool metadata 带 `icon_hint` 后覆盖为更具体类别。
- approval：由 `ConversationItemPayload::ApprovalRequest` / `ApprovalDecision` 直接映射。

渲染细节：

- Summary row：icon、tool name、path/command summary、status，有可用数据时显示 duration。
- Summary row 左侧 icon 固定 16px 宽高，和 Codex app 一样始终可见。
- status 不替换工具类别 icon；running/failed/denied 用右侧 status icon 或 badge 表示，避免运行中 icon 跳动导致用户失去工具类别信息。
- Expanded body：
  - arguments JSON 或结构化 preview。
  - output preview。
  - write/edit tool 显示 diff preview。
  - approval reason 和 pending approval 的 action buttons。
- 审批动作：
  - Approve
  - Deny
  - 后续可选：approve for conversation / always for project。

## Icons

在 `app/ai-chat2/src/foundation/assets.rs` 新增 app-local Lucide variants。GPUI 侧必须使用 `IconName`，不要在 `tool_blocks.rs` 内直接拼 SVG path。

### 新增 `IconName`

需要新增：

- `ShieldCheck => "shield-check"`
- `ShieldAlert => "shield-alert"`
- `LockOpen => "lock-open"`
- `Terminal => "terminal"`
- `FileText => "file-text"`
- `FolderTree => "folder-tree"`
- `FolderSearch => "folder-search"`
- `Regex => "regex"`
- `FilePlus => "file-plus"`
- `FileDiff => "file-diff"`
- `Braces => "braces"`
- `CodeXml => "code-xml"`
- `ScrollText => "scroll-text"`
- `LoaderCircle => "loader-circle"`
- `CircleX => "circle-x"`
- `Globe => "globe"`
- `Plug => "plug"`
- `Wrench => "wrench"`

这些 Lucide slug 已确认存在于 `third_party/lucide/icons/`。

审批 selector：

- `ShieldCheck`：替我审批
- `ShieldAlert`：请求批准
- `LockOpen`：完全访问权限

### Tool row icon 映射

| `ToolTimelineKind` | summary icon | status icon | tone | Codex app 对照 | 备注 |
| --- | --- | --- | --- | --- | --- |
| `ReadFile` | `FileText` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Read` | `document-search-*`、文件资源 icon | 单文件读取；多文件读取 summary 使用 plural 文案。 |
| `ListDirectory` | `FolderTree` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Read` | `folders-*`、`folder-*` | 展开后显示路径、limit、是否递归。 |
| `FindPath` | `FolderSearch` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Search` | `document-search-*` | 文件名/路径搜索。 |
| `Grep` | `Search` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Search` | `code-searching-icon-*`、`searching_animation-*` | model-facing 名字仍叫 `grep`；实现是 ripgrep-backed。展开参数里的 pattern chip 使用 `Regex`。 |
| `WriteFile` | `FilePlus` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Write` | `edit_files_animation-*` | 新建文件。 |
| `EditFile` | `Pencil` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Write` | 截图中的编辑 row 使用 pencil 类图标，bundle 有 `edit-*` | 修改已有文件。 |
| `ApplyPatch` | `FileDiff` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Write` | `file-diff-*`、`patch-item-content-*` | 展开 body 默认显示 diff preview。 |
| `DeleteFile` | `Trash` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Danger` | collapsed summary 有 `Deleted a file` 类别 | V1 可不开放给模型；UI 类型先保留。 |
| `RunCommand` | `Terminal` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Execute` | `run-command-*`、`run_command_animation-*`、`terminal-*` | V1.1 实现；命令、cwd、exit code 放 expanded body。 |
| `WebSearch` | `Globe` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Network` | `web-search-icon-*`、`web-search-favicon-icon-*` | V3；如果只做本地工具，先保留映射不注册工具。 |
| `WebFetch` | `Globe` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Network` | web resource card / favicon 资源 | V3。 |
| `Skill` | `Sparkles` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Neutral` | collapsed summary 有 `Loaded a tool`，bundle 有 `skills-*` | 加载 skills 或工具定义。 |
| `ApprovalRequest` | `ShieldAlert` | `LoaderCircle` when waiting | `Approval` | Zed 审批在 tool row 内；Codex app 也把状态 row 放在时间线 | waiting 时默认展开。 |
| `ApprovalDecision` approved | `ShieldCheck` | `CircleCheck` | `Approval` | approval result row | 决策后默认折叠。 |
| `ApprovalDecision` denied | `ShieldAlert` | `CircleX` | `Danger` | approval result row | 必须显示拒绝原因。 |
| `McpTool` | `Plug` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Neutral` | `mcp-tool-item-content-utils-*` | MCP tool 没有明确类别时使用。 |
| `ProviderHostedTool` | `Cloud` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Network` | provider hosted/web resource | 由 provider output item 映射。 |
| `UnknownLocalTool` | `Wrench` | `LoaderCircle` / `CircleCheck` / `CircleX` | `Neutral` | generic tool fallback | 只能作为 fallback；新增内置工具必须补表。 |

`ToolTimelineStatus` 到 status icon：

- `Pending`：无 status icon。
- `Running`：`LoaderCircle`。
- `WaitingApproval`：`ShieldAlert`。
- `Succeeded`：`CircleCheck`。
- `Failed`：`CircleX`。
- `Denied`：`CircleX`。
- `Canceled`：`Square`。

辅助 icon：

- `Braces`：arguments JSON 标题。
- `CodeXml`：structured output 标题。
- `ScrollText`：approval reason / long text detail 标题。
- `Regex`：`grep` expanded body 中的 pattern 参数，不作为 summary 主 icon。

必须补测试：所有 `ToolTimelineKind` 都能返回非 fallback `ToolVisualSpec`，除 `UnknownLocalTool` 外不得返回 `Wrench`。

## i18n

更新：

- `app/ai-chat2/locales/en-US/main.ftl`
- `app/ai-chat2/locales/zh-CN/main.ftl`

需要新增 keys：

```text
chat-form-approval-mode-label
chat-form-approval-mode-auto
chat-form-approval-mode-auto-description
chat-form-approval-mode-request
chat-form-approval-mode-request-description
chat-form-approval-mode-full-access
chat-form-approval-mode-full-access-description
tool-block-read-file
tool-block-list-directory
tool-block-find-path
tool-block-grep
tool-block-grep-running
tool-block-write-file
tool-block-write-file-running
tool-block-edit-file
tool-block-edit-file-running
tool-block-apply-patch
tool-block-apply-patch-running
tool-block-delete-file
tool-block-delete-file-running
tool-block-run-command
tool-block-run-command-running
tool-block-web-search
tool-block-web-search-running
tool-block-web-fetch
tool-block-web-fetch-running
tool-block-loaded-tool
tool-block-loading-tool
tool-block-mcp-tool
tool-block-provider-hosted-tool
tool-block-unknown-local-tool
tool-block-status-pending
tool-block-status-running
tool-block-status-waiting-approval
tool-block-status-succeeded
tool-block-status-failed
tool-block-status-denied
tool-block-status-canceled
tool-block-approval-request
tool-block-approval-approved
tool-block-approval-denied
tool-block-approve
tool-block-deny
```

主 UI 不添加大段说明文字，只保留 picker label/description 和 tool row status。

需要 plural 的 key 用 Fluent 参数，不拆成多个硬编码字符串：

```text
tool-block-read-files =
    { $count ->
        [one] Read a file
       *[other] Read { $count } files
    }
tool-block-edit-files =
    { $count ->
        [one] Edited a file
       *[other] Edited { $count } files
    }
```

中文同样保留 `$count`，例如 `已读取 { $count } 个文件`，避免英文 plural 逻辑混进 UI 代码。

## 依赖

复杂能力使用现成库。

计划给 `crates/ai-chat-agent/Cargo.toml` 增加直接依赖：

- `ignore = "0.4.25"`
  - 目录遍历，支持 `.gitignore`。
- `globset = "0.4.18"`
  - include/exclude path matching。
- `grep-searcher = "0.1.16"`
  - ripgrep 的 line-oriented search engine；用于 `grep` 工具的内容搜索。
- `grep-regex = "0.1.14"`
  - ripgrep 的 Rust regex matcher adapter。
- `grep-matcher = "0.1.8"`
  - ripgrep matcher trait；实现需要读取 match ranges 时直接使用。
- `regex = "1.12.3"`
  - permission pattern、轻量校验和非 grep 搜索场景；不要作为 `grep` 工具的主搜索引擎。
- `similar = "2.7.0"`
  - write/edit approval UI 的 diff preview。

如果 `edit_file` 需要应用 unified patch，可选：

- `diffy = "0.4.2"`

如果 `run_command` 首版一起实现，可选：

- 给现有 `tokio` dependency 增加 `fs`、`process`、`io-util` features。
- `shlex = "1.3.0"`，仅用于展示或轻量 splitting。
- `which = "8.0.2"`，仅在需要显式 executable resolution 时添加。

首版不添加 tree-sitter shell 依赖。Zed 和 opencode 都说明可靠的 shell path inference 很复杂，本 issue 不手写也不一次性搬完整实现。

## 测试计划

Core：

- `ToolPolicySnapshot` 能反序列化没有 `approval_mode` 的旧 JSON。
- `ToolPolicySnapshot` 能 round-trip 三种 approval mode。
- `ApprovalRequestPayload` 能反序列化没有 `access_requests` 的旧 JSON。
- `ToolAccessRequestPayload` 能 round-trip read/write/execute/network 四类访问。
- `ToolPermissionEvaluator` 允许：
  - project-root read。
  - project-root write。
  - external read。
- `ToolPermissionEvaluator` 在 `RequestApproval` 下对 external write 返回 ask。
- `ToolPermissionEvaluator` 在以下模式允许 external write：
  - `AutoApprove`。
  - `FullAccess`。

Agent：

- 启用 local tools 时，built-in registry 注册 `ToolSource::Local` 工具。
- 每个 V1 tool 的 JSON schema 拒绝未知字段。
- `read_file` 和 `grep` 对 external read path 不触发审批。
- `grep` 使用 ripgrep-backed engine：
  - respect `.gitignore`。
  - respect include glob。
  - invalid regex 返回结构化错误。
  - limit/page 不会把超大结果一次性写入 timeline。
- `write_file` 在 `RequestApproval` 下对 external path 创建 approval request。
- `write_file` 在 project root 内不创建 approval request。
- approved resume 能执行 pending local built-in write，并持久化 approval decision/tool result。
- approved resume 对 MCP/provider-hosted pending tool 返回 explicit unsupported，不伪造 executor。
- denied approval 能持久化 decision、error tool result，并把 run 置为 failed。

UI：

- ChatForm 默认 approval mode 是 `RequestApproval`。
- ChatForm submit 携带所选 approval mode。
- `IconName` 新增 variants 都能从 `AiChatAssets` 解析出 SVG。
- `tool_visual_spec_maps_every_known_tool_kind` 覆盖所有 `ToolTimelineKind`，除 `UnknownLocalTool` 外不得返回 `Wrench`。
- Tool block 的 arguments/output 默认折叠。
- Tool block 从 `ConversationLoadSnapshot` 渲染，不直接查询数据库。
- Approval request block 在 waiting 状态默认展开。
- Approve/deny action 能 dispatch 到 conversation runtime。

## 验证记录

2026-06-16 已执行：

```sh
cargo fmt
cargo test -p ai-chat-core tool_policy
cargo test -p ai-chat-agent builtin_tools
cargo test -p ai-chat2 approval_mode
cargo test -p ai-chat-agent streaming
cargo test -p ai-chat-agent recoverable
cargo test -p ai-chat-agent approval
cargo test -p ai-chat-agent tool_error_output_is_persisted_without_reconstructing_from_model_text
cargo test -p ai-chat-agent
cargo check -p ai-chat-agent -p ai-chat2
cargo clippy -p ai-chat-agent -p ai-chat2 --all-targets -- -D warnings
git diff --check
```

说明：后半段验证因默认 `target/` 当时存在 cargo 文件锁，使用 `CARGO_TARGET_DIR=/private/tmp/gpui-ai-chat-agent-check` 运行。`cargo check` / `cargo clippy` 只保留既有依赖 `block v0.1.6` future-incompat warning。

原计划验证命令：

```sh
cargo fmt
cargo test -p ai-chat-core tool_policy
cargo test -p ai-chat-agent builtin_tools
cargo test -p ai-chat2 approval_mode
cargo check -p ai-chat-agent -p ai-chat2
cargo clippy --all-targets --all-features -- -D warnings
```

## 实现顺序

1. 扩展 `ToolPolicySnapshot`，加入 approval mode 和 permission scope defaults。
2. 扩展 `ApprovalRequestPayload.access_requests`，补旧 JSON 兼容测试。
3. 新增 ChatForm approval selector，并打通 submit 数据。
4. 扩展 `IconName`，并新增 `tool_blocks.rs` 的 `ToolTimelineKind` / `ToolVisualSpec` 映射测试。
5. 新增 V1 tool input/output types 和 JSON schema。
6. 新增 `ToolPermissionEvaluator`，并把动态 path approval 接进 `PersistingPromptHook::on_tool_call`。
7. 在 run request 中启用 `ToolSource::Local` 内置工具。
8. 实现 filesystem/search tools。
9. 实现 external write approval request 的持久化和 approved resume。
10. 新增专门 tool timeline UI 和 approval action buttons。
11. 文件/搜索/编辑路径测试稳定后，再加入 command execution。

## 已决策和待确认问题

已决策：

1. `run_command` 不放进 V1.0，留 V1.1。
2. V1.0 不做 Zed 风格灾难命令硬拦截；如果 V1.1 实现命令执行，再重新设计。
3. ChatForm approval 选择只在本地 UI 状态和 run snapshot 中保持，不按 conversation/project 跨重启持久化。
4. 审批决策首版只支持单次 approve/deny，不做“本对话一直允许”或“本项目一直允许”。

仍待确认：

1. V1.1 `run_command` 是否需要 shell AST、灾难命令硬拦截和 sandbox escalation，还是只做显式 cwd/write paths 的轻量版本。
2. MCP/provider-hosted/custom tools 是否复用同一 approval selector，还是引入单独的 source-specific policy。
3. Tool structured output 是否需要按工具类型做专门 preview/editor，还是继续使用 compact JSON/text preview。
