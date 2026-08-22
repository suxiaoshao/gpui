# gpui-form-macros 开发计划

本目录保存派生宏与代码生成的维护者计划，不定义 crate 的公开用法。使用者应阅读
[英文指南](../guide.md) 或 [中文指南](../guide.zh-CN.md)。

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#205](https://github.com/suxiaoshao/gpui/issues/205) 全 workspace 依赖升级 | `In progress`；trybuild 与 proc-macro cluster 本地验证通过 | [owner plan](issue-205/dependency-upgrade-plan.md) |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) Form derive 演进 | `FormModel` v2已实施；`FormSchema` vNext计划为Draft | [v2历史owner文档](issue-199/README.md)、[vNext跨crate执行计划](../../../gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) 旧版 `FormStore` 派生宏 | Superseded；PR #176 历史归档 | [issue-175/README.md](issue-175/README.md) |
