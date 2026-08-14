# Issue #199：多轮任务总指导、进度与文档索引

## 文档职责

- 状态：`Done`。本轮 Form breaking 重构、Jaco/Feiwen consumer 再迁移、Novel Download、HTTP Client
  Request Form / prepared request、单请求真实 Send / Response、HTTP 测试服务与 consumer 集成测试，以及
  Jaco Conversation 私有 Transition 均已交付。Response 媒体历史计划已由 [#200](https://github.com/suxiaoshao/gpui/issues/200)
  的 Rodio 迁移与 GStreamer 删除方案取代；MCP runtime 已移交 [#201](https://github.com/suxiaoshao/gpui/issues/201)。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 分支：`codex/199-adopt-gpui-store-form-operation`
- 最近更新：2026-08-13

本文只维护 Issue #199 的总指导、轮次、子任务状态、依赖顺序和专题文档入口。架构契约、文件清单、
工作包、测试与完成证据写在独立专题文档中；后续任务不得继续把执行计划堆入本 README。

## 总指导

1. 开发文档只写中文；README/guide 等 crate 对外文档继续维护中英文版本。
2. 一个可独立实施或多轮迭代的任务使用独立命名文档；各 owner 的 `README.md` 只做状态/索引。
3. 已完成历史文档保留原样并继续可发现，不改写成后续 breaking 计划。
4. Form core 可以内部使用 `gpui_operation::Transition`，公开/generated/adapter API 仍为领域方法。
5. Form vNext producer contract 固定并验证后，Jaco/Feiwen 才消费最终签名；应用不得各自创建兼容 shim。
6. HTTP Client 本轮必须做到单请求场景下基础可用，不以迁移 shared crates 作为完成条件；先在 owner
   草稿中闭环产品与运行语义，再建立独立实施计划。Jaco MCP runtime operation 由 [#201](https://github.com/suxiaoshao/gpui/issues/201) 独立承接。已完成的
   Novel Download 保留 owner plan 作为实施证据。
7. `gpui-store` 本轮不重构：复杂状态机由业务类型自己建模，最终使用 Store `set/update` 发布即可。

## 最近完成的轮次

| ID | 范围 | 状态 | 专题文档 | 前置关系 |
| --- | --- | --- | --- | --- |
| `FORM-199-03` | `gpui-form`、`gpui-form-macros`、`gpui-form-gpui-component` 按最终目标契约进行 breaking 重构 | `Done`；producer/consumer 与 aggregate gate 通过 | [core 实施计划](../../../crates/gpui-form/docs/dev/issue-199/form-runtime-breaking-refactor-plan.md)、[macro 实施计划](../../../crates/gpui-form-macros/docs/dev/issue-199/form-schema-generation-update-plan.md)、[adapter 实施计划](../../../crates/gpui-form-gpui-component/docs/dev/issue-199/form-binding-adapter-update-plan.md) | `C-900`–`C-904` 已达 `consumer-complete` |
| `JACO-199-03` | Jaco 迁移到本轮最终 Form API 与 binding/event/version 契约 | `Done`；362 tests 与 Clippy 通过；实际 UI 操作测试未执行 | [Jaco 再迁移计划](../../../app/jaco/docs/dev/issue-199/form-breaking-api-remigration-plan.md) | 已消费 `C-900`–`C-904`；未改 MCP runtime operation |
| `FEI-199-02` | Feiwen Query/Fetch 迁移到本轮最终 Form API 与动态 topology 契约 | `Done`；93 tests 与 Clippy 通过；实际 UI 操作测试未执行 | [Feiwen 再迁移计划](../../../app/feiwen/docs/dev/issue-199/form-breaking-api-remigration-plan.md) | 已消费 `C-900`–`C-904`；未重做现有 Operation/Store/DB/Catalog |
| `NOVEL-199-01` | Novel Download 最小 Form、私有下载 Transition、唯一 Task 与 `.part` 文件事务迁移 | `Done`；39 tests 与定向门禁通过；实现提交 `64b0c4a` 已推送；实际 UI 未执行 | [Novel Download 实施计划](../../../app/novel-download/docs/dev/issue-199/form-operation-download-migration-plan.md) | 消费 `C-900`–`C-904`；不引入 Store、队列、resume 或 repair |
| `HTTP-199-02` | HTTP Client Request Form、五种 Body、Auth、redirect、prepared request 与 Store 适用性 | `Done`；56 tests 与 Check、Clippy、格式、残留扫描通过；实现提交 `933ee09` 已推送；实际 UI 操作未执行 | [Request Form 与 prepared request 实施计划](../../../app/http-client/docs/dev/issue-199/request-form-and-preparation-plan.md) | 消费 `C-900`–`C-904`；不依赖 ResponseData；不引入 Store/Operation/transport |
| `HTTP-199-03` | HTTP Client 真实 Send、私有 Transition、Response 收集、viewer 与完成后 Save | `Done`；116 tests、Check、Clippy、格式与残留扫描通过；实现提交 `24e4a9f` 已推送；实际 UI 操作未执行 | [真实 Send 与 Response 实施计划](../../../app/http-client/docs/dev/issue-199/request-send-and-response-plan.md) | 消费 `HTTP-199-02` 的 `PreparedRequest`；不引入 Store；不包含 `Send and Download` |
| `HTTP-199-05` | Hyper loopback 测试服务与 HTTP Client consumer 集成测试 | `Done`；实现提交 `1559cc8`、稳定性修正 `735bc41`；producer 15 tests、consumer transport 15 tests、app 全量 161 tests 与严格 Clippy 通过；实际 UI 未执行 | [测试服务 producer 计划](../../../crates/http-client-test-server/docs/dev/issue-199/http-test-server-plan.md)、[HTTP Client consumer 计划](../../../app/http-client/docs/dev/issue-199/http-test-server-integration-plan.md) | 消费 `HTTP-199-03` 现有 HTTP runtime；test server 只进入 dev graph |
| `JACO-199-04` | Jaco Conversation 提交、active-run、停止与迟到 completion 的私有 Transition | `Done`；实现提交 `99a073a`；聚焦 Conversation 42 tests、Jaco 全量 369 tests 与严格 Clippy 通过；实际 UI、打包与跨平台 CI 未执行 | [Conversation runtime Transition 实施计划](../../../app/jaco/docs/dev/issue-199/conversation-runtime-transition-plan.md) | 不改 MCP runtime、DB schema 或 Store |

## 已移交范围

| ID | 范围 | 状态 | 专题文档 | 前置关系 |
| --- | --- | --- | --- | --- |
| `HTTP-199-04` | HTTP Client 完整 Response 的媒体/PDF 早期方案 | `Superseded`；历史记录不再含实施指令；当前音频后端、GStreamer 删除、PDF 保留与视频排除由 [#200](https://github.com/suxiaoshao/gpui/issues/200) 唯一规定 | [历史记录](../../../app/http-client/docs/dev/issue-199/response-media-and-pdf-preview-plan.md)、[#200 root hub](../issue-200/README.md) | #200 消费既有 `Arc<ResponseData>`/read lease；不改 HTTP runtime 或 Store |

Form breaking 与 consumer 再迁移轮次的实际实施顺序：

1. `gpui-form` 先冻结 public/hidden contract；
2. `gpui-form-macros` 完成 schema 生成，同时 core 按依赖实现 identity、change、validation、version 与 binding；
3. `gpui-form-gpui-component` 在 final binding façade 上完成四个 adapter，使 `C-900`–`C-904` 达到
   `producer-ready`；
4. Jaco 与 Feiwen 并行迁移；
5. consumer 完成后删除旧 surface，执行 residual scan 与 workspace aggregate gate，使 `C-904` 达到
   `consumer-complete`。

该轮自动化验证已完成；实际 UI 操作测试按用户要求未执行。若后续需要 UI 验收，另行授权并登记。

## 上一轮已完成子任务

| ID | 范围 | 状态 | 专题文档 | 前置关系 |
| --- | --- | --- | --- | --- |
| `FORM-199-02` | 三个 gpui-form crate 的 greenfield vNext 重构 | `Done`；producer/consumer与aggregate gate通过，实际UI操作测试未执行 | [Form vNext 重构计划](../../../crates/gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md) | producer已交付 |
| `FEI-199-01` | Feiwen Form、Query/Fetch Transition、Catalog Store/Operation、DB resource与UI完整迁移 | `Done`；89 tests与aggregate gate通过，实际UI操作测试未执行 | [Feiwen完整迁移计划](../../../app/feiwen/docs/dev/issue-199/form-operation-store-migration.md) | 已消费Form并交付Feiwen内部DB/Catalog契约 |
| `JACO-199-02` | Jaco迁移到Form vNext | `Done`；361 tests与aggregate gate通过，实际UI操作测试未执行 | [Jaco Form vNext迁移计划](../../../app/jaco/docs/dev/issue-199/form-vnext-migration.md) | 已消费 `FORM-199-02` |

上一轮实际执行顺序（历史记录）：

1. 固定 resolver/topology/error/validator/adapter contract并完成三个 Form crate producer；
2. 实施 Feiwen DB resource/QueryCatalog通道；
3. 迁移 Jaco Form consumer与Feiwen Query/Fetch Form consumer；
4. 汇合后删除旧Form surface并执行workspace aggregate gate；
5. 实施与自动化结果已回写专题文档；实际 UI 操作测试按本轮要求未执行。

## 历史子任务

| ID | 范围 | 状态 | 文档/证据 |
| --- | --- | --- | --- |
| `FORM-199-01` | Field描述符、显式Form owner与core-private Transition | `Done` | [专题与验证档案](../../../crates/gpui-form/docs/dev/issue-199/field-descriptors-and-internal-transitions.md) |
| `JACO-199-01` | Jaco迁移到上一轮显式owner Form API | `Implemented` | [Jaco历史迁移文档](../../../app/jaco/docs/dev/issue-199/form-migration.md) |
| `ISSUE-199-ROOT-01` | 上一轮跨owner Form delivery计划与完成审计 | 历史归档 | [原README内容原样归档](explicit-form-owner-delivery.md) |

三个 Form owner 的上一轮详细记录继续保留：

- [gpui-form owner](../../../crates/gpui-form/docs/dev/issue-199/README.md)
- [gpui-form-macros owner](../../../crates/gpui-form-macros/docs/dev/issue-199/README.md)
- [gpui-form-gpui-component owner](../../../crates/gpui-form-gpui-component/docs/dev/issue-199/README.md)

## 调研与决策文档

| 文档 | 状态 | 职责 |
| --- | --- | --- |
| [应用迁移决策与后续调研草稿](application-migration-decisions.md) | `Draft` | 已完成 Conversation 的设计与完成证据由 `JACO-199-04` owner plan 唯一维护；MCP runtime 已移交 #201；保留 HTTP Client owner 草稿入口且不复制 HTTP 专属问题。 |
| [HTTP Client 产品与迁移草稿](../../../app/http-client/docs/dev/issue-199/http-client-product-and-migration-draft.md) | `Draft` | HTTP Client 基础可用目标、功能缺失、已确认决定与未回答 `HTTP-*` 问题的 owner 权威入口。 |
| [HTTP Client Request Form 与 prepared request 实施计划](../../../app/http-client/docs/dev/issue-199/request-form-and-preparation-plan.md) | `Done` | Request Form、五种 Body、Auth、redirect 与 prepared request 已在 `933ee09` 实施并推送；Store 本阶段不适用。 |
| [HTTP Client 真实 Send 与 Response 实施计划](../../../app/http-client/docs/dev/issue-199/request-send-and-response-plan.md) | `Done` | 已交付单请求 Send/Cancel、私有 Transition、head-first Response、受限 body 收集、安全 viewer 与完成后 Save；实现提交 `24e4a9f` 已推送；116 tests、Check、Clippy、格式与残留扫描通过，实际 UI 未执行。 |
| [HTTP Client Response 媒体/PDF 历史记录](../../../app/http-client/docs/dev/issue-199/response-media-and-pdf-preview-plan.md) | `Superseded` | 不作为实施依据；#200 规定 Rodio/CPAL/Symphonia 音频迁移、PDF 保留、视频排除与 GStreamer 全链路删除。 |
| [HTTP 测试服务 producer 实施计划](../../../crates/http-client-test-server/docs/dev/issue-199/http-test-server-plan.md) | `Done` | Hyper HTTP/1 loopback producer、受控 response/abort/echo、CLI、Postman 重定向观察示例与 16 个自动化测试已交付。 |
| [HTTP 测试服务 consumer 集成计划](../../../app/http-client/docs/dev/issue-199/http-test-server-integration-plan.md) | `Done` | HTTP Client 已用 dev-only producer 迁移 normal response/abort 测试，保留三项 request-wire raw fixture；transport 15 tests 与 app 161 tests 通过。 |
| [workspace Store/Operation/Form 适用性调研](workspace-store-operation-form-assessment.md) | 已审阅；MCP runtime 转 #201 | 记录全局候选与“不改Store内部”的结论 |
| [上一轮root delivery归档](explicit-form-owner-delivery.md) | 历史原样 | 保存此前共享规格、工作包、验证与完成审计，不作为vNext执行入口 |

## 后续未实施范围

| 范围 | 本轮状态 | 后续入口 |
| --- | --- | --- |
| HTTP Client 基础可用与 Form/运行/Store | Request Form / prepared request、单请求 Send / Response 与 loopback test-server `HTTP-199-05` 已完成；媒体后端迁移由 [#200](../issue-200/README.md) 独立承接；History/multi-tab/Store/repair 后置 | [HTTP Client owner索引](../../../app/http-client/docs/dev/issue-199/README.md) |
| Jaco MCP runtime Transition | 移交 [#201](https://github.com/suxiaoshao/gpui/issues/201) | `JACO-199-03` 只迁移 MCP 设置表单 consumer，不改连接/OAuth/tool runtime；#201 继承 #184 的 alias/wire-name 兼容护栏 |

## 状态更新规则

- `Draft` 表示仍有会影响实现的精确contract或producer gate；不能据此开始依赖该签名的consumer代码。
- `Ready` 只表示专题执行计划已闭环，不表示代码已实施。
- `In progress` 表示实现或必要验证仍在进行。
- `Done` 必须登记实际代码、定向自动化、跨owner residual/aggregate gate和完成证据；明确排除的
  实际 UI 操作测试单独记录为“未执行”，不能写成通过。
- 后续新增子任务时先建独立命名文档，再在本页增加一行；不把正文重新扩成执行计划。
