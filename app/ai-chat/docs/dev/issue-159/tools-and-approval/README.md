# Issue #159 内置工具与审批

本目录保存 `ai-chat2` 本地内置工具、工具审批、tool timeline 和 approved resume 计划。

## 文档

| 文档 | 用途 |
| --- | --- |
| `plan.md` | 完整实现记录：文件工具、grep、审批模式、数据模型、UI、i18n、icons、验证和后续问题。 |
| `approval-in-run-plan.md` | 已确认计划：把工具审批改为同一 agent run 内的异步 gate，停止 approved resume 拆 run。 |

## 当前结论

- V1.0 已围绕本地文件工具、path approval、ChatForm 审批模式、tool/approval timeline row、approve/deny action、approved resume、streaming delta 和可恢复工具错误反馈落地。
- `run_command`、MCP/provider-hosted source-specific policy、structured output 深度 preview、tool progress/duration、跨重启审批 sticky preference 和完整 rich multimodal timeline 仍是后续项。
