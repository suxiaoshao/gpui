# jaco-conversation：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（本地自动化通过；三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/jaco-conversation`
- Root-owned surfaces consumed：`S-17`、`S-19`
- Owner-local IDs：`F-JC-01`–`F-JC-02`、`R-JC-01`、`T-JC-01`–`T-JC-02`、`WP-JC-01`
- Owns：conversation service 的 `thiserror` update 和现有 service/error tests。
- Does not own：`jaco-core`/`jaco-db` implementation、database schema、workspace lockfile 或 Jaco UI。

## 精确依赖目标

| Dependency | Current | Target | Classification | Local use |
| --- | --- | --- | --- | --- |
| `thiserror` | `2.0.19` | `2.0.20` | Compatible | `src/lib.rs:6,78` 的 service error derive/behavior |
| `jaco-core` | workspace path | 保留 workspace path | Retained | domain types |
| `jaco-db` | workspace path | 保留 workspace path | Retained | repository boundary |
| `tempfile` | `3.27.0` | 保留 `3.27.0` | Retained dev | repository fixture |

## Owner-local 目标与文件

```text
crates/jaco-conversation/
├── Cargo.toml                                      # F-JC-01 [Modify] thiserror 2.0.19 -> 2.0.20
└── src/lib.rs                                      # F-JC-02 [Verify only] service/error mapping and tests
```

`R-JC-01`：升级不得改变公开 error variant、conversation CRUD flow 或 repository failure mapping；预期没有源码改动。

## WP-JC-01：更新 error derive 并验证 service contract

1. 修改 `F-JC-01` 的唯一 registry target，保留两个 workspace path dependency 和 dev fixture。
2. 由 Cargo 更新 root lockfile；仅当 `thiserror` 明确要求时编辑 `F-JC-02`，不重命名公开错误。
3. 运行 focused tests 和 strict Clippy。

完成条件：manifest 只有 `thiserror` target diff，conversation tests 保持通过，公开行为无变化。

## Focused Validation 与 handoff

| T-ID | Command | Expected evidence |
| --- | --- | --- |
| `T-JC-01` | `cargo test -p jaco-conversation --all-features --locked` | service/repository fixture tests 通过 |
| `T-JC-02` | `cargo clippy -p jaco-conversation --all-targets --all-features --locked -- -D warnings` | 无新增 warning |

跨 crate aggregate tests 与最终 lockfile evidence 由 root owner 记录。
