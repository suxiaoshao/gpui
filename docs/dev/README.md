# Workspace development plans

## Architecture

- [Coding Guides 对齐与功能 crate 拆分](issue-255/README.md)：三个应用的能力边界、文本与生命周期、布局，以及官方 skills 管理和项目规则同步。

## Workspace cleanup

- [Jaco 退役与仓库边界](jaco-retirement/README.md)：已删除范围、共享能力保留边界与实际检查结果。

## Dependency upgrade plans

- [GPUI Kit 0.7.0、依赖更新与正文查找](dependency-refresh-0.7.0/README.md)：版本迁移、上游替代、直接依赖盘点与正文查找验证；输入资源和问卷由应用的 #243 文档记录。

- [2026-09：Workspace 依赖更新与官方 skill 接入](dependency-refresh-2026-09/README.md)：全项目依赖版本盘点、GPUI 配套升级、官方 skill 安装及旧资料镜像替换。
- [Issue #215：GPUI Kit 与依赖升级](issue-215/README.md)：发布包替代 Git 来源、应用与共享 crate 迁移、skill/文档同步的实施计划。

## Gupi

Gupi 的设计与后续开发见[独立项目文档](https://github.com/suxiaoshao/gupi/tree/main/docs/dev)。

## Feature plans

| Issue | 入口 |
| --- | --- |
| [#200](https://github.com/suxiaoshao/gpui/issues/200) HTTP Client Response 音频迁移与 GStreamer 删除 | [issue-200/README.md](issue-200/README.md) |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) form owner、app store/form/operation 与 Transition 重构 | [issue-199/README.md](issue-199/README.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) previous typed form delivery | [issue-175/README.md](issue-175/README.md) |

## Framework migrations

这里仅登记跨 workspace 的迁移批次。每次框架迁移使用独立的目标版本或 Git hash 标识，
不会用一个无版本文件覆盖历史计划；具体 app/crate 的实现内容放在各自的 `docs/dev`。

- [2026-07-21：gpui-1a246efd-component-5b45bcb](migrations/gpui-1a246efd-component-5b45bcb/README.md)：该批次的依赖证据、迁移顺序与发布边界。

## 目录约定

计划位置和拆分规则见 [开发文档规范](../../.agents/skills/implementation-plan-design/references/documentation-layout.md)。
索引只保留入口和用途，进度与验证结果由对应计划维护。历史迁移批次保留原路径和目标标识。
历史 `migrations/<target-id>` 使用明确版本或 Git hash 标识；新目标另建批次，不覆盖旧批次。
