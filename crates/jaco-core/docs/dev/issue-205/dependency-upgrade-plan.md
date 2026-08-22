# jaco-core：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（本地自动化通过；三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/jaco-core`
- Root-owned surfaces consumed：`S-17`、`S-19`
- Owner-local IDs：`F-JCORE-01`–`F-JCORE-03`、`R-JCORE-01`–`R-JCORE-02`、`T-JCORE-01`–`T-JCORE-02`、`WP-JCORE-01`
- Owns：domain/payload crate 的 time/UUID dependency targets 和 serialization-focused regression。
- Does not own：database storage, provider runtime, GPUI projection, workspace lockfile 或 domain schema redesign。

## 精确依赖目标

| Dependency | Current | Target | Preserved features | Classification | Local use |
| --- | --- | --- | --- | --- | --- |
| `time` | `0.3.54` | `0.3.55` | `serde` | Compatible | timestamps in `src/domain.rs` and payloads |
| `uuid` | `1.24.0` | `1.24.1` | `serde`, `v7` | Compatible | typed IDs and v7 generation in `src/lib.rs:23`/domain |
| `serde` | `1.0.229` | 保留 `1.0.229` | `derive` | Retained | domain/payload wire forms |
| `serde_json` | `1.0.151` | 保留 `1.0.151` | default | Retained | payload tests/conversions |
| `gpui-operation` | workspace path | 保留 workspace path | workspace features | Retained | operation-facing domain types |

## Owner-local 目标与文件

```text
crates/jaco-core/
├── Cargo.toml                                      # F-JCORE-01 [Modify] time/uuid compatible targets
├── src/domain.rs + src/lib.rs                      # F-JCORE-02 [Verify only] typed IDs and timestamps
└── src/payloads.rs + src/payloads/**/*.rs          # F-JCORE-03 [Verify only] serde contract tests
```

- `R-JCORE-01`：UUID v7 generation、ordering/equality 与 serde representation 保持现有 domain contract。
- `R-JCORE-02`：`OffsetDateTime` payload round trips 和字段 shape 不变；不得借版本升级改 wire schema。

## WP-JCORE-01：升级 time/uuid 并锁定 domain wire contract

1. 在 `F-JCORE-01` 更新 `time = "0.3.55"` 与 `uuid = "1.24.1"`，原样保留 features。
2. 使用 Cargo 更新 root lockfile；预期 `F-JCORE-02`–`F-JCORE-03` 无源码变化。
3. 运行 payload/domain tests 和 strict Clippy。若 serialization snapshot/shape 变化，停止升级并记录具体 upstream semantic change，不引入双格式兼容层。

完成条件：两个 target 精确落地，所有 payload/domain tests 通过，序列化与 ID 不变量无偏差。

## Focused Validation 与 handoff

| T-ID | Command | Expected evidence |
| --- | --- | --- |
| `T-JCORE-01` | `cargo test -p jaco-core --all-features --locked` | domain 与 payload round-trip tests 通过 |
| `T-JCORE-02` | `cargo clippy -p jaco-core --all-targets --all-features --locked -- -D warnings` | 所有 target 无 warning |

数据库 round trip 和 provider consumer tests 分别由 `jaco-db`、`jaco-agent` owner 负责；root 汇总跨 owner 结果。
