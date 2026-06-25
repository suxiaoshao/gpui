# Issue #159 MCP Settings/runtime

本目录保存 `ai-chat2` MCP 设置、运行时、OAuth 和工具注册计划。

## 文档

| 文档 | 用途 |
| --- | --- |
| `settings-and-runtime.md` | 完整 MCP 配置模型、rmcp/Rig 接线、Settings UI、OAuth 后续项、数据流和验证计划。 |

## 当前结论

- MCP server definitions 的 source of truth 是 `config.toml`，不新增 SQLite source table。
- Settings 默认 Add/Edit UI 按 Codex 自定义 MCP 表单收敛，只暴露 stdio/http 常用字段。
- 参数、环境变量、环境变量传递、HTTP headers 和 env-backed headers 使用结构化 rows，不再让用户输入字符串再解析。
- OAuth browser flow、token storage、refresh、scope upgrade、logout、ClientCredentials UI 和 prewarm 都是后续项。
- MCP tool 的默认审批继承 ChatForm 当前 run 的审批模式；per-server/per-tool approval override 暂时只作为 TOML advanced path 保留。
