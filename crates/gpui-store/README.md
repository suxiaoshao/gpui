# gpui-store

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-store` is a typed state container for GPUI applications. It provides local and shared stores, owned snapshot reads, change detection, read-only selections, and optional backend reconciliation without coupling application state to a particular persistence system.

## Core ideas

- `LocalStore<S, B>` is owned by one component or controller.
- `SharedStore<S, B>` is an application-wide GPUI entity.
- Reads return owned snapshots so callers do not retain internal borrows.
- Writes notify observers only when the value actually changes.
- `StoreSelection<T>` is a read-only projection from committed store state.
- `StoreBinding<T>` is reserved for a field that intentionally writes back to the same store.
- Persistence backends are optional and orthogonal to in-memory state ownership.

## Local store

```rust,ignore
use gpui::Context;
use gpui_store::{LocalStore, StoreState};

#[derive(Clone, Default, PartialEq)]
struct EditorState {
    query: String,
}

impl StoreState for EditorState {}

struct Editor {
    state: LocalStore<EditorState>,
}

impl Editor {
    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.state.set(cx, |state| &mut state.query, query);
    }
}
```

## Shared store

```rust,ignore
use gpui_store::{SharedStore, StoreState};

#[derive(Clone, Default, PartialEq)]
struct ModelCatalog {
    models: Vec<ModelSummary>,
}

impl StoreState for ModelCatalog {}

let catalog = SharedStore::new(cx, ModelCatalog::default());
let models = catalog.read_cloned(cx, |state| &state.models);
```

Components may derive rendering options from the snapshot, but refreshing a catalog does not silently rewrite a form-owned selection.

## Stores and forms

`gpui-store` owns committed application state and catalogs. A generated `gpui-form` store owns the current editable typed model, its baseline, validation, and submit runtime. Bound components only project that model and keep interaction-local UI state. Integration is explicit:

```text
committed store snapshot -> form.rebase(committed value)
catalog snapshot -> component options/projection only
form.prepare_submit() -> typed output -> store or repository command
command success -> committed store reconcile
```

`gpui-store` does not depend on `gpui-form` and provides no implicit store-to-form binding.

## Further reading

- [Complete design (English)](docs/design.md)
- [完整设计（中文）](docs/design.zh-CN.md)
- [API and backend reference](docs/reference.md)
- [Documentation index](docs/README.md)
