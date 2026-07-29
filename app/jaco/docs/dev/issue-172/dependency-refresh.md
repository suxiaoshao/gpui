# 依赖与 submodule 更新计划

## 边界与执行规则

本计划覆盖通用 registry crate、Git cluster、submodule，以及独立的 Rig/RMCP 原子批次。
所有依赖工作包必须先于 GPT-5.6 自定义运行时适配完成。Rig/RMCP 不与通用 registry
批次混改，避免无法判断 breaking change 来源。

实施时每个工作包单独产生可解释的 manifest/lockfile diff。联网、lockfile、submodule
改写和批量更新按仓库规则先申请权限。本文件不授权当前修改代码。

目标版本和上游证据见 [dependency-evidence.md](dependency-evidence.md)。如果实施时目标
版本已变化，只允许刷新对应行及其影响分析，不能直接升级到未经审计的新版本。

## DR-10：serialization、async runtime 与公共基础包

本批次只更新 semver-compatible 直接依赖，不改本地 public API，也不借机把 member-local
依赖集中到 workspace。所有直接声明必须更新到同一个已审计版本，保留各 owner 已选择的
feature：

| 依赖 | 当前 -> 目标 | 直接 manifest owner | 必须保留/验证 |
| --- | --- | --- | --- |
| `serde` | `1.0.228 -> 1.0.229` | `tools/mcp-auth-test-server`、`crates/xtask`、`app/jaco`、`crates/jaco-{core,db,agent}` | `derive` 只在当前 owner 开启；payload/config/DB/MCP wire shape |
| `serde_json` | `1.0.150 -> 1.0.151` | test server、`app/jaco`、`crates/app-theme`、`crates/jaco-{core,db,agent}` | provider raw JSON、tool structured output、theme/config fixture |
| `thiserror` | `2.0.18 -> 2.0.19` | 四个 app、`xtask`、`jaco-conversation`、`window-ext`、`platform-ext`、`jaco-db`、`jaco-agent` | error `Display` 和 `source()` chain；不重写 error enum |
| `async-trait` | `0.1.89 -> 0.1.91` | `app/jaco`、`crates/jaco-agent` | persistence/tool/MCP trait object 与 `Send` future |
| `futures` | `0.3.32 -> 0.3.33` | `app/novel-download`、`crates/jaco-agent` | crawler concurrency、agent stream end/cancel |
| `tokio` | `1.52.3 -> 1.53.1` | test server、`gpui-tokio`、`app/jaco`、`jaco-agent` normal/dev | 每个 owner 的现有最小 features；mpsc、timeout、process、shutdown |
| `tokio-util` | `0.7.18 -> 0.7.19` | test server、`jaco-agent` | cancellation token 的 clone/cancel/terminal ordering |
| `time` | `0.3.49 -> 0.3.54` | `app/jaco`、`jaco-core`、`jaco-db`、`jaco-agent` | `formatting/parsing/serde/local-offset` 原样；DB UTC/time string 语义 |
| `toml` | `1.1.2 -> 1.1.4` | `app/jaco`、`crates/xtask` | typed parse/pretty round-trip；不新增 `toml::Value` 中转 |
| `uuid` | `1.23.3 -> 1.24.0` | `crates/jaco-core` | `serde,v7`；`new_id()` 字符串格式 |

### 数据流和 API 约束

- Jaco config/layout：disk TOML -> typed serde model -> validation -> atomic save 的路径不变；
- conversation/provider/tool：typed payload <-> JSON <-> SQLite 的结构不变，golden fixture
  必须证明 field name、null/omitted 和 enum tag 没变化；
- runtime：Tokio channel/cancellation 仍由现有 owner 创建和 drop，不采用 1.53/0.7.19
  新 API 重构任务所有权；
- `time` 继续使用 `OffsetDateTime` 等现有名称，不为上游 alias rename 做无意义重命名；
- 本批次不修改 UI、icon、i18n、数据库 schema 或 migration，也不新增依赖。

实施时先修改所有 direct requirements，再用精确 package update，禁止只靠 lockfile 把
manifest 留在旧版本：

```sh
cargo update -p serde@1.0.228 --precise 1.0.229
cargo update -p serde_json@1.0.150 --precise 1.0.151
cargo update -p thiserror@2.0.18 --precise 2.0.19
cargo update -p async-trait@0.1.89 --precise 0.1.91
cargo update -p futures@0.3.32 --precise 0.3.33
cargo update -p tokio@1.52.3 --precise 1.53.1
cargo update -p tokio-util@0.7.18 --precise 0.7.19
cargo update -p time@0.3.49 --precise 0.3.54
cargo update -p toml@1.1.2 --precise 1.1.4
cargo update -p uuid@1.23.3 --precise 1.24.0
cargo check -p jaco-core -p jaco-db -p jaco-agent -p jaco -p gpui-tokio
cargo test -p jaco-core -p jaco-db -p jaco-agent -p jaco -p gpui-tokio
cargo check -p novel-download -p xtask
cargo check --manifest-path tools/mcp-auth-test-server/Cargo.toml
cargo test --manifest-path tools/mcp-auth-test-server/Cargo.toml
```

`tools/mcp-auth-test-server` 有自己的 `[workspace]` 和 `Cargo.lock`，不能从根 workspace
用 `-p` 选择；本批次必须同步更新它的四个公共依赖声明和独立 lockfile，但其 `rmcp`
保持 1.8.0，留给 DR-60。完成条件是十个目标版本在各自 workspace 中达到计划值、direct
feature 不漂移、focused fixtures 通过，并且两个 workspace 的 `rig-core`/`rmcp` 都
保持原版本。

## DR-20：proc-macro / Syn 3 breaking cluster

### Manifest 与 owner

| manifest | 变化 |
| --- | --- |
| `crates/app-assets-macros/Cargo.toml` | `proc-macro2 1.0.107`、`quote 1.0.47`、`syn 3.0.3`，保留 `syn/full` |
| `crates/gpui-form-macros/Cargo.toml` | 同上，保留 `syn/full,extra-traits` |

两个 crate 的输入/输出契约不变：

- `define_lucide_icons!` / `define_svg_icons!` 仍解析 enum/attributes 并生成同一
  `IconName`/asset metadata trait impl；
- `#[derive(FormStore)]` 仍生成现有 form store、field enum、schema/path mapper、
  validation/transform impl 和 methods；不得因为 Syn 3 改生成 API；
- 当前源码没有使用 Syn 3 release notes 中重命名或移除的 `BareFn`、`Arm.guard`、
  `Signature.unsafety`、literal conversion、visitor span 或 `Punctuated::pop`；
- `gpui-form-macros/src/derive/expand.rs` 对 generic type default 只赋值 `None`，新
  `Option<(=, Type)>` 表示仍能表达这一操作；若 compiler 证明还有影响，只修改这个 owner，
  不添加跨版本 shim。

执行与验证：

```sh
cargo update -p proc-macro2@1.0.106 --precise 1.0.107
cargo update -p quote@1.0.45 --precise 1.0.47
cargo check -p app-assets-macros -p gpui-form-macros
cargo tree -p app-assets-macros --depth 1
cargo tree -p gpui-form-macros --depth 1
cargo test -p app-assets-macros
cargo test -p gpui-form-macros
cargo test -p app-assets
cargo test -p gpui-form
```

先把两个本地 manifest 的 direct requirement 改成精确 `syn = "3.0.3"`，再由上述
package-scoped check 解析 lockfile；不得执行
`cargo update -p syn@2.0.118 --precise 3.0.3`，因为根 graph 中仍有第三方 Syn 2 约束，
该命令会错误地尝试跨不兼容 requirement 替换它们。必须核对 trybuild
compile-pass/compile-fail snapshots 和展开后的 public signatures。`cargo tree -d` 允许
显示不受 Jaco 控制的 transitive Syn 2 owner，但两个本地 macro crate 的 depth-1 direct
edge 必须精确为 Syn 3.0.3；不能为了“只剩一个 Syn”去 patch 第三方依赖。

## DR-25：app asset/capture、搜索栈与 xtask 批次

该工作包按下列三个子批次顺序实施，每个子批次单独审查 lockfile 和运行 focused tests。

### DR-25A：Feiwen regex 与 Jaco asset/capture

| 依赖 | owner | 目标与处理 |
| --- | --- | --- |
| `regex` | `app/feiwen` | `1.13.1`；保留现有 `Regex::new(r"^\d+$")` 冷路径，不采用新 `regex!` |
| `rust-embed` | `app/jaco` | `8.12.0`；保留 `interpolate-folder-path`，不启用新 `compression` |
| `xcap` | `app/jaco` | `0.9.7`；不启用 `wgc` 或其他新 feature，保留 macOS/Windows gating |

RustEmbed 上游已直接解决 binary 重复嵌入、undefined env、folder path 和 cross-compile
host/target 问题；本仓库没有对应 workaround，应直接获益，不新增 wrapper。验证
`AssetsInner::{get,iter}`、`BuildAssets`、dev/release asset lookup，以及
`capture_region -> resolve_monitor -> Monitor::capture_region -> ImageFrame` 的现有数据流。
无组件、icon slug、i18n 或 schema 变化。

### DR-25B：Jaco builtin search 原子栈

| 依赖 | 当前 -> 目标 |
| --- | --- |
| `globset` | `0.4.18 -> 0.4.19` |
| `grep-matcher` | `0.1.8 -> 0.1.9` |
| `grep-searcher` | `0.1.16 -> 0.1.17` |
| `ignore` | `0.4.26 -> 0.4.31` |

owner 只允许是 `crates/jaco-agent/src/tools/builtin/{search,filesystem}.rs` 及其测试。调用链
保持：

`resolve_tool_path -> WalkBuilder(hidden,git_ignore,parents) -> optional GlobMatcher ->
RegexMatcher -> Searcher -> CollectSink -> ordered ToolInvocationOutput`

不采用 upstream incremental walker，不重写 tool output type。新增/补齐 fixture：

- root/parent/nested `.gitignore`、negation、hidden file 和 symlink；
- glob include/exclude、CRLF、invalid UTF-8 loss、before/after context；
- `max_results + 1` 截断标记与 deterministic path/range 顺序；
- 被 ignore 的路径不能经 `find_path` 或 recursive list 泄漏。

### DR-25C：xtask

| 依赖 | 当前 -> 目标 | 重点 |
| --- | --- | --- |
| `clap` | `4.6.1 -> 4.6.4` | derive 内部 Syn 3；CLI surface/help 不变 |
| `plist` | `1.9.0 -> 1.10.0` | RUSTSEC-2026-0194 依赖修复和 24-bit refs；不采用新宏 |
| `tauri-bundler` | `2.9.3 -> 2.9.4` | AppImage symlink 修复；bundle API 不变 |
| `which` | `8.0.4 -> 8.0.5` | absolute query 修复；现有 bare command 探测不变 |

验证 `crates/xtask/src/{cli,cmd,manifest}.rs` 和 `bundle/{settings,common,macos}.rs`；比较
生成的 macOS `Info.plist` key/type，Linux AppImage `.desktop`/`.DirIcon` 必须是相对
symlink，Windows bundle 仍能生成。不得借升级修改产品 bundle metadata。

### DR-25D：独立 MCP test-server support crates

`tools/mcp-auth-test-server` 不属于根 workspace，按自己的 manifest/lockfile 更新：

| 依赖 | manifest / lock 基线 -> 目标 | 约束 |
| --- | --- | --- |
| `anyhow` | `1.0.100 / 1.0.102 -> 1.0.104` | `Context` 错误链/文本保持 |
| `axum` | `0.8.7 / 0.8.9 -> 0.8.9` | 只校正 manifest；`default-features=false`，保留 `form,http1,json,query,tokio`，不启用 `ws` |
| `schemars` | `1.1.0 / 1.2.1 -> 1.2.2` | `EchoRequest: JsonSchema` 生成的 tool input schema 等价 |

本子批次不更新 RMCP；`rmcp 1.8 -> 2.2` 由 DR-60 与根 workspace 原子完成。测试
`/health`、OAuth metadata/registration/authorize/token routes、bearer middleware、
streamable HTTP service 和 graceful shutdown。预期无需改 `src/main.rs`；若编译器要求
修改，范围只能是对应 Axum/Schemars API，不能提前做 RMCP 迁移。

先把独立 manifest 的三个 requirement 改成目标完整版本，再按**当前独立 lockfile 中的
package ID** 更新；`axum` 已经解析为目标 `0.8.9`，不能再用不存在的
`axum@0.8.7` 作为 package ID：

```sh
cargo update --manifest-path tools/mcp-auth-test-server/Cargo.toml \
  -p anyhow@1.0.102 --precise 1.0.104
cargo update --manifest-path tools/mcp-auth-test-server/Cargo.toml \
  -p schemars@1.2.1 --precise 1.2.2
```

统一命令：

```sh
cargo check -p feiwen -p jaco -p jaco-agent -p xtask
cargo test -p feiwen -p jaco -p jaco-agent -p xtask
cargo tree -p jaco-agent -e features
cargo check --manifest-path tools/mcp-auth-test-server/Cargo.toml
cargo test --manifest-path tools/mcp-auth-test-server/Cargo.toml
```

## DR-30：`base64 0.23` breaking/default-feature 批次

只修改 `crates/jaco-agent/Cargo.toml`：

```toml
base64 = { version = "0.23.0", default-features = false, features = ["std"] }
```

理由是本地仅调用 scalar `engine::general_purpose::STANDARD.encode`；0.23 的
`simd-unsafe` 虽默认开启，但没有改变这个常量的 engine，也没有当前性能需求。显式关闭
它可避免把未使用的 unsafe SIMD 实现带入 graph。不要改成 `Simd`，不要自建 engine
wrapper。

数据流保持：

`attachment file bytes -> STANDARD.encode -> Rig image/document Base64 payload`

`crates/jaco-agent/src/runtime/history.rs` 继续是唯一 owner；现有 import 和方法预期无需
修改。补齐 known bytes、padding、空文件、image/document 两条输出 fixture，确保 0.22
与 0.23 产物完全相同。无 global、数据库、UI、icon 或 i18n 变化。

```sh
cargo check -p jaco-agent
cargo tree -p jaco-agent -e normal --depth 1
cargo tree -p jaco-agent -e features | rg 'base64|simd-unsafe'
cargo test -p jaco-agent runtime::history
```

先修改 direct manifest，再由 package-scoped check 解析 lockfile；不得用
`cargo update -p base64@0.22.1 --precise 0.23.0`，因为 Arrow、Reqwest、RMCP、plist 等
transitive owner 仍可合法要求 0.22，该命令会把 direct-owner 升级误写成全 graph
强制替换。只要求 `jaco-agent` 的 depth-1 edge 为 0.23.0。

完成条件包括 feature graph 中 `base64/std` 存在且 `base64/simd-unsafe` 不存在。

## DR-35：DuckDB、Diesel 与 SQLite native 隔离批次

三个数据库依赖不共享数据库或 schema owner，必须按 A/B/C 顺序分别更新、验证并审查
lockfile，不能一次执行一个覆盖三者的 `cargo update`。

### DR-35A：Feiwen `duckdb 1.10505.0`

- owner：`app/feiwen/Cargo.toml` 与 `app/feiwen/src/store/**`；
- 保留 `bundled,parquet,r2d2`；
- 本地不调用 breaking 的 `Statement::step`/Arrow stream；`string_list`、
  `optional_i32_list`、`optional_i32` 对 non-exhaustive `Value` 已有 catch-all error，
  因此预期无需代码修复；
- 运行 schema `execute_batch`、fetch insert/update、advanced query、list conversion、
  cached statement/schema change 和 pool tests；检查 bundled DuckDB 1.5.5 的三平台构建；
- 本仓库不采用新 UHUGEINT/Decimal/GEOMETRY/Arrow API，也不复制 panic-containment
  wrapper。

### DR-35B：Jaco `diesel 2.3.11`

- owner：`crates/jaco-db`；
- 保留 `sqlite,r2d2,time,returning_clauses_for_sqlite_3_35,serde_json`；
- 运行 migration harness、`schema.rs` compile、models/records/repository/service tests，
  特别覆盖 SQLite batch insert + returning；
- 这是依赖等价迁移：不新增 migration，不改表/列/索引，不把后续 provider-step fresh
  schema 混入本批次。

### DR-35C：`libsqlite3-sys 0.37.0` 明确保留

- owner：`crates/jaco-db` 的直接依赖及 `cargo-shear` ignored 说明；
- Diesel `2.3.11` 在非 WASM 平台要求 `libsqlite3-sys >=0.17.2,<0.38.0`；
  `libsqlite3-sys` 又以 `links = "sqlite3"` 排斥并存版本，因此本批不得升级到
  `0.38.1`；
- 保留 Windows `bundled-windows`，确认 `cargo tree -i libsqlite3-sys` 只有一个
  `links = "sqlite3"` owner；
- 保持 manifest/lockfile 的 `0.37.0`，验证 pkg-config/vcpkg/bindgen feature 和三平台
  build 没有因 Diesel patch upgrade 漂移；
- 系统依赖若确需调整，只能改 `script/bootstrap` / `script/install-linux.sh`，不能散落
  到 workflow；出现行为性数据差异时停止，不添加兼容 fallback。

```sh
cargo update -p duckdb@1.10504.0 --precise 1.10505.0
cargo check -p feiwen
cargo test -p feiwen

cargo update -p diesel@2.3.10 --precise 2.3.11
cargo check -p jaco-db -p jaco
cargo test -p jaco-db

cargo tree -i libsqlite3-sys
cargo check -p jaco-db -p jaco
cargo test -p jaco-db
```

最终由 macOS/Linux/Windows CI 覆盖 native build。三个子批次都不修改数据库 schema；
GPT-5.6 provider-step 的 fresh schema 只在后续 WP-80 实施。

## DR-40：`gpui-component` / Zed GPUI 原子 cluster

### 选择和锁定

当前 lockfile 基线与已审计目标：

- 基线 `gpui-component`、`gpui-component-assets`、`gpui-component-macros`：
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`；
- 目标三个 package：
  `57a9903f48160845aabc8b92a1e2f5348c80d439`；
- 基线和目标 checkout 的 **`Cargo.lock`** 都解析 Zed GPUI：
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。

上游 `Cargo.toml` 未固定 Zed rev，不能从 manifest 猜目标 SHA。根 `Cargo.toml` 的以下
入口也已经统一为未固定 rev 的 git source：

- `[workspace.dependencies].gpui`；
- `[workspace.dependencies].gpui_platform`；
- `[patch.crates-io].gpui`；
- `[patch.crates-io].gpui-macros`。

因此本批次不改这些 manifest 行，也不添加容易与 lockfile 漂移的 `rev`；只用精确 package
更新让 `Cargo.lock` 的三个 gpui-component package 原子移动到目标 SHA。保留
`gpui_platform` 的 `font-kit/x11/wayland/runtime_shaders` features。完成后 Cargo 图只能
出现一个 Zed commit，三个 gpui-component package 必须来自同一 commit。

### API 修复与回归所有者

| 上游变化 | 本地范围 | 处理要求 |
| --- | --- | --- |
| `TextDecorationCollection` | `crates/gpui-form-gpui-component` 的 `FormInput`/`FormIntegerInput` owning controls；Jaco 普通 Input | 当前没有等价本地 workaround 可删；只暴露上游 state，不给 typed form core 新增 presentation state |
| PopupMenu rebuild/late submenu wiring | `app/jaco/src/app/title_bar_menu.rs`、chat/home/settings menu | 现有 submenu 同步构建继续保留；验证 dismiss/click-outside/keyboard/priority，不为未使用的异步重建新建 app wrapper |
| horizontal scroll mask capture/occlusion | conversation Markdown/table、settings nested scroller | 直接采用上游修复；回归横向 dominant wheel、纵向冒泡、overlay occlusion 和 scroll edge |
| Select `Caret`、Select/Combobox/Input visual/API | `crates/gpui-form-gpui-component` adapters、Jaco run/settings picker | 不复制 caret；适配公开 state/event API并回归 clean、disabled、selection projection、popup theme |
| `RadarChart` 与 radial plot/tooltip | workspace 当前无 radar chart 调用 | 采用组件与文档，但不在无产品需求时新增 Jaco UI或本地 chart wrapper |
| button/tab/text/native menu 等行为 | 对应 workspace 调用点 | 只做编译与 focused interaction 回归；无证据时不重构 |

复用/删除决定由 [upstream-reuse-audit.md](upstream-reuse-audit.md) 约束，不能只做到编译通过。

### 验证

```sh
cargo tree -p gpui --depth 0
cargo tree -p gpui_platform --depth 0
cargo tree -p gpui-component --depth 0
cargo tree -p gpui-component-assets --depth 0
cargo tree -d
cargo test -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component
cargo check -p jaco -p feiwen -p http-client -p novel-download
```

API 修复完成后才执行 skill 同步，保证文档描述的是实际锁定的源码。

如果实施时要把目标改为 `57a9903f...` 之后的 commit，必须先更新本节、证据表、组件文档
A/M/D、目标 lockfile 的 Zed SHA 和复用审计；不得由 `cargo update` 隐式追到新 HEAD。

## DR-50：Lucide submodule

当前精确 tag `1.21.0`；本地 tag 快照中更新候选为 `1.25.0`。实施时先确认该 tag 仍是
计划接受的稳定系列，再 checkout 精确 tag。

1. 记录 `git diff --submodule=log -- third_party/lucide`。
2. 枚举 app-local `IconName`/slug，确认目标 `icons/*.svg` 存在。
3. 运行 `app-assets` macro tests 和使用 Lucide 的 app check。
4. 不修改用户文案、`.ftl`、macOS bundle localization 或 Jaco app icon。

```sh
git -C third_party/lucide tag --points-at HEAD
cargo test -p app-assets -p app-assets-macros
cargo check -p jaco -p feiwen
```

## DR-60：Rig 0.41 / RMCP 2.2 原子批次

### Manifest 与 feature graph

本批次只采用已经审计的正式版本：

- 根 `Cargo.toml` 删除 `rig-core = 0.39.0`，增加
  `rig = { version = "0.41.0", default-features = false, features = ["agent", "rmcp", "reqwest", "rustls", "websocket"] }`；
- `rmcp 1.8.0 -> 2.2.0`，保留现有五个直接 feature；
- `crates/jaco-agent/Cargo.toml` 的 normal/dev dependency 都从 `rig-core` 改为 `rig`，
  dev-only 增加 `test-utils`；
- `tools/mcp-auth-test-server/Cargo.toml` 的独立 workspace 也把 `rmcp 1.8.0` 更新到
  `2.2.0`，保留 `auth,macros,server,transport-streamable-http-server`，并单独更新
  `tools/mcp-auth-test-server/Cargo.lock`；
- 仅 `crates/jaco-agent` 新增 `async-stream = "0.3.6"`，用于把 Rig public WebSocket
  events 适配为 `StreamingCompletionResponse`；
- 不直接声明 `rig-core`、`rig-agent`、`tokio-tungstenite`；它们必须只由 `rig` facade
  与 `websocket` feature 解析；
- 不升级到 `rmcp 3.x`，即使 crates.io 已有更新，因为 Rig 0.41 的 native RMCP 集成锁定
  2.2，越级会产生双版本和 model type 不兼容。

先只编辑根 workspace 与独立 test-server manifest，不运行会隐式重解 lockfile 的命令；
随后立即以两个 lockfile 当前都存在的 `rmcp@1.8.0` 为 package ID 完成精确更新，最后才
执行 check/tree：

```sh
cargo update -p rmcp@1.8.0 --precise 2.2.0
cargo update --manifest-path tools/mcp-auth-test-server/Cargo.toml \
  -p rmcp@1.8.0 --precise 2.2.0
cargo check -p jaco-agent
cargo check --manifest-path tools/mcp-auth-test-server/Cargo.toml
cargo tree -p jaco-agent -e features
cargo tree -i rig-core
cargo tree -i rig-agent
cargo tree -i rmcp
cargo tree --manifest-path tools/mcp-auth-test-server/Cargo.toml -i rmcp
```

`rig-core -> rig facade` 是 manifest package replacement，不应再对即将删除的
`rig-core@0.39.0` 执行 `cargo update --precise`。上述顺序禁止在两个 `rmcp` 精确更新前
运行 `cargo check`；否则 Cargo 可能已经重解旧 package ID，使后续命令不可重复。
`cargo check -p jaco-agent` 根据新的完整 requirement 解析
`rig/rig-core/rig-agent 0.41.0`。lockfile 审查仍只接受本节与其已记录 transitive graph
可解释的增删。

### import 与 Agent API 迁移

文件范围：

- `crates/jaco-agent/src/**/*.rs`
- `crates/jaco-agent/src/runtime/tests.rs`
- `crates/jaco-agent/src/persistence/model.rs`

具体替换：

| 0.39 调用 | 0.41 调用 | 语义要求 |
| --- | --- | --- |
| `use rig_core::...` | `use rig::...` | Jaco 不绕过 facade 绑定拆分后的内部 crate |
| `AgentBuilder::hook(hook)` | `AgentBuilder::add_hook(hook)` | hook 顺序保持当前 persistence hook 在 Jaco 注册点的位置 |
| `stream_prompt(...).with_history(...)` | `.stream_prompt(...).history(...)` | 仍调用 `.without_memory()`，数据库是唯一 conversation history owner |
| `prompt(...).with_history(...).with_tool_concurrency(n)` | `.prompt(...).history(...).tool_concurrency(n)` | concurrency 和 committed tool result 顺序保持 Rig 0.41 契约 |
| `default_max_turns(max_steps)` | 同名 builder，但按 0.41 exact-total 语义 | `RunGuards.max_steps` 与 model call budget 1:1，不增加隐藏余量 |

`max_steps=1` 的现有测试必须改成“恰好允许一次 model call”；tool call 后需要第二次 model
call 时应产生 `MaxSteps`，不再保留 0.39 的 `n+2` 期望。所有
`CompletionError`/stream enum match 必须包含 non-exhaustive/`Unknown(Value)` 分支。

### Tool 与 hook 迁移

`crates/jaco-agent/src/tools.rs` 不再维护一个模拟旧 Rig public API 的层：

- 删除 `RigToolExecutor`、`RegisteredRigTool`、`ToolDyn` impl、
  `tool_output_to_model_text` 及 JSON 字符串反解析；
- `ToolRegistry::into_rig_tools` 改为
  `ToolRegistry::into_rig_tool_bundle(default_timeout) -> RigToolBundle`；
- `PersistingAgentHook` 不再直接执行 `RegisteredRuntimeTool.executor` 后返回 skip；审批通过
  后返回 Rig `Run`，由 Dynamic/RMCP registration 执行，result hook 再持久化；
- 新增内部 enum：

```rust
enum ToolEntryRuntime {
    Local(Arc<dyn ToolExecutor>),
    Rmcp {
        tool: rmcp::model::Tool,
        server: rmcp::service::ServerSink,
    },
}

struct ToolEntry {
    definition: ToolDefinition,
    runtime_tool_name: String,
    runtime: ToolEntryRuntime,
}

struct RegisteredRuntimeTool {
    definition: RegisteredToolDefinition,
    timeout: Duration,
}

struct RmcpToolRegistration {
    tool: rmcp::model::Tool,
    server: rmcp::service::ServerSink,
    timeout: Duration,
}

struct RigToolBundle {
    dynamic_tools: Vec<rig::tool::DynamicTool>,
    rmcp_tools: Vec<RmcpToolRegistration>,
    definitions: Vec<RegisteredToolDefinition>,
    runtime_tools: Vec<RegisteredRuntimeTool>,
}
```

`RegisteredRuntimeTool` 只供 hook 查 source/policy/timeout，不再持有 executor；实际 executor
只存在于 `ToolEntryRuntime::Local` 捕获进 `DynamicTool` callback。MCP tool definition
在生成 `RmcpToolRegistration` 前把 `name` 改成 finalized runtime name，确保 Rig provider
definition 与 Jaco namespace/collision mapping 相同。

`RmcpToolRegistration`/`RigToolBundle` 只需 `pub(crate)`，不序列化。`RigToolBundle` 实现：

- `definitions(&self) -> &[RegisteredToolDefinition]`
- `runtime_tools(&self) -> &[RegisteredRuntimeTool]`
- `install<M>(self, builder: AgentBuilder<M>) -> AgentBuilder<M, WithBuilderTools>`：
  先调用一次 `.dynamic_tools(self.dynamic_tools)` 进入 Rig 的 `WithBuilderTools` state，
  再逐个
  `.rmcp_tools_with_timeout(vec![registration.tool], registration.server, registration.timeout)`；
  这样空的 local-tool 集合也有确定返回类型。0.41 的 singular
  `rmcp_tool_with_timeout` 只定义在 `NoToolConfig` builder 上，进入
  `WithBuilderTools` 后必须调用 plural method，不能把 typestate 差异留给实施者。

本地 tool 通过 `DynamicTool::new(name, description, parameters, callback)` 注册；callback
接收 `&mut ToolContext` 和 JSON `Value`，执行 Jaco `ToolExecutor`/timeout 后：

1. `context.insert_result::<ToolInvocationOutput>(output.clone())` 保存 host-only canonical
   result；
2. `jaco_output_to_rig_tool_output(&output) -> Result<ToolOutput, ToolExecutionError>` 只负责
   把有序 content 转成给模型的 Rig output；
3. callback 直接返回 `ToolOutput` 或 `ToolExecutionError`。

result hook 通过 `event.tool_context.result::<ToolInvocationOutput>()` 持久化 content、
structured/raw output，而不是从给模型的文本恢复数据。RMCP tool 则读取 Rig 已保存的
`event.tool_context.result::<rmcp::model::CallToolResult>()`。

`PersistingPromptHook` 重命名为 `PersistingAgentHook` 并实现 `AgentHook`：

- `on_completion_call`：只负责 request metadata/策略 patch；
- `on_completion_response`、`on_stream_response_finish`：完成 model-turn 观测；
- `on_tool_call`：执行 Jaco max-call/repetition/approval guard，返回 `Run`、`Skip` 或
  `Stop`；
- `on_tool_result`：从 `ToolContext` 读取 Jaco output 或 RMCP `CallToolResult`，只在这一处
  完成 tool invocation/result entry 持久化；
- `on_invalid_tool_call`：保持当前 repair/skip/stop 产品策略；
- `observes`：只订阅以上实际使用的 `StepEventKind`，避免所有 delta 都触发无效 hook。

Rig 0.41 对被 skip 的 tool 也会发 result hook。审批拒绝/guard skip 必须在 hook context
保存 pending outcome，使 `on_tool_result` 写入 `denied`/`failed` 一次；不得在
`on_tool_call` 和 `on_tool_result` 双写数据库。

### RMCP 2.2 迁移边界

`rig::tool::rmcp::McpTool` 在 0.41 不再是 Jaco 可直接依赖的 public adapter。修改：

- `crates/jaco-agent/src/mcp.rs`
- `crates/jaco-agent/src/mcp/connector.rs`
- `crates/jaco-agent/src/tools.rs`
- `tools/mcp-auth-test-server/{Cargo.toml,Cargo.lock,src/main.rs}`
- 对应 MCP/config/runtime tests。

`McpToolRegistrationOptions` 的注册结果保存 clone 后的 `rmcp::model::Tool`、
`ServerSink`、Jaco 已确定的 runtime tool name 和 timeout。安装到 AgentBuilder 时必须
使用最终 namespaced/collision-safe name，不能退回 server 原始 name。Rig 的
`rmcp_tools_with_timeout` 负责 `CallToolResult` 到 `ToolOutput` 的 rich content/
structured content 映射；Jaco 继续负责：

- `McpSessionManager` 连接生命周期与 list-change；
- child-process/streamable HTTP transport；
- OAuth discovery、token refresh 与配置；
- tool namespace、审批策略、超时和 UI snapshot；
- invocation/result audit persistence。

不要整体采用 Rig 的 `McpClientHandler` 替换这些产品能力。RMCP 2.2 的
`AuthError::PkceUnsupported` 等新增 variant 必须用非穷尽方式映射到现有 Jaco error，
并为 S256 PKCE、取消请求和断开后孤儿 stream 增加 focused tests。

独立 test server 当前使用的 `ToolRouter`、`#[tool_router]`、`#[tool_handler]`、
`ServerHandler::get_info`、`StreamableHttpService` 和 `StreamableHttpServerConfig`
在 2.2 发布 API 中仍存在，预期不修改 server/RMCP 类型结构。compiler 需要的签名修改
保持最小；此外为让 S256 验收可观测，`src/main.rs` 的 authorize query 明确改为：

```rust
#[derive(Debug, Deserialize)]
struct AuthorizationQuery {
    redirect_uri: String,
    state: Option<String>,
    code_challenge: String,
    code_challenge_method: String,
}

impl AuthorizationQuery {
    fn has_valid_s256_challenge(&self) -> bool;
}
```

`has_valid_s256_challenge` 只在 method 精确为 `S256` 且 trim 后 challenge 非空时为 true；
`authorize` 对失败请求返回 OAuth `invalid_request`，成功时沿用现有 redirect/code/state/
issuer。该 server 不保存 verifier，不在 `/token` 做 hash 对比；所以它验证的是 Jaco/RMCP
client 是否声明并发送 S256，不冒充完整 authorization server。为该 helper 和 authorize
route 增加 missing challenge、`plain`、空 challenge、合法 S256 四个 focused tests。

其集成验收必须按真实 HTTP 顺序覆盖：

1. 未授权 `/mcp` 返回带 protected-resource metadata 的 401；
2. discovery 声明 `code_challenge_methods_supported=["S256"]`；
3. dynamic registration -> authorization code + S256 challenge -> token/refresh；
4. bearer token 后 initialize/list-tools/call `echo` 成功；
5. cancellation token 能关闭 streamable HTTP service。

不为 test server 新增 OAuth/crypto 依赖；S256 challenge 生成、state/verifier 保存与 token
exchange 由待测的 RMCP/Jaco client 完成。需要端到端校验 verifier 的工作应另立测试工具
设计，不能在依赖更新中临时实现一个不完整 OAuth authorization server。

### 本批次停止条件

DR-60 的目标是“现有 HTTP/SSE 功能在 Rig 0.41 上等价运行”。遇到以下情况立即停在本
批次，不提前写 WebSocket fallback：

- facade public API 与 0.41 发布源码不一致；
- RMCP 2.2 出现双版本或 `CallToolResult` rich content 丢失；
- tool approval/result hook 发生双写或状态顺序变化；
- `max_steps` 无法按一次 model call 一个预算解释；
- provider streaming step 仍可能在 tool loop 中保持 `running`。

最后一项的结构性修复在 GPT 计划的 provider-step 工作包完成，但 DR-60 至少要有一个
失败测试准确暴露当前问题，防止升级时把它误判为 Rig regression。

## Cargo.lock 审查

- 根 `Cargo.lock` 的所有 direct requirements 必须达到
  [dependency-evidence.md](dependency-evidence.md) 的精确目标；不得残留计划已经替换的
  `time 0.3.49`、`tokio 1.52.3`、`ignore 0.4.26` 等旧 direct owner。第三方需要的旧
  transitive version 可以存在，但必须能由 `cargo tree -i <package>@<version>` 解释；
- `tools/mcp-auth-test-server/Cargo.lock` 单独审查
  `anyhow 1.0.104`、`axum 0.8.9`、`schemars 1.2.2`、Serde/Tokio 公共目标和
  `rmcp 2.2.0`，不能误以为根 lockfile 已覆盖；
- 两个本地 macro crate 必须直接解析 `syn 3.0.3`；第三方 transitive Syn 2 可以保留，
  不通过 patch/alias 强行统一；
- `jaco-agent` 的 Base64 feature graph 必须只有所需 `std`，不能启用
  `base64/simd-unsafe`；
- 根 workspace 只存在 `rig`/`rig-core`/`rig-agent 0.41.0` 与 `rmcp 2.2.0`；独立
  test-server workspace 只存在 `rmcp 2.2.0`。两个 lockfile 都不得残留
  `rig-core 0.39.0`、`rmcp 1.8.0` 或引入 `rmcp 3.x`。
- `tokio-tungstenite` 只能由 Rig websocket feature 引入；TLS backend 保持 rustls，
  不意外引入 native-tls。
- 所有新增/删除 package 均可追溯到本计划中的直接目标。
- Zed GPUI 保持 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` 且 SHA 唯一；
  gpui-component 三个 package 均为
  `57a9903f48160845aabc8b92a1e2f5348c80d439`。
- `libsqlite3-sys` links 唯一；TLS/native backend 无意外变化。
- `cargo tree -d` 中已有多版本按 owner 解释，不以强行消灭重复版本为目标。
- 除计划中的 `rig` facade、`rig-agent` transitive package、WebSocket transitive graph
  和 `async-stream` 外，无新增 provider SDK、数据库 driver 或运行时。

## 全量验收

```sh
cargo fmt
cargo check --workspace
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo shear
git diff --check
git diff --submodule=log -- third_party/lucide
```

`cargo fmt` 只在实际代码修复后运行并保留结果；本次纯文档规划不改代码，不运行构建基线。
