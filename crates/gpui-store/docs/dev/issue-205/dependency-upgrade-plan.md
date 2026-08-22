# gpui-store 依赖升级计划

- 状态：`In progress`（本地自动化通过；三平台 CI 待执行）
- Owner：`crates/gpui-store`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

本 crate 拥有 typed shared in-memory `Store<S>`、global installation、selection 与 observation
生命周期。它直接建立在 GPUI Entity/Context 上，但不消费 UI component；不增加
`gpui-component` 或 `gpui-base`。

## 精确依赖与已知命中

`crates/gpui-store/Cargo.toml:7-10` 的完整 direct audit：normal `gpui` 与 dev
`gpui/test-support` 当前均为 `0.2.2 @ 1a246efd...`，目标均为
`0.2.2 @ e0931d5a...`。没有其他 direct dependency；两个 edge 必须解析到 root 唯一的
Zed Git identity。

- `src/store.rs:6,59-85` 使用 `App`、`AppContext`、`Context`、`Entity`、`Global`、
  `Subscription`、`WeakEntity`、`Window`，并实现 cloneable Store/global installation。
- `src/store.rs:157-468` 的 `select`、`observe`、`observe_select` 及 window-aware variants 依赖
  GPUI observe/defer 生命周期，是升级的主要行为风险。
- `src/projection/{observation,selection}.rs` 保存 subscription 与 callback ownership。
- `src/tests.rs` 覆盖 publication、selection、initial delivery、distinct filtering、owner/source
  loss、self-cancellation、window-aware observation 与 destructor panic；这些 tests 是验收规范。

## 工作包

### STORE-DEP-1：API 兼容

- 用 root GPUI pin 编译 Store 与 projection modules；按真实错误适配 Context/Entity/observe API。
- 不改变 publication 时机、initial delivery 顺序或 weak ownership 来迎合新 API。

### STORE-DEP-2：生命周期回归

- 全量执行 store tests，重点检查 initial delivery 先于 queued publication、drop subscription、
  owner/source loss、callback self-cancellation 与 non-Clone selection output。
- 对 window-aware `observe_in`/`observe_select_in` 至少保留一个真实 window test。

### STORE-DEP-3：依赖边界

- `cargo tree` 确认 normal graph 只有一份 root-pinned GPUI，不含 component/base。
- Jaco/Feiwen 作为消费者只做编译与状态 smoke，不把 app 行为塞入 Store tests。

## Focused verification

```text
cargo check -p gpui-store --locked
cargo test -p gpui-store --locked
cargo clippy -p gpui-store --all-targets --all-features --locked -- -D warnings
cargo tree -p gpui-store --edges normal --locked
cargo check -p jaco -p feiwen --locked
```

## 完成条件

- Store publication、selection 与 observation 的顺序/ownership contracts 不变。
- crate 只直接依赖 GPUI，未引入 component 或 base。
