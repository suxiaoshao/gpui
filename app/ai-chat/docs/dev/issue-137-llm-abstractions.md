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
| #157 | `codex/issue-157-provider-runtime-crate` | 清理 `ai-chat-db` SQLite 时间、布尔和 closed enum/status 类型约束 | 已通过 PR #162 合入集成分支；GitHub issue 仍未关闭 |
| #158 | `codex/issue-158-agent-runtime-persistence` | 建立 `ai-chat-agent`，用 Rig + rmcp 实现 agent loop、file-backed skills、MCP/tool/approval runtime 和持久化写入 | 已通过 PR #163 合入集成分支；GitHub issue 仍未关闭 |
| #159 | `codex/issue-159-ai-chat2-ui`（PR #164 source，已合入；后续增量从集成分支新建） | 在 `app/ai-chat2` 渲染项目、对话、多步 reasoning、tool、approval、多模态 timeline | 进行中：不依赖真实 agent runtime 的 foundation 已通过 PR #164 合入集成分支，包括基础设施壳、app chrome、file-backed logging、About、Sidebar/home skeleton、Home root/sidebar 结构修正、ChatForm 视觉预览、`ComposerEditor` 第一版输入内核、cursor/scroll 修正、Unicode/grapheme-aware 编辑、Settings shell + General/Appearance/Projects、New Conversation 默认页、no-project 项目选择器、Codex-style composer/project tray polish、基础 parity 修复、Provider settings 第一阶段、DB-backed Composer provider/model picker、provider model capability source / reasoning control 第一版、provider brand assets / app-assets 过程宏 refactor，以及 project-first sidebar 第一版；Agent Conversation Page 专项计划已固定；GitHub issue 仍未关闭，完整 project chat 和多模态 timeline UI 仍未完成 |

## Issue 同步快照

最后同步时间：2026-06-05。

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
- #157 已通过 PR #162 把 `codex/issue-157-provider-runtime-crate` 合入 `codex/issue-137-llm-abstractions`：清理 `ai-chat-db` fresh schema 的 SQLite 类型表达和约束，包括 typed time columns、boolean `CHECK` 和 closed enum/status `CHECK`。GitHub issue 仍未关闭。
- #158 已通过 PR #163 把 `codex/issue-158-agent-runtime-persistence` 合入 `codex/issue-137-llm-abstractions`：新增 `ai-chat-agent`，用 Rig + rmcp 实现初版 agent runtime、file-backed skills、tool registry、MCP helper、provider step/tool/approval/usage/terminal state persistence。GitHub issue 仍未关闭。
- #159 已更新为：让 `app/ai-chat2` 使用新 crates 渲染项目、对话、timeline、tool、approval 和多模态内容。
- #159 详细 UI/壳/本机状态/可观测性/legacy mapping 清单见 `app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`。该文档把每项能力标为“已完成”、“占位”、“后端已具备”、“已有专项计划”、“未开始”、“暂不做”、“不照搬”或“已替代”，避免把占位入口误判为已完成业务能力。Agent conversation page 专项计划见 `app/ai-chat/docs/dev/issue-159-ai-chat2-agent-conversation-page.md`。
- 2026-05-23 已读取并更新 GitHub #137、#155-#159。#137 正文已指向 `ai-chat2` 并行重构和新的子 issue 序列；#155-#159 title/body 已按“后续 Issue 调整提案”同步；PR #160 已把 #155 设计文档合入集成分支。
- 2026-05-24 已同步 GitHub #137、#156、#158：agent runtime 采用 Rig + rmcp；skills 和 MCP server 配置从文件/配置读取，不作为 fresh database source tables。现有 fresh DB 结构支持 Rig execution persistence，只需补充 runtime snapshot、skill activation transcript item 和 runtime tool name 映射。
- 2026-05-24 已完成 #156 本地实现和验证：`cargo test -p ai-chat-core`、`cargo test -p ai-chat-db`、`cargo clippy -p ai-chat-core --all-targets --all-features -- -D warnings`、`cargo clippy -p ai-chat-db --all-targets --all-features -- -D warnings` 和 `git diff --check` 均通过。
- 2026-05-26 已同步 GitHub #137、#156-#159 和 PR 状态：PR #161 已合入集成分支；#156-#159 仍为 open issues；下一步实现子 issue 是 #157。
- 2026-05-26 决定不再为 Rig 路线保留独立 `ai-chat-provider` crate。#157 改为数据库类型/约束清理；Rig adapter、provider step 持久化和 provider 观测边界归入 #158 的 `ai-chat-agent`。
- 2026-05-26 已同步 PR #162 状态：#157 已合入集成分支；验证记录为 `cargo fmt`、`cargo test -p ai-chat-db` 和 `git diff --check`，未运行 full workspace build/clippy 或手动 UI 验证；下一步实现子 issue 是 #158。
- 2026-05-27 已同步 PR #163 状态：#158 已合入集成分支；验证记录为 `cargo fmt`、`cargo check -p ai-chat-agent`、`cargo test -p ai-chat-core`、`cargo test -p ai-chat-db`、`cargo test -p ai-chat-agent`、`cargo clippy -p ai-chat-core -p ai-chat-db -p ai-chat-agent --all-targets --all-features -- -D warnings`、`cargo build -p ai-chat-core -p ai-chat-db -p ai-chat-agent` 和 `git diff --check`，未运行 full workspace validation 或手动 GPUI UI 验证；下一步实现子 issue 是 #159。
- 2026-05-28 已同步 #159 本地/远程分支状态：`codex/issue-159-ai-chat2-ui` 已推送提交 `b749528`（`ai-chat2` 基础设施壳）、`0843e15`（bundle/menu/titlebar 壳）、`e7077fc`（About 真实窗口）和 `6d4a34f`（Sidebar/home skeleton）。当前子分支继续推进 Home root/sidebar 结构修正：主窗口 UI root 下沉到 `features/home/shell.rs`，Sidebar 拆为独立组件，并在 Home root 挂载 `gpui-component` sheet/dialog/notification layers。当前没有对应 PR，尚未合入 `codex/issue-137-llm-abstractions`。完整 project chat / multimodal timeline UI 仍未实现。
- 2026-05-29 已同步 #159 本地/远程分支状态：`codex/issue-159-ai-chat2-ui` 已推送提交 `d7a5751`（ChatForm visual preview）。当前分支继续推进 `ComposerEditor` 第一版输入内核：ChatForm 已替换非 input 占位，支持文本输入、IME range、选择/光标、编辑快捷键、plain text 剪贴板、Enter 发送、Shift+Enter 换行、`$skill-name` token 和 `ComposerSnapshot`；仍不接真实附件、prompt/provider/model store、`$` completion UI 或 agent loop。当前没有对应 PR，尚未合入 `codex/issue-137-llm-abstractions`。完整 project chat / multimodal timeline UI 仍未实现。
- 2026-05-31 已同步 live GitHub 和远程分支状态：GitHub #137、#155-#159 仍 open；PR 列表没有 `codex/issue-159-ai-chat2-ui` 对应 PR；本地分支与 `origin/codex/issue-159-ai-chat2-ui` 一致，领先 `origin/codex/issue-137-llm-abstractions` 10 个提交。5/29 后新增 `34ccb6f`（Composer cursor styling）、`26a89fa`（Composer scrolling）和 `09b2f22`（Unicode-aware composer editing）。当前仍不接真实附件、prompt/provider/model store、`$` completion UI 或 agent loop，完整 project chat / multimodal timeline UI 仍未实现。
- 2026-06-01 已同步 live GitHub 和远程分支状态：GitHub #137、#155-#159 仍 open；PR 列表没有 `codex/issue-159-ai-chat2-ui` 对应 PR；本轮推送后子分支领先 `origin/codex/issue-137-llm-abstractions` 14 个提交。5/31 后新增 `ed59682`（Settings shell + General/Appearance）、`57bb3d5`（main/settings window placement、composer focus、quit flush、config tolerance、default Material You visibility）、`edb4a3d`（Settings Projects 列表/添加项目），以及本次 New Conversation 默认页、no-project 项目选择器和 Codex-style composer/project tray polish。当前仍不接真实 project sidebar/conversation navigation、prompt/provider/model data source、agent run/timeline、`$` completion UI、Shortcuts settings 或 Temporary Conversation runtime。
- 2026-06-01 追加：New Conversation 默认页已完成中性 AI Chat 文案、no-project 选择语义、existing-folder add project 和 Codex-style layered neutral muted tray；仍不创建真实 conversation 或接 agent runtime。
- 2026-06-03 本地开发状态追加：Provider settings 后续已完成 DB-backed Composer provider/model picker、provider model capability source 建模、Ollama/Gemini/OpenRouter API-discovered capability enrichment、OpenAI/Anthropic/DeepSeek/Mistral docs-derived reasoning profiles、`ReasoningControl`/`ReasoningSelection` payload、Composer token budget selector、provider-specific reasoning additional params 生成，以及 provider brand assets / app-assets 过程宏 refactor。该条是本地开发文档更新，不代表重新同步过 live GitHub issue/PR 状态。
- 2026-06-03 Provider brand assets 实现记录：Lucide v1 已移除品牌图标，迁移方向是继续用 Lucide 承担 UI pictogram，brand logo 使用 Simple Icons 或品牌官方资源。`ai-chat2` 已把 provider logo 从 app-local Lucide `IconName` 中分离出来，新增 app-owned `ProviderLogoName` / `ProviderVisual`；默认使用 repo-vendored Simple Icons 单色 SVG，缺失、过期或 guideline 要求时再改用官方 SVG；不使用 CDN，也不在运行时联网下载。当前 branded built-in providers 已全覆盖：Simple Icons 来源包括 Anthropic、Google Gemini、Ollama、OpenRouter、DeepSeek、Moonshot AI、Mistral AI 和 Perplexity；新增补齐来源为 theSVG OpenAI、theSVG Azure OpenAI、theSVG Groq（提取前景 mark 后单色化）、theSVG xAI/Grok（xAI 官方下载包在当前环境返回 403，按既定策略用可追溯第三方 SVG 补齐）、Together AI 官方 brand package、Wikimedia Z.AI SVG（记录来源为 `chat.z.ai`，提取 Z mark 后单色化）。ChatForm model picker、Settings provider row 和 provider header 已优先渲染 provider logo；`custom_openai_compatible` 不是固定品牌，继续使用 `Server` fallback，`chat_completions` 只是 API mode，不新增 Chat logo。
- 新增 provider logo 来源 URL 记录：OpenAI = https://thesvg.org/icon/openai（第三方 theSVG）；Azure OpenAI = https://thesvg.org/icon/azure-azure-openai（第三方 theSVG）；Groq = https://thesvg.org/icon/groq（第三方 theSVG）；xAI = https://thesvg.org/icon/xai-grok（第三方 theSVG，官方 https://x.ai/legal/brand-guidelines 下载包当前环境返回 403）；Together = https://www.together.ai/brand（官方 brand package）；Z.AI = https://commons.wikimedia.org/wiki/File:Z.ai_(company_logo).svg（Wikimedia，记录来源为 `chat.z.ai`）。
- 2026-06-03 Provider logo 渲染修正：`gpui_component::Icon` 会按 text color 语义渲染 SVG；Groq/Z.AI 这类“深色底 + 白色反白 mark”的 SVG 会在单色图标渲染中塌成实心方块。当前已改为只 vendor 前景 mark，并统一使用 `currentColor`；资产测试覆盖 Groq/Z.AI 不再依赖反白背景。
- 2026-06-04 已同步 live GitHub 和 PR 状态：GitHub #137 和 #159 仍 open；`codex/issue-159-ai-chat2-ui` 已创建 PR #164 指向 `codex/issue-137-llm-abstractions`，尚未合入。当前 PR 聚焦不依赖真实 agent runtime 的 `ai-chat2` foundation：app shell、Settings、Projects、Provider/model cache、Composer/model/reasoning controls、provider brand assets 和 project-first sidebar。验证记录为 `cargo fmt --check`、`cargo fmt`、`cargo check -p ai-chat2`、`cargo build -p ai-chat2`、`cargo test -p ai-chat-agent -p ai-chat-core -p ai-chat-db`、`cargo clippy -p ai-chat2 -p ai-chat-agent -p ai-chat-core -p ai-chat-db --all-targets --all-features -- -D warnings` 和 `git diff --check`；未运行 full workspace validation 或手动 GPUI UI 验证。
- 2026-06-05 已同步 live GitHub 和 PR 状态：GitHub #137、#155-#159 仍 open；PR #164 `feat(ai-chat2): add non-agent foundation` 已于 2026-06-05 02:40:06 UTC / 10:40:06 Asia/Shanghai 合入 `codex/issue-137-llm-abstractions`，merge commit 为 `738df0b68b0c927a65a084c028d0a7de4dc71dce`。#159 的 foundation 已进入集成分支，但完整 project chat、agent runtime 接线和多模态 timeline UI 仍未完成。
- 2026-06-05 追加 Agent Conversation Page 专项计划：新增 `app/ai-chat/docs/dev/issue-159-ai-chat2-agent-conversation-page.md`，固定 New Conversation 发送后创建 conversation、每会话匿名 scratch project、sidebar 即时刷新、conversation page、真实 `AgentRuntime` observer、canonical timeline、hover copy/time、i18n/icon/dependency 和验证计划。本次仅更新开发文档，未实现产品代码。

## 当前架构事实

- `llm::Message` 已替换为 provider-neutral typed input/output item 词汇。
- `LlmInputItem` 和 `LlmContentPart` 表示 provider wire conversion 之前的 request-side LLM data。
- `LlmOutputItem` 为后续 runtime 和 persistence 保留 provider-neutral output 词汇。
- conversation panel 和 temporary/shortcut flows 现在共享同一个 typed history builder。
- `ProviderModel` 使用 provider-neutral `ModelCapabilities`，不再使用旧 streaming-only `ProviderModelCapability`。
- `ModelCapabilities` 覆盖 text input/output、streaming、image/file/audio input、image generation、tool calling、hosted web search、remote MCP、reasoning、structured output、stateful response continuation，以及 provider-specific typed extensions。
- Fresh `ModelCapabilitiesSnapshot.reasoning` 现在保留 `CapabilitySourceSnapshot` 和 provider-neutral `ReasoningControlSnapshot`，区分 API-discovered、official-doc-derived、heuristic、manual 和 OpenRouter-normalized 来源。
- Composer 发送 snapshot 现在可以携带 `ReasoningSelectionSnapshot`，后续 agent run 可按 provider 生成 OpenAI `reasoning.effort`、Ollama `think`、Gemini `thinkingConfig`、Anthropic thinking/effort、DeepSeek thinking/reasoning_effort、Mistral reasoning_effort 或 OpenRouter reasoning 参数。
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
- `app/ai-chat2` 是薄 GPUI shell。领域模型、fresh database、agent runtime 分别进入 `ai-chat-core`、`ai-chat-db`、`ai-chat-agent`；Rig provider adapter 和 provider step 观测边界属于 `ai-chat-agent`。
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
crates/ai-chat-agent/     # Rig adapter、agent loop、tool registry、approval、continuation、cancel/retry
```

边界约束：

- `ai-chat-core` 不能依赖 GPUI、Diesel、HTTP client 或具体 provider。
- `ai-chat-db` 依赖 `ai-chat-core`，负责 SQL `JSON` typed roundtrip、migration 和 repository transaction。全文搜索/FTS 暂未实现，是否需要单独搜索索引留给后续 issue 决定。
- `ai-chat-agent` 依赖 core 和 repository traits，负责 Rig + rmcp 多步 agent loop、Rig message conversion、provider step 观测、tool registry、file-backed skills、MCP runtime、approval 和 canonical item 写入。
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
- 定义 `ai-chat-core`、`ai-chat-db`、`ai-chat-agent`、`app/ai-chat2` 的 ownership boundary。
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

### #157 ai-chat-db SQLite 类型清理

标题建议：`[tech] ai-chat-db: clean up SQLite types and constraints`

范围：

- 不新增 `ai-chat-provider` crate；未来 Rig provider integration 归入 #158 的 `ai-chat-agent`。
- 在 `crates/ai-chat-db` fresh schema 中把时间列改为 SQLite `DateTime`，Diesel schema 映射为 `TimestamptzSqlite`，repository row 使用 `OffsetDateTime`。
- 将 `providers.enabled`、`prompts.enabled`、`shortcuts.enabled` 表达为 `BOOLEAN NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))`。
- 给 `agent_runs.status`、`provider_steps.status`、`tool_invocations.status`、`conversation_items.kind`、`conversation_items.status` 等 closed enum/status 列补充 `CHECK`。
- 不做 legacy schema 兼容迁移；只修改 fresh schema、typed Diesel schema 和 repository 映射。
- 增加 tests，覆盖 bootstrap DDL、非法 boolean、非法 enum/status 和 repository `OffsetDateTime` roundtrip。

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

范围摘要：

- `app/ai-chat2` 需要实现 project-first navigation、canonical `conversation_items` timeline、prompt/provider/model composer、agent run/retry/cancel/resend、tool/approval/status/usage/multimodal rendering、settings 和 shortcut UI。
- 已合入的 foundation 完成基础设施壳、app chrome、file-backed logging、About、Home/Sidebar skeleton、ChatForm 视觉预览、`ComposerEditor` 第一版输入能力、Settings shell + General/Appearance/Projects、New Conversation 默认页、no-project 项目选择器、Codex-style composer/project tray polish、main/settings window placement 和基础 parity 修复；Provider settings 第一阶段已实现，已具备 fresh DB provider/model 基础 UI、Provider i18n、默认 disabled、独立滚动布局、保存前本地校验、未保存状态标签、GPUI credentials secret write/read、真实 model fetch、`gpui_tokio` runtime bridge、`ListState` provider/model lists、provider list panel/row separator 和右侧 detail 整体滚动；Composer DB-backed provider/model picker 已实现，且 provider model cache 已开始保存 capability source 和 provider-specific reasoning control；Provider brand assets 已完成第一版，包含 `ProviderLogoName` / `ProviderVisual`、Simple Icons-first vendored SVG、generic Lucide fallback 和 `crates/app-assets-macros` 过程宏拆分；Temporary Conversation、Prompt/Shortcut settings、manual provider model editor、agent runtime/timeline 接线仍是占位或未开始。
- 详细清单和旧 `app/ai-chat` 对比见 `app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；agent conversation page 的 implementation-ready 计划见 `app/ai-chat/docs/dev/issue-159-ai-chat2-agent-conversation-page.md`。

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

#158 已通过 PR #163 完成 `ai-chat-agent` 的 Rig/rmcp agent runtime、file-backed skills、MCP/tool/approval runtime 和 fresh persistence 写入，并合入集成分支。当前实现子 issue 是 #159。`codex/issue-159-ai-chat2-ui` 已推进 `ai-chat2` 基础设施壳、app chrome、file-backed logging、About、Sidebar/home skeleton、Home root/sidebar 结构修正、ChatForm 视觉预览、`ComposerEditor` 第一版输入内核、cursor/scroll 修正、Unicode/grapheme-aware 编辑、Settings shell + General/Appearance/Projects、New Conversation 默认页、no-project 项目选择器、Codex-style composer/project tray polish、基础 parity 修复、Provider settings 第一阶段实现（保存校验、secret credentials、真实 model fetch、`gpui_tokio` bridge、`ListState` provider/model lists、provider list panel/row separator、右侧 detail 整体滚动）、DB-backed Composer provider/model picker、provider model capability source / reasoning control 第一版、provider brand assets / app-assets 过程宏 refactor，以及 project-first sidebar 第一版；这些不依赖真实 agent runtime 的 foundation 已通过 PR #164 合入集成分支。下一步应继续 #159 的真实 conversation create/send runtime、agent run/cancel/retry/resend 接线和 canonical timeline 渲染，而不是回到旧 `messages` table 或在 GPUI 层重写 agent loop；具体第一步按 `issue-159-ai-chat2-agent-conversation-page.md` 执行。#155 已把 `ai-chat2` 并行重构、crate 拆分和后续 issue 重排固定到设计文档中。除非明确重新定义 scope，否则 #154 不应作为旧 `messages` table 上的窄补丁继续推进。

- GitHub #155-#159 的 title/body 已与“后续 Issue 调整提案”保持一致；后续修改必须先更新文档再同步线上 issue，避免实现继续沿旧 `app/ai-chat` 原地改造路线推进。
- #156 已从 `crates/ai-chat-core` 和 `crates/ai-chat-db` 落地；没有先改旧 app UI，也没有新增 `app/ai-chat2` shell。
- #157 已清理 fresh DB 的 typed SQLite time、boolean constraints 和 closed enum/status constraints；不要在后续 issue 中重新引入手写 RFC3339 时间转换或无约束 status/kind 文本。
- #158 已从 `crates/ai-chat-agent` 落地。#159 应复用 `AgentRuntime`、repository APIs、skill catalog/loading、MCP registration helpers 和 approval persistence，不应在 GPUI 层重新实现 agent loop 或 tool registry。
- 从新的 canonical transcript schema 开始，不要继续迁移旧 `messages.content` / `send_content` / `input_content_parts` 模型。
- 保持 legacy ai-chat databases 完整。不要覆盖它们，也不要要求完整自动迁移。
- 实现前先决定 legacy access strategy：backup-only retention、read-only legacy viewer、manual export/import，或分阶段组合。
- fresh schema version 和 migration ledger 必须存在数据库内部。文件名可以识别 fresh store，但 schema compatibility 必须从 database metadata 判断。
- 新数据库必须保持 provider-neutral。OpenAI Responses continuation、Ollama local web tools、MCP calls、hosted tools、未来 provider-specific metadata 应通过 typed provider/tool extension points 附加。
- `Provider` 只负责一次 provider run。Multi-step agent loops、tool execution、approvals、retries、continuation 属于 `AgentRuntime` 和 fresh persistence model。
- Skills 和 MCP server 定义必须保持 file/config-backed。#156 不应创建 reusable skill/MCP source tables；#158 只把已加载 skill 快照和实际 MCP tool invocation 写入 transcript/execution tables。
- Rig 只能作为 runtime adapter。数据库真相仍是 `conversation_items` 和 execution indexes，不持久化 Rig 内部 `PromptResponse.messages` 或把 Rig `ConversationMemory` 作为主写入路径。
