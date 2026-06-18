# gpui-store 开发计划

状态：初版代码已按本文档重构完成。当前实现已经移除早期实验 API，默认不再要求用户定义 `Change` / action / reducer；核心模型是 Zustand-like 的直接 `set/update`，由库负责 revision、equality、selector subscription 和外部同步。

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
- external source 默认写 whole snapshot；需要局部写回时再启用可选 delta。

`Change` / `Delta` 从核心必需品降级为高级可选能力。

## 设计目标

- 符合 GPUI ownership / `Entity` / `cx.notify()` 语义。
- 默认 API 像 Zustand：直接 `set` / `update`，不写 action/reducer/change。
- 保留 Rust 类型不变量，domain state 仍然是普通 Rust struct。
- 支持一个 store 多个 `StoreSelection<T>` / `StoreBinding<T>`。
- `StoreSelection<T>` 和 `StoreBinding<T>` 自动持有订阅，并通知创建它们的 owner component。
- 支持组件私有 `LocalStore<S>`，避免所有状态都包 entity。
- source 是和 ownership 正交的维度：`LocalStore` / `SharedStore` 都能接
  `MemorySource`、文件、S3、HTTP、数据库 projection 或用户自定义 source。
- 外部同步由用户实现 `StoreSource<S>` trait；load/write/subscribe/reconcile
  的 GPUI glue 收进库里。
- 复杂场景保留底层 primitive 和 optional delta escape hatch。

## 非目标

- 不做 Redux action/reducer/middleware registry。
- 不做 render-time 隐式依赖收集。
- 不把整个数据库镜像进一个全局 store。
- 不内置 TOML、SQLite、S3、keychain、repository 或具体应用 schema。
- 不把 file/database 写成特殊 store 类型；它们只能是 `StoreSource<S>` 的
  便捷 adapter 或用户实现。
- 不在 store crate 内保存任何应用接入计划。

## 命名规则

公开类型按职责分组命名，不暴露早期实验 API 名字。

| 分组 | 目标命名 | 说明 |
| --- | --- | --- |
| Ownership wrapper | `LocalStore<S, Source = MemorySource>` | 当前组件持有，直接 notify owner。 |
| Ownership wrapper | `SharedStore<S, Source = MemorySource>` | GPUI entity-backed，可跨组件共享。 |
| Smart snapshot | `StoreSelection<T>` | 只读派生 snapshot。 |
| Smart snapshot | `StoreBinding<T>` | 可写 lens，写回 store。 |
| Source trait | `StoreSource<S>` | 用户实现的外部同步扩展点。 |
| Source default | `MemorySource` | 默认 memory-only/no-op source。 |
| Source builder | `StoreSourceBuilder` | closure adapter，不是核心抽象。 |
| Source helpers | `StoreSourceId` / `StoreSourcePolicy` / `StoreSourceFuture` / `StoreSourceCallback` / `StoreSourceWriteAck` | source 子系统辅助类型统一加 `StoreSource` 前缀。 |
| Update metadata | `StoreRevision` / `StoreUpdate` / `StoreUpdateOrigin` | mutation 结果和版本信息，不再使用 change/action 命名。 |

旧名映射：

| 旧名 | 新名 | 原因 |
| --- | --- | --- |
| `StoreEntity` | `SharedStore` | 公开 API 表达共享语义，entity 是实现细节。 |
| `Selection` | `StoreSelection` | 避免和普通 UI selection 概念混淆。 |
| `Binding` | `StoreBinding` | 避免和组件库/输入绑定概念混淆。 |
| `NoSource` | `MemorySource` | 默认 source 仍属于 source 维度，只是 no-op。 |
| `ExternalSource` | `StoreSourceBuilder` | builder 不等于 source trait，也不只代表 external backend。 |
| `ChangeOrigin` | `StoreUpdateOrigin` | 新模型不是 change/action 驱动。 |

## 目标模块结构

```text
crates/gpui-store/src/lib.rs
crates/gpui-store/src/store.rs
crates/gpui-store/src/shared.rs
crates/gpui-store/src/local.rs
crates/gpui-store/src/selection.rs
crates/gpui-store/src/binding.rs
crates/gpui-store/src/source.rs
crates/gpui-store/src/sync.rs
crates/gpui-store/src/delta.rs
crates/gpui-store/src/error.rs
crates/gpui-store/src/test_support.rs
```

模块职责：

- `store.rs`：纯 store core、revision、origin、update result、changed detection。
- `shared.rs`：`SharedStore<S, Source = MemorySource>`，shared observable GPUI entity
  wrapper、global helper 和 entity 级 source lifecycle。
- `local.rs`：`LocalStore<S, Source = MemorySource>`，组件私有 store，不创建额外
  entity，source lifecycle 绑定 owner component。
- `selection.rs`：`StoreSelection<T>`，只读 subscribed snapshot。
- `binding.rs`：`StoreBinding<T>`，可写 subscribed lens。
- `source.rs`：`StoreSource<S>` trait、`MemorySource`、`StoreSourcePolicy`、`StoreSourceId`、
  `StoreSourceBuilder` closure adapter。
- `sync.rs`：initial sync、external event、snapshot reconcile、write-back coordination。
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
- `update_delta` 返回可选 delta。

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

shared observable store。内部持有 `Entity<StoreRuntime<S>>`，对外隐藏底层 entity 更新模板。

目标 API：

```rust
impl<S: StoreState> SharedStore<S, MemorySource> {
    pub fn new(cx: &mut impl AppContext, initial: S) -> Self;
    pub fn install_global(cx: &mut App, initial: S) -> Self;
    pub fn global(cx: &impl AppContext) -> Self;
}

impl<S, Source> SharedStore<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    pub fn new_with_source(cx: &mut impl AppContext, initial: S, source: Source) -> Result<Self>;
    pub fn install_global_from_source(cx: &mut App, initial: S, source: Source) -> Result<Self>;

    pub fn read<R>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> R) -> R;
    pub fn set<T: PartialEq>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, value: T) -> StoreUpdate;
    pub fn update(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq;
    pub fn update_if(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate;

    pub fn select<Owner, T>(&self, cx: &mut Context<Owner>, select: impl Fn(&S) -> T + 'static) -> StoreSelection<T>;
    pub fn bind<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T>;
}
```

`SharedStore` 只有在 store state 真实变化时才对自身 entity `cx.notify()`。
如果带 source，initial load、subscribe handle、write-back task 都由
`StoreRuntime<S, Source>` 持有，生命周期跟随 store entity/global，而不是任意一个
consumer component。

### LocalStore

组件私有 store，不创建额外 entity，不支持其他组件订阅。默认是 `MemorySource`；
带 source 时，source 的订阅和 task 生命周期绑定创建它的 owner component。

目标 API：

```rust
impl<S: StoreState> LocalStore<S, MemorySource> {
    pub fn new(initial: S) -> Self;
}

impl<S, Source> LocalStore<S, Source>
where
    S: StoreState,
    Source: StoreSource<S>,
{
    pub fn with_source<Owner>(cx: &mut Context<Owner>, initial: S, source: Source) -> Result<Self>;

    pub fn read(&self) -> &S;
    pub fn set<Owner, T: PartialEq>(&mut self, cx: &mut Context<Owner>, field: impl FnOnce(&mut S) -> &mut T, value: T) -> StoreUpdate;
    pub fn update<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq;
    pub fn update_if<Owner>(&mut self, cx: &mut Context<Owner>, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate;
}
```

`LocalStore` 直接通知 owner component。它不解决跨组件订阅问题，但可以通过
`StoreSource<S>` 和外部系统同步。也就是说：

```text
LocalStore<S, MemorySource>      -> component-private memory state
LocalStore<S, FileSource>    -> component owns file-backed state
LocalStore<S, S3Source>      -> component owns S3-backed state
SharedStore<S, MemorySource>     -> shared in-memory state
SharedStore<S, S3Source>     -> shared S3-backed state
SharedStore<S, DbProjection> -> shared committed database projection
```

### StoreSelection

只读 subscribed snapshot。

```rust
pub struct StoreSelection<T> {
    snapshot: T,
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

`StoreSelection<T>` 应当表现为只读智能指针：

```rust
impl<T> Deref for StoreSelection<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}
```

允许通过 method-call deref 直接使用 snapshot，例如 `self.rows.iter()`、
`self.query.is_empty()`、`self.model.as_ref()`。bool snapshot 需要写
`*self.can_submit`，不能依赖 `if self.can_submit` 自动解引用。

### StoreBinding

可写 subscribed lens。它是带 setter 的 selection。

```rust
pub struct StoreBinding<T> {
    snapshot: T,
    // hidden store handle, getter, setter, subscription handles
}
```

目标 API：

```rust
impl<T> StoreBinding<T> {
    pub fn get(&self) -> &T;
    pub fn set<Owner>(&self, cx: &mut Context<Owner>, value: T) -> StoreUpdate
    where
        T: Clone + PartialEq;
    pub fn update<Owner>(&self, cx: &mut Context<Owner>, f: impl FnOnce(&mut T)) -> StoreUpdate
    where
        T: Clone + PartialEq;
}
```

`StoreBinding` 的更新流程：

```text
binding.set(value)
  -> store.update_if(...)
  -> setter writes value into store state
  -> store compares old binding snapshot or old whole state
  -> changed: store revision += 1 and notify store observers
  -> binding refreshes from store
  -> owner component notify only if binding snapshot changed
```

`StoreBinding` 不拥有第二份 source of truth。它的 snapshot 是 cache，写入永远回到 store。

`StoreBinding<T>` 也应当表现为只读智能指针：

```rust
impl<T> Deref for StoreBinding<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}
```

但 `StoreBinding<T>` 不能实现 `DerefMut`。可写能力必须经过 `set` / `update`
回到 store，否则会绕过 changed detection、revision、owner notify 和 external write-back。

### StoreSelection / StoreBinding Trait 策略

推荐实现：

| Trait | `StoreSelection<T>` | `StoreBinding<T>` | 说明 |
| --- | --- | --- | --- |
| `Deref<Target = T>` | yes | yes | 读体验像智能指针，支持 method-call deref。 |
| `AsRef<T>` | yes | yes | 方便传给接收引用的 API。 |
| `Borrow<T>` | yes | yes | 方便 map/set 等标准库 API。 |
| `Debug` | `T: Debug` | `T: Debug` | 转发给 snapshot。 |
| `Display` | `T: Display` | `T: Display` | 转发给 snapshot。 |
| `PartialEq` | `T: PartialEq` | `T: PartialEq` | 按 snapshot 比较。 |
| `Eq` | `T: Eq` | `T: Eq` | 按 snapshot 语义。 |

明确不实现：

| Trait | 原因 |
| --- | --- |
| `DerefMut` / `BorrowMut` | 会允许绕过 store 直接改 snapshot。 |
| `Copy` | handle 持有订阅和 cached snapshot，不是 plain value。 |
| 默认 `Clone` | owner-bound 订阅被 clone 后语义不清楚。 |
| `Hash` | snapshot 会随订阅刷新，作为 hash key 容易违反稳定性预期。 |

如果后续确实需要 cloneable handle，应单独设计：

```rust
StoreSelectionHandle<T>
StoreBindingHandle<T>
```

组件字段中的 `StoreSelection<T>` / `StoreBinding<T>` 默认不 clone。需要值副本时显式 clone snapshot：

```rust
let value = self.binding.get().clone();
```

## Store Source

目标：用户实现“怎么读、怎么写、怎么订阅、怎么把 snapshot merge 回 state”，库实现
GPUI glue、task lifecycle、changed detection、revision、selection refresh 和 owner notify。

### Trait API

`StoreSource<S>` 是扩展点。文件、S3、HTTP、Keychain、数据库 projection 都应该通过
这个 trait 接入，而不是在库里写死 backend 分支。

```rust
pub trait StoreSource<S: StoreState>: 'static {
    type Snapshot: Clone + PartialEq + 'static;
    type Event: 'static;
    type Subscription: 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    fn source_id(&self) -> StoreSourceId;
    fn policy(&self) -> StoreSourcePolicy;

    fn load(&self) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error>;

    fn subscribe(
        &self,
        on_change: StoreSourceCallback<Self::Event>,
    ) -> StoreSourceFuture<Option<Self::Subscription>, Self::Error>;

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error>;

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool;

    fn write_snapshot(
        &self,
        state: &S,
    ) -> StoreSourceFuture<Option<StoreSourceWriteAck<Self::Snapshot>>, Self::Error>;
}
```

默认实现策略：

- `MemorySource` 实现 `StoreSource<S>`，所有方法都是 no-op。
- `subscribe` 默认 no-op；没有 live watch 时不要求用户写空实现。
- `load_after_event` 默认调用 `load`，有增量事件能力的 source 可以覆盖。
- `write_snapshot` 默认 no-op；read-only projection 不需要实现写回。
- `reconcile` 必须返回 `bool`，表示 state 是否真实变化。
- 当前 `StoreSourceFuture<T, E>` 是 `Result<T, E>` alias；第一版先实现同步 source glue。
  后续如果接 S3/HTTP 这类真实异步 backend，再把 alias 升级成 GPUI task/future 形态。

### Closure Adapter

`StoreSourceBuilder` 是便捷 adapter，不是核心抽象。它让常见文件/配置场景不用手写一个
struct + trait impl。

```rust
let source = StoreSourceBuilder::store_backed("file:prefs")
    .load(load_snapshot)
    .reconcile(|state, snapshot| {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    })
    .write_snapshot(save_state);

let store = SharedStore::install_global_from_source(cx, State::default(), source)?;
```

自定义 backend 直接实现 `StoreSource<S>`：

```rust
struct S3PrefsSource {
    bucket: String,
    key: String,
    client: S3Client,
}

impl StoreSource<Prefs> for S3PrefsSource {
    type Snapshot = Prefs;
    type Event = S3ObjectVersion;
    type Subscription = S3Watcher;
    type Error = anyhow::Error;

    fn source_id(&self) -> StoreSourceId {
        StoreSourceId::new(format!("s3:{}/{}", self.bucket, self.key))
    }

    fn policy(&self) -> StoreSourcePolicy {
        StoreSourcePolicy::ExternalBacked
    }

    fn reconcile(&self, state: &mut Prefs, snapshot: Prefs) -> bool {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    }

    // load / subscribe / write_snapshot 由应用按自己的 S3 client 实现。
}
```

同一个 source 能接到任意 ownership wrapper：

```rust
let local = LocalStore::with_source(cx, Prefs::default(), s3_source)?;
let global = SharedStore::install_global_from_source(cx, Prefs::default(), s3_source)?;
```

`LocalStore` 的 live source event 需要用户提供 owner accessor，因为库无法凭泛型知道
store 字段位于组件的哪个位置：

```rust
store.subscribe(cx, |owner: &mut PrefsPane| &mut owner.prefs)?;
```

`SharedStore` 不需要 accessor；source subscription 生命周期跟随内部 entity。

### Source Policy

`StoreSourcePolicy` 描述同步语义，不描述 backend 类型。

| Policy | Source of truth | Store role |
| --- | --- | --- |
| `MemoryOnly` | Store itself | transient in-memory state |
| `StoreBacked` | Store after initial load | durable persistence target |
| `ExternalBacked` | External source can push changes | live projection with local writes |
| `Projection` | External committed result | UI snapshot over committed state |

`StoreSourceId` 描述实际 backend identity，例如：

- `memory:runtime`
- `file:prefs`
- `s3:prefs`
- `http:settings`
- `database:prompts`

### Database Flow

数据库写入不应该默认走 `write_snapshot`。

推荐 flow：

```text
UI command
  -> repository transaction
  -> committed snapshot
  -> store.sync_snapshot(committed snapshot)
  -> selections/bindings update from committed data
```

这样数据库仍是 durable source of truth，store 只是 UI snapshot。

## Optional Delta

Delta 是高级能力，不是默认模型。

使用场景：

- state 很大，clone + compare 太贵。
- 持久化需要知道具体哪个字段变化。
- 需要审计日志或精确 effect routing。

目标 API 可以是：

```rust
store.update_delta(cx, |state| {
    state.model = next_model;
    PrefsDelta { model_changed: true }
});

source.write_delta(|state, delta| {
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

### LocalStore Update

```text
component event
  -> LocalStore::set/update
  -> compare old/new
  -> unchanged: stop
  -> changed:
       revision += 1
       owner cx.notify()
       optional source write-back runs
```

### LocalStore Source Event

```text
source event owned by component
  -> source load_after_event/load
  -> reconcile snapshot into local state
  -> unchanged: stop
  -> changed: revision += 1, owner cx.notify()
```

### SharedStore Update

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
       optional external write-back runs
```

### External Event

```text
external event owned by SharedStore
  -> source load/get_snapshot
  -> snapshot equals last external snapshot: stop
  -> reconcile snapshot into store state
  -> source reconcile returns changed bool
  -> changed: revision += 1, notify store
  -> selections/bindings refresh
```

## Ownership / Source 分层规则

保留 GPUI 的显式 entity 语义，不做 render-time magic。

两个维度必须分开判断：

| 维度 | 问题 | 选项 |
| --- | --- | --- |
| Ownership | 谁拥有 state、谁被 notify、生命周期在哪里？ | `LocalStore` / `SharedStore` |
| Source | state 如何和外部世界同步？ | `MemorySource` / `StoreSource<S>` |

适合 `SharedStore`：

- 多个组件读写。
- 需要 global lookup。
- 需要独立 subscription / watcher / background task 生命周期。
- 需要被其他 entity observe。

适合 `LocalStore`：

- 只有当前组件使用。
- 不需要被其他 entity observe。
- 不需要 global。
- source task 生命周期可以跟随当前组件。

适合 `MemorySource`：

- 纯内存 UI state。
- runtime-only cache。
- source of truth 已经由调用方手动同步，不需要库管理。

适合自定义 `StoreSource<S>`：

- 文件、S3、HTTP、Keychain、系统设置等外部数据。
- 数据库 committed projection。
- 需要 initial load、live reload 或 write-back。
- 用户需要自定义一致性、冲突处理、权限、认证或重试策略。

适合 `StoreSelection`：

- 派生值、过滤列表、format label、boolean predicate。
- 没有通用反向写入路径。

适合 `StoreBinding`：

- 字段级读写。
- 可以明确写回 store 的 lens。
- 用户希望通过 snapshot object 自己更新，不想拿 `SharedStore`。

## 第三方库参考

调研时间：2026-06-18。

| Library | 提供能力 | 是否直接使用 | 判断 |
| --- | --- | --- | --- |
| Zustand | 直接 set/update、selector subscription、少模板。 | 只参考 API 方向。 | 核心启发是放弃 action/change 强约束，状态更新保持普通函数。 |
| React `useSyncExternalStore` | 外部 store snapshot + subscribe 协议。 | 只参考同步边界。 | 启发 `StoreSource<S>` 的 load/subscribe/reconcile 分层，但不能照搬 React render-time model。 |
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
- crate 入口已切换到 `SharedStore` / `LocalStore` / `StoreSelection` / `StoreBinding` / `StoreSource`。

### Phase 1: Core Without Mandatory Change

状态：已完成。

- 改 `StoreState` 为 marker trait。
- 新增 `StoreCore<S>`。
- 新增 `StoreUpdate { changed, revision, origin }`。
- 实现 `set`、`update`、`update_if`。
- 旧 change API 未保留；delta 作为独立可选 trait。

### Phase 2: SharedStore and LocalStore

状态：已完成。

- 新增 `SharedStore<S, Source = MemorySource>` wrapper，隐藏
  `Entity<StoreRuntime<S, Source>>`。
- 新增 global install/lookup helper。
- 新增 component-owned `LocalStore<S, Source = MemorySource>`。
- 测试 no-op 不 notify、真实变化 notify、field set 只比较字段。

### Phase 3: StoreSelection and StoreBinding

状态：已完成。

- 用 `StoreSelection<T>` 替代裸 `Selected<S, Sel>` 使用方式。
- `SharedStore::select(cx, selector)` 自动绑定 owner notify。
- 新增 `StoreBinding<T>`，支持 `get/set/update`。
- 确保 `StoreBinding::set` 写回 store，而不是只改自身 snapshot。
- `StoreSelection<T>` / `StoreBinding<T>` 实现只读智能指针 trait：`Deref`、`AsRef`、`Borrow`、`Debug`、`Display`、`PartialEq`、`Eq`。
- 明确不实现 `DerefMut`、`BorrowMut`、`Copy` 和默认 `Clone`。
- 测试 unrelated store update 不 notify selection owner。
- 测试 `StoreBinding<T>` method-call deref 不需要 `.get()`，但写入只能通过 `set/update`。

### Phase 4: StoreSource Trait and StoreSourceBuilder Adapter

状态：第一版已完成。

- 新增 `StoreSource<S>` trait、`MemorySource`、`StoreSourcePolicy`、`StoreSourceId`。
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
- `LocalStore<S, MemorySource>` 不创建 source task。
- `LocalStore<S, FakeSource>` initial load/reconcile 后只 notify owner。
- `LocalStore<S, FakeSource>` source event 通过 owner accessor 更新本地 store。
- `SharedStore::set` no-op 不 bump revision。
- `SharedStore<S, FakeSource>` source event 刷新 selections/bindings。
- `StoreSelection<T>` 只在 selected output 变化时 notify owner。
- `StoreBinding<T>::set` 更新 store，并通过订阅刷新自身 snapshot。
- `StoreSelection<T>` / `StoreBinding<T>` 的 `Deref`、`AsRef`、`Borrow` 和 comparison trait 转发到 snapshot。
- 不能通过 `StoreBinding<T>` 取得 mutable reference 绕过 store。
- external source initial load 相同 state 不 notify。
- external source event 相同 snapshot 不 reconcile。
- local changed update 触发 write snapshot。
- database committed snapshot reconcile 后 selection 更新。

## 待定决策

- `SharedStore::update` 是否要求 `S: Clone + PartialEq`，还是通过 trait 单独表达 equality。
- `StoreBinding::set` 是否只支持 `T: Clone + PartialEq`，还是允许用户传 custom equality。
- `StoreSelection<T>` 内部是否继续用 entity，还是用 store subscription + owner subscription handle。
- 是否需要 cloneable `StoreSelectionHandle<T>` / `StoreBindingHandle<T>`，以及它们和 owner-bound `StoreSelection<T>` / `StoreBinding<T>` 的生命周期边界。
- `StoreSource<S>::Error` 是否保持 associated type，还是统一 boxed error。
- `StoreSourceFuture` 后续是否从同步 `Result` alias 升级成 boxed future、GPUI task wrapper，还是同步/异步双 API。
- `StoreSourceBuilder` closure adapter 是否需要覆盖所有 trait method，还是只覆盖常用组合。
- delta API 是否需要第一版实现，还是等真实性能/持久化需求出现。
