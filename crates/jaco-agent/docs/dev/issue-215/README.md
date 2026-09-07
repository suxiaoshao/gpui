# jaco-agent：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/jaco-agent`；工作包：WP-05。
- 本地编号使用 `jaco-agent/F-*`、`jaco-agent/L-*`、`jaco-agent/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；src/tools/builtin/filesystem.rs；src/mcp/connector.rs；src/runtime/tests.rs。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

保持 Rig 0.42.0；RMCP 受 Q-03。用户已确认（2026-09-06）：similar 升级后采用目标版本的默认算法，保留 `TextDiff::from_lines` 调用，不显式指定 `RawMyers`；接受新版默认策略带来的差异块划分与对齐变化，不要求复现旧版输出。审批、provider 流式生命周期和工具权限不改。

## 验证与完成条件

- 候选命令：`cargo test -p jaco-agent --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：现有 diff/工具/取消回归；diff 按新版默认策略核对差异内容与 unified diff 格式，不以旧版差异块划分或对齐方式作为兼容要求。MCP 的类型编译和工具服务 E2E 分别记录。
