# Ollama `/api/chat` And `/api/show` Reference

## Sources

- Repo: <https://github.com/ollama/ollama>
- Commit inspected for this cache: `f8b657c9670a4319930e8d7e5444460df91a7b5d`
- Primary schema source: `api/types.go`
- Secondary sources: `server/routes.go`, `docs/openapi.yaml`, `docs/api.md`

This file intentionally covers only the two native endpoints needed for the provider work:

- `POST /api/chat`
- `POST /api/show`

## Source-of-Truth Order

Use this precedence when fields disagree:

1. `api/types.go`
2. `server/routes.go`
3. `docs/openapi.yaml`
4. `docs/api.md`

## `POST /api/chat`

This is Ollama's main message-based inference endpoint.

### Request body

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `model` | `string` | yes | Model name |
| `messages` | `Message[]` | yes | Chat history |
| `stream` | `boolean` | no | Defaults to `true` |
| `format` | `"json" | object` | no | JSON mode or JSON Schema |
| `keep_alive` | `string | number` | no | Example: `5m`, `0` |
| `tools` | `ToolDefinition[]` | no | Function tools |
| `options` | `object` | no | Model/runtime options |
| `think` | `boolean | "high" | "medium" | "low"` | no | Thinking control |
| `truncate` | `boolean` | no | Truncate history on overflow |
| `shift` | `boolean` | no | Shift history instead of erroring |
| `_debug_render_only` | `boolean` | no | Debug-only |
| `logprobs` | `boolean` | no | Include token logprobs |
| `top_logprobs` | `integer` | no | Must be `0..20` |

### `messages` item shape

Each `messages[]` item is:

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `role` | `string` | yes | Common values: `system`, `user`, `assistant`, `tool` |
| `content` | `string` | yes | Message text |
| `thinking` | `string` | no | Present in thinking-enabled flows |
| `images` | `string[]` | no | Base64-encoded images |
| `tool_calls` | `ToolCall[]` | no | Tool requests from assistant |
| `tool_name` | `string` | no | Tool result metadata |
| `tool_call_id` | `string` | no | Tool result correlation ID |

### `tools` item shape

Ollama currently uses OpenAI-style function tools:

```json
{
  "type": "function",
  "function": {
    "name": "get_weather",
    "description": "Get the weather",
    "parameters": {
      "type": "object",
      "properties": {
        "city": { "type": "string" }
      },
      "required": ["city"]
    }
  }
}
```

### Non-stream response

When `stream: false`, `/api/chat` returns a single JSON object.

| Field | Type | Notes |
| --- | --- | --- |
| `model` | `string` | Model used |
| `remote_model` | `string` | Present for upstream/cloud-backed models |
| `remote_host` | `string` | Present for upstream/cloud-backed models |
| `created_at` | `string` | ISO 8601 timestamp |
| `message` | `Message` | Assistant output |
| `done` | `boolean` | Usually `true` |
| `done_reason` | `string` | Example: `stop`, `unload` |
| `total_duration` | `integer` | Nanoseconds |
| `load_duration` | `integer` | Nanoseconds |
| `prompt_eval_count` | `integer` | Prompt token count |
| `prompt_eval_duration` | `integer` | Nanoseconds |
| `eval_count` | `integer` | Output token count |
| `eval_duration` | `integer` | Nanoseconds |
| `logprobs` | `Logprob[]` | Present when requested |

### `message` in the response

The returned `message` object may contain:

| Field | Type | Notes |
| --- | --- | --- |
| `role` | `string` | Usually `assistant` |
| `content` | `string` | Assistant text |
| `thinking` | `string` | Thinking trace when enabled |
| `tool_calls` | `ToolCall[]` | Assistant tool calls |
| `images` | `string[]` | Image payloads when applicable |

### Stream behavior

`/api/chat` streams by default. The response content type is NDJSON:

- `application/x-ndjson`
- one JSON object per line
- not SSE
- no event names

Each streamed chunk is a `ChatStreamEvent`-shaped object:

| Field | Type | Notes |
| --- | --- | --- |
| `model` | `string` | Model used |
| `created_at` | `string` | ISO 8601 timestamp |
| `message` | `object` | Partial assistant message |
| `done` | `boolean` | `false` until the terminal chunk |

The streamed `message` object may contain partial values for:

| Field | Type |
| --- | --- |
| `role` | `string` |
| `content` | `string` |
| `thinking` | `string` |
| `tool_calls` | `ToolCall[]` |
| `images` | `string[]` |

### Final stream chunk

The final NDJSON object is effectively the full chat response and can include:

| Field | Type |
| --- | --- |
| `model` | `string` |
| `created_at` | `string` |
| `message` | `Message` |
| `done` | `boolean` |
| `done_reason` | `string` |
| `total_duration` | `integer` |
| `load_duration` | `integer` |
| `prompt_eval_count` | `integer` |
| `prompt_eval_duration` | `integer` |
| `eval_count` | `integer` |
| `eval_duration` | `integer` |
| `logprobs` | `Logprob[]` |

Practical parser rule:

1. Append `message.content` as chunks arrive.
2. Append `message.thinking` separately if present.
3. Collect `message.tool_calls` when they appear.
4. Treat the object with `done: true` as terminal and authoritative for metrics.

### Validation and edge cases

- `top_logprobs` outside `0..20` returns `400`
- missing request body returns `400`
- unknown model returns `404`
- empty `messages` with `keep_alive: 0` unloads the runner and returns a terminal response with:
  - empty assistant `message`
  - `done: true`
  - `done_reason: "unload"`

### Example request

```json
{
  "model": "qwen3",
  "messages": [
    {
      "role": "user",
      "content": "What is the weather in Shanghai?"
    }
  ],
  "stream": false,
  "think": "low",
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get weather by city",
        "parameters": {
          "type": "object",
          "properties": {
            "city": { "type": "string" }
          },
          "required": ["city"]
        }
      }
    }
  ]
}
```

## `POST /api/show`

This is the endpoint that returns full model information.

### Request body

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `model` | `string` | yes | Preferred field |
| `system` | `string` | no | Optional resolution/system override |
| `template` | `string` | no | Deprecated |
| `verbose` | `boolean` | no | Include large verbose metadata |
| `options` | `object` | no | Additional options |
| `name` | `string` | no | Deprecated alias for `model` |

Implementation note:

- If `model` is empty and `name` is present, Ollama copies `name` into `model`.
- If neither is present, the route returns `400`.

### Response body

`/api/show` returns a single JSON object with model details.

| Field | Type | Notes |
| --- | --- | --- |
| `license` | `string` | License text |
| `modelfile` | `string` | Full Modelfile text |
| `parameters` | `string` | Parameter block as text |
| `template` | `string` | Prompt template |
| `system` | `string` | System prompt |
| `renderer` | `string` | Renderer name |
| `parser` | `string` | Parser name |
| `details` | `ModelDetails` | High-level model details |
| `messages` | `Message[]` | Seeded model messages |
| `remote_model` | `string` | Upstream model name |
| `remote_host` | `string` | Upstream Ollama host |
| `model_info` | `object` | Large metadata map |
| `projector_info` | `object` | Projector metadata for multimodal models |
| `tensors` | `Tensor[]` | Tensor metadata |
| `capabilities` | `string[]` | Examples: `completion`, `vision` |
| `modified_at` | `string` | ISO 8601 timestamp |
| `requires` | `string` | Minimum Ollama version requirement |

### `details` shape

| Field | Type |
| --- | --- |
| `parent_model` | `string` |
| `format` | `string` |
| `family` | `string` |
| `families` | `string[]` |
| `parameter_size` | `string` |
| `quantization_level` | `string` |

### `tensors` item shape

| Field | Type |
| --- | --- |
| `name` | `string` |
| `type` | `string` |
| `shape` | `number[]` |

### What `verbose: true` changes

`verbose: true` is mainly relevant for large metadata arrays inside `model_info`, for example tokenizer tables such as:

- `tokenizer.ggml.tokens`
- `tokenizer.ggml.token_type`
- `tokenizer.ggml.merges`

Without `verbose`, these large fields may be omitted or collapsed.

### Example response shape

```json
{
  "modelfile": "# Modelfile generated by ollama show ...",
  "parameters": "temperature 0.7\nnum_ctx 4096",
  "template": "{{ .System }} ...",
  "system": "You are a helpful assistant.",
  "details": {
    "parent_model": "",
    "format": "gguf",
    "family": "llama",
    "families": ["llama"],
    "parameter_size": "8.0B",
    "quantization_level": "Q4_0"
  },
  "model_info": {
    "general.architecture": "llama",
    "general.parameter_count": 8030261248
  },
  "capabilities": ["completion", "vision"],
  "modified_at": "2025-08-14T15:49:43.634137516-07:00"
}
```

### Error behavior

- missing request body -> `400`
- missing `model` and `name` -> `400`
- invalid model reference -> `400`
- local model not found -> `404`

## Provider Summary

If the provider only needs one inference endpoint and one model-info endpoint:

- use `/api/chat` for all text/tool/vision chat interactions
- use `/api/show` for complete per-model metadata

Avoid designing around `/api/generate` if the provider contract is strictly chat-oriented.
