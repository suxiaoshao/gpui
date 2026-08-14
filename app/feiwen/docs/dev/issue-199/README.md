# Issue #199：Feiwen 子任务跟踪

- 状态：`FEI-199-02 Done`；`FEI-199-01` 保持 `Done`
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- Feiwen 开发文档索引：[开发文档](../README.md)
- 负责范围：`app/feiwen`
- 本地 ID 范围：历史 `E/D/F/L/ST/DB/ERR/R/T-700..899`、`WP-700..799`；本轮
  `E/D/F/L/ST/ERR/R/T-1300..1399`、`WP-1300..1309`

本页只跟踪 Issue #199 在 Feiwen 内的多轮子任务、状态和专题文档入口。详细架构、文件动作、工作包、
测试与实施证据只写在命名专题文档中。

## 子任务

| ID | 子任务 | 状态 | 专题文档 | 工作包 | 前置与交接 |
| --- | --- | --- | --- | --- | --- |
| `FEI-199-01` | Form、Query/Fetch Transition、QueryCatalog Store/Operation、DuckDB resource与UI完整迁移 | `Done`；89 tests、Clippy与workspace gate通过；实际UI操作测试未执行 | [form-operation-store-migration.md](form-operation-store-migration.md) | `WP-700`–`WP-708` | Form `C-500`–`C-502`、Feiwen `C-703`–`C-705`已交付 |
| `FEI-199-02` | Query/Fetch 迁移到最终 Form typed path、resolver、snapshot validation 与 impact 契约 | `Done`；93 tests、Clippy 与 workspace gate 通过；实际 UI 操作测试未执行 | [form-breaking-api-remigration-plan.md](form-breaking-api-remigration-plan.md) | `WP-1300`–`WP-1304` | Form `C-900`–`C-904` 已达 `consumer-complete`；保留现有 Operation/Store/DB/Catalog |

## 跟踪规则

- `FEI-199-01` 是完整迁移历史；`FEI-199-02` 只迁移 Query/Fetch 的 Form consumer，不重新实施
  Catalog、DB resource、Store 或现有业务 Operation。
- HTTP Client、Novel Download不属于本轮Feiwen范围。
- `Ready`只表示专题计划闭环；代码、自动化与明确排除的 UI 边界必须分别登记后才能标记 `Done`。
