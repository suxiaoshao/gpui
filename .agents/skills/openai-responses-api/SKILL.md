---
name: openai-responses-api
description: Reference skill for OpenAI Responses API request fields, nested settings, response objects, output items, and streaming events. Use when writing, reviewing, debugging, or explaining code that calls `client.responses.create`, parses `response.output`, handles tool calls, uses structured output, multimodal input, conversation state, background mode, or SSE streaming.
---

# OpenAI Responses API

## Overview

Use this skill as the local source of truth for the OpenAI Responses API shape. Keep `SKILL.md` lean and load the reference files only when needed.

## Workflow

For request construction, parameter review, or SDK mapping, read [api-reference.md](references/api-reference.md).

For SSE handling, event ordering, or delta parsing, read [streaming-events.md](references/streaming-events.md).

When the user asks for "all parameters", "all return fields", or "what does this response/event mean", answer from the reference files first instead of relying on memory.

## Rules

Do not mix legacy Chat Completions terminology into Responses API answers unless the user explicitly asks for a migration comparison.

Call out model-specific support when a setting is not universal, especially `reasoning`, structured output, built-in tools, and verbosity.

Treat `response.output` as a heterogeneous list. Identify behavior from each item's `type` instead of assuming text-only output.

If a question is specifically about stream parsing, include the stable event lifecycle:
`response.created` -> item/content delta events -> `response.completed` or `response.failed` or `response.incomplete`.

## References

- [api-reference.md](references/api-reference.md): create request fields, nested objects, response object, common output item types
- [streaming-events.md](references/streaming-events.md): SSE event catalog, event order, and parser guidance
