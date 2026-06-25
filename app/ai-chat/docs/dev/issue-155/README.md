# Issue #155 ai-chat2、crate 拆分和 fresh database

本目录保存 #155 相关设计文档。#155 的目标是固定 `ai-chat2` 并行重构、crate 拆分、fresh database schema 和 typed data model。

## 文档

| 文档 | 用途 |
| --- | --- |
| `fresh-database-schema.md` | SQLite-first fresh store、typed payload、repository 和 migration 设计。 |

## 边界

- `app/ai-chat2` 是新的 GPUI shell。
- `crates/ai-chat-core` 保存领域数据契约。
- `crates/ai-chat-db` 保存 fresh SQLite schema、typed repositories 和 migrations。
- `crates/ai-chat-agent` 保存 Rig adapter、agent loop、tool registry、skills、MCP 和 approval runtime。
- legacy `app/ai-chat` 不在 #155 中强制迁移。
