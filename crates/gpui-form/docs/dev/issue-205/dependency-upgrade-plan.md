# gpui-form 依赖升级计划

- 状态：`In progress`（本地自动化与 trybuild 通过；三平台 CI 待执行）
- Owner：`crates/gpui-form`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

本 crate 拥有与具体控件库无关的 form、path、binding、validation 与 GPUI owner/context
生命周期。`InputState` 的三分拆适配属于 `gpui-form-gpui-component`，不能泄漏进核心 crate。

## 精确依赖与已知命中

`crates/gpui-form/Cargo.toml:11-20` 的完整 direct audit：

| Edge | Current | Target / disposition |
| --- | --- | --- |
| normal `garde` | optional workspace 0.23.0 | current |
| normal `gpui` | 0.2.2 @ `1a246efd...` | 0.2.2 @ `e0931d5a...` |
| normal `gpui-form-macros` | workspace path 0.1.0 | current |
| normal `gpui-operation` | workspace path 0.1.0 | current |
| dev `gpui/test-support` | 同 normal GPUI source | 0.2.2 @ `e0931d5a...` |
| dev `gpui-component` | 0.5.2 @ `57a9903f...` | 0.5.2 @ `5e5a1a30...` |
| dev `gpui-form-gpui-component` | workspace path 0.1.0 | current |
| dev `trybuild` | 1.0.118 | 1.0.120 |

本 crate 不依赖 `gpui-base`，normal graph 也不依赖完整组件；升级后保持该边界。

GPUI API 命中集中在：

- `src/control.rs:13-14` 的 `AnyWindowHandle`、`App`、`Context`、`Entity`、`Subscription`、
  `WeakEntity`、`Window` 与 operation transition；
- `src/path.rs:5` 的 form entity/path mutation，`src/form.rs:10-11` 的 Context/EventEmitter，
  `src/validation/transition.rs:3-4` 的 `Task`；
- `src/topology/address.rs:148-154` 的 `PathKey -> gpui::ElementId` 转换；
- `tests/*.rs` 的 `TestAppContext`、`#[gpui::test]`、binding defer/blur、async freshness 与
  trybuild contracts。

目标 `gpui-component` 的 `InputState` 分拆不改变这些核心 contracts；不要在此 crate 加
`FormInput`/`FormTextarea`/`FormEditor` 类型或 component dependency。

## 工作包

### FORM-DEP-1：GPUI 核心兼容

- 在 root GPUI pin 更新后编译 core、macros 展开结果、tests 与 doctests。
- 只按真实编译错误调整 Context/Entity/Task API；不借升级重塑 form 状态模型。

### FORM-DEP-2：binding contract 回归

- 锁定 initial projection、UI change、external projection、blur validation、retired occurrence、
  weak owner/window loss 与 async freshness。
- adapter 的三种 text state 必须复用这些 contracts，而不是把特例放入核心。

### FORM-DEP-3：dev graph 验证

- 等组件 adapter 完成后运行完整 dev graph，确认没有 cyclic normal dependency。
- trybuild `.stderr` 只有在编译器诊断确实变化时按逐案证据更新，禁止批量覆盖。
- 将 `trybuild` 更新到 1.0.120，逐个检查 pass/fail fixture 与期望诊断。

## Focused verification

```text
cargo check -p gpui-form --all-features --locked
cargo test -p gpui-form --lib --tests --all-features --locked
cargo test -p gpui-form --doc --all-features --locked
cargo clippy -p gpui-form --all-targets --all-features --locked -- -D warnings
cargo tree -p gpui-form --edges normal --locked
```

通过条件：normal dependency graph 不含 `gpui-component` 或 `gpui-base`；binding、validation、
async 与 trybuild tests 全部通过。

## 完成条件

- form 核心 API/生命周期语义不因组件 input 分拆而改变。
- 正常依赖边界保持 UI-library agnostic，dev graph 在新组件版本下通过。
