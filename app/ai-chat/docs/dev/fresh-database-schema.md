# ai-chat 全新数据库模型

本文档是 issue #155 的设计源文档。它定义全新的 SQLite 数据库模型、规范化的 Rust 数据模型、所有 app 自有 JSON payload 的 Rust 类型、provider 单次调用和未来 agent runtime 之间的边界，以及在 `app/ai-chat2` 和独立 crates 中落地的重构策略。

设计原则是 SQLite-first，但不是把所有细节都拆成关系表。稳定身份、排序、状态和高频查询字段使用列；复杂的 transcript 和 runtime payload 使用 SQL `JSON` 列，但每个 JSON 列都必须由明确的 Rust 类型拥有。

## 核心原则

- 项目就是一个真实存在的文件夹。scratch / 无项目对话也必须由 app 创建真实 scratch 文件夹，并写入 `projects` 行。
- 每个对话都必须属于一个项目，不存在 `project_id = NULL` 的对话。
- 所有对话都是 contextual。全新模型没有 `contextual` / `single` / `assistant-only` mode。
- 删除模板概念。保存下来的系统/开发者指令叫 `prompts`，UI 中使用“提示词”。
- 快捷键绑定提示词、provider/model、输入来源和动作，不再绑定 template 或 mode。
- provider 模型列表采用“设置页手动刷新 + 启动时读缓存”的方式。不要每次启动都访问 provider API。
- fresh model 不直接塞回旧 `app/ai-chat`。落地方式是新建 `app/ai-chat2` 和一组可复用 crates，旧 app 保持可用，后续按边界迁移。
- `app/ai-chat2` 必须是薄 GPUI shell。领域模型、数据库、provider adapter、agent runtime 不应继续堆在 app 目录里。
- v1-v6 数据库都是 legacy 存储。不要覆盖旧库，也不要要求完整自动迁移。
- `ProviderRunRequest`、`ProviderRunEvent`、`ProviderRunState` 只保留为低层 provider-step adapter 边界，不是持久化的 canonical transcript model。
- provider request JSON 只存放在 `provider_steps`，作为 debug/replay snapshot，不作为聊天历史。
- 对话展示、上下文重建、导出、重发的热路径只能读取 `conversation_items`。`agent_runs`、`provider_steps`、`tool_invocations`、`approval_decisions`、`usage_events` 是执行索引、恢复索引或统计索引，不能成为 timeline 渲染必须 join 的来源。
- Skills 和 MCP server 配置是文件/配置驱动的 runtime inputs，不是 chat database 的 source tables。数据库不应新增 `skills`、`skill_roots`、`mcp_servers` 或 `mcp_tools` 这类可从文件/配置和运行时连接恢复的定义表。
- 已加载进某次对话上下文的 skill 内容应作为当次 transcript 快照写入 `conversation_items`，用于历史重放、导出和 debug；这不改变 `SKILL.md` 文件才是 reusable skill source of truth 的边界。
- app 自有结构化 payload 列必须使用 SQL `JSON`，不能用 `TEXT` 列保存 JSON 字符串。SQLite 的运行时 affinity 不是把 JSON 建模为 TEXT 的理由，migration、Diesel schema 和 repository API 都必须表达为 JSON。
- 二进制附件数据不能存进 message text，也不能塞进 JSON payload。数据库只保存 metadata、引用、路径和 hash。

## 参考实现取舍

本设计不是照搬任何一个应用。参考结论如下：

| 应用 | 观察到的存储方式 | 可借鉴点 | 不直接采用的点 |
| --- | --- | --- | --- |
| Zed | Agent thread 内容存成一个 typed JSON document，并以 zstd 压缩后放在 `threads.data BLOB`；列表需要的 `summary`、`updated_at`、`created_at`、`folder_paths` 等保留为列。Skills 从 `SKILL.md` 文件读取，catalog 只暴露 name、description、location；MCP server 从 `context_servers` settings 读取。 | 热路径避免把一条对话拆成很多表；列表查询只读 metadata；项目路径和 thread metadata 面向 UI 反规范化；skill/MCP source 不进聊天数据库。 | 不把 ai-chat transcript 整体压成单个 BLOB，因为我们需要 item 级搜索、streaming 增量写入和部分恢复。 |
| Codex | Canonical session 是 append-only rollout JSONL；SQLite `threads` 表保存 `rollout_path`、`cwd`、title、preview、tokens、git、archive 等索引字段；dynamic tools、spawn edges 这类需要独立查询的关系才单独建表。Skills 由 `SKILL.md` 路径和 metadata 驱动，注入时读文件；MCP server 来自 config `[mcp_servers]`。 | “日志为真相，数据库为索引”的边界；thread 归属真实 `cwd`；启动时可从 canonical log backfill metadata index；skill/MCP 定义属于文件/配置层。 | ai-chat 仍使用 SQLite-first，不把 canonical transcript 放到外部 JSONL 文件；但 `conversation_items` 必须保持 append-only log 的形状。 |
| pi | Session 按 cwd 编码后存到 `~/.pi/agent/sessions/<encoded-cwd>/*.jsonl`；文件首行为 session header，后续 entry 有 `id`、`parentId`、`type`，通过当前 leaf 回溯构建 LLM context；compaction、model change、label、custom entry 都是一等 entry。 | 项目/目录组织对话是正确方向；append-only typed entries 比多表 join 更适合 transcript；context reconstruction 应由 canonical entry path 得到。 | 不照搬文件夹 JSONL 作为主存储；不把扩展数据变成无限制 payload，app 自有 JSON 仍必须有 Rust 类型。 |
| Alma | 大量 agent-app 数据都进入 SQLite，并把 thread/message、agent run、tool、provider/model cache、usage、memory 等边界拆开。 | provider/model cache、usage、MCP/OAuth、agent execution 与 transcript 分离是正确的产品边界。 | 不复制 Alma 的 JSON-like text 列、chat message payload shape 或过度规范化表设计；它不能作为唯一参考。 |

因此 ai-chat 采用混合模型：SQLite 管理事务、索引和设置；`conversation_items.payload_json` 是 append-only typed transcript document；其他表只服务列表、恢复、统计、设置和调试。

## 并行应用和 crate 拆分策略

这次重构不应继续在旧 `app/ai-chat` 内做大范围原地替换。旧 app 已经混合了 legacy database、旧 messages schema、provider compatibility、template/mode 兼容逻辑和 UI 状态；直接在里面改会让 lint、迁移和回归验证被历史债务拖住。

推荐采用并行 app 重写：

```text
app/ai-chat/              # 现有应用，保持可运行和可回退
app/ai-chat2/             # 新应用，薄 GPUI shell，只做窗口、路由、UI 状态组合
crates/ai-chat-core/      # 领域数据契约：project、conversation item、content、prompt、run/tool 类型
crates/ai-chat-db/        # fresh SQLite schema、migrations、typed repositories
crates/ai-chat-agent/     # Rig adapter、agent loop、tool registry、approval、continuation、cancel/retry
```

拆分边界：

- `ai-chat-core` 不依赖 GPUI、Diesel、HTTP client 或具体 provider。它只定义可序列化的数据契约和核心不变量。
- `ai-chat-db` 依赖 `ai-chat-core`，负责 SQL `JSON` typed roundtrip、migration 和 repository transaction。全文搜索/FTS 暂未实现。
- `ai-chat-agent` 依赖 `ai-chat-core` 和 repository traits，负责把 canonical context 转换为 Rig messages、观测 provider step、执行多步 loop、tool execution、approval 和写入 canonical items。
- `app/ai-chat2` 依赖这些 crates，但不拥有长期业务模型。UI 只消费 repository/agent API 和 `ai-chat-core` 类型。

迁移策略：

- #155 只固定文档、schema、Rust 数据契约和 issue 重排。
- #156 先 scaffold `ai-chat-core` 和 `ai-chat-db`，并让 fresh database bootstrap 和 repository tests 可以独立通过；可同时创建最小 `app/ai-chat2` 壳，但不要求迁移 UI。
- #157 清理 `ai-chat-db` fresh schema 的 SQLite 时间、布尔和 closed enum/status 类型约束。
- #158 在 `ai-chat-agent` 中实现 Rig adapter、agent loop 和持久化写入。
- #159 再让 `app/ai-chat2` 使用新 crates 渲染项目、对话、timeline、tool、approval 和多模态内容。
- 旧 `app/ai-chat` 在迁移期间作为 legacy app 保持可用。legacy database 的读取、导出、导入或只读浏览策略必须显式实现，不能默默接入 fresh store。
- 不预设“全部迁完后必须合并回 `app/ai-chat`”。如果拆出的 crates 是清晰边界，应保留它们；最终可以让 `app/ai-chat2` 替换旧 app，也可以让旧 app 逐步改为使用这些 crates。

这样做的目标不是多建目录，而是把大范围重构拆成可独立 lint、test 和 review 的边界。每个 crate 应能用 focused `cargo test -p ...` 验证自身，不被旧 app 的无关 lint 债务阻塞。

## ID 和时间约定

- 全新表使用 app 生成的 `TEXT` id。实现时应统一一种格式，例如 UUID v7 或 ULID。
- 初始版本采用线性 append-only timeline。对话内的人类可见顺序使用 `(conversation_id, seq)`，`seq` 单调递增。
- pi 式 `parentId` / leaf context tree 可作为未来 branching/edit-history 能力，但不进入 #155-#159 的第一阶段。如果产品需要同一对话内 fork、可回溯编辑或多 leaf 并存，应先增加 `conversation_branches`、`parent_item_id` 或 `revision_of_item_id` 等字段，再实现 UI；第一阶段的编辑、重试或分叉应追加新 item 或新建 conversation。
- agent run 内的 provider step 顺序使用 `(agent_run_id, seq)`。
- 时间统一使用 UTC ISO-8601 字符串，除非 repository 层明确统一到另一个 Diesel SQLite 时间编码。
- 打开数据库后必须启用 `PRAGMA foreign_keys = ON`。

## 表总览

| 表 | 作用 | 关键列 | JSON payload 类型 |
| --- | --- | --- | --- |
| `schema_migrations` | 数据库内部 migration ledger | `name TEXT PRIMARY KEY`, `executed_at TEXT NOT NULL` | 无 |
| `schema_metadata` | 随数据库携带的 schema/app 元信息 | `id TEXT PRIMARY KEY DEFAULT 'default'`, `schema_version INTEGER`, app 版本列 | `SchemaMetadataPayload` |
| `projects` | 真实文件夹项目，包含 scratch 项目 | `id`, `path UNIQUE`, `display_name`, `kind`, 时间戳 | `ProjectMetadata` |
| `conversations` | 对话元数据，每个对话必须属于一个项目 | `id`, `project_id`, `title`, `status`, `last_item_seq`, 时间戳 | `ConversationMetadata`, `ConversationSettingsSnapshot` |
| `conversation_items` | append-only canonical contextual timeline；热路径只读此表 | `id`, `conversation_id`, `seq`, 可选 run/step/tool correlation id, `kind`, `status`, 时间戳 | `ConversationItemPayload` |
| `conversation_item_fts` | 暂未实现；是否需要全文搜索索引留给后续 issue 决定 | 不在 #156 schema 内 | 无 |
| `attachments` | 文件、图片、音频、生成产物的元数据 | `id`, `conversation_id`, `kind`, `storage_kind`, path/URI/provider 字段, `sha256` | `AttachmentMetadata` |
| `agent_runs` | 一次用户触发的 agent loop summary 和恢复索引 | `id`, `conversation_id`, `status`, timing 列 | `AgentRunInput`, `AgentRunOutput`, `RunErrorPayload` |
| `provider_steps` | provider 调用 debug/replay snapshot 和 continuation state 索引 | `id`, `agent_run_id`, `seq`, `provider_id`, `model_id`, `status`, timing 列 | `ProviderStepRequestSnapshot`, `ProviderStepResponseSnapshot`, `ProviderRunStateSnapshot`, `RunSettingsSnapshot`, `RunErrorPayload` |
| `tool_invocations` | local / MCP / provider-hosted tool 执行索引 | `id`, `agent_run_id`, 可选 `provider_step_id`, `call_id`, `source`, `tool_name`, `status`, timing 列 | `ToolInvocationInput`, `ToolInvocationOutput`, `RunErrorPayload` |
| `approval_decisions` | pending approval 恢复索引 | `id`, `tool_invocation_id UNIQUE`, `status`, requested/decided/expires 时间 | `ApprovalRequestPayload`, `ApprovalDecisionPayload` |
| `usage_events` | 可派生的 provider-step 用量统计索引 | `id`, `provider_step_id`, `provider_id`, `model_id`, `date_key`, token 列 | `ProviderUsageSnapshot` |
| `prompts` | 保存的系统/开发者提示词 | `id`, `name UNIQUE`, `enabled`, `sort_order`, 时间戳 | `PromptContent` |
| `shortcuts` | 全局快捷键绑定 | `id`, `hotkey UNIQUE`, 可选 `prompt_id`, `provider_id`, `model_id`, `input_source`, `enabled` | `ShortcutAction`, `RunSettingsSnapshot` |
| `providers` | provider 配置实例 | `id`, `kind`, `display_name`, `enabled`, 时间戳 | `ProviderSettingsPayload`, `ProviderSecretRefs` |
| `provider_models` | 用户手动刷新后保存的模型列表 | `id`, `provider_id`, `model_id`, `fetched_at`, `UNIQUE(provider_id, model_id)` | `ModelCapabilitiesSnapshot`, `ProviderModelMetadata` |
| `app_settings` | app 全局设置 | `id TEXT PRIMARY KEY DEFAULT 'default'` | `AppSettingsPayload` |

## 表关系

下面是外键/索引关系，不是 UI 渲染路径。正常打开对话、渲染 timeline、导出和构建 provider input 时，repository 必须只查询 `conversation_items` 并按 `seq` 排序；不得为了展示 tool call、tool result、approval、usage 或 reasoning 再 join 执行表。

```text
projects 1 -> N conversations
conversations 1 -> N conversation_items
conversations 1 -> N attachments
conversations 1 -> N agent_runs
agent_runs 1 -> N provider_steps
agent_runs 1 -> N tool_invocations
provider_steps 1 -> N usage_events
tool_invocations 0/1 -> 1 approval_decisions
providers 1 -> N provider_models
prompts 0/1 -> N conversations
prompts 0/1 -> N shortcuts
```

历史行必须在必要时保存 prompt、provider、model、capability 和 run settings 的反规范化 snapshot。删除或编辑 prompt/provider 不能破坏旧对话。

推荐热路径查询形状：

```sql
SELECT id, seq, kind, status, agent_run_id, provider_step_id, tool_invocation_id, provider_item_id, payload_json, search_text, created_at, updated_at
FROM conversation_items
WHERE conversation_id = ?
ORDER BY seq ASC;
```

执行表只在这些场景读取：恢复未完成 run、查看 debug/replay snapshot、列出 pending approval、计算 usage rollup、诊断 provider/tool 失败。为避免多表 join 成为产品主路径，`ConversationItemPayload` 必须复制足够的展示和上下文信息；这些有意的反规范化是 schema 约束，不是临时优化。

## 运行时文件输入边界

`ai-chat-agent` 的第一版 runtime 采用 Rig + rmcp，但数据库模型不能依赖 Rig 的内部状态，也不能把 skills 或 MCP server 定义复制成数据库 source tables。

Skills 采用 Zed 风格文件模型：

- 默认扫描用户级 `~/.agents/skills/<name>/SKILL.md` 和项目级 `<project>/.agents/skills/<name>/SKILL.md`。
- Catalog 只包含 name、description 和 `SKILL.md` 绝对路径。`SKILL.md` body、`scripts/`、`references/`、`assets/` 等资源只在 skill 被实际加载时从文件系统读取。
- 如果一个 skill 被加载进某次对话上下文，写入一个 `SkillActivation` transcript item，保存当次 rendered skill content、source path、directory、hash 和 source kind。这样历史对话、导出、retry/debug 不会随着文件后续修改而漂移。
- 不建立 `skill_roots` 或 `skills` 表。可扫描的 roots、启用/禁用策略和 catalog cache 属于 app config 或 runtime cache，不属于 conversation database。

MCP 采用配置文件和运行时连接模型：

- MCP server 配置保存在 app/user/project config 中，例如 app config TOML 的 `mcpServers` 节；它支持 stdio command/args/env/cwd 和 streamable HTTP url/headers/oauth 等传输配置。
- fresh database 不建立 `mcp_servers` 或 `mcp_tools` 定义表。运行时通过 rmcp 连接 server、列出 tools，并注册到 Rig `ToolServerHandle`。
- `tool_invocations` 只保存一次实际调用的执行索引：`source = Mcp { server_id }`、原始 `tool_name`、Rig 暴露给模型的 `runtime_tool_name`、arguments、结果、错误和审批状态。
- 为避免多个 MCP server 的 tool name 冲突，`ai-chat-agent::ToolRegistry` 必须生成稳定的 model-visible runtime name，并记录到 transcript 和 invocation input 中。

Rig 持久化采用 adapter 边界：

- 从 `conversation_items` 构建 `Vec<rig::Message>`，通过 Rig prompt request 的 explicit history 传入；不要把 Rig `ConversationMemory` 作为 canonical database 写入路径。
- 一次 Rig prompt/run 对应一行 `agent_runs`；Rig loop 里的每次 completion call 对应一行 `provider_steps`；每个 local/MCP/provider-hosted tool call 对应一行 `tool_invocations`。
- 使用 `PromptHook` 观察 completion call/response、tool call/result 和终止状态；使用 `PersistingCompletionModel<M>` 包装 Rig `CompletionModel`，在调用 provider 前后写入 provider step snapshots。
- `PromptResponse.messages` 可以作为校验或补充来源，但不能成为 canonical transcript 格式。真实 transcript 必须由 `conversation_items` 的 typed payload 表达。

## 表定义细节

### `schema_migrations`

记录全新 store 已执行的 migration。

必要列：

- `name TEXT PRIMARY KEY`
- `executed_at TEXT NOT NULL`

### `schema_metadata`

存储数据库内部兼容信息。文件名可以区分 legacy 存储和全新存储，但 schema 兼容性必须从本表和 `schema_migrations` 读取。

必要列：

- `id TEXT PRIMARY KEY DEFAULT 'default'`
- `schema_version INTEGER NOT NULL`
- `created_app_version TEXT`
- `last_opened_app_version TEXT`
- `payload_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

`payload_json` 类型：`SchemaMetadataPayload`。

### `projects`

项目是 canonical real folder path。

必要列：

- `id TEXT PRIMARY KEY`
- `path TEXT NOT NULL UNIQUE`
- `display_name TEXT NOT NULL`
- `kind TEXT NOT NULL CHECK (kind IN ('normal', 'scratch'))`
- `metadata_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `last_opened_at TEXT`

`metadata_json` 类型：`ProjectMetadata`。

### `conversations`

对话属于项目，并且永远是 contextual。

必要列：

- `id TEXT PRIMARY KEY`
- `project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE`
- `title TEXT NOT NULL`
- `status TEXT NOT NULL CHECK (status IN ('active', 'archived', 'deleted'))`
- `prompt_id TEXT REFERENCES prompts(id) ON DELETE SET NULL`
- `default_provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL`
- `default_model_id TEXT`
- `last_item_seq INTEGER NOT NULL DEFAULT 0`
- `metadata_json JSON NOT NULL DEFAULT '{}'`
- `settings_snapshot_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `archived_at TEXT`
- `deleted_at TEXT`

JSON 类型：

- `metadata_json`: `ConversationMetadata`
- `settings_snapshot_json`: `ConversationSettingsSnapshot`

### `conversation_items`

这是 canonical timeline/context source。展示、导出、重发、contextual history、编辑、provider input reconstruction 都必须读这个表，不能读 provider request snapshot，也不能为了 timeline 展示去 join run/tool/provider-step 表。

它采用 Codex/pi 式 append-only log 思路，但落在 SQLite `JSON` 列里。每个 item 的 `payload_json` 必须完整描述用户可见和 provider-context 所需的信息：

- `SkillActivation` 必须包含 skill name、source kind、`SKILL.md` 路径、skill directory、body hash 和当次 rendered content。Skill source 仍从文件读取；这里保存的是对话历史快照。
- `ToolCall` 必须包含 tool source、tool name、call id、arguments 和足够展示的状态信息。
- `ToolResult` 必须包含输出内容、错误状态、structured output 和附件引用。
- `ApprovalRequest` / `ApprovalDecision` 必须包含可恢复和可展示的审批摘要。
- `Status` / `Error` 必须包含 timeline 需要显示的进度、失败、取消或重试信息。
- usage 如果要出现在 timeline 中，应写入对应 item payload；`usage_events` 只服务统计查询。

执行表中的 `agent_run_id`、`provider_step_id`、`tool_invocation_id` 是 correlation id。它们允许 debug 和恢复时定位执行记录，但不是读取 item payload 的前置条件。

必要列：

- `id TEXT PRIMARY KEY`
- `conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE`
- `seq INTEGER NOT NULL`
- `kind TEXT NOT NULL`
- `status TEXT NOT NULL`
- `agent_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL`
- `provider_step_id TEXT REFERENCES provider_steps(id) ON DELETE SET NULL`
- `tool_invocation_id TEXT REFERENCES tool_invocations(id) ON DELETE SET NULL`
- `provider_item_id TEXT`
- `payload_json JSON NOT NULL`
- `search_text TEXT NOT NULL DEFAULT ''`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `UNIQUE(conversation_id, seq)`

`payload_json` 类型：`ConversationItemPayload`。

`kind` 和 `status` 列是 payload discriminant 的查询索引副本。repository 层必须保证它们和 `payload_json` 保持一致。

### `conversation_item_fts`（暂未实现）

全文搜索需求尚未确认，#156 不创建 FTS 表，也不暴露 `MATCH` 查询 API。

当前 schema 只保留 `conversation_items.search_text` 作为普通派生文本字段，便于未来搜索、导出或 debug 使用。后续如果确认需要全文搜索，应单独设计：

- 普通 FTS5 表加 triggers；
- external-content FTS；
- 或其他不依赖 SQLite FTS 的搜索索引。

如果未来重新引入 FTS，必须同时处理用户 query escaping、空 query 行为、item update/delete、conversation/project cascade delete 后的索引同步。

### `attachments`

附件可以被输入项、assistant 输出、tool result、生成文件和 provider file reference 共享。二进制字节在 app attachment store 或外部/provider storage 中，数据库只保存引用和元数据。

必要列：

- `id TEXT PRIMARY KEY`
- `conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE`
- `kind TEXT NOT NULL CHECK (kind IN ('image', 'file', 'audio', 'attachment'))`
- `storage_kind TEXT NOT NULL CHECK (storage_kind IN ('local_file', 'external_uri', 'provider_file', 'generated_file'))`
- `mime_type TEXT`
- `name TEXT`
- `path TEXT`
- `external_uri TEXT`
- `provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL`
- `provider_file_id TEXT`
- `sha256 TEXT`
- `size_bytes INTEGER`
- `metadata_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

`metadata_json` 类型：`AttachmentMetadata`。

### `agent_runs`

agent run 是一次用户触发的 loop summary，可能包含多次 provider step、tool invocation、approval、retry 和 continuation。它用于恢复、取消、重试和诊断，不是 transcript 的 canonical source。

必要列：

- `id TEXT PRIMARY KEY`
- `conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE`
- `trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('user', 'shortcut', 'resume', 'retry'))`
- `status TEXT NOT NULL`
- `input_json JSON NOT NULL`
- `output_json JSON`
- `error_json JSON`
- `created_at TEXT NOT NULL`
- `started_at TEXT`
- `completed_at TEXT`
- `updated_at TEXT NOT NULL`

JSON 类型：

- `input_json`: `AgentRunInput`
- `output_json`: `AgentRunOutput`
- `error_json`: `RunErrorPayload`

### `provider_steps`

provider step 是对一个 provider/model 的一次调用索引。provider request/response snapshot 只应该出现在这里，用于 debug、replay 和 continuation；用户可见的 provider 输出必须已经写入 `conversation_items`。

必要列：

- `id TEXT PRIMARY KEY`
- `agent_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE`
- `seq INTEGER NOT NULL`
- `provider_id TEXT NOT NULL REFERENCES providers(id)`
- `model_id TEXT NOT NULL`
- `status TEXT NOT NULL`
- `request_snapshot_json JSON NOT NULL`
- `response_snapshot_json JSON`
- `state_snapshot_json JSON`
- `settings_snapshot_json JSON NOT NULL`
- `error_json JSON`
- `created_at TEXT NOT NULL`
- `started_at TEXT`
- `completed_at TEXT`
- `updated_at TEXT NOT NULL`
- `UNIQUE(agent_run_id, seq)`

JSON 类型：

- `request_snapshot_json`: `ProviderStepRequestSnapshot`
- `response_snapshot_json`: `ProviderStepResponseSnapshot`
- `state_snapshot_json`: `ProviderRunStateSnapshot`
- `settings_snapshot_json`: `RunSettingsSnapshot`
- `error_json`: `RunErrorPayload`

### `tool_invocations`

tool invocation 是执行索引。provider output item 可以请求 tool，但 timeline 中展示 tool 请求、审批、结果和错误时必须读取 `conversation_items.payload_json`；本表用于恢复 pending/running tool、重试和诊断。

必要列：

- `id TEXT PRIMARY KEY`
- `agent_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE`
- `provider_step_id TEXT REFERENCES provider_steps(id) ON DELETE SET NULL`
- `call_id TEXT NOT NULL`
- `source TEXT NOT NULL CHECK (source IN ('local', 'mcp', 'provider_hosted'))`
- `namespace TEXT`
- `server_id TEXT`
- `tool_name TEXT NOT NULL`
- `status TEXT NOT NULL`
- `input_json JSON NOT NULL`
- `output_json JSON`
- `error_json JSON`
- `created_at TEXT NOT NULL`
- `started_at TEXT`
- `completed_at TEXT`
- `updated_at TEXT NOT NULL`

JSON 类型：

- `input_json`: `ToolInvocationInput`
- `output_json`: `ToolInvocationOutput`
- `error_json`: `RunErrorPayload`

### `approval_decisions`

审批状态与 tool invocation 分离，方便在不解析所有 transcript item 的情况下列出和恢复 pending approval。审批请求和决策仍必须作为 `conversation_items` 写入，保证 timeline 不依赖 join。

必要列：

- `id TEXT PRIMARY KEY`
- `tool_invocation_id TEXT NOT NULL UNIQUE REFERENCES tool_invocations(id) ON DELETE CASCADE`
- `status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied', 'expired', 'canceled'))`
- `request_json JSON NOT NULL`
- `decision_json JSON`
- `requested_at TEXT NOT NULL`
- `decided_at TEXT`
- `expires_at TEXT`

JSON 类型：

- `request_json`: `ApprovalRequestPayload`
- `decision_json`: `ApprovalDecisionPayload`

### `usage_events`

用量记录采用 provider-step 粒度。conversation 和每日 rollup 从该表计算。它是统计索引；timeline 中展示的 usage snapshot 必须已经进入 `conversation_items`。

必要列：

- `id TEXT PRIMARY KEY`
- `provider_step_id TEXT NOT NULL REFERENCES provider_steps(id) ON DELETE CASCADE`
- `conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE`
- `provider_id TEXT NOT NULL REFERENCES providers(id)`
- `model_id TEXT NOT NULL`
- `date_key TEXT NOT NULL`
- `input_tokens INTEGER NOT NULL DEFAULT 0`
- `output_tokens INTEGER NOT NULL DEFAULT 0`
- `cached_input_tokens INTEGER NOT NULL DEFAULT 0`
- `cache_write_input_tokens INTEGER NOT NULL DEFAULT 0`
- `reasoning_tokens INTEGER NOT NULL DEFAULT 0`
- `total_tokens INTEGER NOT NULL DEFAULT 0`
- `usage_json JSON NOT NULL`
- `created_at TEXT NOT NULL`

`usage_json` 类型：`ProviderUsageSnapshot`。

### `prompts`

prompts 替代 templates。它们保存可复用的 system/developer 指令，不保存示例式模板。

必要列：

- `id TEXT PRIMARY KEY`
- `name TEXT NOT NULL UNIQUE`
- `content_json JSON NOT NULL`
- `enabled INTEGER NOT NULL DEFAULT 1`
- `sort_order INTEGER NOT NULL DEFAULT 0`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

`content_json` 类型：`PromptContent`。

### `shortcuts`

shortcuts 不再保存 mode 或 template id。

必要列：

- `id TEXT PRIMARY KEY`
- `hotkey TEXT NOT NULL UNIQUE`
- `enabled INTEGER NOT NULL DEFAULT 1`
- `prompt_id TEXT REFERENCES prompts(id) ON DELETE SET NULL`
- `provider_id TEXT REFERENCES providers(id) ON DELETE SET NULL`
- `model_id TEXT`
- `input_source TEXT NOT NULL CHECK (input_source IN ('selection_or_clipboard', 'screenshot'))`
- `action_json JSON NOT NULL`
- `settings_snapshot_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

JSON 类型：

- `action_json`: `ShortcutAction`
- `settings_snapshot_json`: `RunSettingsSnapshot`

### `providers`

provider 行描述配置过的 provider 实例。secret 必须存成 keychain reference 或其他 secret reference，不能复制到 snapshot。

必要列：

- `id TEXT PRIMARY KEY`
- `kind TEXT NOT NULL`
- `display_name TEXT NOT NULL`
- `enabled INTEGER NOT NULL DEFAULT 1`
- `settings_json JSON NOT NULL DEFAULT '{}'`
- `secret_refs_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

JSON 类型：

- `settings_json`: `ProviderSettingsPayload`
- `secret_refs_json`: `ProviderSecretRefs`

### `provider_models`

model 行由 provider 设置中的手动刷新写入。

必要列：

- `id TEXT PRIMARY KEY`
- `provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE`
- `model_id TEXT NOT NULL`
- `display_name TEXT`
- `capabilities_json JSON NOT NULL`
- `metadata_json JSON NOT NULL DEFAULT '{}'`
- `fetched_at TEXT NOT NULL`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `UNIQUE(provider_id, model_id)`

JSON 类型：

- `capabilities_json`: `ModelCapabilitiesSnapshot`
- `metadata_json`: `ProviderModelMetadata`

### `app_settings`

不属于项目、对话、provider 或快捷键的全局 app 设置。

必要列：

- `id TEXT PRIMARY KEY DEFAULT 'default'`
- `settings_json JSON NOT NULL DEFAULT '{}'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

`settings_json` 类型：`AppSettingsPayload`。

## 类型化 JSON 规则

每个 app 自有 JSON payload 都必须有 Rust 类型。repository API 必须接收和返回这些 Rust 类型，不能暴露裸 `serde_json::Value`。

默认 serde 形态：

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SomePayload {
    // fields
}
```

payload discriminant 使用 tagged enum：

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ConversationItemPayload {
    Message { role: TranscriptRole, content: Vec<ContentPart> },
    SkillActivation(SkillActivationItem),
    Reasoning { text: String, summary: Option<String> },
    ToolCall(ToolCallItem),
    ToolResult(ToolResultItem),
    ApprovalRequest(ApprovalRequestItem),
    ApprovalDecision(ApprovalDecisionItem),
    Status(StatusItem),
    Error(RunErrorPayload),
}
```

开放 provider/tool 数据只能藏在有名字的 wrapper 后面：

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRawPayload {
    pub(crate) provider_kind: String,
    pub(crate) value: serde_json::Value,
}
```

repository-facing 类型中不要出现匿名 `serde_json::Value` 字段。

## JSON 列和 Rust 类型映射

| 表列 | Rust 类型 |
| --- | --- |
| `schema_metadata.payload_json` | `SchemaMetadataPayload` |
| `projects.metadata_json` | `ProjectMetadata` |
| `conversations.metadata_json` | `ConversationMetadata` |
| `conversations.settings_snapshot_json` | `ConversationSettingsSnapshot` |
| `conversation_items.payload_json` | `ConversationItemPayload` |
| `attachments.metadata_json` | `AttachmentMetadata` |
| `agent_runs.input_json` | `AgentRunInput` |
| `agent_runs.output_json` | `AgentRunOutput` |
| `agent_runs.error_json` | `RunErrorPayload` |
| `provider_steps.request_snapshot_json` | `ProviderStepRequestSnapshot` |
| `provider_steps.response_snapshot_json` | `ProviderStepResponseSnapshot` |
| `provider_steps.state_snapshot_json` | `ProviderRunStateSnapshot` |
| `provider_steps.settings_snapshot_json` | `RunSettingsSnapshot` |
| `provider_steps.error_json` | `RunErrorPayload` |
| `tool_invocations.input_json` | `ToolInvocationInput` |
| `tool_invocations.output_json` | `ToolInvocationOutput` |
| `tool_invocations.error_json` | `RunErrorPayload` |
| `approval_decisions.request_json` | `ApprovalRequestPayload` |
| `approval_decisions.decision_json` | `ApprovalDecisionPayload` |
| `usage_events.usage_json` | `ProviderUsageSnapshot` |
| `prompts.content_json` | `PromptContent` |
| `shortcuts.action_json` | `ShortcutAction` |
| `shortcuts.settings_snapshot_json` | `RunSettingsSnapshot` |
| `providers.settings_json` | `ProviderSettingsPayload` |
| `providers.secret_refs_json` | `ProviderSecretRefs` |
| `provider_models.capabilities_json` | `ModelCapabilitiesSnapshot` |
| `provider_models.metadata_json` | `ProviderModelMetadata` |
| `app_settings.settings_json` | `AppSettingsPayload` |

## 规范 Rust 类型草图

以下类型定义的是数据契约。具体模块位置由 #156 和 #157 决定，但数据契约变化必须同步更新本文档。

```rust
pub(crate) type ProjectId = String;
pub(crate) type ConversationId = String;
pub(crate) type ConversationItemId = String;
pub(crate) type AttachmentId = String;
pub(crate) type AgentRunId = String;
pub(crate) type ProviderStepId = String;
pub(crate) type ToolInvocationId = String;
pub(crate) type ApprovalDecisionId = String;
pub(crate) type ProviderId = String;
pub(crate) type PromptId = String;
pub(crate) type ProviderModelId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TranscriptRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ContentPart {
    Text { text: String },
    Image { attachment_id: AttachmentId },
    File { attachment_id: AttachmentId },
    Audio { attachment_id: AttachmentId },
    Attachment { attachment_id: AttachmentId },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum AttachmentSource {
    LocalFile { path: String },
    ExternalUri { uri: String },
    ProviderFile { provider_id: ProviderId, file_id: String },
    GeneratedFile { path: String },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AttachmentMetadata {
    pub(crate) source: AttachmentSource,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) preview_attachment_id: Option<AttachmentId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ConversationItemPayload {
    Message { role: TranscriptRole, content: Vec<ContentPart> },
    Reasoning { text: String, summary: Option<String> },
    ToolCall(ToolCallItem),
    ToolResult(ToolResultItem),
    ApprovalRequest(ApprovalRequestItem),
    ApprovalDecision(ApprovalDecisionItem),
    Status(StatusItem),
    Error(RunErrorPayload),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SkillActivationItem {
    pub(crate) name: String,
    pub(crate) source_kind: SkillSourceKind,
    pub(crate) skill_file_path: String,
    pub(crate) directory_path: String,
    pub(crate) content_sha256: String,
    pub(crate) content: Vec<ContentPart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SkillSourceKind {
    BuiltIn,
    User,
    Project,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolCallItem {
    pub(crate) tool_invocation_id: Option<ToolInvocationId>,
    pub(crate) call_id: String,
    pub(crate) source: ToolSource,
    pub(crate) name: String,
    pub(crate) runtime_tool_name: String,
    pub(crate) arguments: ToolArguments,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolResultItem {
    pub(crate) tool_invocation_id: Option<ToolInvocationId>,
    pub(crate) call_id: String,
    pub(crate) content: Vec<ContentPart>,
    pub(crate) is_error: bool,
    pub(crate) structured_output: Option<StructuredOutput>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ToolSource {
    Local,
    Mcp { server_id: String },
    ProviderHosted { provider_id: ProviderId },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolArguments {
    pub(crate) value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredOutput {
    pub(crate) value: serde_json::Value,
}
```

### 状态类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentRunStatus {
    Queued,
    Running,
    WaitingForApproval,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderStepStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolInvocationStatus {
    Requested,
    AwaitingApproval,
    Running,
    Succeeded,
    Failed,
    Denied,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Canceled,
}
```

### 运行时类型

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRunInput {
    pub(crate) user_item_id: ConversationItemId,
    pub(crate) prompt_snapshot: Option<PromptContent>,
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
    pub(crate) settings_snapshot: RunSettingsSnapshot,
    pub(crate) runtime_snapshot: AgentRuntimeSnapshot,
    pub(crate) max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRuntimeSnapshot {
    pub(crate) engine: AgentEngineKind,
    pub(crate) engine_version: String,
    pub(crate) skill_catalog_hash: Option<String>,
    pub(crate) mcp_config_hash: Option<String>,
    pub(crate) tool_name_strategy: ToolNameStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentEngineKind {
    Rig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolNameStrategy {
    Direct,
    Namespaced,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRunOutput {
    pub(crate) final_item_id: Option<ConversationItemId>,
    pub(crate) stopped_reason: AgentStoppedReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentStoppedReason {
    Completed,
    MaxSteps,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum AgentRunEvent {
    Started { agent_run_id: AgentRunId },
    ProviderStepStarted { provider_step_id: ProviderStepId },
    ProviderStepEvent { provider_step_id: ProviderStepId, event: ProviderStepEvent },
    ToolInvocationRequested { tool_invocation_id: ToolInvocationId },
    ApprovalRequested { approval_decision_id: ApprovalDecisionId },
    ToolInvocationFinished { tool_invocation_id: ToolInvocationId },
    Completed { output: AgentRunOutput },
    Failed { error: RunErrorPayload },
    Canceled,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRunState {
    pub(crate) agent_run_id: AgentRunId,
    pub(crate) status: AgentRunStatus,
    pub(crate) current_step_id: Option<ProviderStepId>,
    pub(crate) pending_tool_ids: Vec<ToolInvocationId>,
}
```

### Provider 调用步骤类型

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderStepRequestSnapshot {
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
    pub(crate) input_item_ids: Vec<ConversationItemId>,
    pub(crate) snapshot_kind: ProviderStepSnapshotKind,
    pub(crate) request_body: ProviderRawPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderStepSnapshotKind {
    ProviderWire,
    RigCompletionRequest,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderStepResponseSnapshot {
    pub(crate) provider_run_id: Option<String>,
    pub(crate) output_item_ids: Vec<String>,
    pub(crate) response_body: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderRunStateSnapshot {
    pub(crate) provider_id: ProviderId,
    pub(crate) provider_run_id: Option<String>,
    pub(crate) output_item_ids: Vec<String>,
    pub(crate) continuation: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ProviderStepEvent {
    OutputItemStarted { provider_item_id: Option<String>, item: ConversationItemPayload },
    TextDelta { provider_item_id: Option<String>, text: String },
    ReasoningDelta { provider_item_id: Option<String>, text: String },
    OutputItemCompleted { provider_item_id: Option<String>, item: ConversationItemPayload },
    ToolCallRequested { call: ToolCallItem },
    UsageUpdated { usage: ProviderUsageSnapshot },
    Completed { state: Option<ProviderRunStateSnapshot> },
    Failed { error: RunErrorPayload },
}
```

### 设置、Provider 和模型类型

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunSettingsSnapshot {
    pub(crate) prompt: Option<PromptContent>,
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
    pub(crate) model_capabilities: ModelCapabilitiesSnapshot,
    pub(crate) provider_settings: ProviderSettingsPayload,
    pub(crate) tool_policy: ToolPolicySnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PromptContent {
    pub(crate) messages: Vec<PromptMessage>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PromptMessage {
    pub(crate) role: TranscriptRole,
    pub(crate) content: Vec<ContentPart>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderSettingsPayload {
    pub(crate) provider_kind: String,
    pub(crate) fields: Vec<ProviderSettingFieldValue>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderSecretRefs {
    pub(crate) refs: Vec<ProviderSecretRef>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModelCapabilitiesSnapshot {
    pub(crate) text_input: bool,
    pub(crate) text_output: bool,
    pub(crate) streaming: bool,
    pub(crate) image_input: Option<ImageInputCapabilitySnapshot>,
    pub(crate) file_input: Option<FileInputCapabilitySnapshot>,
    pub(crate) audio_input: bool,
    pub(crate) image_generation: bool,
    pub(crate) tool_calling: Option<ToolCallingCapabilitySnapshot>,
    pub(crate) hosted_web_search: bool,
    pub(crate) remote_mcp: bool,
    pub(crate) reasoning: Option<ReasoningCapabilitySnapshot>,
    pub(crate) structured_output: bool,
    pub(crate) stateful_response_continuation: bool,
    pub(crate) extension: ProviderCapabilityExtensionSnapshot,
}
```

### 元数据和错误 payload 类型

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SchemaMetadataPayload {
    pub(crate) store_kind: String,
    pub(crate) legacy_policy: LegacyStorePolicy,
    pub(crate) feature_flags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyStorePolicy {
    Ignore,
    BackupOnly,
    ReadOnly,
    ManualImport,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProjectMetadata {
    pub(crate) scratch_reason: Option<String>,
    pub(crate) git_root: Option<String>,
    pub(crate) last_active_conversation_id: Option<ConversationId>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ConversationMetadata {
    pub(crate) summary: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) pinned: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ConversationSettingsSnapshot {
    pub(crate) prompt: Option<PromptContent>,
    pub(crate) provider_id: Option<ProviderId>,
    pub(crate) model_id: Option<ProviderModelId>,
    pub(crate) model_capabilities: Option<ModelCapabilitiesSnapshot>,
    pub(crate) tool_policy: ToolPolicySnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunErrorPayload {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) retryable: bool,
    pub(crate) provider: Option<String>,
    pub(crate) raw: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StatusItem {
    pub(crate) label: String,
    pub(crate) message: Option<String>,
}
```

### Tool、审批、用量和快捷键 payload 类型

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolInvocationInput {
    pub(crate) source: ToolSource,
    pub(crate) namespace: Option<String>,
    pub(crate) tool_name: String,
    pub(crate) runtime_tool_name: String,
    pub(crate) call_id: String,
    pub(crate) arguments: ToolArguments,
    pub(crate) approval_policy: ToolApprovalPolicy,
    pub(crate) execution_policy: ToolExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolInvocationOutput {
    pub(crate) content: Vec<ContentPart>,
    pub(crate) structured_output: Option<StructuredOutput>,
    pub(crate) raw_output: Option<ProviderRawPayload>,
    pub(crate) is_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolApprovalPolicy {
    Never,
    OnRequest,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolExecutionPolicy {
    Foreground,
    Background,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApprovalRequestItem {
    pub(crate) approval_decision_id: ApprovalDecisionId,
    pub(crate) tool_invocation_id: ToolInvocationId,
    pub(crate) request: ApprovalRequestPayload,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApprovalDecisionItem {
    pub(crate) approval_decision_id: ApprovalDecisionId,
    pub(crate) decision: ApprovalDecisionPayload,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApprovalRequestPayload {
    pub(crate) reason: String,
    pub(crate) tool_source: ToolSource,
    pub(crate) tool_name: String,
    pub(crate) arguments_preview: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApprovalDecisionPayload {
    pub(crate) approved: bool,
    pub(crate) decided_by: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderUsageSnapshot {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) reasoning_tokens: u64,
    pub(crate) total_tokens: u64,
    pub(crate) metadata: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ShortcutAction {
    OpenTemporaryConversation,
    SendToConversation { conversation_id: Option<ConversationId> },
}
```

### Provider 设置和能力 snapshot 类型

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolPolicySnapshot {
    pub(crate) approval_policy: ToolApprovalPolicy,
    pub(crate) enabled_sources: Vec<ToolSource>,
    pub(crate) max_steps: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderSettingFieldValue {
    pub(crate) key: String,
    pub(crate) value: ProviderSettingValue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ProviderSettingValue {
    String { value: String },
    Bool { value: bool },
    Number { value: f64 },
    Object { value: ProviderRawPayload },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderSecretRef {
    pub(crate) key: String,
    pub(crate) storage: String,
    pub(crate) ref_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageInputCapabilitySnapshot {
    pub(crate) max_images: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileInputCapabilitySnapshot {
    pub(crate) max_files: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolCallingCapabilitySnapshot {
    pub(crate) parallel_tool_calls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReasoningCapabilitySnapshot {
    pub(crate) default_effort: String,
    pub(crate) efforts: Vec<String>,
    pub(crate) summaries: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "provider", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ProviderCapabilityExtensionSnapshot {
    None,
    OpenAi { responses_api: bool, raw: Option<ProviderRawPayload> },
    Ollama { raw_capabilities: Vec<String>, family: String, raw: Option<ProviderRawPayload> },
    Other { raw: ProviderRawPayload },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderModelMetadata {
    pub(crate) display_name: Option<String>,
    pub(crate) family: Option<String>,
    pub(crate) raw: Option<ProviderRawPayload>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AppSettingsPayload {
    pub(crate) language: Option<String>,
    pub(crate) theme: Option<String>,
    pub(crate) default_project_id: Option<ProjectId>,
}
```

`ModelCapabilitiesSnapshot` 对应现有 `ModelCapabilities` 概念。provider-specific capability 字段继续放在 typed provider extension 内。

## Provider 和 Agent 边界

provider adapter 边界是一轮 provider 调用：

- 从 typed context 构建 provider-specific request。
- 流式返回 provider-step events。
- 返回 provider continuation state 和 usage。
- 保存 provider request/response snapshots 供 debug 和 replay。

agent runtime 边界是多步 loop：

- 选择 context 和 prompt snapshots。
- 启动 provider steps。
- 执行 local、MCP 或 provider-hosted tools。
- 请求并应用审批。
- 处理 retry、cancel、continuation 和 max-step guard。
- 写入 canonical conversation items，并按需更新执行/恢复/统计索引表。

provider adapter 不应长期拥有 local tool execution。如果 provider 暴露 hosted tool，adapter 可以把它报告为 `ToolSource::ProviderHosted`；local 和 MCP execution 属于 `AgentRuntime`。

## 迁移和验证预期

#155 只做文档和设计。

#156 实现必须验证：

- 创建 fresh database 并读取内部 schema version。
- migration bootstrap 可重复执行。
- foreign keys 和 cascade 行为。
- conversation item 排序和 `last_item_seq` 更新。
- 每个 JSON 列的 typed JSON roundtrip。
- 全文搜索/FTS 暂未实现；#156 只验证没有创建 FTS source/index table，后续确认搜索需求后再单独设计。
- provider model 手动刷新后的持久化。
- legacy v1-v6 存储与全新存储共存且不会被覆盖。

## #156 实现状态

2026-05-24，`codex/issue-156-fresh-db-bootstrap` 已按本文档实现 core/db-only 范围：

- 新增 `crates/ai-chat-core`，定义 canonical typed payload、ID/status enums、Rig runtime snapshot、skill activation snapshot、tool runtime/source name 和 provider-step snapshot kind。
- 新增 `crates/ai-chat-db`，实现 `ai_chat_fresh.sqlite3`、内部 `schema_metadata` / `schema_migrations`、fresh schema、Diesel-backed bootstrap、typed repositories 和 provider model cache upsert。
- 全文搜索/FTS 暂未实现；当前没有 `conversation_item_fts` 表，也没有 FTS `MATCH` 查询 API。
- 未新增 reusable `skills`、`skill_roots`、`mcp_servers`、`mcp_tools` source tables；Skill/MCP source 继续保持 file/config-backed。
- 未新增 `app/ai-chat2` shell，UI 迁移仍留给 #159。

验证已通过：

- `cargo test -p ai-chat-core`
- `cargo test -p ai-chat-db`
- `cargo clippy -p ai-chat-core --all-targets --all-features -- -D warnings`
- `cargo clippy -p ai-chat-db --all-targets --all-features -- -D warnings`
- `cargo fmt`
- `git diff --check`
