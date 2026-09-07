# mcp-auth-test-server：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`tools/mcp-auth-test-server`；工作包：WP-05。
- 本地编号使用 `mcp-auth-test-server/F-*`、`mcp-auth-test-server/L-*`、`mcp-auth-test-server/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；Cargo.lock；README.md；src/。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

独立 workspace，不受 root lock 或 cargo test --workspace 覆盖。Q-03 已确认不升级：保留 RMCP 2.2.0 和现有认证行为；本轮不修改此工具的 manifest、lock 或实现。

## 验证与完成条件

- 候选命令：`cargo test --manifest-path tools/mcp-auth-test-server/Cargo.toml --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：不安排 RMCP 2/3 跨版本验证；工具未改动时不单独重跑测试。本 owner 计划保留为明确排除升级的记录。
