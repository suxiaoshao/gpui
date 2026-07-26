# gpui-store 类型化纯内存 Store 实施计划

## 1. 状态与范围

- GitHub issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)。
- 实施分支：`codex/177-jaco-catalog-startup-model-selection`。
- 文档位置：`crates/gpui-store/dev/in-memory-store.md`。
- 当前阶段：目标 API 已确定，当前源码仍是
  `SharedStore / LocalStore + Backend + Binding + Revision` 旧实现，尚未迁移。
- 兼容策略：这是 `0.1.0` 阶段的破坏性替换；直接删除旧 API，不提供 deprecated
  alias、兼容 wrapper、backend feature 或迁移期 façade。
- 旧计划策略：`docs/development-plan.md` 和
  `docs/catalog-snapshot-projection-plan.md` 保持删除；新计划只放在内部 `dev/` 目录，
  不重新混入对外 `docs/`。
- 发布关系：本 crate 可完成包级实现与验证，但旧 Jaco 调用方会在 breaking rewrite后
  无法编译。最终 workspace/merge gate必须等待后续 Jaco Store调用面迁移全部完成。

### 1.1 目标

1. 公开唯一的 `Store<S>`，拥有一份权威、类型化、纯内存状态。
2. 通过显式 `read`、`set`、`update`、`update_if` 控制借用、修改与通知。
3. 使用 `StoreChange<R>` 将业务返回值和调用者的通知决定放在同一个原子结果中。
4. 使用可复用 `Select<S>` 和只读 `StoreSelection<T>` 表达 owner-bound派生状态。
5. 提供 whole-store、selected-value 和 Window-aware selected observation。
6. 对观察定义严格的 scheduled initial delivery、取消和 GPUI effect coalescing契约。
7. 支持 typed global Store，同时允许非 global Store由应用显式传递。
8. 不要求 `S: Clone + PartialEq + Default + Send + Sync`；只在具体 API增加必要 bound。
9. 删除 Store中的 I/O、backend、commit、reconcile、binding和 revision职责。
10. 保持英文默认文档并同步中文 README 与完整指南。

### 1.2 非目标

- 不加载配置、文件、数据库、keychain或网络数据。
- 不执行持久化、commit、transaction、rollback、ack或 optimistic reconciliation。
- 不定义 reducer、action、middleware、delta、revision或 mutation origin。
- 不提供 writable selection、`StoreBinding` 或表单自动双向同步。
- 不提供 `LocalStore` / `SharedStore` 两套所有权模型。
- 不迁移 Jaco；本计划只列出精确发布门槛。
- 不暴露内部 Entity、状态 cell、观察 phase或 weak handle。

### 1.3 已冻结的需求

| ID | 需求 |
| --- | --- |
| ST-R1 | 公开类型只有 `Store<S>`、`StoreChange<R>`、`Select<S>`、`StoreSelection<T>` |
| ST-R2 | `Store<S>` clone只复制 handle，不 clone `S` |
| ST-R3 | `set` / `update` 总是发布；`update_if` 只在 `Changed` 时发布 |
| ST-R4 | `StoreChange<R>` 同时返回业务结果和通知决定，不回滚 mutation |
| ST-R5 | selector可由 named type、函数或 `Fn` closure复用 |
| ST-R6 | selection同步得到初值，只在 selected output变化时通知 owner |
| ST-R7 | observation注册过程不重入，随后先投递一次当前值，再投递后续变化 |
| ST-R8 | drop selection/subscription停止派生更新；库内部只弱持有 source，不主动延长 Store生命周期 |
| ST-R9 | typed global中保存 Store handle；Store mutation不伪装成 GPUI Global替换 |
| ST-R10 | State、selection output和观察 payload不引入无关 Clone/Send/Sync bound |
| ST-R11 | 当前 backend、binding、revision及双 Store模型整体删除 |
| ST-R12 | 同一 GPUI effect cycle的多次 notify允许合并，observer读取最终当前值 |

## 2. 证据快照

### 2.1 当前仓库

| 证据 | 当前事实 | 目标动作 |
| --- | --- | --- |
| `src/lib.rs` | 导出 backend、binding、delta、Local/Shared Store、revision等大量概念 | 收敛为四个公开概念 |
| `src/store.rs` | `StoreCore<S>` 保存 revision/origin，并依赖全状态比较或 bool变化结果 | 删除 revision/origin；改为显式通知契约 |
| `src/shared.rs` | `SharedStore<S, Backend>` 同时承担 Entity、backend和刷新 | 删除 backend；单一 `Store<S>` |
| `src/local.rs` | 另一套 local owner模型 | 删除；本地状态使用普通字段或 Entity |
| `src/backend.rs` | load/subscribe/reconcile/commit抽象 | 删除；I/O回到应用 service/repository |
| `src/binding.rs` | writable派生值写回 Store | 删除；表单与 command显式写入 |
| `src/selection.rs` | selection暴露 snapshot、revision和比较/格式转发 | 只保留 `read` / `cloned` |
| `src/tests.rs` | 17个测试主要覆盖旧 backend、binding与 revision | 整体替换为目标 API矩阵 |
| `README*`、`docs/guide*` | 已描述 target纯内存设计，但仍标记未实施 | 实施完成后同步最终签名 |

### 2.2 当前 Jaco 调用面

当前静态扫描确认以下直接迁移面；这是 Store计划的已知 inventory，不冒充后续 Jaco迁移的
穷尽证明：

- `app/jaco/src/components/chat_input.rs`：`StoreBinding`；
- `app/jaco/src/components/run_settings.rs`：catalog `entity()` / `read_cloned()`；
- `app/jaco/src/features/home/new_conversation.rs`：旧 `StoreSelection`；
- `app/jaco/src/features/settings/{projects,prompts,skills}.rs`：旧 selection；
- `app/jaco/src/features/settings/shortcuts.rs`：直接观察旧 Store Entity；
- `app/jaco/src/state/config.rs`：`SharedStore`、backend、commit backend；
- `app/jaco/src/state/config/mcp.rs`：`try_update_field`；
- `app/jaco/src/state/skills.rs`：filesystem backend；
- `app/jaco/src/state/prompts.rs`：database backend；
- `app/jaco/src/state/{providers,projects,shortcuts}.rs`：`SharedStore` / `StoreState`；
- `app/jaco/src/state/workspace.rs`：旧 Store handle与 raw Entity观察。

因此本计划不能把 workspace build声明为 crate完成条件，也不能为了维持中间态编译而保留
旧 wrapper。类型名扫描还会漏掉 method-only调用；只有后续 Jaco迁移后的 workspace编译
才能证明迁移面穷尽。

### 2.3 GPUI 上游证据

workspace 的 `gpui = "=0.2.2"` 来自 Zed git，`Cargo.lock` 锁定 commit
`1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。本计划按该 commit核对：

- `Entity<T>` 的手写 `Clone` 不要求 `T: Clone`；
- `Entity::read_with` / `update` 接受任意 `AppContext`；
- `AsyncApp` 实现 `AppContext`，但 `as_mut` 会 panic，因此 Store不得使用 `as_mut`；
- `Global` 只要求 `'static`；
- `AppContext` 能只读 global，但设置 global需要 `App` / `BorrowAppContext`；
- `Context::observe` / `observe_in` 弱持有 owner，并监听 Entity `notify`；
- observer activation本身通过 `defer` 延迟；
- `App::defer`、`Context::defer_in` 可安排非重入 initial delivery；
- `App::on_window_closed` 可将 Window observation绑定到目标 `WindowId`；
- `Subscription::join` 和 drop语义可组合 source subscription与 pending delivery取消；
- 同一 Entity在一个 effect cycle中的多次 notify会按 EntityId合并。

稳定上游证据：

- `crates/gpui/src/app/entity_map.rs`：Entity clone/read/update/weak。
- `crates/gpui/src/gpui.rs`：`AppContext` 与 `BorrowAppContext`。
- `crates/gpui/src/app/context.rs`：observe、observe_in、defer_in。
- `crates/gpui/src/app.rs`：defer、Global API与 notification coalescing。
- `crates/gpui/src/subscription.rs`：join与 drop cancellation。

### 2.4 依赖决策

| 依赖 | 当前 | 目标 | 原因 |
| --- | --- | --- | --- |
| Rust | Edition 2024；仓库说明 Rust 1.95+ | 不变 | 不引入新语言/库要求 |
| `gpui` normal | workspace git dependency | 保留 | Store基于 Entity、Context、Global和Subscription |
| `gpui` dev | 同源 + `test-support` | 保留 | 测试通知、owner、Window和effect顺序 |
| 其他 crate | 无 | 无 | 不自建 observer registry或持久化层 |
| root Cargo / lockfile | 已包含 crate | 不变 | 本计划不增加或升级依赖 |

### 2.5 明确不变化的系统面

| 系统面 | 决定 |
| --- | --- |
| Jaco实现 | 本计划不修改；后续独立迁移 |
| 数据库、migration、schema、查询 | No change |
| 文件、网络、凭据、持久化 | No change |
| UI、组件、图标、assets、Fluent i18n | No change |
| 平台 bootstrap、打包、workflow | No change |
| `Cargo.toml` / `Cargo.lock` | No change |

## 3. 设计决策

### 3.1 一个 Store，一份状态

公开只存在：

```text
Store<S> -> one private GPUI Entity -> one S
```

`Store<S>` 是 shared handle，clone不复制 `S`。不再用 Local/Shared类型区分生命周期；
应用通过普通所有权、Entity字段或 typed global决定 Store活多久。

### 3.2 状态 cell 与通知共用一个 Entity

私有表示固定为：

```rust
struct StoreInner<S> {
    state: Rc<RefCell<S>>,
}

pub struct Store<S: 'static> {
    entity: Entity<StoreInner<S>>,
}
```

`S` 仍只有一份。额外的 `Rc<RefCell<_>>` 只让 whole-store observer在释放 GPUI Entity借用
之后，把 `&S` 与 `&mut Context<Owner>` 同时交给用户 callback；Entity自身继续作为唯一
通知源和生命周期 owner。

所有库生成的 observer和 selection闭包只捕获 `WeakEntity<StoreInner<S>>`，或使用 GPUI
observer传入的 Entity，不保留强 Store handle。用户传入的 selector/callback仍然可以自行
捕获 `store.clone()`；该显式强引用及可能形成的环由用户负责，库无法替用户打破。

### 3.3 修改与通知

- `set` 用 `mem::replace` 取出旧 `S`，释放 `RefMut<S>` 后调用 `cx.notify()`，从
  `Entity::update` 返回旧值，最后在 Entity借用之外 drop旧 `S`。
- `update` 执行 `FnOnce(&mut S) -> R`，释放 `RefMut<S>` 后总是 notify并返回 `R`。
- `update_if` 执行 `FnOnce(&mut S) -> StoreChange<R>`，释放 borrow后仅对
  `Changed` notify，并原样返回 outcome。
- `Unchanged` 不触发 rollback；调用者承诺没有改变可观察状态。
- Store不比较 `S`，所以不要求 `Clone` / `PartialEq`。
- 多个 mutation在同一 GPUI update cycle中可以被合并成一次 observer effect。

### 3.4 Whole-store observer 的借用边界

`observe` 继续把 `&S` 直接交给 callback，这是最短的只读使用路径。实现先从 observed
Entity克隆私有 state cell，再释放 Entity借用，最后借用 `S` 并调用 owner callback。

在 callback内：

- 可以读取参数 `&S`、修改 owner、读取或修改其他 Entity；
- 可以再次只读同一个 Store；
- 不能同步 `set` / `update` / `update_if` 同一个 Store，因为 `&S` 仍然存活；
- 若要反馈命令，必须使用 `cx.defer(...)`，并确保命令不会形成无限发布循环。

同步写回同一 Store是明确的 programmer error，并由测试固定为 panic；不增加 `S: Clone`
来掩盖该借用边界。

### 3.5 Selection

- `Select<S>` 本身不要求 `'static`、Clone或PartialEq。
- `Store::select` 因保存 selector而要求 `Selector: 'static`。
- selection output要求 `PartialEq + 'static`，只在输出变化时替换 snapshot并
  `cx.notify()` owner。
- `StoreSelection<T>` 不 Clone，不公开 source、setter、Deref、snapshot Rc或 revision。
- 创建 selection时同步计算初值；初值属于 owner构造的一部分，不额外 notify。

### 3.6 Observation 初始投递

私有 phase：

```rust
enum ObservationPhase {
    Pending,
    Active,
    Cancelled,
}
```

固定顺序：

1. 注册 `Context::observe` / `observe_in`；GPUI会 defer activation。Window版本同时先
   注册目标 WindowId的 `on_window_closed`。
2. 随后使用 `App::defer` / `Context::defer_in` 安排 initial delivery。
3. `Pending` 阶段若实际进入 source callback，不调用用户 callback；initial delivery会
   重新读取最新当前值。
4. initial delivery先切到 `Active`，再调用 callback。
5. `Active` 后才投递后续 publication或 distinct selected value。
6. 返回 `Subscription::join(source_subscription, cancellation_guard)`。
7. drop-before-initial把 phase切到 `Cancelled`；initial升级 owner/source失败时也显式切到
   `Cancelled`。Window关闭由下述 window-close subscription处理。

这保证注册不重入、第一次 callback是投递时的当前值、初始 callback先于后续 callback。

必须保留 GPUI effect队列的准确语义：

- 注册前已经排队的 source notify可能在 observer activation前执行，只由 initial current
  value体现；
- 注册后、initial effect执行前发生的 mutation会把 notify排在 initial之后；initial会先
  读到最新状态，随后 whole-store observer仍会收到该 publication，因此可能连续看到两次
  相同的 `&S`；
- selected observation会用 `PartialEq` 过滤上述重复 selected output；
- whole-store observation按 publication而不是相等性工作，不增加 epoch/revision或
  `S: PartialEq` 来消除这次合法的后续 callback。

Window版本还要读取 `window.window_handle().window_id()`，并注册
`App::on_window_closed`：

- 目标 Window关闭时，无论 initial尚未执行还是已经 Active，都把 phase切到
  `Cancelled`；
- 返回值嵌套 `Subscription::join`，同时持有 source observer、window-close observer和
  cancellation guard；
- phase Cancelled后 source observer保持 inert，直到返回的 Subscription、owner或 source
  drop；不通过共享 slot偷走 source Subscription，以免破坏 `Subscription::detach`。

### 3.7 Typed Global

- `impl<S: 'static> Global for Store<S>`。
- `install_global(&mut App, S)` 创建 Store、将 handle安装为 Global并返回同一 handle。
- `global(&impl AppContext)` 从 typed global clone handle；缺失时沿用 GPUI global panic。
- 每种 `S` 最多安装一个 global Store，启动顺序由应用负责。
- Store内部 mutation通知其 Entity observer，不替换 Global；消费者不得用
  `observe_global::<Store<S>>` 监听内部状态，应使用 `select` / `observe`。

## 4. 目标架构

### 4.1 文件树

```text
crates/gpui-store/
├── Cargo.toml
├── README.md
├── README.zh-CN.md
├── dev/
│   ├── README.md
│   └── in-memory-store.md
├── docs/
│   ├── README.md
│   ├── guide.md
│   └── guide.zh-CN.md
└── src/
    ├── change.rs
    ├── lib.rs
    ├── observation.rs
    ├── select.rs
    ├── selection.rs
    ├── store.rs
    └── tests.rs
```

删除：

- `src/backend.rs`
- `src/binding.rs`
- `src/delta.rs`
- `src/error.rs`
- `src/local.rs`
- `src/shared.rs`
- `src/test_support.rs`

`store.rs`、`selection.rs`、`tests.rs` 整体重写；不新增 `mod.rs`。
`tests.rs` 内部按 `core`、`selection`、`observation` 三个子 module组织，使每个工作包能
用 module path运行自己的完整测试集。

### 4.2 完整公开 API

`src/lib.rs` 只 re-export：

```rust
pub use change::StoreChange;
pub use select::Select;
pub use selection::StoreSelection;
pub use store::Store;
```

`StoreChange`：

```rust
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreChange<R> {
    Changed(R),
    Unchanged(R),
}

impl<R> StoreChange<R> {
    pub fn changed(result: R) -> Self;
    pub fn unchanged(result: R) -> Self;
    pub fn is_changed(&self) -> bool;
    pub fn into_result(self) -> R;
}
```

`Select`：

```rust
pub trait Select<S: ?Sized> {
    type Output;

    fn select(&self, source: &S) -> Self::Output;
}

impl<S: ?Sized, F, T> Select<S> for F
where
    F: Fn(&S) -> T,
{
    type Output = T;

    fn select(&self, source: &S) -> T;
}
```

`Store`：

```rust
pub struct Store<S: 'static> {
    entity: Entity<StoreInner<S>>,
}

impl<S: 'static> Clone for Store<S>;
impl<S: 'static> Global for Store<S>;

impl<S: 'static> Store<S> {
    #[must_use]
    pub fn new(cx: &mut impl AppContext, state: S) -> Self;

    pub fn install_global(cx: &mut App, state: S) -> Self;

    #[must_use]
    pub fn global(cx: &impl AppContext) -> Self;

    pub fn read<R>(
        &self,
        cx: &impl AppContext,
        read: impl FnOnce(&S) -> R,
    ) -> R;

    pub fn set(
        &self,
        cx: &mut impl AppContext,
        state: S,
    );

    pub fn update<R>(
        &self,
        cx: &mut impl AppContext,
        update: impl FnOnce(&mut S) -> R,
    ) -> R;

    pub fn update_if<R>(
        &self,
        cx: &mut impl AppContext,
        update: impl FnOnce(&mut S) -> StoreChange<R>,
    ) -> StoreChange<R>;

    pub fn select<Owner, Selector>(
        &self,
        cx: &mut Context<Owner>,
        selector: Selector,
    ) -> StoreSelection<Selector::Output>
    where
        Owner: 'static,
        Selector: Select<S> + 'static,
        Selector::Output: PartialEq + 'static;

    pub fn observe<Owner>(
        &self,
        cx: &mut Context<Owner>,
        observe: impl FnMut(
            &mut Owner,
            &S,
            &mut Context<Owner>,
        ) + 'static,
    ) -> Subscription
    where
        Owner: 'static;

    pub fn observe_select<Owner, Selector>(
        &self,
        cx: &mut Context<Owner>,
        selector: Selector,
        observe: impl FnMut(
            &mut Owner,
            &Selector::Output,
            &mut Context<Owner>,
        ) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        Selector: Select<S> + 'static,
        Selector::Output: PartialEq + 'static;

    pub fn observe_select_in<Owner, Selector>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        selector: Selector,
        observe: impl FnMut(
            &mut Owner,
            &Selector::Output,
            &mut Window,
            &mut Context<Owner>,
        ) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        Selector: Select<S> + 'static,
        Selector::Output: PartialEq + 'static;
}
```

`StoreSelection`：

```rust
#[must_use]
pub struct StoreSelection<T> {
    snapshot: Rc<SelectionCell<T>>,
    _subscription: Subscription,
}

impl<T> StoreSelection<T> {
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R;

    pub fn cloned(&self) -> T
    where
        T: Clone;
}
```

不实现：

- `StoreSelection: Clone`；
- `Deref` / `AsRef`；
- `Debug` / `Display` 转发；
- `PartialEq` / `Eq` 转发；
- `snapshot()` / `store_revision()`；
- public `Store::entity()` 或任何 public weak/raw handle；第 4.3 节仅允许 crate-private
  implementation helper。

### 4.3 内部类型

```rust
pub(crate) struct StoreInner<S> {
    state: Rc<RefCell<S>>,
}

pub(crate) struct SelectionCell<T> {
    value: RefCell<T>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObservationPhase {
    Pending,
    Active,
    Cancelled,
}
```

跨 module访问固定为 crate-private helper，而不是公开 raw handle：

```rust
impl<S: 'static> Store<S> {
    pub(crate) fn entity(&self) -> &Entity<StoreInner<S>>;
    pub(crate) fn downgrade(&self) -> WeakEntity<StoreInner<S>>;
}

impl<S> StoreInner<S> {
    pub(crate) fn state_cell(&self) -> Rc<RefCell<S>>;
}

impl<T> SelectionCell<T> {
    pub(crate) fn new(value: T) -> Self;
    pub(crate) fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R;
    pub(crate) fn replace(&self, value: T);
}
```

`StoreInner`、helper与 observation phase保持 crate-private，不出现在 rustdoc。

非 Clone selector与 `FnMut` callback的共享方式也固定：

```rust
let selector: Rc<Selector>;
let observer: Rc<RefCell<Observer>>;
let phase: Rc<Cell<ObservationPhase>>;
let selected_snapshot: Rc<SelectionCell<Selector::Output>>;
```

不得改为 `Selector: Clone`、`Observer: Clone` 或 `Output: Clone`。

### 4.4 端到端控制流

Mutation：

```text
Store command
  -> Entity::update(StoreInner)
  -> RefCell::borrow_mut(S)
  -> application closure
  -> release RefMut<S>
  -> notify according to set/update/update_if contract
  -> return business result
```

`set` 是该流程的特例：旧完整 `S` 被移出 Entity update并在 Entity借用外析构，避免旧值
destructor在 Store锁内运行。`update` / `update_if` 中若用户需要移出会重入或可能 panic的
payload，应把它作为 `R` / `StoreChange<R>` 的一部分返回，在 Store调用返回后再 drop。
GPUI可以在 `Entity::update` 返回前 flush notification effect，因此 Store不承诺旧 `S`
析构与 observer callback的先后；只承诺旧 `S` 不在 Store Entity lease或 state borrow内
析构。

Selection：

```text
Store::select
  -> read S and compute initial T
  -> register weak source observation
  -> source publication
  -> recompute T
  -> compare with stored T
  -> replace + owner cx.notify only when different
```

Observation：

```text
register source observer
  -> schedule initial delivery
  -> return joined Subscription
  -> initial effect reads current value and activates
  -> each later GPUI publication invokes callback
  -> drop Subscription cancels source and pending initial delivery
```

## 5. 上游复用审计

| 能力 | 动作 | 说明 |
| --- | --- | --- |
| `Entity<StoreInner<S>>` | Reuse | 状态生命周期、AppContext访问、notify源 |
| `WeakEntity` | Reuse | pending delivery和派生观察不延长 Store生命 |
| `Context::observe` | Reuse | owner-bound source观察 |
| `Context::observe_in` | Reuse | Window-aware观察 |
| `App::defer` / `Context::defer_in` | Reuse | 非重入 initial delivery |
| `App::on_window_closed` | Reuse | initial前或Active后关闭目标Window时取消phase |
| `Subscription::join` | Reuse | 合并 source取消与 pending delivery取消 |
| GPUI notification coalescing | Retain | 明确 effect语义，不自建逐 mutation队列 |
| `Global` | Reuse | typed global只保存 Store handle |
| `Rc<RefCell<S>>` | Adapt internally | 允许 `&S` 与 owner Context共存，不复制 S |
| 当前 `SnapshotCell`思路 | Adapt | 只保留内部 owner-bound snapshot，不保留 revision/public Rc |
| `SharedStore` / `LocalStore` | Delete | 一个 Store由普通所有权决定范围 |
| Backend / CommitBackend | Delete | I/O由应用 service/repository承担 |
| StoreBinding | Delete | form与command承担编辑 |
| revision/origin/update façade | Delete | 通知由三种 mutation API明确表达 |
| 自建 observer registry/scheduler | Do not add | GPUI已提供生命周期和effect系统 |

剩余自定义责任只有：内存状态 handle、显式 mutation语义、selector、selection snapshot和
initial-delivery glue。

## 6. 工作包

### ST-10：删除旧子系统并实现内存核心

**前置条件**

- 本计划第 3、4 节为冻结契约。
- 接受执行后 Jaco暂时不能通过 workspace编译的发布顺序。

**证据**

- 当前 `lib.rs`、`store.rs` 和 backend/local/shared模块公开旧模型。
- 锁定 GPUI Entity满足 clone、AppContext read/update、notify和Global要求。

**文件**

- 新增 `src/change.rs`、`src/select.rs`。
- 重写 `src/store.rs`、`src/lib.rs`。
- 删除 `src/backend.rs`、`binding.rs`、`delta.rs`、`error.rs`、`local.rs`、
  `shared.rs`。
- `Cargo.toml`、root Cargo与 lockfile不变。

**API 契约**

- 实现第 4.2 节的 `StoreChange`、`Select` 和 Store core/global/read/mutation API。
- `lib.rs` 只公开四个目标概念；ST-20完成前可暂时不 re-export selection实现，
  但最终边界不得出现旧符号。

**实施流程**

1. 先删除旧 module与 re-export，防止兼容层自然残留。
2. 建立 `StoreInner<S> { Rc<RefCell<S>> }` 与 `Store<S> { Entity<_> }`。
3. 手写 Store Clone，避免 `S: Clone` derive bound。
4. 使用 `Entity::read_with` / `update`；禁止 `as_mut`。
5. `set` 将旧完整状态作为 `Entity::update` 返回值移出，释放 Entity借用后再 drop；
   `update` / `update_if` 在 state borrow释放后按方法契约 notify。
6. 实现 typed global安装和读取。
7. 实现 StoreChange helper与 Select closure blanket impl。

**错误与生命周期**

- GPUI Entity缺失沿用 GPUI handle语义；
- `global` 未安装沿用 GPUI panic；
- mutation closure panic时不额外通知，也不捕获/包装 panic；
- `Unchanged` 不回滚 mutation；
- `set` 的旧 `S` destructor在 Entity借用之外运行；若它 panic，新状态已经安装，notify
  可能已排队或已投递，panic仍向上传播；
- Store drop同步释放 handle，不运行 shutdown或I/O。

**UI / 数据 / 数据库 / 图标 / i18n / 依赖**

- UI、数据模型、数据库、图标、i18n、依赖：No change。
- 旧 backend删除不等于迁移其 I/O；Jaco迁移另做。

**测试**

| 需求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| ST-R2 | `src/tests.rs` | `store_clone_shares_non_clone_state` | 非 Clone S | clone handle读到同一 mutation |
| ST-R3 | 同上 | `set_publishes_even_for_equal_value` | notify counter | 分开的effect cycle均通知 |
| set lifecycle | 同上 | `set_drops_replaced_state_exactly_once` | DropProbe S | 新状态已安装，旧S最终只drop一次；代码审查确认drop位于 `Entity::update` 返回后 |
| ST-R3 | 同上 | `update_returns_business_result_and_always_publishes` | counter | 返回 R且通知 |
| ST-R4 | 同上 | `update_if_changed_publishes_and_returns_result` | StoreChange | Changed通知并保留R |
| ST-R4 | 同上 | `update_if_unchanged_does_not_publish` | StoreChange | Unchanged不通知并保留R |
| ST-R9 | 同上 | `global_round_trip_returns_same_store` | TestAppContext | global clone共享状态 |
| ST-R5 | 同上 | `closure_and_named_selectors_share_contract` | named Select + closure | 输出一致 |
| helper API | 同上 | `store_change_helpers_preserve_decision_and_result` | Changed/Unchanged值 | `is_changed` 与 `into_result` 两分支正确 |
| AppContext | 同上 | `store_uses_read_with_and_update_on_async_app` | AsyncApp | 不触发 `as_mut` panic |

**验证**

```bash
cargo check -p gpui-store --lib --locked
cargo test -p gpui-store tests::core:: --lib --locked
```

**完成条件**

- 旧 backend/local/shared/binding公开边界为零；
- Store core不要求 S实现 Clone/PartialEq/Default/Send/Sync；
- mutation通知语义与业务结果均有测试。

### ST-20：实现 owner-bound StoreSelection

**前置条件**

- ST-10。

**证据**

- 当前 `selection.rs` 可复用 snapshot + Subscription基本思路，但 revision、public Rc和
  格式/比较转发均不属于目标。
- GPUI `Context::observe` 已弱持有 owner与 observed Entity。

**文件**

- 整体重写 `src/selection.rs`。
- 扩充 `src/tests.rs`。
- 在 `src/lib.rs` re-export `StoreSelection`。

**API 契约**

- 实现第 4.2 节 `select` 与 `StoreSelection`。
- selector output只要求 `PartialEq + 'static`。
- `cloned` 单独要求 `T: Clone`。

**实施流程**

1. 同步读取 Store并计算 initial output。
2. 将 output存入内部 `Rc<SelectionCell<T>>`。
3. 注册 source Entity观察；闭包只使用 GPUI传入的 observed Entity和 selector。
4. 每次 publication重新计算，先比较再替换。
5. 仅 selected output变化时调用 owner `cx.notify()`。
6. StoreSelection持有 Subscription；drop即退订。

**错误与生命周期**

- selector panic直接传播；
- selector必须纯、确定且不做 I/O；
- source Store先 drop时 selection保留最后一个 output，但不再更新；
- owner drop由 GPUI weak owner自动停订。

**UI / 数据 / 数据库 / 图标 / i18n / 依赖**

- 全部 No change。

**测试**

| 需求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| ST-R6 | `src/tests.rs` | `selection_starts_with_current_value` | owner + selection | 构造后可同步读取 |
| ST-R6 | 同上 | `selection_notifies_owner_only_when_output_changes` | unrelated/related fields | 仅相关输出变化notify |
| 激活顺序 | 同上 | `selection_observes_change_after_registration_in_same_update` | 同一update内修改 | 不丢最新输出 |
| ST-R10 | 同上 | `selection_supports_non_clone_output` | non-Clone PartialEq T | `read`可用 |
| owned read | 同上 | `selection_cloned_returns_owned_output` | Clone T | `cloned`返回当前值 |
| ST-R8 | 同上 | `dropping_selection_unsubscribes` | optional selection | 后续变化不notify owner |
| ST-R8 | 同上 | `selection_does_not_keep_store_alive_without_user_strong_capture` | selector不捕获Store + DropProbe S | 最后Store handle drop即释放S |
| API收敛 | rustdoc `compile_fail` | `selection_has_no_mutation_or_snapshot_api` | 自包含示例 | `set`、`snapshot`、`store_revision` 均不可调用 |

**验证**

```bash
cargo test -p gpui-store tests::selection:: --lib --locked
```

**完成条件**

- selection没有第二份权威状态；
- 库内部不以隐藏强引用延长 source/owner生命周期；用户 closure强捕获不在保证内；
- 无 T: Clone blanket bound。

### ST-30：实现观察与严格 initial delivery

**前置条件**

- ST-10；selected observation复用 ST-20 的 selector/snapshot规则。

**证据**

- GPUI observe只监听后续 notify，不自动投递当前值。
- observer activation、`defer` 和通知都进入同一 effect queue。
- `Subscription::join` 可组合两个取消动作。

**文件**

- 新增 `src/observation.rs`。
- 扩充 `src/tests.rs`。
- `store.rs` 只保留对 observation helper的调用，不重复状态机。

**API 契约**

- 实现第 4.2 节三个 observe方法。
- whole-store callback拿 `&S`；selected callback拿 `&Selector::Output`。
- observer callback是否调用 owner `cx.notify()` 由使用者决定。

**实施流程**

1. 建立 crate-private phase和 cancellation guard。
2. whole-store先注册 source observer，再 defer initial delivery。
3. initial effect升级 WeakEntity source与owner；任一升级失败就切 Cancelled，否则读取
   投递时当前值、切 Active并调用 callback。
4. Pending callback不调用用户代码；注册前已排队的变化由 initial current value体现。
5. Active whole-store notification对每次未被 GPUI合并的 publication调用 callback；
   注册后但 initial前排队的 notify也属于此类，因此可以在 initial之后再次投递相同状态。
6. selected observation另外维护 current T，以 PartialEq过滤。
7. Window版本使用 `observe_in` + `defer_in`，并注册目标 WindowId的
   `on_window_closed`；关闭时将 phase置 Cancelled。
8. 非 Window版本 join source Subscription与 guard；Window版本嵌套 join source、
   window-close Subscription与 guard；drop将 phase置 Cancelled。

**错误与生命周期**

- drop-before-initial、owner消失、window消失或 Store消失均不投递；
- whole-store callback同步写回同一 Store会 panic，文档要求 defer；
- selected callback计算完成后已释放 S borrow，可以发布显式 command，但必须避免反馈循环；
- whole-store initial已读到某次 mutation的结果时，该 mutation对应的 queued publication仍
  可以紧随 initial再次回调；这不是丢失或重入；
- 多次同 cycle通知可合并，不能把 callback次数当 mutation日志。

**UI / 数据 / 数据库 / 图标 / i18n / 依赖**

- Window仅作为现有 GPUI callback参数；不新增 UI。
- 数据、数据库、图标、i18n、依赖：No change。

**测试**

| 需求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| ST-R7 | `src/tests.rs` | `observe_initial_delivery_is_deferred` | owner counter | 注册调用栈内不执行 |
| ST-R7 | 同上 | `observe_initial_delivery_reads_latest_state` | 注册后、initial前mutation | 第一次看到最新值 |
| ST-R7 | 同上 | `observe_initial_precedes_queued_publication` | ordered log | initial先看到最新值，queued notify随后可再次看到同值 |
| ST-R8 | 同上 | `dropping_observation_before_initial_suppresses_delivery` | immediate drop | callback为零 |
| ST-R8 | 同上 | `initial_owner_or_source_loss_cancels_delivery` | initial前drop owner/source | callback为零，phase不再Active |
| ST-R3 | 同上 | `observe_runs_for_equal_set_and_update_publications` | equal state | 仍收到已发布effect |
| ST-R4 | 同上 | `observe_update_if_unchanged_does_not_run` | Unchanged | 无后续callback |
| 通知归属 | 同上 | `observe_does_not_notify_owner_implicitly` | owner notify counter | callback不调用notify时owner不刷新 |
| ST-R6 | 同上 | `observe_select_delivers_initial_then_only_distinct_values` | selected scalar | 相等过滤 |
| ST-R10 | 同上 | `observe_select_supports_non_clone_output` | non-Clone T | callback借用输出 |
| Window | 同上 | `observe_select_in_delivers_initial_and_changes_with_window` | Test window | Window callback正常 |
| Window lifecycle | 同上 | `observe_select_in_window_close_cancels_pending_and_active_delivery` | initial前关闭与Active后关闭两个Test window场景 | phase转为inert Cancelled，后续source变化不回调 |
| ST-R8 | 同上 | `dropping_subscription_stops_callbacks` | stored Subscription | drop后停止 |
| ST-R8 | 同上 | `observation_does_not_keep_store_alive_without_user_strong_capture` | callback不捕获Store + DropProbe S | source及时释放 |
| ST-R12 | 同上 | `multiple_writes_in_one_active_cycle_coalesce_to_latest` | initial完成后同一cycle内ordered writes | 不断言逐 mutation次数，最终值正确 |
| 借用边界 | 同上 | `observe_panics_on_synchronous_same_store_write` | `#[should_panic]` | 契约可见 |
| 借用边界 | 同上 | `observe_can_read_same_store` | nested read | shared read可用 |

**验证**

```bash
cargo test -p gpui-store tests::observation:: --lib --locked
```

**完成条件**

- initial、active和cancelled路径均有测试；
- Window与非 Window语义一致；
- 库内部不以强 Store捕获隐藏保活 source/owner；用户 callback强捕获不在保证内。

### ST-40：替换旧测试并同步双语公开文档

**前置条件**

- ST-10 至 ST-30 的最终签名和行为测试通过。

**证据**

- 当前 README/guide仍写“target API尚未实施”。

**文件**

- 完全重写 `src/tests.rs`，删除 `src/test_support.rs`。
- 更新 `src/lib.rs` crate-level rustdoc。
- 更新 `README.md`、`README.zh-CN.md`。
- 更新 `docs/README.md`、`docs/guide.md`、`docs/guide.zh-CN.md`。
- 不把 `dev/` 计划加入 public `docs/README.md`。

**API 契约**

- 移除 design status和“签名待定”提示。
- whole-store callback精确记录为 `FnMut(&mut Owner, &S, &mut Context<Owner>)`。
- StoreChange helper、initial delivery、same-cycle coalescing、same-store同步写限制全部双语一致。
- 默认英文；中文逐节对应。

**实施流程**

1. 用最终 rustdoc签名更新 API summary和所有示例。
2. 删除 Backend、Binding、Revision、Local/Shared旧术语的正向用法。
3. 更新 typed Global说明：Store内部 notify不是 Global替换。
4. 更新 selection/observation生命周期和初始投递顺序。
5. 更新 persistence和form边界。
6. 将自包含示例改为可编译 rustdoc；完整 app wrapper可保留 `rust,ignore`，但签名必须真实。

**错误与生命周期**

- 文档说明 Global缺失、mutation panic传播、observer reentry和Subscription drop。
- 不把 backend失败包装成 Store error；Store公开无 error type。

**UI / 数据 / 数据库 / 图标 / i18n / 依赖**

- UI、数据、数据库、图标、应用 i18n、依赖：No change。
- 双语 Markdown不新增 Fluent资源。

**测试**

| 需求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| ST-R1–R12 | rustdoc | public examples | crate docs | 自包含示例编译 |
| 旧 API删除 | residual scan | obsolete exports/usages | `rg` | crate源码和public docs无正向旧用法 |
| 双语契约 | manual diff review | section parity | 两份guide | 类型、签名、顺序一致 |

**验证**

```bash
cargo doc -p gpui-store --no-deps --locked
! rg -n 'SharedStore|LocalStore|StoreState|StoreBackend|StoreCommitBackend|StoreBinding|StoreRevision|StoreUpdateOrigin|StoreUpdate|StoreRuntime|MemoryBackend|StoreDelta|StoreBackendUnsupported' \
  crates/gpui-store/src
rg -n 'SharedStore|LocalStore|StoreState|StoreBackend|StoreCommitBackend|StoreBinding|StoreRevision|StoreUpdateOrigin|StoreRuntime' \
  crates/gpui-store/README.md \
  crates/gpui-store/README.zh-CN.md \
  crates/gpui-store/docs
```

源码扫描必须零匹配；public docs扫描只允许非目标/迁移说明中的名称，不允许 re-export、
签名或正向用法。

**完成条件**

- 公开文档与代码逐项一致；
- 旧测试、helper和旧文档计划不再存在；
- `docs/` 只保留对外文档。

### ST-50：包级完成与 Jaco 发布交接

**前置条件**

- ST-10 至 ST-40。

**证据**

- 第 2.2 节记录了当前静态扫描发现的旧 Jaco调用类别，但明确不是穷尽证明。

**文件**

- 本工作包不修改 Jaco。
- 只记录实际包级验证结果和后续迁移门槛。

**API 契约**

- 不增加临时 alias使 Jaco继续编译。

**实施流程**

1. 完成第 7.1 节所有包级门禁。
2. 用类型名和 method名两组 `rg` 重新生成 Jaco旧 API inventory，供后续迁移使用。
3. 将 Store实现状态标记为“package complete, workspace release-gated”。
4. 停止；Jaco迁移由 issue #177 的独立实施计划继续。

**错误与生命周期**

- 本工作包不运行 workspace编译，也不声称已知 inventory穷尽；若 gpui-store包级门禁失败
  则不得交接。

**UI / 数据 / 数据库 / 图标 / i18n / 依赖**

- 全部 No change。

**测试**

| 需求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| 发布边界 | shell scan | `jaco_old_store_api_inventory` | 类型名 + method名 `rg` | 保存当前已知迁移面，不声称穷尽 |
| 包完成 | package gates | 全部 Store tests | GPUI test support | 包级全绿 |

**验证**

```bash
rg -n 'SharedStore|LocalStore|StoreState|StoreBackend|StoreCommitBackend|StoreBinding' \
  app/jaco/src
rg -n 'read_cloned|select_cloned|sync_initial|refresh_from_backend|bind_committed|try_set|try_update|try_update_if|try_update_field' \
  app/jaco/src
rg -n '(catalog|project_catalog|provider_catalog|skill_catalog|prompt_catalog)\.entity\(' \
  app/jaco/src
```

**完成条件**

- Store package实现与文档完整；
- 当前已知 Jaco迁移面已记录且未被兼容层隐藏；穷尽性留给后续迁移与 workspace编译证明；
- 不声称 workspace/CI已通过。

## 7. 跨工作包验证

### 7.1 gpui-store 包级完成门禁

在所有 Store工作包完成后只运行一次：

```bash
cargo fmt --package gpui-store
cargo build -p gpui-store --locked
cargo test -p gpui-store --lib --locked
cargo test -p gpui-store --doc --locked
cargo doc -p gpui-store --no-deps --locked
cargo clippy -p gpui-store --all-targets --all-features --locked -- -D warnings
git diff --check
! rg -n '[[:blank:]]+$' crates/gpui-store
! rg -n $'\r' crates/gpui-store
```

预期：

- core、selection、observation、Window和生命周期测试全部通过；
- rustdoc只显示四个目标概念及其公开方法；
- 没有旧 backend/binding/revision symbol；
- manifest与 lockfile无变化。

### 7.2 Workspace / merge gate

breaking Store实现完成后，不运行或宣称以下命令已通过，直到 Jaco迁移完成：

```bash
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

最终发布顺序：

```text
gpui-store package complete
  + Jaco Store call sites migrated
  -> workspace build/test/clippy
  -> macOS/Linux/Windows CI
  -> merge-ready
```

CI 最终仍以 `.github/workflows/ci.yml` 为准：三平台 build/test，macOS
`--all-targets --all-features` clippy。

## 8. 执行交接审计

- [x] 单一 Store的公开类型、方法、泛型 bound和返回值已确定。
- [x] 内部唯一状态、Entity、RefCell和 WeakEntity生命周期已确定。
- [x] set/update/update_if通知与业务结果契约已确定。
- [x] selection初值、过滤、owner通知和 source drop语义已确定。
- [x] observation initial、激活、取消、Window和effect coalescing顺序已确定。
- [x] whole-store同步写回限制有明确文档和测试，不留给实施者选择。
- [x] typed Global安装、读取和通知边界已确定。
- [x] 旧模块、API、测试和计划均有删除清单。
- [x] GPUI 上游能力已按 lockfile commit核对，不自建 observer。
- [x] 数据库、UI、图标、i18n、依赖和平台面均有明确 No change。
- [x] 包级完成与 Jaco/workspace发布门槛已分开。
- [x] 实施者无需再选择架构；若目标签名无法按本文实现，应停止并回到设计评审，
  不得自行增加 Clone bound、兼容 wrapper、backend或第二份状态。
