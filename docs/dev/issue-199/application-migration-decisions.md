# Issue #199：应用迁移决策与后续调研草稿

## 1. 文档状态与使用方式

- 状态：`Draft`。
- 整理日期：`2026-08-04`。
- 应用源码调研基线：`codex/199-adopt-gpui-store-form-operation` 分支，commit `24d4249`。
- 上游调研：[Workspace Store、Operation 与 Form 适用性调研](workspace-store-operation-form-assessment.md)。
- Form vNext 设计依据：[gpui-form 设计草稿](../../../crates/gpui-form/docs/dev/issue-199/design-draft.md)。
- 本文只记录用户已经确认的产品语义、基于当前源码得到的技术结论，以及仍未回答的问题。
- 本文不是实施计划，不分配 work package，不确认最终类型名，也不授权修改应用代码。
- 本文中的 `CONV-*`、`FEI-*`、`HTTP-*`、`NOVEL-*` 只是问题跟踪编号，不是 Issue #199
  root plan 或 owner plan 的正式 ID。

本文使用四种状态：

| 状态 | 含义 |
| --- | --- |
| **已确认** | 用户已经明确决定，后续设计不得自行改变 |
| **调研结论** | 当前源码支持的技术判断，可继续接受用户审阅 |
| **未回答** | 仍需用户决定；必须保留，不得用实现偏好替代 |
| **暂缓** | 用户明确要求当前不继续设计或实施 |

问题与决策的保留规则：

1. 一个问题编号一旦进入本文，就不得因为已经回答而删除或重新编号；只把状态改为“已确认”，并在原编号
   下保存会约束实现的最终答案。
2. 部分回答的问题继续保留原编号，明确拆开“已确认部分”和“仍未回答部分”；不得用章节摘要代替原答案。
3. 如果后续决定替代旧决定，旧编号仍保留并标记“已取代”，同时引用新的权威编号；不得无痕改写历史
   约束。
4. 可以删除的只有不再影响当前设计的讨论过程、重复解释和已失效备选方案；用户确认的产品语义、范围
   边界、拒绝方案和恢复行为都属于当前设计输入，必须保留。

本文另外用以下三种标签说明新 Form vNext 对某个问题的影响；它们不替代上面的产品决策状态：

| Form 影响 | 含义 |
| --- | --- |
| **Form 设计已覆盖** | Form 库层的模型、路径或生命周期边界已经选定，不再作为应用问题重复讨论 |
| **Form 仅提供原语** | Form 能表达所需动作，但动作时机、产品语义、UI、I/O 与运行 owner 仍由应用决定 |
| **与 Form 无关** | 问题属于 Transition、Store、Task、数据库、持久化或产品策略，Form API 不会解决 |

这里的“设计已覆盖”表示 greenfield vNext 已经给出库层契约；截至 2026-08-05，Form vNext 与本轮
Feiwen owner plan 已实施并通过自动化。本文仍保留实施前源码发现作为决策依据，当前交付状态以本节
总览和对应 owner plan 为准。

本文对上游调研中的以下初步判断作了更新：

1. Feiwen 查询不再采用预定义 `refresh::Operation`，也不保留旧结果；改为应用私有
   `Transition<Message>`。
2. Feiwen 抓取在运行期间允许继续编辑 Form，但本次运行和后续修复必须使用提交时冻结的快照。
3. Feiwen `QueryCatalog` 不是“产品语义未定”；调研时 owner 尚未拆开，本轮已经落成
   application-global Operation Store。
4. Feiwen 数据库恢复不再只作为笼统的“以后再考虑 repair”；继续参考 Jaco 已实施的数据库资源
   生命周期，但必须为 DuckDB 与 r2d2 重新设计具体边界。

## 2. 当前决策总览

| 表面 | 当前状态 | 已确认方向 | 仍需继续调研或回答 |
| --- | --- | --- | --- |
| Jaco Conversation 提交与 active run | 已确认（待 owner plan） | 私有 Transition；共享 `Submitting / Running / Stopping`；非法重入丢弃，不排队 | 迟到 completion 防护 |
| Jaco MCP runtime | 暂缓 | 不开始 Transition / Store 迁移 | 等用户重新恢复该任务 |
| Feiwen 查询 | 已确认并实施 | 私有 Transition；完整 draft 与执行 spec 快照；编辑器与表单 Reset 保持可用；查询按钮禁用并提供专门取消；失败时显示原因并可将快照载入 Form | 唯一父 Task completion route 与非法事件丢弃已由自动化覆盖 |
| Feiwen 抓取 | 已确认并实施 | Form 可编辑但禁止再次开始；Fresh 清空旧状态；修复固定原快照；快照可整体载入 Form；不保留运行历史或并存 run | 唯一 Task、无 detached producer / RunId 已落地 |
| Feiwen QueryCatalog | 已确认并实施 | tags/authors 使用全局 refresh Operation Store；缺失值保留且可提交；非 Ready 时禁用查询按钮和 catalog 相关控件；Reset 只重置 Form；启动后立即加载；以 invalidation generation 合并刷新 | phase UI、显式重新加载、generation 消费与失败后不自动重试已落地 |
| Feiwen 数据库 | 已确认并实施 | 修复动作只在 `Unavailable` 显示；“备份后重建”必须二次确认；本轮无可达 `Retiring`；无跨进程租约；修复后不自动刷新 consumer | DuckDB 主文件+WAL备份、同目录staging、替换、隔离与失败回滚已通过failure-injection tests |
| Feiwen 高级查询 Form | 已确认并实施 | `QueryDraft` 使用 recursive typed tree；field type 改变时清空旧 relation/value；relation 改变不修改 field-owned value；错误只在对应字段展示且不汇总；动态 item identity 由 Form runtime 生成并以 typed `ItemPath` 暴露，业务 model 不保存纯表单 ID | fallible resolver、topology mutation、adapter、missing catalog value与按 `PathKey` 复用projection已落地 |
| HTTP Client | 未回答 | 尚无新的用户决定 | 请求运行、Form、Store 与 repair 全部保留 |
| Novel Download | 未回答 | 尚无新的用户决定 | 并发、取消、提交点、续传、Store 与 Form 全部保留 |

### 2.1 新 Form vNext 已经解决或收束的问题

#### 2.1.1 已在 Form 库设计层关闭

| 原问题 | 结论 | 仍需做什么 |
| --- | --- | --- |
| `FEI-ADVFORM-Q05`：递归异构树如何映射 total / partial field | **Form 设计与精确定位API均已确认**。vNext 不再让应用判断 `FormField` / `PartialFormField`；静态路径是 `TotalPath`，通过runtime返回的typed `ItemPath`进入动态item。case/optional在当前session中通过 `.try_case(&Form, CaseDef)` / `.try_some(&Form)` 捕获incarnation，返回的path不持有entity。最终值类型仍保留为Rust `T`。 | Form core已按 `D-505/D-506` 实现并验证；Feiwen只消费typed path，不接触private topology snapshot。 |
| `FEI-ADVFORM-Q02` 的库机制部分：动态 item 如何保持稳定定位 | **Form 设计已覆盖并确认 ownership**。动态 item identity 由 Form runtime 在 session 内生成和维护；业务 model 不持有纯表单 ID。collection enumeration 与 topology mutation 返回 typed opaque `ItemPath<Root, Item>`，调用方不使用 raw ID 或 index 构造路径。 | Form 已实现 `ItemPath`、collection mutation、路径失效与 adapter UI-key 契约；Feiwen 已直接消费该契约，不再选择 `NodeId`。 |
| `FEI-ADVFORM-Q07` 的库机制部分：嵌套错误如何定位 | **Form 设计已覆盖，Feiwen 展示语义已确认**。validator 通过 typed/canonical path 记录 issue，并按动态 item、case、incarnation 与 generation 防止错误贴到已删除或重建的节点。 | adapter 已只在对应字段旁渲染该字段错误；页面不生成汇总错误区域。 |
| Query / Fetch 的“编辑草稿与运行状态混在一起” | **Form 设计已覆盖**。`Entity<Form<M>>` 只拥有编辑 session；`prepare` 原子捕获 model snapshot 与 revision；业务 Task、progress、日志、运行错误和 retry 不进入 Form。 | Query / Fetch owner 已按各自 Transition 和 Store 契约管理运行。 |

#### 2.1.2 Form vNext 目标设计只提供原语或边界，没有替应用作决定

| 问题 | Form vNext 原语 | 应用仍然负责 |
| --- | --- | --- |
| `FEI-QUERY-Q01`、`FEI-FETCH-01/02`：运行中继续编辑且本轮请求冻结 | `prepare` + `Prepared<M>::map` 生成不可变 typed snapshot；Form 本身没有业务 busy 状态。 | 何时准入运行、按钮状态、运行 snapshot 的 owner、Task 与 completion。 |
| `FEI-QUERY-Q03`：运行中重置下一次查询草稿 | `reset` 只变更 Form session。 | 标题栏 Reset 只调用表单动作且不取消、清空或替换当前 Query run。 |
| `FEI-QUERY-Q05`、`FEI-FETCH-Q04`：把失败/运行快照整体载入表单 | 显式 whole-model `replace`；运行完成不会自动回写 Form。 | Query直接保留完整 `QueryDraft`，Fetch负责 `FetchRequest` 到 draft 的转换；按钮、覆盖提示、snapshot UI 与 Cookie 脱敏仍归应用。 |
| `FEI-FETCH-Q05`：成功后保留用户正在编辑的草稿 | Form 不观察业务成功，也不会自动 `replace`、`reset` 或 `rebase`。 | Fetch owner 不得在成功 completion 中主动修改 Form。 |
| `FEI-CATALOG-Q02`、`FEI-ADVFORM-Q06`：options 变化与已有值 | catalog/options 不属于 Form；owner 显式替换 validator 并触发 dynamic validation，更新 options 不会隐式清值或选择 fallback。 | 已确认显示非阻塞的“当前目录中不存在”，保留 typed value 并允许在 catalog 精确 Ready 时提交；native options 同步采用 `FEI-CATALOG-Q01` 的 Jaco 模式。 |
| `HTTP-RUN-Q05` 与 `HTTP-FORM-Q01..Q06` | vNext 能承载 typed/nested request draft；动态 collection item 由 Form runtime 定位，并通过 `prepare` 冻结请求。 | HTTP 的 URL、Params、header、body、multipart 与运行产品语义仍全部未回答。 |
| `NOVEL-FORM-Q01` | 若决定建立 `DownloadRequestForm`，vNext 能承载 typed draft、validation 与 prepared snapshot。 | 是否升级、何时升级、字段组成与下载提交语义。 |

#### 2.1.3 不会由 Form 解决的问题

- Conversation 的共享 `Submitting / Running / Stopping`、写库前准入、唯一 Task 与迟到 completion。
- Query / Fetch 的结果清理、运行 Transition、取消、Resume / Retry、日志、进度、运行历史和唯一 run。
- QueryCatalog 的加载/刷新/降级状态、invalidation 合并、加载时机与 SQL 语义。
- Feiwen 数据库的 Ready gate、备份、重建、替换和失败回滚；本轮没有可达的 `Retiring` 状态。
- Feiwen 已选择 recursive typed tree；Form 不会替应用决定字段类型或 relation 切换时的业务数据策略。
- HTTP Client 与 Novel Download 的运行、持久化、并发、取消、Store 和 repair 产品语义。

### 2.2 Feiwen 问题保留索引

本表只用于快速核对状态；每个问题的完整答案和剩余边界仍以对应章节的原编号为准。这里的“技术待调研”
表示产品语义已经足够明确，但 owner plan 仍要固定具体类型、消息、Task 或文件协议，不需要用户重复回答
已经确认的产品问题。

| 范围 | 已回答或已关闭，原编号继续保留 | 仍需用户回答 | 仅技术或 owner plan 待调研 |
| --- | --- | --- | --- |
| Query | `FEI-QUERY-01/02`、`FEI-QUERY-Q01..Q05` 全部保留且产品语义已确认 | 无 | 已实施并完成自动化验证；实际 UI 操作测试未执行 |
| Fetch | `FEI-FETCH-01..03`、`FEI-FETCH-Q01..Q07` 全部保留且已确认 | 无 | 已实施并完成自动化验证；实际 UI 操作测试未执行 |
| QueryCatalog | `FEI-CATALOG-01/02`、`FEI-CATALOG-Q01..Q06` 全部保留且产品语义已确认；`Q07/Q08` 已形成调研结论 | 无 | 已实施并完成自动化验证；实际 UI 操作测试未执行 |
| 数据库资源 | `FEI-DB-01`、`FEI-DB-Q01..Q07` 全部保留且产品语义已确认 | 无 | 主文件+WAL、staging、替换、rollback与failure injection已实施验证 |
| 高级查询 Form | `FEI-ADVFORM-Q01..Q07` 全部保留且产品语义已确认；`Q05` 已由 Form 精确resolver关闭，`Q06` 已复用 Catalog Q02 关闭 | 无 | `ItemPath`、mutation、validation、adapter与recursive projection已实施验证 |

## 3. Jaco Conversation active run

### 3.1 已确认的产品语义

#### CONV-01：继续现有运行行为

状态：**已确认**。

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

状态：**已确认**。

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

状态：**已确认并完成源码调研**。

用户的判断正确：agent/provider/tool 的业务性失败应持久化到数据库，不属于 active-run
Transition 需要额外保存的业务错误。

当前错误边界如下：

| 错误类别 | 当前权威记录 | 与 active-run Transition 的关系 |
| --- | --- | --- |
| provider、tool、agent step 等业务运行失败 | `AgentRunStatus::Failed`、`AgentRunRecord.error` 和失败 timeline entry | 只结束 owner 生命周期；不再复制一份业务错误历史 |
| 用户取消 | 持久化为 `AgentRunStatus::Canceled` | Stop 完成后刷新 conversation |
| runtime 未 ready、数据库/provider state 未 ready等启动前错误 | owner 的一次性 `last_errors` / notification | 还没有创建可持久化的 agent run，需要继续保留当前即时错误 |
| 最终持久化提交失败、Tokio/owner task 失败 | owner 的一次性 `last_errors` / notification | 无法可靠形成数据库终态，需要由 owner 报告 |
| Stop 收尾时查询或持久化失败 | owner 的一次性 `last_errors` / notification | 属于取消基础设施失败，不是 agent 业务失败 |

`AgentRuntime` 会把业务失败转换为 `AgentRunOutcome::Failed`，并在 final commit 中写入数据库
（`crates/jaco-agent/src/runtime.rs:527-594`、
`crates/jaco-agent/src/persistence.rs:66-95`）。`last_errors` 则是
`ConversationRuntimeStore` 中按 conversation 保存、随后被页面 `take` 的一次性内存字符串
（`app/jaco/src/features/conversation/runtime.rs:24-30`、
`app/jaco/src/components/chat/detail.rs:276-294`）。

因此目标 Transition 只表达 active-run 生命周期和非法事件诊断，不新增 agent 错误持久化层。

#### CONV-04：不增加新的运行能力

状态：**已确认**。

- 不增加 pause / resume。
- 不增加后台继续运行语义。
- 不增加审批转交给其他 owner / window 的能力。
- 现有单次运行内部的 approval broker 继续保留；这里排除的是新的跨 owner 转交协议。

### 3.2 Transition 的目标约束

状态：**目标边界已确认，尚未形成 owner plan**。

Conversation 使用应用私有 `Transition<Message>` 统一提交准入和 active-run 生命周期，不套用
`refresh` / `repair` 预定义状态机。`ConversationRuntimeStore` 已经是同一 conversation 在各页面间的
共享 owner，继续由它持有这套 Transition；本轮不把该状态迁入 `gpui-store`。

当前状态之所以不清晰，不是因为两个页面会在同一个同步状态上同时执行 `start_run`，而是因为
提交与运行存在一段异步边界：页面级 `submission_task` 只约束当前输入控制器，user entry 持久化完成后
才进入共享的 `start_run`。因此，权威状态需要在不可逆写入前同步进入共享的 `Submitting`，让所有页面
立即看到同一个提交中状态。

自定义 Transition 至少需要集中表达以下事实（消息名仅表示语义，不在本文确定最终 API）：

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

目标行为约束如下：

- `Submitting` 必须在调用 user entry 持久化之前同步建立，并持有本次提交生命周期 Task。
- 所有引用同一 `ConversationRuntimeStore` 的页面都从同一状态投影按钮；`Submitting` 时显示加载且禁止
  再次发送，不提供队列、替换或取消。
- 提交成功后进入 `Running`；提交或启动失败后回到 `Idle`，继续沿用现有即时错误通知。
- 页面局部 `submission_task` 不再是权威准入状态；实施时应由共享 Transition 的 Task 所有权替代。
- `Submitting`、`Running` 或 `Stopping` 收到不合法的再次提交都视为程序错误，保留当前状态、记录诊断
  并丢弃事件。
- agent/provider/tool 业务错误的数据库持久化仍遵循 `CONV-03`，不并入这套生命周期错误状态。

`Submitting` / `Running` / `Stopping` 必须继续拥有或关联各自唯一的生命周期 Task。run key 目前用于拒绝已经
detach 的 producer 或清理任务产生的迟到 completion；迁移时只有在证明唯一 Task 所有权使迟到
completion 不可达之后，才可以删除等价的 identity 防护
（`app/jaco/src/features/conversation/runtime.rs:490-554`）。

### 3.3 问题状态

#### CONV-Q01：运行准入必须发生在不可逆的用户消息写入之前

状态：**已确认**。

当前页面先执行 `send_conversation_message` 持久化 user entry，成功后才调用 `start_run`
（`app/jaco/src/components/chat/detail.rs:218-258`）。active run 虽然由共享的
`ConversationRuntimeStore` 管理，但当前提交中的 Task 属于页面输入控制器；另一个页面不能从它判断
该 conversation 正在写入 user entry。共享资源 owner 对 mutation task 的保留只保证 Task 生命周期，
也没有形成可供按钮读取的提交状态。

已确认的解决方向是 `3.2` 中的共享 `Submitting`：提交命令先同步让私有 Transition 接受
`Submit`，成功取得准入后才开始不可逆的 user entry 持久化。这样所有页面会在写库前看到同一个状态；
提交成功后进入 `Running`，失败则释放回 `Idle`。非法的再次提交直接丢弃，不排队。

这项结论不要求把数据库写入和状态变化做成同一个数据库事务；需要保证的是，共享准入状态必须先于
不可逆写入建立。具体消息 enum、Task 返回值和错误传递由后续 owner plan 落实。

#### CONV-Q02：迟到 completion 的等价保护

状态：**待实现调研**。

需要确认未来所有 producer 是否都受运行态唯一 Task 的取消约束。若仍有 detached producer 或独立
cleanup completion，则必须保留 run key / generation；如果能证明它们不可在状态替换后投递，才可
依靠 Task drop 取消语义。

## 4. Feiwen 查询

### 4.1 已确认的产品语义

#### FEI-QUERY-01：每次查询都不保留上一次结果

状态：**已确认**。

- 用户点击查询时已经明确预期要查看新结果。
- 无论查询条件是否改变，只要本次查询开始，就立即清除上次表格结果。
- 运行期间不显示 stale table。
- 查询失败后不显示表格，只显示错误。

当前校验失败和运行失败都会清空表格，但异步查询开始时只设置 loading，没有清掉旧 rows
（`app/feiwen/src/features/query.rs:234-321`）。后续迁移必须补齐“开始即清空”。

#### FEI-QUERY-02：使用 Feiwen 私有状态机

状态：**已确认**。

- 查询执行不使用预定义 `refresh::Operation`，因为 `Ready / Degraded` 保留旧 Data 的语义与产品决定
  冲突。
- 查询执行也不使用预定义 `repair::Operation`；查询按钮本身就是重新执行当前查询的入口，不需要再
  发明独立 Refresh / Repair 产品动作。
- 使用 Feiwen 私有 `Transition<QueryMessage>` 集中 Start / Complete 的合法状态变化。

### 4.2 建议的运行状态边界

状态：**调研结论，类型名仍是草稿**。

```rust
enum QueryRun {
    Idle,
    Running {
        snapshot: QueryDraft,
        task: Task<()>,
    },
    Succeeded { count: usize },
    Failed {
        snapshot: QueryDraft,
        problem: QueryProblem,
    },
}

enum QueryMessage {
    ClearTerminal,
    Start {
        snapshot: QueryDraft,
        task: Task<()>,
    },
    Complete(Result<QueryResult, QueryProblem>),
    Cancel,
}
```

建议语义：

1. `Start` 只从非运行态合法；先清空结果，再安装 `Running`。
2. `Complete(Ok)` 从 `Running` 进入 `Succeeded`，把本次 `Vec<Novel>` 投影给 table delegate。
3. `Complete(Err)` 从 `Running` 进入 `Failed`；结果保持为空，页面只渲染错误区域。
4. `Running` 收到第二个 `Start` 或不属于当前运行的 completion 时保留原状态、记录程序错误并丢弃。
5. `QueryView` 继续是 I/O owner：准备 `QuerySpec`、获取 DB resource、构造 Task、投递 completion 和
   更新 `ResultsTableDelegate`。
6. 查询结果是 QueryView 的单次运行上下文，不迁入 application-global Store；table delegate
   只保留排序/渲染需要的页面投影。
7. `Running` 保留提交时的完整 `QueryDraft` snapshot；唯一父Task闭包持有从同一 prepared draft 编译的
   `QuerySpec`，运行期间对高级查询编辑器的修改只影响下一次提交。
8. runtime 失败时把同一 snapshot 移入 `Failed`，供错误区域的“恢复到表单”动作使用。
9. `Cancel` 只通过独立取消按钮触发，不复用查询或重置按钮；Task 取消和迟到 completion 的具体边界
   留给 owner plan 调研。

输入校验应在提交前由查询编辑模型或未来 Form 处理。校验失败不进入 runtime `QueryProblem`，但也应
清掉此前结果，避免用户误以为旧表格对应当前输入。

### 4.3 问题状态

#### FEI-QUERY-Q01：运行中是否允许编辑高级查询

状态：**已确认**。

Form 影响：**Form 仅提供原语**。vNext 的 `prepare` 会原子捕获本轮 model snapshot 与 revision，Form
本身不拥有查询 Task 或 busy 状态；是否保持编辑器可用仍是本节已经确认的应用行为。

- 运行期间不禁用高级查询编辑器。
- 当前查询已经在启动前冻结独立 `QueryDraft`，并生成只归本次Task使用的 `QuerySpec`；运行期间的编辑
  不会改变本次请求。
- 后续编辑只参与下一次查询提交。

实施前实现虽然已经把 `QuerySpec` 移入后台查询，但仍在开始和结束时调用
`advanced.set_disabled(true / false)`（`app/feiwen/src/features/query.rs:240-303`）。迁移时应移除这层
运行期禁用，并由运行态明确保留 snapshot。

#### FEI-QUERY-Q02：运行中如何处理查询按钮

状态：**已确认**。

- 查询按钮在运行期间保持禁用，并继续显示运行中状态。
- 取消使用独立按钮，不复用查询按钮。
- UI 不会正常投递第二个 `Start`；如果程序错误仍投递，Transition 保留 `Running`、记录诊断并丢弃，
  不排队。

#### FEI-QUERY-Q03：运行中如何处理“重置”按钮

状态：**已确认**。

Form 影响：**Form 仅提供原语**。vNext 的 `reset` 可以只重置编辑 session；标题栏事件仍需由 Query
owner 拆开，确保它不同时修改 Query Transition 或结果状态。

这里的 Reset 指标题栏原有的“重置”按钮，不是取消运行，也不是重置 Transition。实施前按钮在查询期间
被禁用；即使从其他代码路径投递 `QueryEvent::Reset`，事件处理也会在 `is_searching()` 时直接忽略
（`app/feiwen/src/app/titlebar.rs:103-119`、`app/feiwen/src/features/query.rs:143-187`）。

非运行时点击该按钮，会重新从数据库加载 `QueryOptions`、重建整个 `AdvancedQueryState`、清空表格并
回到 `Init`。已确认的目标行为是：

- 运行期间允许点击“重置”，因为它重置的是当前可编辑的高级查询表单。
- Reset 只把“下一次查询草稿”恢复为默认状态，不取消当前查询，也不修改正在运行的 snapshot。
- Reset 不把 runtime 从 `Running` 改回 `Idle`，不丢弃本次运行的 Task；当前查询仍按原 snapshot 完成。
- 迁移时需要把“重置表单”和当前附带的“清空结果、重置查询运行状态”拆开，避免表单动作破坏运行态。

#### FEI-QUERY-Q04：是否提供显式取消查询

状态：**已确认并实施自动化验证**。

- 提供独立的取消查询按钮。
- 实施前没有该按钮：`QueryEvent` 只有 `Search / Reset`，错误区域也只渲染 `Alert`，没有 Cancel 路径
  （`app/feiwen/src/features/query.rs:69-72`、`341-350`）。
- 取消按钮不承担 Reset 或恢复 Form 的职责。
- owner plan固定 `Running` 持有唯一父Task；DuckDB background work不能直接回写View，只能由该父Task
  await后投递completion。Cancel drop父Task并回到Idle；非法late completion保留当前state并记录bug。
  这条唯一completion route通过定向test证明后，本轮不增加QueryRunId或completion队列。

#### FEI-QUERY-Q05：runtime 失败如何呈现和恢复快照

状态：**已确认**。

Form 影响：**Form 仅提供原语**。vNext 的 whole-model `replace` 已覆盖“用一个 draft 覆盖当前编辑
session”的库动作；完整 Query draft snapshot、错误区域和触发按钮仍由 Feiwen 实现。

已确认：

- 查询失败后不显示表格，只显示本次失败的错误原因。
- 错误区域提供一个动作，把本次失败运行冻结的完整 `QueryDraft` snapshot 设置回高级查询表单。
- 该动作会用失败 snapshot 覆盖用户在运行期间继续编辑的当前草稿；它不恢复失败前的旧查询结果。
- 因此 `Failed` 必须保留完整 draft snapshot；恢复不能从 `QuerySpec` 反向猜测，否则 relation 当前未使用
  的number/author operand会丢失。
- 不提供“复制错误详情”按钮；错误区域除错误原因和“载入表单”外，不增加另一套错误详情交互。

实施前实现只显示 `Alert` 文本，还没有恢复 snapshot 的按钮
（`app/feiwen/src/features/query.rs:341-350`）。

## 5. Feiwen 抓取 Form、Transition 与 Store

### 5.1 已确认的产品语义

#### FEI-FETCH-01：运行期间允许编辑 Form

状态：**已确认**。

- URL、开始页、结束页、Cookie 组成“下一次提交”的可编辑草稿。
- 当前抓取运行期间继续允许用户编辑这些字段。
- 编辑当前草稿不得改变已经运行或已经失败的抓取请求。

当前 `FetchTaskState` 把四个草稿字段、status、logs 和 Task 混在同一个 Entity，并在运行中禁用输入
（`app/feiwen/src/features/fetch.rs:97-130`、`898-1015`）。该结构无法正确支持已确认行为。

#### FEI-FETCH-02：运行请求必须冻结为不可变快照

状态：**已确认**。

- Fresh start 只能使用经过验证的 Form submit 结果创建一个不可变 `FetchRequest` snapshot。
- progress、日志、成功、失败、中断都关联同一 snapshot。
- Form 在运行期间的后续编辑只影响下一次 Fresh start。

#### FEI-FETCH-03：修复从失败位置继续，并使用原运行快照

状态：**已确认**。

- 失败后的修复从失败页重新开始。
- 中断后的继续从上次成功页的下一页开始。
- URL、Cookie 和页码边界继续使用发生失败/中断时保存的 snapshot，不重新读取当前 Form。
- UI 必须显示这次运行/修复实际使用的 snapshot，否则用户无法判断当前运行上下文。

这里的“修复”先作为抓取领域动作记录。由于抓取包含逐页进度、部分数据库提交、中断和续跑，技术上
仍建议由私有 `FetchMessage::RetryFailed` / `Resume` 表达，而不是把整个流程塞进预定义
`repair::Operation`。

### 5.2 建议的所有权拆分

状态：**调研结论，类型名仍是草稿**。

| 权威事实 | 建议 owner | 生命周期 |
| --- | --- | --- |
| 当前可编辑 URL / 页码 / Cookie 与同步校验 | `Entity<Form<FetchDraft>>` | FetchView 编辑会话 |
| 已提交的 `FetchRequest` snapshot | `FetchRun` 各运行/终态 | 单次运行与后续修复 |
| phase、进度、日志、失败详情与生命周期 Task | workspace-owned `Store<FetchRun>` | workspace / 抓取运行 |
| DB Ready / Unavailable 与连接准入 | 全局数据库资源 Store | application |
| tags / authors 目录 | 全局 QueryCatalog Operation Store | application |

Form 模型只保存纯草稿，不保存 Task、progress、错误、日志、Subscription 或控件 Entity：

```rust
struct FetchDraft {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
}
```

`FetchView` 强持有 `Entity<Form<FetchDraft>>`；`prepare` 在同一 snapshot 上完成 submit validation，
再由 `Prepared<FetchDraft>::map` 生成 `FetchRequest`。运行失败不得自动 rebase 或覆盖 Form，因为用户
可能已经开始编辑下一次请求。

建议的抓取运行状态：

```rust
enum FetchRun {
    Idle,
    Running { snapshot, progress, logs, task },
    Interrupted { snapshot, progress, logs },
    Failed { snapshot, progress, logs, failure },
    Succeeded { snapshot, progress, logs },
}
```

`FetchRun` 本身就是当前抓取的唯一权威状态；同一时刻只能处于其中一个 variant，不再建立并行的状态
副本或运行历史。新的 Fresh run 直接覆盖上一个终态，Resume / Retry 则仍属于原 snapshot 的同一轮
生命周期。

建议消息包括 `StartFresh`、`Interrupt`、`Resume`、`RetryFailed`、`PageStarted`、
`PageSucceeded`、`Failed` 和 `Succeeded`。Transition 只负责状态归约；网络、数据库写入和 catalog
失效通知仍由应用 owner 执行。运行态是生命周期 Task 的唯一 owner，替换为 `Interrupted` 时通过
Task drop 取消当前 runner。

### 5.3 运行快照 UI

状态：**已确认**。

UI 至少要同时区分：

1. “当前编辑草稿”：用户下一次 Fresh start 将提交的字段。
2. “当前运行 / 待修复请求”：保存的 URL、原始页码范围、当前页、失败页或下一续跑页，以及 Cookie
   是否已设置。

Cookie 是凭据，snapshot 展示只提供“已设置/未设置”等脱敏摘要，不单独显示、临时揭示或复制原文。
只要当前状态包含 snapshot，快照区域就提供一个“载入表单”动作，把 URL、开始页、结束页和 Cookie
作为一个完整 `FetchDraft` 设置回 Form。这里的“复制回表单”不是写入系统剪贴板；它是用户显式发起的
whole-model Form 替换，会覆盖当前可编辑草稿，但不会改变正在运行或等待修复的 snapshot。

### 5.4 与数据库提交和 QueryCatalog 的关系

状态：**调研结论**。

当前抓取会逐页写入数据库，而且每个 `Novel::save` 都使用独立事务。同一页前几本小说已经提交后，
后续小说保存失败也不会回滚这些提交；已成功页更不会因为后续页失败或中断而整体回滚
（`app/feiwen/src/features/fetch.rs:405-466`、
`app/feiwen/src/store/service/novel.rs:43-115`）。

因此 catalog 失效不能只绑定抓取最终 `Succeeded`。采用以下 generation 契约：

1. 每次会影响 tags/authors 的事务成功提交后，都必须让 QueryCatalog owner 得知 catalog 已失效；
   owner 递增 `invalidation_generation`。如果以后把一整页改为单个事务，则改为在页事务提交后只递增
   一次。
2. 失效通知不能直接为每本 novel 启动一个 Refresh。连续通知只推进 generation；空闲时最多启动一个
   catalog read run。
3. 每次 Load / Refresh / Retry 启动时捕获当前 generation 作为本轮 `target_generation`。运行期间发生的
   新提交继续推进 generation，不计入本轮已经覆盖的范围。
4. 本轮成功安装新 catalog Data 后，`covered_generation` 才推进到本轮 `target_generation`。如果此时
   `invalidation_generation > covered_generation`，并且 DB 仍为 `Ready`、catalog 没有 active Task，
   owner 再合并启动一次 Refresh。
5. 失败、中断和取消都不得推进 `covered_generation`。尚未覆盖的失效继续保留；如果 Operation 已进入
   非 Ready phase，由用户显式发起的 Retry / Load / Refresh 消费，不从非法 phase 自动伪造消息。

### 5.5 问题状态

#### FEI-FETCH-Q01：运行中是否允许再次开始 Fresh run

状态：**已确认**。

- 运行期间的 Fresh start 按钮不可点击。
- 如果程序错误仍投递 `StartFresh`，Transition 保留 `Running` 并拒绝该消息。
- 不替换、不排队，也不并行启动第二个抓取。

当前 UI 已按 `is_running()` 禁用开始按钮，运行入口也会再次检查并忽略请求
（`app/feiwen/src/app/titlebar.rs:138-153`、`app/feiwen/src/features/fetch.rs:708-743`）。目标设计
保留这两层行为，但 Form 字段本身按照 `FEI-FETCH-01` 继续可编辑。

#### FEI-FETCH-Q02：Fresh start 如何处理上一轮日志

状态：**已确认**。

- 新的 Fresh run 清空上一轮 `Interrupted / Failed / Succeeded` 的日志和终态；保留它们没有意义。
- Resume / Retry 是原 snapshot 的同一轮生命周期，继续保留并更新本轮日志。
- 不为被覆盖的运行建立历史记录。

当前 Fresh 已通过 `clear_logs = true` 清空日志；Resume / Retry 不清空
（`app/feiwen/src/features/fetch.rs:204-230`、`734-823`）。

#### FEI-FETCH-Q03：Resume / Retry 是否固定原 snapshot 的范围

状态：**已确认**。

- Resume / Retry 始终固定原 snapshot 的 `end_page`，同时也固定其 URL、Cookie 和原始页码边界。
- 续跑起点只根据原 snapshot 的进度或失败页计算，不重新读取当前 Form。

实施前实现尚未做到这一点：终态没有保存 `FetchRequest`，Resume / Retry 会重新从混合状态中的当前表单
字段构造 request（`app/feiwen/src/features/fetch.rs:195-202`、`734-807`）。

#### FEI-FETCH-Q04：snapshot UI 如何处理 Cookie 与恢复表单

状态：**已确认**。

Form 影响：**Form 仅提供原语**。vNext 的 whole-model `replace` 负责原子替换当前 draft；快照保存、
`FetchRequest -> FetchDraft` 转换、按钮与 Cookie 脱敏仍属于 Fetch owner 和 UI。

- snapshot UI 不单独暴露或复制 Cookie 原文，只显示脱敏摘要。
- 与 Query 的失败快照相同，提供一个把完整 snapshot 载入 Form 的按钮；URL、页码和 Cookie 一次性
  设置，不能只复制其中一个字段。
- 该按钮是应用层显式 Form whole-model 替换动作，不是运行状态自动回写，也不是剪贴板复制。

当前没有 snapshot UI 或“载入表单”按钮；现有 Cookie 区域只有静态的隐藏提示
（`app/feiwen/src/features/fetch.rs:898-982`、`986-1010`）。

#### FEI-FETCH-Q05：成功后是否主动修改 Form

状态：**已确认**。

Form 影响：**Form 仅提供原语**。vNext 不观察 Fetch completion，也不会自动修改 Form；应用 owner 仍须
保证成功路径不主动调用 `replace`、`reset` 或 `rebase`。

- 成功后不主动 replace、reset 或 rebase Form，保留用户当前编辑的下一次 Fresh 草稿。
- 如果用户希望恢复已完成请求，只能显式点击 snapshot 区域的“载入表单”按钮。
- 运行成功本身只更新 `FetchRun`，不产生 Form mutation。

#### FEI-FETCH-Q06：终态保留与运行历史

状态：**已确认**。

- 同一 workspace 同一时刻只有一个 `FetchRun` 状态。
- 不保留运行历史；新的 Fresh run 覆盖旧的成功、失败或中断终态，并按 `FEI-FETCH-Q02` 清空日志。
- 当前终态只保留到新的 Fresh run 覆盖它或 workspace 生命周期结束。

实施前实现同样只有一个 `FetchTaskState.status` 和一个 `task: Option<Task<()>>`；新的 Fresh 会同步安装
`Running`，不存在两个并存状态
（`app/feiwen/src/features/fetch.rs:87-117`、`app/feiwen/src/app/workspace.rs:32-48`、
`app/feiwen/src/features/fetch.rs:734-845`）。

#### FEI-FETCH-Q07：是否需要 RunId 过滤迟到事件

状态：**已确认**。

这里的 detached producer 指“不再由当前 `FetchRun::Running` 中唯一 Task 持有和取消的独立后台
生产者”。例如，runner 又 detach 一个 Task、线程或 channel producer；即使用户已经中断、开始下一轮，
它仍可能向 owner 投递上一轮的 `PageSucceeded`、`Failed` 或 `Succeeded`。这类旧事件可能污染新状态，
所以必须随事件携带 `RunId` / generation，并由 Transition 拒绝不属于当前运行的事件。

当前 Feiwen 抓取不存在这种 producer：唯一的 `cx.spawn` runner 由 `FetchTaskState.task` 持有，runner
通过 `WeakEntity<FetchTaskState>` 串行投递全部进度和终态；源码中没有第二个 detached Task 或独立
completion route（`app/feiwen/src/features/fetch.rs:371-497`、`819-845`）。

已确认不支持两个并存 run，也不预先增加 `RunId`。owner plan 必须把“生命周期事件只能由
`Running` 持有的唯一 Task 产生”作为硬约束：不得 detach 会继续投递进度或终态的子任务；中断必须
终止这条 completion route，之后才能允许新的 Fresh run。

未来如果需求确实要求 detached producer 或两个并存 run，这将改变本次已确认架构，必须重新打开该
问题并同时设计 `RunId` / generation，不能在现有实现中静默加入第二条运行路径。

## 6. Feiwen QueryCatalog

### 6.1 已确认的方向

#### FEI-CATALOG-01：owner 拆分（原问题不是产品语义未定）

状态：**已确认并实施**。

实施前，`QueryOptions` 只在 `QueryView` 创建和 Reset 时同步从数据库加载，然后被复制到
`AdvancedQueryState` 及后续创建的 Select / Combobox 中
（`app/feiwen/src/features/query.rs:94-120`、`162-189`、
`app/feiwen/src/features/query/advanced/state.rs:36-53`）。问题是 catalog loader、运行状态与 UI adapter
混在查询页面，不是缺少 tags/authors 的产品定义。本轮已把 owner 拆到 application-global
`Store<QueryCatalog>`，查询页只消费 phase、problem、data 与显式动作。

#### FEI-CATALOG-02：tags/authors 使用全局 Operation Store

状态：**已确认**。

已确认目标。为了正确合并抓取期间发生的 catalog 失效，最终 Store state 在 Operation 外保存
owner-private 的 invalidation generation；消费者只选择 Operation 的 data / phase / problem：

```text
QueryCatalogStore
  = Store<QueryCatalogState>

QueryCatalogState
  = operation: refresh::Operation<QueryCatalogData, QueryCatalogProblem, Task<()>>
  + invalidation: CatalogInvalidation

CatalogInvalidation
  = current_generation: u64
  + covered_generation: u64

QueryCatalogData
  = tags + authors 的纯值模型
```

- Store 是 application-global 的唯一 catalog 运行状态与内存快照；generation 只是 owner 合并写入失效
  通知、判断某次读取覆盖范围的控制事实，不是第二份 catalog Data，也不向 Form 暴露。
- loader 在 DB Ready 后从 pool 获取连接，在后台复用现有 SQL，并投递 `Load / Refresh / Retry /
  Complete`；每个 read run 捕获自己的 `target_generation`，只有成功安装 Data 才推进
  `covered_generation`。
- `refresh::Operation` 与 catalog 匹配：首次失败为 `Unavailable`；已有成功数据后的刷新失败为
  `Degraded`，可以保留最后一次有效 options。
- Store 内不保存 `SelectItem`、`SearchableVec`、Input/Select/Combobox Entity 或高级查询草稿；这些
  都属于 UI adapter。
- DB resource 离开 Ready 时，catalog owner 先投递 `Cancel` 终止 active catalog Task，再把 `Idle` /
  `Ready` 投影为带 dependency problem 的 `Unavailable` / `Degraded`；本来已经非 Ready 的 phase 保持非
  Ready。该 problem 只用于让 catalog Operation 退出精确 `Ready`，UI 仍显示 DB resource 的权威错误与
  repair 入口，不把它伪装成 catalog 可以独立修复的错误。
- 即使 `Degraded` 仍保留 last-known-good Data 供只读展示，也不能开放 catalog 控件或查询提交。DB
  repair 恢复 `Ready` 后不自动启动 catalog 读取；用户通过独立的“重新加载目录”入口显式发起，owner
  按当前 phase 映射为 `Load` / `Retry` / `Refresh`。成功前保持非 Ready gate；若还有未覆盖 generation，
  本轮成功后再按 generation 契约判断是否需要合并一次 Refresh。

### 6.2 Jaco 模型选择器的可复用边界

状态：**已完成源码对照；用户已确认 Q01 参考该模式**。

Jaco 的模型选择器不是 Store 自动绑定 Form 或控件，而是显式拆成五层：

| 层 | Jaco 当前做法 | Feiwen 可复用部分 |
| --- | --- | --- |
| Catalog owner | application-global `Store<refresh::Operation<ProviderData, ProviderProblem, Task<()>>>`；selector 只投影 models、phase 与 problem | `QueryCatalogStore` 继续保存纯 `QueryCatalogData`、Operation phase/problem 与 owner-private invalidation generation |
| Form value | 只保存稳定的 `ProviderModelKey`，不保存 picker index、label 或 options | tag/author 条件只保存自己的 typed value，不保存 `SelectItem`、`SearchableVec` 或 index |
| Controller subscription | `RunSettingsController` 用 `observe_select_in` 订阅 catalog selector，回调 `reload_models` | 高级查询的 controller/renderer owner 保留订阅，并更新当前已挂载的 tag/author adapter |
| Native projection | `replace_projection` 替换 options，保留搜索 query，再按 Form value 重算 selected item；找不到时 native selection 为空但 Form 不变 | 先替换 Select/Combobox items，再用当前 typed value 静默调用相应 selected-value projection；不得发用户输入事件 |
| Submit/phase | 提交时重新按当前 catalog 解析；精确 `Ready` 才可选择，Refreshing/Degraded 保留旧 options 可见但只读并显示状态 | Feiwen 已按 Q03/Q04 采用非 Ready gate；精确 Ready 中缺失值按 Q02 作为非阻塞 literal 继续允许提交 |

源码依据：

- Jaco catalog/selector/加载：`app/jaco/src/state/providers.rs:16-74`、`250-348`；
- catalog observation 与 native 重投影：
  `app/jaco/src/components/chat/run_settings.rs:535-627`、`883-905`；
- Form value、提交时不可用检查和 phase UI：
  `app/jaco/src/components/chat/run_settings.rs:54-130`、`1013-1182`；
- picker 投影保留搜索条件：`app/jaco/src/components/picker.rs:168-224`、`712-758`。

Feiwen 不能照搬的部分：

1. Jaco 只有一个 model picker controller；Feiwen 有递归动态树中的多个 tag/author Select 与 Combobox，
   因此 catalog subscription 的 owner 必须更新所有仍挂载且路径仍有效的 adapter。
2. Jaco 的 provider mutation 返回足够完整的 committed record，可以向精确 `Ready` Data 投递
   `ProviderMessage` 并原地重建 enabled-model projection。Feiwen tags/authors 是从多本 novel 聚合出的
   派生目录，单本保存结果通常不足以判断旧 author/tag 是否仍应保留，不能默认照搬 direct apply。
3. Jaco 的 model 是发送请求必需资源；Feiwen 只有 tag/author 条件依赖 catalog，不能从 Jaco 推导“整个
   高级查询都必须禁用”。

当前 `gpui-component` 已提供所需的静默 native 原语：`SelectState::set_items` 后调用
`set_selected_value`，`ComboboxState::set_items` 后调用 `set_selected_values`；后两者按新 delegate 查找
typed value，缺失项不会变成 fallback，也不会发出用户确认/Change 事件。最终 vNext adapter 的公开签名
仍由 Form 实施计划确定，本文只固定上述同步顺序和所有权。

### 6.3 问题状态

#### FEI-CATALOG-Q01：已创建控件如何接收新 options

状态：**已确认；参考 Jaco 模型选择器**。

采用以下显式顺序，不重建整个 `AdvancedQueryState`，也不建立 Form↔Store 隐式绑定：

1. QueryCatalog owner 发布新的 data/phase/problem selection。
2. 高级查询 controller/renderer owner 的 retained subscription 收到变化。
3. owner 替换当前已挂载 tag/author Select 与 Combobox 的 native items/delegate。
4. owner 从 Form 重新读取各字段的 authoritative typed value，并静默重投影 native selection。
5. owner 使用同一 catalog snapshot 替换 validator context，再显式运行 dynamic validation。

options 刷新本身不写 Form、不选择 fallback、不 rebase，也不产生 Select/Combobox 用户事件。动态路径已经
删除、case 已切换或 adapter 已 drop 时，更新只忽略该旧 adapter，不得把它重新挂载。

#### FEI-CATALOG-Q02：当前值已不在新 catalog 中

状态：**已确认**。

用户不会直接从当前 options 中选到一个不存在的值。可达路径是：用户先选择当时存在的值，之后数据库
内容发生变化，QueryCatalog Refresh 得到的新 snapshot 不再包含它。

当前源码中 tags 与 authors 的变化规律不同：

- 正常抓取保存对 `tag` 和 `novel_tag` 只有 `INSERT ... ON CONFLICT DO NOTHING`，没有删除或替换旧关系；
  因此已经进入 catalog 的 tag 在当前生产写入路径中基本只增不减
  （`app/feiwen/src/store/service/novel.rs:103-114`、
  `app/feiwen/src/store/service/tag.rs:26-43`）。
- author catalog 没有独立历史表，而是直接对当前 `novel.author_id/author_name` 做 `GROUP BY`；同一本小说
  再次抓取时，upsert 会覆盖这两个字段。如果旧作者只被这本小说引用，它会在下一次 Refresh 后从
  catalog 消失
  （`app/feiwen/src/store/service/novel.rs:62-101`、
  `app/feiwen/src/features/query/advanced/options.rs:230-244`）。
- 未来执行已经确认的“备份后重建”或数据库被外部修改时，tags/authors 也可能与仍在 Query Form 中的
  旧值脱钩。

因此这个问题对 author 是正常可达的，对 tag 则主要是重建或外部修改边界。Form 继续保留原 typed value，
native 控件不得替 Form 清值、选择 fallback 或退化为 index。当前 tag value 是名称 `String`，author value
是 `AuthorRef`；即使值已不在 catalog 中，现有 SQL 仍能把它当作查询 literal 执行，通常得到空结果而
不是数据库错误（`app/feiwen/src/store/query.rs:394-416`）。

已确认采用适合 Feiwen 查询 literal 的行为，不照搬 Jaco 的 blocking model error：

- Form 保留原 `String` / `AuthorRef` typed value；catalog Refresh 不清值、不选择 fallback，也不重写
  baseline。
- native selection 无法在新 options 中解析时，字段显示“当前目录中不存在”的非阻塞提示；不得只显示成
  一个没有原因的空选择。
- 该提示不进入 blocking validation。只要 QueryCatalog 已回到精确 `Ready`，用户仍可用原 typed value
  提交查询；没有匹配数据时正常得到空结果。
- 用户可以显式删除或替换该值；应用不得因 catalog 更新自动替用户修正 Form。

#### FEI-CATALOG-Q03：首次 Loading / Unavailable 时禁用范围

状态：**已确认；沿用 Jaco 的局部控件禁用边界**。

采用 `FEI-CATALOG-Q06` 的启动后立即 Load，不表示 Query 页面出现时 catalog 必然已经 Ready。目标实现会
在同步初始化中先安装全局 Store 和 `Loading(Task)`，随后继续打开窗口并创建 `WorkspaceView` / `QueryView`；
异步读取完成后才投递 `Complete`。因此 Query 页面在首次 Loading 中挂载是正常首屏路径，不是异常竞态
（当前构造顺序见 `app/feiwen/src/main.rs:98-108`、
`app/feiwen/src/app/workspace.rs:42-49`）。

已确认：

- `Loading` 与 `Unavailable` 都禁用查询按钮以及依赖 catalog 的 tag/author 选择控件。
- 文本、数值、布尔、排序和其他不依赖 catalog 的 Form 字段继续允许编辑；不能因为 tags/authors 尚未
  Ready 而锁住整个高级查询。
- `Loading` 显示 catalog 正在加载；`Unavailable` 显示 catalog problem 和显式 Retry。DB Resource 自身
  不可用时仍显示 DB 的恢复入口，不把它伪装成 catalog Retry。
- Form draft 在两种 phase 下都不被清空、重建或回滚；已经存在的 tag/author typed value 继续按
  `FEI-CATALOG-Q02` 保留。
- Load 成功进入精确 `Ready` 后，controller 按 `FEI-CATALOG-Q01` 投影 options、重新投影 Form value、
  运行 dynamic validation，再开放编辑和提交。

#### FEI-CATALOG-Q04：Refreshing / Degraded 时旧 options 的交互

状态：**已确认**。

- `Refreshing`、`Degraded` 与 `RefreshingDegraded` 可以继续显示 last-known-good options 作为上下文，
  同时显示 refresh/problem 状态。
- 旧 options 不允许继续用于选择；依赖 catalog 的 tag/author 控件保持只读。
- Query 不允许使用旧 catalog 提交；查询按钮在 catalog 回到精确 `Ready` 前保持禁用。
- 非 catalog 字段的草稿编辑不等于使用旧 options，本问题不要求清空、回滚或锁住这些 Form value。

#### FEI-CATALOG-Q05：Reset 是否触发 catalog Refresh

状态：**已确认：只 Reset Form**。

- 标题栏 Reset 只调用 Form `reset`，恢复查询草稿的 baseline。
- Reset 不向 QueryCatalog 投递 Load、Refresh 或 Retry，不改变 catalog phase、problem、Data 或 Task。
- 重置后的 catalog 控件继续使用当前 snapshot，并按当前 phase 的规则投影；非 Ready 时仍按
  `FEI-CATALOG-Q03/Q04` 保持只读且禁用查询按钮。
- catalog 的显式 Refresh / Retry 是独立资源动作，不与 Form 生命周期绑定。

#### FEI-CATALOG-Q06：首次加载时机

状态：**已确认：应用启动后立即加载**。

- 应用初始化时安装 application-global QueryCatalog Store。
- 启动期 DB Resource 已经精确 `Ready` 后，立即向 catalog Operation 投递首次 `Load`；不等待用户进入
  Query 页面，也不采用 lazy load。
- 首次读取异步执行，所以 Query 页面仍必须按 `FEI-CATALOG-Q03` 处理可见的 `Loading`。

`FEI-DB-Q06` 已确认数据库 repair completion 不自动 Load / Refresh QueryCatalog，因此“启动后立即加载”
只定义正常启动期的 eager load。若启动时 DB 不可用、之后通过 repair 恢复，catalog 仍需要自己的显式
“重新加载目录”入口；不能改回首次进入 Query 页面时隐式加载，也不能把 DB repair completion 当成刷新
命令。owner 根据当前非 Ready phase 把该动作映射为 `Load` / `Retry` / `Refresh`，具体组件与消息归
owner plan，不需要重新选择 eager / lazy 产品策略。DB 离开 Ready 后，catalog 即使保留 last-known-good
Data 也不得继续发布可提交的精确 `Ready`；显式读取成功前保持非 Ready gate。

#### FEI-CATALOG-Q07：抓取提交后的 invalidation

状态：**调研结论；Jaco direct apply 不适用，保留 Feiwen invalidation generation 方案**。

Jaco 的写入结果能精确更新 provider/model catalog，因此数据库提交成功后直接向 `Ready` Data 投递
业务 message，不重新查询。Feiwen catalog 是 novel/tag 聚合投影，当前每本 `Novel::save` 又是独立事务；
一次保存结果通常不足以判断旧 author/tag 是否仍被其他记录引用。

因此本 issue 继续采用 `5.4` 的规则：每个影响 tags/authors 的成功事务都推进
`invalidation_generation`；每个 Load / Refresh / Retry 捕获本轮 `target_generation`，只有成功安装
Data 才推进 `covered_generation`。运行期间出现更高 generation 时，成功完成后最多再合并一次
Refresh；失败或取消不推进 covered，未覆盖部分留给显式入口，并由 owner 按 phase 映射为 Retry / Load /
Refresh。以后若先把整页保存改成单个原子事务，只改变 invalidation 的通知粒度，不改变 owner 合并规则。
只在整个抓取 Succeeded 后刷新仍然不正确，因为会漏掉失败或中断前已经提交的数据。

#### FEI-CATALOG-Q08：`id IS NULL` tag 的既有 SQL 语义

状态：**调研结论；Jaco 不适用，本 issue 保持现有行为**。

模型选择器没有等价语义。Feiwen `Tag::tags_with_id` 当前明确排除 `id IS NULL`，QueryOptions 继续消费该
结果；QueryCatalog owner 拆分不得顺带改变这一 SQL contract。未来若要让匿名/无 ID tag 进入 catalog，
应作为独立产品与数据查询改动重新讨论。

## 7. Feiwen 数据库资源

### 7.1 已确认的方向

#### FEI-DB-01：参考 Jaco 已实施的数据库设计

状态：**已确认**。

这里“参考 Jaco”指复用数据库资源生命周期原则，不是照搬 Jaco 的 SQLite 文件实现：

- 应用启动时始终安装一个可读取的 DB resource Store；初次打开失败也必须进入可呈现的
  `Unavailable`，不能只写日志后不安装 Global。
- query / fetch 只有在数据库精确处于 Ready 时才能获得新 job / connection。
- 当当前 pool 已不可信或准备替换时，先拒绝新 job，再等待或取消在途 job，释放旧连接后才进入
  repair 或安装新 pool。
- repair 必须是明确的领域动作，不能用 `Repair = ()` 把普通 Retry 伪装成用户修复。
- 非法消息保留原状态、记录诊断并丢弃；不能用临时 Idle 或旧 pool 兜底。

Jaco 的页面映射也作为 Feiwen 的直接参考：`Ready` 只渲染正常 Home，不显示数据库错误页或修复动作；
非 Ready 资源 phase 才渲染或覆盖 `CriticalResourcesView`，并在 problem view 中构造“重新打开”与可用时的
“备份后新建”动作。破坏性动作会先进入确认对话框，再选择备份目录并投递 repair
（`app/jaco/src/features/home/root.rs:193-275`、
`app/jaco/src/components/resource.rs:145-225`）。因此本设计不存在“Ready 时如何显示数据库修复”的问题：
Feiwen 的正常页面与数据库错误资源页同样按 phase 互斥。Jaco 自己的非 Ready phase 包含
`Retiring / Unavailable / Repairing`；Feiwen 本轮只映射实际可达的 `Loading / Unavailable / Repairing`，
不因参考 Jaco 而预建 `Retiring`。

Jaco 使用自定义 `DatabaseOperation` 的原因是 refresh 失败后不能把不可信 session 当作
`Degraded` Data 继续使用；它必须经过 `Retiring`，等待 active jobs drain，再进入 `Unavailable`
（`app/jaco/src/database/operation.rs:8-61`、`135-223`，
`app/jaco/src/database/session.rs:101-211`）。

### 7.2 Feiwen 当前缺口与不可照搬的部分

状态：**调研结论**。

当前 `init_store` 打开 DuckDB 或初始化 schema 失败时只记录日志并返回，不注册 `Db` Global；query
和 fetch 随后又无条件读取该 Global，既没有统一不可用 UI，也没有恢复入口
（`app/feiwen/src/store.rs:40-54`、
`app/feiwen/src/features/query.rs:108-120`、
`app/feiwen/src/features/fetch.rs:708-731`）。

Feiwen 与 Jaco 的差异决定了不能复制实现：

| 表面 | Jaco | Feiwen | 设计影响 |
| --- | --- | --- | --- |
| 数据库 | SQLite / FreshStore | DuckDB | 文件与备份协议必须重新验证 |
| 运行资源 | 长期 DatabaseSession + job gate | r2d2 pool，consumer 当前可直接 clone | 通过精确 Ready gate 发放 pool；本轮没有 Ready pool replacement |
| 路径 | 可随配置重绑定 | 固定 config dir 下 `data.duckdb` | 不复制 AwaitingConfig / target rebind |
| 连接持有 | session executor | query 后台 checkout；fetch runner 全程持有 connection | repair 只从 Unavailable 发起，本轮不增加 retire/drain 协议 |
| 文件保护 | Jaco 有自己的锁/备份流程 | 当前没有等价边界 | 不能假设跨进程替换安全 |

只要 Feiwen 需要验证、替换 pool 或破坏性修复，就应建立应用私有数据库 Transition，至少能表达
`Loading / Ready / Unavailable / Repairing`。owner plan 已确认本轮没有让 Ready pool 退出或替换的实际
producer，`Ready` UI 也不提供 repair 动作，因此目标状态机不包含 `Retiring`、active-job drain 或
cancel-all 分支。用户从 `Unavailable` 选择 repair 时已经没有可用 pool，可直接进入 `Repairing`；
`BackupAndRebuild` 必须先完成二次确认。数据库 Store 是资源状态的唯一运行时 owner，query、fetch 和
QueryCatalog 通过 Ready gate 获取 job，而不是继续直接 clone 裸 pool。未来只有出现具体的 Ready pool
替换 producer 时，才重新设计 retire/drain，而不是在本轮预留不可达状态。

### 7.3 建议的错误语义

状态：**调研结论**。

| phase / problem | 建议含义 |
| --- | --- |
| Opening / Loading | 建目录、创建 DuckDB manager、建立 pool、checkout、初始化 schema |
| Ready | pool 与 schema 可安全使用；允许新 database job |
| Unavailable | 没有可安全使用的 pool；由数据库资源 UI 统一呈现 critical problem |
| Repairing | 执行用户选择的 `Reopen` 或 `BackupAndRebuild` |

数据库错误不应分别伪装成 Query、Fetch 或 QueryCatalog 的普通业务错误。consumer 可以展示“数据库
不可用”投影，但权威 problem 和修复入口属于 DB resource owner。

### 7.4 问题状态

#### FEI-DB-Q01：数据库恢复入口

状态：**已确认**。

- 提供独立的数据库资源 UI，不把恢复入口限制在启动失败时的一次性页面。
- UI 提供两个明确按钮：“重新打开”和“备份后重建”。
- `Ready` 时不显示、也不允许触发这两个修复动作。
- `Unavailable` 时显示这两个恢复动作；`Repairing` 时两个按钮均不可重复触发。

实施前实现没有数据库资源 UI 或恢复入口；初始化失败只记录日志并且不安装 `Db` Global
（`app/feiwen/src/store.rs:41-52`）。

#### FEI-DB-Q02：允许哪些 repair 动作

状态：**已确认**。

目标 repair value 只有两个领域动作（类型名仍是草稿）：

```rust
enum DatabaseRepair {
    Reopen,
    BackupAndRebuild,
}
```

- `Reopen` 重新打开并验证固定路径下的现有 `data.duckdb`，不重建数据。
- `BackupAndRebuild` 必须先成功备份现有数据库，再建立新数据库；不提供“选择新目录”。
- `BackupAndRebuild` 在投递 repair message 前必须经过二次确认；用户取消确认时数据库状态不变。
- 两个按钮投递不同 repair value，不能把它们合并成无语义的 Retry。

#### FEI-DB-Q03：破坏性 repair 的文件协议

状态：**产品方向与技术协议均已确认并完成 failure-injection 验证**。

当前 duckdb-rs `1.10505.0` 对应 DuckDB `1.5.5`。DuckDB 官方文档确认：WAL包含崩溃恢复需要的数据，
`CHECKPOINT`负责把WAL同步进主文件；但本动作从无法安全打开的 `Unavailable` 开始，不能把“先成功
checkpoint”作为备份前提。因此最终协议不做逻辑导出，也不只复制主文件：

1. 二次确认并选择尚不存在的backup目录；取消时不投递repair。
2. 先复制存在的 `data.duckdb` 与 `data.duckdb.wal`，逐文件和目录同步；任一步失败都不开始重建。
3. 在live数据库同一父目录建立、checkpoint、关闭并重新验证一个全新staging数据库。
4. 把原artifacts先rename进同目录rollback位置，再把已验证staging artifact集合rename到固定live路径，
   同步父目录并重新打开验证。
5. 新live数据库验证及父目录同步成功前，rename、同步、reopen或schema验证任一步失败，都关闭新连接并
   反向恢复原artifacts；backup目录始终保留，不自动导入或删除。
6. 新live数据库验证及父目录同步成功是commit point；此后才发布新的Ready pool。commit后的rollback
   临时目录只尽力清理，失败时保留残留路径并记录诊断，不撤销已验证的新live数据库。主文件与WAL都
   不存在时视为backup失败，不以空目录伪装成功。

完整错误分类、文件顺序与测试矩阵由
[Feiwen完整owner plan `DB-700`](../../../app/feiwen/docs/dev/issue-199/form-operation-store-migration.md#db-700duckdb-备份后重建文件协议)
负责。官方依据：

- [CHECKPOINT](https://duckdb.org/docs/current/sql/statements/checkpoint)
- [crash/WAL recovery](https://duckdb.org/docs/current/guides/troubleshooting/crashes)
- [DuckDB local files](https://duckdb.org/docs/current/operations_manual/footprint_of_duckdb/files_created_by_duckdb)

#### FEI-DB-Q04：Retiring 如何处理运行中的 job

状态：**已确认并完成源码调研**。

当前 Feiwen 没有 `Retiring` 或任何运行时 pool 替换路径：`Db` 只是启动时设置一次的 Global，query /
fetch 的数据库错误只进入各自页面状态，不会让 DB owner 突然进入 `Retiring`
（`app/feiwen/src/store.rs:19-52`、`app/feiwen/src/features/query.rs:257-321`、
`app/feiwen/src/features/fetch.rs:708-845`）。

目标行为是：

- 普通 query / fetch 失败不自动触发 DB `Retiring`。
- `Ready` UI 不提供 reopen / rebuild，因此 repair 动作不会触发 `Ready -> Retiring`。
- 只有 DB owner 因实际资源级原因必须让仍被 job 持有的 pool 退出 Ready 时，才允许进入 `Retiring`；
  owner plan 必须给出这条路径的具体 producer，否则不实现该 phase。
- 进入 `Retiring` 后立即关闭 Ready gate、拒绝新 job，并直接取消所有在途 query、fetch 和
  QueryCatalog job；不等待它们自然完成，也不按 consumer 类型分别处理。
- 文件备份、替换或重新打开仍需等待已取消 job 的 connection / pool 句柄实际释放；“直接取消”不等于
  在旧连接仍存活时操作数据库文件。
- 从 `Unavailable` 发起 repair 时已经没有可用 pool，不经过 `Retiring`；`Reopen` 可直接进入
  `Repairing`，`BackupAndRebuild` 则在二次确认后进入 `Repairing`。

本轮owner plan没有发现任何需要让Ready pool运行时退出的实际producer，因此按用户结论不实现
`Retiring`、active-job drain或cancel-all分支。若未来增加Ready状态下的资源替换需求，再以新的具体
producer重新打开该设计，不能为了状态对称预建不可达phase。

#### FEI-DB-Q05：跨进程文件租约

状态：**已确认不纳入范围**。

- 不设计应用级跨进程 lease、PID 文件或第二个 Feiwen 进程之间的 repair 协调。
- 本次恢复协议按单个 Feiwen 进程拥有修复流程设计；底层 DuckDB 自身的文件占用错误只作为打开或
  repair 失败呈现，不再增加一层应用锁协议。

#### FEI-DB-Q06：修复完成后的 consumer 行为

状态：**已确认**。

- 数据库修复成功只把 DB resource 恢复为 `Ready`，不自动 Refresh 或重新执行任何 consumer。
- QueryCatalog 不自动重新 Load / Refresh；由已经确认的独立“重新加载目录”入口重新读取，owner 根据
  当前 phase 映射为 `Load` / `Retry` / `Refresh`，具体组件与消息归 QueryCatalog owner plan。DB repair
  completion 不替它投递该动作。
- query / fetch UI 在 Ready gate 恢复后重新允许用户操作，但不自动重跑先前被取消或失败的请求。

#### FEI-DB-Q07：Ready phase 下修复动作与破坏性确认

状态：**已确认**。

- 数据库处于 `Ready` 时不显示“重新打开”和“备份后重建”，其他事件入口也不得绕过 UI 投递这两个
  repair 动作。
- 两个修复动作只在 `Unavailable` 时显示；`Reopen` 可以直接开始，`BackupAndRebuild` 必须先经过
  明确的二次确认。
- 用户取消二次确认时不投递 repair message，数据库 phase、problem、Data 与 Task 全部保持不变。
- 因此用户修复不存在 `Ready -> Retiring` 路径；从 `Unavailable` 确认修复后进入 `Repairing`。

## 8. Feiwen 高级查询 Form

状态：**产品语义已全部确认并实施；实际 UI 操作测试未执行**。

原先笼统的“高级查询 Form 暂缓”现在拆成两层：gpui-form vNext 已经讨论并选定递归、异构、动态嵌套
所需的库设计；Feiwen 已选择并实施 recursive typed tree、field/relation 切换和字段错误展示语义。
动态 item identity 的 ownership 由 Form runtime 负责。以下“已覆盖”结论已经由 Form producer 与
Feiwen consumer 自动化验证。

### 8.1 已由 Form vNext 收束的库问题

#### FEI-ADVFORM-Q02：stable identity 与动态路径稳定性

状态：**已确认：由 Form runtime 生成并持有**。

- `QueryDraft` / `QuerySpec` 不保存只服务于表单寻址、校验或 UI 生命周期的 ID；业务 model 也不需要为
  dynamic collection item 实现 identity trait 或标注 identity 字段。
- Form runtime 在 session 内为每个 dynamic item occurrence 生成 opaque identity，并只通过 collection
  enumeration 与 topology mutation 返回 typed `ItemPath<Root, Item>`。调用方不能从 raw ID、业务值或
  index 构造 item path。
- 当前 Feiwen 的 `u64` ID 所承担的递归定位、删除/移动目标、事件回调、validation、binding、subscription
  与 GPUI UI key 等编辑期职责，目标上全部改由 Form 的 typed path 及其受控投影承担；真正属于业务领域的
  tag ID、`AuthorRef` 等仍保留，不受本决定影响。
- same-parent reorder 保留现有 item path。remove、`replace_all`、whole-model replace/rebase、case
  reconstruction 与被移动 occurrence 的 cross-parent move 会 retire 受影响的旧 path；cross-parent move
  是同一 root Form 的原子 operation，并返回 destination 下的新 path。
- 删除后重新插入一个业务值相等的 item 仍是新 occurrence，旧 binding、issue、subscription 与 async
  completion 不得复活或命中新节点。
- Form 已固定并实现 `ItemPath`、collection enumeration/mutation、路径 freshness 与 adapter UI-key
  投影的精确签名；Feiwen不再选择或保存 `NodeId`。

#### FEI-ADVFORM-Q05：递归异构树如何映射 Form path

状态：**Form 设计已覆盖；不再作为 Feiwen 未回答问题**。

vNext 已删除让调用方手工选择 `FormField` / `PartialFormField` 的问题。root-first 静态路径为
`TotalPath<Root, T>`；通过 Form runtime 返回的 `ItemPath` 进入动态 item。case/optional payload必须在
当前session中fallible locate并捕获incarnation，不能使用不接收form的纯 `.case(...)` / `.some()`。
用户已确认精确入口为 `.try_case(&Form, CaseDef)` / `.try_some(&Form)`；调用方通常传入
`form_entity.read(cx)`，resolver不接收Entity或cx，返回的 `DynamicPath<Root, T>` 也不持有entity。
所有后代保持dynamic，最终value类型仍由Rust类型系统保留为 `T`。`TopologyIndex` 与本次操作的snapshot
完全属于Form core，Feiwen不会看到或传入。

#### FEI-ADVFORM-Q07：嵌套校验错误的定位

状态：**已确认**。

Form issue 使用 typed/canonical path，并携带动态 address、incarnation、source、trigger 与 generation；
subtree 删除、case replacement 或新 occurrence 重建后，旧错误与迟到 async completion 不会贴到新节点。

Feiwen 只在实际出错的那个字段旁展示该字段的错误，不建立 condition、group 或页面级错误汇总，也不在
其他字段重复同一错误。Form core 只提供按 typed/canonical path 查询字段 issue 的能力；具体行内样式由
component adapter 负责。

#### FEI-ADVFORM-Q06：catalog 值消失时的验证与提交

状态：**Form 边界已覆盖；应用行为已由 `FEI-CATALOG-Q02` 确认**。

- catalog/options 更新不会隐式修改 typed value、选择 fallback、重写 baseline 或触发持久化。
- catalog owner 更新 native options 后，显式替换 validator context 并触发 dynamic validation。
- 已消失的 tag/author 显示“当前目录中不存在”的非阻塞提示，不形成 blocking issue。
- QueryCatalog 精确 `Ready` 时允许用保留的 typed value 提交；非 Ready 时由页面的 catalog phase gate
  禁用查询按钮，不把资源 phase 伪装成字段 validation error。
- 用户可以显式删除或替换缺失值，Form 和 adapter 不主动修改它。

### 8.2 Feiwen 业务决策

`FEI-ADVFORM-Q02` 的 identity 设计和 `FEI-ADVFORM-Q07` 的定位、展示结论保留在 `8.1`；下面继续按原
编号保存已经确认的业务决定。

#### FEI-ADVFORM-Q01：业务 draft 的权威形状

状态：**已确认：使用 recursive typed tree**。

- `QueryDraft` 直接拥有递归的 group / condition typed tree，不使用 root-owned normalized records + ID
  引用来表达父子关系。
- group 直接拥有有序 children；每个 node 只有一个父节点，删除 group 即删除完整子树，提交时从同一
  prepared tree 递归生成 `QuerySpec`。
- condition / group 使用 Rust enum 和具名 payload 表达有效结构，不使用一组 optional fields 模拟互斥
  variant。
- 业务 draft 与 native control Entity、subscription、focus、IME 和 incomplete editor state 分离。
- `QueryDraft` / `QuerySpec` 不包含纯表单 identity；只有真正参与查询或持久化语义的领域 ID 才进入
  business draft。

#### FEI-ADVFORM-Q03：field type 改变时如何处理旧值

状态：**已确认：置空**。

用户把一个 condition 切换到不同 field type 时，保留新选择的 field，但清空旧 relation 和旧 typed
value；condition 回到“已选 field、尚未选择 relation/value”的可见空状态。不得在 Form 外保留不可见旧
草稿，也不按表面兼容性自动迁移旧值。Form 的 case replacement 负责原子替换、清理旧 issue/binding 并
重新验证新 case。

#### FEI-ADVFORM-Q04：relation 改变时如何处理已有 value

状态：**已确认：value 不变化**。

relation 只是同一 field type 下的运算符。用户改变 relation 时，只更新 relation 本身，不清空、不迁移、
不替换该 field 已有的 value；这和 `FEI-ADVFORM-Q03` 中切换 field type 是不同操作。

实施前实现会让 relation 选择 native value 形状，例如数字单值/范围、tag 有值/无值、author 文本/单选/多选，
因此 relation setter 会重建 `ConditionDraft` 和控件。目标设计不得继承这种耦合：

- field type 决定一个稳定的 typed value draft；
- relation 只决定提交时如何解释该 draft，以及当前需要显示、校验哪些输入；
- 暂时不被当前 relation 使用的 field-owned value 继续保留，不产生 validation issue，也不进入本次
  `QuerySpec`；切回需要它的 relation 时仍可继续使用；
- group 的 `All / Any` 同样只改变组合运算符，不修改任何 child condition。

具体 Rust 容器由 owner plan 固定，例如 number 可以稳定拥有单值/范围所需的 operands，author 可以稳定
拥有文本与 typed author selection；这属于落实“relation 不改变 value”的类型设计，不再是产品问题。

Feiwen 高级查询已没有待回答的产品问题。独立
[Feiwen完整owner plan](../../../app/feiwen/docs/dev/issue-199/form-operation-store-migration.md)已经创建；
其中高级查询的fallible case/optional定位、`ItemPath`、topology mutation、路径失效、validation与
adapter契约已经完成 producer/consumer 自动化验证；实际 UI 操作测试按要求未执行。

## 9. HTTP Client：尚未回答的问题

状态：**全部未回答**。当前 Send runtime 仍未实现，因此本文只保存问题，不预建状态机。

### 9.1 请求运行与 Operation

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

### 9.2 RequestDraft Form

- **HTTP-FORM-Q01：** 支持哪些 URL scheme，以及相对 URL 是否依赖 environment base URL。
- **HTTP-FORM-Q02：** URL 与 Params 的唯一 source of truth；URL 无法解析时 Params 如何显示和编辑。
- **HTTP-FORM-Q03：** Params 修改后是否规范化或重写用户原始 URL 字符串。
- **HTTP-FORM-Q04：** empty / duplicate headers 的合法性、顺序和大小写保留规则。
- **HTTP-FORM-Q05：** body 类型与 Content-Type 的同步规则，以及重复 x-form key 的语义。
- **HTTP-FORM-Q06：** multipart FormData 尚未实现，未来文件字段、文本字段和重复 key 如何进入 Form。

### 9.3 Store 与 repair

- **HTTP-STORE-Q01：** history、favorites、environment、auth、cookie jar 分别是否跨 tab/window 共享，
  哪些需要 Store，哪些需要持久化服务。
- **HTTP-STORE-Q02：** 多 tab catalog、active tab 和 request identity 的 owner。
- **HTTP-STORE-Q03：** secret/auth/cookie 的内存与持久化安全边界；不能因为共享就直接放入普通 UI
  snapshot。
- **HTTP-REPAIR-Q01：** auth challenge、客户端证书、代理或 TLS 问题是否提供显式修复动作；在动作
  未定义前不采用预定义 `repair::Operation`。

## 10. Novel Download：尚未回答的问题

状态：**全部未回答**。当前只确认下载流程不适用预定义 refresh / repair family。

### 10.1 并发、取消与运行 owner

- **NOVEL-RUN-Q01：** 单 active download、取消并替换、队列，还是多个并行下载。
- **NOVEL-RUN-Q02：** Start 发生在已有运行时应拒绝、替换还是排队。
- **NOVEL-RUN-Q03：** 取消检查点在哪里；网络请求、解析和文件写入分别如何停止。
- **NOVEL-RUN-Q04：** 页面关闭或应用退出时取消、等待 drain，还是允许后台继续。
- **NOVEL-RUN-Q05：** 是否需要稳定 `DownloadId`、不可变 request snapshot 和 generation 防止迟到
  completion。
- **NOVEL-RUN-Q06：** 如果采用队列，最大并发、去重和相同 URL 的身份规则。

### 10.2 文件提交、续传与 repair

- **NOVEL-FILE-Q01：** 下载的 commit point 是整本文件、章节、页，还是更细粒度。
- **NOVEL-FILE-Q02：** 取消/失败后的部分文件保留、删除，还是使用 `.part` staging。
- **NOVEL-FILE-Q03：** 重试如何保证不会把已经 append 的内容重复写入。
- **NOVEL-FILE-Q04：** checkpoint 与 resume 粒度；远端来源发生变化时如何识别旧 checkpoint 已失效。
- **NOVEL-FILE-Q05：** 用户修改目标文件、目标名称冲突和 overwrite 策略。
- **NOVEL-REPAIR-Q01：** 更换目录、覆盖、删除部分文件后重试、从 checkpoint 继续等动作中，哪些是
  正式 repair。

### 10.3 Preview、Store 与 Form

- **NOVEL-PREVIEW-Q01：** 是否先展示元数据 preview；preview 包含哪些字段、以什么 URL / source
  identity 绑定。
- **NOVEL-PREVIEW-Q02：** preview 重读时是否保留旧数据，以及如何拒绝旧 URL 的迟到 completion。
- **NOVEL-STORE-Q01：** 是否形成后台下载中心、队列、历史、跨窗口详情；只有出现多个独立消费者时
  才建立 `Store<DownloadCenterState>`。
- **NOVEL-FORM-Q01：** 何时从当前单 URL 输入升级为包含来源、目录、章节/页范围、覆盖和续传策略的
  `DownloadRequestForm`。
- **NOVEL-RETRY-Q01：** 当前单页网络 retry 继续保持 runner 内部控制流，还是有某类失败需要暴露给
  UI；不能为每页短暂重试建立 Operation。

## 11. 明确暂缓或不变的范围

- Jaco MCP runtime 的自定义 Transition 与 status Store 继续暂缓；本轮不再调研或建计划。
- Feiwen 高级查询的产品决定已经关闭，完整owner plan已经创建；Form相关工作包仍等待 Form vNext
  固定精确契约。递归 typed path 等 Form 库能力不再回退到旧 `FormField` / `PartialFormField` 方案。
- `gpui-operation` 不新增第三个预定义 family；不符合 refresh / repair 的流程由应用定义自己的
  `Transition<Message>`。
- `gpui-store` 不新增公共 dispatch / message API；应用在 Store 所有的领域状态上调用 Transition，
  并通过现有 `update` / `update_if` 发布。
- `Entity<Form<M>>` 不进入 Store，也不拥有业务 Task；`prepare` 只捕获验证后的 typed snapshot 与
  revision，I/O 和运行状态继续由应用 owner 管理。
- 本文不修改 `gpui-form`、`gpui-store` 或 `gpui-operation` 的公共 API。
- 本文不开始任何 app 迁移。

## 12. 后续入口

本轮已经创建执行计划，但没有开始代码实施。后续按以下边界推进：

1. Feiwen 已没有待回答的产品问题；HTTP Client 与 Novel Download 的原问题继续原样保留。
2. QueryCatalog 的 `FEI-CATALOG-Q01..Q08` 已进入Feiwen owner plan；实施时落实phase-to-UI、显式
   Load/Refresh/Retry、控件投影与invalidation generation，Q08 SQL scope保持不变。
3. Feiwen数据库 `FEI-DB-Q03` 的文件协议已写入owner plan；实施时只在tempdir/failure injection中验证，
   本轮不实现不可达Retiring或跨进程lease。
4. Conversation owner plan 按已确认的 `Submitting -> Running -> Stopping` 共享状态落实消息与 Task
   owner，并继续调研 `CONV-Q02` 的迟到 completion 防护。
5. HTTP Client 与 Novel Download 在用户回答对应问题前，只保留问题，不预先选择产品语义。
6. Feiwen高级查询 `FEI-ADVFORM-Q01..Q07` 已无待回答的产品问题且owner plan已创建；case/optional
   resolver已经固定，等Form vNext的 `ItemPath`、collection mutation、路径失效、validation与adapter
   contract完成producer验证后进入对应工作包。
