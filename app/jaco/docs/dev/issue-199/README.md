# Issue #199：Jaco 子任务跟踪

- 状态：`JACO-199-03 Done`；`JACO-199-01`、`JACO-199-02` 保持已完成
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 根计划：[工作区 Issue #199 计划](../../../../../docs/dev/issue-199/README.md)
- Jaco 开发文档索引：[开发计划](../README.md)
- 负责范围：`app/jaco`
- 本地 ID 范围：历史 `E/D/F/L/ST/R/T-400..499`、`WP-400..499` 与
  `E/D/F/L/ST/R/T-600..699`、`WP-600..699`；本轮
  `E/D/F/L/ST/ERR/R/T-1200..1299`、`WP-1200..1209`

本页只跟踪 Issue #199 在 Jaco 内的多轮子任务、状态、依赖、实施引用与专题开发文档。
架构决定、调用面、工作包、测试契约和验收证据写在各自的专题文档中，不继续堆入本页。

## 子任务

| ID | 子任务 | 状态 | 专题文档 | 工作包 | 前置与交接 | 实施引用 |
| --- | --- | --- | --- | --- | --- | --- |
| `JACO-199-01` | 迁移到显式 form owner API | 已实施；自动化通过；UI smoke 部分完成 | [form-migration.md](form-migration.md) | `WP-400`–`WP-406` | `C-03`/`C-04` 已达到 `consumer-complete` | `24d4249` |
| `JACO-199-02` | 迁移到 greenfield Form vNext | `Done`；361 tests、Clippy与workspace gate通过；实际UI操作测试未执行 | [form-vnext-migration.md](form-vnext-migration.md) | `WP-600`–`WP-604` | Form `C-500`–`C-502`与Jaco `C-603`已交付 | 本次 Issue #199 实施提交 |
| `JACO-199-03` | 迁移到最终 Form path、event、binding 与 FormVersion 契约 | `Done`；362 tests、Clippy 与 workspace gate 通过；实际 UI 操作测试未执行 | [form-breaking-api-remigration-plan.md](form-breaking-api-remigration-plan.md) | `WP-1200`–`WP-1204` | Form `C-900`–`C-904` 已达 `consumer-complete` | 当前工作区，尚未提交 |

## 跟踪规则

- 后续轮次只有在范围确认后才分配下一个子任务 ID、新建专题文档并增加一行；不在本页预写
  尚未确认的设计或实施计划。
- 子任务状态只反映实际阶段；开发文档完成不等于代码已经实施、验证或可标记 `Done`。
- 跨 crate 顺序、全局完成门和共享契约以根计划为准；本页不复制它们。
