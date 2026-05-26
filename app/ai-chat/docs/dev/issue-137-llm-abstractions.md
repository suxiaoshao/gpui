# Issue #137 LLM 抽象协调文档

本文档是 `ai-chat` issue #137 LLM 抽象工作的临时协调文档。
在集成分支活跃期间，它是跨会话的事实来源。
最终合入 `main` 前应删除本文档，除非剩余内容被提升为长期开发文档。

## 分支策略

- 集成分支：`codex/issue-137-llm-abstractions`
- 子 issue 分支必须从集成分支创建。
- 子 issue PR 应该指向 `codex/issue-137-llm-abstractions`，不要直接指向 `main`。
- 每个子 issue 合入后，集成分支必须保持可构建。
- #138、#142、#139 稳定共享抽象后，再从集成分支分阶段向 `main` 开 PR，以减少长期分支漂移。

## Issue 和分支映射

| Issue | 分支 | 目的 | 状态 |
| --- | --- | --- | --- |
| #137 | `codex/issue-137-llm-abstractions` | 父级集成工作 | 活跃 |
| #138 | `codex/issue-138-model-capabilities` | provider-neutral 模型能力类型 | 已通过 PR #147 合入集成分支；GitHub issue 仍未关闭 |
| #142 | `codex/issue-142-llm-items` | typed input、content、output items | 已通过 PR #148 合入集成分支；GitHub issue 仍未关闭 |
| #139 | `codex/issue-139-provider-runtime` | 基于 run 的 provider trait 和 events | 已通过 PR #149 合入集成分支；GitHub issue 仍未关闭 |
| #141 | `codex/issue-141-llm-persistence` | run state、output items、tools、attachments persistence | 已通过 PR #150 合入集成分支；GitHub issue 仍未关闭 |
| #143 | `codex/issue-143-openai-responses-abstraction` | 在共享抽象上迁移 OpenAI Responses | 已通过 PR #151 合入集成分支；GitHub issue 仍未关闭 |
| #144 | `codex/issue-144-ollama-shared-abstraction` | 在共享抽象上迁移 Ollama | 已通过 PR #152 合入集成分支；GitHub issue 仍未关闭 |
| #140 | `codex/issue-140-capability-gating` | template、shortcut、UI capability gating | 已通过 PR #153 合入集成分支；GitHub issue 仍未关闭 |
| #154 | `codex/issue-154-typed-message-content` | 合并 typed message content model | 已被 fresh database 方向取代；除非重新定义 scope，否则只保留为上下文 |
| #155 | `codex/issue-155-ai-chat2-crate-split` | 固定 `ai-chat2` 并行重构、crate 拆分、fresh database 和 typed data model | 设计文档已通过 PR #160 合入集成分支；GitHub issue 仍未关闭 |
| #156 | `codex/issue-156-fresh-db-bootstrap` | scaffold `ai-chat-core` / `ai-chat-db`，实现 fresh database bootstrap 和 typed repositories | 已通过 PR #161 合入集成分支；GitHub issue 仍未关闭 |
| #157 | `codex/issue-157-provider-runtime-crate` | 抽出 `ai-chat-provider`，沉淀 provider-neutral trait、capabilities 和 adapter 边界 | 待处理 |
| #158 | `codex/issue-158-agent-runtime-persistence` | 建立 `ai-chat-agent`，用 Rig + rmcp 实现 agent loop、file-backed skills、MCP/tool/approval runtime 和持久化写入 | 待处理 |
| #159 | `codex/issue-159-ai-chat2-ui` | 在 `app/ai-chat2` 渲染项目、对话、多步 reasoning、tool、approval、多模态 timeline | 待处理 |

## Issue 同步快照

最后同步时间：2026-05-26。

- #137 仍未关闭，是父级跟踪 issue。它的评论记录了子 issue 列表、集成分支和文档工作流。
- #138 仍未关闭，但 PR #147 已把 `codex/issue-138-model-capabilities` 合入 `codex/issue-137-llm-abstractions`。
- #142 仍未关闭，但 PR #148 已把 `codex/issue-142-llm-items` 合入 `codex/issue-137-llm-abstractions`。
- #139 仍未关闭，但 PR #149 已把 `codex/issue-139-provider-runtime` 合入 `codex/issue-137-llm-abstractions`。
- #141 仍未关闭，但 PR #150 已把 `codex/issue-141-llm-persistence` 合入 `codex/issue-137-llm-abstractions`。
- #143 仍未关闭，但 PR #151 已把 `codex/issue-143-openai-responses-abstraction` 合入 `codex/issue-137-llm-abstractions`。
- #144 仍未关闭，但 PR #152 已把 `codex/issue-144-ollama-shared-abstraction` 合入 `codex/issue-137-llm-abstractions`。
- #140 仍未关闭，但 PR #153 已把 `codex/issue-140-capability-gating` 合入 `codex/issue-137-llm-abstractions`。
- #154 来自 PR #153 review follow-up。它原本想在当前 `messages` schema 内合并 typed message content，但这个方向对 agent/multimodal 路线太受限。
- 当前首选方向是设计 fresh ai-chat database schema，不再继续在旧 `messages.content` / `send_content` / `input_content_parts` 模型上叠 migration 和兼容逻辑。
- #155 已通过 PR #160 把 `ai-chat2` 并行重构、crate 拆分、fresh database schema 和 typed data model 设计合入集成分支；GitHub issue 仍未关闭。
- #156 已通过 PR #161 把 `codex/issue-156-fresh-db-bootstrap` 合入 `codex/issue-137-llm-abstractions`：新增 `ai-chat-core` / `ai-chat-db`，完成 fresh database bootstrap、typed payload、typed repository、migration ledger 和 legacy-store coexistence tests；未新增 `app/ai-chat2` 壳，全文搜索/FTS 暂未实现。GitHub issue 仍未关闭。
- #157 已更新为：抽出 `ai-chat-provider`，把 provider-neutral trait、capabilities、OpenAI/Ollama adapter 边界从旧 app 债务中隔离出来。
- #158 已更新为：建立 `ai-chat-agent`，用 Rig + rmcp 实现 agent loop、file-backed skills、MCP/tool registry、approval、continuation、cancel/retry，并写入 fresh persistence。
- #159 已更新为：让 `app/ai-chat2` 使用新 crates 渲染项目、对话、timeline、tool、approval 和多模态内容。
- 2026-05-23 已读取并更新 GitHub #137、#155-#159。#137 正文已指向 `ai-chat2` 并行重构和新的子 issue 序列；#155-#159 title/body 已按“后续 Issue 调整提案”同步；PR #160 已把 #155 设计文档合入集成分支。
- 2026-05-24 已同步 GitHub #137、#156、#158：agent runtime 采用 Rig + rmcp；skills 和 MCP server 配置从文件/配置读取，不作为 fresh database source tables。现有 fresh DB 结构支持 Rig execution persistence，只需补充 runtime snapshot、skill activation transcript item 和 runtime tool name 映射。
- 2026-05-24 已完成 #156 本地实现和验证：`cargo test -p ai-chat-core`、`cargo test -p ai-chat-db`、`cargo clippy -p ai-chat-core --all-targets --all-features -- -D warnings`、`cargo clippy -p ai-chat-db --all-targets --all-features -- -D warnings` 和 `git diff --check` 均通过。
- 2026-05-26 已同步 GitHub #137、#156-#159 和 PR 状态：PR #161 已合入集成分支；#156-#159 仍为 open issues；下一步实现子 issue 是 #157。

## 当前架构事实

- `llm::Message` 已替换为 provider-neutral typed input/output item 词汇。
- `LlmInputItem` 和 `LlmContentPart` 表示 provider wire conversion 之前的 request-side LLM data。
- `LlmOutputItem` 为后续 runtime 和 persistence 保留 provider-neutral output 词汇。
- conversation panel 和 temporary/shortcut flows 现在共享同一个 typed history builder。
- `ProviderModel` 使用 provider-neutral `ModelCapabilities`，不再使用旧 streaming-only `ProviderModelCapability`。
- `ModelCapabilities` 覆盖 text input/output、streaming、image/file/audio input、image generation、tool calling、hosted web search、remote MCP、reasoning、structured output、stateful response continuation，以及 provider-specific typed extensions。
- OpenAI model classification 现在输出 Responses API、reasoning effort、hosted web search、structured output、stateful response continuation 等 typed capabilities。
- Ollama `/api/show` metadata 现在映射到 typed capabilities，并通过 `OllamaModelCapabilities` extension 保留 raw capabilities、family data、thinking mode、local web tools、vision image input。
- `Provider` 现在从 typed input items 构建 `ProviderRunRequest`，并流式输出 provider-neutral `ProviderRunEvent`。
- `ProviderRunRequest` 保留 provider request JSON snapshot，以兼容现有 `messages.send_content` resend 行为。
- `ProviderRunEvent` 替代 `FetchUpdate`，覆盖 thinking start、reasoning summary delta、text delta、output item added/done、tool call/result、MCP approval request、usage update、completed、failed。
- `ProviderRunState` 和 `ProviderUsage` 可表示 provider response/run metadata、output item ids、continuation metadata、token usage，#141 以 additive 方式把它们持久化到 assistant messages。
- `message_run_states`、`message_output_items`、`message_attachments` 现在 additive 地持久化 provider run state、output item events、usage、attachment metadata，不改变 `messages.content` 或 `messages.send_content`。
- `messages.content` 保存 rendered message content；`messages.send_content` 保存用于 resend 的 request body snapshot。
- `messages.input_content_parts` 保存 provider-neutral user input parts，给未来 contextual history 使用；旧 migrated messages 默认空列表，并 fallback 到 rendered text。
- `messages.content`、`messages.send_content`、`messages.input_content_parts` 并存的模型现在视为 legacy compatibility model。不要在这个形状上构建 agent runtime 或新的 multimodal persistence。
- OpenAI 使用 `/v1/responses`、reasoning effort、reasoning summaries、hosted web search citations、provider-neutral output item events，并在 compatible run state 可用时使用 persisted `previous_response_id` continuation。
- OpenAI adapter-specific Responses request 字段，例如 `include`、`text`、`tool_choice`、`tools`、`parallel_tool_calls`，必须留在 OpenAI provider schema 内，不要进入 generic provider trait。
- Ollama 有 provider-specific thinking、image input、experimental web search/fetch 行为，不能强行套成 OpenAI 形状。
- Ollama image input 只接受 raw base64 或 `data:image/...;base64,...` reference；URL、file-id、local-path、file、audio、generic attachment input 都明确不支持。
- Ollama run events 现在输出 provider-neutral output item、tool call/result、usage、completion data，同时保持 `ProviderRunState.run_id` 为空，避免 OpenAI-style continuation semantics。
- `CapabilityRequirement` 记录 templates 和 UI gating 所需的 provider-neutral feature requirements。
- `conversation_templates.required_capabilities` 位于尚未合入的 v6 schema 内，旧 migrated templates 默认空列表。
- Chat 和 shortcut UI 通过 `ModelCapabilities::missing_requirements` 展示 template/model compatibility，而不是读取 provider metadata 或 ad hoc JSON。
- Screenshot shortcuts 在 selected model 支持 `image_input` 时发送 PNG data URL image input，否则保留现有 OCR text fallback。

## 共享设计决策

- 新 LLM 抽象必须先 provider-neutral。OpenAI Responses 只是一个 adapter，不是 core model 的形状。
- Provider/model capabilities 必须使用 typed Rust structures 表达，不能只用 string metadata 或任意 JSON。
- Provider-specific features 应该放在 provider extension types 中，并挂在 generic model/provider metadata 上。
- UI、templates、shortcut flows 应该询问 generic capability model 某个 feature 是否可用。
- 迁移期间，现有 pure-text conversations、templates、shortcuts、resend behavior、OpenAI、Ollama 必须继续工作。
- 新 agent 和 multimodal persistence 应围绕 `app/ai-chat2`、fresh database 和独立 crates 设计，不要求把 legacy ai-chat stores 完整迁移。
- Legacy databases 不能被覆盖。如果保留 legacy support，它必须是显式策略：backup、read-only viewer、manual export，或 manual import。不要要求完整自动迁移到新的 agent schema。
- Fresh database versioning 必须存在数据库内部。不要只通过 `history_v6.sqlite3` 这类文件名表达当前 schema version。

## 已实现的能力词汇

Issue #138 建立了以下 Rust 能力类型：

- `ModelCapabilities`
- `ReasoningCapability`
- `ReasoningEffort`
- `ImageInputCapability`
- `FileInputCapability`
- `ToolCallingCapability`
- `ProviderCapabilityExtension`
- `OpenAIModelCapabilities`
- `OllamaModelCapabilities`
- `OllamaThinkingCapability`

当前实现保持请求执行行为不变：OpenAI 和 Ollama 仍产生相同的 provider request JSON shape，但 provider/template 代码中的 capability gating 已改为读取 typed model capabilities，不再读取 ad hoc JSON metadata 或 streaming-only enum state。

## 已实现的运行时词汇

Issue #139 建立了以下 Rust 运行时类型：

- `ProviderRunRequest`
- `ProviderRunEvent`
- `ProviderRunState`
- `ProviderUsage`

当前实现保持 request persistence 为 additive：现有 provider request JSON 仍作为 `messages.send_content` 的兼容 snapshot；新的 runtime 代码内部使用 `ProviderRunRequest` 和 `ProviderRunEvent`。OpenAI 和 Ollama 仍在各自 provider adapter 内负责 wire/request conversion。

## Provider 扩展规则

- Generic code 可以检查 common capabilities。
- Provider adapter 可以检查 provider-specific extension data。
- OpenAI-only 概念，例如 Responses output item ids、hosted tools、remote MCP details，不能变成所有 provider 的 required fields。
- Ollama-only 概念，例如 `think` values 和 experimental local web search/fetch，必须能表达出来，但不要伪装成 OpenAI tools。

## Persistence 方向

- 当前 v6 tables 足以支持 provider-run compatibility，但不是完整 agent 行为的目标 schema。
- 下一阶段 persistence 应该在 `ai-chat-db` crate 中为 `app/ai-chat2` 创建 fresh database，不要继续扩展旧 `messages` table model。
- Fresh schema 应让 provider-neutral transcript items 成为 display、edit、export、resend、contextual history、provider input reconstruction 的 canonical source of truth。
- `send_content` 风格的 provider request JSON 只保留为 provider step 上的 debug/replay snapshot，不作为 chat history。
- Fresh schema 必须保存 provider response/run state、output items、reasoning items、tool calls、tool results、MCP approval state、attachments metadata、usage、model、settings snapshots、provider-specific continuation metadata。
- 不要求从 v1-v6 history stores 完整自动迁移。如果旧数据仍要可访问，应实现明确的 legacy path，例如 backup-only retention、read-only legacy browsing、manual export/import。
- 不要把 binary attachment data 存到 message text 中。

## 全新数据库和并行重构方向

#155 的事实来源是 `app/ai-chat/docs/dev/fresh-database-schema.md`。

高层决策：

- 新模型通过 `app/ai-chat2` 和独立 crates 落地，不在旧 `app/ai-chat` 内做原地大重构。旧 app 保持可运行，用作 legacy path。
- `app/ai-chat2` 是薄 GPUI shell。领域模型、fresh database、provider adapter、agent runtime 分别进入 `ai-chat-core`、`ai-chat-db`、`ai-chat-provider`、`ai-chat-agent`。
- #156 优先创建 `ai-chat-core` 和 `ai-chat-db`，让 schema bootstrap 和 typed repository tests 独立通过；UI 迁移留给 #159。
- 使用 SQLite-first fresh store，typed JSON payload 必须落到 SQL `JSON` 列。每个 app 自有 JSON 列必须映射到一个具名 Rust 类型，并使用严格 serde 解码；不要使用 `TEXT` 列保存 JSON 字符串。
- 项目就是实际文件夹。scratch 或 no-project chats 必须使用 app 创建的 scratch folder，并写入 `projects` 行。
- `conversation_items` 是 canonical contextual timeline。Display、edit、export、resend、provider input reconstruction 必须读取它，并且不能为了渲染 tool/reasoning/approval/usage 再 join 执行表。
- 第一阶段采用线性 append-only timeline：`seq` 是对话内顺序。pi 式 `parentId` / leaf context tree 暂不进入 #155-#159；如果未来需要同一对话内 branching、编辑历史或多 leaf context，应先扩展 schema，再做 UI。
- fresh model 移除 folders、templates、conversation modes。保存的指令是 `prompts`；所有 conversations 都是 contextual。
- `ProviderRunRequest`、`ProviderRunEvent`、`ProviderRunState` 保持为低层 provider-step adapter boundary。持久化 canonical transcript 是 `conversation_items`；`agent_runs`、`provider_steps`、`tool_invocations`、approvals、usage 是执行、恢复、统计或调试索引。
- Provider request JSON 只保存在 `provider_steps`，作为 debug/replay snapshot，不作为 chat history。
- Rig 是 #158 的首选 agent runtime adapter，但不能成为数据库模型的 source of truth。`conversation_items` 重建上下文后转换成 Rig messages；`agent_runs`、`provider_steps`、`tool_invocations`、approvals、usage 记录 Rig loop 的执行事实。
- Skills 采用 Zed 风格 `SKILL.md` 文件模型。Catalog 只包含 name、description、path；body 在调用时从文件读取。已加载进对话的 rendered skill content 作为 `conversation_items` 快照保存，用于历史重放和 debug。
- MCP server 配置采用 Codex/Zed 风格 app/user/project config，不写入 chat database。运行时通过 rmcp 连接并把 tools 注册到 Rig `ToolServerHandle`；数据库只记录实际 tool invocation。
- Provider models 在 settings 中手动刷新，启动时读取 cached `provider_models` rows。
- Legacy v1-v6 stores 必须保持 intact。Legacy access 必须显式实现，不能要求完整自动迁移。

## app 和 crate 边界

推荐目标结构：

```text
app/ai-chat/              # 现有 legacy app，迁移期保持可运行
app/ai-chat2/             # 新 GPUI app shell，只组合 UI、窗口、路由和状态
crates/ai-chat-core/      # 领域数据契约和核心不变量
crates/ai-chat-db/        # fresh SQLite schema、migrations、typed repositories
crates/ai-chat-provider/  # provider-neutral trait、capabilities、OpenAI/Ollama adapter
crates/ai-chat-agent/     # agent loop、tool registry、approval、continuation、cancel/retry
```

边界约束：

- `ai-chat-core` 不能依赖 GPUI、Diesel、HTTP client 或具体 provider。
- `ai-chat-db` 依赖 `ai-chat-core`，负责 SQL `JSON` typed roundtrip、migration 和 repository transaction。全文搜索/FTS 暂未实现，是否需要单独搜索索引留给后续 issue 决定。
- `ai-chat-provider` 依赖 `ai-chat-core`，负责 provider wire conversion 和 provider-step events。
- `ai-chat-agent` 依赖 core/provider/repository traits，负责 Rig + rmcp 多步 agent loop、tool registry、file-backed skills、MCP runtime、approval 和 canonical item 写入。
- `app/ai-chat2` 只依赖这些 crates，不重新拥有长期业务模型。
- 旧 `app/ai-chat` 不在 #156-#159 中被强制迁移；它可以继续承载 legacy UI 和旧数据库访问，直到有明确替代策略。

## 多应用参考说明

#155 必须参考多种 agent/chat 应用，而不是只参考 Alma：

- Zed 的 native agent thread 把整条 thread 存成 typed JSON document，再压缩为 `threads.data BLOB`；列表和 sidebar 只读 `summary`、`updated_at`、`created_at`、`folder_paths`、`sidebar_threads` 等 metadata。可借鉴的是：热路径不要把一条 transcript 拆到很多表里，列表需要的字段应直接列化或反规范化。
- Codex 的 canonical session 是 append-only rollout JSONL；SQLite `threads` 只保存 `rollout_path`、`cwd`、title、preview、tokens、git、archive 等索引字段，dynamic tools 和 spawn edges 这类独立查询关系才单独建表。可借鉴的是：canonical log 和 metadata index 分层，thread 归属真实 cwd。
- pi 的 session 按 cwd 组织到 `~/.pi/agent/sessions/<encoded-cwd>/*.jsonl`；session header 后面是 append-only entries，每个 entry 有 `id`、`parentId`、`type`，context 通过当前 leaf 回溯构建。可借鉴的是：项目/目录组织对话、append-only typed entries、compaction/model change/label/custom entry 一等化。
- Alma 的价值在于把 provider/model cache、usage、MCP/OAuth、agent execution、memory/observability 与 transcript 分离。不能把 Alma 的 JSON-like text 列、chat message payload shape 或过度规范化表设计搬进 ai-chat。
- Zed 和 Codex 的 skills 都是文件驱动：catalog 暴露 metadata/path，body 在需要时读取 `SKILL.md`；MCP 都来自 settings/config。ai-chat 应复用这个边界，不把 skills/MCP 定义表放进 fresh database。

最终取舍：ai-chat 继续 SQLite-first，但采用“typed transcript log + metadata/execution indexes”模型。`conversation_items.payload_json` 是 canonical append-only transcript document；执行表只用于恢复、debug、统计和设置索引，不能成为 UI/context 主路径的 join 依赖。

## 后续 Issue 调整提案

现有 #155-#159 已经创建，并且已在 2026-05-23 按下面的边界同步到 GitHub title/body。后续如果继续拆分或改名，应先更新本文档和 `fresh-database-schema.md`，再同步线上 issue。

### #155 ai-chat2、crate 拆分和 fresh database 设计

标题建议：`[tech] ai-chat: design ai-chat2 crate split and fresh database model`

范围：

- 明确采用 `app/ai-chat2` 并行重构，而不是在旧 `app/ai-chat` 中原地替换全部数据层和 UI。
- 定义 `ai-chat-core`、`ai-chat-db`、`ai-chat-provider`、`ai-chat-agent`、`app/ai-chat2` 的 ownership boundary。
- 围绕 projects、canonical conversation items、agent runs、provider steps、tool invocations、approvals、usage events、prompts、shortcuts、providers、provider model caches、app settings、deduplicated attachments 定义新 schema。
- 在实现前定义每个 app 自有 JSON payload 对应的 Rust 类型。Repository API 中不要留下含糊的 `serde_json::Value` payload。
- 决定新 database file name 和 bootstrap strategy。新 store 不能覆盖已有 v1-v6 stores。
- 使用 `schema_migrations` 和 `schema_metadata` 把 schema version 和 migration state 放到 fresh database 内部；文件名不能成为 schema truth。
- 参考 Zed、Codex、pi、Alma 的共同边界：transcript 热路径应是 typed append-only log；project/cwd/thread metadata 应面向列表查询反规范化；需要独立查询的 execution、provider/model cache、usage、MCP/OAuth、spawn/tool relationships 才单独建索引表。
- 明确禁止把 timeline 渲染设计成多表 join。tool call/result、reasoning、approval、usage summary 等用户可见内容必须完整写入 `conversation_items.payload_json`。
- 明确第一阶段使用线性 append-only timeline；branching、edit tree、多个 leaf context 不在 #155-#159 scope 内。
- 明确 automatic full migration from legacy `messages` schema 不在 scope 内。
- 定义 legacy data policy：backup-only retention、read-only legacy viewer、manual export/import，或分阶段组合。
- 产出文档，不改 Rust 代码；schema tests 留给 #156。

### #156 core/db crates、fresh database bootstrap 和 repository layer

标题建议：`[tech] ai-chat: scaffold core/db crates and fresh database repositories`

范围：

- 新增 `crates/ai-chat-core`，放置 canonical Rust 数据契约、typed JSON payload、ID/status enums、prompt/settings/capability snapshots。
- 新增 `crates/ai-chat-db`，放置 fresh SQLite schema、migrations、Diesel mappings、typed repositories。
- 可新增最小 `app/ai-chat2` shell 以验证 workspace wiring，但不迁移旧 UI。
- 增加 fresh database bootstrap path，且不覆盖 legacy v1-v6 stores。
- Bootstrap 时打开 fresh database，读取 `schema_metadata` 和 `schema_migrations`，再在事务内应用缺失 migrations。
- 数据库 schema 不新增 skills/MCP source tables。#156 只定义 Rig runtime 所需的 typed payload：`AgentRuntimeSnapshot`、`SkillActivationItem`、`runtime_tool_name`、provider step snapshot kind 等。
- 实现 typed repository/service APIs，覆盖 projects、conversations、conversation items、attachments、runs、provider steps、tools、approvals、usage、prompts、shortcuts、providers、provider models、app settings。
- SQL schema 中 app 自有结构化 payload 使用 `JSON` 列，不使用 `TEXT` 保存 JSON 字符串。
- 保持旧 database code 隔离为 legacy compatibility code，直到它可以被移除或替换为 read-only/export path。
- 增加 tests，覆盖 new database creation、internal version detection、repeated idempotent bootstrap、empty first run、transaction boundaries、cascade deletes、item ordering、typed JSON roundtrip、provider model cache persistence、legacy-store coexistence。
- 全文搜索/FTS 不在 #156 实现。`conversation_items.search_text` 仅作为普通派生文本字段保留，后续如果确认需要搜索，再单独设计 FTS、external-content FTS 或其他索引方案。

### #157 provider runtime crate

标题建议：`[tech] ai-chat: extract provider-neutral runtime crate for ai-chat2`

范围：

- 新增 `crates/ai-chat-provider`，承载 provider-neutral provider trait、provider-step request/event/state、model capability snapshots 和 adapter conversion。
- 保持 `Provider` 只负责一次 provider run，不执行 local/MCP tools，不拥有 multi-step loop。
- 把 OpenAI Responses、Ollama chat/show 等 provider-specific request/stream conversion 留在 adapter 内。
- provider request/response snapshot 只进入 `provider_steps` debug/replay path，不成为 canonical chat history。
- provider models 采用设置页手动刷新、保存到 `provider_models`、启动读缓存的策略。
- 增加 tests，覆盖 capability mapping、unsupported content rejection、provider-specific extension roundtrip、request snapshot redaction boundary。

### #158 agent runtime 和持久化

标题建议：`[tech] ai-chat: implement Rig/rmcp agent runtime and persistence for ai-chat2`

范围：

- 新增 `crates/ai-chat-agent`，承载 `AgentRunRequest`、`AgentRunEvent`、`AgentRunState`、`AgentStep`、`ToolRegistry`、`ToolDefinition`、`ToolInvocation`、`ToolExecutionPolicy`、`ToolApprovalPolicy`，并把 Rig 作为第一版 runtime adapter。
- 使用 Rig agent loop 执行 multi-step prompt；通过 `PromptHook` 和 `PersistingCompletionModel<M>` 写入 provider step、tool invocation、usage、terminal state。
- 使用 rmcp 连接 MCP servers，并把 MCP tools 注册到 Rig `ToolServerHandle`。MCP server 配置来自 app/user/project config，不进入 chat database。
- 实现 Zed 风格 file-backed skills：扫描 `SKILL.md` catalog，按需读取 body，已加载内容作为 `SkillActivation` transcript 快照写入 `conversation_items`。
- 实现 tool execution、MCP/provider-hosted tool reporting、approval、continuation、retry、cancel 和 max-step guard。
- 持久化每个 agent run、provider step、tool invocation、tool result、approval decision、usage update、retry、cancellation、terminal state。
- 保存每个 provider step 的 provider request/response snapshots 和 continuation metadata。
- 同步写入 `conversation_items`，保证 timeline/context/export/resend 不依赖 execution tables join。
- 让 crash recovery 和 debug inspection 不依赖 rendered message text 重建状态。
- 增加 tests，覆盖 Rig multi-step tool runs、MCP tool runs、tool name collision、failed tools、denied approvals、paused/canceled runs、resend/retry behavior，以及 skill body 修改后旧 transcript 仍可复现。

### #159 ai-chat2 agent 和多模态时间线 UI

标题建议：`[tech] ai-chat: build ai-chat2 project chat and multimodal timeline UI`

范围：

- 在 `app/ai-chat2` 中实现项目优先的导航：项目就是真实文件夹，scratch/no-project 也创建真实 scratch folder。
- 所有对话都是 contextual；不再提供 `assistant-only`、`single`、`contextual` mode 选项。
- UI 使用“提示词”管理保存的 system/developer prompts；不再出现 template 概念。
- 快捷键绑定 prompt、provider/model、input source 和 action，不绑定 template/mode。
- 从新 transcript model 渲染多个 reasoning blocks、text deltas、tool calls、tool progress、tool results、approvals、generated images/files、retries、usage status。
- 支持 local tools 和 MCP tools 的 approval prompts。
- 对不支持的 tool、MCP、image、file、audio、structured output、reasoning modes 保持 capability gating。
- 分别验证 `app/ai-chat` legacy pure-text behavior 和 `app/ai-chat2` 新 agent timeline behavior。

## 验证预期

- 每个子 issue 如果修改 Rust 文件，都应该运行 `cargo fmt`。
- 每个子 issue 都应该运行与改动 subsystem 相关的 targeted tests。
- Integration-stage PR 合入 `main` 前必须运行：
  - `cargo build`
  - `cargo test`
  - `cargo clippy --all-targets --all-features -- -D warnings`
- UI 或 shortcut 阶段必须包含 manual verification notes，覆盖 old text chat、OpenAI、Ollama、resend、shortcut flows。

## 已完成子 Issue 记录

### #138 Provider-neutral 模型能力类型

- 用 `ModelCapabilities` 替换 `ProviderModelCapability`。
- 从 `llm.rs` 重新导出新的 capability vocabulary，供后续阶段使用。
- 把 OpenAI reasoning/web-search capability checks 迁移到 typed capabilities。
- 把 Ollama thinking/tool capability checks 迁移到 typed `OllamaModelCapabilities`。
- 保留现有 OpenAI Responses request body、Ollama chat request body、template replay、streaming、ext-setting behavior。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::preset`
  - `cargo test -p ai-chat state::chat::models`
  - `cargo test -p ai-chat components::chat_form::model_select`
  - `cargo test -p ai-chat features::settings::shortcut_settings`

### #142 Provider-neutral typed input 和 output items

- 用 provider-neutral `LlmInputItem` 和 `LlmContentPart` 替换 public LLM request item shape。
- 增加 provider-neutral output vocabulary，覆盖 message、reasoning、tool call/result、MCP approval、hosted tool call items。
- 为 conversation panel 和 temporary/shortcut flows 增加共享 history builder。
- 把 OpenAI 和 Ollama provider request construction 迁移为接受 typed input items，并在 adapter 内部转换。
- 保留现有 pure-text OpenAI Responses request bodies、Ollama chat request bodies、template replay、resend、shortcut behavior。
- 非 text input parts 现在会在当前 adapters 中明确失败，不会静默丢弃或强行转成 text。
- full multimodal、tool output、MCP、stateful continuation、provider run events、persistence 留给 #139、#141、#143、#144。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat llm::types`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`

### #139 基于 run 的 Provider trait 和事件

- 用 provider-neutral `ProviderRunEvent` 替换旧 `FetchUpdate` stream path。
- 增加 `ProviderRunRequest`、`ProviderRunState`、`ProviderUsage` 作为第一阶段 runtime abstraction。
- 把 `Provider` trait 改为通过 `build_run_request` 构建 run request，并通过 `run` 执行。
- 保留 `Provider::request_body` 作为 persisted `messages.send_content` snapshots 的兼容 helper。
- 把 conversation panel 和 temporary detail streaming consumers 迁移到 `ProviderRunRunner`。
- 把 OpenAI Responses stream parsing 迁移到 provider-neutral events，不在 core runtime 暴露 OpenAI event names。
- 增加 OpenAI parser coverage，覆盖 completed、failed、incomplete、top-level error events。
- 把 Ollama chat streaming 迁移到同一 run event vocabulary，同时保持 experimental web search/fetch loop 为 provider-local。
- generic app-level tool execution、MCP approval UI、run-state database persistence 留给 #141、#143、#144、#140。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat llm::types`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::provider::openai`
  - `cargo test -p ai-chat llm::provider::ollama`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #141 LLM run state、output item、tool 和 attachment 持久化

- 增加 database v6，并把 legacy v1-v5 stores 迁移到 `history_v6.sqlite3`。
- 保留 `messages.content` 和 `messages.send_content` 作为 display、export、resend 的兼容 surface。
- 增加 additive run persistence tables，用于 assistant message run state、ordered output item events、attachment metadata。
- 增加围绕 `ProviderRunState`、`ProviderUsage`、`LlmOutputItem`、attachment refs 的 typed message persistence wrappers。
- Conversation streaming 现在会累积 output item events、tool/MCP events、usage、completed run state，并随 terminal message state 一起持久化。
- Temporary chat 仍为 in-memory，但保存到 normal conversation 时会携带 run persistence。
- Resend assistant message 会清除旧 run persistence，同时保留现有 request body snapshot 行为。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat database::migrations`
  - `cargo test -p ai-chat database::service`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo test -p ai-chat`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #143 OpenAI Responses adapter 迁移

- 增加 optional provider run state plumbing，让 OpenAI 可用 `previous_response_id` 构建 request bodies，其他 provider 保持现有 request construction behavior。
- Conversation panel 和 temporary chat 在 contextual mode 下会使用 compatible persisted OpenAI assistant run state，裁剪该 response 之前的 history；non-contextual modes 或 incompatible state 会 fallback 到 full transcript behavior。
- OpenAI continuation 由 persisted provider/model/run id、non-secret provider settings snapshot、request context key 共同 gate。Request context key 是去掉 `input` 和 `previous_response_id` 的 Responses request body，因此 template/tool/reasoning/stream 变化会阻止 stale continuation，而 input delta 不会。
- OpenAI request conversion 现在输出 Responses content parts，覆盖 text、image references、file references、tool results、item references；unsupported audio 或 generic attachments 会明确失败。
- OpenAI stream 和 response parsing 把 message、reasoning、hosted tool、function-call、MCP-related output items 映射为现有 core types 可表达的 provider-neutral events。
- Function-call argument completion 现在输出 `ToolCallRequested`；本阶段刻意不增加 generic tool execution、MCP server configuration、approval UI、capability-gated controls。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat llm::run_persistence`
  - `cargo test -p ai-chat llm::provider::openai -- --nocapture`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel -- --nocapture`
  - `cargo test -p ai-chat features::temporary::detail -- --nocapture`
  - `cargo test -p ai-chat database::service -- --nocapture`
  - `cargo test -p ai-chat database::migrations -- --nocapture`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #144 Ollama 共享抽象迁移

- 把 Ollama `vision` capability 映射到 generic `ModelCapabilities.image_input`，不暴露 OpenAI-only hosted web search、remote MCP、stateful continuation settings。
- 保持 Ollama thinking 和 experimental local web search/fetch 为 provider-specific extension behavior。
- 把 Ollama input conversion 从 single-text messages 扩展出去：normal messages 支持 multi-part text 以空行 join，并支持 base64/data-URL image inputs；text-only tool results 映射为 Ollama `role: "tool"` messages。
- Unsupported URL images、OpenAI file ids、local paths、files、audio、generic attachments、item references 会明确失败，不会静默转义。
- Ollama stream 和 non-stream responses 现在输出 provider-neutral output item events、tool call/result events、带 Ollama timing metadata 的 token usage、completed content。
- Ollama run state 仍为 additive 和 compatibility-oriented：没有 provider run id，没有 continuation 所需 output item ids，没有 OpenAI-shaped `previous_response_id` 行为。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat llm::provider::ollama`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::run_persistence`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #140 能力门控

- 增加 `CapabilityRequirement` 和 `ModelCapabilities::{supports_requirement, missing_requirements}`，作为 templates、chat UI、shortcuts 的 typed compatibility surface。
- 扩展尚未合入的 v6 template schema，增加 `conversation_templates.required_capabilities JSON NOT NULL DEFAULT '[]'`；v1-v5 migrations 为旧 templates 回填空列表。
- 为 template create/edit 增加 required capabilities 控件，并在 template lists、template view dialogs、chat template picker 显示 compatibility/missing-capability 状态。
- Chat 会保留不可兼容的 selected template 可见，但禁用发送并展示缺失 capabilities。
- Shortcut settings 显示 `CapabilityMismatch`，编辑时在 selected template/model pair 不兼容时警告，并在 required capabilities 缺失时阻止正常 shortcut execution。
- Shortcut screenshots 在 image-capable models 下发送 typed `LlmContentPart::ImageRef` PNG data URLs，否则 fallback 到现有 OCR text path。
- Conversation panel、temporary detail、chat form 现在把当前 user input 作为 content parts 传递，同时保留普通 text input 为单个 text part。
- Persisted normal 和 temporary conversation history 现在携带 `input_content_parts`，不再从 rendered text 或 provider-specific request bodies 重建 context。
- 已运行验证：
  - `cargo fmt`
  - `cargo test -p ai-chat llm::types`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::preset`
  - `cargo test -p ai-chat database::migrations`
  - `cargo test -p ai-chat database::service`
  - `cargo test -p ai-chat components::chat_form`
  - `cargo test -p ai-chat features::settings::template_settings`
  - `cargo test -p ai-chat features::settings::shortcut_settings`
  - `cargo test -p ai-chat features::hotkey`
  - `cargo test -p ai-chat features::conversation::preview`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat state::workspace`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

## 下一个子 Issue 约束

#156 已通过 PR #161 完成 fresh core/db bootstrap 并合入集成分支。下一步实现子 issue 是 #157，用于抽出 provider runtime crate。#155 已把 `ai-chat2` 并行重构、crate 拆分和后续 issue 重排固定到设计文档中。除非明确重新定义 scope，否则 #154 不应作为旧 `messages` table 上的窄补丁继续推进。

- GitHub #155-#159 的 title/body 已与“后续 Issue 调整提案”保持一致；后续修改必须先更新文档再同步线上 issue，避免实现继续沿旧 `app/ai-chat` 原地改造路线推进。
- #156 已从 `crates/ai-chat-core` 和 `crates/ai-chat-db` 落地；没有先改旧 app UI，也没有新增 `app/ai-chat2` shell。
- 从新的 canonical transcript schema 开始，不要继续迁移旧 `messages.content` / `send_content` / `input_content_parts` 模型。
- 保持 legacy ai-chat databases 完整。不要覆盖它们，也不要要求完整自动迁移。
- 实现前先决定 legacy access strategy：backup-only retention、read-only legacy viewer、manual export/import，或分阶段组合。
- fresh schema version 和 migration ledger 必须存在数据库内部。文件名可以识别 fresh store，但 schema compatibility 必须从 database metadata 判断。
- 新数据库必须保持 provider-neutral。OpenAI Responses continuation、Ollama local web tools、MCP calls、hosted tools、未来 provider-specific metadata 应通过 typed provider/tool extension points 附加。
- `Provider` 只负责一次 provider run。Multi-step agent loops、tool execution、approvals、retries、continuation 属于 `AgentRuntime` 和 fresh persistence model。
- Skills 和 MCP server 定义必须保持 file/config-backed。#156 不应创建 reusable skill/MCP source tables；#158 只把已加载 skill 快照和实际 MCP tool invocation 写入 transcript/execution tables。
- Rig 只能作为 runtime adapter。数据库真相仍是 `conversation_items` 和 execution indexes，不持久化 Rig 内部 `PromptResponse.messages` 或把 Rig `ConversationMemory` 作为主写入路径。
