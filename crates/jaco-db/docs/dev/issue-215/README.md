# jaco-db：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/jaco-db`；工作包：WP-05。
- 本地编号使用 `jaco-db/F-*`、`jaco-db/L-*`、`jaco-db/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；src/。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

Diesel 与 libsqlite3-sys 原子升级，保留 bundled-windows 和 cargo-shear 对直接 SQLite 依赖的说明。验证单一 links owner；不删除数据库，不自动重建 schema，不新增兼容迁移。

## 验证与完成条件

- 候选命令：`cargo test -p jaco-db --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：既有事务、约束、映射测试使用临时库；平台链接单独确认。
