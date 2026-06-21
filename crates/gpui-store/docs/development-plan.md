# gpui-store 开发计划

状态：capability-safe backend API 已实现。早期实验 API 已移除，默认不再要求用户定义
`Change` / action / reducer；`StoreSource` 命名已经迁移为 `StoreBackend` /
`StoreCommitBackend`，只读 projection 在类型层面没有 `set/update/bind`，可提交
backend 只能通过 fallible `try_*` API 更新。

本文档是内部开发计划，不是 README。README 展示目标使用方式；本文档固定模块结构、内部类型、数据流和实现阶段。

## 设计修正

之前的设计把 `StoreState::Change` 作为核心约束，导致用户要为每个 state 写：

- change struct / enum
- `StoreChange::is_empty`
- `StoreChange::merge`
- 每个 setter 返回 change
- external source 按 change 写回

这接近 Redux/change-set 思路，模板太多。新的默认模型是：

- 用户直接改状态。
- 库判断是否真的变了。
- 变了才 bump revision 和 `cx.notify()`。
- selector/binding 只在自己的 snapshot 变化时 notify owner。
- memory store 默认直接写 in-memory state。
- committed backend 先提交 draft，提交成功后才更新 store snapshot。
- read-only projection 只能从外部 committed snapshot 同步，不暴露 generic local setter。
- optional delta 只作为性能或 effect routing 逃生口。

`Change` / `Delta` 从核心必需品降级为高级可选能力。

## 设计目标

- 符合 GPUI ownership / `Entity` / `cx.notify()` 语义。
- 默认 memory API 像 Zustand：直接 `set` / `update`，不写 action/reducer/change。
- 后端提交 API 使用 `try_set` / `try_update`，避免持久化失败后 UI state 已经先变更。
- 保留 Rust 类型不变量，domain state 仍然是普通 Rust struct。
- 支持一个 store 多个 `StoreSelection<T>` / `StoreBinding<T>`。
- `StoreSelection<T>` 和 `StoreBinding<T>` 自动持有订阅，并通知创建它们的 owner component。
- 支持 `observe_select` / `observe_select_in`，用于只在 selected value 变化时执行副作用，
  避免 observe 整个 store 后因无关字段变化触发重活。
- 支持组件私有 `LocalStore<S>`，避免所有状态都包 entity。
- backend 是和 ownership 正交的维度：`LocalStore` / `SharedStore` 都能接
  `MemoryBackend`、文件、S3、HTTP、数据库 projection 或用户自定义 backend。
- 后端同步由用户实现 `StoreBackend<S>` trait；load/subscribe/reconcile
  的 GPUI glue 收进库里。
- 后端写入能力单独由 `StoreCommitBackend<S>` 表达；没有实现这个 trait 的
  backend 不应该能调用 `try_set` / `try_update` 或创建 writable binding。
- 复杂场景保留底层 primitive 和 optional delta escape hatch。

## 非目标

- 不做 Redux action/reducer/middleware registry。
- 不做 render-time 隐式依赖收集。
- 不把整个数据库镜像进一个全局 store。
- 不内置 TOML、SQLite、S3、keychain、repository 或具体应用 schema。
- 不把 file/database 写成特殊 store 类型；它们只能是 `StoreBackend<S>` 的
  便捷 adapter 或用户实现。
- 不在 store crate 内保存任何应用接入计划。
- 不用 runtime `policy` / enum 来弥补类型设计；读、提交、projection 能力必须
  由 trait bound 决定。

## 命名规则

公开类型按职责分组命名，不暴露早期实验 API 名字。

| 分组 | 目标命名 | 说明 |
| --- | --- | --- |
| Ownership wrapper | `LocalStore<S, Backend = MemoryBackend>` | 当前组件持有，直接 notify owner。 |
| Ownership wrapper | `SharedStore<S, Backend = MemoryBackend>` | GPUI entity-backed，可跨组件共享。 |
| Smart snapshot | `StoreSelection<T>` | 只读派生 snapshot。 |
| Smart snapshot | `StoreBinding<T, Error = Infallible>` | 可写 lens；memory binding 提供 `set/update`，committed binding 提供 `try_set/try_update`。 |
| Selected observer | `observe_select` / `observe_select_in` | selected value 变化时执行副作用，返回 owner 持有的 `Subscription`。 |
| Backend trait | `StoreBackend<S>` | 用户实现的外部 load/subscribe/reconcile 扩展点。 |
| Backend commit trait | `StoreCommitBackend<S>` | 可提交 local draft 的 backend 单独实现。 |
| Backend default | `MemoryBackend` | 默认 memory-only/no-op backend。 |
| Backend builder | `StoreBackendBuilder` | closure adapter，不是核心抽象。 |
| Backend helpers | `StoreBackendId` / `StoreBackendFuture` / `StoreBackendCallback` / `StoreCommitAck` | backend 子系统辅助类型统一加 `StoreBackend` / `StoreCommit` 前缀。 |
| Update metadata | `StoreRevision` / `StoreUpdate` / `StoreUpdateOrigin` | mutation 结果和版本信息，不再使用 change/action 命名。 |

旧名映射：

| 旧名 | 新名 | 原因 |
| --- | --- | --- |
| `StoreEntity` | `SharedStore` | 公开 API 表达共享语义，entity 是实现细节。 |
| `Selection` | `StoreSelection` | 避免和普通 UI selection 概念混淆。 |
| `Binding` | `StoreBinding` | 避免和组件库/输入绑定概念混淆。 |
| `NoSource` | `MemoryBackend` | 默认 backend 仍属于 backend 维度，只是 no-op。 |
| `MemorySource` | `MemoryBackend` | 这里表达的是 store 后端维度，不是 event/source policy。 |
| `StoreSource` | `StoreBackend` | `source` 容易暗示只有外部数据源；backend 也覆盖 memory、file、projection 和 remote。 |
| `StoreSourceBuilder` | `StoreBackendBuilder` | builder 不等于 backend trait，也不只代表 external backend。 |
| `StoreSourceId` | `StoreBackendId` | 和 backend 命名对齐。 |
| `StoreSourceWriteAck` | `StoreCommitAck` | ack 来自 commit 能力，不属于所有 backend。 |
| `StoreSourceUnsupported` | `StoreBackendUnsupported` | 和 backend 命名对齐。 |
| `ExternalSource` | `StoreBackendBuilder` | builder 不等于 backend trait，也不只代表 external backend。 |
| `ChangeOrigin` | `StoreUpdateOrigin` | 新模型不是 change/action 驱动。 |

## 目标模块结构

```text
crates/gpui-store/src/lib.rs
crates/gpui-store/src/store.rs
crates/gpui-store/src/shared.rs
crates/gpui-store/src/local.rs
crates/gpui-store/src/selection.rs
crates/gpui-store/src/binding.rs
crates/gpui-store/src/backend.rs
crates/gpui-store/src/delta.rs
crates/gpui-store/src/error.rs
crates/gpui-store/src/test_support.rs
```

模块职责：

- `store.rs`：纯 store core、revision、origin、update result、changed detection。
- `shared.rs`：`SharedStore<S, Backend = MemoryBackend>`，shared observable GPUI entity
  wrapper、global helper、selected observer helper 和 entity 级 backend lifecycle。
- `local.rs`：`LocalStore<S, Backend = MemoryBackend>`，组件私有 store，不创建额外
  entity，backend lifecycle 绑定 owner component。
- `selection.rs`：`StoreSelection<T>`，只读 subscribed snapshot。
- `binding.rs`：`StoreBinding<T, Error = Infallible>`，可写 subscribed lens。
- `backend.rs`：`StoreBackend<S>` trait、`StoreCommitBackend<S>` trait、
  `MemoryBackend`、`StoreBackendId`、`StoreBackendBuilder` closure adapter。
- initial sync、external event、snapshot reconcile、commit coordination 当前在
  `shared.rs` / `local.rs` 的 runtime impl 中实现；尚未拆出 `sync.rs`。
- `delta.rs`：可选 delta/change escape hatch。
- `error.rs`：通用错误类型。
- `test_support.rs`：crate 自测 helper；默认不作为稳定外部 API。

早期 `change.rs`、`selected.rs`、`external.rs` 已移除，避免继续暴露旧心智模型。

## 核心类型

### StoreState

目标形态：

```rust
pub trait StoreState: 'static {}
```

`StoreState` 只是 marker。默认 changed detection 依赖具体 API：

- `set` 比较单字段。
- `update` 需要 `S: Clone + PartialEq`。
- `update_if` 由用户返回 `bool`。
- `StoreDelta` 目前只是 marker；尚未接入 `update_delta` 写入 API。

### StoreCore

纯 Rust core，不直接依赖 GPUI context。

```rust
pub struct StoreCore<S> {
    state: S,
    revision: StoreRevision,
    last_origin: Option<StoreUpdateOrigin>,
}

pub struct StoreUpdate {
    revision: StoreRevision,
    changed: bool,
    origin: StoreUpdateOrigin,
}
```

职责：

- 拥有 state。
- 统一处理 revision。
- 统一处理 old/new equality。
- 不调用 `cx.notify()`。

目标 API：

```rust
impl<S> StoreCore<S> {
    pub fn set<T: PartialEq>(&mut self, field: impl FnOnce(&mut S) -> &mut T, value: T) -> StoreUpdate;
    pub fn update(&mut self, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq;
    pub fn update_if(&mut self, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate;
}
```

### SharedStore

shared observable store。内部持有 `Entity<StoreRuntime<S, Backend>>`，对外隐藏底层 entity 更新模板。

目标 API：

```rust
impl<S: StoreState> SharedStore<S, MemoryBackend> {
    pub fn new(cx: &mut impl AppContext, initial: S) -> Self;
    pub fn install_global(cx: &mut App, initial: S) -> Self;
}

impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn new_with_backend(cx: &mut impl AppContext, initial: S, backend: Backend) -> Result<Self>;
    pub fn install_global_with_backend(cx: &mut App, initial: S, backend: Backend) -> Result<Self>;
    pub fn install_global_with_default(cx: &mut App, backend: Backend) -> Result<Self>
    where
        S: Default;
    pub fn global(cx: &impl AppContext) -> Self;

    pub fn read<R>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> R) -> R;
    pub fn read_cloned<T: Clone>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> &T) -> T;
    pub fn revision(&self, cx: &impl AppContext) -> StoreRevision;
    pub fn refresh_from_backend(&self, cx: &mut impl AppContext) -> Result<StoreUpdate, Backend::Error>;
    pub fn sync_snapshot(&self, cx: &mut impl AppContext, snapshot: Backend::Snapshot) -> Result<StoreUpdate, Backend::Error>;
    pub fn select<Owner, T>(&self, cx: &mut Context<Owner>, select: impl Fn(&S) -> T + 'static) -> StoreSelection<T>;
    pub fn select_cloned<Owner, T>(&self, cx: &mut Context<Owner>, select: impl Fn(&S) -> &T + 'static) -> StoreSelection<T>
    where
        Owner: 'static,
        T: Clone + PartialEq + 'static;
    pub fn observe_select<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static;
    pub fn observe_select_in<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static;
}

impl<S> SharedStore<S, MemoryBackend>
where
    S: StoreState,
{
    pub fn set<T: PartialEq>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, value: T) -> StoreUpdate;
    pub fn update(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq;
    pub fn update_if(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate;
    pub fn bind<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T>;
}

impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState + Clone + PartialEq,
    Backend: StoreCommitBackend<S>,
{
    pub fn try_set<T: PartialEq>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, value: T) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S)) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update_if(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S) -> bool) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update_field<T>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, update: impl FnOnce(&mut T)) -> Result<StoreUpdate, Backend::Error>;
    pub fn bind_committed<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T, Backend::Error>;
    pub fn bind_committed_field<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T, Backend::Error>;
}
```

`SharedStore` 只有在 store state 真实变化时才对自身 entity `cx.notify()`。
如果带 backend，initial load、subscribe handle、commit task 都由
`StoreRuntime<S, Backend>` 持有，生命周期跟随 store entity/global，而不是任意一个
consumer component。

关键约束：

- `sync_snapshot` 是 store wrapper 的方法，不是 `StoreBackend<S>` trait method。
  backend 只定义 snapshot 如何 reconcile；store 负责 revision、notify 和
  selection/observer refresh。
- `set/update/update_if/bind` 只在 `MemoryBackend` impl 上存在。
- `try_set/try_update/try_update_if/try_update_field/bind_committed` 只在
  `Backend: StoreCommitBackend<S>` impl 上存在。
- 只实现 `StoreBackend<S>` 的 projection backend 没有任何 generic mutation API。

### LocalStore

组件私有 store，不创建额外 entity，不支持其他组件订阅。默认是 `MemoryBackend`；
带 backend 时，backend 的订阅和 task 生命周期绑定创建它的 owner component。

目标 API：

```rust
impl<S: StoreState> LocalStore<S, MemoryBackend> {
    pub fn new(initial: S) -> Self;
}

impl<S, Backend> LocalStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn with_backend<Owner>(cx: &mut Context<Owner>, initial: S, backend: Backend) -> Result<Self>;

    pub fn read(&self) -> &S;
    pub fn read_cloned<T: Clone>(&self, f: impl FnOnce(&S) -> &T) -> T;
    pub fn revision(&self) -> StoreRevision;
    pub fn refresh_from_backend<Owner>(&mut self, cx: &mut Context<Owner>) -> Result<StoreUpdate, Backend::Error>;
    pub fn sync_snapshot<Owner>(&mut self, cx: &mut Context<Owner>, snapshot: Backend::Snapshot) -> Result<StoreUpdate, Backend::Error>;
}

impl<S> LocalStore<S, MemoryBackend>
where
    S: StoreState,
{
    pub fn set<Owner, T: PartialEq>(&mut self, cx: &mut Context<Owner>, field: impl FnOnce(&mut S) -> &mut T, value: T) -> StoreUpdate;
    pub fn update<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq;
    pub fn update_if<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate;
}

impl<S, Backend> LocalStore<S, Backend>
where
    S: StoreState + Clone + PartialEq,
    Backend: StoreCommitBackend<S>,
{
    pub fn try_set<Owner, T: PartialEq>(&mut self, cx: &mut Context<Owner>, field: impl FnOnce(&mut S) -> &mut T, value: T) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S)) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update_if<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S) -> bool) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update_field<Owner, T>(&mut self, cx: &mut Context<Owner>, field: impl FnOnce(&mut S) -> &mut T, update: impl FnOnce(&mut T)) -> Result<StoreUpdate, Backend::Error>;
}
```

`LocalStore` 直接通知 owner component。它不解决跨组件订阅问题，但可以通过
`StoreBackend<S>` 和外部系统同步。也就是说：

```text
LocalStore<S, MemoryBackend>         -> component-private memory state
LocalStore<S, FilePrefsBackend>      -> component owns file-backed committed state
LocalStore<S, S3PrefsBackend>        -> component owns S3-backed committed state
SharedStore<S, MemoryBackend>        -> shared in-memory state
SharedStore<S, S3PrefsBackend>       -> shared S3-backed committed state
SharedStore<S, DbProjectionBackend>  -> shared read-only database projection
```

### StoreSelection

只读 subscribed snapshot。

```rust
pub struct StoreSelection<T> {
    snapshot: Rc<SnapshotCell<T>>,
    // hidden selected entity / subscription handles
}
```

职责：

- 在创建时从 store 取初始 snapshot。
- 持有 store subscription。
- store changed 后重新计算 selector。
- 只有 `T` 变化时通知 owner component。
- 不提供 `set`。

`StoreSelection<T>` 的 owner notify 必须由库自动处理。用户不应该再手写 `cx.observe(selected, ...)`。

`StoreSelection<T>` 不暴露 `&T` 到可替换的内部存储。公开读取必须让调用方显式选择
owned snapshot 或闭包内短借用：

```rust
impl<T> StoreSelection<T> {
    pub fn snapshot(&self) -> Rc<T>;
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R;
    pub fn cloned(&self) -> T
    where
        T: Clone;
}
```

需要跨后续 store notification 保留旧值时，用 `snapshot() -> Rc<T>`。只在当前表达式
读取时，用 `read(|value| ...)`。需要 owned value 时，用 `cloned()`。

### Selected Observer

`observe_select` / `observe_select_in` 是 selected value 级别的副作用订阅。它不返回
snapshot handle，而是返回 owner 持有的 `Subscription`。

目标 API：

```rust
impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn observe_select<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static;

    pub fn observe_select_in<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static;
}
```

语义：

- 创建时读取一次 selected value 作为 previous snapshot，但不立即调用 callback。
- store changed 后重新计算 selector。
- selected value 相等时停止，不调用 callback。
- selected value 变化时更新 previous snapshot，然后调用 callback。
- `observe_select` 不自动调用 `cx.notify()`；callback 自己决定是否需要 notify。
- `observe_select_in` 只是在 callback 中额外传入 `Window`，用于 theme、menu、i18n
  这类需要 window context 的副作用。

适合场景：

- config 里的 language 变化后重载 i18n / menu。
- theme 变化后应用 theme。
- 登录状态、权限、feature flag 变化后刷新外部 service。

不适合场景：

- render 里要读取值：用 `StoreSelection<T>`。
- 字段要写回 store：用 `StoreBinding<T, E>` 或 store-level mutation。
- 副作用依赖整个 state 且任何字段变化都应触发：直接 observe store entity。

### Helper API

这些 helper 目标是减少 app 接入时的模板代码，而不是改变数据所有权：

| Helper | 目标模板压缩 |
| --- | --- |
| `read_cloned(cx, |state| &state.field)` | 替代每个调用点的 `read(cx, |state| state.field.clone())`。 |
| `select_cloned(cx, |state| &state.field)` | 替代 owner 初始化里的 `select(cx, |state| state.field.clone())`。 |
| `observe_select` / `observe_select_in` | 替代 `entity()` + `observe_in` + 手动读取/比较 selected field。 |
| `refresh_from_backend(cx)` | 替代 repository projection 写入后手动 `list_*` + `sync_snapshot`。 |
| `sync_snapshot(cx, snapshot)` | 保留给 command 已经返回 committed snapshot 的场景。 |
| `try_update_field(cx, |state| &mut state.field, update)` | 替代 committed backend 上 clone 全量 state、改一个字段、save、sync 的 helper。 |
| `bind_committed` / `bind_committed_field` | 替代表单组件里“binding 读 + 单独 persistence helper 写”的双路径。 |
| `install_global_with_default(cx, backend)` | 替代 backend-backed store 初始化时重复传 `S::default()`。 |

`StoreBackendBuilder` 应覆盖最常见 reconcile 模板：

```rust
StoreBackendBuilder::new("file:prefs")
    .load(load_prefs)
    .reconcile_replace()
    .commit_snapshot(save_prefs);

StoreBackendBuilder::new("database:prompts")
    .load(list_prompts)
    .reconcile_field(|state: &mut PromptState| &mut state.prompts);
```

### StoreBinding

可写 subscribed lens。它是带 setter 的 selection，但写入能力取决于创建它的
store capability。

```rust
pub struct StoreBinding<T, Error = Infallible> {
    snapshot: Rc<SnapshotCell<T>>,
    // hidden store handle, getter, setter, subscription handles
}
```

目标 API：

```rust
impl<T, Error> StoreBinding<T, Error> {
    pub fn snapshot(&self) -> Rc<T>;
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R;
    pub fn cloned(&self) -> T
    where
        T: Clone;
    pub fn store_revision(&self) -> StoreRevision;
}

impl<T> StoreBinding<T, Infallible> {
    pub fn set<Owner>(&self, cx: &mut Context<Owner>, value: T) -> StoreUpdate
    where
        T: Clone + PartialEq;
    pub fn update<Owner>(&self, cx: &mut Context<Owner>, f: impl FnOnce(&mut T)) -> StoreUpdate
    where
        T: Clone + PartialEq;
}

impl<T, Error> StoreBinding<T, Error> {
    pub fn try_set<Owner>(&self, cx: &mut Context<Owner>, value: T) -> Result<StoreUpdate, Error>
    where
        T: Clone + PartialEq;
    pub fn try_update<Owner>(&self, cx: &mut Context<Owner>, f: impl FnOnce(&mut T)) -> Result<StoreUpdate, Error>
    where
        T: Clone + PartialEq;
}
```

memory `StoreBinding<T, Infallible>` 的更新流程：

```text
binding.set(value)
  -> store.update_if(...)
  -> setter writes value into store state
  -> store compares old binding snapshot or old whole state
  -> changed: store revision += 1 and notify store observers
  -> binding refreshes from store
  -> owner component notify only if binding snapshot changed
```

committed `StoreBinding<T, E>` 的更新流程：

```text
binding.try_set(value)
  -> clone whole store state into draft
  -> setter writes value into draft
  -> unchanged: stop
  -> backend.commit_snapshot(draft)
  -> error: return Err, do not change store, do not notify
  -> success: install committed draft or reconcile commit ack snapshot
  -> changed: store revision += 1 and notify store observers
  -> binding refreshes from store
  -> owner component notify only if binding snapshot changed
```

`StoreBinding` 不拥有第二份 source of truth。它的 snapshot 是 cache，写入永远回到 store。

`StoreBinding<T, E>` 不能实现 `Deref` / `AsRef` / `Borrow` / `DerefMut`。这些 trait
会把内部 snapshot 暴露成没有 owned guard 的 `&T`，或允许绕过 store 直接修改。可写能力必须经过 `set/update` 或
`try_set/try_update` 回到 store，否则会绕过 changed detection、revision、owner
notify 和 backend commit。

### StoreSelection / StoreBinding 读取与 Trait 策略

推荐实现：

| API / Trait | `StoreSelection<T>` | `StoreBinding<T, E>` | 说明 |
| --- | --- | --- | --- |
| `snapshot() -> Rc<T>` | yes | yes | 让调用方显式持有可跨后续更新存活的旧 snapshot。 |
| `read(|&T| ...)` | yes | yes | 只在闭包内短暂借用当前 snapshot。 |
| `cloned() -> T` | `T: Clone` | `T: Clone` | 需要 owned value 时的便捷 API。 |
| `Debug` | `T: Debug` | `T: Debug` | 转发给 snapshot。 |
| `Display` | `T: Display` | `T: Display` | 转发给 snapshot。 |
| `PartialEq` | `T: PartialEq` | `T: PartialEq` | 按 snapshot 比较。 |
| `Eq` | `T: Eq` | `T: Eq` | 按 snapshot 语义。 |

明确不实现：

| Trait | 原因 |
| --- | --- |
| `Deref` / `AsRef<T>` / `Borrow<T>` | 会暴露没有 owned snapshot guard 的 `&T`；后续 store notification 可能替换内部 snapshot。 |
| `DerefMut` / `BorrowMut` | 会允许绕过 store 直接改 snapshot。 |
| `Copy` | handle 持有订阅和 cached snapshot，不是 plain value。 |
| 默认 `Clone` | owner-bound 订阅被 clone 后语义不清楚。 |
| `Hash` | snapshot 会随订阅刷新，作为 hash key 容易违反稳定性预期。 |

如果后续确实需要 cloneable handle，应单独设计：

```rust
StoreSelectionHandle<T>
StoreBindingHandle<T, E>
```

组件字段中的 `StoreSelection<T>` / `StoreBinding<T, E>` 默认不 clone。需要值副本时显式 clone snapshot：

```rust
let value = self.binding.cloned();
```

## Store Backend

目标：用户实现“怎么读、怎么订阅、怎么把 snapshot merge 回 state”，库实现
GPUI glue、task lifecycle、changed detection、revision、selection refresh 和 owner
notify。写入能力单独放到 `StoreCommitBackend<S>`，避免 projection backend 被误当成
可写 store。

能力边界：

- `StoreBackend<S>` trait 自身只定义 `load` / `subscribe` / `load_after_event` /
  `reconcile`。
- `sync_snapshot` 是 `SharedStore` / `LocalStore` 的方法，适用于任意
  `Backend: StoreBackend<S>`。
- `StoreCommitBackend<S>` 单独定义 committed write，适用于需要 `try_*` 写入的 backend。
- `MemoryBackend` 是纯内存路径，适用于 infallible `set/update/bind`。

### Trait API

`StoreBackend<S>` 是基础扩展点。文件、S3、HTTP、Keychain、数据库 projection 都应该
通过这个 trait 接入，而不是在库里写死 backend 分支。

```rust
pub trait StoreBackend<S: StoreState>: 'static {
    type Snapshot: Clone + PartialEq + 'static;
    type Event: 'static;
    type Subscription: 'static;
    type Error: 'static;

    fn backend_id(&self) -> StoreBackendId;

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(None)
    }

    fn subscribe(
        &self,
        on_change: StoreBackendCallback<Self::Event>,
    ) -> StoreBackendFuture<Option<Self::Subscription>, Self::Error> {
        Ok(None)
    }

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        self.load()
    }

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool;
}

pub trait StoreCommitBackend<S: StoreState>: StoreBackend<S> {
    fn commit_snapshot(
        &self,
        draft: &S,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Self::Snapshot>>, Self::Error>;
}
```

默认实现策略：

- `MemoryBackend` 是默认 backend。它只用于 memory impl 上暴露 `set/update/bind`；
  不需要实现 `StoreCommitBackend<S>`。
- `subscribe` 默认 no-op；没有 live watch 时不要求用户写空实现。
- `load_after_event` 默认调用 `load`，有增量事件能力的 backend 可以覆盖。
- `reconcile` 必须返回 `bool`，表示 state 是否真实变化。
- `commit_snapshot` 没有默认 no-op；只有真正可提交 local draft 的 backend 才实现
  `StoreCommitBackend<S>`。
- 当前 `StoreBackendFuture<T, E>` 是 `Result<T, E>` alias；第一版先实现同步 backend
  glue。后续如果接 S3/HTTP 这类真实异步 backend，再把 alias 升级成 GPUI
  task/future 形态。

### Commit Flow

`try_update` / `try_set` 不能先改 store 再尝试持久化。正确顺序是：

```text
try_update
  -> clone current state as draft
  -> mutate draft
  -> compare draft with current state
  -> unchanged: return unchanged StoreUpdate
  -> backend.commit_snapshot(&draft)
  -> error: return Err, keep store unchanged, do not notify
  -> success with ack snapshot: reconcile ack snapshot into store
  -> success without snapshot: replace store state with committed draft
  -> changed: bump revision and notify
```

这条约束是本次 API 重设计的核心。只给旧 API 加一个 `try_set` 不够，因为调用者仍然
可能在需要 commit 的 backend 上误用 infallible `set`。

### Closure Adapter

`StoreBackendBuilder` 是便捷 adapter，不是核心抽象。它让常见文件/配置场景不用手写一个
struct + trait impl。

```rust
let backend = StoreBackendBuilder::new("file:prefs")
    .load(load_snapshot)
    .reconcile_replace()
    .commit_snapshot(save_draft);

let store = SharedStore::install_global_with_backend(cx, State::default(), backend)?;
```

只替换 state 中一个 projection 字段时，用 `reconcile_field`：

```rust
let backend = StoreBackendBuilder::new("database:prompts")
    .load(list_prompts)
    .reconcile_field(|state: &mut PromptState| &mut state.prompts);
```

自定义 backend 直接实现 `StoreBackend<S>`；只有可提交 local draft 时才额外实现
`StoreCommitBackend<S>`：

```rust
struct S3PrefsBackend {
    bucket: String,
    key: String,
    client: S3Client,
}

impl StoreBackend<Prefs> for S3PrefsBackend {
    type Snapshot = Prefs;
    type Event = S3ObjectVersion;
    type Subscription = S3Watcher;
    type Error = anyhow::Error;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new(format!("s3:{}/{}", self.bucket, self.key))
    }

    fn reconcile(&self, state: &mut Prefs, snapshot: Prefs) -> bool {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    }

    // load / subscribe 由应用按自己的 S3 client 实现。
}

impl StoreCommitBackend<Prefs> for S3PrefsBackend {
    fn commit_snapshot(
        &self,
        draft: &Prefs,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Prefs>>, Self::Error> {
        self.client.save_toml(&self.bucket, &self.key, draft)?;
        Ok(None)
    }
}
```

同一个 backend 能接到任意 ownership wrapper：

```rust
let local = LocalStore::with_backend(cx, Prefs::default(), s3_backend)?;
let global = SharedStore::install_global_with_backend(cx, Prefs::default(), s3_backend)?;
```

`LocalStore` 的 live backend event 需要用户提供 owner accessor，因为库无法凭泛型知道
store 字段位于组件的哪个位置：

```rust
store.subscribe(cx, |owner: &mut PrefsPane| &mut owner.prefs)?;
```

`SharedStore` 不需要 accessor；backend subscription 生命周期跟随内部 entity。

### Backend Semantics

同步语义不通过单独的 enum 表达。读、订阅、提交能力由 trait method 和 trait bound
共同表达：

| Shape | Source of truth | Store role | Store API |
| --- | --- | --- | --- |
| `MemoryBackend` | Store itself | transient in-memory state | `set/update/bind` |
| `StoreBackend` with `load` | External snapshot | hydrated UI snapshot | `read/select/observe_select/sync_snapshot` |
| `StoreBackend` with `subscribe` / `load_after_event` | External backend can push changes | live projection | `read/select/observe_select/sync_snapshot` unless commit trait exists |
| `StoreCommitBackend` | Backend commit result | committed UI snapshot | `try_set/try_update/bind_committed` |
| `StoreBackend` + repository command 后手动 `sync_snapshot` | External committed result | UI snapshot over committed state | domain command only |

`StoreBackendId` 描述实际 backend identity，例如：

- `memory:runtime`
- `file:prefs`
- `s3:prefs`
- `http:settings`
- `database:prompts`

### Database Flow

数据库 projection 不实现 `StoreCommitBackend<S>`，因此没有 generic setter。

推荐 flow：

```text
UI command
  -> repository transaction
  -> committed snapshot
  -> store.sync_snapshot(committed snapshot)
  -> selections update from committed data
```

这样数据库仍是 durable source of truth，store 只是 UI snapshot。需要可写 form 时，
form 自己持有 draft state；提交走 repository，然后用 committed snapshot 刷新 projection。

## Optional Delta

Delta 是高级能力，不是默认模型。

使用场景：

- state 很大，clone + compare 太贵。
- 持久化需要知道具体哪个字段变化。
- 需要审计日志或精确 effect routing。

`StoreDelta` 当前只是 marker trait，尚未接入 store-level delta write API。后续
API 可能是：

```rust
// Possible future shape, not implemented today:
store.update_delta(cx, |state| {
    state.model = next_model;
    PrefsDelta { model_changed: true }
});

backend.write_delta(|state, delta| {
    if delta.model_changed {
        save_model(&state.model)?;
    }
    Ok(())
});
```

规则：

- 没有 delta 的用户不需要知道这个 API。
- delta 不参与 selector equality；selector 仍然比较 output。
- delta 只用于 persistence/effect routing。

## 数据流

### LocalStore Memory Update

```text
component event
  -> LocalStore::set/update
  -> compare old/new
  -> unchanged: stop
  -> changed:
       revision += 1
       owner cx.notify()
       no backend I/O
```

### LocalStore Committed Update

```text
component event
  -> LocalStore::try_set/try_update
  -> clone current state into draft
  -> unchanged: stop
  -> backend.commit_snapshot(draft)
  -> error: state unchanged, no notify
  -> success: install committed draft or reconcile ack snapshot
  -> changed: revision += 1, owner cx.notify()
```

### LocalStore Backend Event

```text
backend event owned by component
  -> backend load_after_event/load
  -> reconcile snapshot into local state
  -> unchanged: stop
  -> changed: revision += 1, owner cx.notify()
```

### SharedStore Memory Update

```text
component event
  -> SharedStore::set/update/binding.set
  -> underlying StoreCore compares old/new
  -> unchanged: stop
  -> changed:
       store revision += 1
       store entity cx.notify()
       selections/bindings refresh
       only changed selections/bindings notify their owners
       no backend I/O
```

### SharedStore Committed Update

```text
component event
  -> SharedStore::try_set/try_update/binding.try_set
  -> clone current state into draft
  -> unchanged: stop
  -> backend.commit_snapshot(draft)
  -> error: state unchanged, no notify
  -> success:
       install committed draft or reconcile ack snapshot
       store revision += 1
       store entity cx.notify()
       selections/bindings refresh
       only changed selections/bindings notify their owners
```

### SharedStore Selected Observer

```text
store entity notifies
  -> observe_select recomputes selected value
  -> selected value unchanged: stop
  -> selected value changed:
       update cached selected value
       call observer callback
       callback decides whether to call cx.notify()
```

### External Event

```text
external event owned by SharedStore
  -> backend load/get_snapshot
  -> snapshot equals last external snapshot: stop
  -> reconcile snapshot into store state
  -> backend reconcile returns changed bool
  -> changed: revision += 1, notify store
  -> selections/bindings refresh
```

## Ownership / Backend 分层规则

保留 GPUI 的显式 entity 语义，不做 render-time magic。

两个维度必须分开判断：

| 维度 | 问题 | 选项 |
| --- | --- | --- |
| Ownership | 谁拥有 state、谁被 notify、生命周期在哪里？ | `LocalStore` / `SharedStore` |
| Backend | state 如何和外部世界同步？ | `MemoryBackend` / `StoreBackend<S>` / `StoreCommitBackend<S>` |

适合 `SharedStore`：

- 多个组件读写。
- 需要 global lookup。
- 需要独立 subscription / watcher / background task 生命周期。
- 需要被其他 entity observe。

适合 `LocalStore`：

- 只有当前组件使用。
- 不需要被其他 entity observe。
- 不需要 global。
- backend task 生命周期可以跟随当前组件。

适合 `MemoryBackend`：

- 纯内存 UI state。
- runtime-only cache。
- source of truth 已经由调用方手动同步，不需要库管理。

适合自定义 `StoreBackend<S>`：

- 文件、S3、HTTP、Keychain、系统设置等外部数据。
- 数据库 committed projection。
- 需要 initial load 或 live reload。
- 用户需要自定义一致性、冲突处理、权限、认证或重试策略。

适合自定义 `StoreCommitBackend<S>`：

- 文件、S3、HTTP、Keychain 等可以接受 whole-state draft 的 backend。
- 本地 UI 状态必须在 backend commit 成功后才更新。
- 写失败需要同步返回给 UI，而不是存进 `last_error` 后继续显示未提交状态。

适合 `StoreSelection`：

- 派生值、过滤列表、format label、boolean predicate。
- 没有通用反向写入路径。
- render 中需要稳定读取 selected snapshot。

适合 `observe_select` / `observe_select_in`：

- selected value 变化时执行副作用。
- 不需要在 render 中持有 snapshot。
- 不希望因同一个 store 的无关字段变化触发副作用。
- `observe_select_in` 用于副作用需要 `Window` 的场景。

适合 `StoreBinding`：

- memory 字段级读写，或 committed backend 上的 fallible 字段级读写。
- 可以明确写回 store 的 lens。
- 用户希望通过 snapshot object 自己更新，不想拿 `SharedStore`。

## 第三方库参考

调研时间：2026-06-18。

| Library | 提供能力 | 是否直接使用 | 判断 |
| --- | --- | --- | --- |
| Zustand | 直接 set/update、selector subscription、少模板。 | 只参考 API 方向。 | 核心启发是放弃 action/change 强约束，状态更新保持普通函数。 |
| React `useSyncExternalStore` | 外部 store snapshot + subscribe 协议。 | 只参考同步边界。 | 启发 `StoreBackend<S>` 的 load/subscribe/reconcile 分层，但不能照搬 React render-time model。 |
| `reactive_graph` | fine-grained signals、computations、effects。 | 暂不直接使用。 | runtime graph 会和 GPUI entity ownership 重叠。 |
| `leptos_reactive` | Leptos signals、memos、resources、effects。 | 不直接使用。 | 绑定 Leptos runtime，不适合作为 GPUI-native store 基础。 |
| `dioxus-signals` | signals、local subscriptions、computed data。 | 不直接使用。 | 可参考 ergonomics，但绑定 Dioxus 假设。 |
| `tokio::sync::watch` | latest-value channel，每个 receiver 跟踪版本。 | 可作为 external source 内部工具。 | 适合 background bridge，不替代 GPUI selector snapshot。 |
| `notify` | 跨平台 filesystem watcher。 | 可作为 file source 内部工具。 | 适合后续 live reload，不是 state store。 |
| `salsa` | incremental computation。 | 不直接使用。 | 适合 compiler-like query system，对 UI persistence sync 过重。 |

## 实现阶段

### Phase 0: 当前骨架

状态：已完成。

- 早期 `Store` / `Selected` / `ExternalStoreBinding` 已移除。
- mandatory `StoreChange` 模型已移除。
- crate 入口已切换到 `SharedStore` / `LocalStore` / `StoreSelection` / `StoreBinding`。
- crate backend 入口已切换到 `StoreBackend` / `StoreCommitBackend` /
  `MemoryBackend`。

### Phase 1: Core Without Mandatory Change

状态：已完成。

- 改 `StoreState` 为 marker trait。
- 新增 `StoreCore<S>`。
- 新增 `StoreUpdate { changed, revision, origin }`。
- 实现 `set`、`update`、`update_if`。
- 旧 change API 未保留；delta 作为独立可选 trait。

### Phase 2: SharedStore and LocalStore

状态：已完成。

- 新增 `SharedStore<S, Backend = MemoryBackend>` wrapper，隐藏
  `Entity<StoreRuntime<S, Backend>>`。
- 新增 global install/lookup helper。
- 新增 component-owned `LocalStore<S, Backend = MemoryBackend>`。
- 测试 no-op 不 notify、真实变化 notify、field set 只比较字段。

### Phase 3: StoreSelection and StoreBinding

状态：已完成。

- 用 `StoreSelection<T>` 替代裸 `Selected<S, Sel>` 使用方式。
- `SharedStore::select(cx, selector)` 自动绑定 owner notify。
- 新增 `SharedStore::observe_select(cx, selector, callback)`，只在 selected value
  变化时执行 callback。
- 新增 `SharedStore::observe_select_in(cx, window, selector, callback)`，用于需要
  `Window` 的 selected side effect。
- 新增 `StoreBinding<T, Error = Infallible>`，memory binding 支持
  `snapshot/read/cloned/set/update`。
- committed binding 目标支持 `try_set/try_update`，并通过类型参数携带 backend error。
- 确保 `StoreBinding::set` 写回 store，而不是只改自身 snapshot。
- `StoreSelection<T>` / `StoreBinding<T, E>` 通过 `snapshot() -> Rc<T>`、`read(...)`
  和 `cloned()` 读取 snapshot。
- `StoreSelection<T>` / `StoreBinding<T, E>` 实现 `Debug`、`Display`、`PartialEq`、`Eq`
  这类不暴露长期引用的 trait。
- 明确不实现 `Deref`、`AsRef`、`Borrow`、`DerefMut`、`BorrowMut`、`Copy` 和默认 `Clone`。
- 测试 unrelated store update 不 notify selection owner。
- 测试旧的 owned snapshot 在后续 store 更新后仍然可读，但写入只能通过 `set/update`
  或 `try_set/try_update`。

### Phase 4: StoreSource Trait and StoreSourceBuilder Adapter

状态：历史阶段，已被 Phase 4B 取代。

- 新增 `StoreSource<S>` trait、`MemorySource`、`StoreSourceId`。
- 新增 `StoreSourceBuilder` closure adapter，作为常见场景便捷写法。
- `LocalStore<S, Source>` 和 `SharedStore<S, Source>` 共享同一套 source sync。
- 支持 `load`、`load_after_event`、`reconcile`、`write_snapshot`、`subscribe`。
- 支持 no-op subscription / no-op write 默认。
- 支持 write ack / hash / source revision 的 self-write event 过滤策略。
- 测试 `MemorySource`、file-like source、fake remote source、database-like
  committed snapshot source。

当前限制：

- `StoreSourceFuture` 仍是同步 `Result` alias。
- `LocalStore` live source event 需要显式调用 `subscribe(cx, |owner| &mut owner.store)`。
- 第一版没有实现真正 background task / async cancellation。

第一版的设计缺口：

- `set/update/update_if/bind` 暴露在所有 `Source: StoreSource<S>` 的 store 上。
- `write_snapshot` 是 `StoreSource<S>` 的默认 no-op 方法，projection source 也看起来像
  可写 source。
- 持久化失败只进入 `last_error`，store state 已经先更新并 notify，无法表达“commit
  失败则 UI state 不变”。

### Phase 4B: Capability-Safe Backend API

状态：已完成。

- `StoreSource<S>` -> `StoreBackend<S>`。
- `MemorySource` -> `MemoryBackend`。
- `StoreSourceBuilder` -> `StoreBackendBuilder`。
- `StoreSourceId` -> `StoreBackendId`。
- `StoreSourceWriteAck` -> `StoreCommitAck`。
- 新增 `StoreCommitBackend<S>`，只有实现该 trait 的 backend 才能使用
  `try_set/try_update/try_update_if/bind_committed`。
- 从基础 backend trait 移除 `write_snapshot` 默认 no-op；commit 只属于
  `StoreCommitBackend<S>`。
- 把 `set/update/update_if/bind` 从泛型 backend impl 移到
  `SharedStore<S, MemoryBackend>` / `LocalStore<S, MemoryBackend>` impl。
- 新增 committed update runtime：先 clone draft、提交 backend，成功后再更新 store 和
  notify。
- projection backend 只保留 `read/select/observe_select/sync_snapshot`；数据库写入走 repository
  command 后同步 committed snapshot。
- 新增 `read_cloned` / `select_cloned` / `refresh_from_backend` /
  `try_update_field` / `bind_committed_field` / `install_global_with_default`。
- `StoreBackendBuilder` 新增 `reconcile_replace` / `reconcile_field`，覆盖常见
  snapshot 替换和字段 projection 模板。

### Phase 5: Optional Delta

状态：仅保留 `StoreDelta` marker，尚未接入 write delta。

- 明确 delta API 是否需要。
- 如果需要，delta 只用于 persistence/effect routing。
- README 中保持 delta 为高级例子，不进入默认 quick start。

### Phase 6: Docs and Migration

目标：

- README 只展示目标高层 API。
- development plan 保持内部设计细节。
- 如果保留底层 primitive，要明确它们是 escape hatch。
- 应用接入必须另写应用专项文档，不写进本 crate 计划。

## 验证计划

实现阶段需要：

- `cargo fmt`
- `cargo test -p gpui-store`
- `cargo check -p gpui-store`
- `git diff --check`

重点测试场景：

- `LocalStore::set` no-op 不 notify owner。
- `LocalStore::update` 真实变化 notify owner。
- `LocalStore<S, MemoryBackend>` 不创建 backend task。
- `LocalStore<S, FakeBackend>` initial load/reconcile 后只 notify owner。
- `LocalStore<S, FakeBackend>` backend event 通过 owner accessor 更新本地 store。
- `LocalStore<S, CommitBackend>::try_update` commit 成功后才更新 state。
- `LocalStore<S, CommitBackend>::try_update` commit 失败时不更新 state、不 notify owner。
- `SharedStore::set` no-op 不 bump revision。
- `SharedStore<S, FakeBackend>` backend event 刷新 selections/bindings。
- `SharedStore<S, CommitBackend>::try_set` commit 成功后才 bump revision。
- `SharedStore<S, CommitBackend>::try_set` commit 失败时不 bump revision、不 notify store。
- `SharedStore<S, ProjectionBackend>` 不提供 `set/update/bind/try_set/try_update/bind_committed`。
- `StoreSelection<T>` 只在 selected output 变化时 notify owner。
- `observe_select` 只在 selected output 变化时调用 callback。
- `observe_select` 不在创建时调用 callback。
- `observe_select` 不自动 notify owner；callback 自己决定是否 notify。
- `observe_select_in` 在 selected output 变化时能拿到 `Window` 并调用 callback。
- `StoreBinding<T>::set` 更新 memory store，并通过订阅刷新自身 snapshot。
- `StoreBinding<T, E>::try_set` 通过 committed runtime 写入并返回 backend error。
- `StoreSelection<T>` / `StoreBinding<T, E>` 的 `snapshot()` 返回 owned `Rc<T>`，旧 snapshot
  在后续 store 更新后仍然安全可读。
- `StoreSelection<T>` / `StoreBinding<T, E>` 的 `read`、`cloned` 和 comparison trait
  转发到当前 snapshot。
- 不能通过 `StoreBinding<T, E>` 取得 mutable reference 绕过 store。
- external backend initial load 相同 state 不 notify。
- external backend event 相同 snapshot 不 reconcile。
- database committed snapshot reconcile 后 selection 更新。

## 待定决策

- `SharedStore::try_update` 是否要求 `S: Clone + PartialEq`，还是通过 trait 单独表达 equality。
- `StoreBinding::set` / `try_set` 是否只支持 `T: Clone + PartialEq`，还是允许用户传 custom equality。
- `StoreSelection<T>` 内部是否继续用 entity，还是用 store subscription + owner subscription handle。
- 是否需要 cloneable `StoreSelectionHandle<T>` / `StoreBindingHandle<T, E>`，以及它们和 owner-bound `StoreSelection<T>` / `StoreBinding<T, E>` 的生命周期边界。
- `StoreBackend<S>::Error` 是否保持 associated type，还是统一 boxed error。
- `StoreBackendFuture` 后续是否从同步 `Result` alias 升级成 boxed future、GPUI task wrapper，还是同步/异步双 API。
- `StoreBackendBuilder` closure adapter 是否需要覆盖所有 trait method，还是只覆盖常用组合。
- `StoreCommitAck` 是否只携带 snapshot，还是也携带 backend revision/hash，用于过滤 self-generated events。
- delta API 是否需要第一版实现，还是等真实性能/持久化需求出现。
