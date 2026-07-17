# gpui-store typed catalog snapshot 计划

状态：已实施。`gpui-store` 的失败提交/selection 生命周期回归已补齐，Jaco project/provider/shortcut 已接入
typed `SharedStore` snapshot；消费端的重复 business cache 已收敛为 `StoreSelection` 与显式 projection。
本文只定义 `gpui-store` 在 catalog 场景中的边界；form/component 分离见
`crates/gpui-form/docs/external-state-synchronization-plan.md`。

## 1. 范围与决定

目标：catalog store 保存完整 typed committed snapshot；多个消费者通过 `StoreSelection` 读取；repository
command 成功后由 app 明确 reload 并一次性替换 snapshot；options/catalog 永远不 hydrate/rebase form。

第一阶段不修改 `gpui-store` public API，不增加 computed selector、catalog abstraction、async backend、error
status、event coalescing、retry 或 task cancellation。不定义 Jaco 类型，不依赖 `gpui-form`，无 schema、依赖、
icon、i18n 或平台变化。

## 2. 证据与依赖

- `StoreSelection<T>` 已提供 owner-bound `Rc<T>` snapshot，并仅在 selected value 改变时通知。
- `SharedStore<S, MemoryBackend>` 已满足跨页面 typed snapshot owner 和同步 command 更新。
- 当前 `StoreBackendFuture<T, Error>` 是同步 `Result<T, Error>`；为 Jaco repository reload 强行实现
  backend 不会减少 I/O 或生命周期复杂度。
- Jaco project/provider event 当前只是 reload signal。页面各自查询和缓存 rows/choices 才是 stale owner 根因。

依赖保持：`gpui 0.2.2`（Zed rev `1d217ee39d381ac101b7cf49d3d22451ac1093fe`）、workspace
`gpui-store 0.1.0`，无 Cargo/lockfile 变化。

## 3. 最终模型

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProjectCatalogState {
    pub(crate) projects: Arc<[ProjectSummary]>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProviderCatalogState {
    pub(crate) models: Arc<[ProviderModelChoice]>,
}
```

不在 state 重复保存 store revision；`SharedStore::revision()` 已提供元数据。load error 属于执行 reload command
的 page/service。失败时 store 不变。

```text
repository/event signal
  -> app reload command queries repository
  -> success: normalize/sort complete snapshot
  -> SharedStore<..., MemoryBackend>::set/update_if
  -> StoreSelection refreshes changed projections

failure -> keep last committed snapshot -> route app error
```

component options 是 catalog 的消费结果：

```text
catalog selection -> component-specific set_items/set_disabled
                  -> no form mutation
```

submit 读取 form draft 和 catalog store snapshot，由 app pure resolver 计算 effective value；不读取 UI cache。

## 4. Ownership

| 数据 | owner | 写入 |
| --- | --- | --- |
| project/provider catalog | app shared `SharedStore<S, MemoryBackend>` | repository reload success |
| form draft | generated form entity | user/programmatic form command |
| component options | app/controller projection | catalog command |
| component interaction | component entity | component events |
| persisted DB/config | repository/config service | domain command |

project selection 与 catalog 不合并为第三个 store。页面持有 selected id，按需用纯函数从 catalog snapshot 派生
presentation；这不是跨-store computed selector 需求。

## 5. 删除优先审计

| 当前/旧计划 | 决定 |
| --- | --- |
| revision-only Jaco catalog entity | Replace with typed SharedStore snapshot |
| per-page rows/choices business cache | Delete |
| Jaco-specific `StoreBackend` | Do not add in phase 1 |
| catalog load/error status inside committed state | Do not add |
| store snapshot -> form rebase | Delete |
| cross-store `ComputedSelection` | Do not add |
| existing `StoreSelection` | Reuse directly |
| existing backend traits | Retain for genuine external backends |

## 6. 工作包

### STORE-10：锁定现有 crate contract

只补 `crates/gpui-store/src/tests.rs`：`selection_ignores_unrelated_state_changes`、
`selection_holds_replaced_snapshot_safely`、`failed_command_does_not_replace_memory_snapshot`、
`selection_drop_releases_owner_subscription`。不新增 public API。

### STORE-20：Jaco typed catalog owner（已实施）

修改 `app/jaco/src/state/projects.rs`、`state/providers.rs`：定义 app-owned catalog state/global handle，
repository query 成功后一次 `set/update_if`；失败保留旧 snapshot。删除 revision-only entity 和重复 cache。

测试 initial load、reload success/failure、rename/remove、model disable/remove。

### STORE-30：消费端迁移（已实施）

修改 Jaco project/run-settings controllers：catalog selection 只更新 component config/presentation；form draft
保持不变。submit pure resolver 直接读取 catalog snapshot。

测试 options update 不改变 form、project rename 刷新 presentation、removed model 遵循 caller policy、attachment
capability 使用 resolved model。

## 7. GPUI 与系统面

- 同一个 entity 不能 nested update；store callback 不更新正在触发它的 component/form entity。
- catalog reload task 由 app owner 保存和取消；成功结果回 foreground command。
- DB schema/query/transaction 不变；复用现有 list queries。
- UI/focus/keyboard/accessibility、icons/assets、i18n、dependencies/platform：No change。

## 8. 验证

```bash
cargo fmt --all
cargo test -p gpui-store
cargo check -p gpui-store
cargo clippy -p gpui-store --all-targets --all-features -- -D warnings
cargo check -p jaco
cargo test -p jaco --no-run
cargo clippy -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

完成条件：catalog 是完整 typed snapshot；Jaco 没有 revision-only catalog 或 per-consumer business cache；
options 更新不写 form；`gpui-store` public API 无新增。
