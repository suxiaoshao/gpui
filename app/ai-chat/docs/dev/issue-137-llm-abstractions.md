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
| #138 | `codex/issue-138-model-capabilities` | Provider-neutral model capability types | Pending |
| #142 | `codex/issue-142-llm-items` | Typed input, content, and output items | Pending |
| #139 | `codex/issue-139-provider-runtime` | Run-based provider trait and events | Pending |
| #141 | `codex/issue-141-llm-persistence` | Run state, output items, tools, attachments persistence | Pending |
| #143 | `codex/issue-143-openai-responses-abstraction` | OpenAI Responses migration on shared abstraction | Pending |
| #144 | `codex/issue-144-ollama-shared-abstraction` | Ollama migration on shared abstraction | Pending |
| #140 | `codex/issue-140-capability-gating` | Template, shortcut, and UI capability gating | Pending |

## Current Architecture Facts

- `llm::Message` is currently text-only: role plus `String` content.
- `ProviderModelCapability` currently only differentiates streaming from non-streaming.
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

## Planned Capability Vocabulary

The first child issue should establish the exact Rust names, but the shared vocabulary is:

- Text input and text output
- Streaming
- Image input
- File input
- Audio input
- Image generation
- Tool calling
- Hosted web search
- Remote MCP
- Reasoning
- Structured output
- Stateful response continuation

Capability parameters should cover reasoning effort options, image limits, parallel tool calls, and stateful continuation support where applicable.

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

## Next Child Issue Constraints

Start with #138.
It should introduce provider-neutral capability types without changing request execution behavior yet.
It should migrate existing OpenAI and Ollama model metadata enough for current ext settings to continue working.
