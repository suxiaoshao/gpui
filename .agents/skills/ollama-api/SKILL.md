---
name: ollama-api
description: Reference skill for Ollama's native `/api/chat` and `/api/show` endpoints. Use when implementing, reviewing, or debugging an Ollama provider that needs chat inference, chat streaming, or full model-information retrieval from Ollama.
---

# Ollama API

## Overview

Use this skill as the local reference for Ollama's `/api/chat` and `/api/show` behavior. Prefer the bundled references over memory because the shipped OpenAPI file does not fully cover every implemented field.

## Workflow

For `/api/chat`, including request fields, non-stream and stream responses, read [chat-and-show-reference.md](references/chat-and-show-reference.md).

For `/api/show`, including request fields, full response structure, and implementation gaps relative to OpenAPI, read [chat-and-show-reference.md](references/chat-and-show-reference.md).

When a field appears in `api/types.go` but not in `docs/openapi.yaml`, trust the Go type definitions first, then confirm behavior against `server/routes.go` and `docs/api.md`.

## Rules

Treat `/api/chat` streaming as NDJSON, not SSE. Ollama streams newline-delimited JSON objects over `application/x-ndjson`.

Call out that `/api/chat` streams by default unless `stream: false`.

Call out provider-relevant `/api/chat` fields that are easy to miss:
`think`, `truncate`, `shift`, `keep_alive`, `logprobs`, and `top_logprobs`.

For `/api/show`, include fields that are present in actual Go types even when the OpenAPI file omits them, especially `modelfile`, `system`, `messages`, `projector_info`, `tensors`, and `requires`.

## References

- [chat-and-show-reference.md](references/chat-and-show-reference.md): `/api/chat` and `/api/show` request fields, response objects, and `/api/chat` stream behavior
