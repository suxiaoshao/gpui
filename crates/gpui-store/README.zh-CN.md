# gpui-store

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-store` 是面向 GPUI 应用的小型、类型化纯内存状态容器。一个 `Store<S>` 持有一份
权威数据，通过显式 API 让调用者读取和修改，并向组件、observer 和只读 selection 发布
变化。

本 crate 有意不读取文件、不查询数据库，也不持久化变更。持久化写入仍由应用命令或
repository 负责。

## 快速开始

为需要共享的状态创建一个 store：

```rust
use gpui_store::{Store, StoreChange};

#[derive(Default)]
struct CounterState {
    count: u64,
    label: String,
}

let counter = Store::new(
    cx,
    CounterState {
        count: 0,
        label: "Requests".into(),
    },
);
```

通过闭包读取，并通过 store 修改：

```rust
let count = counter.read(cx, |state| state.count);

counter.update(cx, |state| {
    state.count += 1;
});

let outcome = counter.update_if(cx, |state| {
    if state.label == "Completed requests" {
        return StoreChange::unchanged(());
    }

    state.label = "Completed requests".into();
    StoreChange::changed(())
});
```

`update` 总是发布变化，并且可以返回闭包产生的业务值。`update_if` 返回
`StoreChange<R>`，在同一个原子结果中携带业务结果和调用者的通知决定。这样相等性策略
保留在调用处，也不要求 `CounterState: Clone + PartialEq`。

当组件只需要状态的一部分时，创建只读 selection：

```rust
struct CounterPane {
    counter: Store<CounterState>,
    count: StoreSelection<u64>,
}

let count = counter.select(cx, |state: &CounterState| state.count);
```

store 变化后，selection 会重新计算，并且只在选中结果变化时通知 owner。它没有 setter，
永远不会成为第二份事实来源。

## 全局状态

store 可以作为类型化的应用 global 安装和获取：

```rust
Store::install_global(cx, CounterState::default());

let counter = Store::<CounterState>::global(cx);
```

Clone `Store<S>` 只会 clone handle，不会 clone `S`；所有 handle 都指向同一份状态。

## 职责边界

使用：

- `gpui-store` 保存共享、可观察的纯内存应用状态；
- `gpui-form` 处理可编辑表单 model、验证和提交准备；
- 应用 service 或 repository 处理持久化和领域命令。

## 文档

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [文档索引](docs/README.md)
