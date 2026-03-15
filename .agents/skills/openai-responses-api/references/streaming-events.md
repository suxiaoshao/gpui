# OpenAI Responses API Streaming Events

## Sources

- Streaming events reference: <https://platform.openai.com/docs/api-reference/responses-streaming>
- Create response reference: <https://developers.openai.com/api/reference/resources/responses/methods/create>

Use this file when implementing or reviewing SSE parsers for `stream: true`.

## Core Lifecycle

The official streaming reference describes a stable high-level lifecycle:

1. `response.created`
2. `response.in_progress`
3. zero or more item/content/tool delta events
4. one terminal event:
   - `response.completed`
   - `response.failed`
   - `response.incomplete`

The stream may also emit a top-level `error` event.

Every event carries:

- `type`
- `sequence_number`

Most item-level events also carry:

- `output_index`
- `item_id`
- `content_index` when the event targets a message content part

## Response-Level Events

| Event | Meaning |
| --- | --- |
| `response.created` | Response object was created |
| `response.in_progress` | Response is actively generating |
| `response.completed` | Successful terminal state |
| `response.failed` | Terminal error state with `response.error` |
| `response.incomplete` | Terminal partial state with `response.incomplete_details` |

### `response.created`

The official example shows a full `response` object snapshot with fields like `id`, `status`, `model`, `output`, `usage`, and settings echoes.

### `response.failed`

The reference example includes:

- `response.status = "failed"`
- `response.error.code`
- `response.error.message`

### `response.incomplete`

The reference example includes:

- `response.status = "incomplete"`
- `response.incomplete_details.reason`

Treat `response.incomplete` as a first-class terminal branch, not as success.

## Output Item Events

| Event | Meaning |
| --- | --- |
| `response.output_item.added` | A new output item entered the stream |
| `response.output_item.done` | That output item is finalized |

The official `response.output_item.added` example looks like:

```json
{
  "type": "response.output_item.added",
  "output_index": 0,
  "item": {
    "id": "msg_123",
    "status": "in_progress",
    "type": "message",
    "role": "assistant",
    "content": []
  },
  "sequence_number": 1
}
```

Use `response.output_item.added` to allocate item state in your parser. Use `response.output_item.done` as the point where the item snapshot is complete.

## Content Part Events

| Event | Meaning |
| --- | --- |
| `response.content_part.added` | A new content part was attached to a message item |
| `response.content_part.done` | That content part is finalized |

The official `response.content_part.added` example shows:

```json
{
  "type": "response.content_part.added",
  "item_id": "msg_123",
  "output_index": 0,
  "content_index": 0,
  "part": {
    "type": "output_text",
    "text": "",
    "annotations": []
  },
  "sequence_number": 1
}
```

## Text Events

| Event | Meaning |
| --- | --- |
| `response.output_text.delta` | Incremental text bytes/chunks |
| `response.output_text.done` | Finalized text payload, optionally with logprobs |

The official delta example:

```json
{
  "type": "response.output_text.delta",
  "item_id": "msg_123",
  "output_index": 0,
  "content_index": 0,
  "delta": "In",
  "sequence_number": 1
}
```

The official `done` event includes:

- `item_id`
- `output_index`
- `content_index`
- `text`
- `logprobs`
- `sequence_number`

Parser rule:

- Append every `delta` to the targeted content buffer.
- Treat `done.text` as the authoritative finalized text for that part.

## Function Calling Events

| Event | Meaning |
| --- | --- |
| `response.function_call_arguments.delta` | Partial JSON-argument text |
| `response.function_call_arguments.done` | Finalized argument string plus function name |

The official delta example:

```json
{
  "type": "response.function_call_arguments.delta",
  "item_id": "item-abc",
  "output_index": 0,
  "delta": "{ \"arg\":",
  "sequence_number": 1
}
```

The official done event includes:

- `arguments`
- `item_id`
- `name`
- `output_index`
- `sequence_number`

Parser rule:

- Buffer the raw argument string exactly as streamed.
- JSON-parse only when the `done` event arrives or when your parser deliberately supports incremental validation.

## Refusal and Reasoning Text Events

The streaming reference also lists specialized delta/done pairs for:

- `response.refusal.delta`
- `response.refusal.done`
- `response.reasoning_summary_part.added`
- `response.reasoning_summary_part.done`
- `response.reasoning_summary_text.delta`
- `response.reasoning_summary_text.done`
- `response.reasoning_text.delta`
- `response.reasoning_text.done`

Treat these the same way as text deltas: route by `type`, `item_id`, and indexes instead of assuming all deltas are assistant-visible text.

## Tool-Specific Event Families

The current reference includes dedicated event families for built-in and platform tools. The table below is the useful grouping for parser design.

| Family | Events shown in reference |
| --- | --- |
| File search | `response.file_search_call.in_progress`, `.searching`, `.completed` |
| Web search | `response.web_search_call.in_progress`, `.searching`, `.completed` |
| Image generation | `response.image_generation_call.in_progress`, `.generating`, `.completed`, `.partial_image` |
| MCP | `response.mcp_call_arguments.delta`, `.done`, `response.mcp_call.in_progress`, `.completed`, `.failed`, `response.mcp_list_tools.in_progress`, `.completed`, `.failed` |
| Code interpreter | `response.code_interpreter_call.in_progress`, `.interpreting`, `.completed`, `response.code_interpreter_call_code.delta`, `.done`, `response.code_interpreter_call.output_text.annotation.added`, `.queued` |
| Custom tools | `response.custom_tool_call_input.delta`, `.done` |

Design implication:

- Your parser should dispatch on event `type`.
- Do not hard-code only text and function-call events if the app enables tools.

## Top-Level `error` Event

The streaming reference defines an `error` event with:

- `code`
- `message`
- `param`
- `sequence_number`
- `type = "error"`

Handle this even if you also expect a terminal `response.failed`.

## Practical Parser Strategy

1. Index response state by `response.id`.
2. Index output items by `output_index` and `item_id`.
3. Index content parts by `(item_id, content_index)`.
4. Append delta payloads by event family.
5. Replace buffered drafts with the corresponding `done` payload when present.
6. Finalize only on `response.completed`, `response.failed`, or `response.incomplete`.

## Common Mistakes

- Assuming the stream only emits text.
- Assuming `response.completed` is the only terminal event.
- Parsing function arguments before the final `done`.
- Ignoring `sequence_number`.
- Ignoring tool-specific event families when tools are enabled.
- Treating `output_index` as stable without also tracking `item_id`.
