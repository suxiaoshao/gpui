# gpui-form 开发计划

状态：第一阶段 runtime、derive 宏、nested group、dynamic array、组件 binding、`garde + validify`
pipeline 和字段级 `required` 元数据已落地。当前开发计划不再把所有细节堆在一个文件里，完整历史记录移到
`phase-1-development-plan.md`，专题设计拆到独立文档。

## 文档结构

| 文档 | 当前职责 |
| --- | --- |
| `README.md` | docs 目录索引和拆分规则。 |
| `phase-1-development-plan.md` | 第一阶段完整历史设计、已实现能力和验证记录。 |
| `array-design.md` | dynamic array 的当前实现、结构 dirty/default-value 根因修复方案和后续测试计划。 |
| `validation-routing.md` | validation report 路由到普通字段、group 和 array 的路径归属规则。 |

## 当前边界

- `crates/gpui-form` 负责 form runtime、field/group/array store、component binding、validation/transform
  pipeline 和基础 view 语义。
- `crates/gpui-form-macros` 负责 derive 属性解析和 generated glue code。
- 接入 app 负责业务 validator、数据库/config/keychain 写回、UI row action、icon、i18n resolver 和全局状态。
- `gpui-form` 不访问数据库、keychain、app runtime config 或网络数据源。
- `required` 是 field metadata 和 UI marker 语义，不自动生成 required validation error。

## 近期优先级

1. 修复 dynamic array 结构性 dirty/default-value 建模，具体设计见 `array-design.md`。
2. 修复 nested group validation routing 的 sibling 同名字段误归属，具体设计见 `validation-routing.md`。
3. 给 array structural edits 和 validation routing 增加 focused tests：append/remove/move/swap/replace、
   回到默认值、reset rebase、parent/child 同名字段。
4. 继续保持 app-specific validation 在接入 app 内，不把 Provider/MCP/Prompt/Shortcut 的业务规则下沉到
   `gpui-form`。

## 主题拆分规则

- 设计超过一个具体问题域时，新建专题文档；本文件只保留状态和入口。
- 每个专题文档必须明确文件/模块结构、所用组件、自定义类型、数据流、全局数据管理、数据库变更、
  数据获取方式、icon、i18n 和新增依赖。
- 对不涉及的项也要明确写“无”，避免后续实现时自行补语义。
