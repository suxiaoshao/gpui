# 依赖证据快照（2026-07-29）

## 证据规则

优先级固定为：正式 release/changelog/migration guide -> crate/package metadata -> 发布包
source/examples/tests -> commit/PR compare。没有独立 migration guide 不等于没有 breaking
change；表中必须明确写出替代证据和验证责任。

本页来自只读 `cargo upgrade --dry-run --incompatible --verbose`、`cargo info`、本地 registry
发布包和上游 checkout。它是计划快照，不代表已经修改依赖。

## registry 调查结果

本轮重新执行的是：

```sh
cargo upgrade --dry-run --incompatible --verbose --verbose
cargo upgrade --dry-run --incompatible --verbose --verbose \
  --manifest-path tools/mcp-auth-test-server/Cargo.toml
```

下面列出两个 workspace 在 2026-07-29 返回的全部可采纳直接依赖更新。表中的“无 migration
guide”是调查结果，不是省略：这类包改用正式 release/changelog、发布包
`Cargo.toml`、source diff 和本地调用点作为实施证据。

### proc-macro cluster

| crate | 当前 -> 目标 | 已阅读的正式证据与重要变化 | 本地影响与处理 |
| --- | --- | --- | --- |
| `proc-macro2` | `1.0.106 -> 1.0.107` | [1.0.107 release](https://github.com/dtolnay/proc-macro2/releases/tag/1.0.107) 只有文档改进；MSRV 1.71、默认 `proc-macro` feature 不变；无 migration guide | `app-assets-macros`、`gpui-form-macros`；不需要 API 修改，与 `quote`/`syn` 一起验证 |
| `quote` | `1.0.45 -> 1.0.47` | [1.0.47 release](https://github.com/dtolnay/quote/releases/tag/1.0.47) 只有文档改进；MSRV 1.71、默认 `proc-macro` feature 不变；无 migration guide | 两个 proc-macro owner 继续使用 `quote!`/`quote_spanned!`；不改生成代码 |
| `syn` | `2.0.118 -> 3.0.3` | [3.0.0 release/migration notes](https://github.com/dtolnay/syn/releases/tag/3.0.0) 记录完整 breaking surface；[3.0.3](https://github.com/dtolnay/syn/releases/tag/3.0.3) 只有文档修复；MSRV 1.71，`full`/`extra-traits` 保持 | 高风险独立批次。当前只用 `DeriveInput`、`ItemEnum`、`Fields`、`Type::Path`、`GenericParam::Type`、attribute parser；未使用被重命名的 `BareFn`/`Signature.unsafety`/`Arm.guard`/`Punctuated::pop` 等 API。`TypeParam.default = None` 在新表示下仍是合法操作；必须以两个宏 crate 的 unit/trybuild/展开结果确认 |

Syn 3 的重要 breaking change 包括：`Type::BareFn -> Type::FnPtr`、pointer mutability
建模变化、所有 `Type` variant 携带 attributes、closure token 字段重命名、
`Arm.guard -> Pat::Guard`、`Signature.unsafety -> Safety`、receiver/generic default
结构变化、严格 lifetime parsing、移除若干 `From` impl、`Punctuated::pop` 语义变化以及
visit 不再遍历 `Span`。本地当前调用不命中这些接口，所以计划不是预写兼容层，而是先让
compiler 和 trybuild 证明“无需源码适配”；若出现错误，只在对应宏 owner 内按 3.0
release notes 修正。

### serialization、async runtime 与公共基础包

| crate | 当前 -> 目标 | 已阅读的正式证据与重要变化 | 本地影响与处理 |
| --- | --- | --- | --- |
| `serde` | `1.0.228 -> 1.0.229` | [1.0.229 release](https://github.com/serde-rs/serde/releases/tag/v1.0.229)：derive 内部更新到 Syn 3；MSRV 1.56；无 migration guide | 更新所有六个直接 manifest owner；现有 derive/wire shape 不变，跑 core/DB/config/MCP serialization fixtures |
| `serde_json` | `1.0.150 -> 1.0.151` | [1.0.151 release](https://github.com/serde-rs/json/releases/tag/v1.0.151)：新增 `RawValue::from_string_unchecked`；MSRV 1.71；无 migration guide | 本仓库不用新 unsafe constructor；provider raw JSON、tool structured output、theme/config fixtures必须字节/结构等价 |
| `thiserror` | `2.0.18 -> 2.0.19` | [2.0.19 release](https://github.com/dtolnay/thiserror/releases/tag/2.0.19)：derive 更新到 Syn 3；MSRV 1.71；无 migration guide | 更新全部 error owner；`Display` 文本和 `source()` chain 必须保持，禁止借升级重写错误模型 |
| `async-trait` | `0.1.89 -> 0.1.91` | [0.1.91 release](https://github.com/dtolnay/async-trait/releases/tag/0.1.91)：Syn 3 和 by-reference receiver mutability 修复；MSRV 1.71；无 migration guide | `jaco-agent`/`app/jaco` 的 persistence、tool、MCP trait；覆盖 `Send` future、`&self`/`&mut self` receiver 和 mock impl |
| `futures` | `0.3.32 -> 0.3.33` | [0.3.33 release](https://github.com/rust-lang/futures-rs/releases/tag/0.3.33)：修复 `ReadLine` exception-safety、`IterPinRef`/`Iter` 不正确 `Send`、stacked-borrows、`FuturesUnordered::IntoIter` leak；MSRV 1.71 | `jaco-agent` stream/tool loop 与 `novel-download` crawler；无 API 迁移，重点跑取消、stream 结束与并发爬取测试 |
| `tokio` | `1.52.3 -> 1.53.1` | [1.53.1 changelog](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/CHANGELOG.md)：Windows signal MSRV 修复；1.53.0 调整 mpsc receiver drop/wakeup、time/runtime/io；MSRV 1.71 | 保留每个 owner 当前最小 feature 集；覆盖 jaco-agent cancellation/tool timeout、MCP transport、gpui-tokio runtime 和测试 server shutdown |
| `tokio-util` | `0.7.18 -> 0.7.19` | [0.7.19 changelog](https://github.com/tokio-rs/tokio/blob/tokio-util-0.7.19/tokio-util/CHANGELOG.md)：CancellationToken `PartialEq/Eq`、DropGuard/DelayQueue 修复等；MSRV 1.71 | 仍只使用当前直接 features；覆盖 persistence cancellation 与 runtime guard，不采用新 API重构 |
| `time` | `0.3.49 -> 0.3.54` | [0.3.54 changelog](https://github.com/time-rs/time/blob/v0.3.54/CHANGELOG.md)：`PrimitiveDateTime`/`Duration` 新名称并保留非 deprecated alias、解析修复；MSRV 1.88 | 本地用 `OffsetDateTime`、`Date`、`Month`、`Weekday`，不命中 rename；保留 `formatting/parsing/serde/local-offset`，验证 DB mapping、格式化和 serde |
| `toml` | `1.1.2 -> 1.1.4` | [1.1.4 changelog](https://github.com/toml-rs/toml/blob/toml-v1.1.4/crates/toml/CHANGELOG.md)：跨格式反序列化时保留 `Value::Datetime`；package metadata 为 `1.1.4+spec-1.1.0`，MSRV 1.85 | Jaco config/layout 与 xtask manifest 都是 typed serde；验证 parse/pretty round-trip，不引入 `toml::Value` 中转 |
| `uuid` | `1.23.3 -> 1.24.0` | [1.24.0 release](https://github.com/uuid-rs/uuid/releases/tag/v1.24.0)：新增向 `MaybeUninit` buffer 编码；MSRV 1.85，`serde,v7` 保持 | Jaco 只调用 `Uuid::now_v7().to_string()`；不采用新 API，验证格式、唯一性和 serde |

### app、搜索与 xtask 包

| crate | 当前 -> 目标 | 已阅读的正式证据与重要变化 | 本地影响与处理 |
| --- | --- | --- | --- |
| `regex` | `1.12.4 -> 1.13.1` | [CHANGELOG](https://github.com/rust-lang/regex/blob/1.13.1/CHANGELOG.md)：1.13.0 新增 `regex!`，1.13.1 修复 reverse suffix/inner optimization 造成的错误 match offset；MSRV 1.65 | Feiwen 只有整数整串校验，不消费 match offset；保留 `Regex::new`，不为单一冷路径改宏 |
| `rust-embed` | `8.11.0 -> 8.12.0` | 发布包 [changelog](https://docs.rs/crate/rust-embed/8.12.0/source/changelog.md)：修复 binary 重复嵌入、undefined env、folder path、cross-compile host/target，并新增可选 compressed data；MSRV 1.80 | Jaco 只启用 `interpolate-folder-path` 和 `RustEmbed::{get,iter}`；不启用 `compression`，直接获得现有修复并验证 dev/release asset lookup |
| `xcap` | `0.9.6 -> 0.9.7` | [0.9.7 release](https://github.com/nashaofu/xcap/releases/tag/v0.9.7)：更新依赖并修补 xcb 安全问题；未声明 MSRV，无 migration guide | Jaco 只在 macOS/Windows 使用 `Monitor::all/capture_region`；覆盖显示器解析、裁剪和权限错误，并由 Linux check 审查 target graph |
| `globset` | `0.4.18 -> 0.4.19` | [发布源码 compare](https://github.com/BurntSushi/ripgrep/compare/0b0e013f5ac6ae1dbfdf97f6f6aaa27d7c9bc317...28c687d630f1491ccdea078a9be56b4cdc124519)：主要修复 `GlobSet::matches_all`；无 crate migration guide | Jaco 只用单个 `Glob::compile_matcher().is_match()`，不命中修复；保留 glob path fixture |
| `grep-matcher` | `0.1.8 -> 0.1.9` | [发布源码 compare](https://github.com/BurntSushi/ripgrep/compare/a5ba50ceaf01908fed077eb914a84bd02e016a70...41e0ae702bcbd47b093a33ee73f1497aa4ed20d6)：public source 无 API diff，测试/版本同步；无 crate migration guide | 保持 `Matcher::find_iter` 和 range 顺序；与整个 grep stack 原子更新 |
| `grep-searcher` | `0.1.16 -> 0.1.17` | [发布源码 compare](https://github.com/BurntSushi/ripgrep/compare/86e0ab12eff635bd924e3f92bd01be3545eac7b5...576005b322c41174bb21d76122b1411e5d983402)：实现主要为 mmap/文档修正；无 crate migration guide | 保持自定义 `CollectSink` 的 match/context/context-break 顺序，验证 CRLF、before/after context、截断和 binary/UTF-8 loss |
| `ignore` | `0.4.26 -> 0.4.31` | [ripgrep 15.2 changelog](https://github.com/BurntSushi/ripgrep/blob/15.2.0/CHANGELOG.md)：多目录 gitignore 修复与大型目录 traversal 性能改进；[发布源码 compare](https://github.com/BurntSushi/ripgrep/compare/82313cf95849bfe425109ad9506a52154879b1b1...59e318f5ace48db54f37bb67c152535bc17fa153)；MSRV 1.88 | Jaco `WalkBuilder` 的 hidden/git_ignore/parents 语义直接受影响；增加 nested `.gitignore`、parent ignore、hidden、symlink 与 result limit fixtures |
| `anyhow` | `1.0.100 -> 1.0.104` | [1.0.104 release](https://github.com/dtolnay/anyhow/releases/tag/1.0.104)：只更新 Syn 3 dev-dependency；MSRV 1.68；无 migration guide | 独立 MCP test server 继续用 `Context`；error context 文本不变 |
| `axum` | `0.8.7 -> 0.8.9` | [0.8.9 release](https://github.com/tokio-rs/axum/releases/tag/axum-v0.8.9)：MSRV 1.80、MethodRouter CONNECT 修复和 WebSocket subprotocol API；无 0.8 patch migration guide | test server 保留 `default-features=false` 与 `form,http1,json,query,tokio`，不启用 `ws`；验证 OAuth routes/middleware/graceful shutdown |
| `schemars` | `1.1.0 -> 1.2.2` | [1.2.2 release](https://github.com/GREsau/schemars/releases/tag/v1.2.2)：derive 更新到 Syn 3；MSRV 1.74；无 migration guide | test server 的 `EchoRequest: JsonSchema` 和 RMCP tool input schema 必须等价 |
| `clap` | `4.6.1 -> 4.6.4` | [4.6.4 release](https://github.com/clap-rs/clap/releases/tag/v4.6.4)：内部更新到 Syn 3；MSRV 1.85，`derive` 保持 | xtask CLI flags/subcommands/help snapshots；不采用新 CLI surface |
| `plist` | `1.9.0 -> 1.10.0` | [1.10.0 changelog](https://github.com/ebarnard/rust-plist/blob/v1.10.0/CHANGELOG.md)：新增宏、修复 24-bit binary refs，并更新 `quick-xml` 处理 RUSTSEC-2026-0194；MSRV 1.88 | xtask 继续用 typed `Dictionary/Value`，不采用新宏；验证 macOS `Info.plist` 的 key/type 和可解析性 |
| `tauri-bundler` | `2.9.3 -> 2.9.4` | [2.9.4 release](https://github.com/tauri-apps/tauri/releases/tag/tauri-bundler-v2.9.4)：修复 AppImage `.desktop`/`.DirIcon` 相对 symlink；MSRV 1.77.2，默认 rustls/platform-certs | xtask bundle settings/API 无变化；三平台 bundle smoke，Linux 额外检查 AppImage symlink |
| `which` | `8.0.4 -> 8.0.5` | [8.0.5 changelog](https://github.com/harryfei/which-rs/blob/8.0.5/CHANGELOG.md)：absolute query 不再错误搜索 current directory；MSRV 1.70 | xtask 只用 `which(command)` 做普通命令探测；增加 bare command/absolute path/不存在命令 focused test |

### breaking 与 native/database 包

| crate | 当前 -> 目标 | 已阅读的正式证据与重要变化 | 本地影响与处理 |
| --- | --- | --- | --- |
| `base64` | `0.22.1 -> 0.23.0` | [0.23.0 release notes](https://github.com/marshallpierce/rust-base64/blob/v0.23.0/RELEASE-NOTES.md)：新增 custom padding 和 SIMD engine；`simd-unsafe` 成为默认 feature；MSRV 1.71 | Jaco 只有 `general_purpose::STANDARD.encode`，该常量仍是 scalar `GeneralPurpose`。manifest 改为 `default-features = false, features = ["std"]`，不引入未使用的 unsafe SIMD；验证 image/document data URI 的 padded output |
| `duckdb` | `1.10504.0 -> 1.10505.0` | [1.10505.0 release](https://github.com/duckdb/duckdb-rs/releases/tag/v1.10505.0)：DuckDB 1.5.5、新 type/nested Arrow、panic containment；breaking 为 `Type/Value/ValueRef` non-exhaustive、`Statement::step` 与 Arrow stream API；package MSRV 1.85.1 | Feiwen 不用 `Statement::step`/Arrow；现有 `Value` matches 都有 catch-all，预计无需源码修复。保留 `bundled,parquet,r2d2`，单独执行 schema 初始化、query/list conversion、fetch insert 和三平台 native build |
| `diesel` | `2.3.10 -> 2.3.11` | [2.3.11 release](https://github.com/diesel-rs/diesel/releases/tag/v2.3.11)：修复 SQLite batch insert + returning，并 harden read-only deserialize；MSRV 1.86；同系列无 migration guide | 保留 `sqlite,r2d2,time,returning_clauses_for_sqlite_3_35,serde_json`；运行 schema compile、migration harness、repository tests；本依赖批次不改 schema |
| `libsqlite3-sys` | 保留 `0.37.0` | [Diesel 2.3.11 package dependency](https://docs.rs/crate/diesel/2.3.11/source/Cargo.toml)：非 WASM SQLite 依赖要求 `<0.38.0`；该 crate 还声明 `links = "sqlite3"`，不能让 `0.37`/`0.38` 并存 | 不采用原候选 `0.38.1`；保留 `bundled-windows`，确认唯一 `links=sqlite3`、系统/捆绑选择和三平台 CI；不改 Diesel schema/migration |

`cargo upgrade` 还报告：

- `rig-core 0.39.0 -> 0.41.0` 与 `rmcp 1.8.0 -> 3.0.0`。Rig 独立批次采用正式
  `rig 0.41.0` facade，并把直接 RMCP 只更新到 Rig 同版本图使用的 `2.2.0`，不能机械
  接受 3.0；
- `winresource` 在现有宽泛 `0.1` requirement 内有兼容解析更新，但不需要改 direct
  manifest requirement；只在 lockfile 审查中记录实际解析；
- 其余直接依赖在本次 dry-run 中没有可采纳更新，或属于 Git/local dependency。计划不
  为制造“全量升级”而重写未变化的版本。

## `gpui-component` / Zed 证据

| 项目 | 当前 | 已审计目标 | 证据与结论 |
| --- | --- | --- | --- |
| gpui-component cluster | `5b45bcb26b9343d91a123a4d5ed8a654360512e5` | `57a9903f48160845aabc8b92a1e2f5348c80d439` | [commit compare](https://github.com/longbridge/gpui-component/compare/5b45bcb26b9343d91a123a4d5ed8a654360512e5...57a9903f48160845aabc8b92a1e2f5348c80d439)、source、stories/docs 和 tests；上游 release 信息不足，不能代替源码审计 |
| Zed GPUI | `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | 目标由 gpui-component `Cargo.lock` 解析；本次无需移动 Zed GPUI |

目标 compare 共 21 个 commit，与本仓库直接相关的已知变化：

- Input 新增 `TextDecoration`、`TextDecorationCollection` 与
  `InputState::create_decorations_collection`，range 使用 UTF-8 byte offset、会随 edit
  调整，并提供 `set/append/clear/get_ranges`；
- PopupMenu 增加显式 `rebuild` 能力，并修复通过 `item() + PopupMenuItem::submenu()`
  延迟挂接 submenu 时的 parent/priority wiring；
- horizontal `ScrollableMask` 改为 viewport sibling，并在 capture phase 按主轴处理 wheel，
  避免 nested list 抢占横向滚动，同时尊重 occlusion；
- Select 新增可复用 `Caret`，并调整 trigger/popover visual；Input、Combobox、button、tab、
  text 和 native menu 也有 API/行为变化；
- Chart 新增 `RadarChart`、radial plot/tooltip；组件文档只修改 `chart.md`，无新增/删除；
- `skills/gpui` 在两个 commit 之间没有上游 A/M/D；本地镜像存在 5 个 reference 的空行漂移，
  实施同步时仍须消除。

这不是“release note 已保证兼容”的升级。执行者必须先以锁定目标跑 source/API compare 和
workspace tests，再依据 [upstream-reuse-audit.md](upstream-reuse-audit.md) 删除或保留本地实现。

## Lucide 证据

- 当前 submodule 精确 tag：`1.21.0`。
- 本地已获取的同系列稳定候选：`1.25.0`；同时存在 `v0.x` tag，不能只按全局 semver 排序
  自动选择另一发布线。
- 发布证据：[1.21.0...1.25.0 compare](https://github.com/lucide-icons/lucide/compare/1.21.0...1.25.0)
  和 [1.25.0 tag source](https://github.com/lucide-icons/lucide/tree/1.25.0)；实施时按
  `icons/*.svg` 实际 A/M/D 审计，不以 changelog 代替资源 diff。
- 没有 Rust API migration；风险是 SVG slug rename/delete、图形变化和 asset lookup。

## Rig 0.41 正式发布证据

2026-07-29 的实查结果：

| 项目 | 证据 | 结论 |
| --- | --- | --- |
| crates.io package | `cargo info rig@0.41.0`、`rig-core@0.41.0`、`rig-agent@0.41.0` | 三个 package 均为正式 `0.41.0`，非 git 依赖 |
| source identity | 三个发布包 `.cargo_vcs_info.json` 均为 `68b4eabb8c9cf749ca73c917b9306e97fb0eda24` | facade、core、agent 来自同一源码提交 |
| 正式 release/tag | [rig-v0.41.0](https://github.com/0xPlaygrounds/rig/releases/tag/v0.41.0) | 2026-07-28 发布，非 draft/pre-release |
| migration guide | [MIGRATING.md@v0.41.0](https://github.com/0xPlaygrounds/rig/blob/v0.41.0/MIGRATING.md) | 0.38 到 0.41 的 breaking change 和 silent behavior change 均有明确说明 |
| 前一版本 | [rig-v0.40.0](https://github.com/0xPlaygrounds/rig/releases/tag/v0.40.0) | 本仓库从 0.39 升级，必须同时覆盖 0.40 与 0.41 |
| 发布源码/API | [v0.41.0 source](https://github.com/0xPlaygrounds/rig/tree/v0.41.0)、[rig 0.41 docs](https://docs.rs/rig/0.41.0/rig/) | 具体 public API 以发布包 source/tests 为准，不以 merged PR 猜测 |

### 0.40 / 0.41 与 Jaco 直接相关的变化

| 上游变化 | 当前 Jaco 调用点 | 计划结论 |
| --- | --- | --- |
| core/agent 拆包并由 `rig` facade 重导出 | 根 `Cargo.toml`、`crates/jaco-agent` 全部 `rig_core::*` import | 直接依赖改为 `rig`；生产 features 显式开启 `agent,rmcp,reqwest,rustls,websocket` |
| `AgentRunner` 成为统一执行路径 | `runtime.rs` 的 `AgentBuilder`/stream/prompt 链 | 保持高层 Agent API，但迁移 `.history()`、`.tool_concurrency()`、`.add_hook()`；不复制 runner |
| `max_turns` 精确定义为总 model call 数 | `RunGuards.max_steps` 与 `max_steps_is_persisted_as_max_steps_stop` | 保持 `max_steps -> max_turns` 1:1，不再按旧 `n+2` 行为解释 |
| `AgentHook` 拆成 event-specific methods | `PersistingPromptHook` | 改为 `PersistingAgentHook: AgentHook`，按 completion/tool/stream event 分工 |
| `ToolDyn` public API 移除 | `tools.rs` 的 `RigToolExecutor`、`RegisteredRigTool` | 删除兼容层；本地 tool 用 `DynamicTool`，直接传 `ToolOutput`/`ToolExecutionError` |
| tool call 加入 `ToolContext` 与 structured result hook | `tools.rs` 字符串序列化/反解析、persistence hook | 通过 `ToolContext` 携带 Jaco/MCP metadata；结果只持久化一次，不再从字符串猜 structured JSON |
| RMCP tool 由 AgentBuilder 注册 | `mcp.rs`/`mcp/connector.rs` 直接使用旧 public `McpTool` | Jaco 保留 session/OAuth/审批；保存 `rmcp::model::Tool + ServerSink + timeout`。先用 `dynamic_tools` 进入 `WithBuilderTools`，再用该 typestate 上可用的 `rmcp_tools_with_timeout(vec![tool], server, timeout)` 注册 |
| Responses system message 变成顶层 instructions | 当前每次传完整 Rig history | continuation 增量请求仍必须重发当前 preamble/leading system instructions |
| streaming 增加 `Unknown(Value)` | `runtime.rs` 对 `StreamedAssistantContent` 的 match | 不丢弃；保存到 provider response snapshot，不新增无语义的 chat entry |
| provider error/unknown output 保留 raw JSON | 当前多处只用 `error.to_string()` | 用 `provider_response_json/status/body` 做结构化分类，raw provider output 保留在 audit snapshot |
| GPT-5.6 constants 与 typed reasoning | 当前 raw JSON `reasoning_additional_params` | 使用 `Reasoning`、`ReasoningEffort`、`ReasoningMode`、`ReasoningContext`；产品层不自建字符串 enum |
| Responses WebSocket session | 当前只有 `CompletionModel::stream` 的 HTTP/SSE | 复用 `ResponsesWebSocketSession` 的 connect/send/next_event/close/previous-id 状态机；Jaco 只补 pool 和 Rig stream decoder |

### Rig feature 与依赖目标

根 workspace 使用：

```toml
rig = { version = "0.41.0", default-features = false, features = [
  "agent",
  "rmcp",
  "reqwest",
  "rustls",
  "websocket",
] }
rmcp = { version = "2.2.0", features = [
  "auth",
  "client",
  "macros",
  "transport-child-process",
  "transport-streamable-http-client-reqwest",
] }
```

`crates/jaco-agent` dev dependency 通过 `rig = { workspace = true, features = ["test-utils"] }`
扩展测试能力。`derive` 不启用，因为本仓库没有使用 Rig derive；`tokio-tungstenite` 不作为
直接依赖，交给 Rig 的 `websocket` feature。WebSocket decoder 若使用
`async_stream::stream!`，新增精确依赖 `async-stream = "0.3.6"`。

## RMCP 2.2 正式发布证据

| 项目 | 证据 | 结论 |
| --- | --- | --- |
| package | `cargo info rmcp@2.2.0` | Rig 0.41 的 `rmcp` feature 使用同一 `2.2.0`；虽然 3.0 已发布，本工作包不得越级 |
| source identity | 发布包 `.cargo_vcs_info.json` 为 `519577601db3823616dbd7c4eb84ed569d8e17d4` | 审计锁定发布源码 |
| 1.8 -> 2.0 | [compare](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-1.8.0...rmcp-2.0.0) | model types 对齐 MCP 2025-11-25，`structuredContent` 等 wire model 有 breaking change |
| 2.0 -> 2.1 | [compare](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-2.0.0...rmcp-2.1.0) | 重点检查 cancel-safe transport 与 auth 修复 |
| 2.1 -> 2.2 | [compare](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-2.1.0...rmcp-2.2.0) | 重点检查 conformance、取消/孤儿 stream、S256 PKCE enforcement |

Jaco 现有 `McpSessionManager`、配置、OAuth token、list-change UI 和 tool approval 是产品
所有权，不能因 Rig 也有 MCP client handler 而整体替换。可删除的仅是旧
`rig_core::tool::rmcp::McpTool` 转换和把 `CallToolResult` 压成字符串再反解析的层。

## OpenAI 官方 API 证据

- [GPT-5.6 model guidance](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-5.6)：
  2026-07-29 实时核对时 `/guides/model-guidance` 会重定向到该 canonical 路径；
  alias 指向 Sol；family 为 Sol/Terra/Luna；effort 为
  `none/low/medium/high/xhigh/max`，默认 `medium`；GPT-5.6 默认
  `reasoning.context=all_turns`，响应会返回 effective context。
- [WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)：同一连接
  单次只允许一个 in-flight response，最长 60 分钟；后续请求必须只发送新 input 和
  `previous_response_id`；连接外 ID 在 `store=true` 时可从持久状态恢复，找不到时返回
  `previous_response_not_found`。
- [Conversation state](https://developers.openai.com/api/docs/guides/conversation-state)：
  Responses 默认保存 30 天；使用 `previous_response_id` 时，链上历史 input token
  仍然计费。
- [Prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching) 与上述
  GPT-5.6 migration quickstart：implicit caching 不要求改代码；新的 explicit contract 是
  `prompt_cache_options.mode/ttl`，旧 `prompt_cache_retention` 应停止新增。#172 保持
  implicit，只记录 provider usage 中的 cached/cache-write tokens。
- [Images and vision](https://developers.openai.com/api/docs/guides/images-vision)：GPT-5.6
  对 `auto`/`original` 保留原始尺寸，可能提高 input token/latency；Jaco 保留现有
  `ImageDetail::Auto`，不新增设置，但在 smoke usage 中记录影响。
- [File inputs](https://developers.openai.com/api/docs/guides/file-inputs)：PDF 继续作为
  typed file/document input；Rig 0.41 document type 没有独立 PDF detail，本轮不发明 raw
  参数或把 PDF 转为逐页图片。

因此计划显式发送 `store=true`，continuation 本地 TTL 取 30 天，并在连接达到 55 分钟
时于下一次请求前主动重连。TTL 只避免已知过期请求，不代替 provider 的结构化失效判断。
