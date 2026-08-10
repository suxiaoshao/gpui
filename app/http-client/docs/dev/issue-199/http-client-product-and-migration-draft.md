# Issue #199：HTTP Client 产品与迁移草稿

## 状态与文档边界

- 状态：`Draft`
- 最近整理：`2026-08-10`
- 子任务入口：[HTTP Client Issue #199 跟踪](README.md)
- 已完成子阶段：[Request Form、prepared request 与 Store 适用性实施计划](request-form-and-preparation-plan.md)

本草稿只保留仍未实施、会影响后续 Send / Operation / Response 或未来共享状态设计的产品目标、已确认运行语义和未回答问题。Request Form、Params、Headers、五种 Body、Auth、redirect、timeout 快照和 prepared request 的已交付契约，以已完成实施计划及源码为准，不在这里维护第二份副本。

## 尚未实施的基础可用目标

- 从 accepted `PreparedRequest` 发起真实 HTTP 请求；不得从 live Form、native control 或 Store 重建请求。
- 为单请求页面建立 Send / Cancel / completion 的运行 owner，并落实下列已确认的 `refresh::Operation` 语义。
- 展示至少 status、headers 和 body 的 Response UI；`ResponseData` 的精确数据契约仍由 `HTTP-RUN-Q01` 闭环。
- transport、Response 和 Operation 的后续实施不得顺手加入多 tab、History、Favorites、Environment、自动 Cookie Jar、脚本、GraphQL、OAuth、代理、客户端证书或其他高级配置。

## 已确认、仍约束后续 Send 的运行语义

### HTTP-RUN-Q02：4xx / 5xx 是正常 Response

状态：**已确认，尚未实施**。

只要 transport 已收到并成功读取 HTTP response，任何 status code 都进入 `ResponseData`。4xx / 5xx 不是 `RequestProblem`；禁用 redirect 时收到的 3xx 同样是正常 Response。DNS、连接、TLS、timeout、redirect loop / 超过跳转上限，以及完整 response 形成前的 I/O 失败才是运行问题。

### HTTP-RUN-Q03：resend 保留上一份 Response

状态：**已确认，尚未实施**。

运行采用 `refresh::Operation<ResponseData, RequestProblem, Task>`：第一次 Send 为 `Idle -> Loading`；已有 Response 的再次 Send 为 `Ready -> Refreshing` 并保留旧 Response；刷新失败为 `Degraded`，保留旧 Response 并展示最新问题；刷新成功才替换数据。

### HTTP-RUN-Q05：Send 冻结不可变请求快照

状态：**已确认，尚未实施**。

Send 对当前 `RequestDraft` 提交验证并从 accepted `Prepared<RequestDraft>` 编译不可变快照。Task 只读取该快照；运行中编辑 Form 只影响下一次 Send。

### HTTP-RUN-Q06：运行中拒绝重复 Send，取消后才能重发

状态：**已确认，尚未实施**。

`Loading` / `Refreshing` 期间禁用 Send、提供 Cancel；异常路径到达的 Send 也直接丢弃，不排队、不替换。取消初次请求回到 `Idle`；取消刷新回到 `Ready` 并保留旧 Response。Operation state 是唯一 Task owner，Cancel 先转换状态再 drop Task；迟到 completion/progress 不得覆盖后续请求。timeout 是 `RequestProblem`：初次请求进入 `Failed`，刷新进入保留旧 Response 的 `Degraded`。

### HTTP-RUN-Q07：显式 resend 不做二次确认

状态：**已确认，尚未实施**。

请求不运行时，每次用户显式点击 Send 都表示新请求；无论 method 是否幂等或快照是否相同，均不二次确认。基础版本不自动 Retry，是否重发完全由用户决定。

## 未回答的问题

### 请求运行与 Operation

- **HTTP-RUN-Q01：** `ResponseData` 的权威内容是什么：status、headers、timing、body、大小、截断和 binary 表示分别如何建模。
- **HTTP-RUN-Q04：** 未来引入 request tab 后，是每个 tab 独立 runtime 还是共享 runtime，以及是否允许多 tab 并行。

### Store 与 repair

- **HTTP-STORE-Q01：** history、favorites、environment、auth、cookie jar 分别是否跨 tab/window 共享，哪些需要 Store，哪些需要持久化服务。
- **HTTP-STORE-Q02：** 多 tab catalog、active tab 和 request identity 的 owner。
- **HTTP-STORE-Q03：** secret/auth/cookie 的内存与持久化安全边界；不能因为共享就直接放入普通 UI snapshot。
- **HTTP-REPAIR-Q01：** auth challenge、客户端证书、代理或 TLS 问题是否提供显式修复动作；在动作未定义前不采用预定义 `repair::Operation`。

## 后续跟踪规则

1. 已完成 Request Form 的契约、文件动作和验证证据只维护在已完成实施计划；本草稿不回填其实现细节。
2. `HTTP-RUN-Q02/Q03/Q05/Q06/Q07` 是已确认的后续 Send 约束，不能以实施偏好改变。
3. `HTTP-RUN-Q01` 闭环后，新建独立 Send / Operation / Response 实施计划；`HTTP-RUN-Q04`、`HTTP-STORE-*` 和 `HTTP-REPAIR-Q01` 不阻塞单请求 Send / Response。
