# Issue #199：HTTP Client 受控测试服务接入实施计划

## 状态、范围与执行边界

- 状态：`Done`
- 子任务：`HTTP-199-05`
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- owner：`app/http-client`
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- producer owner：[http-client-test-server 计划](../../../../../crates/http-client-test-server/docs/dev/issue-199/http-test-server-plan.md)
- 前置：`HTTP-199-03` 的 `HttpTransport`、`PreparedRequest`、`WorkerEvent` 与 `RequestProblem` 已交付；
  `C-1800`–`C-1803` 和 `ERR-1800` 起的 producer 契约已可消费。
- 待确认问题：无。
- 实施引用：producer/consumer 主实现提交 `1559cc8`；workspace feature-unification 稳定性修正
  `735bc41`。

本计划只把可由受控 HTTP 语义表达的 `HttpTransport` local-loopback 测试迁移到
`http-client-test-server` 的 dev-dependency。生产代码、请求/响应产品契约、UI、Store、Form、Operation、
媒体/PDF 预览与真实外网验收均不在范围内。

保留 raw TCP fixture 的唯一理由是断言精确 request wire 行为：redirect 每跳实际发出的 method、request body、
自动生成或覆盖的 request header 与重复 header。response 中途截断已由 `C-1802` 产生真实 Hyper body error，
不再保留并行的手写 short-body fixture。测试服务
返回的正常 HTTP 抽象不能替代这些断言；迁移不得降低它们的覆盖力度。

## 已消费的根契约

| 根 ID | HTTP Client consumer 方式 | 本计划约束 |
| --- | --- | --- |
| `C-1800` | 为每个测试创建本机回环 server，并从 handle 取得 base URL 与可等待的确定性 shutdown。 | 不绑定固定端口、不访问外网；每个测试独立拥有并在结束前收束 server。 |
| `C-1801` | 调用受控 response endpoint，配置延迟、status、重复 response headers 与 JSON/base64/body stream。 | 只断言 HTTP Client 的可观察 transport 结果，不在 consumer 重复实现 response encoder 或 framing。 |
| `C-1802` | 调用无 response head 或 body 中途断连的 abort endpoint。 | 依据根 `ERR-1800+` 的 phase 映射断言 `RequestProblemKind`；不得把断连伪装成 HTTP status。 |
| `C-1803` | 调用 echo endpoint，令服务回传 request bytes 与 `Content-Type`。 | 仅用于可由回显证明的 body/content-type 测试；redirect replay 的 wire-level 断言继续使用 raw fixture。 |
| `ERR-1800+` | 接收 server startup、请求解析、受控响应与 abort 行为的 typed failure。 | server/control-plane failure 令测试明确失败；不把 fixture 失败误判为 client `RequestProblem`。 |

## Owner-local 证据与决定

| ID | 分类 | 结论 | 证据 | 后果 |
| --- | --- | --- | --- | --- |
| `E-1810` | 当前事实 | `transport.rs` 测试模块内嵌 `TcpListener`、`FixtureResponse` 与 `read_request`，所有本地服务器仅绑定 `127.0.0.1:0`。 | `app/http-client/src/features/request/transport.rs:88-169` | 用新 crate 取代适合的正常 HTTP fixture，保留 raw helper 至少覆盖 precise-wire 场景。 |
| `E-1811` | 当前事实 | 现有 raw fixture 同时覆盖 redirect replay、body/header wire、head delay、正常 3xx/4xx、HEAD/204 与 `Content-Length` 宣告后截断。 | `transport.rs:207-450` | 不能整体删除 fixture；按测试语义逐项迁移。 |
| `E-1812` | 当前事实 | `HttpTransport::run` 总是以唯一 `WorkerEvent::Finished` 收束；HTTP 3xx/4xx/5xx 保留为正常 response，读 body 失败映射为 typed problem。 | `transport.rs:30-69`；`request-send-and-response-plan.md:R-1602,R-1605` | 新测试必须观测 head/terminal 顺序并区分 response status 与 transport failure。 |

| ID | 决定 | 依据 | 后果 |
| --- | --- | --- | --- |
| `D-1810` | `http-client-test-server` 只作为 `app/http-client` 的 dev-dependency 引入，不进入 `[dependencies]`。 | `E-1810`；本计划非产品功能。 | 打包产物、运行时 dependency graph 与生产 API 不变。 |
| `D-1811` | 迁移 status、head delay、正常空 body、echo 与两种 abort；保留 redirect replay、重复/覆盖 request header 和 multipart/binary per-hop bytes 的 raw TCP fixture。 | `E-1811`；`C-1801`–`C-1803`。 | normal HTTP 与 response abort 场景有共享 fixture，request wire contract 仍直接可见。 |
| `D-1812` | 所有 migrated test 通过 producer handle 的 URL/control API 配置，且必须 wait/await server 的完成或 drop 后关闭。 | `C-1800`；`ERR-1800+`。 | 测试互不占端口、无 detached listener，并把服务端失败暴露给测试。 |

## Owner-local 目标设计

### 文件与责任

```text
app/http-client/
├── Cargo.toml                                      # F-1810 [Modify, handwritten] 增加本地 test-server dev-dependency
├── src/features/request/transport.rs               # F-1811 [Modify, handwritten] 消费 server，保留 raw wire fixture
└── docs/dev/issue-199/http-test-server-integration-plan.md
                                                     # F-1812 [Add, handwritten] 本 owner 的实施、测试与验证记录
```

### L-1810：测试 server 生命周期与现有 transport driver

- **路径与可见性：** `F-1811` 的 `#[cfg(test)] mod tests`；仅测试模块可见。
- **消费者：** 已有 `run_to_terminal`、`run_to_terminal_with_head` 与本计划新增/迁移的 `#[tokio::test]`。
- **契约：** 测试先以 `C-1800` 创建 handle，再将其 base URL 冻结进已有 `PreparedRequest`，随后复用现有
  `HttpTransport::channel` 与 terminal-drain helper。测试结束前 await `shutdown()`；若 producer 报
  `ERR-1804`，测试直接失败。
- **非职责：** 不在 app 测试侧实现路由、HTTP body encoder、计时器、socket close 或 server 业务模型；这些均属于 producer。

### L-1811：raw TCP fixture 边界

- `FixtureResponse`、`fixture` 与 `read_request` 保留在 `F-1811`，仅服务以下 precise-wire 测试：
  1. `post_redirect_rewrites_to_get_and_explicit_headers_override_generated_ones`；
  2. `temporary_redirect_rebuilds_multipart_file_stream_for_each_hop`；
  3. `url_encoded_and_binary_bodies_use_their_frozen_bytes`。
- 其他原先只依赖正常 HTTP status、无 body、等待 response head 或服务端主动断连的测试，改从
  `C-1801`/`C-1802` 构造服务端行为。
- raw helper 不升级为共享 crate，也不混入 producer 私有类型；它继续准确读取 client 实际写出的 bytes。

### ST-1810：测试资源 authority 与顺序

- **Authority：** 每个 `#[tokio::test]` 的局部 server handle 是唯一 listener/control authority；
  `HttpTransport` task 仍是 client I/O authority。
- **初始化：** test 创建 handle，取得 endpoint URL；不共享全局 server、port、request capture 或时间状态。
- **读取与 mutation：** 测试通过 `C-1801`–`C-1803` 提交固定行为，client 仅请求 endpoint；不从 app 修改
  producer 状态。
- **收束：** terminal event 已收到后显式 `shutdown().await`；timeout/cancel 或 assertion panic 时 handle Drop
  只作 best-effort 泄漏保护。任何 server lifecycle failure 作为 test failure 传播。
- **不变量：** response head 一旦被现有 helper 记录，随后的 body failure 仍保留该 head；没有 response head 的
  abort 绝不产生 status assertion。

## 工作包

### WP-1810 ✅：冻结 consumer dependency 与迁移清单

**文件：** `F-1810`、`F-1811`。

1. 在 `app/http-client/Cargo.toml` 的 `[dev-dependencies]` 增加精确的本地 path dependency；不移动到
   production `[dependencies]`，不为 app 增加 server runtime feature。
2. 逐项标记 `transport.rs` raw fixture 的消费者：`D-1811` 列出的 redirect/request-wire 测试保留，其余候选
   使用 producer contract 替换。
3. 由 Cargo 产生仅由该 direct dev-dependency 引起的 `Cargo.lock` 机械变化；审阅没有新生产依赖或无关升级。

**完成条件：** 依赖方向为 `http-client (dev) -> http-client-test-server`，所有 raw fixture 留存理由可在测试名中对应。

### WP-1811 ✅：迁移正常 response、延迟与状态测试

**前置：** `WP-1810`、`C-1800`、`C-1801`。

1. 将 `frozen_total_timeout_covers_waiting_for_the_final_head` 改为 response endpoint 的 header 前延迟；继续断言
   frozen total timeout 得到 `RequestProblemKind::Timeout`。
2. 将 disabled redirect 的 302、404 正常 response，以及 HEAD/204 无 body，迁移为受控 status/header/body
   配置；继续经 `run_to_terminal_with_head` 断言 status、declared/received/stored body sizes 和终态。
3. 增加或迁移一项 echo 测试，验证 text/binary request bytes 与明确 `Content-Type` 由 endpoint 原样回显；不替代
   `D-1811` 保留的 redirect wire 检查。
4. 以 `delay_between_chunks_ms` 覆盖 head-first 时序：先收到 `HeadReceived`，再收到 progress/
   terminal；另一项在 head 后 abort transport task，丢弃 receiver 并显式 shutdown server，证明
   Receiving 阶段取消不留下挂起 connection。页面 loading 状态继续由现有 pure runtime tests 断言。
5. 以小型 JSON/Base64 fixture 分别覆盖 gzip/br/deflate/zstd，并以 caller-supplied 未知
   `Content-Encoding` 覆盖 unsupported-decode；断言 head 保留原始 header，完成 body 是解码后 bytes。
6. 以 `Repeat` 覆盖 8 MiB 以上的 TempFile spill 和 `50 MiB + 1` encoded cap；后者必须是
   `RequestProblemKind::BodyTooLarge { dimension: BodySizeDimension::Encoded, .. }` 且不留下
   partial `CompletedBody`。

**完成条件：** 正常 HTTP status 永不被当作 `RequestProblem`；延迟响应不需要 raw sleep/socket write 实现。

### WP-1812 ✅：迁移真实断连失败测试

**前置：** `WP-1810`、`C-1800`、`C-1802`、`ERR-1800+`。

1. 以 before-head abort 覆盖没有 `ResponseHead` 的 transport failure，断言 terminal error 与根错误 phase 相符。
2. 以 mid-body abort 替换原 `interrupted_body_keeps_the_head_but_never_completes_partial_bytes`：先断言已收到
   status head，再断言 `RequestProblemKind::ResponseBodyRead`，且不会取得 `CompletedBody`。
3. 不用伪造 4xx/5xx 或 local error enum 代替 socket-level abort；服务端控制失败依 `ERR-1800+` 直接使 fixture 失败。

**完成条件：** client transport failure 和正常 HTTP response 的分类由独立、可重复的 server 行为验证。

### WP-1813 ✅：清理测试模块并回填计划证据

**前置：** `WP-1811`、`WP-1812`。

1. 删除只为已迁移 normal HTTP 场景服务的 raw fixture 分支、imports 和响应字节常量；保留 `L-1811` 的最小 raw
   reader/writer 能力。
2. 确认 retained raw tests 与 migrated tests 都等待/释放它们各自的 server task，测试中不存在 fixed port 或外网 URL。
3. 执行本计划的定向验证，将实际命令、测试数与未执行边界回填本文件，
   并只将 owner/root 索引的 `HTTP-199-05` 状态更新为实际结果。

**完成条件：** fixture 分工可从代码和本文档辨识，且没有 unused raw server helper 或重复覆盖同一语义的第二套 fixture。

## 要求与验证映射

| ID | 要求 | 测试/验证 |
| --- | --- | --- |
| `R-1810` | test server 只属于 `http-client` dev graph，生产代码及其 runtime dependency 不变。 | `T-1810` manifest/diff inspection 与定向 Cargo build/test。 |
| `R-1811` | 可表达为正常 HTTP 的 status、delayed head/chunks、HEAD/204、echo、coding 与 large body 场景使用 producer；既有 transport result/head assertions 保持。 | `T-1811` migrated response/echo/timeout/cancel/coding/limit tests。 |
| `R-1812` | before-head 与 mid-body 断连分别保留无 head 和有 head 后 body failure 的可观察语义。 | `T-1812` controlled abort tests。 |
| `R-1813` | redirect replay、重复/覆盖 header 与 multipart/binary per-hop body 仍由 raw fixture 精确断言。 | `T-1813` retained raw TCP tests。 |
| `R-1814` | 每个测试拥有独立 loopback server，正常和失败路径都确定性收束，无固定端口、无外网访问。 | `T-1814` full targeted suite 与 code review of lifecycle paths。 |

| T-ID | 场景 | 断言 |
| --- | --- | --- |
| `T-1810` | 依赖边界 | `Cargo.toml` 仅 `[dev-dependencies]` 引入本地 crate；production source 无 server import。 |
| `T-1811` | controlled response、header/chunk 延迟、echo/coding/large body | 3xx/4xx/5xx 仍为 response；timeout、HEAD/204、head-first progress/cancel、body/content-type 回显、四种解码、spill 与 encoded cap 保持既有 contract。 |
| `T-1812` | controlled before-head / mid-body abort | 前者无 response head；后者保留 head、以 `ResponseBodyRead` 终结且不产生部分 `CompletedBody`。 |
| `T-1813` | retained raw redirect/request wire | 每跳 method/header/body 与重复 headers 仍从 client wire bytes 断言。 |
| `T-1814` | isolated lifecycle | 并行执行定向 tests 时没有 port collision、hanging listener 或 detached fixture failure。 |

## 实施验证

实施后按此顺序执行；仅在 producer `HTTP-199-05` 已先通过其 focused gate 后运行 consumer gate：

```sh
cargo fmt --all -- --check
cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast
cargo check -p http-client --bin http-client --all-features --locked
cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings
git diff --check -- app/http-client/Cargo.toml app/http-client/src/features/request/transport.rs app/http-client/docs/dev/issue-199/http-test-server-integration-plan.md Cargo.lock
```

若 loopback bind 只因 sandbox `PermissionDenied` 失败，对同一条原命令申请提权重跑，不改写
endpoint 或切换到外网 fixture。

验证只覆盖 deterministic local loopback。实际桌面 UI、打包 app、真实 TLS/proxy/external endpoint 不在本计划内；
未实测时必须记录为“未执行”。

## 完成定义与证据

`HTTP-199-05` 的本 owner 部分在 `WP-1810`–`WP-1813`、`R-1810`–`R-1814` 与 `T-1810`–`T-1814`
都有实际证据后完成。完成回填应记录：

- producer contract 的实际版本/commit 与消费的 `C-1800`–`C-1803`、`ERR-1800+`；
- modified files 与 `Cargo.lock` 的直接依赖变化；
- 保留 raw test 和迁移 test 的精确名称；
- 已执行命令、测试数和结果；
- 未执行的 UI、打包和真实网络边界。

不得以编译成功替代受控 abort、head-before-body、timeout、echo 或 raw-wire 断言。

### 完成证据

| 证据 | 实际结果 |
| --- | --- |
| 依赖边界 | `http-client-test-server` 仅加入 `app/http-client` 的 `[dev-dependencies]`；production graph 未引入 server crate |
| 迁移后的受控测试 | timeout、3xx/4xx、HEAD/204、两阶段 abort、echo、delayed chunks、Receiving cancel、四种 coding、unknown coding、8 MiB spill 与 50 MiB cap 均改用 producer |
| 保留的 raw fixture | `post_redirect_rewrites_to_get_and_explicit_headers_override_generated_ones`、`temporary_redirect_rebuilds_multipart_file_stream_for_each_hop`、`url_encoded_and_binary_bodies_use_their_frozen_bytes`，只断言 request wire |
| 环境隔离 | test-only `HttpTransport::new_without_proxy()` 禁用环境代理；production `HttpTransport::new()` 行为保持不变 |
| 实施提交 | producer/consumer 主实现 `1559cc8`；workspace feature-unification 稳定性修正 `735bc41` |
| 聚焦测试 | `cargo test -p http-client --bin http-client --all-features --locked 'features::request::transport::tests::'`：15 passed |
| 完整测试 | `cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast`：161 passed |
| 静态门禁 | `cargo check -p http-client --bin http-client --all-features --locked` 与严格 Clippy 通过；workspace fmt check、diff check 通过 |
| 未执行边界 | 实际桌面 UI、packaged app、真实 TLS/proxy/external endpoint；按用户要求未进行实际 UI 测试 |
