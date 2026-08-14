# Issue #199：gpui-form 总指导、进度与状态

## 文档职责

- 总体状态：`FORM-199-03 Done`；`C-900`–`C-904` 已达到 `consumer-complete`，上一轮
  `FORM-199-02` 保持 `Done`。自动化验证已通过，实际 UI 操作测试按范围未执行。
- 跟踪 Issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 计划 ID：`issue-199`
- 根计划：[Issue #199 根计划](../../../../../docs/dev/issue-199/README.md)
- 所有者目录：`crates/gpui-form`
- 所有者索引：[gpui-form 开发计划](../README.md)
- 最近更新：2026-08-09
- 已完成实现引用：`24d42494c598f48572f862caa3fbe76d8aab5b5a`（`refactor(form): adopt explicit form ownership`）

本文档是 Issue #199 在 `gpui-form` 所有者下的总入口，只维护总指导、子任务边界、进度和状态。
具体证据、API 契约、工作包与验证记录由对应的专题文档负责；设计草稿保存本轮确认设计、
验收边界与实现依据。双语 README/guide、skill 与当前源码已按 `FORM-199-03` 最终 API 对齐。

## 总指导

### 上一轮已实现基线

1. 每个编辑会话由一个 `Entity<Form<M>>` 持有 typed model、baseline、revision、validation 与 runtime
   topology；业务 model 通过 `FormSchema` 描述，不生成 per-model Form entity。
2. 静态 `TotalPath`、动态 `ItemPath`/`DynamicPath` 与 opaque `PathKey` 分工明确；collection item identity
   由 Form session 生成，业务 model 不保存 form-only ID。
3. total path 的普通读写不返回 `Result`；item、case、optional 等可能失效的定位显式返回
   `ResolveError`/`MutationError`，并通过 session/address/incarnation 拒绝 stale work。
4. 对外 API 保持 `set`、`replace`、`reset`、`append`、`remove`、`validate`、`prepare`、
   `rebase_if_revision` 等领域方法。`gpui_operation::Transition` 只用于 core-private runtime 归约。
5. focus、IME、选择区、popup、查询状态与 native subscription 由原生控件或 adapter 持有；options、
   catalog、持久化、远程请求、重试和业务 operation 仍由应用所有者持有。
6. 新能力应保持描述符组合、路径、验证作用域、稳定身份、事件与控件生命周期一致；不能只增加一个
   能读值但无法正确写入、验证或订阅的局部入口。

### 文档与实施规则

1. 新事实、已确认的目标契约和后续实施缺口直接更新[设计草稿](design-draft.md)。
2. 新轮次从草稿提取独立专题文档；总 README 只保留摘要、链接和状态。
3. 已完成专题保留原有证据、工作包和验证档案，不在总 README 中重复维护第二份详细规格。
4. 本轮双语 README/guide、skill 与实际导出已完成逐项核对；后续修改公开 API 时继续同步中英文文档。
   应用迁移由对应 app 的同 Issue 文档负责。
5. 被取代的候选方案从草稿删除；Git 与 Issue 保存历史，当前设计正文不维护第二套叙述。

## 文档地图

| 文档 | 状态 | 职责 |
| --- | --- | --- |
| [Field 描述符与内部消息 Transition 改造](field-descriptors-and-internal-transitions.md) | `Done` | 保存 explicit form ownership、total/partial descriptor、validation/submit runtime、core-private Transition、`WP-100` 至 `WP-104` 及验证档案。 |
| [gpui-form 设计草稿](design-draft.md) | 已确认 | 保存本轮完整目标设计：identity/topology、change impact、binding、validation、version 与私有 Transition。 |
| [Form vNext 重构计划](form-vnext-refactor-plan.md) | `Done` | `FORM-199-02` 的独立执行与验证记录。 |
| [Form runtime breaking 重构实施计划](form-runtime-breaking-refactor-plan.md) | `Done` | `FORM-199-03` 的 core 协调与实施证据；`C-900`–`C-904` 已完成 producer/consumer 门禁。 |
| [Issue #199 根入口](../../../../../docs/dev/issue-199/README.md) | 进行中 | 只维护 workspace 多轮子任务、顺序和专题文档索引。 |

相关 owner 的详细交付记录：

- [gpui-form-macros owner 文档](../../../../gpui-form-macros/docs/dev/issue-199/README.md)
- [gpui-form-gpui-component owner 文档](../../../../gpui-form-gpui-component/docs/dev/issue-199/README.md)
- [gpui-form-macros 本轮实施计划](../../../../gpui-form-macros/docs/dev/issue-199/form-schema-generation-update-plan.md)
- [gpui-form-gpui-component 本轮实施计划](../../../../gpui-form-gpui-component/docs/dev/issue-199/form-binding-adapter-update-plan.md)
- [Jaco 本轮 Form 再迁移计划](../../../../../app/jaco/docs/dev/issue-199/form-breaking-api-remigration-plan.md)
- [Feiwen 本轮 Form 再迁移计划](../../../../../app/feiwen/docs/dev/issue-199/form-breaking-api-remigration-plan.md)
- [Jaco 上一轮 form 迁移文档](../../../../../app/jaco/docs/dev/issue-199/form-migration.md)
- [Jaco Form vNext 迁移计划](../../../../../app/jaco/docs/dev/issue-199/form-vnext-migration.md)
- [Feiwen Form/Operation/Store/DB 完整迁移计划](../../../../../app/feiwen/docs/dev/issue-199/form-operation-store-migration.md)

## 进度与状态

| 子任务 | 状态 | 当前结论 | 下一步或完成证据 |
| --- | --- | --- | --- |
| Field descriptor 与显式 form ownership | `Done` | 静态 descriptor、显式强 form、total/partial API 与唯一 weak control boundary 已落地。 | 见[专题文档](field-descriptors-and-internal-transitions.md)及提交 `24d4249`。 |
| Form/validation 内部消息 Transition | `Done` | 权威 runtime 变化已使用 core-private message/effect 与 `Transition`；公开 API 仍为领域方法。 | 见[专题文档](field-descriptors-and-internal-transitions.md)及提交 `24d4249`。 |
| Macro、组件 adapter 与 Jaco 原子迁移 | `Done` | 三个 form crate 与 Jaco 当前 form 消费方已迁到 v2 契约。 | 见上方相关 owner 文档及提交 `24d4249`。 |
| 递归、异构与动态嵌套表单 | `Done` | `FormSchema` + `Entity<Form<M>>` + runtime ItemPath、显式 resolver 与 private topology snapshot 已落地。 | 见[vNext 实施记录](form-vnext-refactor-plan.md#实施结果2026-08-05)。 |
| breaking public API 与 consumer | `Done` | generated `FormState`、child-first `within`、public `FormItemId` 与 writable projection 已删除；双语对外文档按实际 API 更新。 | Jaco/Feiwen consumer、Clippy 与 workspace aggregate gate 已通过；实际 UI 操作测试未执行。 |
| 来源感知控件投影 | `Done` | 真实路径身份、精确 change impact、binding mailbox、revision 水位和 lifecycle 已统一实现。 | 见[本轮实施计划的实施证据](form-runtime-breaking-refactor-plan.md#实施证据)。 |
| 最终 Form runtime breaking 重构 | `Done` | PathKey/topology、ChangeSet/typed event、binding、snapshot validation、FormVersion 与私有 Transition 已落地。 | 三个 producer crate、Jaco/Feiwen、Clippy 与 workspace gate 已通过；实际 UI 操作测试未执行。 |

## 当前设计焦点

本轮已把 model schema、Form session、typed path、公开 identity 与私有 topology 明确分层：
`PathKey` 使用 session-local opaque identity，真实 canonical address 只留在 core；item/case/Some occurrence
由 Form runtime 分配且不复用。一次 mutation 通过私有 `ChangeSet` 和 Transition 原子发布 typed
`ModelChange`，binding 以 writer/projector/mailbox 处理来源抑制、合并与退休。

ValidationRequest 绑定同一 model/topology/version snapshot；异步验证以 version、occurrence 和 generation
拒绝过期完成。提交边界使用 session-bound `FormVersion` 与 `rebase_if_current`。三个 crate 的中英文
README/guide、skill、Jaco 与 Feiwen consumer 均已消费这套实现。

后续每轮讨论只更新草稿中被本轮事实或用户决定影响的部分；当一项设计被确认并具备可实施契约时，
再在本表中更新状态并建立对应专题文档。
