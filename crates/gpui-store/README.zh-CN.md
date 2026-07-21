# gpui-store

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-store` 是面向 GPUI 应用的类型化状态容器。它提供局部/共享 store、owned snapshot 读取、变化检测、只读 selection 和可选 backend reconcile，同时不把应用状态绑定到特定持久化系统。

## 核心概念

- `LocalStore<S, B>` 由单个组件或 controller 持有；
- `SharedStore<S, B>` 是应用级 GPUI entity；
- 读取返回 owned snapshot，调用方不持有内部 borrow；
- 只有值实际变化时才通知 observer；
- `StoreSelection<T>` 是 committed store state 的只读 projection；
- `StoreBinding<T>` 只用于明确需要写回同一 store 的字段；
- 持久化 backend 是可选能力，与内存状态所有权正交。

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

组件可以从 snapshot 派生渲染 options，但 catalog 刷新不能静默改写 form 拥有的当前选择。

## Store 与表单

`gpui-store` 拥有 committed 应用状态与 catalog。生成的 `gpui-form` store 拥有当前可编辑 typed model、baseline、验证与提交 runtime；bound component 只投影该模型并保存交互局部 UI 状态。三者通过显式边界协作：

```text
committed store snapshot -> form.rebase(committed value)
catalog snapshot -> 只更新组件 options/projection
form.prepare_submit() -> typed output -> store 或 repository command
command success -> reconcile committed store
```

`gpui-store` 不依赖 `gpui-form`，也不提供隐式 store-to-form binding。

## 延伸阅读

- [Complete design (English)](docs/design.md)
- [完整设计（中文）](docs/design.zh-CN.md)
- [API 与 backend 参考](docs/reference.md)
- [文档索引](docs/README.md)
