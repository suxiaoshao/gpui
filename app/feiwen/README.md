# Feiwen

Feiwen 是一个基于 GPUI 的本地小说元数据工具。它可以按页抓取小说数据并写入 DuckDB，再通过结构化条件、排序和结果表格完成高级检索。

## 当前功能

- 配置抓取入口、页码范围与 Cookie，查看逐页日志和任务进度。
- 支持 Fresh、Resume 与 Retry 工作流，以及运行中的取消操作。
- 通过嵌套条件、包含或排除标签、字段排序和结果表格查询小说数据。
- 在本地 DuckDB 中保存小说、作者、标签和最新章节等元数据。
- 数据库不可用时提供重新打开，以及备份后重建的恢复入口。
- 跟随系统强调色，提供英语和简体中文界面。

## 数据与安全

- 数据库文件为操作系统配置目录下的 `top.sushao.feiwen/data.duckdb`，首次运行时自动创建表和索引。
- 数据库恢复可能同时处理 `data.duckdb` 与其 WAL；执行重建前应选择安全的备份目录，并保留完整备份。
- Cookie 属于敏感信息。开发和测试应使用占位 Cookie、本地 mock HTTP 服务和隔离数据，避免记录或提交真实凭据。
- 当前应用没有公开的数据库路径覆盖选项；无法确认隔离环境时，不要直接修改默认用户数据库。

## 目录结构

```text
app/feiwen/
├─ src/main.rs       # 日志、GPUI 初始化和主窗口
├─ src/app.rs        # Workspace、标题栏与数据库资源页
├─ src/features.rs   # Fetch 与 Query 功能入口
├─ src/fetch.rs      # 抓取领域模型与解析
├─ src/store.rs      # DuckDB 路径、连接池与 schema
├─ src/store/        # 数据库生命周期、catalog、query 与 service
├─ src/foundation.rs # 资源与 i18n
├─ locales/          # en-US、zh-CN 与 macOS bundle 文案
├─ build-assets/     # 打包图标
└─ docs/             # 产品、测试和开发文档
```

## 开发

环境基线与系统依赖见[工作区 README](../../README.md)。以下命令均从 workspace 根目录执行：

```bash
cargo run -p feiwen
cargo build -p feiwen --locked
cargo test -p feiwen --locked
cargo fmt --all -- --check
cargo clippy -p feiwen --all-targets --all-features --locked -- -D warnings
cargo run -p xtask -- bundle feiwen
```

## 文档

- [功能文档](docs/features/README.md)
- [测试场景](docs/tests/README.md)
- [开发计划与迁移记录](docs/dev/README.md)
