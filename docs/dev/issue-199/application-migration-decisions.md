# Issue #199：应用迁移决策与后续调研草稿

## 1. 文档状态与使用方式

- 状态：`Draft`
- 最近整理：`2026-08-13`
- 总入口：[Issue #199 多轮任务索引](README.md)

本文只保留尚未实施且仍会影响后续设计的用户决定、技术结论、暂缓范围和未回答问题。已经由源码、
自动化与独立 owner 执行文档承接的 Form vNext、Jaco Form consumer、Jaco Conversation Transition、
Feiwen Query/Fetch/Catalog/DB/Form 和 Novel Download 内容已从本草稿删除，不在这里维护第二份完成态说明。HTTP Client
也已建立 owner 草稿；本文只保留其总状态和入口，不复制 HTTP 专属问题或回答。

保留规则：

1. 已回答但尚未实施的决定继续保留，直到对应 owner 计划实施并记录完成证据。
2. 未回答或只完成部分回答的问题必须保留原编号，不用实现偏好替代答案。
3. 明确暂缓的范围保留恢复入口，不提前设计或创建实施计划。
4. 已完成内容从草稿删除；其历史、契约和验证以 Git 及对应 owner 执行文档为准。

HTTP 专属 `HTTP-*` 编号由
[HTTP Client owner 草稿](../../../app/http-client/docs/dev/issue-199/http-client-product-and-migration-draft.md)
维护。

## 2. 当前未完成范围

| 范围 | 状态 | 已确认方向 | 仍需处理 |
| --- | --- | --- | --- |
| Jaco MCP runtime | 已移交 [#201](https://github.com/suxiaoshao/gpui/issues/201) | #199 不开始 Transition / Store 迁移 | 由 #201 建立独立 owner plan |
| HTTP Client | 单请求 Request Form、Send 与 Response 均为 `Done` | 未来 History、multi-tab、Store 与 repair 问题只在 app owner 中维护 | [HTTP Client owner索引](../../../app/http-client/docs/dev/issue-199/README.md) |

Issue 范围内继续有效的共通边界：

- `gpui-operation` 不新增第三个预定义 family；不符合 refresh / repair 的流程由应用定义自己的
  `Transition<Message>`。
- `gpui-store` 不新增公共 dispatch / message API；应用在 Store 所有的领域状态上运行 Transition，再用
  现有 `update` / `update_if` 发布。

## 3. Jaco MCP runtime（已移交 #201）

状态：**已从 #199 移交**。

- 自定义 Transition、status Store、连接/OAuth/tool runtime 的后续设计由
  [#201](https://github.com/suxiaoshao/gpui/issues/201) 唯一承接。
- #201 必须继承 [#184](https://github.com/suxiaoshao/gpui/issues/184) 已确认的 MCP alias/wire-name
  兼容护栏；本草稿不复制其问题和回答。

## 4. HTTP Client

状态：单请求 Request Form、不可变 prepared request、真实 Send、私有 Transition、Response 收集、
viewer 与完成后 Save 均为 `Done`。已完成内容由
[Request Form 实施计划](../../../app/http-client/docs/dev/issue-199/request-form-and-preparation-plan.md)和
[Send / Response 实施计划](../../../app/http-client/docs/dev/issue-199/request-send-and-response-plan.md)
承接，本根草稿不维护第二份实现说明。

未来 History、multi-tab、Store 与 repair 的问题继续由
[HTTP Client 产品与迁移草稿](../../../app/http-client/docs/dev/issue-199/http-client-product-and-migration-draft.md)
维护；它们不属于本轮单请求基础可用范围。

## 5. 后续入口

1. Jaco MCP runtime 由 [#201](https://github.com/suxiaoshao/gpui/issues/201) 重新调研并建立 owner plan；必须继承 #184 的 alias/wire-name 兼容护栏。
2. HTTP Client 后续只有在用户选择 History、multi-tab、Store 或 repair 范围后，才从 owner 草稿建立新的
   独立计划。
