# Gupi：独立 Pi 原生客户端

- 总 issue：[#217](https://github.com/suxiaoshao/gpui/issues/217)
- 本文职责：产品边界、阶段关系与计划导航；各阶段状态由各自计划维护。
- [未完成项与能力边界索引](follow-ups.md)：集中查看依赖阻塞、后续功能和发行验证边界；详细内容引用各阶段原文档。

Gupi 是独立 GPUI 桌面应用，通过用户本机 Pi 的 RPC 能力提供原生交互。Pi 的安装、升级、登录及模型提供方配置由用户在外部完成；个人包、Skill、模板与系统提示词管理由 [#231 设置计划](../../../app/gupi/docs/dev/issue-231/README.md)接入。Gupi 不继承 Jaco 的数据库、provider 或 MCP 管理体系。

## 阶段与依赖

| 顺序 | Issue | 交付边界 | 本地计划 |
| --- | --- | --- | --- |
| 1 | [#218](https://github.com/suxiaoshao/gpui/issues/218) | 应用骨架、启动引导、分类恢复、设置、主题、本地化与基础打包 | [第一阶段](../issue-218/README.md) |
| 2 | [#219](https://github.com/suxiaoshao/gpui/issues/219) | Pi RPC 与进程生命周期 | [第二阶段](../issue-219/README.md) |
| 3 | [#220](https://github.com/suxiaoshao/gpui/issues/220) | 会话目录、恢复、主对话、只读树与时间线、原生 fork | [第三阶段开发计划](../issue-220/README.md) |
| 4 | [#221](https://github.com/suxiaoshao/gpui/issues/221) | 临时窗口与全局快捷翻译 | 到该阶段再建立 |
| 5 | [#222](https://github.com/suxiaoshao/gpui/issues/222) | 完整队列与更完整的扩展 UI；基础历史、fork 和工具详情已有前序实现 | [扩展 UI 子计划](../../../app/gupi/docs/dev/issue-222/README.md)；其余内容开工时再展开 |
| 6 | [#223](https://github.com/suxiaoshao/gpui/issues/223) | 原生体验、完整打包与发行验收 | 到该阶段再建立 |

按表中顺序推进，阶段分支通过 PR 汇入总 Issue 分支 `codex/217-gupi-pi-rpc-client`，后续阶段从更新后的总 Issue 分支开始。第一阶段的基础打包用于验证桌面启动链路，第六阶段承担完整发行验收。开发期 logo 的使用决定与许可状态见第一阶段 D-04。

## 文档归属

父子 issue 使用平级的 `docs/dev/issue-<number>/`，通过链接表达父子关系。父 issue 不收纳子 issue 的文件目录，也不重复维护子阶段的类型和工作包；跨阶段遗留项统一由[未完成项索引](follow-ups.md)提供摘要和原文入口。

```text
docs/dev/issue-217/README.md             # 总览、阶段关系
docs/dev/issue-218/README.md             # 第一阶段决定、契约、依赖与交付状态
app/gupi/docs/dev/issue-218/README.md    # 第一阶段应用内部设计
crates/xtask/docs/dev/issue-218/README.md # 第一阶段打包工具接入
```

后续阶段按需要增加自己的独立目录，不预建空计划。跨阶段长期稳定的应用说明在实施后归入应用 README；阶段计划保留当时的决定和证据。
