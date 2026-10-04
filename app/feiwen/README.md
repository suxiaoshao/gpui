# feiwen

feiwen 是基于 GPUI 的小说数据管理工具，支持分页抓取小说数据，并通过高级检索查询本地数据库。

## 数据库

- 使用 DuckDB，首次启动自动创建 schema；保留原有配置目录中的 `data.duckdb`。
- 数据库准备、重开和备份重建由 feiwen-data 持有，窗口组合资源状态页。

## 产品与测试文档

- 功能文档入口：[docs/features/README.md](docs/features/README.md)
- 测试步骤入口：[docs/tests/README.md](docs/tests/README.md)

测试文档要求使用隔离测试数据库、本地 mock HTTP 服务和测试 Cookie，不使用用户真实数据库、真实 Cookie 或真实抓取入口。

隔离运行可设置 `FEIWEN_DATA_DIR=/tmp/feiwen-qa-data`；未设置时继续使用系统配置目录。

## 功能结构

`src/` 负责启动、窗口、导航和菜单；[feiwen-data](crates/feiwen-data/README.md) 管理数据库与数据服务，[feiwen-fetch](crates/feiwen-fetch/README.md) 管理抓取，[feiwen-query](crates/feiwen-query/README.md) 管理检索。查询不依赖抓取内部状态，由 shell 转交摘要与导航意图。结果只保留一份数据，排序使用索引并保留小说 ID。

```sh
cargo test -p feiwen-data -p feiwen-fetch -p feiwen-query -p feiwen
```
