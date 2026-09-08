# Workspace development plans

## Dependency upgrade plans

- [Issue #215：GPUI Kit 与依赖升级](issue-215/README.md)：发布包替代 Git 来源、应用与共享 crate 迁移、skill/文档同步的实施计划。

## Discussion drafts

- [Gupi 总览与阶段关系](issue-217/README.md) — 父 issue 导航。
- [Gupi 第一阶段：应用骨架、启动引导与恢复入口](issue-218/README.md) — 保存产品决定、应用/打包设计及 Ready 阻断项。

## Feature plans

| Issue | 入口 |
| --- | --- |
| [#219](https://github.com/suxiaoshao/gpui/issues/219) Gupi Pi RPC 与进程生命周期 | [issue-219/README.md](issue-219/README.md) |
| [#200](https://github.com/suxiaoshao/gpui/issues/200) HTTP Client Response 音频迁移与 GStreamer 删除 | [issue-200/README.md](issue-200/README.md) |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) form owner、app store/form/operation 与 Transition 重构 | [issue-199/README.md](issue-199/README.md) |
| [#196](https://github.com/suxiaoshao/gpui/issues/196) Jaco provider 生成图片持久化与展示 | [issue-196/README.md](issue-196/README.md) |
| [#195](https://github.com/suxiaoshao/gpui/issues/195) Jaco 会话时间线持久化文件附件 | [issue-195/README.md](issue-195/README.md) |
| [#193](https://github.com/suxiaoshao/gpui/issues/193) Jaco 侧边栏会话悬浮预览、活动时间与运行状态 | [issue-193/README.md](issue-193/README.md) |
| [#190](https://github.com/suxiaoshao/gpui/issues/190) Jaco 持久化工具调用详情 | [issue-190/README.md](issue-190/README.md) |
| [#189](https://github.com/suxiaoshao/gpui/issues/189) Jaco 消息请求用量、输入框上下文占用、时间范围统计、活动热力图与费用 | [issue-189/README.md](issue-189/README.md) |
| [#188](https://github.com/suxiaoshao/gpui/issues/188) Jaco 侧边栏项目与对话上下文菜单 | [issue-188/README.md](issue-188/README.md) |
| [#178](https://github.com/suxiaoshao/gpui/issues/178) Jaco 外部文件变更监控 | [issue-178/README.md](issue-178/README.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) previous typed form delivery | [issue-175/README.md](issue-175/README.md) |

## Framework migrations

这里仅登记跨 workspace 的迁移批次。每次框架迁移使用独立的目标版本或 Git hash 标识，
不会用一个无版本文件覆盖历史计划；具体 app/crate 的实现内容放在各自的 `docs/dev`。

- [2026-07-21：gpui-1a246efd-component-5b45bcb](migrations/gpui-1a246efd-component-5b45bcb/README.md)：该批次的依赖证据、迁移顺序与发布边界。

## 目录约定

计划位置和拆分规则见 [开发文档规范](../../.agents/skills/implementation-plan-design/references/documentation-layout.md)。
索引只保留入口和用途，进度与验证结果由对应计划维护。历史迁移批次保留原路径和目标标识。
历史 `migrations/<target-id>` 使用明确版本或 Git hash 标识；新目标另建批次，不覆盖旧批次。
