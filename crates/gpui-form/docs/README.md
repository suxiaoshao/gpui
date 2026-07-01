# gpui-form docs

`gpui-form` 文档按主题拆分，避免单个开发计划同时承载历史、架构和具体缺陷设计。

| 文档 | 用途 |
| --- | --- |
| `development-plan.md` | 当前开发入口：状态、文档结构、近期优先级和跨主题边界。 |
| `array-design.md` | dynamic array 的结构、dirty/default-value 语义、数据流和修复计划。 |
| `number-input-design.md` | number 字段使用 `NumberInput` 的组件约束，以及 raw input dirty/default 语义。 |
| `validation-routing.md` | validation report 路由到普通字段、group 和 array 的路径归属规则。 |
| `meta-and-submit-state.md` | `FieldMeta` / `FormMeta` 的保存事实、派生查询和 submit final report 判定模型。 |
| `phase-1-development-plan.md` | 第一阶段完整历史计划和已落地实现记录，保留细节但不再作为唯一入口。 |

新增专题时优先新建独立文档，并在 `development-plan.md` 中只保留入口和状态摘要。
