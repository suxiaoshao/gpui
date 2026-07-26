# gpui-operation 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

本指南记录 `gpui-operation` 0.1 已实现的公开 API。示例会使用应用定义的 Data、Problem、
Repair 和 task source，以便具体展示 ownership 与状态转换模式。

## 1. 用途

`gpui-operation` 描述可失败异步工作的安全状态转换。它是转换库，不是 executor 或 owner：

```text
当前状态 + 消息 -> 下一状态
```

调用者负责：

- 构造并启动 task；
- 把 task 最终结果作为消息送回；
- 选择 Entity、普通 Global、`gpui-store::Store`、局部变量或自定义 owner；
- 发布 owner 通知；
- 决定每种 Problem 对应哪些产品操作。

库为两个 family 分别提供完整的 runtime `Operation` enum。库不定义 `OperationSource`、
不 await task、不安装 owner、不路由 completion，也不持久化数据。它有意不提供跨 family 的
万能 `Operation<S>`。

## 2. 两类 Operation

失败在产品上有两种本质不同的含义：

| 类型 | 失败后的恢复方式 | 典型场景 |
| --- | --- | --- |
| `refresh` | 再执行一次相同读取 | 数据库查询、catalog、远程元数据 |
| `repair` | 选择明确的恢复方案 | 配置文件损坏、数据库打开或 migration 失败 |

这个差异属于公开类型系统。只刷新的 operation 没有 Repair 类型，也没有 repair 转换。可修复
operation 不会用 `Repair = ()` 假装“不支持修复”。

两类 operation 都保存相同事实：

- `Ready` 包含有效 Data；
- `Unavailable` 包含 Problem，但没有有效 Data；
- `Degraded` 包含最后有效 Data 和最新 Problem；
- 运行状态同时拥有此前的稳定状态和 Task。

## 3. 消息与转换

两层 API 使用同一个转换 trait：

```rust
pub trait Transition<Message> {
    type Output;

    fn transition(self, message: Message) -> Self::Output;
}
```

对于具名状态，`self` 是 owned 状态，转换会消费它。对于完整 runtime enum，receiver 是
`&mut Operation`，转换会原地替换当前 variant，并且 `Output = ()`。

公共消息为：

```rust
pub struct Load<Task>(pub Task);
pub struct Refresh<Task>(pub Task);
pub struct Retry<Task>(pub Task);

pub struct Repair<Kind, Task> {
    pub repair: Kind,
    pub task: Task,
}

pub struct Complete<Data, Problem: std::error::Error>(
    pub Result<Data, Problem>,
);

pub struct Cancel;
```

消息拥有自己的 payload。转换把 payload 移入下一状态；Data、Problem、Repair 或 Task 都不需要
统一实现 `Clone`、`PartialEq`、`Send` 或 `Sync`，但 Problem 必须实现
`std::error::Error`。

完整 runtime enum 接受其 family 支持的消息：

| Runtime family | 原地接收的消息 |
| --- | --- |
| `refresh::Operation` | `Load`、`Refresh`、`Retry`、`Complete`、`Cancel` |
| `repair::Operation` | `Load`、`Refresh`、`Repair`、`Complete`、`Cancel` |

如果消息不适用于当前 runtime variant，operation 保持不变，消息及其 owned payload 会被
drop；runtime 消息传递返回 `()`，没有其他返回路径。如果工作 payload 不能被丢弃，应用应先
匹配当前 variant，再构造工作。开启可选的 `tracing` feature 后，runtime 消息被忽略时会记录
包含 family、phase 和消息类型的 debug event。

`Fetching<Previous, Task>` 表示正在 load、refresh 或 retry；
`Repairing<Previous, Repair, Task>` 表示正在 repair。`Load`、`Refresh`、`Retry` 与
`Repair` 是进入这些运行状态的消息。

具名状态暴露借用 payload。下表分别写出两个 family 的准确泛型形式：

| 状态 | 借用 API |
| --- | --- |
| `refresh::Ready<Data>` / `repair::Ready<Data, Repair>` | `data() -> &Data` |
| `refresh::Unavailable<Problem>` / `repair::Unavailable<Problem, Repair>` | `problem() -> &Problem` |
| `refresh::Degraded<Data, Problem>` / `repair::Degraded<Data, Problem, Repair>` | `data() -> &Data`、`problem() -> &Problem` |
| 两个 family 的 `Fetching<Previous, Task>` | `previous() -> &Previous` |
| `repair::Repairing<Previous, Repair, Task>` | `previous() -> &Previous`、`repair() -> &Repair` |

这些投影不会 clone payload，也不会暴露 mutable reference。第 7 节会展示精确 `Ready`
如何接收应用定义的领域消息，而不暴露可变 Data。

## 4. 只刷新 Operation

失败后只需重新执行同一次读取时，使用 `gpui_operation::refresh`。

### 4.1 状态图

稳定状态为：

```rust
refresh::Idle
refresh::Ready<Data>
refresh::Unavailable<Problem>
refresh::Degraded<Data, Problem>
```

所有运行状态统一表示为：

```rust
refresh::Fetching<Previous, Task>
```

`Previous` 是启动 task 前准确的稳定状态：

```text
Idle
  + Load<Task>
  -> Fetching<Idle, Task>

Ready<Data>
  + Refresh<Task>
  -> Fetching<Ready<Data>, Task>

Unavailable<Problem>
  + Retry<Task>
  -> Fetching<Unavailable<Problem>, Task>

Degraded<Data, Problem>
  + Refresh<Task>
  -> Fetching<Degraded<Data, Problem>, Task>
```

这种表示让取消可以统一实现：

```rust
impl<Previous, Task> Transition<Cancel>
    for refresh::Fetching<Previous, Task>
{
    type Output = Previous;
}
```

转换消费并 drop Task，然后返回 `Previous`。它不根据 status 标记重建旧状态。

### 4.2 完成

此前没有 Data 时，completion 返回：

```rust
pub enum FetchCompleted<Data, Problem: std::error::Error> {
    Ready(refresh::Ready<Data>),
    Unavailable(refresh::Unavailable<Problem>),
}
```

它适用于第一次加载，以及从 `Unavailable` 重试：

```text
Fetching<Idle, Task> + Complete(Ok(new data))
  -> Ready(new data)

Fetching<Idle, Task> + Complete(Err(new problem))
  -> Unavailable(new problem)

Fetching<Unavailable(old problem), Task> + Complete(Ok(new data))
  -> Ready(new data)

Fetching<Unavailable(old problem), Task> + Complete(Err(new problem))
  -> Unavailable(new problem)
```

此前存在 Data 时，completion 返回：

```rust
pub enum RefreshCompleted<Data, Problem: std::error::Error> {
    Ready(refresh::Ready<Data>),
    Degraded(refresh::Degraded<Data, Problem>),
}
```

刷新成功会替换旧 Data；刷新失败会把旧 Data 与新 Problem 移入 `Degraded`。

### 4.3 直接使用

```rust
use gpui_operation::{Complete, Load, Refresh, Transition};
use gpui_operation::refresh::{
    FetchCompleted, Idle, RefreshCompleted,
};

let loading = Idle::new().transition(Load(load_task));

let ready = match loading.transition(Complete(Ok(initial_data))) {
    FetchCompleted::Ready(ready) => ready,
    FetchCompleted::Unavailable(_) => unreachable!(),
};

let refreshing = ready.transition(Refresh(refresh_task));

let settled = match refreshing.transition(Complete(refresh_result)) {
    RefreshCompleted::Ready(ready) => CatalogState::Ready(ready),
    RefreshCompleted::Degraded(degraded) => {
        CatalogState::Degraded(degraded)
    }
};
```

## 5. 可修复 Operation

Problem 可能需要调用者选择明确恢复方案时，使用 `gpui_operation::repair`。

### 5.1 状态图

稳定状态为：

```rust
repair::Idle<Repair>
repair::Ready<Data, Repair>
repair::Unavailable<Problem, Repair>
repair::Degraded<Data, Problem, Repair>
```

第一次加载与普通刷新使用：

```rust
repair::Fetching<Previous, Task>
```

明确修复使用：

```rust
repair::Repairing<Previous, Repair, Task>
```

合法的启动转换为：

```text
Idle<Repair>
  + Load<Task>
  -> Fetching<Idle<Repair>, Task>

Ready<Data, Repair>
  + Refresh<Task>
  -> Fetching<Ready<Data, Repair>, Task>

Unavailable<Problem, Repair>
  + Repair<Repair, Task>
  -> Repairing<Unavailable<Problem, Repair>, Repair, Task>

Degraded<Data, Problem, Repair>
  + Repair<Repair, Task>
  -> Repairing<Degraded<Data, Problem, Repair>, Repair, Task>
```

Repair 类型属于该 operation 类型，因此 runtime owner 不会误用另一个 Resource 的 Repair 值。

### 5.2 完成与取消

第一次加载与普通刷新使用该 family 自己的结果类型：

```rust
pub enum FetchCompleted<Data, Problem: std::error::Error, Repair> {
    Ready(repair::Ready<Data, Repair>),
    Unavailable(repair::Unavailable<Problem, Repair>),
}

pub enum RefreshCompleted<Data, Problem: std::error::Error, Repair> {
    Ready(repair::Ready<Data, Repair>),
    Degraded(repair::Degraded<Data, Problem, Repair>),
}
```

`Ready` 的普通刷新成功后产生新 Data，失败后把旧 Data 保存在 `Degraded`。

Repair completion 保留准确的合法目标集合。此前没有 Data 时返回：

```rust
pub enum RepairWithoutDataCompleted<
    Data,
    Problem: std::error::Error,
    Repair,
> {
    Ready(repair::Ready<Data, Repair>),
    Unavailable(repair::Unavailable<Problem, Repair>),
}
```

此前存在 Data 时返回：

```rust
pub enum RepairWithDataCompleted<
    Data,
    Problem: std::error::Error,
    Repair,
> {
    Ready(repair::Ready<Data, Repair>),
    Degraded(repair::Degraded<Data, Problem, Repair>),
}
```

失败后的 variant 由 `Previous` 决定：

```text
Repairing<Unavailable(old problem), repair, task>
  + Complete(Err(new problem))
  -> Unavailable(new problem)

Repairing<Degraded(old data, old problem), repair, task>
  + Complete(Err(new problem))
  -> Degraded(old data, new problem)
```

成功总是产生 `Ready(new data)`。

`Cancel` 会 drop Task 与 Repair，并返回准确的 `Previous`：

```text
Repairing<Unavailable(problem), repair, task> + Cancel
  -> Unavailable(problem)

Repairing<Degraded(data, problem), repair, task> + Cancel
  -> Degraded(data, problem)
```

### 5.3 直接使用

```rust
use gpui_operation::{Complete, Repair, Transition};
use gpui_operation::repair::RepairWithoutDataCompleted;

enum DatabaseRepair {
    RetryOpen,
    RestoreBackup,
    Recreate,
}

let repairing = unavailable.transition(Repair {
    repair: DatabaseRepair::RestoreBackup,
    task,
});

let settled = match repairing.transition(Complete(result)) {
    RepairWithoutDataCompleted::Ready(ready) => {
        DatabaseState::Ready(ready)
    }
    RepairWithoutDataCompleted::Unavailable(unavailable) => {
        DatabaseState::Unavailable(unavailable)
    }
};
```

## 6. Task 与取消契约

库把 Task 当作 owned generic value，不 poll、不 abort、也不检查它。进入运行状态会把 Task 移入
该状态；完成或取消会消费运行状态并 drop Task。

对于 GPUI `Task`，drop handle 会取消 task。因此预期的 UI 契约是：

1. 运行状态是其 task 的唯一 owner；
2. 应用在构造 attempt 与 Task 前先匹配当前 runtime variant；
3. 构造 Task 时不能同步重新进入 owner，也不能在启动消息保存 Task 前送达 `Complete`；
4. owner 在检查与转换之间不 yield，并准确发送一个合法的 `Load`、`Refresh`、`Retry` 或
   `Repair` 消息；
5. cancel 先恢复稳定状态，再 drop 当前 task，然后返回；
6. 只有完成该转换后，调用者才能启动下一 task；
7. 已取消 task 不能再发送 `Complete`。

在这个契约下，不需要 stale-completion 状态、attempt identifier、generation counter、opaque
Completion 或 acceptance check。

如果某个自定义 runtime handle 在 drop 后不会取消实际工作，调用者必须用所需的
abort-on-drop 行为包装它。取消后仍可能送达消息的 detached producer 不属于该契约，必须自行
执行 generation 检查。

drop Task 不能回滚文件、数据库、网络服务或 repair action 已经产生的外部副作用。

## 7. 两个预定义 Runtime Enum

直接持有具名状态时，可以获得编译期安全的 `Transition<Message>`。Entity、Global、Store
或普通字段需要长期保存状态时，直接使用库提供的两个完整 enum：

```rust
use gpui::Task;
use gpui_operation::{refresh, repair};

type CatalogOperation =
    refresh::Operation<CatalogData, CatalogProblem, Task<()>>;

type DatabaseOperation = repair::Operation<
    Database,
    DatabaseProblem,
    DatabaseRepair,
    Task<()>,
>;
```

`refresh::Operation` 公开八个 variant：

```text
Idle / Loading
Ready / Refreshing
Unavailable / Retrying
Degraded / RefreshingDegraded
```

`repair::Operation` 也公开八个 variant：

```text
Idle / Loading
Ready / Refreshing
Unavailable / RepairingUnavailable
Degraded / RepairingDegraded
```

两个 enum 都提供：

- `new` / `default`：从 `Idle` 开始，不要求任一 payload 实现 `Default`；
- `phase`：返回可比较、可复制的 family-specific `Phase`；
- `data` / `problem`：借用当前有效 Data 或最近 Problem；
- `is_running`；
- repair family 另外提供 `active_repair`。

两个 enum 都为 `&mut Operation` 实现 `Transition<Message>`。只刷新 runtime 的显式消息
路由为：

```text
Idle + Load<Task> -> Loading
Ready + Refresh<Task> -> Refreshing
Unavailable + Retry<Task> -> Retrying
Degraded + Refresh<Task> -> RefreshingDegraded
运行状态 + Complete<Result<Data, Problem>> -> 稳定状态
运行状态 + Cancel -> 准确的上一个稳定状态
```

可修复 runtime 在 `Idle` 接收 `Load`，在 `Ready` 接收 `Refresh`，在 `Unavailable` 或
`Degraded` 接收 `Repair { repair, task }`。`Complete` 与 `Cancel` 适用于它的所有运行
variant。

runtime 消息传递有意设计为单向：

```rust
use gpui_operation::{Complete, Load, Transition, refresh};

let mut operation =
    refresh::Operation::<CatalogData, CatalogProblem, CatalogTask>::new();

operation.transition(Load(task));
operation.transition(Complete(result));
```

非法 runtime 消息会保留当前 variant 并 drop 消息。即使应用在错误 phase 发送消息，已发送的
Task、Repair 或 completion result 也归状态机所有。如果这会构成产品错误，应先匹配状态；
需要诊断被忽略消息时可开启 `tracing`。

接受 completion 或 cancel 时，库会先安装最终合法状态，再按 Task、Repair、被淘汰旧 payload
的顺序 drop；泛型析构发生重入或 panic 时也不会暴露临时 `Idle`。

### 7.1 用领域消息更新精确 Ready Data

已提交的领域变更有时需要更新内存 Data，同时又不应 clone 整个 catalog 或暴露
`&mut Data`。可以在 Data 类型上定义领域消息：

```rust
use gpui_operation::Transition;

struct ReplaceRecord(Record);

impl Transition<ReplaceRecord> for &mut CatalogData {
    type Output = ();

    fn transition(self, message: ReplaceRecord) {
        self.insert_or_replace(message.0);
    }
}
```

两个 family 都为 `&mut Ready<Data>` 实现了相应委托。调用者必须先匹配准确的 runtime
variant：

```rust
if let refresh::Operation::Ready(ready) = &mut operation {
    ready.transition(ReplaceRecord(committed_record));
}
```

API 不提供 mutable Data accessor。`Refreshing`、`Degraded` 或 degraded repair 保留的 Data
仍然只读，因此应用不会把已提交 mutation 意外发布到非 Ready 生命周期 phase。

后续示例只需定义领域 wrapper，不再定义 runtime enum：

```rust
struct CatalogResource {
    operation: CatalogOperation,
    repository: CatalogRepository,
}

struct DatabaseResource {
    operation: DatabaseOperation,
    database: DatabaseService,
}
```

## 8. 使用 Entity

Resource 属于某个组件、文档、窗口或其他具有独立生命周期边界的对象时，使用 Entity。

### 8.1 只刷新的 Entity

```rust
use gpui::{Context, Entity};

impl CatalogResource {
    fn load(&mut self, cx: &mut Context<Self>) {
        let refresh::Operation::Idle(_) = &self.operation else {
            return;
        };

        let attempt = self.repository.fetch();
        let task = cx.spawn(async move |owner, cx| {
            let result = attempt.await;

            let _ = owner.update(cx, |owner, cx| {
                let refresh::Operation::Loading(_) =
                    &owner.operation
                else {
                    return;
                };
                owner.operation.transition(Complete(result));
                cx.notify();
            });
        });

        self.operation.transition(Load(task));
        cx.notify();
    }
}

let catalog: Entity<CatalogResource> =
    cx.new(|_| CatalogResource::new(repository));

catalog.update(cx, |catalog, cx| catalog.load(cx));

let has_data = catalog.read(cx).operation.data().is_some();
```

这个 command 在构造工作前匹配 `Idle`，然后直接发送 `Load`。refresh 或 retry command
采用相同结构，但会匹配 `Ready`/`Degraded` 或 `Unavailable`，再发送 `Refresh` 或
`Retry`。

组件观察 Entity，并保存返回的 subscription：

```rust
let subscription = cx.observe(&catalog, |_view, _catalog, cx| {
    cx.notify();
});
```

### 8.2 可修复的 Entity

```rust
impl DatabaseResource {
    fn repair(
        &mut self,
        repair: DatabaseRepair,
        cx: &mut Context<Self>,
    ) {
        let problem = match &self.operation {
            repair::Operation::Unavailable(state) => state.problem(),
            repair::Operation::Degraded(state) => state.problem(),
            _ => return,
        };

        // 应用根据借用输入构造 owned future。
        // DatabaseRepair 本身继续用于运行状态。
        let attempt = self.database.repair_attempt(problem, &repair);

        let task = cx.spawn(async move |owner, cx| {
            let result = attempt.await;

            let _ = owner.update(cx, |owner, cx| {
                match &owner.operation {
                    repair::Operation::RepairingUnavailable(_)
                    | repair::Operation::RepairingDegraded(_) => {}
                    _ => return,
                }
                owner.operation.transition(Complete(result));
                cx.notify();
            });
        });

        self.operation.transition(Repair { repair, task });
        cx.notify();
    }
}
```

转换 API 不要求 Data、Problem 或 Repair 实现 `Clone`。`repair_attempt` 是返回 owned
future 的应用代码；它可以根据自身 runtime 的需要提取 owned request value、只 clone 必要输入，
或者使用 `Arc` 共享。这不是本 crate 的 trait hook。第一次正常加载和刷新与 catalog 使用相同
Entity 模式。

如果要暴露取消，Entity 先匹配某个准确的运行 variant，再发送 `Cancel` 并调用
`cx.notify()`。向稳定 operation 发送 `Cancel` 也只会保持原状态，但匹配可以避免无意义
通知。

## 9. 使用普通 Global

Resource 具有应用级生命周期，并且整个进程天然只有一个实例时，使用普通 GPUI Global。

### 9.1 只刷新的 Global

```rust
use gpui::{
    App, AppContext as _, BorrowAppContext as _, Global,
};

struct CatalogGlobal(CatalogResource);
impl Global for CatalogGlobal {}

fn install_catalog(repository: CatalogRepository, cx: &mut App) {
    cx.set_global(CatalogGlobal(CatalogResource::new(repository)));
}

fn refresh_catalog(cx: &mut App) {
    let catalog = cx.global::<CatalogGlobal>();
    let refresh::Operation::Ready(_) = &catalog.0.operation else {
        return;
    };

    let attempt = catalog.0.repository.fetch();
    let task = cx.spawn(async move |cx| {
        let result = attempt.await;

        cx.update_global::<CatalogGlobal, _>(|catalog, _| {
            let refresh::Operation::Refreshing(_) =
                &catalog.0.operation
            else {
                return;
            };
            catalog.0.operation.transition(Complete(result));
        });
    });

    cx.update_global::<CatalogGlobal, _>(|catalog, _| {
        catalog.0.operation.transition(Refresh(task));
    });
}
```

`update_global` 会向 Global observer 发布。只读匹配可以避免在非 `Ready` phase 构造刷新
工作，mutation 会直接发送 `Refresh`。

组件观察 Global：

```rust
let subscription = cx.observe_global::<CatalogGlobal>(|_view, cx| {
    cx.notify();
});
```

### 9.2 可修复的 Global

```rust
struct DatabaseGlobal(DatabaseResource);
impl Global for DatabaseGlobal {}

fn repair_database(repair: DatabaseRepair, cx: &mut App) {
    let Some(attempt) = cx.read_global::<DatabaseGlobal, _>(
        |database, _| {
            let problem = match &database.0.operation {
                repair::Operation::Unavailable(state) => {
                    state.problem()
                }
                repair::Operation::Degraded(state) => state.problem(),
                _ => return None,
            };
            Some(database.0.database.repair_attempt(problem, &repair))
        },
    ) else {
        return;
    };

    let task = cx.spawn(async move |cx| {
        let result = attempt.await;

        cx.update_global::<DatabaseGlobal, _>(|database, _| {
            match &database.0.operation {
                repair::Operation::RepairingUnavailable(_)
                | repair::Operation::RepairingDegraded(_) => {}
                _ => return,
            }
            database.0.operation.transition(Complete(result));
        });
    });

    cx.update_global::<DatabaseGlobal, _>(|database, _| {
        database
            .0
            .operation
            .transition(Repair { repair, task });
    });
}
```

task 不捕获 Global；完成时通过 GPUI context 找到唯一的应用级 owner。

## 10. 使用 gpui-store

本节使用当前 `gpui-store` API：`Store<S>`、`Store::install_global`、
`Store::global`、`read`、`update`、`update_if`、`select` 与 `observe`。

多个消费者需要相同 operation state，并且需要 Store 原生读取、selection 或观察能力时，使用
Store。operation 是权威 Store state 中的一个字段；不要再把 Data 复制到另一个 Store。

### 10.1 只刷新的 Store

安装一个类型化 global Store：

```rust
use gpui_store::Store;

Store::install_global(
    cx,
    CatalogResource::new(repository),
);
```

启动重试：

```rust
fn retry_catalog_store(cx: &mut App) {
    let catalog = Store::<CatalogResource>::global(cx);

    let Some(attempt) = catalog.read(cx, |resource| {
        let refresh::Operation::Unavailable(_) =
            &resource.operation
        else {
            return None;
        };

        Some(resource.repository.fetch())
    }) else {
        return;
    };

    let task = cx.spawn(async move |cx| {
        let result = attempt.await;
        let catalog = Store::<CatalogResource>::global(cx);

        catalog.update(cx, |resource| {
            let refresh::Operation::Retrying(_) =
                &resource.operation
            else {
                return;
            };
            resource.operation.transition(Complete(result));
        });
    });

    catalog.update(cx, |resource| {
        resource.operation.transition(Retry(task));
    });
}
```

task 不捕获强 Store handle，而是在 completion 准备好时重新获取类型化 global Store。read
会在构造工作前匹配 `Unavailable`，Store `update` 再直接发送 `Retry`。load、refresh 与
cancel 通过同一 owner route 发送各自显式消息。

消费者可以只 select 渲染需要的状态：

```rust
let status = catalog.select(
    cx,
    |resource: &CatalogResource| resource.operation.phase(),
);
```

### 10.2 可修复的 Store

```rust
Store::install_global(
    cx,
    DatabaseResource::new(database_service),
);

fn repair_database_store(
    repair: DatabaseRepair,
    cx: &mut App,
) {
    let database = Store::<DatabaseResource>::global(cx);

    let Some(attempt) = database.read(cx, |resource| {
        let problem = match &resource.operation {
            repair::Operation::Unavailable(state) => state.problem(),
            repair::Operation::Degraded(state) => state.problem(),
            _ => return None,
        };
        Some(resource.database.repair_attempt(problem, &repair))
    }) else {
        return;
    };

    let task = cx.spawn(async move |cx| {
        let result = attempt.await;
        let database = Store::<DatabaseResource>::global(cx);

        database.update(cx, |resource| {
            match &resource.operation {
                repair::Operation::RepairingUnavailable(_)
                | repair::Operation::RepairingDegraded(_) => {}
                _ => return,
            }
            resource.operation.transition(Complete(result));
        });
    });

    database.update(cx, |resource| {
        resource
            .operation
            .transition(Repair { repair, task });
    });
}
```

Store 不加载数据库，也不选择 Repair。应用 command 构造工作，再通过纯状态转换修改 Store。
Store 只负责共享内存所有权与发布。

非 global Store 需要应用自行选择到同一 Store 实例的 completion route。`gpui-store`
不暴露弱 Store handle，所以示例使用 typed-global lookup；应用也可以通过已经持有该 Store
的其他 owner 路由 completion。

## 11. 选择 Owner

| 需求 | Owner |
| --- | --- |
| 组件、文档或窗口范围的生命周期 | Entity |
| 整个进程只有一个 Resource | 普通 Global |
| 共享读取、selection 与观察 | `Store<S>`；以上示例使用 typed-global Store |
| 不需要观察且生命周期很短 | 局部变量 |

owner 选择不会改变状态机：

- owner 在构造 task 前匹配合法启动 variant；
- 运行状态拥有 Task；
- task completion 回到同一个 owner；
- owner 同步应用 `Complete(result)`；
- owner 在每次有意发送消息后发布。

## 12. 依赖与 Runtime 选择

依赖是构造 task 时使用的应用输入：

```rust
let attempt = repository.fetch_catalog();
let task = cx.spawn(async move |owner, cx| {
    let result = attempt.await;
    // 把 Complete(result) 送到所选 owner。
});
```

转换 crate 没有依赖图或 `Waiting` 状态。依赖尚未可用时，应用不构造也不发送启动消息。

Task 是泛型。调用者可以使用：

- GPUI `Task<()>`；
- bridge `gpui-tokio` 工作的 GPUI task；
- 其他满足第 6 节契约的 abort-on-drop handle。

runtime 特有的 `Send` 或 `Sync` 要求只影响 task 构造，不会施加给所有状态 payload。

## 13. 产品策略

状态机只报告事实：

- `Ready` 有有效 Data；
- `Unavailable` 没有有效 Data；
- `Degraded` 有此前有效 Data 和更新的 Problem；
- `Fetching` 或 `Repairing` 正在运行，并拥有可取消 task。

应用决定：

- 降级 Data 是否仍然可用；
- 向用户展示哪些 Problem；
- 是否暴露取消；
- 哪些 Repair 需要确认；
- 如何说明 repair 副作用；
- 某个 Resource 是否是应用启动必需项。

库不会在失败后伪造默认 Data。

## 14. 非目标

本 crate 不提供：

- `OperationSource` 或 Source hook；
- 跨 refresh/repair family 的万能 `Operation<S>` enum；
- 命令式 runtime `start_*`、`complete`、`cancel` 或 `can_*` 方法；
- task spawn、await、route 或 runtime 选择；
- attempt identifier 或 stale-completion reconciliation；
- 自动启动、自动重试、自动刷新、自动修复或自动取消；
- Entity、Global 或 Store adapter；
- 持久化、事务或回滚；
- 通用依赖图或 `Waiting` 状态；
- observe、selection 或通知；
- payload 的统一 `Clone`、`PartialEq`、`Send` 或 `Sync` 约束。

## 相关文档

- [`gpui-operation` README](../README.zh-CN.md)
- [English README](../README.md)
- [`gpui-store` 使用指南](../../gpui-store/docs/guide.zh-CN.md)
- [`gpui-form` 使用指南](../../gpui-form/docs/guide.zh-CN.md)
