# Gupi：独立 Pi 原生客户端

- 总 issue：[#217](https://github.com/suxiaoshao/gpui/issues/217)
- 本文职责：产品边界、阶段关系与计划导航；各阶段状态由各自计划维护。

Gupi 是独立 GPUI 桌面应用，通过用户本机 Pi 的 RPC 能力提供原生交互。Pi 的安装、升级、登录、模型与扩展配置由用户在外部完成。Gupi 不继承 Jaco 的数据库、provider 或 MCP 管理体系。

## 阶段与依赖

| 顺序 | Issue | 交付边界 | 本地计划 |
| --- | --- | --- | --- |
| 1 | [#218](https://github.com/suxiaoshao/gpui/issues/218) | 应用骨架、启动引导、分类恢复、设置、主题、本地化与基础打包 | [第一阶段](../issue-218/README.md) |
| 2 | [#219](https://github.com/suxiaoshao/gpui/issues/219) | Pi RPC 与进程生命周期 | 到该阶段再建立 |
| 3 | [#220](https://github.com/suxiaoshao/gpui/issues/220) | 主窗口基础对话闭环 | 到该阶段再建立 |
| 4 | [#221](https://github.com/suxiaoshao/gpui/issues/221) | 临时窗口与全局快捷翻译 | 到该阶段再建立 |
| 5 | [#222](https://github.com/suxiaoshao/gpui/issues/222) | 历史会话与完整 RPC 交互 | 到该阶段再建立 |
| 6 | [#223](https://github.com/suxiaoshao/gpui/issues/223) | 原生体验、完整打包与发行验收 | 到该阶段再建立 |

按表中顺序推进，阶段分支通过 PR 汇入总 Issue 分支 `codex/217-gupi-pi-rpc-client`，后续阶段从更新后的总 Issue 分支开始。第一阶段的基础打包用于验证桌面启动链路，第六阶段承担完整发行验收。开发期 logo 的使用决定与许可状态见第一阶段 D-04。

## 文档归属

父子 issue 使用平级的 `docs/dev/issue-<number>/`，通过链接表达父子关系。父 issue 不收纳子 issue 的文件目录，也不重复维护子阶段的类型、工作包或完成状态。

```text
docs/dev/issue-217/README.md             # 总览、阶段关系
docs/dev/issue-218/README.md             # 第一阶段决定、契约、依赖与交付状态
app/gupi/docs/dev/issue-218/README.md    # 第一阶段应用内部设计
crates/xtask/docs/dev/issue-218/README.md # 第一阶段打包工具接入
```

后续阶段按需要增加自己的独立目录，不预建空计划。跨阶段长期稳定的应用说明在实施后归入应用 README；阶段计划保留当时的决定和证据。
