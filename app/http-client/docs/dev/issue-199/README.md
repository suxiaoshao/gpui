# Issue #199：HTTP Client 子任务跟踪

- 状态：`HTTP-199-01 Draft`；`HTTP-199-02 Done`（实现提交 `933ee09` 已推送）；
  `HTTP-199-03 Done`（实现提交 `24e4a9f` 已推送）；`HTTP-199-04 In progress`
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- HTTP Client 开发文档索引：[开发文档](../README.md)
- 负责范围：`app/http-client`
- 本轮产品目标：不只迁移共享 crate，而是把 HTTP Client 做到单请求场景下基础可用

本页只跟踪 Issue #199 在 HTTP Client 内的多轮子任务、状态和专题文档入口。功能缺失、
已确认决定、未回答问题和后续迁移边界写在独立草稿中，不把正文或实施计划堆入本
README。

## 子任务

| ID | 子任务 | 状态 | 专题文档 |
| --- | --- | --- | --- |
| `HTTP-199-01` | HTTP Client 基础可用产品范围、功能缺失与 Form / Operation / Store 迁移边界 | `Draft`；Request 与单请求 Send / Response 均已完成；媒体/PDF 独立计划正在实施，History/multi-tab/Store/repair 仍待确认 | [HTTP Client 产品与迁移草稿](http-client-product-and-migration-draft.md) |
| `HTTP-199-02` | Request Form、prepared request 与 Store 适用性 | `Done`；56 个测试及 Check、Clippy、格式、残留扫描通过；实现提交 `933ee09` 已推送；实际 UI 操作未执行 | [Request Form 与 prepared request 实施计划](request-form-and-preparation-plan.md) |
| `HTTP-199-03` | 真实 Send、私有 Transition、Response 收集与 viewer | `Done`；116 tests、Check、Clippy、格式与残留扫描通过；实现提交 `24e4a9f` 已推送；实际 UI 未执行 | [真实 Send 与 Response 实施计划](request-send-and-response-plan.md) |
| `HTTP-199-04` | Response 音频、视频与 PDF 只读预览 | `In progress`；fork 已固定为 `4f1a6cc49...`，app 代码与定向自动化已实施；`C-1701` runtime manifest/notices/fixtures/三平台发行验证仍阻塞，实际 UI 未执行 | [Response 媒体与 PDF 实施计划](response-media-and-pdf-preview-plan.md) |

## 跟踪规则

- 草稿是 HTTP Client 专属决定和缺失项的唯一权威记录；根草稿只保留状态和链接。
- `Draft` 不是可执行的实施计划；某个可独立交付的子阶段问题闭环后，新建独立命名的实施文档并在
  本页引用。Request 与单请求 Send / Response 子阶段均已 `Done`；媒体/PDF 已进入 `HTTP-199-04` 实施，
  但仍受 `C-1701` 发行门禁阻塞；History、multi-tab、Store 与 repair 问题必须在闭环后以新的独立计划继续处理。
- 现有依赖迁移文档保留在 `docs/dev/migrations/`，不搬入本草稿。
