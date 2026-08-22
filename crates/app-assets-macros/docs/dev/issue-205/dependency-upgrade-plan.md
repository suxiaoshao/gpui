# app-assets-macros：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（本地自动化通过；三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/app-assets-macros`
- Root-owned surfaces consumed：`S-17`、`S-19`
- Owner-local IDs：`F-AAM-01`、`R-AAM-01`、`T-AAM-01`–`T-AAM-02`、`WP-AAM-01`
- Owns：proc-macro 直接依赖的版本声明，以及宏 crate 的 focused 编译验证。
- Does not own：workspace `Cargo.lock`、上游版本证据、`app-assets` 的运行时资源行为或其他 owner 的依赖。

本 owner 在 2026-08-20 的目标快照中没有 manifest 版本变化；它仍然需要参与全量升级验证。执行前由
root plan 联网刷新目标，若出现新候选，先同步 root/owner 计划再修改 manifest。

## 精确依赖目标

| Dependency | Current | Issue #205 target | Features/kind | Classification | Local use |
| --- | --- | --- | --- | --- | --- |
| `proc-macro2` | `1.0.107` | 保留 `1.0.107` | runtime proc-macro | Retained | `src/lib.rs:100,239,333` 的 token stream |
| `quote` | `1.0.47` | 保留 `1.0.47` | runtime proc-macro | Retained | `src/lib.rs:108-271` 的代码生成 |
| `syn` | `3.0.3` | 保留 `3.0.3` | `full` | Retained | `src/lib.rs:3-6,48` 的输入解析 |

所有依赖继续来自 crates.io registry；不新增 feature、Git source、build dependency 或 dev dependency。

## Owner-local 目标与文件

```text
crates/app-assets-macros/
├── Cargo.toml                                      # F-AAM-01 [Retain] 三个直接依赖声明不变
└── src/lib.rs                                      # [No change expected] 宏解析与生成行为保持不变
```

`R-AAM-01`：升级后的 workspace resolution 必须仍能展开 `lucide_icons!` 与 SVG enum 宏，且不得为一次
无候选版本审计制造源码或 snapshot 改动。

## WP-AAM-01：确认保留项并完成 proc-macro gate

1. 在 root 在线版本刷新完成后复核三项依赖；目标仍相同则不改 `F-AAM-01`。
2. 由 root owner 使用 Cargo 重新生成 workspace lockfile；本 owner 不手改 `Cargo.lock`。
3. 运行 `T-AAM-01` 与 `T-AAM-02`。若新版 Rust/解析依赖改变宏输出或诊断，停止本 WP 并先补充精确迁移计划，不以无关源码改动绕过。

完成条件：本 crate manifest 无非预期 diff，宏 crate 的测试和严格 Clippy 通过，root completion evidence 记录
“audited, retained”。

## Focused Validation 与 handoff

| T-ID | Command | Expected evidence |
| --- | --- | --- |
| `T-AAM-01` | `cargo test -p app-assets-macros --all-features --locked` | proc-macro crate 编译及现有测试通过 |
| `T-AAM-02` | `cargo clippy -p app-assets-macros --all-targets --all-features --locked -- -D warnings` | 无新增 warning |

任何 target、feature、source 或源码偏差都必须回填 root canonical plan；聚合 `cargo build/test/clippy` 和
最终 lockfile diff 仍由 root owner 负责。
