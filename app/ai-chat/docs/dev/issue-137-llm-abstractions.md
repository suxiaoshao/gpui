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
| #138 | `codex/issue-138-model-capabilities` | Provider-neutral model capability types | Implemented locally; PR pending |
| #142 | `codex/issue-142-llm-items` | Typed input, content, and output items | Pending |
| #139 | `codex/issue-139-provider-runtime` | Run-based provider trait and events | Pending |
| #141 | `codex/issue-141-llm-persistence` | Run state, output items, tools, attachments persistence | Pending |
| #143 | `codex/issue-143-openai-responses-abstraction` | OpenAI Responses migration on shared abstraction | Pending |
| #144 | `codex/issue-144-ollama-shared-abstraction` | Ollama migration on shared abstraction | Pending |
| #140 | `codex/issue-140-capability-gating` | Template, shortcut, and UI capability gating | Pending |

## Current Architecture Facts

- `llm::Message` is currently text-only: role plus `String` content.
- `ProviderModel` now holds provider-neutral `ModelCapabilities` instead of the old streaming-only `ProviderModelCapability`.
- `ModelCapabilities` covers text input/output, streaming, image/file/audio input, image generation, tool calling, hosted web search, remote MCP, reasoning, structured output, stateful response continuation, and provider-specific typed extensions.
- OpenAI model classification now emits typed capabilities for Responses API usage, reasoning effort options, hosted web search, structured output, and stateful response continuation.
- Ollama `/api/show` metadata now maps into typed capabilities and an `OllamaModelCapabilities` extension for raw capabilities, family data, thinking mode, and local web tools.
- `Provider` currently builds provider request JSON and fetches a single response stream.
- `FetchUpdate` currently only covers thinking start, reasoning summary delta, text delta, and complete content.
- `messages.content` stores rendered message content; `messages.send_content` stores the request body snapshot used for resend.
- OpenAI already uses `/v1/responses`, reasoning effort, reasoning summaries, and hosted web search citations.
- Ollama has provider-specific thinking and experimental web search/fetch behavior that must not be forced into OpenAI-shaped types.

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

## Next Child Issue Constraints

Next child issue is #142.
It should introduce typed input, content, and output items on top of the `ModelCapabilities` foundation without forcing OpenAI Responses output items onto every provider.
It must keep existing text-only conversations, templates, shortcuts, resend behavior, OpenAI, and Ollama working while the new item model is added.
