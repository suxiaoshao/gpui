# Issue #199：多轮任务总指导、进度与文档索引

## 文档职责

- 状态：`进行中`。本轮 Form vNext、Feiwen 完整迁移与 Jaco Form consumer 迁移均为 `Done` 并通过自动化；
  实际 UI 操作测试按要求未执行，HTTP Client、Novel Download 与 Jaco MCP runtime 继续暂缓。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 分支：`codex/199-adopt-gpui-store-form-operation`
- 最近更新：2026-08-05

本文只维护 Issue #199 的总指导、轮次、子任务状态、依赖顺序和专题文档入口。架构契约、文件清单、
工作包、测试与完成证据写在独立专题文档中；后续任务不得继续把执行计划堆入本 README。

## 总指导

1. 开发文档只写中文；README/guide 等 crate 对外文档继续维护中英文版本。
2. 一个可独立实施或多轮迭代的任务使用独立命名文档；各 owner 的 `README.md` 只做状态/索引。
3. 已完成历史文档保留原样并继续可发现，不改写成后续 breaking 计划。
4. Form core 可以内部使用 `gpui_operation::Transition`，公开/generated/adapter API 仍为领域方法。
5. Form vNext producer contract 固定并验证后，Jaco/Feiwen 才消费最终签名；应用不得各自创建兼容 shim。
6. HTTP Client、Novel Download 和 Jaco MCP runtime operation 本轮不做；未回答问题继续保留在总草稿。
7. `gpui-store` 本轮不重构：复杂状态机由业务类型自己建模，最终使用 Store `set/update` 发布即可。

## 当前轮子任务

| ID | 范围 | 状态 | 专题文档 | 前置关系 |
| --- | --- | --- | --- | --- |
| `FORM-199-02` | 三个 gpui-form crate 的 greenfield vNext 重构 | `Done`；producer/consumer与aggregate gate通过，实际UI操作测试未执行 | [Form vNext 重构计划](../../../crates/gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md) | producer已交付 |
| `FEI-199-01` | Feiwen Form、Query/Fetch Transition、Catalog Store/Operation、DB resource与UI完整迁移 | `Done`；89 tests与aggregate gate通过，实际UI操作测试未执行 | [Feiwen完整迁移计划](../../../app/feiwen/docs/dev/issue-199/form-operation-store-migration.md) | 已消费Form并交付Feiwen内部DB/Catalog契约 |
| `JACO-199-02` | Jaco迁移到Form vNext | `Done`；361 tests与aggregate gate通过，实际UI操作测试未执行 | [Jaco Form vNext迁移计划](../../../app/jaco/docs/dev/issue-199/form-vnext-migration.md) | 已消费 `FORM-199-02` |

本轮实际执行顺序：

1. 固定 resolver/topology/error/validator/adapter contract并完成三个 Form crate producer；
2. 实施 Feiwen DB resource/QueryCatalog通道；
3. 迁移 Jaco Form consumer与Feiwen Query/Fetch Form consumer；
4. 汇合后删除旧Form surface并执行workspace aggregate gate；
5. 实施与自动化结果已回写专题文档；实际 UI 操作测试按本轮要求未执行。

## 历史子任务

| ID | 范围 | 状态 | 文档/证据 |
| --- | --- | --- | --- |
| `FORM-199-01` | Field描述符、显式Form owner与core-private Transition | `Done` | [专题与验证档案](../../../crates/gpui-form/docs/dev/issue-199/field-descriptors-and-internal-transitions.md) |
| `JACO-199-01` | Jaco迁移到上一轮显式owner Form API | `Implemented` | [Jaco历史迁移文档](../../../app/jaco/docs/dev/issue-199/form-migration.md) |
| `ISSUE-199-ROOT-01` | 上一轮跨owner Form delivery计划与完成审计 | 历史归档 | [原README内容原样归档](explicit-form-owner-delivery.md) |

三个 Form owner 的上一轮详细记录继续保留：

- [gpui-form owner](../../../crates/gpui-form/docs/dev/issue-199/README.md)
- [gpui-form-macros owner](../../../crates/gpui-form-macros/docs/dev/issue-199/README.md)
- [gpui-form-gpui-component owner](../../../crates/gpui-form-gpui-component/docs/dev/issue-199/README.md)

## 调研与决策文档

| 文档 | 状态 | 职责 |
| --- | --- | --- |
| [应用迁移调研与待确认问题](application-migration-decisions.md) | 持续维护 | 保留全部问题编号、用户回答、已确认产品语义与仍未回答项；Feiwen owner plan直接消费 |
| [workspace Store/Operation/Form 适用性调研](workspace-store-operation-form-assessment.md) | 已审阅；MCP runtime暂缓 | 记录全局候选与“不改Store内部”的结论 |
| [上一轮root delivery归档](explicit-form-owner-delivery.md) | 历史原样 | 保存此前共享规格、工作包、验证与完成审计，不作为vNext执行入口 |

## 暂缓范围

| 范围 | 本轮状态 | 后续入口 |
| --- | --- | --- |
| HTTP Client Form/运行/Store | 不做 | 总草稿 `HTTP-*` 问题保持未回答 |
| Novel Download Form/并发/文件提交 | 不做 | 总草稿 `NOVEL-*` 问题保持未回答 |
| Jaco Conversation Transition | 本轮不做 | 总草稿保留已确认语义与技术问题，另建owner plan后再实施 |
| Jaco MCP runtime Transition | 本轮不做 | 只允许 `JACO-199-02` 迁移MCP设置表单，不改连接/OAuth/tool runtime |

## 状态更新规则

- `Draft` 表示仍有会影响实现的精确contract或producer gate；不能据此开始依赖该签名的consumer代码。
- `Ready` 只表示专题执行计划已闭环，不表示代码已实施。
- `In progress` 表示实现或必要验证仍在进行。
- `Done` 必须登记实际代码、定向自动化、跨owner residual/aggregate gate和完成证据；明确排除的
  实际 UI 操作测试单独记录为“未执行”，不能写成通过。
- 后续新增子任务时先建独立命名文档，再在本页增加一行；不把正文重新扩成执行计划。
