# gpui-form-gpui-component development plans

按目标版本或 Git hash 保存适配层实施计划；新迁移新增文件，不覆盖历史批次。

## 功能计划

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) Form adapter 演进 | 显式binding v2已实施；vNext adapter计划为Draft | [v2历史owner文档](issue-199/README.md)、[vNext跨crate执行计划](../../../gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) 旧版类型化绑定控件 | Superseded；PR #176 历史归档 | [issue-175/README.md](issue-175/README.md) |

## 依赖迁移

| 迁移批次 | 状态 | 入口 |
| --- | --- | --- |
| `gpui-1a246efd-component-5b45bcb` | **当前迁移**；计划待审阅 | [View 与 Combobox value API 迁移](migrations/gpui-1a246efd-component-5b45bcb.md) |
