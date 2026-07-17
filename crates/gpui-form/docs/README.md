# gpui-form docs

`gpui-form` 文档按主题拆分，避免单个开发计划同时承载历史、架构和具体缺陷设计。

| 文档 | 用途 |
| --- | --- |
| `development-plan.md` | 当前开发入口：状态、文档结构、近期优先级和跨主题边界。 |
| `binding-architecture.md` | 目标 component adapter 架构：三通道 owner、field handle、caller-owned subscriptions 和事件方向。 |
| `macro-generation-boundary.md` | 目标 derive 边界：只生成 form domain glue，不生成 component state/config/subscription。 |
| `array-design.md` | dynamic array 的结构、dirty/default-value 语义、数据流和修复计划。 |
| `number-input-design.md` | number raw draft、纯 codec 与 app-owned component policy 的分离设计。 |
| `validation-routing.md` | validation report 路由到普通字段、group 和 array 的路径归属规则。 |
| `validation-pipeline-strengthening-plan.md` | validation/required/custom context/transform/array error routing 的强化计划，目标是让字段校验进入 `gpui-form` pipeline。 |
| `meta-and-submit-state.md` | `FieldMeta` / `FormMeta` 的保存事实、派生查询和 submit final report 判定模型。 |
| `submit-handler-design.md` | sync/async submit handler、submit task ownership、`is_submitting` 派生模型和 handler trait 取舍结论。 |
| `external-state-synchronization-plan.md` | 最终架构：纯 form draft、独立 component adapter/config/interaction 三通道，以及旧 binding API 删除计划。 |
| `phase-1-development-plan.md` | 第一阶段历史计划和实现记录；不是当前 API 或新代码参考。 |

新增专题时优先新建独立文档，并在 `development-plan.md` 中只保留入口和状态摘要。
