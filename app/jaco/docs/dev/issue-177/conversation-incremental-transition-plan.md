# Issue #177：Conversation 精确消息转换与跨窗口增量同步计划

关联 issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)

状态：**已实施**。自动化验证已通过；可见 UI 按用户要求留给人工测试。

本文接续已经实施的
[Conversation 领域边界与剩余 Operation UI 接入计划](conversation-domain-and-operation-plan.md)。
上一阶段已经建立 Session 级 `ConversationRegistry` 和按 `ConversationId` 复用的共享
`Entity<ConversationModel>`；本计划继续解决以下问题：

- Conversation 变化目前只通过整个 `ConversationModel` 的 `notify` 发布；
- 一条消息变化后，各 DetailView 仍会扫描全部消息并重建 Timeline 投影；
- Runtime 更新 Model 后还会向 DetailView 发送重复的数据变化事件；
- `EntryUpdated` 携带并替换完整 Entry，缺少消息内部的精确变化语义；
- `ProviderStepChanged` 当前未进入 Conversation Data；
- Attachment 还没有对应的领域变化消息；
- 非 `Ready` 状态收到已提交的 Conversation 变化时可能被静默忽略；
- Markdown 虽然已经支持 `push_str`，但它仍依赖全量消息扫描后自行比较新旧字符串。

## 1. 目标

1. Conversation 的所有业务变化统一建模为实现 `Transition<Message>` 的领域消息。
2. `ConversationModel` 继续作为同一 Session、同一 Conversation 的唯一共享数据 owner。
3. 一条 Entry、Run、Provider Step、Tool Invocation 或 Attachment 变化时，只修改对应领域
   对象，不替换整个 Conversation。
4. Transition 返回精确的 `ConversationEffect`，DetailView 根据 effect 只更新对应组件。
5. 同一个 Conversation 在主窗口和临时窗口打开时，两个页面收到相同的数据 effect。
6. 两个页面继续独立持有 Focus、Scroll、Composer、展开状态、Timeline 测量状态和
   Markdown `TextViewState`。
7. Markdown 纯追加只调用 `TextViewState::push_str`；非追加修改才调用 `set_text`。
8. 所有 Operation 状态都必须处理收到的领域事件，但只有精确 `Ready` 可以直接应用局部
   变化。
9. 非 `Ready` 状态不缓存无法安全合并的局部事件，而是取消过时获取并重新读取完整
   Conversation。
10. Conversation 数据只经过
    `Runtime/Service → Registry → ConversationModel → DetailView` 一条发布链路。

## 2. 非目标

- 不共享两个窗口的 View 或 `TextViewState`。
- 不同步窗口滚动位置、焦点、Composer 草稿或展开状态。
- 不为每个 Entry 立即建立独立的全局 Store 或永久 Global。
- 不修改 `gpui-operation` 的生命周期状态、消息或状态转换规则；只让精确
  `Ready` 委托保留领域 Transition 的 Output。
- 不允许 View 直接修改共享 Conversation Data。
- 不使用另一个与 `gpui-operation` 同构的 phase enum。
- 不使用事件队列、attempt id 或 generation id 掩盖不明确的获取顺序。
- 不在本计划阶段执行代码修改、UI 自动化或最终验收。

## 3. 核心规则

### 3.1 “接收事件”与“应用局部变化”分开

所有 Operation 状态都必须接收并明确处理 Conversation 领域事件，不能因为当前不是
`Ready` 就直接丢弃。

局部变化必须建立在完整、可信的 Conversation 基线上，因此只有精确 `Ready` 可以直接执行
领域 Transition：

| 当前状态 | 收到已提交局部变化后的处理 |
| --- | --- |
| `Ready` | 对当前 Conversation 执行 Transition，发布精确 effects |
| `Refreshing` | 取消旧 Refresh，恢复 Ready，再应用局部变化 |
| `Idle` | 保持未启动；未来 Load 必须读取最新完整数据 |
| `Loading` | 取消可能早于该变化的 Load，并重新 Load |
| `Unavailable` | 不拼接局部数据；按当前 active-resource 策略重新 Retry |
| `Retrying` | 取消旧 Retry，并重新 Retry |
| `Degraded` | 保留旧数据和 Problem，不把局部变化伪装成恢复成功；重新 Refresh |
| `RefreshingDegraded` | 取消旧 Refresh，并从 Degraded 重新 Refresh |

这里的事件来自已经成功提交的应用内 Conversation mutation。它说明持久化事实已经变化，
因此正在运行且可能更旧的读取任务不能在随后覆盖新变化。

`Idle` 是否启动仍由调用者决定。只有已经加载、正在加载或正在被 active runtime 使用的
ConversationModel 才因为领域事件主动重新获取。

### 3.2 Operation 与领域 Transition 分层

`gpui-operation` 继续只负责获取生命周期：

```text
Load / Refresh / Retry / Complete / Cancel
```

Conversation 领域类型负责业务变化：

```text
ConversationChanges
  → Conversation
    → ConversationEntry
    → AgentRun
    → ProviderStep
    → ToolInvocation
    → ConversationAttachment
```

不要把 `EntryAppended`、`RunChanged` 等业务消息提升为
`Transition<Message> for refresh::Operation<...>`。`ConversationModel` 先 match
Operation 的精确变体，再把业务消息交给 `Ready` 中的数据。

## 4. 领域消息与 Transition

### 4.1 Conversation 完整数据

`Conversation` 应包含 Detail UI 和运行时变化所需的完整领域集合：

```rust
pub struct Conversation {
    pub summary: ConversationSummary,
    pub project: ProjectSummary,
    pub entries: Vec<ConversationEntry>,
    pub attachments: Vec<ConversationAttachment>,
    pub runs: Vec<AgentRun>,
    pub provider_steps: Vec<ProviderStep>,
    pub tool_invocations: Vec<ToolInvocation>,
}
```

当前缺少的 `provider_steps` 需要进入完整 Conversation。Attachment 也必须有对应变化消息，
不能只在首次完整加载时存在。

### 4.2 精确领域消息

目标消息结构：

```rust
pub struct ConversationChanges(pub Vec<ConversationChange>);

pub enum ConversationChange {
    SummaryChanged {
        summary: ConversationSummary,
    },
    EntryAppended {
        entry: ConversationEntry,
    },
    EntryUpdated {
        entry: ConversationEntry,
        kind: EntryChangeKind,
    },
    EntryRemoved {
        entry_id: ConversationEntryId,
    },
    AttachmentUpserted {
        attachment: ConversationAttachment,
    },
    AttachmentRemoved {
        attachment_id: AttachmentId,
    },
    RunStatusChanged {
        run: AgentRun,
    },
    ProviderStepChanged {
        step: ProviderStep,
    },
    ToolInvocationChanged {
        invocation: ToolInvocation,
    },
    Deleted,
}

pub enum EntryChangeKind {
    TextAppended,
    Replaced,
    StatusChanged,
}
```

持久化层仍可以返回完整 Entry 作为提交后的权威结果。`EntryChangeKind` 只说明变化语义，
不能代替最终数据，也不能要求 UI 根据 delta 猜测完整 Entry。

如果一次数据库 transaction 同时改变 Summary、Entry 和 Tool Invocation，应作为同一个
`ConversationChanges` 批次发布。

### 4.3 分层实现 Transition

```rust
impl Transition<ConversationChange> for &mut Conversation {
    type Output = ConversationEffect;
}

impl Transition<ConversationChanges> for &mut Conversation {
    type Output = Vec<ConversationEffect>;
}

impl Transition<ConversationChanges> for &mut Option<Conversation> {
    type Output = Vec<ConversationEffect>;
}
```

Conversation Transition 负责：

- 按稳定 ID 查找并原地更新目标对象；
- Entry 插入时维护稳定顺序；
- 删除目标对象；
- 把子 Transition 的结果提升为 Conversation effect；
- 保持其他领域对象不变。

当前 Entry 消息由持久化后的完整 `ConversationEntry` 承载。
`EntryAppended` 和 `EntryUpdated` 是提交事实，实际 effect 仍由 Transition 根据当前 Data
决定；例如缺失 Entry 收到 `EntryUpdated` 时会插入并返回 `EntryInserted`，不会在转换前根据
消息类型预估 effect。

不在 Transition 中调用 GPUI、数据库、Task、通知或 View API。

## 5. ConversationModel 输出事件

### 5.1 精确 effect

Transition 的 Output 用于告诉消费者实际发生了什么：

```rust
pub enum ConversationEffect {
    SummaryChanged,
    EntryInserted {
        entry_id: ConversationEntryId,
    },
    EntryChanged {
        entry_id: ConversationEntryId,
        kind: EntryChangeKind,
    },
    EntryRemoved {
        entry_id: ConversationEntryId,
    },
    AttachmentChanged {
        attachment_id: AttachmentId,
    },
    RunChanged {
        run_id: AgentRunId,
    },
    ProviderStepChanged {
        provider_step_id: ProviderStepId,
    },
    ToolInvocationChanged {
        tool_invocation_id: ToolInvocationId,
    },
    Deleted,
}
```

`ConversationModel` 实现 `EventEmitter<ConversationModelEvent>`：

```rust
pub enum ConversationModelEvent {
    Reloaded,
    Changed(Vec<ConversationEffect>),
}
```

- `Reloaded`：Load、Refresh 或 Retry 成功安装完整数据，需要消费者进行一次完整同步；
- `Changed`：已经在 Ready Data 上成功应用局部 Transition，只更新 effect 指向的部分。

一个 `ConversationChanges` 批次只：

1. 更新 Model 一次；
2. 执行领域 Transition 一次；
3. emit 一次 `Changed(effects)`；
4. `cx.notify()` 一次。

### 5.2 Registry 是唯一变化入口

`ConversationRegistry` 负责：

- 取得当前 Session 中共享的 ConversationModel；
- 把提交后的 `ConversationChanges` 路由到该 Model；
- 把 Summary effect 同步到 Session Catalog；
- 在 Active Run 期间保留 Model；
- 不直接通知任何 DetailView。

Runtime 和 Service 不再把同一份数据变化同时发送给 Model 与 View。

## 6. DetailView 增量消费

每个 DetailView：

- 保存共享的 `Entity<ConversationModel>`；
- 订阅 `ConversationModelEvent`；
- 保存窗口独立的 `MessageTextState`、Timeline/ListState、展开状态和 Composer；
- 不再订阅 Runtime 的 Conversation 数据变化事件。

处理规则：

```text
Reloaded
  → 完整同步 Conversation 投影

EntryInserted(id)
  → 创建该消息的 MessageTextState
  → 插入对应 Timeline row

EntryChanged(id)
  → 只读取该 Entry
  → 更新对应 MessageTextState
  → 只重测对应 Timeline row

EntryRemoved(id)
  → 删除对应 MessageTextState 和 Timeline row

Run/ProviderStep/ToolInvocation/Attachment effect
  → 只更新依赖该对象的 row 或 block

SummaryChanged
  → 只更新标题、Catalog 投影和依赖 Summary 的组件
```

两个窗口订阅同一个 ConversationModel，因此会收到相同的 `ConversationModelEvent`。它们分别
更新自己的 View 状态，不需要父子传参，也不互相发送消息。

## 7. Markdown 局部追加

`TextViewState` 仍属于每个 DetailView，不能放入共享 ConversationModel。

收到 `EntryChanged { kind: TextAppended, .. }` 时：

1. 从共享 Model 读取该 Entry 的最新完整数据；
2. 生成该 Entry 的最新 Markdown source；
3. 验证新 source 是否以当前 source 为前缀；
4. 是纯追加时调用 `TextViewState::push_str(delta)`；
5. 如果变化语义或最终 source 不满足纯追加，则调用 `set_text`；
6. 只 remeasure 对应 Timeline row。

`EntryChangeKind::TextAppended` 是优化提示，不是正确性前提。最终 source 不满足追加条件时必须
安全退回 Replace，不能拼接出错误 Markdown。

## 8. Runtime 事件收口

`ConversationRuntimeEvent` 最终只保留运行时职责，例如：

- Run started；
- Run finished；
- runtime recovery 状态；
- 当前 run 的停止能力；
- 只属于运行时、没有进入 Conversation Data 的瞬时错误。

以下数据变化不再由 DetailView 直接订阅：

- Conversation committed；
- Entry appended/updated；
- Provider Step changed；
- Tool Invocation changed；
- Tool approval changed。

这些变化全部转换成 `ConversationChanges` 并经过 Registry 和 ConversationModel。

现有只携带 ID、随后要求 View 完整 Refresh 的 Runtime 事件需要逐步替换为提交后的完整领域
结果。只有无法得到权威提交结果的异常路径才允许把 Model 标记为需要重新加载。

## 9. 实施工作包

### WP-01：补齐 Conversation 领域数据与消息

涉及：

- `crates/jaco-core/src/domain.rs`
- `crates/jaco-db/src/records/*`
- `crates/jaco-db/src/repository/*`
- `crates/jaco-conversation/src/*`

内容：

1. `Conversation` 纳入 Provider Steps。
2. 补齐 Attachment、删除和精确 Entry 变化消息。
3. transaction commit 返回完整、按批次组织的领域变化。
4. 为领域类型分层实现 `Transition<Message>`。

### WP-02：ConversationModel 精确事件

涉及：

- `app/jaco/src/features/conversation/model.rs`
- `app/jaco/src/features/conversation/registry.rs`

内容：

1. Model 对 Ready Data 执行批量 Transition。
2. Transition Output 转为 `ConversationModelEvent`。
3. 每批变化只 notify/emit 一次。
4. 实现所有非 Ready 状态的显式 invalidation/reload 规则。
5. Registry 统一维护 Catalog Summary 投影。

### WP-03：Runtime 数据事件收口

涉及：

- `crates/jaco-agent/src/runtime/*`
- `crates/jaco-agent/src/persistence/*`
- `app/jaco/src/features/conversation/runtime.rs`

内容：

1. 所有成功 transaction 发布权威 `ConversationChanges`。
2. 删除 Runtime → DetailView 的重复数据变化链路。
3. Runtime 只保留运行时生命周期与瞬时错误事件。
4. 移除只携带 ID 并触发整份 Conversation Refresh 的正常路径。

### WP-04：DetailView 精确消费

涉及：

- `app/jaco/src/components/chat/detail.rs`
- `app/jaco/src/components/chat/detail/timeline.rs`
- 相关 message、tool block 和 attachment 组件

内容：

1. DetailView 订阅 `ConversationModelEvent`。
2. `Reloaded` 执行完整同步。
3. `Changed` 只修改对应消息、row 或 block。
4. Markdown 追加保留现有 `push_str`，并只重测对应 row。
5. 删除 Runtime Conversation data subscription 和重复全量同步。

### WP-05：清理与文档同步

内容：

1. 删除不再使用的粗粒度 Conversation data Runtime events。
2. 删除 effect 已覆盖的完整消息扫描路径。
3. 更新 Conversation 领域计划的实施状态和最终数据流。
4. 保留全量 Reload 作为 Load/Refresh/Retry 成功后的明确路径。

## 10. 行为测试计划

实施时至少覆盖：

1. 同一 Session、同一 Conversation ID 的两个 View 使用同一个 ConversationModel。
2. Ready 收到 Entry change 后只修改对应 Entry。
3. 一个 `ConversationChanges` 批次只产生一次 Model notification/event。
4. 两个 View 都收到同一个 Conversation effect。
5. 两个 View 的滚动、展开和 Markdown `TextViewState` 仍彼此独立。
6. Markdown 纯追加保持原 `TextViewState` Entity，并调用追加路径。
7. 非追加变化保持同一条消息身份，但使用 replace 路径。
8. 修改一条消息不会重建或重测其他消息。
9. Refreshing 收到变化时旧 Refresh 不能覆盖新数据。
10. Loading/Retrying 收到变化时会重新读取最新完整 Conversation。
11. Degraded 收到局部变化不会错误进入 Ready。
12. Provider Step、Tool Invocation 和 Attachment 变化能到达对应 View。
13. Runtime 不再让同一次数据变化通过 Model 和 Runtime subscription 重复处理。
14. Load/Refresh/Retry 完成后的 `Reloaded` 能正确重建完整投影。

## 11. 实施结果

- 完整 Conversation 已包含 Provider Step，附件也进入提交后的领域变化批次。
- `ConversationChanges` 通过 `Transition` 原地更新领域对象并返回实际
  `ConversationEffect`；精确 `Ready` 会原样返回领域 Transition 的 Output。
- Session 级 `ConversationRegistry` 是数据变化的唯一入口；同一 Conversation 的主窗口与临时
  窗口复用同一个 `Entity<ConversationModel>`。
- `ConversationModel` 对每批变化只 emit/notify 一次；Refreshing 会先取消旧读取再应用，
  Loading/Retrying 会重新读取，Degraded 系列不会把局部变化伪装成 Ready。
- Runtime 到 DetailView 的重复 Conversation 数据事件已经删除；Runtime 只保留 Run started /
  finished 生命周期事件。
- DetailView 根据 effect 更新目标 Entry、Run、Attachment 或结构行；Markdown 纯追加继续复用
  窗口私有的 `TextViewState::push_str`，非追加才 replace。
- 可见 UI 仍由用户人工验证，本次未运行 UI 自动化。

实施验证：

- `cargo fmt --all`
- `cargo check -p gpui-operation -p jaco-core -p jaco --locked`
- `cargo test -p gpui-operation -p jaco-core --locked`
- `cargo test -p jaco-core -p jaco-db -p jaco-conversation -p jaco-agent --locked`
- `cargo test -p jaco --locked`
- `git diff --check`
