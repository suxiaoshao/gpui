# HTTP Client 开发文档

## 功能与重构计划

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#200](https://github.com/suxiaoshao/gpui/issues/200) Response 音频迁移与 GStreamer 删除 | `In progress`；使用 Rodio/CPAL/Symphonia 播放音频，保留 PDF，删除视频与全部 GStreamer runtime/打包链路 | [issue-200/README.md](issue-200/README.md) |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) HTTP Client 基础可用与 Form / Operation / Store 迁移 | `Done`；Request Form / prepared request（`933ee09`）、单请求 Send / Response（`24e4a9f`）与 loopback test-server producer/consumer 已交付；媒体历史记录已由 #200 的 Rodio 迁移取代 | [issue-199/README.md](issue-199/README.md) |

## 依赖迁移

| 日期 | 迁移批次 | 状态 | 入口 |
| --- | --- | --- | --- |
| 2026-07-21 | `gpui-1a246efd-component-5b45bcb` | **当前迁移**；待执行 | [GPUI `1d217ee` → `1a246efd`；gpui-component `c36b0c6` → `5b45bcb`](migrations/gpui-1a246efd-component-5b45bcb.md) |

迁移文档使用目标依赖的短 SHA 命名，正文同时记录完整 source SHA。后续依赖迁移新增文件和索引项，
不覆盖既有迁移记录。
