# Issue #199：Feiwen 子任务跟踪

- 状态：`FEI-199-01 Done`；实际 UI 操作测试未执行
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- Feiwen 开发文档索引：[开发文档](../README.md)
- 负责范围：`app/feiwen`
- 本地 ID 范围：`E/D/F/L/ST/DB/ERR/R/T-700..899`、`WP-700..799`

本页只跟踪 Issue #199 在 Feiwen 内的多轮子任务、状态和专题文档入口。详细架构、文件动作、工作包、
测试与实施证据只写在命名专题文档中。

## 子任务

| ID | 子任务 | 状态 | 专题文档 | 工作包 | 前置与交接 |
| --- | --- | --- | --- | --- | --- |
| `FEI-199-01` | Form、Query/Fetch Transition、QueryCatalog Store/Operation、DuckDB resource与UI完整迁移 | `Done`；89 tests、Clippy与workspace gate通过；实际UI操作测试未执行 | [form-operation-store-migration.md](form-operation-store-migration.md) | `WP-700`–`WP-708` | Form `C-500`–`C-502`、Feiwen `C-703`–`C-705`已交付 |

## 跟踪规则

- 本任务不是单独的Form迁移；更新状态时必须同时核对Query、Fetch、Catalog、DB resource、UI与验证。
- HTTP Client、Novel Download不属于本轮Feiwen范围。
- `Ready`只表示专题计划闭环；代码、自动化与明确排除的 UI 边界必须分别登记后才能标记 `Done`。
