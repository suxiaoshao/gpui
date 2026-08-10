# Issue #199：多轮任务总指导、进度与文档索引

## 文档职责

- 状态：`进行中`。本轮 Form breaking 重构、Jaco/Feiwen consumer 再迁移与 Novel Download 均为 `Done`；
  HTTP Client、Jaco Conversation/MCP runtime 等后续轮次仍未开始。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 分支：`codex/199-adopt-gpui-store-form-operation`
- 最近更新：2026-08-10

本文只维护 Issue #199 的总指导、轮次、子任务状态、依赖顺序和专题文档入口。架构契约、文件清单、
工作包、测试与完成证据写在独立专题文档中；后续任务不得继续把执行计划堆入本 README。

## 总指导

1. 开发文档只写中文；README/guide 等 crate 对外文档继续维护中英文版本。
2. 一个可独立实施或多轮迭代的任务使用独立命名文档；各 owner 的 `README.md` 只做状态/索引。
3. 已完成历史文档保留原样并继续可发现，不改写成后续 breaking 计划。
4. Form core 可以内部使用 `gpui_operation::Transition`，公开/generated/adapter API 仍为领域方法。
5. Form vNext producer contract 固定并验证后，Jaco/Feiwen 才消费最终签名；应用不得各自创建兼容 shim。
6. HTTP Client 和 Jaco MCP runtime operation 继续暂缓；已完成的 Novel Download 保留 owner plan 作为
   实施证据，不直接从总草稿或本 README 派生后续改动。
7. `gpui-store` 本轮不重构：复杂状态机由业务类型自己建模，最终使用 Store `set/update` 发布即可。

## 最近完成的轮次

| ID | 范围 | 状态 | 专题文档 | 前置关系 |
| --- | --- | --- | --- | --- |
| `FORM-199-03` | `gpui-form`、`gpui-form-macros`、`gpui-form-gpui-component` 按最终目标契约进行 breaking 重构 | `Done`；producer/consumer 与 aggregate gate 通过 | [core 实施计划](../../../crates/gpui-form/docs/dev/issue-199/form-runtime-breaking-refactor-plan.md)、[macro 实施计划](../../../crates/gpui-form-macros/docs/dev/issue-199/form-schema-generation-update-plan.md)、[adapter 实施计划](../../../crates/gpui-form-gpui-component/docs/dev/issue-199/form-binding-adapter-update-plan.md) | `C-900`–`C-904` 已达 `consumer-complete` |
| `JACO-199-03` | Jaco 迁移到本轮最终 Form API 与 binding/event/version 契约 | `Done`；362 tests 与 Clippy 通过；实际 UI 操作测试未执行 | [Jaco 再迁移计划](../../../app/jaco/docs/dev/issue-199/form-breaking-api-remigration-plan.md) | 已消费 `C-900`–`C-904`；未改 MCP runtime operation |
| `FEI-199-02` | Feiwen Query/Fetch 迁移到本轮最终 Form API 与动态 topology 契约 | `Done`；93 tests 与 Clippy 通过；实际 UI 操作测试未执行 | [Feiwen 再迁移计划](../../../app/feiwen/docs/dev/issue-199/form-breaking-api-remigration-plan.md) | 已消费 `C-900`–`C-904`；未重做现有 Operation/Store/DB/Catalog |
| `NOVEL-199-01` | Novel Download 最小 Form、私有下载 Transition、唯一 Task 与 `.part` 文件事务迁移 | `Done`；39 tests 与定向门禁通过，当前工作树未提交；实际 UI 未执行 | [Novel Download 实施计划](../../../app/novel-download/docs/dev/issue-199/form-operation-download-migration-plan.md) | 消费 `C-900`–`C-904`；不引入 Store、队列、resume 或 repair |

本轮实际实施顺序：

1. `gpui-form` 先冻结 public/hidden contract；
2. `gpui-form-macros` 完成 schema 生成，同时 core 按依赖实现 identity、change、validation、version 与 binding；
3. `gpui-form-gpui-component` 在 final binding façade 上完成四个 adapter，使 `C-900`–`C-904` 达到
   `producer-ready`；
4. Jaco 与 Feiwen 并行迁移；
5. consumer 完成后删除旧 surface，执行 residual scan 与 workspace aggregate gate，使 `C-904` 达到
   `consumer-complete`。

本轮自动化验证已完成；实际 UI 操作测试按用户要求未执行。若后续需要 UI 验收，另行授权并登记。

## 上一轮已完成子任务

| ID | 范围 | 状态 | 专题文档 | 前置关系 |
| --- | --- | --- | --- | --- |
| `FORM-199-02` | 三个 gpui-form crate 的 greenfield vNext 重构 | `Done`；producer/consumer与aggregate gate通过，实际UI操作测试未执行 | [Form vNext 重构计划](../../../crates/gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md) | producer已交付 |
| `FEI-199-01` | Feiwen Form、Query/Fetch Transition、Catalog Store/Operation、DB resource与UI完整迁移 | `Done`；89 tests与aggregate gate通过，实际UI操作测试未执行 | [Feiwen完整迁移计划](../../../app/feiwen/docs/dev/issue-199/form-operation-store-migration.md) | 已消费Form并交付Feiwen内部DB/Catalog契约 |
| `JACO-199-02` | Jaco迁移到Form vNext | `Done`；361 tests与aggregate gate通过，实际UI操作测试未执行 | [Jaco Form vNext迁移计划](../../../app/jaco/docs/dev/issue-199/form-vnext-migration.md) | 已消费 `FORM-199-02` |

上一轮实际执行顺序（历史记录）：

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
| [应用迁移决策与后续调研草稿](application-migration-decisions.md) | `Draft` | 只保留未实施的 Jaco Conversation、暂缓的 Jaco MCP runtime 与 HTTP Client 未回答问题；已完成的 Form、Jaco、Feiwen 与 Novel Download 内容不再重复。 |
| [workspace Store/Operation/Form 适用性调研](workspace-store-operation-form-assessment.md) | 已审阅；MCP runtime暂缓 | 记录全局候选与“不改Store内部”的结论 |
| [上一轮root delivery归档](explicit-form-owner-delivery.md) | 历史原样 | 保存此前共享规格、工作包、验证与完成审计，不作为vNext执行入口 |

## 后续未实施范围

| 范围 | 本轮状态 | 后续入口 |
| --- | --- | --- |
| HTTP Client Form/运行/Store | 不做 | 总草稿 `HTTP-*` 问题保持未回答 |
| Jaco Conversation Transition | 本轮不做 | 总草稿保留已确认语义与技术问题，另建owner plan后再实施 |
| Jaco MCP runtime Transition | 本轮不做 | `JACO-199-03` 只迁移 MCP 设置表单 consumer，不改连接/OAuth/tool runtime |

## 状态更新规则

- `Draft` 表示仍有会影响实现的精确contract或producer gate；不能据此开始依赖该签名的consumer代码。
- `Ready` 只表示专题执行计划已闭环，不表示代码已实施。
- `In progress` 表示实现或必要验证仍在进行。
- `Done` 必须登记实际代码、定向自动化、跨owner residual/aggregate gate和完成证据；明确排除的
  实际 UI 操作测试单独记录为“未执行”，不能写成通过。
- 后续新增子任务时先建独立命名文档，再在本页增加一行；不把正文重新扩成执行计划。
