# feiwen

feiwen 是基于 GPUI 的小说数据管理工具，支持分页抓取小说数据，并通过高级检索查询本地数据库。

## 数据库

- 使用 Diesel + SQLite。
- 迁移文件已内置，首次运行会自动初始化数据库。

## 产品与测试文档

- 功能文档入口：[docs/features/README.md](docs/features/README.md)
- 测试步骤入口：[docs/tests/README.md](docs/tests/README.md)

测试文档要求使用隔离测试数据库、本地 mock HTTP 服务和测试 Cookie，不使用用户真实数据库、真实 Cookie 或真实抓取入口。
