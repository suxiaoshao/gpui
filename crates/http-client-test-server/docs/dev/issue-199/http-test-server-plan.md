# Issue #199：HTTP Client Hyper 测试服务实施计划

## 状态、范围与冻结决定

- 状态：`Done`
- 子任务：`HTTP-199-05`
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- owner：`crates/http-client-test-server`；[owner 索引](README.md)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- 实施引用：producer/consumer 主实现提交 `1559cc8`；workspace feature-unification 稳定性修正
  `735bc41`
- 最后证据刷新：2026-08-14
- 待确认问题：无。

后续补充：`examples/postman_redirect.rs` 复用既有 `/v1/respond`、`Location` 与 `/v1/echo`，启动两个
不同端口的 loopback origin 并输出 302/307 URL，用于在 Postman Console 中观察跨 origin 重定向。
它不增加第四个服务端接口，也不记录或回显认证 header value。

### 目标

交付一个仅监听 loopback、基于 Hyper `1.10.1` HTTP/1.1 connection API 的可复用测试服务。它让
HTTP Client 的自动化测试可稳定构造可延迟、可分块、可编码、可重定向的普通 response，真正没有
response head 的连接中断，已有 response head 后的 body 中断，以及上传 body/`Content-Type` 回显。

### 非目标

- TLS、HTTP/2、HTTP/3、proxy、cookie jar、认证、SSE、WebSocket、请求历史、持久化或外网服务。
- 畸形 HTTP wire framing、伪造无效 `Content-Length`、自定义 TCP reset、吞吐 benchmark 或压力工具。
- 修改 HTTP Client runtime、`PreparedRequest`、Response collector 或其错误种类；本 crate 只提供测试
  producer，consumer 迁移只限其测试 fixture。
- 将实施细节或测试证据堆入 owner/root `README.md`；索引只同步本计划的状态与链接。

### 用户确认的决定

1. 使用直接 Hyper 实现，不引入 Axum 或其他 Web framework。
2. 保留三个主测试接口：`/v1/respond`、`/v1/abort`、`/v1/echo`；另有不属于测试行为的
   `GET /healthz` readiness endpoint。
3. `TestServer::spawn()` 固定绑定 `127.0.0.1:0`；CLI 只有 `--port`，默认端口也是 `0`。
4. `RespondSpec.framing` 提供 `ContentLength`（默认）与 `Chunked`；framing headers 由服务控制。
5. graceful shutdown 最多等待 2 秒，随后 abort 尚未结束的 connection task 并 drain 所有 join result。
6. `GET /healthz` 固定返回 `200`、`text/plain; charset=utf-8` 与 `ok\n`；CLI 就绪行固定为
   `HTTP_CLIENT_TEST_SERVER=http://127.0.0.1:<port>`。

### 兼容性与迁移

这是 `publish = false` 的新 workspace test-support crate，没有已发布 Rust API、持久化数据、配置文件或
跨版本 rollout。新增 public API 可以在 Issue #199 分支内按本计划一次完成；不得为未存在的旧 API 增加
compatibility shim。当前 workspace/lockfile 的未提交 crate skeleton 是本计划输入，实施不得回退它。

## 系统面适用性

| S-ID | 系统面 | 状态 | 当前证据 | 目标决定 |
| --- | --- | --- | --- | --- |
| S-01 | Workspace、文件、模块与 owner 边界 | 适用 | crate 的 manifest、library、CLI 与 integration tests 已落地 | 测试服务器所有 runtime/contract 留在此 crate，HTTP Client 仅通过 dev-dependency 消费 |
| S-02 | GPUI 组件、布局、交互与可访问性 | 不适用 | crate 不链接 GPUI | 无 UI |
| S-03 | Entity、Store、Global、identity 与 projection | 不适用 | crate 不链接 GPUI | 无 GPUI state |
| S-04 | Action、event、subscription、focus 与 window | 不适用 | crate 不链接 GPUI | 无 action/window |
| S-05 | 异步 task、并发、取消与 shutdown | 适用 | `HttpTransport` 测试现以临时 TCP task 建 fixture | `TestServer` 拥有 accept loop 与 connection `JoinSet`，明确 2 秒 shutdown |
| S-06 | 数据获取与 Operation 状态 | 不适用 | 服务不拥有业务 Operation | 不引入 `gpui-operation` |
| S-07 | Form 与可编辑状态 | 不适用 | 无 GPUI form | JSON 仅是 test-control wire format |
| S-08 | 跨 crate、provider、Rig、MCP、平台与外部契约 | 适用 | HTTP Client 现有 loopback tests 直接写 TCP response | 本 crate 定义 C-1800--C-1803 的 HTTP/test Rust contract |
| S-09 | 错误 identity、传播与错误 UI | 适用 | HTTP Client 已区分 transport/body-read 等错误 | service control error 与两种刻意 abort 语义固定为 ERR-1800--ERR-1804 |
| S-10 | 数据库、持久化与 migration | 不适用 | 无数据库 | 无持久化 |
| S-11 | 生成、同步、复制或 vendored 内容 | 无变化 | 无 generator | 仅 Cargo 解析 `Cargo.lock` |
| S-12 | 图标与 asset | 不适用 | 无 assets | 无变化 |
| S-13 | Fluent i18n 与 bundle 本地化 | 不适用 | 无用户界面 | 控制错误为稳定 machine code，不加 locale |
| S-14 | 安全、隐私与凭据 | 适用 | 服务用于本机测试 | loopback-only、大小/并发/时长限制，错误不回显用户 data |
| S-15 | 可观测性与诊断 | 适用 | 现有 client 对 source 做 redaction | crate 不记录 request body、header value 或 spec；CLI 仅打印 bind URL |
| S-16 | 打包、平台行为与 CI/release | 适用 | workspace CI 覆盖三平台 | 纯 Rust/loopback；不修改 bundle/CI；CLI 的 Unix SIGINT 由 black-box test 验证 |
| S-17 | 依赖、framework、Git source 与 toolchain | 适用 | lockfile 已有 Hyper 1.10.1 等 transitive resolution | 增加精确 direct dependency，HTTP/1-only feature set |
| S-18 | Owner 文档、索引与 ADR | 适用 | 本文是 owner plan 与完成证据 | owner/root 索引只登记 `HTTP-199-05 Done`、结果摘要与本文链接 |
| S-19 | 验证与完成证据 | 适用 | producer 15 tests 与 consumer transport 15 tests 已通过 | R-1800--R-1809 映射到定向 crate/client 测试 |

## 证据与决策

| E-ID | 分类 | 结论 | 证据 | 计划后果 |
| --- | --- | --- | --- | --- |
| E-1800 | 实施前事实 | crate skeleton 已登记为 workspace member，但 manifest 没有直接依赖，lib 只有说明注释 | 实施前的 `Cargo.toml` 与 `crates/http-client-test-server/{Cargo.toml,src/lib.rs}` | WP-1800 已建立实际模块/manifest |
| E-1801 | 当前事实 | HTTP Client 禁用 Reqwest 自动 redirect/referer/content decoding，并手工处理最终 response | `app/http-client/src/features/request/transport.rs` | respond 必须能测 redirect 与四种 content coding |
| E-1802 | 当前事实 | worker 在最终 response head 后才流式收集 body | `app/http-client/src/features/request/transport/worker.rs` | 两种 abort 需精确区分有无 head |
| E-1803 | 当前事实 | client 已区分 `Transport` 与 `ResponseBodyRead`，且不接受截断 partial body | `app/http-client/src/features/request/runtime.rs`；`transport.rs` interrupted-body test | C-1802 成为现有错误语义的真实 fixture |
| E-1804 | 当前事实 | client capture encoded/stored 上限是 50 MiB，memory spill 是 8 MiB | `app/http-client/src/features/request/response/` 与 `request-send-and-response-plan.md` | `Repeat` 必须能以有界 server memory 发出 `50 MiB + 1` |
| E-1805 | 上游事实 | Hyper 1.10.1 的 HTTP/1 `serve_connection` service error 可终止 connection；response body error 可终止已开始的 body | 本机 Cargo registry 中 `hyper-1.10.1` HTTP/1 server API | before-head 在 service boundary 返回 error；mid-body 在 body poll 返回 error |
| E-1806 | 上游事实 | `http-body-util 0.1.3` 与 `futures-util 0.3.33` 可构造按需拉取的 HTTP body stream | 本机 Cargo registry | 普通 response 不以无界 producer task 缓冲 body |
| E-1807 | 用户决定 | Hyper、三个 endpoint、固定 spawn/CLI/shutdown/framing 契约 | 本轮会话 | 无待确认产品选择 |

| D-ID | 决定 | 依据 | 后果 |
| --- | --- | --- | --- |
| D-1800 | 只启用 Hyper `server` + `http1` 和 hyper-util `server-graceful` + `http1` + `tokio` | E-1805、E-1807 | 不侦听 h2c，不引入 Axum/Tower router |
| D-1801 | `respond` 的任意 method、`abort` 的标准非 CONNECT method 只要带 `?spec=` 就消费 query control；只有不带 `spec` 的 `POST` 消费 JSON control body | E-1807、HTTP method 语义 | 同一 endpoint 可验证 method/redirect；`HEAD` 只支持 `BeforeHead`，因为 `MidBody` 无可发送的 response body；POST 仍能承载大 control payload |
| D-1802 | 普通 response 默认明确 `Content-Length`；请求 `framing: chunked` 时不写该 header | E-1804、E-1807 | 控制 header 不得写 framing，两个 client path 都可测试 |
| D-1803 | duplicate response headers 使用数组并逐项 append | E-1801 | `Set-Cookie` 等不被折叠 |
| D-1804 | 正常 body 使用 demand-driven stream；不为 response body spawn producer task | E-1806 | 客户端背压限制 server allocation/生成速度 |
| D-1805 | before-head 使用 service error；mid-body 使用 body error；`HEAD + MidBody` 在执行前稳定拒绝 | E-1802、E-1805 | 两个可执行 phase 有真实 TCP 语义，非伪造 HTTP status |
| D-1806 | echo 有独立 64 MiB input cap，超限不回显 partial payload | E-1804、E-1807 | 支持 50 MiB upload/collector tests，同时有资源上限 |
| D-1807 | `TestServer` 的 accept loop 持有所有 connection task，先停止 accept 并驱动 Hyper graceful shutdown，2 秒后才 abort/drain | E-1807 | 没有 detached listener/connection lifecycle |
| D-1808 | CLI 只解析 `--port <u16>`，固定 host `127.0.0.1` | E-1807 | 不允许意外暴露到 LAN |
| D-1809 | control errors 只返回固定 code | E-1807 | 不泄露 spec、body、headers 或 URL |

上游 API 权威入口：

- [Hyper 1.10.1 HTTP/1 server connection](https://docs.rs/hyper/1.10.1/hyper/server/conn/http1/)
- [Hyper 1.10.1 service](https://docs.rs/hyper/1.10.1/hyper/service/)
- [hyper-util 0.1.20 graceful server](https://docs.rs/hyper-util/0.1.20/hyper_util/server/graceful/)
- [hyper-util 0.1.20 TokioIo](https://docs.rs/hyper-util/0.1.20/hyper_util/rt/tokio/)
- [http-body-util 0.1.3 StreamBody](https://docs.rs/http-body-util/0.1.3/http_body_util/struct.StreamBody.html)

## 目标设计

### 文件与 owner 边界

```text
crates/http-client-test-server/
├── Cargo.toml                         [F-1800 Modify] 精确直接依赖
├── src/
│   ├── lib.rs                          [F-1801 Modify] public 契约/重导出
│   ├── contract.rs                     [F-1802 Add] JSON spec、限制、校验/错误码
│   ├── server.rs                       [F-1803 Add] TCP accept、Hyper HTTP/1 service、shutdown
│   ├── respond.rs                      [F-1804 Add] response 构造、编码、按需流
│   ├── abort.rs                        [F-1805 Add] connection-level abort body
│   ├── echo.rs                         [F-1806 Add] 有界 request collector/echo response
│   └── main.rs                         [F-1807 Add] `--port`、readiness 输出、Ctrl-C
├── tests/integration.rs                [F-1808 Add] black-box Hyper/Reqwest/raw-TCP 覆盖
└── examples/postman_redirect.rs        [F-1809 Add] 两个 loopback origin 的 302/307 手工对照夹具
```

`F-1800` 的 manifest 变更由 Cargo 机械更新 workspace `Cargo.lock`。本计划与 owner/root 索引是状态与
证据文档，不作为另一组生产源码 F-ID。

`server.rs` owns TCP/Hyper lifecycle，`contract.rs` 是 control schema 的唯一事实来源；`respond.rs`、
`abort.rs`、`echo.rs` 不创建 listener 或长存 task。`main.rs` 复用 library 的 listener/service API，不能复制
route 或 shutdown 逻辑。所有路径均为手写 source，无生成/同步产物。

### L-1800：公开 Rust 契约

```rust,ignore
pub struct TestServer { /* private base_url, cancellation token, AbortOnDropHandle */ }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerErrorKind { Bind, Accept, TaskPanicked, Shutdown }

pub struct ServerError { /* private kind only */ }

impl ServerError {
    pub fn kind(&self) -> ServerErrorKind;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecUrlError { TooLarge }

impl TestServer {
    pub async fn spawn() -> Result<Self, ServerError>;
    pub fn base_url(&self) -> &str;
    pub fn respond_url(&self, spec: &RespondSpec) -> Result<String, SpecUrlError>;
    pub fn abort_url(&self, spec: &AbortSpec) -> Result<String, SpecUrlError>;
    pub async fn shutdown(self) -> Result<(), ServerError>;
}
```

- `spawn()` 只调用 `TcpListener::bind("127.0.0.1:0")`，成功返回前已取得 `local_addr`，所以不需要轮询
  readiness；`base_url()` 精确为 `http://127.0.0.1:<actual-port>`。
- `respond_url`/`abort_url` 将 JSON UTF-8 用 URL-safe unpadded Base64 放入 `spec` query value；序列化
  JSON 或编码后 query value 任一超过 8 KiB 就返回 `SpecUrlError::TooLarge`，测试改用
  POST control endpoint。
- `shutdown(self)` 停止 accept，驱动所有已接受 connection 的 graceful shutdown，等待最多 2 秒；
  逾时后 `abort_all()` 并 await/drain `JoinSet`。`ServerError` 只保留 bind/shutdown/join 阶段与稳定 kind，
  `Debug`/`Display` 不含 request detail 或底层 cause。`Drop` 只发出 best-effort cancellation，不承诺 await。
- `TestServer` 不实现 `Clone`；测试必须显式 `shutdown().await`。accept-loop handle 由
  `tokio_util::task::AbortOnDropHandle` 包装，Drop 路径先 cancel，再 abort 还未退出的 owner task，
  只用于 assertion panic 等提前 unwind 的泄漏保护。

### L-1801：control schema、body 与 framing

```rust,ignore
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RespondSpec {
    #[serde(default = "status_200")]
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<HeaderSpec>,
    #[serde(default)]
    pub body: ResponseBodySpec,
    #[serde(default)]
    pub delay_before_headers_ms: u32,
    #[serde(default = "chunk_16_kib")]
    pub chunk_size_bytes: u32,
    #[serde(default)]
    pub delay_between_chunks_ms: u32,
    #[serde(default)]
    pub content_encoding: Option<ContentEncoding>,
    #[serde(default)]
    pub framing: ResponseFraming,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderSpec { pub name: String, pub value: String }

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResponseBodySpec {
    #[default]
    Empty,
    Json { value: serde_json::Value },
    Base64 { value: String },
    Repeat { byte: u8, len: u64 },
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFraming { #[default] ContentLength, Chunked }

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentEncoding { Gzip, Br, Deflate, Zstd }
```

`RespondSpec`、`AbortSpec`、`HeaderSpec` 与 body variant 只能实现手写脱敏 `Debug`；可输出 status、
header 数量、body kind/长度和 delay，不得输出 header value、JSON/Base64 内容或 URL query。

- `status` 只接受 `200..=599`。`204`、`205`、`304` 仅可配 `Empty` 且不可编码；`HEAD` request 不写 body，
  `ContentLength` 仍为若按同 spec 发 GET 时的长度。
- `Json` 使用 `serde_json::to_vec` 的精确 bytes，**不自动添加 `Content-Type`**；请求者通过 `headers` 控制。
  `Base64` 用标准 Base64 解码；`Repeat` 逐 chunk 生成 `byte`，不 materialize 整个 body。
  四个 body variant 都不自动补 `Content-Type`，以便测试缺失或与内容不匹配的头部。
- `headers` 最多 128 项，每项 name 与 value 合计最多 8 KiB，逐项 `HeaderMap::append`。禁止 control 写入 `content-length`、`transfer-encoding`、
  `connection`、`keep-alive`、`upgrade`、`trailer`；`ContentLength` 由实际最终 encoded byte length 写入，
  `Chunked` 不写 `Content-Length`，交给 Hyper 产生 HTTP/1 chunk framing。
- `content_encoding` 先编码 body、再添加唯一 `Content-Encoding`；若 caller header 已含该字段，control 请求
  以 `conflicting_content_encoding` 拒绝。若不设 `content_encoding`，caller 可显式写未知 coding，测试 client
  的 unsupported path。
- `Location` 是普通可控 response header；因此 `301/302/303/307/308` 与另一 `respond_url` 可组成
  redirect source/final target，不另设 redirect endpoint。

### L-1802：abort 与 echo schema

```rust,ignore
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "phase", rename_all = "kebab-case")]
pub enum AbortSpec {
    BeforeHead,
    MidBody {
        #[serde(default = "sixteen")]
        bytes_before_abort: u32,
        #[serde(default = "chunk_16_kib")]
        chunk_size_bytes: u32,
        #[serde(default)]
        delay_between_chunks_ms: u32,
    },
}
```

- `/v1/abort?spec=` 接受 `GET/HEAD/POST/PUT/DELETE/OPTIONS/PATCH/TRACE`，不带 `spec` query 的
  `POST /v1/abort` 解析同一个 `AbortSpec`。`CONNECT` 与自定义 method 返回带精确 `Allow` 的 `405`。
- `HEAD + BeforeHead` 仍执行真实 head 前中断；`HEAD + MidBody` 因协议不会传送 response body，稳定返回
  `400 invalid_request`。其余有效 method/phase 不转换成 HTTP error response。
- `BeforeHead` 在 `hyper::service::service_fn` 返回 `Err(AbortBeforeHead)`；不构造 `Response`，因此 wire
  上没有 status line/header/body。
- `MidBody` 固定发送 `200 OK`、`Content-Type: application/octet-stream`、
  `Content-Length: bytes_before_abort + 1`；pull body 发送精确 prefix 后返回 body error，Hyper drop TCP
  connection。`bytes_before_abort` 为 `1..=64 KiB`，所以该 phase 总有实际 partial data。
- `ANY /v1/echo` 流式收集 request data，随后 `200` 回显精确 bytes，逐个 copy request `Content-Type`
  values；没有该 header 时添加 `application/octet-stream`。它不回显其他 request headers。

### L-1803：路由与稳定 control failure response

| Method/path | 输入 | 成功 | 失败 |
| --- | --- | --- | --- |
| `GET /healthz` | 无 | `200`, `text/plain; charset=utf-8`, `ok\n` | 其他 method 为 `405` |
| `ANY /v1/respond?spec=` | encoded/decoded spec 各自 ≤8 KiB | C-1801 response | ERR-1800/1801 JSON response |
| `POST /v1/respond` without `spec` | ≤24 MiB JSON | 同 query control | ERR-1800/1801 JSON response |
| 标准非 CONNECT method `/v1/abort?spec=` 或 `POST` body | C-1802；`HEAD` 仅允许 `BeforeHead` | 实际 connection interruption | 无效 control/method-phase 为 ERR-1800/1801，unsupported method 为 `405` |
| `ANY /v1/echo` | body ≤64 MiB | C-1803 echo | ERR-1801 JSON response |

```json
{ "code": "invalid_request" }
```

control failure 的 body 仅含 `code`，`Content-Type: application/json`；ERR-1800 使用 `400`，
ERR-1801 使用 `413`。合法 code 只有
`invalid_request`、`invalid_status`、`invalid_header`、`restricted_header`、
`conflicting_content_encoding`、`limit_exceeded`、`request_body_too_large`。未知 path 为 404，已知 path
错误 method 为 405 并携带正确 `Allow`；两者都是 `Content-Length: 0` 的空 body，不含
user input。

query control 只接受唯一 `spec` 键；重复 `spec`、其他 query key 或空值返回
`invalid_request`。存在 `spec` 时，request body 是被测 client 的业务载荷，服务不把它解析为
control JSON；`/v1/respond?spec=` 在 64 MiB 上限内逐 frame 丢弃该 body 后再构造 response，
只有缺少 `spec` 的 POST 才读取 control body。

### ST-1800：listener、connection 与 shutdown

1. `spawn` 或 CLI 先 bind loopback TCP listener，再创建 shared `CancellationToken`、
   `hyper_util::server::graceful::GracefulShutdown` 和 accept-loop task。
2. accept loop 使用容量 64 的 semaphore；无 permit 的 accepted socket 直接 drop。获得 permit 的 socket
   进入同一个 `JoinSet`，每个 task 运行被 graceful tracker 监管的
   `hyper::server::conn::http1::Builder::keep_alive(false).serve_connection(TokioIo, service)`。每个测试 request
   使用单一 connection，query-control request 可不缓冲其业务 body。
3. cancellation token 让 header delay、chunk delay 和 response producer 及时停止；connection future 由 Hyper
   graceful shutdown 结束，不在收到 shutdown 的同一瞬间直接丢弃所有 future。
4. `shutdown` 取出 `AbortOnDropHandle` 的 inner handle，先停止 accept，再触发 graceful shutdown 并
   poll `JoinSet`；2 秒后对剩余 task `abort_all`，再 drain 全部 join result，最后返回。

### ST-1801：respond 流与背压

1. decode/serialize/validate spec；query-control respond 先有界 drain 其 request body；随后
   `delay_before_headers_ms` 以 cancellation-aware sleep 完成，再构造 response。
2. `ContentLength` 写实际 encoded length；`Chunked` 不写该 header。header 写完才由 Hyper poll body。
3. normal `ResponseBody` 每次 `poll_frame` 最多生成一个 `chunk_size_bytes` data frame；两帧间才 poll delay。
   Hyper/TCP 不能写入时不 poll 下一帧，因而没有 server-side queued body。
4. `Repeat` 只分配当前 ≤64 KiB chunk；JSON/Base64 的 decoded source 与 compression temporary buffer 留在
   request-local owner，response EOF/drop 后释放。

### ST-1802：实际 abort 时序

- C-1802 before-head：request head 已由 Hyper 解析，service future error 使 connection closed；client
  `send()` 无 `ResponseHead`，应映射现有 HTTP Client `Transport`。
- C-1802 mid-body：response head 已被 Hyper 写入且至少一个 data frame 已 emitted，下一 `poll_frame` error
  使 connection closed；client 收 body 时失败，应映射现有 `ResponseBodyRead`，不得产生 `StoredBody`。
- 这两个故障属于 `ERR-1802`/`ERR-1803`；它们不含 status code error payload，也不自动 retry。

### ST-1803：echo 收集

读取 `Incoming` frame 时每个 data frame 先 checked-add 到 64 MiB counter，再 append；任何 declared
`Content-Length > 64 MiB` 可在读 body 前拒绝，未知长度在越界 frame 前拒绝。成功时 service 才构造
`Full<Bytes>` echo response 和服务生成的 content length；失败时丢弃已收集 buffer，不回显 partial bytes。
control JSON 与 echo 都必须逐 frame 检查上限；不得对未受信任的 `Incoming` 直接调用无上限
`BodyExt::collect()`。

### ST-1804：CLI

CLI 只接受零个参数或 `--port <u16>`；其它参数向 stderr 写 usage 并以 exit 2 退出。bind 后 stdout 精确写一行
`HTTP_CLIENT_TEST_SERVER=http://127.0.0.1:<port>` 并 flush，随后等待 Ctrl-C；收到 signal 后调用
与 `TestServer::shutdown` 等价的 2 秒 cancellation/drain sequence。CLI 不接受 host、远程地址、spec 或 secret。
library 以 `#[doc(hidden)] pub async fn run_cli(port: u16) -> Result<(), ServerError>` 作为 binary 跨 crate
边界，绑定、readiness 输出、signal 与 shutdown 实现仍只在 library 一处；`main.rs` 只解析参数和
选择 exit code。该 doc-hidden 函数不作为 HTTP Client consumer API。

## 跨 owner 契约

| C-ID | 方向/机制 | 权威定义 | Producer/consumer | 兼容性 | ERR |
| --- | --- | --- | --- | --- | --- |
| C-1800 | test -> crate Rust API | `src/lib.rs:TestServer` | crate -> HTTP Client integration tests | 新增 | ERR-1804 |
| C-1801 | test client -> HTTP/1 | `contract.rs:RespondSpec` 与 `/v1/respond` | crate -> HTTP Client transport tests | 新增 | ERR-1800/1801 |
| C-1802 | test client -> HTTP/1 connection lifecycle | `abort.rs:AbortSpec` 与 `/v1/abort` | crate -> HTTP Client transport tests | 新增 | ERR-1800/1802/1803 |
| C-1803 | test client -> HTTP/1 request/response body | `echo.rs` 与 `/v1/echo` | crate -> HTTP Client body tests | 新增 | ERR-1800/1801 |

| ERR-ID | 分类 | 精确触发/安全细节 | 恢复与测试含义 |
| --- | --- | --- | --- |
| ERR-1800 | control 校验 | JSON/Base64/spec 格式错误，或 status/header/framing/coding 无效；`400` 且 response 只含稳定 `code` | 修正 test fixture；不回显输入 |
| ERR-1801 | 资源上限 | control/body/header/chunk/delay 超过 R-1801--R-1805，或 echo request 超过 64 MiB；`413` + 稳定 code | 测试缩小输入；不回显 partial body |
| ERR-1802 | head 前 abort | 有效 `AbortSpec::BeforeHead`；service error 在零 response bytes 时结束 connection | client 必须观测到 transport failure 且无 head |
| ERR-1803 | body 中 abort | 有效 `AbortSpec::MidBody`；写出 prefix 后 body error，声明长度还差一字节 | client 必须保留 head 并归类 body read failure |
| ERR-1804 | server lifecycle | loopback bind、accept task join 或 shutdown drain 失败；只暴露稳定 phase/kind | test fixture 直接失败，不映射为 HTTP Client `RequestProblem` |

## 限制、依赖策略与安全

| R-ID | 不变量 |
| --- | --- |
| R-1800 | Listener 只绑定 `127.0.0.1`；CLI 不能覆盖 host。 |
| R-1801 | GET control spec 的 encoded query value 与 decoded JSON 各自 ≤8 KiB；POST control JSON ≤24 MiB。 |
| R-1802 | JSON/Base64 decoded source ≤16 MiB；`Repeat.len` ≤64 MiB；压缩输入 ≤16 MiB，压缩临时输出 ≤18 MiB。 |
| R-1803 | chunk 为 1 B--64 KiB，response 最多 4096 chunks，head 前延迟 ≤30 s，每 chunk 延迟 ≤1 s，配置的总流延迟 ≤60 s。 |
| R-1804 | 最多 64 个 active connections；普通 body producer 不得超过 Hyper demand 预排 chunk。 |
| R-1805 | Control 最多 128 个 headers，每个 name+value ≤8 KiB；不得伪造 framing/connection headers，只由 `framing` 选择 content length 或 chunked wire。 |
| R-1806 | Control error/log/CLI 不得泄漏 request body、header value、URL query 或 source error text。 |
| R-1807 | `shutdown` 最多等待两秒后 abort/drain；owner shutdown 后没有 detached connection task 存活。 |
| R-1808 | Before-head 写出零 response bytes；mid-body 写出 status/head/prefix 后不存在完整 body completion。 |
| R-1809 | Echo 只在完成有界收集后精确回显；保留 Content-Type values，不保留其他 request metadata。 |

`Cargo.toml` 只增加以下精确 direct dependencies；均已在当前 lockfile resolution 中存在，不变更 toolchain、Git
source 或 workspace patch：

```toml
async-compression = { version = "0.4.42", default-features = false, features = ["tokio", "brotli", "gzip", "zlib", "zstd"] }
base64 = { version = "0.23.0", default-features = false, features = ["std"] }
bytes = "1.12.0"
futures-util = "0.3.33"
http-body-util = "0.1.3"
hyper = { version = "1.10.1", features = ["http1", "server"] }
hyper-util = { version = "0.1.20", features = ["http1", "server-graceful", "tokio"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.19"
tokio = { version = "1.53.1", features = ["io-util", "macros", "net", "rt-multi-thread", "signal", "sync", "time"] }
tokio-util = { version = "0.7.19", features = ["rt"] }
```

black-box tests 另在 `[dev-dependencies]` 使用已锁定的
`reqwest = { version = "0.13.4", default-features = false, features = ["rustls", "stream"] }`；它只是
test client，不参与 server production path。不增加 Axum、Tower、Clap 或直接 `http` dependency，
HTTP types 统一消费 Hyper re-export。

`async-compression` 仅实现 C-1801 的 four coding，不用于 client behavior。若 codec output 超过 18 MiB，
在发送 head 前报告 `limit_exceeded`；私有 bounded writer 在每次 append 前检查上限，不得通过全量
unbounded `Vec` 或临时文件绕过该限制。

## 工作包

### WP-1800 ✅：冻结 manifest、public contract 与 validation

**文件：** F-1800--F-1802。
**前置：** D-1800--D-1803，R-1801--R-1806。

1. 增加精确 dependency/feature declarations，并由 Cargo 更新 lockfile。
2. 定义 `RespondSpec`、`AbortSpec`、`HeaderSpec`、framing/coding/body enum、`SpecUrlError` 与 stable control
   code；所有 schema 使用 `deny_unknown_fields`。
3. 实现 shared bounded JSON/body reader 和 validation；在 header conversion 前拒绝 restricted names，并在
   response construction 前验证 length/delay/chunk/status/body invariants。

**完成条件：** 任何 handler 均只消费已验证 typed spec；无 handler 自行解析 JSON 或自由写 framing header。

### WP-1801 ✅：Hyper HTTP/1 server 与 managed lifecycle

**文件：** F-1801、F-1803。
**前置：** WP-1800，D-1800，D-1807--D-1808，R-1800/R-1804/R-1807。

1. 建 `TcpListener` accept loop、64 permit、`JoinSet`、`TokioIo` 和 HTTP/1 builder；根据 method/path dispatch。
2. 实现 `TestServer::spawn/base_url/shutdown` 及 healthz。所有 normal response body type 统一为 boxed
   Hyper body，service error 能保留给 abort path。
3. 实现 shared cancellation + 2-second graceful, then abort/drain sequence；`Drop` 不 await。

**完成条件：** test 可顺序/并行 spawn 多个 server，explicit shutdown 后 accept-loop 与 connection join 均结束。

### WP-1802 ✅：可控普通 response、framing 与 backpressure

**文件：** F-1804。
**前置：** WP-1800--WP-1801，C-1801，R-1801--R-1805。

1. 实现 ANY-method query 与 POST-without-query JSON 的 C-1801 parser，及 `respond_url` 8 KiB guard。
2. 逐项 append response headers，生成 status、exact content length 或 chunked stream；Json 不补 Content-Type。
3. 实现 JSON/Base64/Repeat 与 cancellation-aware delayed pull stream；实现 gzip/br/deflate/zstd encoding。

**完成条件：** 发送速度由 body poll 决定；duplicate headers、Location、status、body bytes、framing 均可断言。

### WP-1803 ✅：两个真实 abort phase

**文件：** F-1805。
**前置：** WP-1800--WP-1801，C-1802，ERR-1802--ERR-1803，R-1808。

1. 在 service boundary 解析 valid abort spec 后，before-head 返回 typed service error。
2. 实现返回 prefix 后 `poll_frame -> Err` 的 bounded `MidBody`，不得把 error 翻译为 normal response。
3. `HEAD + BeforeHead` 保持真实中断；`HEAD + MidBody` 在写 head 前以 `invalid_request` 拒绝。
4. 确认 server cancellation/drop 在任一 phase 均关闭 stream/socket。

**完成条件：** raw TCP tests 可区分零 response bytes 与 head-plus-prefix 的 wire output。

### WP-1804 ✅：bounded echo

**文件：** F-1806。
**前置：** WP-1801，C-1803，ERR-1801，R-1806/R-1809。

1. 逐 frame 收集 incoming data，实行 declared/observed 64 MiB guard。
2. 成功时以 `Full<Bytes>` 和 generated length 回显，并复制所有 `Content-Type` values；无值时使用
   `application/octet-stream`。
3. 越界/parse failure 丢弃 buffer，返回 stable 413/400 JSON，绝不回显 partial input。

**完成条件：** binary、multipart、无 Content-Type 和 large upload 都有确定语义。

### WP-1805 ✅：CLI

**文件：** F-1807。
**前置：** WP-1801，ST-1804。

1. 只实现 `--port` parser、usage 和 loopback bind。
2. stdout 打印实际 URL 后 flush；Ctrl-C 调用 shared shutdown path。

**完成条件：** `cargo run -p http-client-test-server -- --port 0` 可被 harness 读取实际 URL，并能 Ctrl-C 有界退出。

### WP-1806 ✅：crate black-box tests

**文件：** F-1808。
**前置：** WP-1800--WP-1805，T-1800--T-1807。

实现 server crate 的 TCP/Hyper/Reqwest integration tests，所有 test 使用 `TestServer::spawn()` 与 explicit
`shutdown()`；不依赖固定端口或 sleep-only cleanup。

### WP-1807 ✅：producer-ready 交接

**文件：** F-1800--F-1808，本 owner plan/index。
**前置：** WP-1806，C-1800--C-1803。

1. 确认 public spec/helper/error API 与 HTTP endpoints 已经由 black-box tests 锁定，没有 consumer 需要
   读取的 private state 或测试专用 bypass。
2. 在 [HTTP Client consumer 计划](../../../../../app/http-client/docs/dev/issue-199/http-test-server-integration-plan.md)
   登记实际可消费的 C/ERR 契约；不从 producer 直接修改 app 文件。

**完成条件：** `C-1800`–`C-1803` 达到 `producer-ready`；consumer 可只通过 public API/HTTP 契约开始实施。

### WP-1808 ✅：格式、focused gates 与残留扫描

**前置：** WP-1806--WP-1807，T-1800--T-1807。

执行本 producer 计划 Validation 表的最小充分命令；检查没有 Axum、HTTP/2 listener、固定端口、detached connection
task、手写 framing header 或 unrestricted buffer 的残留。

### WP-1809 ✅：完成回填

**前置：** WP-1808。

实际文件、Cargo.lock diff、命令结果、CLI SIGINT 结果、未执行边界和 owner/index 状态已回填；producer 与
consumer 均完成后，根 `HTTP-199-05` 已标为 `Done`。

## 测试矩阵与验证

| T-ID | R-ID | 层级/场景 | Assertions |
| --- | --- | --- | --- |
| T-1800 | R-1800/R-1807 | `TestServer::spawn`、healthz、explicit shutdown、CLI parser/readiness | port 非固定；healthz 精确 body/type；shutdown 在 deadline 内 join/拒绝新连接；CLI 坏参数 exit 2 且就绪行拼写固定 |
| T-1801 | R-1801/R-1805 | GET encoded/decoded 各 8 KiB cap、POST 24 MiB boundary、invalid JSON/status/header | 可到达的 encoded 边界成功；decoded 或 encoded 任一超限、invalid/restricted header 均返回 stable code；无 reflected value |
| T-1802 | R-1802/R-1803 | Json/Base64/Repeat、duplicate `Set-Cookie`、status 404、ContentLength/Chunked | exact bytes/values；Json 无 automatic Content-Type；repeat 只按 chunk allocation |
| T-1803 | R-1803/R-1804 | before-head delay、per-chunk delay、slow body reader、64-connection saturation | head 延后；next frame 不早于 delay；server 不预排完整 body；超出 permit 的新 socket 关闭且 shutdown 能收束全部 task |
| T-1804 | R-1802/R-1805 | gzip/br/deflate/zstd、caller unknown coding、Location redirect target | 测试 client 显式关闭自动 content decoding；encoded bytes/header 正确；coding conflict rejected；redirect final target 可到达 |
| T-1805 | R-1808 | raw TCP before-head | 发出 valid abort request 后 EOF 前 response bytes 长度为零 |
| T-1806 | R-1808 | raw TCP/HTTP client mid-body | 200/head/prefix 已到达；missing final byte/terminal chunk；body read fails |
| T-1807 | R-1806/R-1809 | echo binary、multiple/no Content-Type、64 MiB boundary | exact body；only expected type values；413 无 partial echo |
| T-1808 | R-1802/R-1808 | HTTP Client consumer: delay/cancel/timeout and both abort phases | Sending/Receiving transitions；before-head `Transport`；mid-body head retained + `ResponseBodyRead` |
| T-1809 | R-1802/R-1805/R-1809 | HTTP Client consumer: redirect/coding/large repeat/echo upload | manual redirect policy；decode/unsupported behavior；50 MiB cap；outbound bytes/type |
| T-1810 | R-1802/R-1805 | Postman redirect example：两个 `TestServer` origin + 307 `Location` 到 `/v1/echo` | source/target origin 不同；Location 精确指向 target；target 可达；实际 Postman header 行为由 Console 人工观察 |

实施后的验证顺序：

1. `cargo fmt --all -- --check`
2. `cargo test -p http-client-test-server --all-features --locked`
3. `cargo clippy -p http-client-test-server --all-targets --all-features --locked -- -D warnings`
4. `git diff --check`
5. CLI black-box：启动 `--port 0`、读取 URL、请求 `/healthz`、在 Unix 发送 SIGINT，并断言 3 秒内以
   exit code 0 退出；Windows 分支以显式终止保证测试不遗留子进程。

若 local-loopback 测试只因 sandbox `PermissionDenied` 失败，以同一条原命令申请提权重跑，
不改写为外网服务或换用固定端口。

当前不把 packaged desktop UI、真实外网、TLS/proxy 或三平台 release app 验收当作本 crate 的完成前置条件；
它们没有被 loopback server 自动化覆盖。

## 完成证据

| 证据 | 实际结果 |
| --- | --- |
| 实施 commit/PR | producer/consumer 主实现提交 `1559cc8`；workspace feature-unification 稳定性修正 `735bc41`；本轮未创建 PR |
| 新增/修改文件与 Cargo.lock resolution | 新增 `contract/server/respond/abort/echo/main`、Postman redirect 对照 example 与 13 个 black-box integration tests；lockfile 只增加本 crate、`httpdate` 及既有 feature 所需的 `futures-util` |
| 交付的 C/ERR/F/L/ST/R/T/WP ID | `C-1800`–`C-1803`、`ERR-1800`–`ERR-1804`、`F-1800`–`F-1809`、`L-1800`–`L-1804`、`ST-1800`–`ST-1804`、`R-1800`–`R-1809`、`T-1800`–`T-1810`、`WP-1800`–`WP-1809` |
| 自动化命令与结果 | `cargo test -p http-client-test-server --all-features --locked`：2 library unit + 1 CLI unit + 13 integration，合计 16 passed；严格 Clippy、全 workspace fmt check 与 diff check 通过 |
| CLI Ctrl-C 场景 | macOS/Unix black-box test 读取 readiness、请求 healthz、发送 SIGINT，并在 3 秒内以 code 0 退出；Windows native Ctrl-C 未在本机执行 |
| HTTP Client consumer 迁移 | 已完成；最终 transport 聚焦 15/15、app 全量 161/161 通过，详见 consumer 计划 |
| Owner/root 索引同步 | producer、consumer、owner/root 索引均已同步为 `Done` |
| 未验证边界 | 实际桌面 UI、packaged app、真实外网、TLS/proxy 与三平台 release matrix；按用户要求未做实际 UI 测试 |
