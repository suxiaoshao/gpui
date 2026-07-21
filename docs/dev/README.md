# Workspace development plans

这里仅登记跨 workspace 的迁移批次。每次框架迁移使用独立的目标版本或 Git hash 标识，
不会用一个无版本文件覆盖历史计划；具体 app/crate 的实现内容放在各自的 `docs/dev`。

| 日期 | 迁移批次 | Source | Target | 状态 | 总计划 |
| --- | --- | --- | --- | --- | --- |
| 2026-07-21 | `gpui-1a246efd-component-5b45bcb` | GPUI `0.2.2@1d217ee`；gpui-component `0.5.2@c36b0c6` | GPUI `0.2.2@1a246efd`；gpui-component `0.5.2@5b45bcb` | **当前迁移**；计划待审阅；TextView 主题生命周期存在上游阻断，修复后必须新建后继 target 文档 | [README.md](migrations/gpui-1a246efd-component-5b45bcb/README.md) |

## 目录约定

- `docs/dev/migrations/<target-id>/README.md`：只保存跨 workspace 的顺序、发布门和子计划索引。
- `docs/dev/migrations/<target-id>/workspace.md`：root manifest/toolchain、dependency graph 与最终 CI 门。
- `docs/dev/migrations/<target-id>/dependency-evidence.md`：共享依赖与上游证据。
- `docs/dev/migrations/<target-id>/skill-sync.md`：不属于任何 Cargo package 的 repo-local skill 同步。
- `app/<name>/docs/dev/migrations/<target-id>.md`：应用自己的迁移计划。
- `crates/<name>/docs/dev/migrations/<target-id>.md`：crate 自己的迁移计划。

Git dependency 的 `<target-id>` 固定为
`gpui-<gpui-target-sha前8位>-component-<gpui-component-target-sha前8位>`；完整 crate version 与
source/target SHA 写入文档状态区。若未来改用正式 release，则 ID 使用明确的 `v<version>`，不能只写
`latest`、`upgrade` 或其他会被复用的名字。

表格按创建日期倒序排列，并且只能有一项标记为“当前迁移”。新迁移必须新增 `<target-id>`，
不能修改旧批次来表示新的目标版本。
