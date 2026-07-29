# gpui-operation

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-operation` 使用两个完整 runtime 状态机表达由调用者控制的可失败工作。调用者构造 task、
选择 runtime、路由 completion 并发布 owner 通知；库负责状态匹配、payload 移动、取消和恢复
规则。

| Runtime enum | 失败后的处理方式 |
| --- | --- |
| `refresh::Operation<Data, Problem, Task>` | 再执行一次相同获取 |
| `repair::Operation<Data, Problem, Repair, Task>` | 选择一种明确的修复方案 |

数据库查询或远程 catalog 通常使用 `refresh`。损坏的配置文件或无法打开的数据库通常使用
`repair`。

## 同步工作

如果调用者已经同步完成工作，直接用 `Settle` 发送结果，不需要构造 task：

```rust
use std::io;

use gpui_operation::{Settle, Transition, refresh::{Operation, Phase}};

struct Task;

let mut config = Operation::<String, io::Error, Task>::new();
config.transition(Settle(Ok("ready".to_owned())));
assert_eq!(config.data().map(String::as_str), Some("ready"));

config.transition(Settle(Err(io::Error::other("write failed"))));
assert_eq!(config.phase(), Phase::Degraded);
assert_eq!(config.data().map(String::as_str), Some("ready"));
```

`Settle` 在 `Idle` 和 `Ready` 生效。从 `Ready` 结算成功会替换当前数据，失败则保留数据
并进入 `Degraded`；其他状态会忽略该消息。`Complete` 仍专用于运行中的 task，因此取消后
迟到的 `Complete` 不能把 idle operation 结算为新状态。

## 只刷新 Operation

发送与当前稳定状态匹配的消息：

```rust
use std::io;

use gpui_operation::{
    Complete, Load, Refresh, Transition,
    refresh::{Operation, Phase},
};

struct Task;

let mut catalog =
    Operation::<Vec<i32>, io::Error, Task>::new();

catalog.transition(Load(Task));
assert_eq!(catalog.phase(), Phase::Loading);

catalog.transition(Complete(Ok(vec![1, 2])));
assert_eq!(catalog.data().map(Vec::as_slice), Some(&[1, 2][..]));

catalog.transition(Refresh(Task));
catalog.transition(Complete(Err(io::Error::other("scan failed"))));
assert_eq!(catalog.phase(), Phase::Degraded);
assert_eq!(catalog.data().map(Vec::as_slice), Some(&[1, 2][..]));
assert!(catalog.problem().is_some());
```

## 可修复 Operation

带有 Problem 的状态必须使用调用者选择的 Repair：

```rust
use std::io;

use gpui_operation::{
    Complete, Load, Repair, Transition,
    repair::{Operation, Phase},
};

struct Config;
struct Task;

enum ConfigRepair {
    Retry,
    RestoreBackup,
    Reset,
}

let mut config =
    Operation::<Config, io::Error, ConfigRepair, Task>::new();

config.transition(Load(Task));
config.transition(Complete(Err(io::Error::other("invalid config"))));
assert_eq!(config.phase(), Phase::Unavailable);

config.transition(Repair {
    repair: ConfigRepair::RestoreBackup,
    task: Task,
});
assert!(matches!(
    config.active_repair(),
    Some(ConfigRepair::RestoreBackup)
));
```

取消运行中的 operation 会恢复准确的上一个稳定状态。任何转换都不要求 Data、Problem、
Repair 或 Task 实现 `Clone`。

完整 runtime enum 为 `&mut Operation` 实现
`Transition<Settle/Load/Refresh/Retry/Repair/Complete/Cancel>`。如果消息不适用于当前 variant，
operation 保持不变，消息及其 payload 会被 drop。开启可选的 `tracing` feature 后，库会为
被忽略的消息记录 debug event。如果 payload 不能被丢弃，应先匹配当前 variant，再构造工作。

## 更新准确的 Ready Data

应用可以为已提交的内存更新定义领域消息，而不暴露 mutable Data 引用：

```rust
use gpui_operation::{Transition, refresh};

struct Append(i32);
struct CatalogData(Vec<i32>);

impl Transition<Append> for &mut CatalogData {
    type Output = usize;

    fn transition(self, message: Append) -> Self::Output {
        self.0.push(message.0);
        self.0.len()
    }
}

fn apply_append<Problem: std::error::Error, Task>(
    operation: &mut refresh::Operation<CatalogData, Problem, Task>,
    message: Append,
) {
    if let refresh::Operation::Ready(ready) = operation {
        ready.transition(message);
    }
}
```

`&mut Ready<Data>` 会把领域消息委托给 `&mut Data`，并返回领域 Transition 的 Output。
显式匹配 variant 可以保证 refreshing 或 degraded 状态中保留的 Data 仍然只读。

## 具名状态 API

每个 family 仍公开具名状态和 consuming `Transition<Message>`。直接持有某个精确状态时，
这个底层 API 可以在编译期约束合法转换；需要长期保存时，应优先使用 family 的
`Operation` enum。

## 所有权

库有意不提供跨 family 的万能 `Operation<S>`。两个预定义 family enum 可以保存在：

- 组件、文档或窗口范围的 Entity；
- 整个应用生命周期内唯一的普通 Global；
- 需要共享内存状态、selection 和观察能力的 `gpui-store::Store<S>`；
- 普通字段或局部变量。

工作运行时，runtime enum 拥有调用者提供的 task；drop 或取消运行状态会 drop task。库不
spawn、不 await、不路由 completion、不发布通知、不执行持久化，也不替调用者选择 Repair。

完整状态图，以及两类 operation 分别使用 Entity、普通 Global 和 Store 的方式，请查看
[使用指南](docs/guide.zh-CN.md)。

## 文档

- [使用指南](docs/guide.zh-CN.md)
- [English user guide](docs/guide.md)
- [文档索引](docs/README.md)
