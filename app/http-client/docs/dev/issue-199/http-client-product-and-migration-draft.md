# Issue #199：HTTP Client 产品与迁移草稿

## 状态与文档边界

- 状态：`Draft`
- 最近整理：`2026-08-11`
- 子任务入口：[HTTP Client Issue #199 跟踪](README.md)

本草稿只保留**尚未进入代码且会影响后续范围拆分或共享状态设计**的问题。已完成的 Request Form、
Send 与 Response 范围以源码及索引中标记为 `Done` 的实施计划为唯一权威；本文件不再复述完成清单、
实现契约或历史讨论。

## 已确认的后续边界

- 未来如加入 `Send and Download`，必须单独设计网络流到用户目标路径的 commit、取消、限额与清理语义；
  不得复用普通 Send 的完整 response 临时文件路径作为旁路。
- 未来 Repair 先定义用户可执行的修复动作；在没有动作契约前，不采用预定义
  `gpui_operation::repair::Operation`。

Response 媒体/PDF 的未完成发行门禁已移交 [#200](https://github.com/suxiaoshao/gpui/issues/200)；该 issue 的
[owner plan](../issue-200/README.md) 是后续入口，#199 的 [历史计划](response-media-and-pdf-preview-plan.md)
仅保留已进入代码的实施证据。

## 尚未解决的未来范围

### History、多 tab 与 Store

- **HTTP-RUN-Q04：** 引入 request tab 后，每个 tab 是否拥有独立 runtime，是否允许多 tab 并行。
- **HTTP-STORE-Q01：** History、Favorites、Environment、auth、Cookie Jar 分别是否跨 tab/window 共享，
  哪些由 Store 管理，哪些需要持久化服务。
- **HTTP-STORE-Q02：** 多 tab catalog、active tab 与 request identity 的权威 owner。
- **HTTP-STORE-Q03：** secret/auth/cookie 的内存与持久化安全边界；共享不等于可以放入普通 UI snapshot。

### Repair

- **HTTP-REPAIR-Q01：** auth challenge、客户端证书、代理或 TLS 问题是否提供显式修复动作，以及每个动作
  的状态、权限和取消语义。

## 后续跟踪规则

1. 已完成范围只维护在源码和相应的 `Done` 计划；本草稿不回填实现细节。
2. 本草稿中的每个问题都需要在范围被授权后再建立独立命名的实施计划；不得以回答这些问题为由改动已完成
   的单请求运行态。
3. 新计划如引入 Store、持久化、secret 或 Repair，必须先固定 owner、数据边界、迁移/清理与安全契约。
