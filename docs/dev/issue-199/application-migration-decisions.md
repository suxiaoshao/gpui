# Issue #199：应用迁移决策与后续调研草稿

## 1. 文档状态与使用方式

- 状态：`Draft`
- 最近整理：`2026-08-10`
- 总入口：[Issue #199 多轮任务索引](README.md)

本文只保留尚未实施且仍会影响后续设计的用户决定、技术结论、暂缓范围和未回答问题。已经由源码、
自动化与独立 owner 执行文档承接的 Form vNext、Jaco Form consumer、Feiwen Query/Fetch/Catalog/DB/Form
和 Novel Download 内容已从本草稿删除，不在这里维护第二份完成态说明。HTTP Client
也已建立 owner 草稿；本文只保留其总状态和入口，不复制 HTTP 专属问题或回答。

保留规则：

1. 已回答但尚未实施的决定继续保留，直到对应 owner 计划实施并记录完成证据。
2. 未回答或只完成部分回答的问题必须保留原编号，不用实现偏好替代答案。
3. 明确暂缓的范围保留恢复入口，不提前设计或创建实施计划。
4. 已完成内容从草稿删除；其历史、契约和验证以 Git 及对应 owner 执行文档为准。

本文中的 `CONV-*` 只是问题跟踪编号，不是 root plan 或 owner plan 的正式 ID。HTTP
专属 `HTTP-*` 编号由 [HTTP Client owner 草稿](../../../app/http-client/docs/dev/issue-199/http-client-product-and-migration-draft.md)
维护。

## 2. 当前未完成范围

| 范围 | 状态 | 已确认方向 | 仍需处理 |
| --- | --- | --- | --- |
| Jaco Conversation active run | 产品语义已确认，Transition 尚未实施 | 共享 `Submitting / Running / Stopping` 私有 Transition；非法重入丢弃，不排队 | 建立 owner plan；闭环迟到 completion 防护 |
| Jaco MCP runtime | 暂缓 | 不开始 Transition / Store 迁移 | 等用户明确恢复任务 |
| HTTP Client | Request Form / prepared request 子阶段 `Done`；Send / Response 仍为 `Draft` | HTTP 专属目标、缺失、决定和问题只在 app owner 中维护 | [HTTP Client owner索引](../../../app/http-client/docs/dev/issue-199/README.md) |

Issue 范围内继续有效的共通边界：

- `gpui-operation` 不新增第三个预定义 family；不符合 refresh / repair 的流程由应用定义自己的
  `Transition<Message>`。
- `gpui-store` 不新增公共 dispatch / message API；应用在 Store 所有的领域状态上运行 Transition，再用
  现有 `update` / `update_if` 发布。

## 3. Jaco Conversation active run

### 3.1 已确认的产品语义

#### CONV-01：继续现有运行行为

状态：**已确认；当前行为已存在，Transition 迁移必须保持**。

- 同一个 conversation 同时最多有一个 active run。
- 不同 conversation 可以并行运行。
- `Stopping` 仍属于 active run；清理完成前不能开始下一次运行。
- 保留现有取消、审批取消、运行完成与 conversation 刷新行为。

#### CONV-02：`Stopping` 期间出现新消息时直接丢弃

状态：**已确认；当前 UI gate 已存在，Transition 迁移必须保持**。

- UI 必须继续阻止用户在 `Stopping` 时发送新消息。
- 将来迁入自定义 Transition 后，`Running` 或 `Stopping` 收到非法 `Start` / `Send` 是程序错误。
- Transition 应先恢复或保留原状态，再丢弃事件，并可写 debug / tracing 诊断。
- 不建立队列，不自动替换，不在停止完成后补发，也不把该事件转成正常重试。

#### CONV-03：区分 agent 业务失败与 owner 基础设施错误

状态：**已确认；Transition 迁移必须保持**。

agent/provider/tool 的业务性失败继续持久化到数据库，不属于 active-run Transition 需要额外保存的
业务错误；用户取消继续形成现有持久化终态。启动前、最终提交或 Stop 收尾等 owner 基础设施错误继续即时
报告。目标 Transition 只表达 active-run 生命周期和非法事件诊断，不新增或复制错误持久化层。

#### CONV-04：不增加新的运行能力

状态：**已确认；Transition 迁移必须保持**。

- 不增加 pause / resume。
- 不增加后台继续运行语义。
- 不增加审批转交给其他 owner / window 的能力。
- 现有单次运行内部的 approval broker 继续保留；这里排除的是新的跨 owner 转交协议。

### 3.2 Transition 的目标约束

状态：**目标边界已确认，尚未形成 owner plan**。

Conversation 使用应用私有 `Transition<Message>` 统一提交准入和 active-run 生命周期，不套用
`refresh` / `repair` 预定义状态机。`ConversationRuntimeStore` 已经是同一 conversation 在各页面间的
共享 owner，继续由它持有这套 Transition；不把该状态迁入 `gpui-store`。

权威状态必须在不可逆 user entry 写入前同步进入共享的 `Submitting`，让所有引用同一 runtime owner
的页面立即看到同一个提交中状态。

```text
Idle
  -- Submit(submission task) --> Submitting

Submitting
  -- SubmissionSucceeded(run resources) --> Running
  -- SubmissionFailed --> Idle
  -- Submit/Start --> Submitting（非法事件，记录诊断后丢弃）

Running
  -- Stop --> Stopping
  -- RunFinished --> Idle
  -- Start/Send --> Running（非法事件，记录诊断后丢弃）

Stopping
  -- StopFinished --> Idle
  -- Start/Send/Stop --> Stopping（非法或重复事件，丢弃）
```

目标行为约束：

- `Submitting` 必须在调用 user entry 持久化之前同步建立，并持有本次提交生命周期 Task。
- 所有引用同一 `ConversationRuntimeStore` 的页面从同一状态投影按钮；`Submitting` 时显示加载并禁止
  再次发送，不提供队列、替换或取消。
- 提交成功后进入 `Running`；提交或启动失败后回到 `Idle`，继续使用现有即时错误通知。
- 页面局部 `submission_task` 不再是权威准入状态，由共享 Transition 的 Task 所有权替代。
- `Submitting`、`Running` 或 `Stopping` 收到非法再次提交时保留当前状态、记录诊断并丢弃事件。
- agent/provider/tool 业务错误仍按 `CONV-03` 持久化，不并入生命周期错误状态。

### 3.3 问题状态

#### CONV-Q01：运行准入必须发生在不可逆的用户消息写入之前

状态：**已确认，尚未实施**。

提交命令先同步让私有 Transition 接受 `Submit`，成功取得共享 `Submitting` 准入后才开始不可逆的
user entry 持久化。所有页面必须在写库前看到同一个状态；提交成功后进入 `Running`，失败则释放回
`Idle`。非法的再次提交直接丢弃，不排队。

这不要求数据库写入和状态变化处于同一个数据库事务；共享准入状态必须先于不可逆写入建立。具体消息
enum、Task 返回值和错误传递由后续 owner plan 落实。

#### CONV-Q02：迟到 completion 的等价保护

状态：**待实现调研**。

迁移时逐一审计所有 completion producer 是否都受运行态唯一 Task 的取消约束。若仍有 detached
producer 或独立 cleanup completion，则必须保留 run key / generation；只有证明它们不可能在状态
替换后投递，才可仅依靠 Task drop 的取消语义。

## 4. Jaco MCP runtime

状态：**暂缓**。

- 自定义 Transition 与 status Store 当前不继续调研、不建立 owner plan、不实施。
- 用户明确恢复该任务后，再从当前源码重新调研；本草稿不预先选择状态机或 Store owner。

## 5. HTTP Client

状态：Request Form / prepared request 子阶段已经 `Done`，Send / Operation / Response 子阶段仍为
`Draft`。本轮 HTTP Client 必须做到单请求场景下基础可用，不以迁移 shared crates 作为完成条件。
其专属功能缺失、`HTTP-*` 问题和产品回答继续由
[HTTP Client 产品与迁移草稿](../../../app/http-client/docs/dev/issue-199/http-client-product-and-migration-draft.md)
维护；Request 子阶段的文件动作、工作包与门禁由
[独立实施计划](../../../app/http-client/docs/dev/issue-199/request-form-and-preparation-plan.md)维护。
ResponseData 的未回答问题不再影响已完成的 Request 子阶段，但继续阻塞后续 Send / Response 计划。本根草稿不维护
HTTP 详细副本。

## 6. 后续入口

1. Jaco Conversation 下一步先建立独立 owner plan，并在实现前闭环 `CONV-Q02`。
2. Jaco MCP runtime 只有在用户明确恢复后才重新调研。
3. HTTP Client 下一步在 owner 草稿中闭环 ResponseData，并为 Send / Operation / Response 新建另一份
   独立计划。
