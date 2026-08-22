# xtask：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（本地自动化与四应用 Windows bundle 通过；macOS bundle/CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/xtask`
- Root-owned surfaces consumed：`S-16`、`S-17`、`S-19`
- Owner-local IDs：`F-XT-01`–`F-XT-04`、`R-XT-01`–`R-XT-03`、`T-XT-01`–`T-XT-03`、`WP-XT-01`–`WP-XT-02`
- Owns：xtask CLI/bundler dependency declarations 和 CLI/bundle focused tests。
- Does not own：app-local bundle metadata/assets、CI workflow、workspace lockfile 或 packaging product policy。

## 精确依赖目标

### Compatible updates

| Dependency | Current | Target | Preserved features | Local use |
| --- | --- | --- | --- | --- |
| `clap` | `4.6.4` | `4.6.6` | `derive` | `src/cli.rs`; `src/main.rs` |
| `thiserror` | `2.0.19` | `2.0.20` | default | `src/error.rs` and command/bundle errors |

### Retained direct dependencies

| Dependency | Retained target | Features/kind |
| --- | --- | --- |
| `image` | `0.25.10` | runtime |
| `plist` | `1.10.0` | runtime |
| `serde` | `1.0.229` | `derive` |
| `toml` | `1.1.4` | runtime |
| `tauri-bundler` | `2.9.4` | runtime |
| `tauri-utils` | `2.9.3` | runtime |
| `tracing` | `0.1.44` | runtime |
| `tracing-subscriber` | `0.3.23` | `local-time` |
| `walkdir` | `2.5.0` | runtime |
| `which` | `8.0.5` | runtime |

所有依赖继续来自 crates.io；不改变 bundle backend、命令集合、环境变量或 app manifest schema。

## Owner-local 目标与文件

```text
crates/xtask/
├── Cargo.toml                                      # F-XT-01 [Modify] clap/thiserror compatible targets
├── src/cli.rs + src/main.rs                        # F-XT-02 [Verify; edit only if required] derive/parser/dispatch
├── src/error.rs + src/cmd.rs                       # F-XT-03 [Verify; edit only if required] typed failures
└── src/bundle.rs + src/bundle/**/*.rs              # F-XT-04 [Verify] bundler settings/platform flows
```

- `R-XT-01`：所有现有 subcommand、option、default、help/parse failure 保持兼容。
- `R-XT-02`：bundle settings、artifact discovery/staging 与 platform routing 不变。
- `R-XT-03`：错误 variant 和 exit behavior 不因 thiserror patch 变化。

## Owner-local Work Packages

### WP-XT-01：升级 CLI/error dependencies

1. 在 `F-XT-01` 更新 `clap = "4.6.6"` 与 `thiserror = "2.0.20"`，保留 `derive` 和其余声明。
2. 由 Cargo 更新 root lockfile；预期无需修改 `F-XT-02`–`F-XT-04`。
3. 若 Clap derive/help output 有 semantic diff，逐项记录命令、旧/新输出和兼容结论；不静默更名或删除 option。

### WP-XT-02：验证 CLI 与 bundle helpers

1. 运行 xtask unit suite 和 strict Clippy。
2. 运行 `--help` smoke，确认 parser/dispatch 可启动；不要求在本 owner WP 实际签名或发布 bundle。
3. 将真正三平台 packaging 结果留给 root `S-16` release gate。

完成条件：manifest 仅含两个目标版本变化，CLI/bundle tests 通过，现有命令和 bundle contract 无偏差。

## Focused Validation 与 handoff

| T-ID | Command/scenario | Expected evidence |
| --- | --- | --- |
| `T-XT-01` | `cargo test -p xtask --all-features --locked` | CLI、manifest、bundle helper tests 通过 |
| `T-XT-02` | `cargo clippy -p xtask --all-targets --all-features --locked -- -D warnings` | binary/tests 无 warning |
| `T-XT-03` | `cargo run -p xtask --locked -- --help` | 现有 top-level commands/help 可生成且进程成功退出 |

实际 app bundling、签名与发行验证由 root plan 汇总，不以 help smoke 替代。
