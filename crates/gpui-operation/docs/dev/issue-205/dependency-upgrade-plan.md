# gpui-operation 依赖升级计划

- 状态：`In progress`（本地自动化通过；三平台 CI 待执行）
- Owner：`crates/gpui-operation`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

本 crate 拥有不绑定 executor 的 refresh/repair 状态机与 message-driven transition。它的正常
依赖图不含 GPUI；只有 integration tests 使用真实 `gpui::Task` 验证取消与 completion routing。

## 精确依赖与已知命中

`crates/gpui-operation/Cargo.toml:11-14` 的完整 direct audit：

| Edge | Current | Target / disposition |
| --- | --- | --- |
| normal optional `tracing` | 0.1.44 | current |
| dev `gpui/test-support` | 0.2.2 @ `1a246efd...` | 0.2.2 @ `e0931d5a...` |

没有其他 direct dependency。本 crate 不依赖 `gpui-component` 或 `gpui-base`；`tracing`
的 current 结论来自 2026-08-20 root audit，实施当天重审。

- `crates/gpui-operation/tests/gpui_task.rs:10-11` 导入 `App`、`AppContext`、`Task`、
  `TestAppContext` 与 operation messages。
- `tests/gpui_task.rs:77-209` 用 `#[gpui::test]` 锁定 pending task drop cancellation、已取消
  task 不得 completion、AsyncApp 回写 owner 等行为。
- `src/refresh.rs`、`src/repair.rs`、`src/transition.rs` 不引用 GPUI 类型；不能因测试适配把
  GPUI dependency 提升为 normal dependency。

## 工作包

### OP-DEP-1：保持纯状态机边界

- 只更新 dev GPUI pin，保留默认 normal graph 为空和 `tracing` 可选 feature。
- 若 `Task`/`AsyncApp` API 改变，仅适配 integration test harness；状态机公开 API 不跟随 UI
  framework 漂移。

### OP-DEP-2：任务语义回归

- 重跑 pending task drop、cancel 后 stale completion、completion 回写与 self-replacement tests。
- 禁止用 `detach` 或额外 fallback 让测试通过；Task drop 取消仍是 owner 语义的一部分。

### OP-DEP-3：依赖图门禁

- 分别检查 default 与 `tracing` normal dependency graph，确认没有 GPUI、component 或 base。
- dev graph 允许唯一 root-pinned GPUI source，不允许出现第二份 Git identity。

## Focused verification

```text
cargo check -p gpui-operation --all-features --locked
cargo test -p gpui-operation --lib --tests --all-features --locked
cargo test -p gpui-operation --doc --all-features --locked
cargo clippy -p gpui-operation --all-targets --all-features --locked -- -D warnings
cargo tree -p gpui-operation --edges normal --locked
cargo tree -p gpui-operation --edges normal --features tracing --locked
```

## 完成条件

- 默认与 tracing normal graph 保持无 GPUI。
- 真实 GPUI Task cancellation/completion tests 在新 pin 下保持原语义。
