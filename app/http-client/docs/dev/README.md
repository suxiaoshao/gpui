# HTTP Client 开发文档

## 功能与重构计划

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) HTTP Client 基础可用与 Form / Operation / Store 迁移 | Request Form / prepared request `Done`（`933ee09`）；单请求 Send / Response `Done`（`24e4a9f`）；loopback test-server producer/consumer `Done`；Response 媒体/PDF `In progress` | [issue-199/README.md](issue-199/README.md) |

## 依赖迁移

| 日期 | 迁移批次 | 状态 | 入口 |
| --- | --- | --- | --- |
| 2026-07-21 | `gpui-1a246efd-component-5b45bcb` | **当前迁移**；待执行 | [GPUI `1d217ee` → `1a246efd`；gpui-component `c36b0c6` → `5b45bcb`](migrations/gpui-1a246efd-component-5b45bcb.md) |

迁移文档使用目标依赖的短 SHA 命名，正文同时记录完整 source SHA。后续依赖迁移新增文件和索引项，
不覆盖既有迁移记录。
