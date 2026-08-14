# Issue #199：单请求 Send、Response 收集与查看实施计划

## 状态、范围与执行边界

- 状态：`Done`
- 子任务：`HTTP-199-03`
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 目标分支：`codex/199-adopt-gpui-store-form-operation`
- owner：`app/http-client`；[子任务索引](README.md)
- 产品决定权威：[HTTP Client 产品与迁移草稿](http-client-product-and-migration-draft.md)
- 前置：`HTTP-199-02` 已在 `933ee09` 交付 `Form<RequestDraft>` 与不可变
  `PreparedRequest`；本计划只消费该快照。
- 文档语言：中文；类型、API、crate、HTTP 术语与命令保留源码拼写。
- 待确认问题：无。产品草稿中剩余的 multi-tab、Store、History 与 repair 问题不阻塞本计划。

### 实施回填（2026-08-11）

- 已接通单 `RequestView` 的 Send、Cancel、Clear、head-first response 与完成后 Save；运行态由
  `request/runtime.rs` 中私有的 `Transition<HttpRunMessage>` 唯一持有，未引入 Store 或预定义
  `refresh`/`repair` family。
- `request/transport.rs` 复用显式关闭自动 redirect、referer 与 content decoding 的 Reqwest client；其
  worker 只消费 accepted `PreparedRequest`，负责 body replay、手工 redirect、content decoding 与有界
  event bridge。
- `request/response/` 已交付 `ResponseData`/`StoredBody`、内存溢写临时文件、read lease、有界 viewer
  和同目录 staging Save；response body 仍不写回 Form。
- 两份 runtime locale 与 required-key contract 已补齐 Response、问题、viewer 与 Save 文案。
- 实现提交 `24e4a9f` 已推送；`cargo fmt --check`、`cargo check`、完整 116 项测试、Clippy `-D warnings`、
  文档空白检查与残留扫描均已通过。本轮没有、也不应据此推断任何实际桌面 UI、系统保存面板、
  打包 app 或真实外网验收。

### 目标

让单个 `RequestView` 从一次已接受的 `PreparedRequest` 发出普通 Send：立即显示最终 response
head，流式收集有明确上限的完整 response，完成后可安全查看和保存；Cancel、Clear、失败、临时文件
与页面销毁均有唯一且可验证的 owner。

### 明确范围

1. 增加私有 `Transition<HttpRunMessage>` 运行态、唯一 owner-bound `gpui::Task<()>`、Reqwest
   client、手工 redirect、body replay、head-first 收集、content-encoding 解码、response viewer 和
   Save。
2. 普通 Send 的三个固定阈值：8 MiB memory spill、2 MiB inline preview、50 MiB encoded 与 stored
   capture cap（两项均要限制）。
3. Response 分为 `Body` 与 `Headers` tabs；请求编辑区域与 response 区域用可调整的纵向面板。

### 非目标

- `Send and Download`、无限/不同上限的下载流、History、multi-tab、Favorites、Environment、Store、
  repair、Cookie Jar、SSE、自动 retry、代理/客户端证书/OAuth、持久化和真实 UI/live external network
  验收。
- 修改 `gpui-form` 或 HTTP-199-02 的 Form/`PreparedRequest` API；transport 不得从 live Form、
  native control 或 Store 重建请求。
- 保存未完成 body、显示部分 body、把 HTTP 3xx/4xx/5xx 变成错误、执行 HTML/Markdown/SVG 或加载其
  外部资源。

## 系统面适用性

| ID | 系统面 | 本计划结论 | 工作包 |
| --- | --- | --- | --- |
| `S-1600` | 入口与生命周期 | `RequestView` 直接拥有 transport、运行态、viewer 与导出 Task；页面 drop 取消所有未完成工作 | `WP-1600`–`WP-1605` |
| `S-1601` | action / command / keybinding | 接通现有 Send 按钮并新增 Cancel/Clear/Save 页面命令；不加全局快捷键 | `WP-1603`、`WP-1605` |
| `S-1602` | UI 结构与交互 | Request/Response 纵向可调整分区，Response 提供 Body/Headers 与状态工具栏 | `WP-1604`、`WP-1605` |
| `S-1603` | focus / IME / native identity | 运行中 Form 继续可编辑；Response 投影不得重建 Request native controls | `WP-1603`、`WP-1604` |
| `S-1604` | 异步与 Task owner | runtime 唯一 outer GPUI Task，内部持有 abort-on-drop Tokio Task；preview/Save 由页面持有 | `WP-1601`、`WP-1603`–`WP-1605` |
| `S-1605` | Operation / 状态机 | 使用 app 私有 `Transition<HttpRunMessage>`；不用 `refresh`/`repair` family | `WP-1603` |
| `S-1606` | Form | 只消费 `prepare_request` 的 accepted `PreparedRequest`；不修改 Form API | `WP-1603` |
| `S-1607` | Store | 单页面、单 consumer，不引入 Store 或 Global response authority | — |
| `S-1608` | 外部协议 | Reqwest、HTTP redirect、headers、content coding、charset 与文件 body 重放均适用 | `WP-1601`、`WP-1602` |
| `S-1609` | 错误与恢复 | typed `RequestProblem`；Cancel 不是错误，Retry/repair 不在本轮 | `WP-1601`–`WP-1603` |
| `S-1610` | 数据库与持久化 | 不保存 request/response/history；完成 body 只在内存或 owner-local 临时文件 | — |
| `S-1611` | generated / synchronized 内容 | 无生成源码；`Cargo.lock` 只由 Cargo 随 direct dependency 更新 | `WP-1600` |
| `S-1612` | assets / icon | 无新增 icon、asset 或 bundle 资源 | — |
| `S-1613` | Fluent i18n | 两个 runtime locale 同步新增 Response/错误/Save 文案与变量契约 | `WP-1606` |
| `S-1614` | 安全与隐私 | 不执行 response markup，不记录 URL/query/header value/body/token/路径，图片有解码预算 | `WP-1600`、`WP-1604`、`WP-1606` |
| `S-1615` | tracing | 只记录 phase、message kind、problem kind 与安全计数；非法消息同样脱敏 | `WP-1603`、`WP-1606` |
| `S-1616` | packaging / CI | app/bundle/workflow 不变；本轮不做打包或实际桌面 UI 验收 | `WP-1607` |
| `S-1617` | 依赖 | 增加精确版本的 transport/decoder/storage/viewer 依赖并更新 lockfile | `WP-1600` |
| `S-1618` | owner 文档 | 本计划承载执行契约；README 只维护状态与入口 | `WP-1607` |
| `S-1619` | 验证证据 | local TCP、pure collector/decoder、GPUI owner/viewer、i18n 与残留扫描 | `WP-1607` |

## 当前证据与已固定决定

| ID | 分类 | 已核实事实/决定 | 证据 | 计划后果 |
| --- | --- | --- | --- | --- |
| E-1600 | 实施前事实 | `RequestView` 仅拥有 Form、transport settings、五个 request tabs 和 focus；`button-send` 固定 `.disabled(true)`。 | `HTTP-199-02` 的 `933ee09` 与本计划建立时的 `src/features/request.rs:RequestView` | 此 view 是唯一 response/runtime owner，实施后接通 Send/Cancel/Clear/Save。 |
| E-1601 | 当前事实 | `prepare_request` 先 `Form::prepare`，再 `compile_request`，产生含 method、URL、`HeaderMap`、body、redirect 和 timeout 的不可变 `PreparedRequest`。 | `src/features/request.rs:prepare_request`、`request/prepared.rs:compile_request` | Send 只 move 此值进入 task；prepare 失败时不改变当前 runtime。 |
| E-1602 | 当前事实 | 编译期已验证 HTTP(S) URL，body 可为内存 text/urlencoded、multipart 文件或 binary 文件；redirect 已冻结 max 10 hops、method 和跨 host Authorization 策略。 | `request/prepared.rs:PreparedRequest`、`PreparedBody`、`PreparedRedirect` | executor 每一跳从 frozen value 重建 request/body，不能重用已消耗的 stream。 |
| E-1603 | 当前事实 | `gpui-tokio::Tokio::spawn` 在 workspace Tokio runtime 运行 `Send + 'static` future，返回的 GPUI Task drop 会 abort Tokio JoinHandle。 | `crates/gpui-tokio/src/lib.rs:Tokio::spawn` | outer owner task 是取消根；不得 detach worker 或另造 generation state。 |
| E-1604 | 当前事实 | `gpui-operation` 的 runtime task 规则是 final state 先安装、再 drop task/payload；tracing feature 只记录脱敏非法消息。 | `crates/gpui-operation/README.md`、`dev/message-driven-transitions.md` | 私有 Transition 复用该顺序与诊断原则，不能把此 HTTP 多阶段状态塞进 `refresh::Operation`。 |
| E-1605 | 当前事实 | app 已使用 `TabBar`、`Button`、`v_flex` 和 runtime Fluent locale；`InputState::code_editor` 使用 Rope、Tree-sitter、可见行渲染和内建搜索，`v_resizable`/`resizable_panel` 支持 constrained panels。 | `request/tab.rs`、`foundation/i18n.rs`、gpui-component Editor/Resizable 文档 | response UI 组合既有组件，不写通用自定义 tab、split 或 text renderer。 |
| E-1606 | 当前事实 | lockfile 已解析 reqwest 0.13.4、async-channel 2.5.0、async-compression 0.4.42、bytes 1.12.0、encoding_rs 0.8.35、futures-util 0.3.33、serde_json 1.0.151、tempfile 3.27.0、tokio 1.53.1、tokio-util 0.7.19、image 0.25.10。 | `Cargo.lock` | 本轮只把这些精确版本/feature 写为 app direct dependencies，不手写 lockfile。 |
| D-1600 | 用户决定 | 普通 Send 上限为 8 MiB / 2 MiB / 50 MiB，且 encoded 与 stored 必须同时受限。 | 本轮任务 | 常量和测试固定为此值；“Send and Download”另立设计。 |
| D-1601 | 用户决定 | `RequestView` 不用 Store；`Ready` 必须持有 `Arc<ResponseData>`，Save 使用 clone 的 Arc/read lease。 | 本轮任务 | 不新增 `gpui-store`，不暴露裸 `PathBuf`，没有第二份 response business state。 |
| D-1602 | 用户决定 | Reqwest 必须 `.no_gzip().no_brotli().no_deflate().no_zstd()`，并关闭自动 redirect；应用自行 decode 与 redirect。 | 本轮任务 | 保留原 response headers 和编码语义，不使用 `error_for_status`。 |

## 文件、依赖与边界

### 文件图

```text
.
├── Cargo.lock                                           # F-1601 [修改，Cargo 生成] 解析后的依赖锁定
├── docs/dev/issue-199/README.md                         # F-1622 [修改，手写] root 状态索引
└── app/http-client/
    ├── Cargo.toml                                       # F-1600 [修改，手写] 精确依赖与 feature
    ├── src/main.rs                                      # F-1602 [修改，手写] gpui-tokio 初始化
    ├── src/features/request.rs                          # F-1603 [修改，手写] RequestView owner、命令与总布局
    ├── src/features/request/runtime.rs                  # F-1604 [新增，手写] 私有 Transition、message、effect 与问题类型
    ├── src/features/request/transport.rs                # F-1605 [新增，手写] HttpTransport、worker/event 边界
    ├── src/features/request/transport/body.rs           # F-1606 [新增，手写] PreparedBody replay 与 request body I/O
    ├── src/features/request/transport/redirect.rs       # F-1607 [新增，手写] 手工 redirect policy
    ├── src/features/request/transport/worker.rs         # F-1608 [新增，手写] Reqwest/Tokio 执行与 channel 顺序
    ├── src/features/request/response.rs                 # F-1609 [新增，手写] Response pane/controller 入口
    ├── src/features/request/response/data.rs            # F-1610 [新增，手写] ResponseData、StoredBody、read lease
    ├── src/features/request/response/collector.rs       # F-1611 [新增，手写] 8/50 MiB collector 与临时文件
    ├── src/features/request/response/decoding.rs        # F-1612 [新增，手写] Content-Encoding/charset 与 projection helper
    ├── src/features/request/response/viewer.rs          # F-1613 [新增，手写] Body/Headers、安全文本/图片 viewer
    ├── src/features/request/response/save.rs            # F-1614 [新增，手写] 完成 Response 的事务式 Save
    ├── src/features/request/tests.rs                    # F-1615 [修改，手写] page/runtime GPUI tests
    ├── src/foundation/i18n.rs                           # F-1616 [修改，手写] required-key 与变量契约
    ├── locales/en-US/main.ftl                           # F-1617 [修改，手写] 英文 runtime 文案
    ├── locales/zh-CN/main.ftl                           # F-1618 [修改，手写] 中文 runtime 文案
    ├── docs/dev/issue-199/request-send-and-response-plan.md # F-1619 [修改，手写] 完成证据
    ├── docs/dev/issue-199/README.md                     # F-1620 [修改，手写] owner 状态索引
    └── docs/dev/README.md                               # F-1621 [修改，手写] app 状态索引
```

不新增 `mod.rs`。`transport` 只消费现有私有 `prepared` 模块并产生 worker event，不认识 GPUI/Form/UI；
`response` 只拥有 Response 数据、存储、投影与 UI，不编辑 Request draft；`runtime` 只做状态转换，不直接
发网络请求或渲染。`RequestView` 组合三者并拥有全部 Task。测试就近放入相应模块的 `#[cfg(test)]`
子模块；不为测试建立第二套生产 façade，也不新增共享 crate consumer。

### D-1603：依赖清单和 feature 契约

`Cargo.toml` 的实现须用下列精确声明；改动 manifest 后仅由 Cargo 更新 `Cargo.lock`。`reqwest`
default features 必须关闭，且不启用 `gzip`、`brotli`、`deflate`、`zstd`、`cookies` 或 HTTP/3。

| 依赖 | 声明 / features | HTTP-199-03 用途 |
| --- | --- | --- |
| `gpui-tokio` | `workspace = true` | worker runtime 与 abort-on-drop JoinHandle bridge |
| `gpui-operation` | `workspace = true, features = ["tracing"]` | 只使用 `Transition` trait 与脱敏非法消息 tracing；不用 family `Operation` |
| `reqwest` | `0.13.4`, `default-features = false`, features `rustls,http2,multipart,stream,system-proxy` | async Client、stream、multipart、TLS；`redirect(Policy::none())` |
| `async-channel` | `2.5.0` | `bounded(8)` worker-to-outer events |
| `async-compression` | `0.4.42`, `default-features = false`, features `tokio,brotli,gzip,zlib,zstd` | app-side `Content-Encoding` decoding |
| `bytes` | `1.12.0` | in-memory `StoredBody::Memory(Bytes)` |
| `encoding_rs` | `0.8.35` | Content-Type charset text projection |
| `futures-util` | `0.3.33` | `StreamExt` response chunks / stream adaptation |
| `serde_json` | `1.0.151` | ≤2 MiB JSON parse/pretty projection |
| `tempfile` | `3.27.0` | collector `NamedTempFile` → `TempPath` ownership |
| `tokio` | `1.53.1`, features `fs,io-util,time`; dev `macros,net,rt-multi-thread` | async file I/O、timeouts、local TCP tests |
| `tokio-util` | `0.7.19`, feature `io` | response stream/decoder reader bridge |
| `image` | `0.25.10`, `default-features = false`, features `gif,jpeg,png,webp` | 有界 raster decode；明确不启用 SVG |

不增加 `http-body`、`http-body-util`、`futures` 元 crate、`async-compat` 或 `smol`。不启用 Reqwest 的
`charset/json/query/form/cookies/http3/gzip/brotli/deflate/zstd` features；Request 已编译为 bytes，charset/
JSON 只用于 viewer，content decoding 由本 app 控制。没有上游类型能替代 app 私有状态机：Reqwest 不
表达本产品的 head-first UI、手工 redirect 与 collector 上限，gpui-operation 预定义 family 也没有单一的
head/progress/body authority，因此按 `D-1604` 保留 feature-local 实现。

### D-1604：运行态使用私有完整 Transition

Send 在完整 Response 前必须经历 `Sending` 与 `Receiving`，因此只实现私有
`Transition<HttpRunMessage>`，不在旁边维护独立 head/progress，也不套 `refresh::Operation` 或
`repair::Operation`。运行态是唯一 Task/response authority，`RequestView` 只是其 owner 与 UI 投影者。

### D-1605：accepted Send、Cancel 与历史边界

先拒绝运行中重复 Send，再执行 Form prepare/compile；prepare 失败保持当前 terminal state。只有同步
安装 `Start` 后才算 accepted Send，并立即释放旧 Response/problem。Cancel 与 Clear 都进入 `Idle`；不
恢复旧结果、不排队、不自动 Retry。未来 History 是独立持久化/共享 owner，不进入本运行态。

### D-1606：viewer 与导出是有界派生状态

完成 body 是唯一权威 bytes；格式化文本、Hex、Base64、图片和 header rows 都是可丢弃投影。viewer
失败不得改变 `Ready`。Save 只消费完成 Response 的 read lease；`Send and Download` 具有不同的目标文件
事务和上限，明确不在本计划。

### 上游与仓内复用审计

| 能力 | 复用结论 | 不采用的替代 |
| --- | --- | --- |
| HTTP/TLS/pool/multipart/stream | 直接使用 Reqwest 0.13.4；app 只补产品特有 redirect/collector/decode policy | 不引入 Hyper/http-body façade，不自写 socket/TLS |
| Tokio bridge | 使用 `gpui-tokio` 的 runtime 与 abort-on-drop Task | 不引入 smol/async-compat 网络 worker，不 detach |
| 状态转换 | 使用 `gpui_operation::Transition` trait | 多阶段 head/progress 不适合 `refresh`/`repair` family，不并行维护 phase bool |
| Response 布局/交互 | 使用 gpui-component `v_resizable`、`TabBar`、`Table`、`Progress`、`Alert`、`Select`、`Button` | 不重复实现 splitter/tab/table/progress |
| 只读文本 | 使用持久化的 `InputState::code_editor`，以 `Input::disabled(true)` 禁写并保留选择、复制、搜索和滚动 | 不把 response 交给 Markdown/HTML renderer，不订阅编辑事件后回写，不在每帧重建 Editor |
| 图片 | 使用 `image` crate 做有界 decode，再交 `Arc<RenderImage>` | 不直接把未验证动画 bytes 交给 GPUI decoder，不支持 SVG image |
| 临时文件与导出 | 使用 `tempfile::TempPath`/staging persist 与 GPUI save panel | 不暴露裸临时路径，不直接写坏用户目标，不增加第三方 picker |
| shared state | `RequestView` 直接拥有运行态与 Response | 单 consumer 无 Store 适用性，不建立 Global/Store 镜像 |

## 目标契约

### L-1600：完成 response 数据、临时文件与 read lease

```rust,ignore
const MEMORY_SPILL_BYTES: u64 = 8 * 1024 * 1024;
const INLINE_PREVIEW_BYTES: u64 = 2 * 1024 * 1024;
const CAPTURE_LIMIT_BYTES: u64 = 50 * 1024 * 1024;

pub(crate) struct ResponseData {
    head: ResponseHead,
    timing: ResponseTiming,
    sizes: ResponseSizes,
    body_decoding: BodyDecoding,
    body: StoredBody,
}
pub(crate) struct ResponseHead {
    status: http::StatusCode,
    version: http::Version,
    final_url: url::Url,
    headers: http::HeaderMap,
}
pub(crate) struct ResponseTiming { head_after: Duration, completed_after: Duration }
pub(crate) struct ResponseSizes {
    declared_encoded_bytes: Option<u64>, received_encoded_bytes: u64, stored_body_bytes: u64,
}
pub(crate) enum BodyDecoding { Identity, Decoded, Unsupported }
pub(crate) enum StoredBody {
    Empty,
    Memory(bytes::Bytes),
    TempFile { path: tempfile::TempPath, len: u64 },
}
pub(crate) struct ResponseReadLease(Arc<ResponseData>);
pub(crate) struct PrefixBytes { bytes: bytes::Bytes, complete: bool }

impl ResponseData {
    pub(crate) fn read_lease(self: &Arc<Self>) -> ResponseReadLease;
}

impl ResponseReadLease {
    pub(crate) fn len(&self) -> u64;
    pub(crate) async fn read_prefix(
        &self,
        limit: usize,
    ) -> Result<PrefixBytes, ResponseReadProblem>;
    pub(crate) async fn copy_all_to<W>(
        &self,
        writer: &mut W,
    ) -> Result<u64, ResponseReadProblem>
    where
        W: tokio::io::AsyncWrite + Unpin;
}
```

- `ResponseData`、`ResponseHead`、`StoredBody`、`RequestProblem` 与 `ResponseReadLease` 不派生会打印
  URL/query、header value、response 内容或临时路径的 `Debug`；需要 Debug 时手写脱敏实现。
  `HeaderMap` 保持权威，保留重复字段与非 UTF-8 value。
- `ResponseData` 没有值语义 `Clone`。`RequestRuntime::Ready` 持有 `Arc<ResponseData>`；读取、Save 与
  projection 先 clone Arc，再调用 `ResponseData::read_lease(self: &Arc<Self>)`。opaque lease 在打开
  memory/file 前保留 Arc，使新 Send、Clear 或 view 重置不能提前删除仍在使用的 `TempPath`。
- `TempPath` 是临时文件删除责任的唯一 owner。collector 成功后只把它移交一次；所有成功前的错误、
  Cancel 与 worker drop 都自动删除部分文件。Save 复制到用户目标，绝不公开或移动临时路径。
- `received_encoded_bytes` 指 Reqwest transfer decoding 后、应用 content decoding 前收到的 payload；
  不是 TLS/framing/header wire size。`stored_body_bytes` 是 `StoredBody` 实际 bytes，解压后可以更大。
- `read_prefix` 只返回 `min(limit, len)` 的起始 bytes；`complete` 只在完整 body 不大于 limit 时为 true。
  TempFile 短读映射为脱敏 `ResponseReadProblem::LengthMismatch`；截断 preview 不扫描剩余文件。
- `copy_all_to` 从 offset 0 流式复制，必须恰好读取 `len`，随后额外探测 1 byte 以拒绝文件被外部异常
  缩短/增长；返回值必须等于 `len`。Memory/Empty 走同一语义。方法不暴露 path/file handle，open/read/
  length mismatch 只映射 `ResponseReadProblem::{Open, Read, LengthMismatch}`。

### L-1601：私有运行态与完整 message/合法表

```rust,ignore
type RequestTask = gpui::Task<()>;
enum RequestRuntime {
    Idle,
    Sending { task: RequestTask, started_at: Instant },
    Receiving { task: RequestTask, receipt: ResponseReceipt },
    Ready { response: Arc<ResponseData> },
    Failed { attempt: FailedAttempt },
}
struct ResponseReceipt { head: ResponseHead, progress: ResponseProgress, head_after: Duration }
struct ResponseProgress {
    declared_encoded_bytes: Option<u64>, received_encoded_bytes: u64,
    stored_body_bytes: u64, storage: ActiveBodyStorage,
}
enum ActiveBodyStorage { Memory, TempFile }
struct FailedAttempt { problem: RequestProblem, receipt: Option<ResponseReceipt>, failed_after: Duration }
enum HttpRunMessage {
    Start { task: RequestTask, started_at: Instant },
    HeadReceived { head: ResponseHead, head_after: Duration, progress: ResponseProgress },
    BodyProgress(ResponseProgress),
    Finished { result: Result<CompletedBody, RequestProblem>, finished_after: Duration },
    Cancel, Clear,
}
struct CompletedBody { body: StoredBody, body_decoding: BodyDecoding, sizes: ResponseSizes }
enum HttpRunEffect {
    Ignored,
    Started,
    HeadAccepted,
    Progressed,
    Ready,
    Failed,
    Cancelled,
    Cleared,
}
```

`impl Transition<HttpRunMessage> for &mut RequestRuntime` 保持私有。实现先取出完整 enum；合法 terminal
转换必须先安装下一状态，再 drop 退出的 task、problem、response、collector payload 或 `TempPath`；非法
输入先原样恢复旧状态，只记录 kind/phase/message，再 drop 消息。完整合法表为：

`Transition::Output` 使用不携带敏感数据或 owner payload 的私有 `HttpRunEffect`。`RequestView` 只根据
accepted effect 重置 viewer、notify 或启动 projection；`Ignored` 不 notify、不调度后续 effect。

| 当前状态 | 接受的消息 | 下一状态 / 必须副作用 |
| --- | --- | --- |
| `Idle`, `Ready`, `Failed` | `Start` | `Sending`；accepted Start 安装后释放旧 response/problem，viewer 不再显示旧 body |
| `Sending` | `HeadReceived` | `Receiving`，持有本次 head/progress |
| `Sending` | `Finished(Err)` | `Failed { receipt: None }` |
| `Receiving` | `BodyProgress` | 保持 `Receiving`，只接受不回退的绝对计数 |
| `Receiving` | `Finished(Ok)` | `Ready(Arc::new(ResponseData))`；消费现有 head，不接收第二份 head |
| `Receiving` | `Finished(Err)` | `Failed`，保留 receipt 但不保留部分 body |
| `Sending`, `Receiving` | `Cancel` | `Idle`；清除本次 head/progress/collector 并 drop 唯一 Task |
| `Ready`, `Failed` | `Clear` | `Idle`；释放完整 response/problem |

其他组合全部非法：运行中 `Start`、稳定态 `Cancel`、运行态 `Clear`、`Sending+BodyProgress`、重复 head、
无 head 的成功完成以及 Cancel/terminal 后全部迟到事件。accepted Send 清空旧 Response；Form prepare
失败发生在 `Start` 之前，保持当前状态。

### ST-1600：RequestView authority、tasks与顺序

- **Authority：** `RequestView { form, transport_settings, transport, runtime, response_pane,
  preview_task, save_task, ... }`。Form 仍是唯一可编辑 request authority，runtime 是唯一
  transport/response authority；native control 和 `ResponsePane` 只保留交互/投影状态。
- **生命周期与读取：** `RequestView::render` 只投影 runtime。`Ready` 向 headers、preview、image decode 与
  Save 提供 `Arc<ResponseData>`；`Failed` 只提供安全的 problem/receipt metadata；两者都不持久化。
- **不使用 Store：** 页面是唯一 consumer，Response 没有跨窗口/history identity；不得新增
  `gpui-store`、Global response、镜像 phase bool 或第二个可变 authority。
- **Client：** `RequestView::new` 只构造一次 `HttpTransport`。其内部保存
  `Result<reqwest::Client, Arc<RequestProblem>>`，builder 固定设置
  `.redirect(Policy::none()).referer(false).no_gzip().no_brotli().no_deflate().no_zstd()`；成功 Client 在
  页面内复用。构造失败不 panic，也不在初始化时伪造运行态；下一次 accepted Send 的 lazy Task 可靠
  路由 `Finished(Err(Transport))`。
- **Send 顺序：** (1) 先检查 runtime，`Sending/Receiving` 立即拒绝且不再次触发 Submit validation；
  (2) 调用现有 `prepare_request`，错误保持精确 Form issue 与当前 terminal state；(3) lazy `cx.spawn`
  只捕获 `PreparedRequest`、transport handle 和 weak owner；(4) 同步 `transition(Start)`，此刻才清除旧
  terminal data；(5) `cx.notify`；(6) outer Task 首次 poll 时才调用 `gpui_tokio::Tokio::spawn`。运行中
  后续 Form 编辑只影响下一次 Send。
- **Task ownership / Cancel：** `RequestRuntime` 持有 outer `RequestTask`；outer task 持有 bounded
  receiver 与 abort-on-drop Tokio task。Cancel、accepted terminal 与 `RequestView` drop 都先安装最终
  state，再 drop outer task；worker abort 后 collector 自动清理。没有 detached producer，因此不需要
  attempt ID/generation。weak owner 升级失败时 outer task 结束并取消 inner task。

### C-1600：PreparedRequest → HTTP 与 response worker

`transport::run(prepared: PreparedRequest, client: reqwest::Client, sender: async_channel::Sender<WorkerEvent>)`
是唯一外部执行边界；未发布 app 无兼容/迁移 rollout，也没有持久化。

```rust,ignore
enum WorkerEvent {
    HeadReceived {
        head: ResponseHead,
        head_after: Duration,
        progress: ResponseProgress,
    },
    BodyProgress(ResponseProgress),
    Finished {
        result: Result<CompletedBody, RequestProblem>,
        finished_after: Duration,
    },
}
```

1. 每一跳从冻结 method/URL/重复值 `HeaderMap` 与可重放 body 新建 `RequestBuilder`。Text/UrlEncoded 先
   转 `Bytes` 后廉价 clone；Binary 用 `tokio::fs::File + ReaderStream` 重开绝对路径；每个 Multipart
   file 与整个 form 每一跳重新打开/重建，不能复用已消费 stream，也不要求 `PreparedRequest: Clone`。
2. body builder 先产生默认 `Content-Type`/boundary/length，随后用 prepared headers 覆盖同名 header，
   从而保持 HTTP-199-02 的“显式 Content-Type 优先”契约与重复 header values。跳转改为 GET 后永久删除
   body、`Content-Length`、`Content-Type`、`Transfer-Encoding`，后续跳不得从原快照恢复。
3. 不调用 `error_for_status`。只跟随 301/302/303/307/308；`follow=false` 或缺少 `Location` 时把当前 3xx
   当普通最终 Response。`preserve_method=false` 时 301/302/303 均改 GET（HEAD 保持 HEAD）并删除 body，
   307/308 保持 method/body；`preserve_method=true` 始终保持。相对 Location 基于当前 URL join，
   请求前删除 fragment；无效 Location、重复 URL loop 与 hop exhaustion 映射 `ERR-1602`。
4. scheme、host 或有效 port 任一变化都算跨 origin。删除显式 `Host` 与 `Cookie`；除非
   `forward_authorization_cross_host=true`，否则同时删除 `Authorization`。其他自定义 header（包括任意
   名称的 API Key）继续发送；已删除的 header 在后续跳转中不得从最初 headers 恢复。
5. 只有最终 head 可靠发送
   `HeadReceived { status, version, final_url, HeaderMap, Content-Length }`；中间 3xx 不进入 UI。整个
   redirect chain、最终 body read、content decode 与临时文件写入只包一层 frozen timeout，不能给每一
   hop 重新获得完整 timeout 额度。
6. 使用 `async_channel::bounded(8)`：head、错误前最后一次绝对 progress 与 terminal event 使用
   `send().await` 保证 FIFO；普通 progress 最多约每 100ms `try_send` 一次，Full 时只允许丢/合并
   progress。outer receiver 通过 weak `RequestView` 转成 `HttpRunMessage` 并 notify；channel 未收到
   terminal 就关闭时等待 inner Join 结果，panic/异常取消映射 `ERR-1608`，不能静默留在运行态。
   每个未取消 worker 必须且只能产生一次 `Finished`；Cancel/drop sender 不伪造 terminal。

### L-1602：collector、decode、limits与失败分类

`BodyCollector` 只存在于 worker：初始 `Memory(Vec<u8>)`；下一次写入将跨过 8 MiB 时，在
`spawn_blocking` 中创建 `NamedTempFile::into_parts()`，把 std file 转为 `tokio::fs::File`，同时保留
唯一 `TempPath`，先写已有 buffer 再写当前 decoded chunk。只有 EOF、limit、flush 与 close 全部成功后
才能产生 `StoredBody`；临时 response cache 不要求 `fsync`。错误、Cancel 与 Task abort 都 drop
collector/TempPath。

先完整解析所有 `Content-Encoding` header token：全部为 `identity/gzip/br/deflate/zstd` 时，使用
`StreamReader` 与 `Pin<Box<dyn AsyncRead + Send>>` 按逆序包 decoder；HTTP `deflate` 明确使用 zlib
wrapper 的 `ZlibDecoder`。任一 token 未知、为空、非 UTF-8 或不可解析时完全不解码，收集完整 encoded
payload 并标记 `Unsupported`，不能先解一层再回退。已选择 decoder 的流错误属于 terminal decode
failure，不保留部分输出。

最终 head 可靠发送后，若声明的 `Content-Length` 已超过 50 MiB，立即以 encoded size problem 结束。
无论是否有长度，都在 decoder 前逐块限制 encoded ≤ 50 MiB，在 decoder 后逐块限制 stored ≤
50 MiB，且每次写入前检查，避免多写一个 chunk 与解压炸弹。progress 报告 encoded、stored 与
Memory/TempFile 的绝对值。HEAD/204/零长度仍严格走 `HeadReceived -> Finished(StoredBody::Empty)`。

```rust,ignore
enum RequestProblemKind {
    Transport,
    Timeout,
    Redirect(RedirectProblemKind),
    RequestBodyRead,
    ResponseBodyRead,
    ResponseBodyDecode,
    TemporaryStorage,
    BodyTooLarge {
        dimension: BodySizeDimension,
        limit: u64,
        observed: u64,
    },
    Internal,
}

enum RedirectProblemKind { InvalidLocation, Loop, HopLimit }
enum BodySizeDimension { Encoded, Stored }
```

| ERR-ID | `RequestProblemKind` | 触发、恢复与可见安全信息 |
| --- | --- | --- |
| ERR-1600 | `Transport` | DNS、连接、TLS 或 builder；用户可编辑后再 Send，只展示类别 |
| ERR-1601 | `Timeout` | frozen total timeout 到期；不自动 Retry |
| ERR-1602 | `Redirect(InvalidLocation\|Loop\|HopLimit)` | 无效 Location、loop 或跳数上限；缺少 Location 不是错误 |
| ERR-1603 | `RequestBodyRead` | Binary/Multipart path 在 prepare 后无法重新打开/读取；不得归入 response read |
| ERR-1604 | `ResponseBodyRead` | 最终 response stream 中断；可保留 receipt，不保留部分 body |
| ERR-1605 | `ResponseBodyDecode` | 已支持 coding 的 decoder 失败；只保留 head/progress |
| ERR-1606 | `TemporaryStorage` | response collector 的 spill/create/write/flush 失败；UI/log 不含路径 |
| ERR-1607 | `BodyTooLarge { dimension: Encoded\|Stored, limit, observed }` | 声明或实测超过 50 MiB；保留 head 与安全计数 |
| ERR-1608 | `Internal` | worker panic、channel 无 terminal 关闭或内部不变量；只记录类别/phase |

Cancel 不是 problem；3xx/4xx/5xx 都是 Response status。底层 reqwest/io source 可以私有保留给
`Error::source`，但 Fluent 与默认 tracing 只接收 ERR kind、phase 与安全 byte counter，不接收 URL、
headers、body、temp path、auth、token 或 source 的 Display/Debug 文本。

完成后的 `ResponseReadProblem`、`ResponseViewProblem` 与 `ResponseSaveProblem` 属于 viewer/action 局部错误，
分别覆盖 read lease、charset/JSON/image projection 与 Save staging/copy/persist；它们只更新 Response pane
warning/Alert，不发送 `HttpRunMessage`，也不能把 `Ready` 改成 `Failed`。这三类同样保留私有 source 且
使用脱敏 Display/Debug。

### ST-1601：response viewer、image projection与 Save

- `ResponsePane` 保存 owner-local 投影：`response_tab: Body|Headers`、
  `mode: Auto|Text|Json|Xml|Hex|Base64|Image`、warning、projection task 与可选
  `Arc<RenderImage>`；不复制 `StoredBody`。accepted Send 重置为 Body/Auto，新的 Ready 或 Clear 会取消旧
  preview/image task。Headers 从 `HeaderMap` 逐值投影，重复值不折叠，非 UTF-8/不可见 bytes 使用
  `escape_ascii`，禁止 `to_str().unwrap()`。
- preview 通过 read lease 最多读取 2 MiB source bytes；生成的 editor source、pretty JSON、Hex 与 Base64
  也各自硬限制在 2 MiB 和 50,000 行，builder 到达输出上限即停止并标注截断。完整 JSON 大于 2 MiB
  不解析；pretty 结果超限退回有界 Text。Base64/Hex 不允许先读取完整 TempFile。
- Auto 先检查 `BodyDecoding`；`Unsupported` 只允许 Hex/Base64/Save。其余按 `Content-Type` 与
  `+json/+xml` 选择。显式 charset 只接受 `encoding_rs` 已知 label；无 charset 时先识别 UTF BOM，否则
  尝试 UTF-8。未知 charset、产生 replacement 或无法无损解码时进入 bytes view + warning，不改变
  runtime。截断 preview 使用 streaming decoder/移除不完整尾字符，不能因 2 MiB 边界切在多字节字符
  中间而把完整 Response 误判为不可解码。
- Text/JSON/XML/HTML/CSS/JS/YAML/Markdown/SVG 都通过 owner-local
  `InputState::code_editor(language)` 和 `Input::disabled(true)` 展示；禁写状态保留 selection、Copy、Search
  与 scrolling，并用 `replaceable(false)` 删除 Search 的 Replace 能力。每次当前 token 的新投影只创建一次
  Editor entity，模式切换、新 Send、Clear 与 owner drop 都释放它。XML/SVG 在当前语言集合中使用 plain text，
  其余使用可用的精确 highlighter。响应正文不进入 Markdown/HTML renderer，SVG 也不得进入 image path。
- Image 只接受完整 stored body ≤ 2 MiB 的 PNG/JPEG/GIF/WebP，并按已验证的格式选择对应
  `image` decoder，不把 bytes 直接交给 GPUI 的通用 decoder。decoder 必须从 `Limits::default()`
  创建 limits，再把 `max_image_width/max_image_height/max_alloc` 分别设为 `4096/4096/64 MiB`；
  其中宽高是 strict limit，`max_alloc` 只作为 defense-in-depth，不代替应用自己的预算。
- 在静态图或动画每一帧解码前，先对 canvas `width * height * 4` 做 checked arithmetic，再从
  64 MiB 累计 RGBA 预算中保留这一帧；溢出、超尺寸或无法保留时不调用下一次 decode。
  GIF/WebP 一次只从 iterator 取一帧，最多保留 16 帧；为检测第 17 帧，也必须先在同一
  64 MiB 预算中保留一个 canvas 后才允许请求下一帧，若确实存在则整张图拒绝并丢弃已解码帧。
  每帧将解码得到的 RGBA `Vec<u8>` 原地交换 R/B 得到 BGRA，再直接移入 GPUI frame；不同时保留
  image-crate 像素副本与 RenderImage 像素副本。最终构造 `Arc<RenderImage>`，Task 完成时以
  `Arc::ptr_eq` 校验当前 source，迟到结果丢弃；任何格式、预算或 decode 失败只形成 viewer warning。
- Save 仅在 `Ready` 可用，页面使用私有
  `ResponseSaveController { status: Idle|Succeeded|Failed(ResponseSaveProblem), task: Option<Task<()>> }`。
  `task.is_some()` 时 Save 按钮禁用，事件入口也直接拒绝重入；点击后先安装覆盖 picker 与复制全过程的
  page-owned Task，再调用 `prompt_for_new_path`。picker Cancel 回到 `Idle`，不显示 Alert、不创建文件；
  picker error 进入 `Failed`。
- 确认目标后，Task clone `Arc<ResponseData>` 并创建 read lease，在目标同目录创建由
  `NamedTempFile`/`TempPath` 拥有的 staging file。通过 `copy_all_to` 流式复制并再校验返回数等于
  lease `len`，然后 flush/close，最后在 `spawn_blocking` 内使用已确认 overwrite 语义的 persist。
  任何 read/copy/flush/close/persist 错误都进入 `Failed`，staging owner 自动清理，用户目标不留下
  部分内容。成功进入 `Succeeded`并清空 task。
- Clear 或新 accepted Send 只重置 Response pane，不 drop 已开始的 Save Task；因为 Task 持有 Arc/read
  lease，旧 `TempPath` 会保留到复制结束。页面 drop 才取消 Save，staging 随 owner 清理；weak owner
  已消失时 completion 不更新 UI。Save 是 viewer-local action，全过程不发送 `HttpRunMessage`。
  `BodyDecoding::Unsupported` 时复制 encoded payload，并在 UI 明示；其他情况复制 stored representation bytes。

### L-1603：UI composition与 i18n

`RequestView::render` 使用 `v_resizable("request-response")`：上部保留现有 Request editor 且可扩展，下部
Response pane 默认约 320 px、最小 160 px。Response 使用 `TabBar("response-tabs")` 的 Body/Headers；
按钮 ID 固定为 `request-send`、`request-cancel`、`response-clear`、`response-save`。运行态禁用 Send 并
显示 Cancel；terminal 显示 Clear；Request 输入仍可编辑并保留 focus，不新增全局 keybinding。

- `Idle`：稳定空态；`Sending`：状态文字与 indeterminate `Progress`；
- `Receiving`：立即显示 status/final URL/protocol/head time、Headers 与 received/stored bytes；有
  Content-Length 时显示比例，否则保持 indeterminate；Body tab 只显示接收中，不显示部分 body；
- `Ready`：显示 summary、Body/Headers、mode Select、Save/Clear；4xx/5xx 可按 status family 着色，但
  不使用 runtime error Alert；
- `Failed`：`Alert::error` 显示稳定问题；有 receipt 时仍允许查看 summary/Headers，但不出现 Body/Save。

| Key | locale 文件 | 调用者 / 状态 |
| --- | --- | --- |
| `button-cancel`, `button-clear-response`, `button-save-response` | 两份 `main.ftl` | Cancel/Clear/Save |
| `tab-response-body`, `tab-response-headers` | 两份 | Response tabs |
| `response-empty`, `response-sending` | 两份 | Idle / Sending 状态 |
| `response-receiving-known`, `response-receiving-unknown` | 两份 | 已知长度参数 `{ $received }`、`{ $total }`；未知长度只用 `{ $received }` |
| `response-status`, `response-final-url`, `response-protocol`, `response-head-time`, `response-total-time`, `response-received-size`, `response-stored-size` | 两份 | summary labels |
| `response-header-name`, `response-header-value`, `response-headers-empty` | 两份 | Headers table |
| `request-problem-transport`, `request-problem-timeout`, `request-problem-redirect`, `request-problem-request-body`, `request-problem-response-read`, `request-problem-response-decode`, `request-problem-storage`, `request-problem-too-large-encoded`, `request-problem-too-large-stored`, `request-problem-internal` | 两份 | ERR-1600–1608；size keys 参数固定 `{ $limit }`、`{ $observed }` |
| `response-preview-truncated`, `response-decoding-unsupported`, `response-viewer-mode-unavailable`, `response-viewer-invalid-json`, `response-viewer-invalid-image`, `response-image-too-large`, `response-save-complete`, `response-save-failed` | 两份 | projection / Save；不得传 URL、完整 path 或 source 文本 |
| `response-view-auto`, `response-view-text`, `response-view-json`, `response-view-xml`, `response-view-hex`, `response-view-base64`, `response-view-image` | 两份 | viewer mode |

`foundation::i18n::REQUIRED_REQUEST_KEYS` 纳入全部新 key。两份 locale 的 key 集合与每个 key 的变量集合
必须一致；本轮无 bundle 可见文案，macOS `InfoPlist.strings` 不变。

## 工作包

### [x] WP-1600：依赖、Tokio 初始化与核心数据

**文件：** F-1600–F-1602、F-1609–F-1610。
**前置：** D-1600–D-1606、L-1600。

1. 增加精确 direct dependencies，通过 Cargo 更新 lockfile，并在创建 `RequestView` 前调用
   `gpui_tokio::init(cx)`。
2. 建立脱敏 `ResponseData`、`StoredBody`、`ResponseReadLease`、`RequestProblem` 与固定 limit 常量。
3. 补 Debug/redaction、Arc/read lease 与 Memory/TempPath 生命周期 pure tests。

**完成条件：** response 值、header value、body 与路径不会通过 Debug/error/tracing 泄漏；read lease 是
临时文件异步读取的唯一入口。

### [x] WP-1601：request body replay 与手工 redirect

**文件：** F-1605–F-1608。
**前置：** WP-1600、C-1600、ERR-1600–ERR-1608。

1. 构造显式关闭自动 redirect/decode/referer 的长期 Client，并从每个 `PreparedBody` 产生可重放 body。
2. 实施 generated/explicit headers 顺序、Postman-compatible GET rewrite、五种 redirect status、relative
   Location、loop/hop、cross-origin Host/Cookie/Authorization 与 single total timeout。
3. 用 local TCP fixture 覆盖 method、重复 headers、Text/UrlEncoded/Binary/Multipart、文件在 prepare 后失效、
   302 disabled、redirect replay 与 timeout。

**完成条件：** 每个 hop 都只消费 frozen `PreparedRequest`，文件 stream 不复用，显式 Content-Type 与 Auth
契约不被 transport 改写。

### [x] WP-1602：response stream、content decoding 与 collector

**文件：** F-1608、F-1611–F-1612。
**前置：** WP-1601、L-1602。

1. 实施 head-first `bytes_stream -> encoded counter -> StreamReader/decoder -> stored counter -> collector`。
2. 实施 8 MiB spill-before-write、50 MiB encoded/stored 双 cap、`spawn_blocking` temp creation 与全部失败
   cleanup。
3. 覆盖 gzip/br/zlib-deflate/zstd、多层逆序、未知/非法 coding 原样保存、decoder failure、解压炸弹、
   204/HEAD/empty、中途断流与阈值边界。

**完成条件：** 只有 EOF/flush/close/limit 全成功才产生 `StoredBody`；所有失败均无部分 body。

### [x] WP-1603：私有 Transition、event bridge 与页面生命周期

**文件：** F-1603–F-1605、F-1608、F-1615。
**前置：** WP-1601–WP-1602、L-1601、ST-1600。

1. 实施完整合法表、final-state-before-drop 与脱敏非法消息；不创建预定义 Operation 或 mirrored state。
2. 按 gate→prepare→lazy Task→Start→notify→首次 poll 启 worker 的顺序接通 Send，并接通可靠 head/
   terminal、节流 progress、Cancel 与 Clear。
3. GPUI tests 覆盖 prepare 失败保留 terminal、accepted Send 清旧 Ready、运行中重复 Send 不再验证/不建
   Task、head 先到、Cancel→Idle、迟到 payload 清理、owner drop 与 channel/Join 异常。

**完成条件：** 任意时刻最多一个 active worker，唯一 Task 在 runtime 中，Form 运行中仍可编辑。

### [x] WP-1604：Response pane、Headers 与有界文本 viewer

**文件：** F-1603、F-1609、F-1612–F-1613、F-1615。
**前置：** WP-1600、WP-1603、ST-1601、L-1603。

1. 组合 `v_resizable`、状态 summary、Progress、Body/Headers tabs、mode Select、Alert 与 escaped header table。
2. 实施 Auto/MIME/charset matrix、2 MiB/50,000 行 source+output cap、只读 Editor、Text/JSON/XML/Hex/Base64 与截断/
   fallback warning。
3. pure/GPUI tests 覆盖 `+json/+xml`、BOM/charset、binary header、JSON pretty overflow、HTML/Markdown/SVG
   不执行、Editor 禁写但可选择复制、生命周期 teardown 和非 Ready 无 body viewer。

**完成条件：** head 在 Receiving 即可查看，body 只在 Ready 查看；任何 viewer failure 不改变 runtime。

### [x] WP-1605：有界图片与完成后 Save

**文件：** F-1603、F-1613–F-1615。
**前置：** WP-1604、D-1606。

1. 按格式选择 PNG/JPEG/GIF/WebP decoder，对 source、strict dimensions、checked canvas bytes、最多
   16 帧与 64 MiB 累计 RGBA budget 做解码前保留；原地 RGBA→BGRA 后生成
   `Arc<RenderImage>`，用 Arc identity 丢弃迟到 projection。
2. 实施 `ResponseSaveController`的单 Task 准入，system save panel 取消/错误、Arc/read lease 精确长度复制、
   同目录 staging 和 flush/close/persist；允许新 Send/Clear 后旧 Save 继续，页面 drop 则取消
   并清 staging。
3. 图片测试覆盖 source 上限、尺寸与乘法溢出、预算恰好边界/超界、GIF/WebP 多帧、第 17 帧
   和 SVG exclusion；Save 测试覆盖 picker cancel/error、Memory/TempFile 精确 bytes、短读/增长、
   overwrite-confirmed persist、copy/flush/persist failure、Save across Clear/new Send 与 non-Ready/重入禁止。

**完成条件：** 图片的应用持有解码像素受硬预算约束，decoder `max_alloc` 不被当作唯一保证；
Save 只导出精确长度的完整 representation，不留下部分目标。

### [x] WP-1606：Fluent 文案与安全诊断

**文件：** F-1604、F-1616–F-1618。
**前置：** WP-1603–WP-1605、ERR-1600–ERR-1608。

1. 增加两份同构 locale key/变量，更新 required-key contract。
2. 把 RequestProblem、viewer warning 与 Save result 映射到稳定文案；禁止拼接底层 source、URL、token 或
   path。
3. 覆盖 locale parse/parity/variables 与所有 problem/viewer mapping。

**完成条件：** 所有新 UI 文案都有双语 key；默认日志与 UI 只包含安全类别/计数。

### [x] WP-1607：集成清理、验证与文档回填

**文件：** F-1600–F-1622。
**前置：** WP-1600–WP-1606。

1. 删除 `prepared`/`prepare_request` 的 dead-code annotation，确认 Send 按钮不再永久 disabled；不删除或
   改写 HTTP-199-02 的 typed Request contract。
2. 执行定向 Cargo gate、格式、文档/locale 检查与残留扫描；测试只使用 local loopback，不访问外网。
3. 已将实现状态、未执行的桌面 UI/live network 边界回填本计划和三个索引；root 完成最终自动化后，
   仅补写实际命令、测试数与残留扫描结果，不改变本计划范围。

**完成条件：** R-1600–R-1611 与 T-1600–T-1611 全部有自动化证据，索引状态与实际代码一致。

## 要求与验证映射

| R-ID | 要求 | T-ID | 自动化证据 |
| --- | --- | --- | --- |
| R-1600 | 只有 accepted `PreparedRequest` 启动 I/O，运行中重复 Send 在 validation/Task/network 前被拒绝 | T-1600 | RequestView gate/lifecycle GPUI tests |
| R-1601 | 手工 redirect 正确重放五种 body、rewrite method/header/auth，timeout 覆盖整个 chain | T-1601 | local TCP redirect/replay/timeout tests |
| R-1602 | final head 可靠先到，progress 有界可丢，terminal 可靠有序 | T-1602 | delayed/chunked body 与 channel congestion tests |
| R-1603 | 8/2/50 MiB 边界、spill、双 cap 与 TempPath/read lease 生命周期正确 | T-1603 | collector/storage boundary tests |
| R-1604 | 自动 content decode 被强制关闭，应用支持/未知 coding 行为正确 | T-1604 | raw headers + decoder fixtures |
| R-1605 | 3xx/4xx/5xx 是 Response；transport/body/redirect/internal failure typed 且脱敏 | T-1605 | local status/error/redaction tests |
| R-1606 | Cancel/Clear 均回 Idle；accepted Send 清旧结果，prepare 失败保留结果 | T-1606 | Transition 全表与 RequestView tests |
| R-1607 | Receiving 只显示 head/progress/Headers，Ready 才显示完整 Body/Save | T-1607 | Response pane GPUI tests |
| R-1608 | Text/JSON/XML/Hex/Base64 的 source/output 均有界，markup 无法逃出 fence | T-1608 | projection pure tests |
| R-1609 | raster decode 受 format/strict dimension/checked frame/RGBA 应用预算约束，SVG 永不进 image path | T-1609 | 边界/溢出/第 17 帧/image decoder tests |
| R-1610 | Save 通过 Arc/read lease 精确长度与 staging 只导出完整 bytes；新 Send 不破坏已开始 Save | T-1610 | picker/短读/storage/atomic persist tests |
| R-1611 | 每个新 UI key 在两 locale 中存在且变量集合相同 | T-1611 | `foundation::i18n` contract tests |

最终自动化门禁命令（已执行）：

```sh
cargo fmt --package http-client -- --check
cargo test -p http-client --bin http-client --all-features --locked
cargo check -p http-client --bin http-client --all-features --locked
cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings
rg -n 'gpui_store|gpui-store|refresh::Operation|repair::Operation|error_for_status|Client::new\(|\.detach\(' app/http-client/src app/http-client/Cargo.toml
rg -n '\.gzip\(|\.brotli\(|\.deflate\(|\.zstd\(' app/http-client/src/features/request
rg -n 'TextView::html|TextView::markdown|fenced_source' app/http-client/src/features/request
rg -n 'TempPath|PathBuf' app/http-client/src/features/request/response app/http-client/src/features/request/transport
git diff --check -- app/http-client Cargo.lock docs/dev/issue-199
```

残留扫描要求：Store/predefined Operation/`error_for_status`/`Client::new`/detach/自动解压 enablement 零命中；
response body 不得进入 Markdown/HTML renderer；`TempPath` 只允许在 `StoredBody`/collector/read lease 私有实现，
`PathBuf` 只允许 request body frozen path 与 Save 用户目标/staging，不得作为 Response 临时文件裸 owner。
`.no_gzip/.no_brotli/.no_deflate/.no_zstd` 是预期配置，扫描的正向 `.gzip(...)` 等必须零命中。

## 完成定义与已知未验证边界

只有 WP-1600–WP-1607、R-1600–R-1611 与 T-1600–T-1611 全部完成，`HTTP-199-03` 才能标记 `Done`：单页面发送唯一 frozen
request；final head 在 body 完成前可见；未完成 bytes 永不成为 Response；存储、解码、文本与图片预算均
受限；Task/TempPath/read lease 生命周期可证明；只查看/保存完整 Response；错误与双语文案契约齐全；
Form API、Store、History、multi-tab、repair 与 `Send and Download` 仍未出现。

### 完成证据

| 证据 | 实际结果 |
| --- | --- |
| 实现提交与 PR | 实现提交 `24e4a9f` 已推送；本计划未创建 PR。 |
| 代码、依赖与文档 | F-1600–F-1622 已按文件图落地；`Cargo.lock` 由 Cargo 更新。 |
| 交付的工作包 | WP-1600–WP-1607 已完成；不扩大到 Form API、Store、History、multi-tab、repair 或 `Send and Download`。 |
| 自动化命令与测试数 | `cargo fmt --package http-client -- --check`、`cargo check -p http-client --bin http-client --all-features --locked`、`cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast`（116 passed）、`cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings` 与 `git diff --check` 均通过；三项禁止面扫描零命中，`TempPath`/`PathBuf` 扫描只命中 `StoredBody`、collector、Save 用户目标和 staging 的预期 owner。local TCP fixture 只绑定本机回环端口。 |
| 手工、打包 app、真实 API 场景 | **未执行**：按本轮范围不进行实际桌面 UI、系统保存面板、打包 app 或 live external network/TLS/proxy endpoint 验收。 |
| owner README、索引与草稿 | 本计划、HTTP Client owner README/index、root Issue #199 索引和 HTTP Client 草稿已同步。 |
| 偏离与未验证边界 | 无已接受的实现偏离；不以编译或自动化测试推断手工场景通过。 |

必须执行 deterministic local transport fixtures 与 GPUI tests。实际桌面交互、真实保存面板体验、打包
app，以及 live external network/TLS/proxy endpoint 验收不在本轮自动化完成门禁内；没有单独授权与实测
时只能登记“未执行”，不能从编译/单测推断为通过。
