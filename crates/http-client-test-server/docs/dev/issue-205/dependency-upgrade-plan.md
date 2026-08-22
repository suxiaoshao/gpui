# http-client-test-server：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（本地 HTTP/compression 自动化通过；三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/http-client-test-server`
- Root-owned surfaces consumed：`S-17`、`S-19`
- Owner-local IDs：`F-HTS-01`–`F-HTS-05`、`R-HTS-01`–`R-HTS-03`、`T-HTS-01`–`T-HTS-03`、`WP-HTS-01`–`WP-HTS-02`
- Owns：loopback HTTP/1 test server 的 manifest 更新，以及 framing、compression、stream、abort、shutdown 的 focused 回归。
- Does not own：HTTP Client app 的 consumer 行为、workspace `Cargo.lock`、外网/TLS 测试或 server 契约重设计。

## 精确依赖目标

### Compatible updates

| Dependency | Current | Target | Preserved features | Principal call sites |
| --- | --- | --- | --- | --- |
| `async-compression` | `0.4.42` | `0.4.43` | `default-features = false`; `tokio`, `brotli`, `gzip`, `zlib`, `zstd` | `src/respond.rs:8,228` |
| `base64` | `0.23.0` | `0.23.1` | `default-features = false`; `std` | `src/contract.rs:3`; `src/respond.rs:9`; integration tests |
| `bytes` | `1.12.0` | `1.12.1` | default | body/frame owners in `src/{abort,contract,echo,respond,server}.rs` |
| `futures-util` | `0.3.33` | `0.3.34` | default | `src/{abort,respond}.rs`; `tests/integration.rs` |
| `http-body-util` | `0.1.3` | `0.1.5` | default | boxed/full/stream bodies in `src/{abort,contract,respond,server}.rs` |
| `hyper` | `1.10.1` | `1.11.0` | `http1`, `server` | request/response and HTTP/1 service boundary |
| `thiserror` | `2.0.19` | `2.0.20` | default | typed control/wire/server errors |

### Retained direct dependencies

| Dependency | Retained target | Features/kind |
| --- | --- | --- |
| `hyper-util` | `0.1.20` | `http1`, `server-graceful`, `tokio` |
| `serde` | `1.0.229` | `derive` |
| `serde_json` | `1.0.151` | runtime |
| `tokio` | `1.53.1` | `io-util`, `macros`, `net`, `rt-multi-thread`, `signal`, `sync`, `time` |
| `tokio-util` | `0.7.19` | `rt` |
| `reqwest` | `0.13.4` | dev; `default-features = false`; `rustls`, `stream` |

所有项目继续来自 crates.io；不得新增 Axum/Tower、HTTP/2、TLS server 或改变当前 feature policy。

## Owner-local 目标与文件

```text
crates/http-client-test-server/
├── Cargo.toml                                      # F-HTS-01 [Modify] 上述 7 项 compatible targets
├── src/respond.rs                                  # F-HTS-02 [Verify; edit only if required] codec/stream body
├── src/server.rs + src/abort.rs + src/echo.rs      # F-HTS-03 [Verify; edit only if required] Hyper/body lifecycle
├── src/contract.rs                                 # F-HTS-04 [Verify; edit only if required] Base64/bounded parsing
└── tests/integration.rs                            # F-HTS-05 [Verify] black-box wire and lifecycle coverage
```

- `R-HTS-01`：gzip/br/deflate/zstd、Base64 和 exact body bytes 保持当前 wire behavior。
- `R-HTS-02`：before-head 与 mid-body abort 仍可区分；stream 继续受 Hyper demand 驱动，shutdown 不遗留 task。
- `R-HTS-03`：所有既有 size/header/chunk/delay limits 和 loopback-only policy 保持不变。

## Owner-local Work Packages

### WP-HTS-01：更新 HTTP/body/codec dependency batch

1. 在 `F-HTS-01` 一次性写入 7 个精确 target，完整保留 features/default-feature policy。
2. 由 root owner 使用 Cargo 更新 workspace lockfile；检查没有第二个不必要的 Hyper/body 栈。
3. 仅当 target API 要求时修改 `F-HTS-02`–`F-HTS-04`，采用上游推荐 API；不得改变 Issue #199 已固定的 HTTP contract 来绕过编译错误。

完成条件：manifest 只含列出的版本变化，server public/wire contract 不变，`R-HTS-01`–`R-HTS-03` 有测试证据。

### WP-HTS-02：运行真实 loopback 回归

1. 运行 `T-HTS-01`，覆盖 compression、framing、redirect、echo、两种 abort、并发与 shutdown。
2. 运行严格 Clippy；检查 source 不出现无界 collect、fixed public port 或新增 transport fallback。
3. 若 sandbox 拒绝 loopback bind，以同一命令提权重跑；不替换成外网服务。

完成条件：所有现有 black-box tests 通过，任何 API edit 都有对应 scenario，root plan 记录实际 diff。

## Focused Validation 与 handoff

| T-ID | Command/scenario | Expected evidence |
| --- | --- | --- |
| `T-HTS-01` | `cargo test -p http-client-test-server --all-features --locked` | unit、CLI 和 loopback integration 全部通过 |
| `T-HTS-02` | `cargo clippy -p http-client-test-server --all-targets --all-features --locked -- -D warnings` | server、tests、example 无 warning |
| `T-HTS-03` | `cargo tree -p http-client-test-server --duplicates --locked` | 无本次升级引入的非必要 Hyper/body duplicate；结果回填 root evidence |

HTTP Client app 的 consumer regression、workspace aggregate gates 与 lockfile diff 不在本 owner 重复定义。
