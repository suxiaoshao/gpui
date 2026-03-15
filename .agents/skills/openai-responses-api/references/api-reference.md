# OpenAI Responses API Reference

## Sources

- Create response reference: <https://developers.openai.com/api/reference/resources/responses/methods/create>
- Streaming reference: <https://platform.openai.com/docs/api-reference/responses-streaming>
- Responses guide: <https://platform.openai.com/docs/guides/responses>

This file is a working reference derived from the official OpenAI docs available on 2026-03-15. Treat it as the project's cached documentation. If OpenAI later adds fields, prefer updating this skill instead of guessing.

## Endpoint Shape

- Primary endpoint: `POST /v1/responses`
- Primary object returned: `response`
- Common SDK entrypoints:
  - JavaScript: `client.responses.create(...)`
  - Python: `client.responses.create(...)`

## Top-Level Request Fields

These are the documented request fields shown on the create reference page.

| Field | Type | Notes |
| --- | --- | --- |
| `model` | `string` | Model ID to run. Required in normal usage. |
| `input` | `string | ResponseInputItem[]` | Text shortcut or full multimodal/item-based input. |
| `instructions` | `string` | Injected system/developer-style instruction block. When combined with `previous_response_id`, prior instructions are not carried forward. |
| `background` | `boolean` | Run the response asynchronously in background mode. |
| `context_management` | `{ type, compact_threshold }` | Context compaction settings. Current documented object type is `compaction`. |
| `conversation` | `string | { id: string }` | Attach the response to a conversation state container. |
| `include` | `string[]` | Request additional payloads that are omitted by default. |
| `max_output_tokens` | `number` | Upper bound across visible output tokens and reasoning tokens. |
| `max_tool_calls` | `number` | Upper bound across all built-in tool calls in the response. |
| `metadata` | `map[string]string` | Small app-defined metadata map. |
| `parallel_tool_calls` | `boolean` | Allow multiple tool calls in parallel when supported. |
| `previous_response_id` | `string` | Continue from an earlier response without resending all prior items manually. |
| `prompt` | `object` | Hosted prompt/template reference plus version, variables, tools, and file inputs when used. |
| `prompt_cache_key` | `string` | Stable cache bucketing key. Replaces `user` for caching guidance. |
| `prompt_cache_retention` | `"in-memory" | "24h" | null` | Prompt cache retention policy. |
| `reasoning` | `object` | Reasoning-model configuration. |
| `safety_identifier` | `string` | Stable hashed end-user identifier for abuse detection. Max length 64. |
| `service_tier` | `"auto" | "default" | "flex" | "priority" | null` | Requested processing tier. Response may echo the actual tier used. |
| `store` | `boolean` | Persist the response for later retrieval. |
| `stream` | `boolean` | Return SSE events instead of a single JSON payload. |
| `stream_options` | `object` | Stream-specific options such as obfuscation behavior. |
| `temperature` | `number` | Sampling temperature. |
| `text` | `object` | Text output formatting and verbosity controls. |
| `tool_choice` | `string | object` | Constrain whether and which tool the model may call. |
| `tools` | `Tool[]` | Built-in tools, MCP tools, or custom/function tools the model may call. |
| `top_logprobs` | `number` | Return token logprobs. Integer `0..20`. |
| `top_p` | `number` | Nucleus sampling alternative to temperature. |
| `truncation` | `"disabled" | "auto"` | Context truncation strategy. |
| `user` | `string` | Deprecated. Prefer `safety_identifier` and `prompt_cache_key`. |

## `include` Values

The create reference currently lists these includable expansions:

- `file_search_call.results`
- `web_search_call.results`
- `web_search_call.action.sources`
- `message.input_image.image_url`
- `computer_call_output.output.image_url`
- `code_interpreter_call.outputs`
- `reasoning.encrypted_content`
- `message.output_text.logprobs`

Use `include` when a downstream parser needs data that is intentionally omitted from default responses.

## `input` Shapes

`input` supports a plain string shortcut or an explicit array of input items.

### Simple form

```json
{
  "model": "gpt-5.1",
  "input": "Summarize this file."
}
```

This is equivalent to a single `user` text item.

### Item-based form

An explicit `input` array is the general form. Common item/content types documented on the create page include:

| Shape | Purpose |
| --- | --- |
| `EasyInputMessage { role, content, phase?, type? }` | Normal user/system/developer/assistant message wrapper |
| `input_text` | Text content item |
| `input_image` | Image content item via `file_id` or `image_url`, with `detail` |
| `input_file` | File content item, typically by `file_id` |
| `input_audio` | Audio content item where supported |
| `item_reference` | Reference an existing prior item |

Key points:

- `developer` and `system` roles have higher instruction priority than `user`.
- `assistant` items can be included as prior model outputs.
- Multimodal content is carried in a message's `content` array.

## `text` Object

`text` controls the format of assistant text output.

### `text.format`

Documented formats:

- `{ "type": "text" }`
- `{ "type": "json_object" }`
- `{ "type": "json_schema", "name": "...", "schema": { ... }, "description"?: "...", "strict"?: true }`

Guidance:

- Prefer `json_schema` over `json_object` for structured outputs.
- `json_object` is the older JSON mode.
- `strict: true` enforces the supported subset of JSON Schema more tightly.

### `text.verbosity`

Documented values:

- `"low"`
- `"medium"`
- `"high"`

Treat this as a length/detail control, not a hard token cap.

## `reasoning` Object

Documented shape:

```json
{
  "effort": "none | minimal | low | medium | high | xhigh",
  "generate_summary": "concise | detailed | auto",
  "summary": "concise | detailed | auto"
}
```

Important notes from the official reference:

- `reasoning` is for `gpt-5` and `o`-series reasoning models.
- Support is model-specific.
- `gpt-5.1` defaults to `none`.
- Models before `gpt-5.1` default to `medium` and do not support `none`.
- `gpt-5-pro` only supports `high`.
- `xhigh` is supported on models after `gpt-5.1-codex-max`.

## `tool_choice`

Documented choices include:

- `"none"`
- `"auto"`
- `"required"`
- A named tool object that forces a specific tool
- An allowed-tools object that restricts the tool set and mode

Use cases:

- `"none"`: forbid all tool calls
- `"auto"`: let the model decide whether to call tools
- `"required"`: force at least one tool call
- Named/object form: force or constrain a specific tool or subset

## `tools`

The official docs group tools into three categories:

- Built-in tools from OpenAI
- MCP tools
- Function/custom tools defined by you

### Tool families present on the OpenAPI `tools` union

The current OpenAPI spec shows these tool families on the `tools` union:

- `function`
- `custom`
- `file_search`
- `web_search_preview`
- `computer-preview`
- `code_interpreter`
- `image_generation`
- `mcp`

When documenting or reviewing code, always identify the tool by its `type` first, then inspect that tool's dedicated object shape.

### Function tools

Typical function definition shape:

```json
{
  "type": "function",
  "name": "lookup_weather",
  "description": "Fetch current weather",
  "parameters": {
    "type": "object",
    "properties": {
      "city": { "type": "string" }
    },
    "required": ["city"]
  },
  "strict": true
}
```

### MCP tools

MCP tools are configured as tool entries that point to an MCP server and optional tool name constraints. Use them when the model should call external connectors or custom MCP servers.

## Prompt and Conversation State

### `previous_response_id`

Use this to continue a stateless chain from a prior response ID without resending the entire transcript manually.

### `conversation`

Use this to bind the response to a conversation container managed by the API. The OpenAPI description states that conversation items are prepended to the request and new input/output items are added automatically after completion.

### `prompt`

The response object example shows `prompt` as:

```json
{
  "id": "prompt_id",
  "variables": {
    "foo": "bar"
  },
  "version": "version"
}
```

The OpenAPI schema also shows prompt-related fields such as:

- `id`
- `version`
- `variables`
- `messages`
- `tools`
- `files`
- `service_tier`

## Response Object

The returned object type is `response`. The example in the official reference shows these top-level fields:

| Field | Meaning |
| --- | --- |
| `id` | Response ID |
| `object` | Always `response` |
| `created_at` | Unix timestamp |
| `status` | Lifecycle status such as `in_progress`, `completed`, `failed`, or `incomplete` |
| `completed_at` | Completion timestamp when available |
| `error` | Error object when the response fails |
| `incomplete_details` | Why the response stopped early, for example `max_output_tokens` |
| `instructions` | Effective instructions carried on the response |
| `metadata` | Echoed metadata map |
| `model` | Model used |
| `output` | Heterogeneous output item array |
| `output_text` | Convenience flattened text when applicable |
| `parallel_tool_calls` | Echoed setting |
| `previous_response_id` | Prior response linkage |
| `prompt` | Prompt/template descriptor when used |
| `prompt_cache_key` | Echoed cache key |
| `prompt_cache_retention` | Echoed retention policy |
| `reasoning` | Reasoning settings/summary data |
| `background` | Background mode flag |
| `conversation` | Conversation descriptor |
| `max_output_tokens` | Echoed token cap |
| `max_tool_calls` | Echoed built-in tool cap |
| `safety_identifier` | Echoed safety identifier |
| `service_tier` | Actual tier used |
| `store` | Whether the response is stored |
| `temperature` | Echoed sampling value |
| `text` | Echoed text formatting object |
| `tool_choice` | Echoed tool selection mode |
| `tools` | Echoed tool declarations |
| `top_logprobs` | Echoed logprob cap |
| `top_p` | Echoed nucleus value |
| `truncation` | Echoed truncation setting |
| `usage` | Token accounting |
| `user` | Deprecated echoed user field |

## `usage`

The official example shows:

```json
{
  "input_tokens": 81,
  "input_tokens_details": {
    "cached_tokens": 0
  },
  "output_tokens": 1035,
  "output_tokens_details": {
    "reasoning_tokens": 832
  },
  "total_tokens": 1116
}
```

Practical meaning:

- `input_tokens`: total prompt/input tokens counted
- `cached_tokens`: prompt tokens served from cache
- `output_tokens`: all generated tokens
- `reasoning_tokens`: hidden reasoning token usage when applicable
- `total_tokens`: input + output

## `output` Item Model

Do not assume `output[0]` is always a text message. The array can mix message items and tool-related items.

### Common item types to expect

| Item `type` | Meaning |
| --- | --- |
| `message` | Assistant/user/system/developer message item |
| `function_call` | Model-issued custom function call |
| `function_call_output` | Tool result returned to the model |
| `reasoning` | Reasoning item or summary |
| `web_search_call` | Built-in web search invocation/result |
| `file_search_call` | Built-in retrieval/file search invocation/result |
| `code_interpreter_call` | Code interpreter execution item |
| `image_generation_call` | Image generation item |
| `computer_call` / `computer_call_output` | Computer use item/result |
| `mcp_call` / `mcp_list_tools` | MCP execution/listing item |
| `custom_tool_call` | Custom tool execution item |

### Message item shape

The message item in the official example contains:

```json
{
  "id": "msg_...",
  "type": "message",
  "role": "assistant",
  "status": "completed",
  "content": [
    {
      "type": "output_text",
      "text": "...",
      "annotations": []
    }
  ]
}
```

### Message content parts

Common content-part types:

- `output_text`
- `refusal`
- Tool-specific annotation payloads

If `include` requests logprobs, the `output_text` part may also include token logprob detail.

## Minimal Examples

### Plain text

```json
{
  "model": "gpt-5.1",
  "input": "Write a haiku about compilers."
}
```

### Structured JSON

```json
{
  "model": "gpt-5.1",
  "input": "Extract invoice data.",
  "text": {
    "format": {
      "type": "json_schema",
      "name": "invoice_extract",
      "schema": {
        "type": "object",
        "properties": {
          "invoice_number": { "type": "string" },
          "total": { "type": "number" }
        },
        "required": ["invoice_number", "total"]
      },
      "strict": true
    }
  }
}
```

### Function calling

```json
{
  "model": "gpt-5.1",
  "input": "What's the weather in Shanghai?",
  "tool_choice": "auto",
  "tools": [
    {
      "type": "function",
      "name": "lookup_weather",
      "description": "Get current weather",
      "parameters": {
        "type": "object",
        "properties": {
          "city": { "type": "string" }
        },
        "required": ["city"]
      },
      "strict": true
    }
  ]
}
```

## Review Checklist

When using this reference to review code, verify:

- The code uses `responses.create`, not a legacy endpoint by mistake.
- `input` shape matches the model capability being used.
- Structured output uses `json_schema` where possible.
- Tool items are parsed by `type`, not by array position.
- Streaming code handles non-text events, not just `output_text.delta`.
- Deprecated `user` is not introduced unless there is a compatibility reason.
