# ai-chat

ai-chat 是基于 GPUI 的 AI 聊天桌面应用，支持会话管理、流式响应、模板、快捷键和消息预览等能力。

## 数据库

- 使用 Diesel + SQLite。
- 迁移文件已内置，首次运行会自动初始化数据库。

```bash
# 在 app/ai-chat 目录下生成迁移（需要 diesel_cli）
diesel migration generate migration_name
```

## 产品与测试文档

- 功能文档入口：[docs/features/README.md](docs/features/README.md)
- 测试步骤入口：[docs/tests/README.md](docs/tests/README.md)

测试文档要求使用隔离测试数据目录或测试数据库，不使用用户真实配置、真实会话、真实 API Key 或真实导出目录。
