# Issue #137 LLM Abstraction Coordination

This is a temporary coordination document for the `ai-chat` LLM abstraction work tracked by issue #137.
It is the cross-conversation source of truth while the integration branch is active.
Delete it before the final merge to `main`, unless the remaining content is promoted into long-lived developer documentation.

## Branch Strategy

- Integration branch: `codex/issue-137-llm-abstractions`
- Child issue branches must be created from the integration branch.
- Child issue pull requests should target `codex/issue-137-llm-abstractions`, not `main`.
- The integration branch must remain buildable after each child issue merge.
- After #138, #142, and #139 stabilize the shared abstraction, open a staged PR from the integration branch to `main` to reduce branch drift.

## Issue And Branch Map

| Issue | Branch | Purpose | Status |
| --- | --- | --- | --- |
| #137 | `codex/issue-137-llm-abstractions` | Parent integration work | Active |
| #138 | `codex/issue-138-model-capabilities` | Provider-neutral model capability types | Merged to integration via PR #147; GitHub issue still open |
| #142 | `codex/issue-142-llm-items` | Typed input, content, and output items | Merged to integration via PR #148; GitHub issue still open |
| #139 | `codex/issue-139-provider-runtime` | Run-based provider trait and events | Merged to integration via PR #149; GitHub issue still open |
| #141 | `codex/issue-141-llm-persistence` | Run state, output items, tools, attachments persistence | Merged to integration via PR #150; GitHub issue still open |
| #143 | `codex/issue-143-openai-responses-abstraction` | OpenAI Responses migration on shared abstraction | Merged to integration via PR #151; GitHub issue still open |
| #144 | `codex/issue-144-ollama-shared-abstraction` | Ollama migration on shared abstraction | Merged to integration via PR #152; GitHub issue still open |
| #140 | `codex/issue-140-capability-gating` | Template, shortcut, and UI capability gating | Pending |

## Issue Sync Snapshot

Last synchronized: 2026-05-21.

- #137 remains open and is the parent tracking issue. Its comments record the child issue list and the integration branch/document workflow.
- #138 remains open on GitHub, but PR #147 merged `codex/issue-138-model-capabilities` into `codex/issue-137-llm-abstractions`.
- #142 remains open on GitHub, but PR #148 merged `codex/issue-142-llm-items` into `codex/issue-137-llm-abstractions`.
- #139 remains open on GitHub, but PR #149 merged `codex/issue-139-provider-runtime` into `codex/issue-137-llm-abstractions`.
- #141 remains open on GitHub, but PR #150 merged `codex/issue-141-llm-persistence` into `codex/issue-137-llm-abstractions`.
- #143 remains open on GitHub, but PR #151 merged `codex/issue-143-openai-responses-abstraction` into `codex/issue-137-llm-abstractions`.
- #144 remains open on GitHub, but PR #152 merged `codex/issue-144-ollama-shared-abstraction` into `codex/issue-137-llm-abstractions`.
- #140 remains open and pending behind the Ollama adapter stage.

## Current Architecture Facts

- `llm::Message` has been replaced by provider-neutral typed input/output item vocabulary.
- `LlmInputItem` and `LlmContentPart` now represent request-side LLM data before provider wire conversion.
- `LlmOutputItem` now reserves provider-neutral output vocabulary for follow-up runtime and persistence work.
- Conversation panel and temporary/shortcut flows now share the same typed history builder.
- `ProviderModel` now holds provider-neutral `ModelCapabilities` instead of the old streaming-only `ProviderModelCapability`.
- `ModelCapabilities` covers text input/output, streaming, image/file/audio input, image generation, tool calling, hosted web search, remote MCP, reasoning, structured output, stateful response continuation, and provider-specific typed extensions.
- OpenAI model classification now emits typed capabilities for Responses API usage, reasoning effort options, hosted web search, structured output, and stateful response continuation.
- Ollama `/api/show` metadata now maps into typed capabilities and an `OllamaModelCapabilities` extension for raw capabilities, family data, thinking mode, local web tools, and vision image input.
- `Provider` now builds `ProviderRunRequest` values from typed input items and streams provider-neutral `ProviderRunEvent` values.
- `ProviderRunRequest` keeps the provider request JSON snapshot so existing `messages.send_content` resend behavior remains compatible.
- `ProviderRunEvent` replaces `FetchUpdate` and covers thinking start, reasoning summary delta, text delta, output item added/done, tool call/result, MCP approval request, usage update, completed, and failed states.
- `ProviderRunState` and `ProviderUsage` are available for provider response/run metadata, output item ids, continuation metadata, and token usage, and #141 persists them additively for assistant messages.
- `message_run_states`, `message_output_items`, and `message_attachments` now persist provider run state, output item events, usage, and attachment metadata additively without changing `messages.content` or `messages.send_content`.
- `messages.content` stores rendered message content; `messages.send_content` stores the request body snapshot used for resend.
- OpenAI uses `/v1/responses`, reasoning effort, reasoning summaries, hosted web search citations, provider-neutral output item events, and persisted `previous_response_id` continuation when compatible run state is available.
- OpenAI adapter-specific Responses request fields such as `include`, `text`, `tool_choice`, `tools`, and `parallel_tool_calls` remain inside the OpenAI provider schema rather than the generic provider trait.
- Ollama has provider-specific thinking, image input, and experimental web search/fetch behavior that must not be forced into OpenAI-shaped types.
- Ollama image input accepts raw base64 or `data:image/...;base64,...` references only; URL, file-id, local-path, file, audio, and generic attachment inputs remain explicitly unsupported.
- Ollama run events now emit provider-neutral output item, tool call/result, usage, and completion data while keeping `ProviderRunState.run_id` empty and avoiding OpenAI-style continuation semantics.

## Shared Design Decisions

- The new LLM abstraction must be provider-neutral first. OpenAI Responses should be one adapter, not the shape of the core model.
- Provider/model capabilities must be represented with typed Rust structures, not only string metadata or arbitrary JSON.
- Provider-specific features should live in provider extension types that are attached to the generic model/provider metadata.
- The UI, templates, and shortcut flows should ask the generic capability model whether a feature is available.
- Existing pure-text conversations, templates, shortcuts, resend behavior, OpenAI, and Ollama must keep working during the migration.

## Implemented Capability Vocabulary

Issue #138 established these Rust capability types:

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

The current implementation keeps request execution behavior unchanged: OpenAI and Ollama still produce the same provider request JSON shape, but capability gating inside provider/template code now reads typed model capabilities instead of ad hoc JSON metadata or streaming-only enum state.

## Implemented Runtime Vocabulary

Issue #139 established these Rust runtime types:

- `ProviderRunRequest`
- `ProviderRunEvent`
- `ProviderRunState`
- `ProviderUsage`

The current implementation keeps request persistence additive: existing provider request JSON remains the compatibility snapshot for `messages.send_content`, while new runtime code uses `ProviderRunRequest` and `ProviderRunEvent` internally. OpenAI and Ollama still own their own wire/request conversion inside provider adapters.

## Provider Extension Rules

- Generic code may inspect common capabilities.
- Provider adapters may inspect provider-specific extension data.
- OpenAI-only concepts such as Responses output item ids, hosted tools, and remote MCP details must not become required fields for every provider.
- Ollama-only concepts such as `think` values and experimental local web search/fetch must remain expressible without pretending they are OpenAI tools.

## Persistence Direction

- Persistence changes should preserve old `messages.content` and `messages.send_content` compatibility.
- New run state should be additive first, so older conversations can still load and resend.
- Future persistence must be able to store provider response/run state, output items, tool calls, MCP approval state, attachments metadata, usage, model, and settings snapshots.
- Do not store binary attachment data directly inside message text.

## Validation Expectations

- Every child issue should run `cargo fmt` if it changes Rust files.
- Every child issue should run targeted tests for the changed subsystem.
- Integration-stage PRs to `main` must run:
  - `cargo build`
  - `cargo test`
  - `cargo clippy --all-targets --all-features -- -D warnings`
- UI or shortcut stages must include manual verification notes for old text chat, OpenAI, Ollama, resend, and shortcut flows.

## Completed Child Issue Notes

### #138 Provider-Neutral Model Capability Types

- Replaced `ProviderModelCapability` with `ModelCapabilities`.
- Re-exported the new capability vocabulary from `llm.rs` for downstream stages.
- Migrated OpenAI reasoning/web-search capability checks to typed capabilities.
- Migrated Ollama thinking/tool capability checks to typed `OllamaModelCapabilities`.
- Preserved existing OpenAI Responses request body, Ollama chat request body, template replay, streaming, and ext-setting behavior.
- Validation run:
  - `cargo fmt`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::preset`
  - `cargo test -p ai-chat state::chat::models`
  - `cargo test -p ai-chat components::chat_form::model_select`
  - `cargo test -p ai-chat features::settings::shortcut_settings`

### #142 Provider-Neutral Typed Input And Output Items

- Replaced the public LLM request item shape with provider-neutral `LlmInputItem` and `LlmContentPart` types.
- Added provider-neutral output vocabulary for message, reasoning, tool call/result, MCP approval, and hosted tool call items.
- Added a shared history builder for conversation panel and temporary/shortcut flows.
- Migrated OpenAI and Ollama provider request construction to accept typed input items and translate them inside each adapter.
- Preserved existing pure-text OpenAI Responses request bodies, Ollama chat request bodies, template replay, resend, and shortcut behavior.
- Non-text input parts now fail explicitly in current adapters instead of being silently dropped or coerced into text.
- Left full multimodal, tool output, MCP, stateful continuation, provider run events, and persistence changes to #139, #141, #143, and #144.
- Validation run:
  - `cargo fmt`
  - `cargo test -p ai-chat llm::types`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`

### #139 Run-Based Provider Trait And Events

- Replaced the old `FetchUpdate` stream path with provider-neutral `ProviderRunEvent`.
- Added `ProviderRunRequest`, `ProviderRunState`, and `ProviderUsage` as the first-stage runtime abstraction.
- Changed the `Provider` trait to build run requests with `build_run_request` and execute them with `run`.
- Kept `Provider::request_body` as a compatibility helper for persisted `messages.send_content` snapshots.
- Migrated conversation panel and temporary detail streaming consumers to `ProviderRunRunner`.
- Migrated OpenAI Responses stream parsing into provider-neutral events without exposing OpenAI event names in the core runtime.
- Added OpenAI parser coverage for completed, failed, incomplete, and top-level error events.
- Migrated Ollama chat streaming to the same run event vocabulary while keeping its experimental web search/fetch loop provider-local.
- Left generic app-level tool execution, MCP approval UI, and run-state database persistence to #141, #143, #144, and #140.
- Validation run:
  - `cargo fmt`
  - `cargo test -p ai-chat llm::types`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::provider::openai`
  - `cargo test -p ai-chat llm::provider::ollama`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #141 LLM Run State, Output Item, Tool, And Attachment Persistence

- Added database v6 and migrates legacy v1-v5 stores into `history_v6.sqlite3`.
- Preserved `messages.content` and `messages.send_content` as the compatibility surface for display, export, and resend.
- Added additive run persistence tables for assistant message run state, ordered output item events, and attachment metadata.
- Added typed message persistence wrappers around `ProviderRunState`, `ProviderUsage`, `LlmOutputItem`, and attachment refs.
- Conversation streaming now accumulates output item events, tool/MCP events, usage, and completed run state, then persists them with terminal message state.
- Temporary chat remains in-memory but carries run persistence when saved into a normal conversation.
- Resending an assistant message clears old run persistence while keeping the existing request body snapshot behavior.
- Validation run:
  - `cargo fmt`
  - `cargo test -p ai-chat database::migrations`
  - `cargo test -p ai-chat database::service`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo test -p ai-chat`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #143 OpenAI Responses Adapter Migration

- Added optional provider run state plumbing so OpenAI can build request bodies with `previous_response_id` while other providers keep existing request construction behavior.
- Conversation panel and temporary chat now use compatible persisted OpenAI assistant run state in contextual mode, trim history before that response, and fall back to full transcript behavior for non-contextual modes or incompatible state.
- OpenAI continuation is gated by matching persisted provider/model/run id, non-secret provider settings snapshot, and request context key. The request context key is the Responses request body with `input` and `previous_response_id` removed, so template/tool/reasoning/stream changes prevent stale continuation while input deltas do not.
- OpenAI request conversion now emits Responses content parts for text, image references, file references, tool results, and item references; unsupported audio or generic attachments fail explicitly.
- OpenAI stream and response parsing now maps message, reasoning, hosted tool, function-call, and MCP-related output items into provider-neutral events where existing core types can represent them.
- Function-call argument completion now yields `ToolCallRequested`; this stage intentionally does not add generic tool execution, MCP server configuration, approval UI, or capability-gated controls.
- Validation run:
  - `cargo fmt`
  - `cargo test -p ai-chat llm::run_persistence`
  - `cargo test -p ai-chat llm::provider::openai -- --nocapture`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel -- --nocapture`
  - `cargo test -p ai-chat features::temporary::detail -- --nocapture`
  - `cargo test -p ai-chat database::service -- --nocapture`
  - `cargo test -p ai-chat database::migrations -- --nocapture`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

### #144 Ollama Shared Abstraction Migration

- Mapped Ollama `vision` capability to generic `ModelCapabilities.image_input` without exposing OpenAI-only hosted web search, remote MCP, or stateful continuation settings.
- Kept Ollama thinking and experimental local web search/fetch as provider-specific extension behavior.
- Migrated Ollama input conversion beyond single-text messages: normal messages support multi-part text joined with blank lines and base64/data-URL image inputs; text-only tool results map to Ollama `role: "tool"` messages.
- Unsupported URL images, OpenAI file ids, local paths, files, audio, generic attachments, and item references fail explicitly instead of being silently coerced.
- Ollama stream and non-stream responses now emit provider-neutral output item events, tool call/result events, token usage with Ollama timing metadata, and completed content.
- Ollama run state remains additive and compatibility-oriented: no provider run id, no output item ids for continuation, and no OpenAI-shaped `previous_response_id` behavior.
- Validation run:
  - `cargo fmt`
  - `cargo test -p ai-chat llm::provider::ollama`
  - `cargo test -p ai-chat llm::provider`
  - `cargo test -p ai-chat llm::run_persistence`
  - `cargo test -p ai-chat features::home::tabs::conversation_panel`
  - `cargo test -p ai-chat features::temporary::detail`
  - `cargo clippy -p ai-chat --all-targets --all-features -- -D warnings`

## Next Child Issue Constraints

Next child issue is #140.

#140 should add template, shortcut, and UI capability gating on top of the shared OpenAI and Ollama capability model.
