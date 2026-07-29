# gpui-store

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-store` is a small, typed, in-memory state container for GPUI
applications. A `Store<S>` owns one authoritative value, lets callers read and
update it through explicit APIs, and publishes changes to components,
observers, and read-only selections.

The crate deliberately does not load files, query databases, or persist
changes. Application commands or repositories remain responsible for durable
writes.

## Quick start

Create one store for the state that must be shared:

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

Read through a closure and mutate through the store:

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

`update` always publishes a change and may return a business value from its
closure. `update_if` returns a `StoreChange<R>`, which carries the business
result and the caller's notification decision as one atomic outcome. This
keeps equality policy local and does not require
`CounterState: Clone + PartialEq`.

Create a read-only selection when a component only needs part of the state:

```rust
struct CounterPane {
    counter: Store<CounterState>,
    count: StoreSelection<u64>,
}

let count = counter.select(cx, |state: &CounterState| state.count);
```

The selection recomputes after store changes and notifies its owner only when
the selected output changes. It has no setter and never becomes a second
source of truth.

## Global state

A store can be installed and retrieved as a typed application global:

```rust
Store::install_global(cx, CounterState::default());

let counter = Store::<CounterState>::global(cx);
```

Cloning a `Store<S>` clones the handle, not `S`; every clone refers to the same
state.

## Responsibility boundary

Use:

- `gpui-store` for shared, observable, in-memory application state;
- `gpui-form` for editable form models, validation, and submit preparation;
- application services or repositories for persistence and domain commands.

## Documentation

- [User guide](docs/guide.md)
- [使用指南（中文）](docs/guide.zh-CN.md)
- [Documentation index](docs/README.md)
