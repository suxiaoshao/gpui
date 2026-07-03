# gpui-form 开发计划

状态：第一阶段 runtime、derive 宏、nested group、dynamic array、`garde + validify` pipeline 和字段级
`required` 元数据已落地。leaf field 已收敛到 Draft-aware `ComponentFieldStore<Value, Binding>`；
`gpui-component` binding 已拆到独立 adapter crate。derive 宏生成边界已收口：组件 subscription 由
binding 安装，宏只保留字段枚举、路径、accessor、setter、validation routing 等无法从普通 Rust 数据结构自动
推导的 glue code。当前开发计划不再把所有细节堆在一个文件里，完整历史记录移到
`phase-1-development-plan.md`，专题设计拆到独立文档。

## 文档结构

| 文档 | 当前职责 |
| --- | --- |
| `README.md` | docs 目录索引和拆分规则。 |
| `phase-1-development-plan.md` | 第一阶段完整历史设计、已实现能力和验证记录。 |
| `binding-architecture.md` | leaf field binding 架构：core 不依赖 `gpui-component`，统一 store + Draft parse 模型，adapter crate。 |
| `macro-generation-boundary.md` | derive 宏生成边界和瘦身计划：binding-owned subscriptions、runtime helper、where 去重和保留生成项。 |
| `array-design.md` | dynamic array 的当前实现、结构 dirty/default-value 根因修复方案和后续测试计划。 |
| `number-input-design.md` | number raw input dirty/default、typed `NumberInputPolicy` 和 `new_state(...)` 类型化配置记录；最终模型见 `binding-architecture.md`。 |
| `validation-routing.md` | validation report 路由到普通字段、group 和 array 的路径归属规则。 |
| `validation-pipeline-strengthening-plan.md` | validation/required/custom context/transform/array error routing 的已落地强化设计和验收标准。 |
| `meta-and-submit-state.md` | `FieldMeta` / `FormMeta` 保存事实与派生查询边界，以及 submit final report 判定流程。 |
| `submit-handler-design.md` | sync/async submit handler、submit task ownership、`is_submitting` 派生模型和 handler trait 取舍结论。 |

## 当前边界

- `crates/gpui-form` 负责 form runtime、field/group/array store、Draft-aware component binding、
  validation/transform pipeline 和基础 view 语义；不依赖 `gpui-component` runtime。
- `crates/gpui-form-macros` 负责 derive 属性解析和 generated glue code；不直接订阅组件 state。
- `crates/gpui-form-gpui-component` 作为 adapter crate，负责 `gpui-component` 的 input、number、
  bool、select、combobox binding 以及对应组件事件订阅。
- 接入 app 负责业务 validator 的规则本身、数据库/config/keychain 写回、UI row action、icon、i18n resolver
  和全局状态；字段级 validator 的执行、错误路由、required 空值错误和 submit 前 normalize 已进入
  `gpui-form` pipeline，强化设计见 `validation-pipeline-strengthening-plan.md`。
- `gpui-form` 不访问数据库、keychain、app runtime config 或网络数据源。
- `required` 同时是 field metadata/UI marker 和 submit validation 语义；custom validator forms 可选择由
  app-specific validator 负责 required，避免把 saved secret refs 等 app 语义塞入 core `RequiredValue`。

## 当前维护重点

1. 继续完善 dynamic array 结构性 dirty/default-value 和 internal array error 语义，具体设计见
   `array-design.md`。
2. 继续给 binding draft、array structural edits、number raw dirty、submit handler runtime 和 validation routing
   增加 focused tests：
   invalid draft、typed-equal draft dirty、normalize writeback、binding-owned subscriptions、
   append/remove/move/swap/replace、回到默认值、reset rebase、parent/child 同名字段、async task lifecycle、
   handler start error 和 `SubmitError::Invalid(report)`。
3. 继续保持 app-specific validation 规则定义和持久化规则在接入 app 内；`gpui-form` 只负责执行
   validator、transform 写回和 field/group/array 错误路由。

## 主题拆分规则

- 设计超过一个具体问题域时，新建专题文档；本文件只保留状态和入口。
- 每个专题文档必须明确文件/模块结构、所用组件、自定义类型、数据流、全局数据管理、数据库变更、
  数据获取方式、icon、i18n 和新增依赖。
- 对不涉及的项也要明确写“无”，避免后续实现时自行补语义。
