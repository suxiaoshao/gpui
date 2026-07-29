# gpui-store 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

本指南记录 `gpui-store` 0.1 已实现的公开 API。示例使用应用自行定义的领域类型，
展示 Store 如何接入真实的 GPUI owner。

## 1. 用途

`gpui-store` 提供一个抽象：

```text
Store<S>
  持有一份权威的纯内存 S
  提供受控的读取和修改
  发布显式变化
  支持只读派生 selection
```

当多个 GPUI 组件或 service 需要使用同一份运行时状态，并且需要在状态变化时作出响应，
它就很有用。

store 不会为其中保存的值赋予领域含义；它只持有纯内存 `S`，并按照 Store API 发布修改。

## 2. 职责模型

应用应该为一个值保留一份可变事实来源：

```text
commands ──修改──> Store<S> ──发布──> components 和 observers
                       │
                       └──派生──> StoreSelection<T>
```

`StoreSelection<T>` 是可丢弃的派生状态。它可以从 `S` 重新计算，没有 setter，也不能成为
另一份权威数据。

本 crate 有意只负责纯内存所有权和观察：

| 关注点 | 负责人 |
| --- | --- |
| 共享的纯内存状态 | `gpui-store` |
| 可编辑 form 值、验证和提交准备 | `gpui-form` |
| 文件/数据库/网络写入与事务 | 应用 service 或 repository |
| UI 局部交互状态 | Component |

## 3. 状态类型

任何 `'static` Rust 类型都可以放进 store，不需要 marker trait：

```rust
struct WorkspaceState {
    active_project: Option<ProjectId>,
    sidebar_open: bool,
    pending_navigation: Option<Route>,
}
```

`S` 不需要实现 `Clone`、`PartialEq`、`Default`、`Send` 或 `Sync`。单个操作只会为自己
实际需要的值增加约束：

- read 可以返回 Copy、Clone 或新计算的值；
- selection 输出需要 `PartialEq`，从而过滤没有变化的输出；
- 当 selection 输出实现 `Clone` 时，可以使用 `StoreSelection::cloned`。

状态应该保持领域形状。保存使用者真正需要的类型化数据，不要只保存 revision 或
invalidation flag，迫使每个使用者各自重建缓存。

## 4. 创建和共享 store

使用一份初始有效的纯内存数据创建 store：

```rust
use gpui_store::Store;

let workspace = Store::new(
    cx,
    WorkspaceState {
        active_project: None,
        sidebar_open: true,
        pending_navigation: None,
    },
);
```

`Store::new` 不执行 I/O。调用者必须已经拥有一份对纯内存模型有效的数据。

`Store<S>` 是共享 handle。Clone 这个 handle，就可以让另一个组件或 service 访问同一份
状态：

```rust
struct Sidebar {
    workspace: Store<WorkspaceState>,
}

let sidebar = Sidebar {
    workspace: workspace.clone(),
};
```

Clone handle 不会 clone `S`，也不会创建另一份事实来源。

## 5. 类型化全局 store

应用级状态可以只安装一次，作为类型化 global：

```rust
Store::install_global(
    cx,
    WorkspaceState {
        active_project: None,
        sidebar_open: true,
        pending_navigation: None,
    },
);
```

安装之后获取它：

```rust
let workspace = Store::<WorkspaceState>::global(cx);
```

状态类型就是 global key。对于一个 `S`，最多安装一个全局 `Store<S>`，并在应用启动时
明确安排安装顺序。如果状态只在应用的有限范围内共享，优先直接传递 store handle。
`Store::global` 沿用 GPUI typed-global 语义；对应的 `Store<S>` 尚未安装时会 panic。

## 6. 读取状态

所有读取都通过闭包完成：

```rust
let active_project = workspace.read(cx, |state| state.active_project);

let route_label = workspace.read(cx, |state| {
    state
        .pending_navigation
        .as_ref()
        .map(Route::label)
        .unwrap_or("No pending navigation")
        .to_owned()
});
```

闭包可以返回：

- Copy 的标量或标识符；
- 某个字段的 owned clone；
- 新计算的 view model；
- 任何不会继续持有借用 `S` 的其他结果。

把借用限制在闭包内，可以显式表达一次读取的生命周期，并阻止调用者跨越后续修改继续
持有引用。

## 7. 修改状态

所有修改都经过 store。公开 API 有三种修改形状：

| 方法 | 含义 | 通知 |
| --- | --- | --- |
| `set` | 替换完整的 `S` | 总是通知 |
| `update` | 修改当前 `S` 并返回业务值 | 总是通知 |
| `update_if` | 修改，并一起返回业务值和变化决定 | 只在 `StoreChange::Changed` 时通知 |

条件修改使用显式结果：

```rust
pub enum StoreChange<R> {
    Changed(R),
    Unchanged(R),
}
```

`StoreChange<R>` 让业务结果和通知决定属于同一次修改。store 会把该结果返回给调用者，
不需要 side channel 或第二次读取。

### 7.1 替换完整状态

当调用者已经拥有下一份完整的纯内存数据时，使用 `set`：

```rust
workspace.set(
    cx,
    WorkspaceState {
        active_project: Some(project_id),
        sidebar_open: true,
        pending_navigation: None,
    },
);
```

`set` 总是发布。它不比较新旧数据，因此不要求 `S: PartialEq`。新值会先安装，旧值在
Store Entity lease 之外 drop。如果旧值 destructor panic，panic 会继续向上传播，新值仍然
保持已安装状态，而通知可能已经进入队列。

### 7.2 确定会变化的修改

当操作确定会改变可观察状态时，使用 `update`。闭包可以返回业务值：

```rust
let sidebar_open = workspace.update(cx, |state| {
    state.sidebar_open = !state.sidebar_open;
    state.sidebar_open
});
```

闭包完成后，`update` 总是发布。不要用它执行可能是 no-op 的操作。

### 7.3 条件修改

当调用者知道如何判断是否需要通知时，使用 `update_if`：

```rust
use gpui_store::StoreChange;

let outcome = workspace.update_if(cx, |state| {
    let previous = state.active_project;

    if previous == Some(project_id) {
        return StoreChange::unchanged(previous);
    }

    state.active_project = Some(project_id);
    StoreChange::changed(previous)
});

let changed = outcome.is_changed();
let previous = outcome.into_result();
```

`Changed` 或 `Unchanged` variant 同时就是通知使用的变化决定，而 `R` 仍会返回给调用者。
`update_if` 不会 clone `S`、比较整个状态或回滚修改。只有在可观察状态没有变化时，闭包
才能返回 `Unchanged`。

这样可以把相等性保留在操作本地。大状态可以只比较一个字段，集合可以比较稳定 key，
领域类型也可以使用自己的语义相等性，而不需要全状态 trait bound。

### 7.4 通知是修改契约的一部分

store 修改和通知是一个操作。调用者无法获取 mutable reference，在不选择上述任一种通知
语义的情况下修改 `S`。

mutation closure 和 destructor 的 panic 不会被捕获或包装，而是直接向调用者传播。
`update` 与 `update_if` 只有在 closure 正常返回后才通知。

store 不暴露 revision、mutation origin、action、reducer 或 change set。如果领域需要这些
概念，应把它们显式建模为 `S` 的一部分或 command 层的一部分。

## 8. 可复用 selector

selector 从借用的状态计算一个值：

```rust
pub trait Select<S: ?Sized> {
    type Output;

    fn select(&self, source: &S) -> Self::Output;
}
```

函数和 `Fn` 闭包会自动实现 `Select`：

```rust
let sidebar_open = workspace.select(
    cx,
    |state: &WorkspaceState| state.sidebar_open,
);
```

当多个使用者共享同一个投影时，使用具名 selector：

```rust
use gpui_store::Select;

#[derive(Clone, Copy)]
struct IsProjectActive(ProjectId);

impl Select<WorkspaceState> for IsProjectActive {
    type Output = bool;

    fn select(&self, state: &WorkspaceState) -> Self::Output {
        state.active_project == Some(self.0)
    }
}

let is_active = workspace.select(cx, IsProjectActive(project_id));
```

`Select` 本身不要求 `Clone`、`PartialEq` 或 `'static`。保存 selector 或其输出的 API
只会增加对应使用方式真正需要的 bound。

selector 应该：

- 纯粹且没有副作用；
- 足够廉价，可以在相关 store 通知后运行；
- 对相同的 `S` 具有确定性；
- 不读取文件、数据库、网络或无关 entity。

如果计算输出需要异步工作或可能失败，应在 selector 之外完成，并通过显式 store 修改发布
结果。

## 9. StoreSelection

`Store::select` 创建一个属于调用组件的只读 `StoreSelection<T>`：

```rust
use gpui_store::{Store, StoreSelection};

struct WorkspaceHeader {
    workspace: Store<WorkspaceState>,
    active_project: StoreSelection<Option<ProjectId>>,
}

impl WorkspaceHeader {
    fn new(workspace: Store<WorkspaceState>, cx: &mut Context<Self>) -> Self {
        let active_project =
            workspace.select(cx, |state: &WorkspaceState| state.active_project);

        Self {
            workspace,
            active_project,
        }
    }
}
```

selection 会：

1. 在创建时计算初始输出；
2. 在 source store 发布后重新计算；
3. 比较新输出和上一次输出；
4. 只在输出变化时通知 owner。

因此，无关的 store update 不会让只使用这个 selection 的组件重绘。

通过闭包读取 selection 输出：

```rust
let has_project = self
    .active_project
    .read(|project_id| project_id.is_some());
```

只有确实需要 owned 值时才 clone 输出：

```rust
let active_project = self.active_project.cloned();
```

为了过滤变化，`StoreSelection<T>` 要求 `T: PartialEq + 'static`。`cloned` 还要求
`T: Clone`。

selection 没有 `set`、`update` 或 mutable reference。command 始终通过 source
`Store<S>` 写入。

## 10. 观察变化

selection 用于组件渲染时读取数据。observation 用于让副作用先与当前值同步，然后响应后续
变化。

只要 observation 需要保持活跃，就要把每个返回的 `Subscription` 保存在 owner 中。

observation 契约会在注册后安排一次初始投递，之后再投递后续发布或 selected value
变化。第一次 callback 读取投递时的当前值，不会在注册过程中重入调用，并且一定先于后续
change callback；第一次 callback 前发生的变化可以合并进这次当前值。这样 controller
不需要额外 bootstrap read 就能建立派生状态。

### 10.1 观察整个 store

当每次发布的 store 变化都重要时，使用 `observe`：

```rust
struct WorkspaceController {
    workspace: Store<WorkspaceState>,
    subscriptions: Vec<Subscription>,
}

let subscription = workspace.observe(cx, |this, state, cx| {
    this.rebuild_commands(state);
    cx.notify();
});

self.subscriptions.push(subscription);
```

callback 会在安排好的初始投递中先收到当前 `S`，随后在每次通知时运行，包括 `set` 或
`update` 的结果按照领域规则比较后恰好相等的情况。如果只关心一个投影，使用 selected
observation。

### 10.2 观察 selected value

使用 `observe_select`，只在 selected 输出变化时运行副作用：

```rust
let subscription = workspace.observe_select(
    cx,
    |state: &WorkspaceState| state.active_project,
    |this, active_project, cx| {
        this.rebuild_project_actions(*active_project);
        cx.notify();
    },
);
```

selector 输出必须实现 `PartialEq + 'static`。callback 会收到新的 selected 输出。

observer 会在注册后安排一次 callback，并在投递时读取当前输出。之后只有 `PartialEq`
判断 selected 输出发生变化时才调用 callback。

### 10.3 使用 window 观察

当副作用还需要 `Window` 时，使用 `observe_select_in`：

```rust
let subscription = workspace.observe_select_in(
    cx,
    window,
    |state: &WorkspaceState| state.sidebar_open,
    |this, sidebar_open, window, cx| {
        this.sync_sidebar_focus(*sidebar_open, window, cx);
    },
);
```

observer callback 自己决定是否调用 `cx.notify()`。observation API 不会假定每个副作用
都会改变 owner 的渲染状态。

支持在 callback 内 drop 自己返回的 `Subscription`：当前 callback 会完成，但不会再投递
后续 callback。关闭目标 Window 也会取消 `observe_select_in`，无论 initial delivery
仍在 pending，还是 observation 已经 active。

whole-store callback 持有收到的 `&S` borrow，因此在该 callback 内同步写回同一个 Store
属于 programmer error，并会 panic；应把 command defer 到 callback 之后。selected callback
运行时已经释放 source `S` borrow，可以发出显式 command，但仍需避免反馈循环。

避免反馈循环。如果 observer 必须发出另一个 command，应明确 command 边界，并确保它不会
持续重复发布同一个 selected value。

## 11. 选择 selection 还是 observation

| 需求 | API |
| --- | --- |
| 一次读取状态的任意部分 | `Store::read` |
| 从派生值渲染，并跳过无关重绘 | `Store::select` |
| 响应每次 store 通知 | `Store::observe` |
| 只在一个派生值变化时运行副作用 | `Store::observe_select` |
| 运行需要 `Window` 的 selected 副作用 | `Store::observe_select_in` |

不要为了调用副作用而创建 selection，也不要在小而稳定的 selector 已经能表达依赖时观察
整个 store。

## 12. 持久化与 command

store 修改不是持久化。`set`、`update` 和 `update_if` 只修改内存，不会因为文件或数据库
写入失败而失败。

对于持久化写入，先执行领域 command，再发布 committed 结果：

```rust
let saved_project = repository.save_project(input).await?;

projects.update(cx, |state| {
    state.replace(saved_project);
});
```

是否适合 optimistic update 以及如何回滚由应用决定。`gpui-store` 不提供 backend、commit、
reconcile、ack 或 transaction 抽象。

## 13. 表单

`gpui-form` 持有当前可编辑 model、验证状态、baseline 和提交准备。store 可以持有应用最后
一次 committed 的值，但不能静默镜像每次按键。

显式集成流程是：

```text
committed Store 值
  -> form.rebase(committed value)

form.prepare_submit()
  -> 应用 command 或 repository
  -> committed 结果
  -> Store::set/update
```

catalog selection 可以向 control 提供 options。它不能替换 form 的 selected value，也不能
仅仅因为 catalog store 变化就 rebase form。

公开 API 中没有可写 `StoreBinding`。可编辑值使用类型化 form field，应用状态使用显式
store command。

## 14. 所有权和生命周期

- 多个 owner 需要同一份状态时，Clone `Store<S>`。
- 非 global store handle 保存在定义其生命周期的应用对象或组件中。
- 只有真正的应用级状态才安装成 global store。
- 把 `StoreSelection<T>` 保存在渲染它的组件中。
- 把返回的 `Subscription` 保存在执行观察的 owner 中。
- Drop selection 或 subscription 会停止对应的派生观察。
- Drop 一个 store handle 不会影响指向同一 store 的其他 handle。

避免隐藏镜像。组件可以缓存交互局部 UI 状态，但共享领域或应用状态仍应该只有一个权威
owner。

## 15. 公开 API 汇总

公开 API 有意保持精简。

### `Store<S>`

| API | 用途 |
| --- | --- |
| `Store::new` | 从已有有效 `S` 创建共享 store |
| `Store::install_global` | 创建并安装类型化 global store |
| `Store::global` | 获取类型化 global store |
| `Clone` | 不 clone `S`，只共享 handle |
| `read` | 在一次闭包内借用 `S` |
| `set` | 替换 `S` 并发布 |
| `update` | 修改 `S`、返回闭包结果并发布 |
| `update_if` | 修改，并原子返回业务结果和发布决定 |
| `select` | 创建属于 owner 的只读派生 selection |
| `observe` | 观察每次 store 通知 |
| `observe_select` | 观察 selected 输出的变化 |
| `observe_select_in` | 使用 `Window` 观察 selected 输出 |

### `Select<S>`

| API | 用途 |
| --- | --- |
| `type Output` | 派生输出类型 |
| `select(&self, &S)` | 从借用状态计算输出 |
| `Fn` blanket implementation | 无需 adapter，直接使用函数和闭包 |

### `StoreChange<R>`

| API | 用途 |
| --- | --- |
| `changed` | 返回业务结果并发布修改 |
| `unchanged` | 返回业务结果但不发布 |
| `is_changed` | 检查通知决定 |
| `into_result` | 消费 outcome 并返回业务结果 |

### `StoreSelection<T>`

| API | 用途 |
| --- | --- |
| `read` | 在一次闭包内借用当前 selected 输出 |
| `cloned` | 当 `T: Clone` 时 clone 当前输出 |

## 16. 非目标

本 crate 不提供：

- local 和 shared store 两种 variant；
- state marker trait；
- 外部 backend 或 source adapter；
- 持久化、commit、transaction、reconciliation 或 write acknowledgement；
- revision、mutation origin、reducer、action、middleware 或 delta；
- 可写 selection、binding 或自动 form 同步；
- render-time 隐式依赖收集。

这些排除项让 `Store<S>` 保持为可预测的纯内存 primitive，其他库可以在它上面构建能力，
而不会争夺同一份数据的所有权。
