# Jaco 开发计划

这里保存 Jaco 尚未实施或正在实施的开发计划。计划必须以当前代码为依据，先完成产品与架构决策，再进入实现。

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#173](https://github.com/suxiaoshao/gpui/issues/173) ConversationEntry 与 AgentRun 分层重建 | 实现完成，自动化、bundle、隔离数据启动与窗口交互 smoke 均已验证 | [issue-173/README.md](issue-173/README.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) 纯 UI ChatForm、快捷键运行设置与临时窗口 | 设计、实现、自动化测试、残留 API 和文档一致性审计已完成；已完成 home、provider、shortcut 定向 UI smoke，临时窗口全局快捷键与有数据列表流程待人工验证 | [设计（中文）](issue-175/design.zh-CN.md)、[Design (English)](issue-175/design.md)、[文档索引](issue-175/README.zh-CN.md) |

## 跨功能迁移

- [内置主题来源与同步规则](theme-sources.md)
- **当前迁移（2026-07-21）**：[Jaco GPUI `1d217ee39d381ac101b7cf49d3d22451ac1093fe` ->
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`、gpui-component
  `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` ->
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5` 迁移](migrations/gpui-1a246efd-component-5b45bcb.md)：
  依赖与非视觉兼容部分已完成；会改变颜色、边框、圆角、间距或组件外观的改动已回退，待重新讨论；覆盖 Jaco window/timer/layout、JSON themes 与 Aurora、ThemeToken 背景、
  ListItem/picker/defer、Scrollable/Input，以及共享 `app-theme` 在 editor/Markdown 中的运行时消费回归；
  generated Material 配色不由 Jaco 生成。当前 gpui-component `5b45bcb` 的 TextView 会缓存
  parse-time highlight theme，因此主题切换验收受上游
  [`UPSTREAM-TEXT-15`](../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/upstream-text-theme.md)
  阻断；Jaco 不添加主题监听或重解析 workaround。
- [Jaco gpui-form 类型化双向绑定迁移](gpui-form-migration.md)：源码、自动化验证、依赖升级、
  `trybuild` compile-fail harness、residual audit 与定向 Computer Use smoke 已完成；临时窗口
  全局快捷键与有数据列表键盘流程仍需人工验证。
