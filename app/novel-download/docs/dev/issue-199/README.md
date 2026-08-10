# Issue #199：Novel Download 子任务跟踪

- 状态：`NOVEL-199-01 Done`（当前工作树实现已完成；未提交）
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- 决策归档：已由专题计划完整吸收；总草稿不再保留完成态副本
- Novel Download 开发文档索引：[开发文档](../README.md)
- 负责范围：`app/novel-download`
- 本地 ID 范围：`E/D/F/L/ST/ERR/R/T-1400..1499`、`WP-1400..1409`

本页只跟踪 Issue #199 在 Novel Download 内的多轮子任务、状态和专题文档入口。架构契约、文件动作、
工作包、测试与实施证据只写在命名专题文档中，不把执行计划正文写进本 README。

## 子任务

| ID | 子任务 | 状态 | 专题文档 | 工作包 | 前置与交接 |
| --- | --- | --- | --- | --- | --- |
| `NOVEL-199-01` | 最小 Form、私有下载 Transition、唯一 Task、取消与 `.part` 文件事务迁移 | `Done`；当前工作树已实施，39 tests 与定向门禁通过，未提交；UI 未执行 | [form-operation-download-migration-plan.md](form-operation-download-migration-plan.md) | `WP-1400`–`WP-1407` | 消费 Form `C-900`–`C-904`；不引入 `gpui-store` |

## 跟踪规则

- `Ready` 只表示专题计划可直接实施，不表示代码、自动化或实际 UI 操作已经完成。
- 后续轮次只有在范围确认后才分配新的子任务 ID 和专题文档；不在本页预写尚未确认的能力。
- 实际自动化、UI 未执行和未提交等边界均在专题文档记录。
