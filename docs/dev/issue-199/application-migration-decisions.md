# Issue #199：应用迁移决策与后续调研草稿

## 1. 文档状态与使用方式

- 状态：`Draft`
- 整理日期：`2026-08-05`
- 应用源码基线：`codex/199-adopt-gpui-store-form-operation` 分支，commit `f0ca3c5`
- 总入口：[Issue #199 多轮任务索引](README.md)

本文只保留尚未实施且仍会影响后续设计的用户决定、技术结论、暂缓范围和未回答问题。已经由源码、
自动化与独立 owner 执行文档承接的 Form vNext、Jaco Form consumer 和 Feiwen Query/Fetch/Catalog/DB/
Form 内容已从本草稿删除，不在这里维护第二份完成态说明。

保留规则：

1. 已回答但尚未实施的决定继续保留，直到对应 owner 计划实施并记录完成证据。
2. 未回答或只完成部分回答的问题必须保留原编号，不用实现偏好替代答案。
3. 明确暂缓的范围保留恢复入口，不提前设计或创建实施计划。
4. 已完成内容从草稿删除；其历史、契约和验证以 Git 及对应 owner 执行文档为准。

本文中的 `CONV-*`、`HTTP-*` 和 `NOVEL-*` 只是问题跟踪编号，不是 root plan 或 owner plan 的正式 ID。

## 2. 当前未完成范围

| 范围 | 状态 | 已确认方向 | 仍需处理 |
| --- | --- | --- | --- |
| Jaco Conversation active run | 已确认，尚未实施 | 共享 `Submitting / Running / Stopping` 私有 Transition；非法重入丢弃，不排队 | 建立 owner plan；闭环迟到 completion 防护 |
| Jaco MCP runtime | 暂缓 | 不开始 Transition / Store 迁移 | 等用户明确恢复任务 |
| HTTP Client | 全部未回答 | 尚无产品决定 | 请求运行、Form、Store 与 repair 问题 |
| Novel Download | 全部未回答 | 只确认不直接套用预定义 refresh / repair family | 并发、取消、文件提交、续传、Store 与 Form 问题 |

Issue 范围内继续有效的共通边界：

- `gpui-operation` 不新增第三个预定义 family；不符合 refresh / repair 的流程由应用定义自己的
  `Transition<Message>`。
- `gpui-store` 不新增公共 dispatch / message API；应用在 Store 所有的领域状态上运行 Transition，再用
  现有 `update` / `update_if` 发布。
- `Entity<Form<M>>` 不进入 Store，也不拥有业务 Task；`prepare` 只捕获验证后的 typed snapshot 与
  revision，I/O 和运行状态由应用 owner 管理。

## 3. Jaco Conversation active run

### 3.1 已确认的产品语义

#### CONV-01：继续现有运行行为

状态：**已确认，尚未实施**。

- 同一个 conversation 同时最多有一个 active run。
- 不同 conversation 可以并行运行。
- `Stopping` 仍属于 active run；清理完成前不能开始下一次运行。
- 保留现有取消、审批取消、运行完成与 conversation 刷新行为。

当前 `ConversationRuntimeStore` 使用
`HashMap<ConversationId, ActiveRun>` 保存运行，`ActiveRun` 持有 run key、cancellation token、
approval broker、主任务和事件任务（`app/jaco/src/features/conversation/runtime.rs:24-56`）。
`start_run` 发现同一 conversation 已有 entry 时会直接返回错误，不会排队或替换
（`app/jaco/src/features/conversation/runtime.rs:199-220`）。

#### CONV-02：`Stopping` 期间出现新消息时直接丢弃

状态：**已确认，尚未实施 Transition**。

- UI 必须继续阻止用户在 `Stopping` 时发送新消息。
- 将来迁入自定义 Transition 后，`Running` 或 `Stopping` 收到非法 `Start` / `Send` 是程序错误。
- Transition 应先恢复或保留原状态，再丢弃事件，并可写 debug / tracing 诊断。
- 不建立队列，不自动替换，不在停止完成后补发，也不把该事件转成正常重试。

当前 UI 已有两层 gate：聊天输入在 `Stopping` 时没有 submit / 主按钮动作，Conversation page 在写消息前
还会检查 `submission_pending || runtime.is_running()`
（`app/jaco/src/components/chat/input.rs:502-536`、
`app/jaco/src/components/chat/detail.rs:203-214`）。现有测试也覆盖了停止期间的 gate 与旧 completion
不得清理新状态。

#### CONV-03：区分 agent 业务失败与 owner 基础设施错误

状态：**已确认，尚未实施 Transition**。

agent/provider/tool 的业务性失败继续持久化到数据库，不属于 active-run Transition 需要额外保存的
业务错误。

| 错误类别 | 当前权威记录 | 与 active-run Transition 的关系 |
| --- | --- | --- |
| provider、tool、agent step 等业务运行失败 | `AgentRunStatus::Failed`、`AgentRunRecord.error` 和失败 timeline entry | 只结束 owner 生命周期；不复制业务错误历史 |
| 用户取消 | 持久化为 `AgentRunStatus::Canceled` | Stop 完成后刷新 conversation |
| runtime、数据库或 provider state 未 ready 等启动前错误 | owner 的一次性 `last_errors` / notification | 尚未创建可持久化 agent run，保留即时错误 |
| 最终持久化提交失败、Tokio/owner task 失败 | owner 的一次性 `last_errors` / notification | 无法可靠形成数据库终态，由 owner 报告 |
| Stop 收尾时查询或持久化失败 | owner 的一次性 `last_errors` / notification | 属于取消基础设施失败，不是 agent 业务失败 |

`AgentRuntime` 会把业务失败转换为 `AgentRunOutcome::Failed`，并在 final commit 中写入数据库
（`crates/jaco-agent/src/runtime.rs:527-594`、
`crates/jaco-agent/src/persistence.rs:66-95`）。`last_errors` 是
`ConversationRuntimeStore` 中按 conversation 保存、随后被页面 `take` 的一次性内存字符串
（`app/jaco/src/features/conversation/runtime.rs:24-30`、
`app/jaco/src/components/chat/detail.rs:276-294`）。

目标 Transition 只表达 active-run 生命周期和非法事件诊断，不新增 agent 错误持久化层。

#### CONV-04：不增加新的运行能力

状态：**已确认，尚未实施 Transition**。

- 不增加 pause / resume。
- 不增加后台继续运行语义。
- 不增加审批转交给其他 owner / window 的能力。
- 现有单次运行内部的 approval broker 继续保留；这里排除的是新的跨 owner 转交协议。

### 3.2 Transition 的目标约束

状态：**目标边界已确认，尚未形成 owner plan**。

Conversation 使用应用私有 `Transition<Message>` 统一提交准入和 active-run 生命周期，不套用
`refresh` / `repair` 预定义状态机。`ConversationRuntimeStore` 已经是同一 conversation 在各页面间的
共享 owner，继续由它持有这套 Transition；不把该状态迁入 `gpui-store`。

当前状态之所以不清晰，不是因为两个页面会在同一个同步状态上同时执行 `start_run`，而是因为
提交与运行存在一段异步边界：页面级 `submission_task` 只约束当前输入控制器，user entry 持久化完成后
才进入共享的 `start_run`。权威状态需要在不可逆写入前同步进入共享的 `Submitting`，让所有页面立即
看到同一个提交中状态。

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

`Submitting` / `Running` / `Stopping` 必须继续拥有或关联各自唯一的生命周期 Task。run key 目前用于拒绝
已经 detach 的 producer 或清理任务产生的迟到 completion；只有证明唯一 Task 所有权使迟到
completion 不可达后，才可删除等价 identity 防护
（`app/jaco/src/features/conversation/runtime.rs:490-554`）。

### 3.3 问题状态

#### CONV-Q01：运行准入必须发生在不可逆的用户消息写入之前

状态：**已确认，尚未实施**。

当前页面先执行 `send_conversation_message` 持久化 user entry，成功后才调用 `start_run`
（`app/jaco/src/components/chat/detail.rs:218-258`）。active run 虽然由共享的
`ConversationRuntimeStore` 管理，但当前提交中的 Task 属于页面输入控制器；另一个页面不能从它判断
该 conversation 正在写入 user entry。共享资源 owner 对 mutation task 的保留只保证 Task 生命周期，
也没有形成可供按钮读取的提交状态。

已确认方向是共享 `Submitting`：提交命令先同步让私有 Transition 接受 `Submit`，成功取得准入后才开始
不可逆的 user entry 持久化。这样所有页面会在写库前看到同一个状态；提交成功后进入 `Running`，失败
则释放回 `Idle`。非法的再次提交直接丢弃，不排队。

这不要求数据库写入和状态变化处于同一个数据库事务；共享准入状态必须先于不可逆写入建立。具体消息
enum、Task 返回值和错误传递由后续 owner plan 落实。

#### CONV-Q02：迟到 completion 的等价保护

状态：**待实现调研**。

需要确认未来所有 producer 是否都受运行态唯一 Task 的取消约束。若仍有 detached producer 或独立
cleanup completion，则必须保留 run key / generation；如果能证明它们不可在状态替换后投递，才可
依靠 Task drop 取消语义。

## 4. Jaco MCP runtime

状态：**暂缓**。

- 自定义 Transition 与 status Store 当前不继续调研、不建立 owner plan、不实施。
- Jaco Form consumer 已完成不代表 MCP 连接、OAuth、tool runtime 或运行状态已经迁移。
- 用户明确恢复该任务后，再从当前源码重新调研；本草稿不预先选择状态机或 Store owner。

## 5. HTTP Client：尚未回答的问题

状态：**全部未回答**。当前 Send runtime 仍未实现，因此本文只保存问题，不预建状态机。

### 5.1 请求运行与 Operation

- **HTTP-RUN-Q01：** `ResponseData` 的权威内容是什么：status、headers、timing、body、大小、截断和
  binary 表示分别如何建模。
- **HTTP-RUN-Q02：** HTTP 4xx / 5xx 是成功的 HTTP response Data，还是 RequestProblem。
- **HTTP-RUN-Q03：** resend 开始后是否保留旧 response；这会决定是否适用
  `refresh::Operation` 的 Refreshing / Degraded 语义。
- **HTTP-RUN-Q04：** 运行 owner 是单 request page、未来 request tab，还是共享 runtime；是否允许
  多 tab 并行。
- **HTTP-RUN-Q05：** Send 是否冻结不可变 prepared request snapshot，运行中编辑是否只影响下一次
  Send。
- **HTTP-RUN-Q06：** 取消、超时、重发和迟到 completion 的规则。
- **HTTP-RUN-Q07：** 非幂等请求的 Retry / Resend 是否需要显式确认。

### 5.2 RequestDraft Form

- **HTTP-FORM-Q01：** 支持哪些 URL scheme，以及相对 URL 是否依赖 environment base URL。
- **HTTP-FORM-Q02：** URL 与 Params 的唯一 source of truth；URL 无法解析时 Params 如何显示和编辑。
- **HTTP-FORM-Q03：** Params 修改后是否规范化或重写用户原始 URL 字符串。
- **HTTP-FORM-Q04：** empty / duplicate headers 的合法性、顺序和大小写保留规则。
- **HTTP-FORM-Q05：** body 类型与 Content-Type 的同步规则，以及重复 x-form key 的语义。
- **HTTP-FORM-Q06：** multipart FormData 尚未实现，未来文件字段、文本字段和重复 key 如何进入 Form。

### 5.3 Store 与 repair

- **HTTP-STORE-Q01：** history、favorites、environment、auth、cookie jar 分别是否跨 tab/window 共享，
  哪些需要 Store，哪些需要持久化服务。
- **HTTP-STORE-Q02：** 多 tab catalog、active tab 和 request identity 的 owner。
- **HTTP-STORE-Q03：** secret/auth/cookie 的内存与持久化安全边界；不能因为共享就直接放入普通 UI
  snapshot。
- **HTTP-REPAIR-Q01：** auth challenge、客户端证书、代理或 TLS 问题是否提供显式修复动作；在动作
  未定义前不采用预定义 `repair::Operation`。

## 6. Novel Download：尚未回答的问题

状态：**全部未回答**。当前只确认下载流程不适用预定义 refresh / repair family。

### 6.1 并发、取消与运行 owner

- **NOVEL-RUN-Q01：** 单 active download、取消并替换、队列，还是多个并行下载。
- **NOVEL-RUN-Q02：** Start 发生在已有运行时应拒绝、替换还是排队。
- **NOVEL-RUN-Q03：** 取消检查点在哪里；网络请求、解析和文件写入分别如何停止。
- **NOVEL-RUN-Q04：** 页面关闭或应用退出时取消、等待 drain，还是允许后台继续。
- **NOVEL-RUN-Q05：** 是否需要稳定 `DownloadId`、不可变 request snapshot 和 generation 防止迟到
  completion。
- **NOVEL-RUN-Q06：** 如果采用队列，最大并发、去重和相同 URL 的身份规则。

### 6.2 文件提交、续传与 repair

- **NOVEL-FILE-Q01：** 下载的 commit point 是整本文件、章节、页，还是更细粒度。
- **NOVEL-FILE-Q02：** 取消/失败后的部分文件保留、删除，还是使用 `.part` staging。
- **NOVEL-FILE-Q03：** 重试如何保证不会把已经 append 的内容重复写入。
- **NOVEL-FILE-Q04：** checkpoint 与 resume 粒度；远端来源发生变化时如何识别旧 checkpoint 已失效。
- **NOVEL-FILE-Q05：** 用户修改目标文件、目标名称冲突和 overwrite 策略。
- **NOVEL-REPAIR-Q01：** 更换目录、覆盖、删除部分文件后重试、从 checkpoint 继续等动作中，哪些是
  正式 repair。

### 6.3 Preview、Store 与 Form

- **NOVEL-PREVIEW-Q01：** 是否先展示元数据 preview；preview 包含哪些字段、以什么 URL / source
  identity 绑定。
- **NOVEL-PREVIEW-Q02：** preview 重读时是否保留旧数据，以及如何拒绝旧 URL 的迟到 completion。
- **NOVEL-STORE-Q01：** 是否形成后台下载中心、队列、历史、跨窗口详情；只有出现多个独立消费者时
  才建立 `Store<DownloadCenterState>`。
- **NOVEL-FORM-Q01：** 何时从当前单 URL 输入升级为包含来源、目录、章节/页范围、覆盖和续传策略的
  `DownloadRequestForm`。
- **NOVEL-RETRY-Q01：** 当前单页网络 retry 继续保持 runner 内部控制流，还是有某类失败需要暴露给
  UI；不能为每页短暂重试建立 Operation。

## 7. 后续入口

1. Jaco Conversation 下一步先建立独立 owner plan，并在实现前闭环 `CONV-Q02`。
2. Jaco MCP runtime 只有在用户明确恢复后才重新调研。
3. HTTP Client 与 Novel Download 在用户回答对应问题前，不预先选择产品语义或建立实施计划。
4. Form 来源感知控件投影继续在独立 Form 草稿中讨论，不在本文件复制。
