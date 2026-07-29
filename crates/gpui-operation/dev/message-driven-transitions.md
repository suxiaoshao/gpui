# gpui-operation 消息驱动状态转换实施计划

## 1. 状态与范围

- GitHub issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)。
- 实施分支：`codex/177-jaco-catalog-startup-model-selection`。
- 文档位置：`crates/gpui-operation/dev/message-driven-transitions.md`。
- 当前阶段：消息接口已实现，等待最终包级验证与 Jaco 接入。
- 兼容策略：crate 仍处于 `0.1.0`，直接修正错误设计，不提供 deprecated alias 或兼容
  wrapper。
- 发布关系：本 crate 与 `gpui-store` 保持独立；Jaco Resource 迁移在两个 crate 的 API
  确认后单独执行。

### 1.1 目标

1. 提供纯同步、消息驱动的状态转换：

   ```text
   current named state + owned message -> next named state
   &mut complete Operation + owned message -> ()
   ```

2. 在库内预定义两个可长期保存的完整 runtime enum：

   ```rust
   refresh::Operation<Data, Problem, Task>
   repair::Operation<Data, Problem, Repair, Task>
   ```

3. 让 Entity、Global、Store 或普通字段可以直接保存 library Operation，不再由每个应用重复
   定义八个 variant 和所有转换 match。
4. 让 running state 按值拥有 Task、准确的 previous state，以及 repair family 中正在执行的
   Repair。
5. 取消时恢复准确 previous state；完成时按 Result 产生新的稳定状态。
6. 不要求 Data、Problem、Repair 或 Task 实现 `Clone`、`PartialEq`、`Default`、`Send`
   或 `Sync`。
7. 调用者继续决定 task 构造/runtime、completion 路由、owner、通知和具体 Repair。
8. 让 `Ready` 中的业务 Data 通过具名消息原地更新，不公开 `data_mut`，也不要求
   `Data: Clone`。
9. 默认不引入日志依赖；启用 `tracing` feature 后记录非法 runtime 消息。
10. 保持英文默认文档，并同步中文 README 与完整指南。
11. 同步完成的工作使用独立 `Settle(Result)` 消息：从 `Idle` 进入 `Ready` 或
    `Unavailable`，从 `Ready` 进入 `Ready` 或 `Degraded`；异步 Task 的结果继续使用
    `Complete(Result)`。

### 1.2 非目标

- 不定义 `OperationSource`、Resource trait 或跨 family 的万能 `Operation<S>`。
- 不 spawn、不 await Task，也不选择 GPUI backend runtime 或 Tokio runtime。
- 不提供 Entity、Global、Store adapter；owner 只保存两个通用 enum。
- 不提供 attempt id、generation 或 stale-completion acceptance。
- 不自动启动、刷新、重试、修复或取消。
- 不建立资源依赖图，不提供 `Waiting`，不编排多个 Operation。
- 不执行持久化、事务、回滚或 repair 副作用。
- 不建模 partial progress；只有最终 `Result` 进入 completion。
- 不使用宏生成状态机。

### 1.3 已冻结需求

| ID | 需求 |
| --- | --- |
| OP-R1 | `Transition<Message>` 消费具名状态与消息，返回精确具名输出 |
| OP-R2 | `refresh` 与 `repair` 是两个独立 family，不使用 `Repair = ()` 混用 |
| OP-R3 | Problem 只要求 `std::error::Error` |
| OP-R4 | running state 拥有 Task 与 Previous；repair running state还拥有 Repair |
| OP-R5 | Task drop 是库唯一使用的取消原语 |
| OP-R6 | 库提供两个完整 runtime `Operation` enum；应用不得重复定义同构 enum |
| OP-R7 | 具名状态和 runtime enum 只公开借用 getter，不公开 Task 或可变 payload 引用 |
| OP-R8 | 两个完整 enum 通过 `Transition<Message> for &mut Operation` 接收消息，不提供命令式 runtime 方法 |
| OP-R9 | 调用者按当前 variant 显式选择 `Load`、`Refresh`、`Retry` 或 `Repair`，库不替调用者合并语义 |
| OP-R10 | 非法 runtime 消息先恢复原状态，再可选 debug，最后消费并 drop owned 消息 |
| OP-R11 | completion/cancel 先安装最终合法状态，再 drop Task、Repair 和旧 payload |
| OP-R12 | `new/default` 只产生 Idle，不给 payload 增加 `Default` bound |
| OP-R13 | `Ok(data)` 一律成功；空集合、空字符串等领域空值不会变成 Problem |
| OP-R14 | 默认 feature 只使用 `std`；可选 `tracing` feature 提供非法消息 debug；GPUI 仅作为 dev dependency |
| OP-R15 | `&mut Ready<Data>` 把业务消息委托给 `&mut Data` 的 `Transition`，只允许 Ready -> Ready 更新 |
| OP-R16 | 不保留 `start_fetch`、`start_repair`、`complete`、`cancel`、`can_*` 或 `Rejected` 兼容层 |
| OP-R17 | `Settle(Result)` 处理 `Idle` 或 `Ready` 中同步完成的工作；`Ready + Err` 保留旧 Data 并进入 `Degraded`；`Complete(Result)` 只处理 running state，取消后迟到的 `Complete` 不得结算 `Idle` |

## 2. 当前证据

### 2.1 源码

| 文件 | 当前责任 |
| --- | --- |
| `src/message.rs` | `Settle`、`Load`、`Refresh`、`Retry`、`Repair`、`Complete`、`Cancel` |
| `src/transition.rs` | named-state consuming 与 runtime `&mut` 共用的 `Transition<Message>` |
| `src/refresh.rs` | refresh 具名状态、转换矩阵、`refresh::Operation` 与 `Phase` |
| `src/repair.rs` | repair 具名状态、转换矩阵、`repair::Operation` 与 `Phase` |
| `src/lib.rs` | crate 文档、公共消息、`Transition` 与两个 family |
| `Cargo.toml` | 默认关闭、可选启用的 `tracing` feature |

旧 `OperationSource + Source-coupled Operation + opaque Completion` 已不存在。本轮纠正的
问题不是恢复旧 façade，而是在当前无 Source 的消息模型上提供两个 family-specific runtime
enum。

### 2.2 测试

| 文件 | 覆盖范围 |
| --- | --- |
| `tests/refresh.rs` | refresh 具名转换、runtime 八态、非法消息 drop、Ready 业务消息与无 Clone/Send bound |
| `tests/repair.rs` | repair 具名转换、runtime 八态、Repair 所有权、非法消息与 drop |
| `tests/gpui_task.rs` | 真实 GPUI Task 取消、completion 回写 owner、自替换安全 |

### 2.3 GPUI 与依赖

- workspace 的 `gpui` 来自锁定的 Zed git revision。
- `gpui::Task<T>` drop 会取消未 detach 的 task，且 Task 不实现 `Clone`。
- `Context<T>::spawn` 使用 WeakEntity 路由 completion。
- `App::spawn` 可在完成时按类型重新查找普通 Global 或 typed-global Store。
- 默认 feature 下 `gpui-operation` 没有 normal dependency；`tracing` 只在对应 feature
  开启时进入依赖图。GPUI 只在 dev dependency 中用于行为测试。
- Jaco 接入时开启 `gpui-operation/tracing`，让非法状态消息成为可诊断的开发错误。

## 3. 公开设计

### 3.1 两层 API

库同时提供两层互补 API：

1. 具名状态 + `Transition<Message>`：
   - 直接持有某个精确状态时使用；
   - `Transition` 消费具名状态和 owned 消息；
   - 非法转换因为没有对应 trait impl 而不能编译。
2. family `Operation` enum：
   - Entity、Global、Store 和普通字段长期保存时使用；
   - 为 `&mut Operation` 实现同一个 `Transition<Message>` trait，调用者不需要
     `mem::take` 或临时占位状态；
   - 库内完成 variant match、owned payload 移动、恢复和 drop 顺序；
   - 非法 runtime 消息保持原状态，消息本身被消费并 drop。

不要为完整 enum 实现 consuming 的 `Transition<Message> for Operation`。长期 owner 无法从
`&mut self` 字段中直接移出 owned Operation，这会把 `mem::take` 重新推给 Entity、Global
或 Store。完整 enum 只实现 `Transition<Message> for &mut Operation`；具名状态继续实现
consuming Transition。

```rust
let mut operation = refresh::Operation::new();
operation.transition(Settle(sync_result));
operation.transition(Load(task));
operation.transition(Complete(result));
operation.transition(Cancel);
```

runtime Transition 的 `Output = ()`。消息是否合法由当前 variant 决定；应用应在构造 Task
之前根据自身状态流确保消息合法，非法消息是可诊断的开发错误，而不是需要兼容返回值的业务
分支。

### 3.2 非法消息与 tracing

完整 enum 收到非法消息时执行固定顺序：

```text
取出当前 enum
  -> 确认消息对当前 variant 非法
  -> 写回原 enum
  -> tracing feature 开启时 debug
  -> drop owned message
```

`debug` 只记录 family、稳定 phase 与消息类型，不格式化 Data、Problem、Repair 或 Task，
因此不增加 `Debug` 等 bound。日志必须发生在原状态写回之后，避免 tracing subscriber
重入时观察临时 Idle。

Jaco 在依赖中开启该 feature：

```toml
gpui-operation = { workspace = true, features = ["tracing"] }
```

crate 不提供 `Rejected`，也不保留命令式 runtime 兼容层：

```text
start_fetch / start_repair / complete / cancel / can_* / Rejected
```

### 3.3 Refresh runtime enum

```rust
pub enum refresh::Operation<Data, Problem: Error, Task> {
    Idle(refresh::Idle),
    Loading(refresh::Fetching<refresh::Idle, Task>),

    Ready(refresh::Ready<Data>),
    Refreshing(
        refresh::Fetching<refresh::Ready<Data>, Task>,
    ),

    Unavailable(refresh::Unavailable<Problem>),
    Retrying(
        refresh::Fetching<refresh::Unavailable<Problem>, Task>,
    ),

    Degraded(refresh::Degraded<Data, Problem>),
    RefreshingDegraded(
        refresh::Fetching<
            refresh::Degraded<Data, Problem>,
            Task,
        >,
    ),
}
```

借用投影：

```rust
pub const fn new() -> Self;
pub fn phase(&self) -> refresh::Phase;
pub fn data(&self) -> Option<&Data>;
pub fn problem(&self) -> Option<&Problem>;
pub fn is_running(&self) -> bool;
```

runtime 消息矩阵：

```text
Idle        + Settle(result) -> Ready / Unavailable
Ready       + Settle(result) -> Ready / Degraded
Idle        + Load(task)    -> Loading
Ready       + Refresh(task) -> Refreshing
Unavailable + Retry(task)   -> Retrying
Degraded    + Refresh(task) -> RefreshingDegraded
running     + Complete      -> Ready / Unavailable / Degraded
running     + Cancel        -> exact previous state
```

### 3.4 Repair runtime enum

```rust
pub enum repair::Operation<Data, Problem: Error, Repair, Task> {
    Idle(repair::Idle<Repair>),
    Loading(repair::Fetching<repair::Idle<Repair>, Task>),

    Ready(repair::Ready<Data, Repair>),
    Refreshing(
        repair::Fetching<repair::Ready<Data, Repair>, Task>,
    ),

    Unavailable(repair::Unavailable<Problem, Repair>),
    RepairingUnavailable(
        repair::Repairing<
            repair::Unavailable<Problem, Repair>,
            Repair,
            Task,
        >,
    ),

    Degraded(repair::Degraded<Data, Problem, Repair>),
    RepairingDegraded(
        repair::Repairing<
            repair::Degraded<Data, Problem, Repair>,
            Repair,
            Task,
        >,
    ),
}
```

借用投影：

```rust
pub const fn new() -> Self;
pub fn phase(&self) -> repair::Phase;
pub fn data(&self) -> Option<&Data>;
pub fn problem(&self) -> Option<&Problem>;
pub fn active_repair(&self) -> Option<&Repair>;
pub fn is_running(&self) -> bool;
```

runtime 消息矩阵：

```text
Idle        + Settle(result)         -> Ready / Unavailable
Ready       + Settle(result)         -> Ready / Degraded
Idle        + Load(task)            -> Loading
Ready       + Refresh(task)         -> Refreshing
Unavailable + Repair(repair, task)  -> RepairingUnavailable
Degraded    + Repair(repair, task)  -> RepairingDegraded
running     + Complete              -> Ready / Unavailable / Degraded
running     + Cancel                -> exact previous state
```

### 3.5 同步结算、异步完成与取消

`Settle(Result<Data, Problem>)` 与 `Complete(Result<Data, Problem>)` 不能合并：

- `Settle` 只在 `Idle` 或 `Ready` 合法，用于调用者已经同步完成的工作，不产生 running
  state，也不保存 Task；`Ready + Ok` 替换 Data，`Ready + Err` 保留旧 Data 并进入
  `Degraded`；
- `Complete` 只在 running variant 合法，是此前某个 Task 的完成消息；
- async load 被 `Cancel` 恢复到 `Idle` 后，即使迟到的 `Complete` 仍被送达，也必须按非法
  消息处理，不能覆盖当前状态；
- 调用者若要在此后同步初始化，必须明确发送新的 `Settle`。

`Transition<Message> for &mut Operation` 内部可以短暂使用 `mem::take(self)`，但必须遵守：

1. `take` 后不调用 source、owner、通知、await 或其他用户 callback；
2. 非法路径先恢复原 enum，再 debug 并 drop owned 消息；
3. 合法路径先把最终合法 enum 写回 `self`；
4. 写回后依次 drop Task、Repair 和被淘汰的旧 payload。

第 4 点很重要：Task、Repair、Data 和 Problem 都可能实现用户自定义 Drop。若先 drop 再写回，
析构重入或 panic 会暴露临时 Idle。这个复杂性必须封装在库内，不能再让每个 Resource 重写。

`Complete` 与 `Cancel` 的 runtime 实现不能直接委托给具名状态的 consuming Transition。
具名状态转换会在返回前 drop Task 或旧 payload，而完整 enum 此时还未写回最终状态。runtime
实现必须单独安排 commit 顺序，先安装最终状态，再执行所有可能运行用户析构器的 drop。

取消恢复的是准确 previous state：

```text
refresh:
Loading              -> Idle
Refreshing           -> Ready
Retrying             -> Unavailable
RefreshingDegraded   -> Degraded

repair:
Loading              -> Idle
Refreshing           -> Ready
RepairingUnavailable -> Unavailable
RepairingDegraded    -> Degraded
```

### 3.6 Ready 业务消息

`Ready` 不公开 `data_mut`。应用为 `&mut Data` 实现自己的 typed
`Transition<BusinessMessage>`，库把 `&mut Ready<Data>` 收到的同一消息委托给
`&mut Data`：

```rust
impl Transition<SelectModel> for &mut CatalogData {
    type Output = ();

    fn transition(self, message: SelectModel) {
        self.select(message.model_id);
    }
}

match &mut resource.operation {
    refresh::Operation::Ready(ready) => ready.transition(message),
    _ => {
        // UI 对其他状态分别展示 loading、problem 或只读 stale data。
    }
}
```

对应的库内 blanket delegation 保持 `Ready -> Ready`：

```rust
impl<Data, Message> Transition<Message> for &mut refresh::Ready<Data>
where
    for<'data> &'data mut Data: Transition<Message, Output = ()>,
{
    type Output = ();
}
```

repair family 的 `Ready<Data, Repair>` 提供同样委托。业务消息不会提升到整个
`Operation`：应用必须先 match 精确的 Ready variant，因而非 Ready 时如何反馈由应用 UI
决定。整个过程不移动 Data、不要求 Clone，panic 时 Operation 也始终保持合法 Ready
variant。

### 3.7 Owner 与 Store

应用直接保存库 enum：

```rust
struct CatalogResource {
    operation: refresh::Operation<
        CatalogData,
        CatalogProblem,
        gpui::Task<()>,
    >,
}
```

应用仍负责：

```text
构造 attempt
  -> 构造负责 completion route 的 Task
  -> operation.transition(Load / Refresh / Retry / Repair)
  -> owner 发布
  -> Task await result
  -> owner 中 operation.transition(Complete(result))
  -> owner 发布
```

`gpui-store` 不需要 Operation adapter。`Store::update` 给出 `&mut S`，对
`&mut Operation` 的 Transition 实现已经把 owned 状态移动复杂性封装在
`gpui-operation`：

```rust
store.update(cx, |resource| {
    resource.operation.transition(Complete(result));
});
```

runtime Transition 返回 `()`，所以调用者应先根据当前 variant 和产品事件选择合法消息，再
构造生命周期关键的 Task。非法消息仍会恢复状态、记录 debug 并 drop 输入，不能作为正常
控制流或成功信号使用。

## 4. 实施工作包

### OP-10：Runtime 基础与 refresh enum

**文件**

- `src/message.rs`
- `src/transition.rs`
- `src/lib.rs`
- `src/refresh.rs`
- `tests/refresh.rs`

**内容**

1. 保留 refresh `Operation`、`Phase` 与具名状态 consuming Transition；
2. 为 `&mut refresh::Operation` 实现 `Settle`、`Load`、`Refresh`、`Retry`、`Complete`、
   `Cancel`；
3. 删除命令式 runtime API、`can_*` 与 `Rejected`；
4. 非法消息恢复状态后 drop，合法 completion/cancel 先安装最终状态；
5. 为 `&mut refresh::Ready<Data>` 实现 Data 消息委托；
6. 覆盖同步结算、八态、非法消息、空 Data、Ready 更新与无 Clone/Send/Default bound。

### OP-20：Repair enum

**文件**

- `src/repair.rs`
- `tests/repair.rs`

**内容**

1. 保留 repair `Operation`、`Phase`、具名状态 consuming Transition 与
   `active_repair`；
2. 为 `&mut repair::Operation` 实现 `Settle`、`Load`、`Refresh`、`Repair`、`Complete`、
   `Cancel`；
3. 问题态只接受 caller-selected `Repair` 消息；
4. 为 `&mut repair::Ready<Data, Repair>` 实现 Data 消息委托；
5. 覆盖同步结算、八态、非法消息、Repair 所有权、完成、取消、drop 和无 hidden bound。

### OP-30：tracing 与真实 GPUI Task

**文件**

- `Cargo.toml`
- `tests/gpui_task.rs`

**内容**

1. 新增默认关闭的 `tracing` feature；
2. 非法消息只在恢复稳定状态后 debug，不增加 payload bound；
3. Entity 字段直接保存 library Operation，不定义应用 runtime enum；
4. 验证 pending GPUI Task cancel 与取消后不能路由 completion；
5. 验证 Task 完成时通过 `Transition<Complete>` 淘汰持有自身的 running state；
6. repair cancel 恢复 Problem 并 drop Repair。

### OP-40：双语公开文档

**文件**

- `README.md`
- `README.zh-CN.md`
- `docs/README.md`
- `docs/guide.md`
- `docs/guide.zh-CN.md`
- `src/lib.rs`

**内容**

1. README 以两个 runtime enum 为主入口；
2. guide 保留具名状态 API，并展示两个完整 enum；
3. Entity、Global、Store 六组示例通过 `Transition<Message>` 使用 library Operation；
4. 展示 Ready 业务消息委托，并明确 stale data 只读；
5. 删除命令式 runtime API、`can_*`、`Rejected` 与兼容层表述；
6. 英文默认，中文逐节对应。

## 5. 验证

### 5.1 包级门禁

```bash
cargo fmt --package gpui-operation
cargo build -p gpui-operation --locked
cargo test -p gpui-operation --lib --tests --locked
cargo test -p gpui-operation --doc --locked
cargo doc -p gpui-operation --no-deps --locked
cargo tree -p gpui-operation --edges normal --depth 1 --locked
cargo tree -p gpui-operation --edges normal --depth 1 --features tracing --locked
cargo clippy -p gpui-operation --all-targets --all-features --locked -- -D warnings
git diff --check
```

### 5.2 残留扫描

源码与公开文档不得残留命令式 runtime API、Rejected 或应用同构 runtime enum：

```bash
! rg -n \
  'start_fetch|start_repair|can_start|can_cancel|Rejected|src/rejected.rs|application-owned runtime enum|应用.*runtime enum' \
  crates/gpui-operation/README.md \
  crates/gpui-operation/README.zh-CN.md \
  crates/gpui-operation/docs \
  crates/gpui-operation/src
```

允许说明“不提供跨 family 万能 `Operation<S>`”，但必须同时明确提供
`refresh::Operation` 与 `repair::Operation`。

### 5.3 本轮验证结果

旧命令式 runtime API 的验证结果在本轮 breaking contract 后失效。最终结果只记录实际对
消息接口、非法消息、tracing、Ready 委托与真实 GPUI Task 重新执行的验证，不沿用旧测试
数量或旧残留扫描结论。

### 5.4 Workspace 发布门槛

Jaco 尚未迁移到 breaking Store/Operation API 时，不宣称 workspace 命令通过。最终合并仍需：

```bash
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

## 6. 完成交接审计

- [x] refresh 与 repair 两个 runtime enum 归库所有。
- [x] 应用不再重复定义八个 variant 与转换 match。
- [x] 具名状态继续提供编译期精确转换。
- [x] 完整 enum 通过 `Transition<Message> for &mut Operation` 原地接收消息。
- [x] 命令式 runtime API、`can_*` 与 `Rejected` 已从契约删除，不保留兼容层。
- [x] 非法消息先恢复原状态，再 debug 并 drop。
- [x] Ready 业务消息委托给 `&mut Data`，无 `data_mut` 和 Clone 要求。
- [x] Problem 只要求 Error；其他 payload 无 blanket bound。
- [x] final-state-before-drop 的重入与 panic 不变量已记录。
- [x] Task/runtime、owner、通知、Repair 选择仍归调用者。
- [x] Store 只需普通 `&mut S` update，不新增专用 adapter。
- [x] 默认 feature 仍只有 std；非法消息 tracing 是可选 feature，Jaco 接入时开启。
- [x] 双语文档、测试与验证命令有明确交接。
