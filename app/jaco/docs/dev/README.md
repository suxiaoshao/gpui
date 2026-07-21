# Jaco 开发计划

这里保存 Jaco 尚未实施或正在实施的开发计划。计划必须以当前代码为依据，先完成产品与架构决策，再进入实现。

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#173](https://github.com/suxiaoshao/gpui/issues/173) ConversationEntry 与 AgentRun 分层重建 | 实现完成，自动化、bundle、隔离数据启动与窗口交互 smoke 均已验证 | [issue-173/README.md](issue-173/README.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) 纯 UI ChatForm、快捷键运行设置与临时窗口 | 设计、实现、自动化测试、残留 API 和文档一致性审计已完成；已完成 home、provider、shortcut 定向 UI smoke，临时窗口全局快捷键与有数据列表流程待人工验证 | [设计（中文）](issue-175/design.zh-CN.md)、[Design (English)](issue-175/design.md)、[文档索引](issue-175/README.zh-CN.md) |

## 跨功能迁移

- [Jaco gpui-form 类型化双向绑定迁移](gpui-form-migration.md)：源码、自动化验证、依赖升级、
  `trybuild` compile-fail harness、residual audit 与定向 Computer Use smoke 已完成；临时窗口
  全局快捷键与有数据列表键盘流程仍需人工验证。
