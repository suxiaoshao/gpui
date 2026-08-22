# gpui-tokio 上游替代与本地 crate 退役计划

- 状态：`In progress`（本地退役与 parity 自动化通过；三平台 CI 待执行）
- Owner：`crates/gpui-tokio`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

这是当前 `crates/gpui-tokio` 的**过渡 owner plan**。本地 crate 拥有 GPUI 与 Tokio runtime 的窄 bridge，
但目标 Zed revision 已包含 API/取消语义等价且更完整的 `gpui_tokio` package。本计划的目标不是继续维护本地
bridge，而是在消费者 parity 通过后用上游 package 替代并删除本地 workspace member。

## 上游等价证据与目标图

| Surface | 本地 `crates/gpui-tokio` | Zed target `gpui_tokio 0.1.0` | 决定 |
| --- | --- | --- | --- |
| runtime init | `init` / `init_from_handle` | 同名 API、同样保存 owned runtime/handle | 直接复用 |
| spawn | `Tokio::spawn -> Task<Result<R, JoinError>>` | 同签名、同 result | 直接复用 |
| cancellation | 自写 `AbortOnDrop` | `gpui_util::defer`，GPUI Task drop 时 abort | parity 后删除本地 helper |
| handle | `Tokio::handle` | 同名 API | 直接复用 |
| fallible task | 无 | `Tokio::spawn_result -> Task<anyhow::Result<R>>` | 可用但本批不强制改 consumer |

目标依赖图：

```text
app/jaco ───────┐
                ├─ workspace dependency key gpui-tokio
app/http-client ┘       └─ package = "gpui_tokio"
                            git = canonical Zed source @ e0931d5a...
```

根 dependency key 可保留连字符，以 `package = "gpui_tokio"` 映射上游 underscore package；Rust import 继续是
`gpui_tokio`。不得增加 re-export facade、复制 `spawn_result` 或让本地和上游 package 同时进入 graph。

## 工作包

### TOKIO-RETIRE-1：冻结 parity contract

- 为 owned runtime、external handle、successful join、panic/JoinError 和 GPUI Task drop abort 建立测试。
- Jaco 选取一个 MCP/network path，HTTP Client 选取 request worker 与 timer path，记录替换前结果。
- 不把上游新增 `spawn_result` 作为删除前提，也不改变现有错误类型。

### TOKIO-RETIRE-2：切换 root dependency

- 在 root `[workspace.dependencies]` 将 `gpui-tokio` 改为同 canonical Zed URL 的 package `gpui_tokio`。
- 让 Cargo 与 `gpui` 一起锁到 `e0931d5a...`，确认 `gpui_util`、Tokio features 与 GPUI types 只来自目标图。
- 上游 bridge 只直接开启 Tokio `rt,rt-multi-thread`；逐一确认 Jaco/HTTP Client 在自己的 manifest 声明
  `net/io-util/sync/time/fs` 等真实需求，不借用另一个 member 的 feature union。
- 不改 Jaco/HTTP Client import 或调用点，先编译和运行 parity tests。

### TOKIO-RETIRE-3：删除本地 owner

- parity 通过后，从 workspace members 删除 `crates/gpui-tokio`，并删除该 crate 的 manifest 与 `src/`。
- 保留本 `docs/dev/issue-205` 目录作为 durable retirement record；回填实际命令、结果、最终 package/SHA，
  将状态标记为 `Done`，确保 root owner map 不悬空。
- 对 path edge、member path、自写 `AbortOnDrop` 和本地 package 做 residual scan。
- Jaco/HTTP Client 再跑一次 focused tests、strict Clippy 和 dependency-tree assertion。

## Focused verification

```text
cargo check -p jaco -p http-client --locked
cargo test -p jaco --locked
cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast
cargo clippy -p jaco -p http-client --all-targets --all-features --locked -- -D warnings
cargo tree -p jaco -i gpui_tokio --locked
cargo tree -p http-client -i gpui_tokio --locked
rg -n 'path = "\./crates/gpui-tokio"|"crates/gpui-tokio"' Cargo.toml
```

## 完成条件

- 上游 runtime ownership、completion、panic/JoinError 和 drop-to-abort parity tests 通过。
- Jaco/HTTP Client 的 network/time/cancellation consumer 回归通过，imports 无 compatibility shim。
- workspace 只解析 Zed target 的 `gpui_tokio`，本地 member、path dependency、source 和 `AbortOnDrop` 零残留。
- 本退役计划与 root canonical plan 已回填实际命令、结果、最终 package/SHA 与风险，并保持可导航。
