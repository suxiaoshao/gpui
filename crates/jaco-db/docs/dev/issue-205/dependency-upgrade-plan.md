# jaco-db：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（Diesel/SQLite 本地自动化通过；三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/jaco-db`
- Root-owned surfaces consumed：`S-10`、`S-17`、`S-19`
- Owner-local IDs：`F-JDB-01`–`F-JDB-05`、`R-JDB-01`–`R-JDB-04`、`T-JDB-01`–`T-JDB-04`、`WP-JDB-01`–`WP-JDB-02`
- Owns：Diesel/SQLite dependency pair、Windows bundled SQLite feature 和 repository/schema regression。
- Does not own：workspace `Cargo.lock`、Jaco UI/service、业务 schema redesign 或新 migration。

## 精确依赖目标

### Atomic Diesel/SQLite update

| Dependency | Current | Target | Preserved features | Classification | Local use/constraint |
| --- | --- | --- | --- | --- | --- |
| `diesel` | `2.3.11` | `2.3.12` | `sqlite`, `r2d2`, `time`, `returning_clauses_for_sqlite_3_35`, `serde_json` | Compatible | models、repository、migration、pool |
| `libsqlite3-sys` | `0.37.0` | `0.38.2` | `bundled-windows` | Breaking version; compatible only in this atomic pair | direct dependency solely forces Windows bundled SQLite and owns `links = "sqlite3"` |

Diesel 2.3.12 的 verified constraint 接受 `libsqlite3-sys < 0.39.0`；0.38.2 仍提供 `bundled-windows`。
两项必须同一 WP、同一 lockfile resolution 落地，且 graph 中只能有一个 `sqlite3` links owner。

### Other direct dependencies

| Dependency | Current | Target | Features/classification |
| --- | --- | --- | --- |
| `thiserror` | `2.0.19` | `2.0.20` | Compatible |
| `time` | `0.3.54` | `0.3.55` | Compatible; `formatting`, `parsing`, `serde` retained |
| `serde` | `1.0.229` | 保留 `1.0.229` | `derive` |
| `serde_json` | `1.0.151` | 保留 `1.0.151` | default |
| `url` | `2.5.8` | 保留 `2.5.8` | default |
| `tempfile` | `3.27.0` | 保留 `3.27.0` | dev |
| `jaco-core` | workspace path | 保留 workspace path | workspace |

保留 `[package.metadata.cargo-shear].ignored = ["libsqlite3-sys"]` 及其注释；源码不直接 import 该 crate 是有意设计。

## Owner-local 目标与文件

```text
crates/jaco-db/
├── Cargo.toml                                      # F-JDB-01 [Modify] atomic pair + time/thiserror targets
├── src/models.rs + src/models/**/*.rs              # F-JDB-02 [Verify; edit only if Diesel requires]
├── src/repository.rs + src/repository/**/*.rs      # F-JDB-03 [Verify; edit only if Diesel requires]
├── src/store.rs + src/validation.rs                 # F-JDB-04 [Verify] pool/pragmas/integrity
└── src/schema.rs + src/migrations.rs + src/tests/** # F-JDB-05 [Retain/verify] no schema or migration change
```

- `R-JDB-01`：最终 graph 只有 `libsqlite3-sys 0.38.2` 提供 `links = "sqlite3"`，Windows 启用 `bundled-windows`。
- `R-JDB-02`：现有 bootstrap/migration transaction、schema validation、foreign-key/integrity behavior 全部不变。
- `R-JDB-03`：本升级不增加、删除或重写表、列、index、migration 或 `SCHEMA_VERSION`。
- `R-JDB-04`：repository CRUD、returning clause、r2d2 pool 与 time/JSON round trips 保持现有语义。

## Owner-local Work Packages

### WP-JDB-01：原子升级 Diesel 与 SQLite native owner

1. 在 `F-JDB-01` 同时写入 `diesel = "2.3.12"`、`libsqlite3-sys = "0.38.2"`、`time = "0.3.55"`、`thiserror = "2.0.20"`，保留全部 features 和 cargo-shear ignore。
2. 由 Cargo 更新 root lockfile；运行 inverse tree 验证单一 SQLite links owner，不接受同时解析 0.37/0.38。
3. 仅在 Diesel 2.3.12 API 明确要求时修改 `F-JDB-02`–`F-JDB-04`；不得通过 migration/schema churn 绕过编译或测试失败。

完成条件：atomic pair 与 features 精确落地，`R-JDB-01` 成立，`F-JDB-05` 没有 schema/migration diff。

### WP-JDB-02：验证持久化与三平台 native boundary

1. 运行全量 jaco-db tests，覆盖 bootstrap rollback、legacy validation、catalog、projects、attachments 与 agent persistence。
2. 运行 strict Clippy 和 dependency-tree checks。
3. root CI 在 Windows、macOS、Linux 构建此 crate；Windows 证据必须显示 bundled SQLite 成功链接，其他平台不得出现 duplicate `sqlite3` links。

完成条件：所有持久化测试通过，三平台构建无 native link failure，未产生 data migration。

## Focused Validation 与 handoff

| T-ID | Command/scenario | Expected evidence |
| --- | --- | --- |
| `T-JDB-01` | `cargo test -p jaco-db --all-features --locked` | 全部 schema/repository/migration tests 通过 |
| `T-JDB-02` | `cargo clippy -p jaco-db --all-targets --all-features --locked -- -D warnings` | models、queries、tests 无 warning |
| `T-JDB-03` | `cargo tree -p jaco-db -i libsqlite3-sys@0.38.2 -e features --locked` | 唯一 0.38.2 resolution，包含 `bundled-windows` feature path |
| `T-JDB-04` | root Windows/macOS/Linux CI build | 三平台 SQLite/Diesel link 成功；结果归档到 root plan |

若 schema、migration 或现有数据策略发生任何变化，必须停止并建立独立 DB 设计；该变化不属于本依赖 WP。
