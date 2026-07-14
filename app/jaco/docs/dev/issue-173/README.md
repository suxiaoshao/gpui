# Issue #173：ConversationEntry 与 AgentRun 分层重建

关联 issue：[suxiaoshao/gpui#173](https://github.com/suxiaoshao/gpui/issues/173)

## 1. 状态与范围

本文档集记录实施方案与当前实现结果。实现位于分支
`codex/173-jaco-error-message-order`，基线为 `origin/main@ad7c9ad`；`third_party/lucide`
已从本地 Git 对象库恢复，Jaco bundle、隔离数据库启动与窗口交互 smoke 均已验证。

### 目标

1. 将用户可见、可排序、可搜索、可复制或可进入模型历史的事实统一建模为
   `ConversationEntry`。
2. 将 `AgentRun` 保留为后台执行聚合，不再让没有 Entry 的终态 Run 参与持久化时间线排序。
3. 将 Run 的触发 Entry 与最终 Entry 提升为数据库外键；终态 Run 不允许缺少最终 Entry。
4. 失败、取消、max-steps、无正文完成、审批请求和审批决定都持久化为明确的 Entry。
5. 保留一次用户发送对应一条 User Entry、Entry 内含多个有序 `ContentPart`、附件实体单独存储的多模态模型。
6. 删除 UI 依据 `AgentRun.error` 临时合成终态内容、把无 Entry Run 追加到底部、以及从
   `tool_invocations.approval_json` 派生审批行的逻辑。

### 用户决定

- 应用尚未发布且没有真实用户；不兼容旧数据库、不回填历史数据、不保留 legacy
  projection，也不为旧序列增加兜底。
- 设计以最终模型为准，不以减少改动量为目标。
- `Project -> Conversation -> ConversationEntry` 是产品时间线；`AgentRun` 是独立运行时实体。
- 一条 User Entry 可包含文字、图片、文件等多个 `ContentPart`，不能拆成多条 Entry。
- Agent 最终回答只存于最终 Entry；Run 只保存最终 Entry 的引用，不复制正文。

### 非目标

- 不改变 provider/Rig API、请求协议、模型能力或 provider 配置。
- 不改变附件文件写入目录、文件格式或图片预览设计。
- 不重新设计 conversation/sidebar/search UI，不新增通用组件或图标。
- 不升级依赖，不修改 `Cargo.lock`。
- 不迁移、修复或兼容现有 `jaco.sqlite3` 数据。

### 数据库重建策略

- 将 fresh schema 直接改写为目标 schema；`SCHEMA_VERSION` 固定为 `1`，不随本次 schema
  重建设计递增。
- 不增加 schema 版本升级、旧版本识别或兼容错误分支。开发阶段的现有数据库由开发者在运行前
  手工删除，不进入应用运行时的数据处理流程。
- 实施与手工验收前关闭 Jaco，删除本地 `jaco.sqlite3` 后重新启动，由 fresh schema
  创建空数据库；不编写 `0002` 数据迁移，不重排旧 `seq`。
- 不自动删除数据库文件。数据库删除是明确的开发者操作，不属于运行时恢复逻辑。

## 2. 文档地图

- [data-model.md](data-model.md)：目标 Rust 类型、SQLite schema、外键、约束、repository
  API、附件与审批模型。
- [runtime-lifecycle.md](runtime-lifecycle.md)：当前根因、发送/流式/失败/取消/恢复/审批流程、
  UI 投影和测试矩阵。

## 3. 当前证据快照

| 当前事实 | 证据 | 后果 |
| --- | --- | --- |
| `conversation_items` 按 `conversation_id, seq` 查询 | `crates/jaco-db/src/repository.rs::conversation_items` | Entry 已经是持久化顺序事实源 |
| `agent_runs` 单独按 `created_at` 查询 | `crates/jaco-db/src/repository.rs::agent_runs_for_conversation` | 两套顺序只能由 UI 临时合并 |
| 普通 provider/prompt 失败只更新 `agent_runs.error_json`，`output` 为 `None` | `crates/jaco-agent/src/runtime.rs` 的 `Err(error)` 分支 | 零输出失败没有 Entry 和 `seq` |
| setup failure 会追加 Error item 并设置 `final_item_id` | `AgentRuntime::record_setup_failed_started_run` | 同一种终态存在两套持久化路径 |
| 空 streaming accumulator 的 `finish(Failed, None)` 不创建 item | `crates/jaco-agent/src/runtime/streaming.rs` | 网络建立阶段失败稳定复现 orphan Run |
| timeline 先遍历 items，再把 unseen runs 追加到末尾 | `app/jaco/src/components/conversation_detail/timeline.rs::build_rows` | 较早错误沉到后续对话之后 |
| terminal fallback 从 `run.error` 临时生成 Markdown | `app/jaco/src/components/conversation_detail/message.rs::agent_run_terminal_fallback_markdown` | UI 出现第二内容源，且没有 Entry identity |
| 审批行由 `tool_invocations.approval_json` 临时派生 | `timeline.rs::items_with_derived_approvals` | 审批不是可独立排序的持久化 Entry |
| User message 已是 `Message { role: User, content: Vec<ContentPart> }` | `crates/jaco-core/src/payloads.rs` 与 `app/jaco/src/state/conversations.rs` | 多模态用户输入无需重建 |
| repository 已有 item + run 原子写入方法 | `FreshRepository::append_conversation_item_and_update_agent_run` | 可重构为统一终态事务，无需新增依赖 |

## 4. 架构决定

### D-01：统一命名为 ConversationEntry

`ConversationItemId/Kind/Status/Payload/Record/NewConversationItem`、数据库表
`conversation_items`、事件 `ConversationItemAppended/Updated` 和 `AgentStep::ConversationItem`
全部重命名为 Entry 对应名称。`ConversationEntry` 表示 Conversation 内唯一有序事实；
不新增 `Turn`、`Message` 或 `TimelineEntry` 第二抽象。

### D-02：AgentRun 是执行聚合，不是时间线来源

Run 继续拥有 provider/model/runtime snapshot、状态、provider steps、tool invocations、审批当前
状态、error 和时间戳。UI 可用 Run 渲染耗时与展开状态，但行的位置只能由其 Entries 决定。
持久化 timeline 不允许追加“没有 Entry 的终态 Run”。

### D-03：关系字段必须归一化

- `agent_runs.trigger_entry_id` 是非空外键，指向触发本次 Run 的 User Entry。
- `AgentRunInput` 删除 `user_item_id`；同一信息不能同时存在于外键列和 JSON。
- `agent_runs.final_entry_id` 与 `stopped_reason` 取代 `output_json`。
- Rust 层仍可用 `AgentRunOutput` 表达终态输出，但 `final_entry_id` 改为非 `Option`，其值由列构造。
- `AgentRunRequest.user_item_id` 重命名为 `trigger_entry_id`。

### D-04：每个终态 Run 都有最终 Entry

| Run 结果 | 最终 Entry |
| --- | --- |
| 正常完成且有 assistant 正文 | `Message { role: Assistant }` |
| failed，包括 setup/provider/transport/tool-loop/recovery | `Error(RunErrorPayload)`，始终新追加 |
| canceled 且已有部分 assistant 正文 | 最后一条 assistant Entry |
| canceled 且没有 assistant 正文 | typed `Status(Canceled)` |
| max steps 且已有 assistant 正文 | 最后一条 assistant Entry |
| max steps 且没有 assistant 正文 | typed `Status(MaxStepsReached)` |
| provider 正常结束但没有任何可作为最终输出的正文 | typed `Status(CompletedWithoutOutput)` |

终态 Entry 的写入、`agent_runs.status/error/final_entry_id/stopped_reason` 更新必须处于同一 SQLite
immediate transaction。失败后的安全重试必须先读取 Run；如果 Run 已终态，返回现有记录且不得追加
第二条终态 Entry。

### D-05：Status 持久化语义，不持久化本地化标签

现有 `StatusItem { label, message }` 改为 typed `ConversationStatusEntry { code, message }`。
`code` 使用 `Canceled | MaxStepsReached | CompletedWithoutOutput`；UI 根据 code 读取 Fluent 文案。
数据库不保存 `Canceled`/`已取消` 这类 locale-dependent label。

### D-06：审批是 Entry，审批 JSON 是运行状态

`tool_invocations.approval_json` 保留为当前审批状态，用于并发控制、恢复和幂等判断；
`ApprovalRequest` 与 `ApprovalDecision` Entry 是不可变时间线事实。请求、决定与 tool invocation
状态更新必须原子提交。UI 删除派生审批 Entry 的逻辑。

### D-07：用户多模态消息保持单 Entry

一次发送只创建一条 `Message { role: User }` Entry。文字、图片、文件、音频与一般附件继续按
`Vec<ContentPart>` 有序存储；附件二进制/路径/元数据继续位于 `attachments`，Entry 只保存
`attachment_id`。附件记录与 User Entry 继续在同一事务提交。

## 5. 目标文件与模块

### 新增文档

- `app/jaco/docs/dev/README.md`
- `app/jaco/docs/dev/issue-173/README.md`
- `app/jaco/docs/dev/issue-173/data-model.md`
- `app/jaco/docs/dev/issue-173/runtime-lifecycle.md`

### 重命名

- `crates/jaco-agent/src/persistence/conversation_items.rs` ->
  `crates/jaco-agent/src/persistence/conversation_entries.rs`

### 修改：领域与数据库

- `crates/jaco-core/src/payloads.rs`
- `crates/jaco-core/src/lib.rs`
- `crates/jaco-db/src/migrations.rs`
- `crates/jaco-db/src/schema.rs`
- `crates/jaco-db/src/models.rs`
- `crates/jaco-db/src/records.rs`
- `crates/jaco-db/src/repository.rs`
- `crates/jaco-db/src/error.rs`
- `crates/jaco-db/src/tests.rs`

### 修改：Agent runtime

- `crates/jaco-agent/src/types.rs`
- `crates/jaco-agent/src/persistence.rs`
- `crates/jaco-agent/src/persistence/conversation_entries.rs`
- `crates/jaco-agent/src/persistence/tool_hook.rs`
- `crates/jaco-agent/src/runtime.rs`
- `crates/jaco-agent/src/runtime/finalization.rs`
- `crates/jaco-agent/src/runtime/streaming.rs`
- `crates/jaco-agent/src/provider_models.rs`
- `crates/jaco-agent/src/history.rs`
- `crates/jaco-agent/src/runtime/tests.rs`

### 修改：Jaco app 与 UI

- `app/jaco/src/state/conversations.rs`
- `app/jaco/src/state/conversation_runtime.rs`
- `app/jaco/src/state/attachments.rs`
- `app/jaco/src/foundation/conversation_format.rs`
- `app/jaco/src/components/conversation_detail.rs`
- `app/jaco/src/components/conversation_detail/timeline.rs`
- `app/jaco/src/components/conversation_detail/message.rs`
- `app/jaco/src/components/conversation_detail/attachments.rs`
- `app/jaco/src/components/conversation_detail/tool_blocks.rs`
- `app/jaco/src/foundation/i18n.rs`
- `app/jaco/locales/en-US/main.ftl`
- `app/jaco/locales/zh-CN/main.ftl`

实现时必须再次执行以下 residual scan；如出现额外结果，只允许修改实际引用旧模型的文件：

```bash
rg -n "ConversationItem|conversation_items|final_item_id|user_item_id" \
  crates/jaco-core/src crates/jaco-db/src crates/jaco-agent/src app/jaco/src
```

## 6. 上游复用审计

| 本地实现 | 已有能力 | 决定 |
| --- | --- | --- |
| `append_conversation_item_and_update_agent_run` | 已有 SQLite immediate transaction | Adapt：重命名并收口为幂等 `finish_agent_run`，不新建事务框架 |
| `update_tool_invocation_approval` 与 combined tool update | 已有 approval 状态机和原子 tool 更新 | Adapt：同事务追加审批 Entry |
| `StreamingOutputAccumulator` | 已管理流式 Entry 创建与更新 | Retain：只改 Entry 命名；终态 Error 由 Run finalizer 统一负责 |
| `TextViewState` 增量更新、GPUI `ListState` | 已满足渲染与重测量 | Reuse directly：不创建新列表/滚动组件 |
| `items_with_derived_approvals` | UI 本地派生持久化事实 | Delete：审批改为 repository/runtime 持久化 Entry |
| `agent_run_terminal_fallback_markdown` | UI 从 Run error/status 合成内容 | Delete：最终 Entry 是唯一内容源 |
| unseen terminal Run append loop | 无 Entry 时的排序兜底 | Delete：终态 Run 必须有 final Entry |

无依赖变化，因此 dependency evidence、release note、MSRV、feature、TLS/native 和 lockfile
调查均为 `No change`。

## 7. 实施工作包

依赖关系：`WP-10 -> WP-20 -> WP-30 -> WP-40 -> WP-50 -> WP-60`。

### WP-10：重建 Entry 领域类型

**结果**：完成 Item -> Entry 的完整命名和 typed Status，编译错误清楚暴露所有旧调用点。

**文件**：`crates/jaco-core/src/payloads.rs`、`crates/jaco-core/src/lib.rs`。

**实施**：按 [data-model.md](data-model.md) 的 Rust contract 重命名类型；从
`AgentRunInput` 删除 `user_item_id`；将 `AgentRunOutput.final_entry_id` 设为非可选；更新
`AgentRunEvent`、serde tag 和 payload `kind()`/`search_text()`。

**测试**：更新 payload serde round-trip、status code、search text 和 unknown-field tests。

**完成条件**：core 中无 `ConversationItem`、`final_item_id`、`user_item_id` 残留。

### WP-20：重建 fresh SQLite schema 与 repository contract

**结果**：版本号保持为 `1` 的 fresh schema 直接表达 Entry/Run 关系和终态约束。

**文件**：全部 `crates/jaco-db` 列表文件。

**实施**：改写 fresh schema、Diesel schema/models/records/conversions；实现
`finish_agent_run`、approval-with-entry 事务和同 conversation/role/link 验证；禁止终态重复写；
将 `SCHEMA_VERSION` 设为 `1`，不实现旧 schema 检测、迁移或兼容错误。

**测试**：详见 [data-model.md](data-model.md) 数据库测试表。

**完成条件**：fresh DB schema SQL、Diesel schema 和 record mapping 一致；外键检查开启；没有
版本升级或兼容分支。

### WP-30：统一 AgentRun 终态状态机

**结果**：所有完成、失败、取消、max-steps 与 recovery 路径使用同一 finalizer。

**文件**：全部 `crates/jaco-agent` 列表文件。

**实施**：建立 `PersistenceContext::finish_run`，按结果选择 existing/new final Entry，调用
repository 原子事务；setup failure、provider error、blocking/streaming error、取消和 recovery
不能直接调用通用 `update_agent_run_status` 写终态。

**测试**：详见 [runtime-lifecycle.md](runtime-lifecycle.md) runtime 测试矩阵。

**完成条件**：任何终态 `AgentRunRecord.output` 都是 `Some` 且 final Entry 存在；failed Run 的
final Entry 必为 Error；重复 finalization 不产生重复 Entry。

### WP-40：持久化审批 Entry

**结果**：审批请求/决定进入统一 Entry 序列，tool invocation approval JSON 只保存当前状态。

**文件**：`crates/jaco-agent/src/persistence/tool_hook.rs`、repository/records/tests、
`crates/jaco-core/src/payloads.rs`。

**实施**：请求审批时原子写 `ApprovalRequest` + AwaitingApproval；人工/自动决定时原子写
`ApprovalDecision` + approval state；denied/canceled tool result 保持后续独立 Entry。

**完成条件**：一次状态转换只对应一条审批 Entry；重复决定被拒绝且不追加 Entry。

### WP-50：改造 Jaco timeline 投影

**结果**：持久化 Entry 决定全部历史顺序，Run 只为 Entry 组提供状态/耗时元数据。

**文件**：全部 Jaco app 与 UI 列表文件、两个 locale 文件。

**实施**：`build_rows` 只遍历 Entries；删除 derived approval 与 unseen terminal Run 追加；仅允许
当前 non-terminal active Run 在没有首条 Entry 前显示尾部 ephemeral processing row，且该 row
不进入 copy/search/history；final Markdown 只从 `final_entry_id` 读取；typed Status 由 Fluent
本地化。

**完成条件**：重新加载 conversation 后不需要 `run.error` fallback；旧错误沉底代码不存在；
用户文字+图片+文件仍是一条 User Entry。

### WP-60：全链路验证与开发数据库重建

**结果**：完成格式、focused/full tests、clippy、三平台 CI 覆盖和实际 Jaco UI 验证。

**实施**：关闭 Jaco，显式删除本地开发数据库，重新启动生成目标 fresh schema；构造零输出 provider
错误、部分输出错误、取消、重试、审批与多模态消息场景。

**完成条件**：所有命令与手工证据满足第 9 节；没有 compatibility/migration/fallback 代码。

## 8. 系统面决策

| 系统面 | 决定 |
| --- | --- |
| 文件/模块 | 仅按第 5 节修改；一个 Rust 文件重命名；不新增 `mod.rs` |
| UI 组件 | 继续使用 GPUI `ListState`、`TextViewState`、现有 detail blocks；无自定义通用组件 |
| 数据流 | Entry 是持久化历史唯一来源；Run 是执行状态；active zero-entry row 仅为内存态尾部 projection |
| 状态所有权 | SQLite 持久化 Entry/Run；`ConversationRuntimeStore` 只持有当前 active task/error 通知；页面只缓存 `TextViewState` |
| 数据库 | `SCHEMA_VERSION = 1`，直接改写 fresh schema；无 migration/backfill/版本兼容；终态事务和 approval 事务原子化 |
| 数据获取 | 继续通过 `FreshRepository::conversation_timeline_records` 读取本地 SQLite；无网络分页/cache/TTL 变化 |
| 错误/恢复 | provider/transport/setup/recovery error 均为 Error Entry；DB 写失败不得先发 terminal runtime event |
| 取消 | 有部分 assistant 时引用它；无正文时追加 typed Canceled Status Entry |
| 重试 | 多个 Run 可共享同一 `trigger_entry_id`；各自 Entries 按 conversation `seq` 排序 |
| 附件 | 一条 User Entry + 多个 ContentPart + 独立 AttachmentRecord；无格式或目录变化 |
| 搜索 | `conversation_entries.search_text` 继续聚合文字；附件二进制不进入全文搜索 |
| icon/assets | No change；不新增 icon/SVG/runtime/bundle asset |
| i18n | 新增 typed status 三个 key；删除 terminal fallback key；Error label 使用现有 `conversation-error` |
| 依赖 | No change；不改 manifest/lockfile/features/MSRV |
| 平台 | No platform-specific implementation；由现有 macOS/Linux/Windows CI 覆盖 |

## 9. 交叉验证

实施完成后必须实际运行：

```bash
cargo fmt
cargo test -p jaco-core
cargo test -p jaco-db
cargo test -p jaco-agent
cargo test -p jaco
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

数据库与 residual 验证：

```bash
rg -n "ConversationItem|conversation_items|final_item_id|user_item_id" \
  crates/jaco-core/src crates/jaco-db/src crates/jaco-agent/src app/jaco/src
sqlite3 -readonly <fresh-db> ".schema conversations" ".schema conversation_entries" ".schema agent_runs"
sqlite3 -readonly <fresh-db> "PRAGMA foreign_key_check;"
```

手工 UI 验证：

1. 新建临时 custom OpenAI-compatible provider，将 base URL 指向本机未监听端口并使用 dummy key，
   主动制造首 token 前 connection-refused；发送消息 A，确认 Error 显示在 A 后，再发送消息 B，确认
   Error 不移动到 B 后；重启 Jaco 后再次确认顺序不变。此步骤只用于手工 smoke test，自动测试使用
   `FailBeforeFirstTokenModel`，不依赖本机网络状态。
2. provider 输出部分文本后失败：展开显示部分文本与 Error，折叠以 Error 为最终内容。
3. retry：失败 Run 与成功 Run 都位于同一触发 User Entry 后，顺序符合 Entry `seq`。
4. cancel before output：显示本地化 Canceled Status。
5. 人工审批 approve/deny：请求与决定各出现一次，重载后顺序不变。
6. 同时发送文字、两张图片和一个文件：只产生一条 User Entry，content part/attachment 引用完整。
7. 退出并重启后 recovery：interrupted Run 生成 Error Entry，不出现 orphan Run。

## 10. 执行交接审计

- 架构、schema、命名、兼容策略、终态 Entry 选择、审批持久化和 UI source of truth 已确定。
- 无“实现时再决定”或广泛研究任务。
- 无新增/升级依赖，因此无 release gate。
- 每个新增/变更类型、repository 事务、失败/取消/retry/recovery 路径均在子文档给出 contract。
- 不允许以保留旧数据库、减少 rename 或兼容旧 JSON 为理由恢复旧字段或增加兜底。
