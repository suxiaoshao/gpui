# gpui-operation

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-operation` models caller-controlled, fallible work with two complete
runtime state machines. The caller constructs tasks, chooses their runtime,
routes completion, and publishes owner notifications. The library owns the
state matching, payload movement, cancellation, and restoration rules.

| Runtime enum | Use when failure is handled by |
| --- | --- |
| `refresh::Operation<Data, Problem, Task>` | Running the same fetch again |
| `repair::Operation<Data, Problem, Repair, Task>` | Choosing an explicit recovery |

A database query or remote catalog normally uses `refresh`. A malformed
configuration file or a database that cannot be opened normally uses
`repair`.

## Synchronous initial work

When the caller has already completed the initial work synchronously, deliver
its result with `Settle` instead of constructing a task:

```rust
use std::io;

use gpui_operation::{Settle, Transition, refresh::Operation};

struct Task;

let mut config = Operation::<String, io::Error, Task>::new();
config.transition(Settle(Ok("ready".to_owned())));
assert_eq!(config.data().map(String::as_str), Some("ready"));
```

`Settle` is accepted only from `Idle`. `Complete` remains reserved for a
running task, so a late `Complete` after cancellation cannot settle an idle
operation.

## Refresh-only operation

Deliver the message that matches the current settled state:

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

## Repair-capable operation

Problem-bearing states require a caller-selected Repair:

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

Cancelling a running operation restores its exact previous settled state.
No transition requires Data, Problem, Repair, or Task to implement `Clone`.

The complete runtime enums implement
`Transition<Settle/Load/Refresh/Retry/Repair/Complete/Cancel>` for
`&mut Operation`. A message that is invalid for the current variant leaves
the operation unchanged and drops the message and its payload. Enable the
optional `tracing` feature to emit a debug event for ignored messages. Match
the current variant before constructing work when the payload must not be
dropped.

## Updating exact Ready data

Applications can define domain messages for committed in-memory updates
without exposing a mutable Data reference:

```rust
use gpui_operation::{Transition, refresh};

struct Append(i32);
struct CatalogData(Vec<i32>);

impl Transition<Append> for &mut CatalogData {
    type Output = ();

    fn transition(self, message: Append) {
        self.0.push(message.0);
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

`&mut Ready<Data>` delegates domain messages to
`&mut Data: Transition<Message, Output = ()>`. The explicit variant match
keeps retained Data in refreshing or degraded states read-only.

## Named-state API

Each family also exposes named states and consuming
`Transition<Message>` implementations. This lower-level API provides
compile-time transition safety when a caller directly owns one exact state.
The family `Operation` enums are the primary API for long-term storage.

## Ownership

There is deliberately no cross-family universal `Operation<S>`. The two
predefined family enums can be stored in:

- an Entity for component-, document-, or window-scoped work;
- an ordinary Global for one application-lifetime resource;
- a `gpui-store::Store<S>` for shared in-memory state, selections, and
  observation;
- an ordinary field or local variable.

The runtime enum owns the caller-provided task while work is running. Dropping
or cancelling that state drops the task. The library does not spawn, await,
route completion, publish notifications, perform persistence, or choose
repairs.

See the [user guide](docs/guide.md) for complete state graphs and Entity,
ordinary Global, and Store examples for both operation families.

## Documentation

- [User guide](docs/guide.md)
- [中文使用指南](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
