# Issue #205 全 workspace 依赖升级计划

## 文档状态

- 状态：`In progress`
- 关联 issue：[#205](https://github.com/suxiaoshao/gpui/issues/205)
- Root hub：[Issue #205](README.md)
- 计划类型：Lestty 实施前置依赖升级
- Canonical plan：`docs/dev/issue-205/dependency-upgrade-plan.md`
- 基线证据日期：`2026-08-20`
- 实施状态：本地源码迁移、lock 审阅、workspace 验证和 Windows bundle smoke 已完成；外部三平台 CI、
  跨版本 MCP E2E 与人工 UI/a11y smoke 待执行
- Supersedes：Issue #172 中 Rig 0.41 的依赖目标，以及旧
  `gpui-1a246efd-component-5b45bcb` 批次作为“当前迁移”的状态；历史证据仍保留原文

本文是本轮依赖升级的唯一总计划。各 app、crate 与独立工具目录中的
`dependency-upgrade-plan.md` 只负责自己的 manifest、源码迁移和 focused 验证；各级 `README.md`
只作为导航，不复制本计划。

旧 GPUI migration 文档仍用于解释从更早 revision 迁入当前基线时形成的约束，但不再拥有本批 target、顺序
或完成状态；实现者不得继续按旧文档中的 `5b45bcb` target 更新 lockfile。

## 目标

在开始 Lestty 终端实现前，完成一次覆盖整个仓库的依赖升级：

1. 审计当前 22 个 workspace member 与 `tools/mcp-auth-test-server` 的全部直接外部依赖；实施后以 Zed
   `gpui_tokio` 取代本地同名 bridge，workspace 收敛为 21 个 member。
2. 把兼容更新提升到 `2026-08-20` 在线索引确认的最新版本；对 major/breaking 更新逐项迁移。
3. 将 GPUI、gpui-component 和新增 gpui-base 固定到一组可复现、单一类型宇宙的 Git revision。
4. Lestty 是唯一在 normal/runtime 图中直接使用 `gpui-base` 的应用；现有应用继续使用完整
   `gpui-component`，`app-theme` 仅允许 projection 回归测试使用 dev-only edge。
5. 用 Cargo 重新生成两个 lockfile，并在 macOS、Linux、Windows 通过 workspace 与 owner 验证。

“全部升级”在本计划中的准确含义是：**全部直接依赖都必须有审计结论；能安全升级的采用最新目标，存在已证实
兼容阻断的依赖以明确 pin、原因和解除条件记录**。不以引入双版本核心类型、破坏持久化或失去三平台构建为代价
机械追逐版本号。

## 非目标

- 不在本批实现 Lestty 的终端 backend、PTY、renderer、配置或主题。
- 不把 Jaco、Feiwen、HTTP Client 或 Novel Download 从 `gpui-component` 迁到 `gpui-base`。
- 不顺手重构业务状态、数据库 schema、migration、UI 视觉或打包结构。
- 不升级 `third_party/lucide`；它不是 Cargo package，本 issue 当前没有授权扩展该 vendored subtree。
- 不手工编辑 `Cargo.lock` 或 `tools/mcp-auth-test-server/Cargo.lock`。

## 已固定决定

### D-205-01：Lestty 是唯一 normal/runtime app direct consumer

目标依赖边界为：

```text
Lestty
├─ gpui
├─ gpui_platform
└─ gpui-base
```

- `app/lestty` 不依赖 `gpui-component`、`gpui-component-assets`、当前 `app-theme` 或当前 `app-assets`；后两者
  都会把完整组件库重新带入依赖图。
- 本批只建立 Lestty 空 crate 的依赖边界，不创建窗口，也不调用 init；后续开始应用启动链时调用
  `gpui_base::init(cx)`，不调用 `gpui_component::init(cx)`。
- Lestty 的按钮、tab、titlebar、图标和主题投影由 app 自己拥有；`gpui-base` 提供行为与基础设施，不是带
  默认视觉的完整组件库。
- 其他应用继续调用 `gpui_component::init(cx)`，不增加 normal/runtime direct `gpui-base` edge，也不批量改
  import。`app-theme` 的 dev-dependency 只用于断言 component theme 到 base theme 的投影，不进入生产图。

### D-205-02：Git 依赖必须只有一个 source identity

| Family | 当前 lock | 本批目标 | 决策依据 |
| --- | --- | --- | --- |
| Zed / `gpui` | `0.2.2 @ 1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | `0.2.2 @ e0931d5a9dbf4f781b336fdf448739e74a2ac0b5` | 最新 component 目标自身 lock 验证的 GPUI revision |
| Zed / `[patch.crates-io].gpui_macros` | `0.1.0 @ 1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`，manifest 无版本约束 | `=0.1.0 @ e0931d5a9dbf4f781b336fdf448739e74a2ac0b5` | proc-macro patch 与 GPUI 必须共享 source identity；补齐本仓要求的完整版本约束 |
| Zed / `gpui_tokio` | 本地 path crate `crates/gpui-tokio` | `0.1.0 @ e0931d5a9dbf4f781b336fdf448739e74a2ac0b5` | 上游 API 覆盖本地 bridge 并新增 `spawn_result`；删除 repo-local fork |
| longbridge / `gpui-component` | `0.5.2 @ 57a9903f48160845aabc8b92a1e2f5348c80d439` | `0.5.2 @ 5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3` | 当前 upstream main，首次包含本批需要的 gpui-base |
| `gpui-base` | 不存在 | `0.5.2 @ 5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3` | `publish = false`，只能从同一 Git source 获取 |

Zed 本地参考 checkout 的 `32a0e813...` 比 component 目标验证的 `e0931d5a...` 更新，但不作为本批默认 lock
目标。只有先在隔离 preflight 中证明 `32a0e813... + 5e5a1a30...` 通过完整编译和测试，且先更新本文目标表，
才允许继续前移；不能让“latest main”在实施期间漂移。

根 manifest 继续使用 canonical、无 query 的 Git URL，由 lockfile 固定完整 SHA。不得只在一条 dependency 上
添加 `?rev=` 或不同 URL，因为这会产生不可互换的两套 GPUI 类型。

### D-205-03：主 workspace 升级 Rig，暂不跨 RMCP major

- `rig 0.41.0 -> 0.42.0`，按 breaking migration 处理。
- 主 workspace 的 `rmcp` 使用精确要求 `=2.2.0`。Rig 0.42 的 `rig-agent` 仍依赖 `rmcp = "2"`，而
  `crates/jaco-agent` 会把 direct RMCP model types 交给 Rig；升级 direct edge 到 3.x 会形成不兼容 Rust 类型。
- 解除 pin 的条件：Rig 发布支持 RMCP 3 的同一 type universe，或另立 issue 设计显式 adapter；不能在本批
  同时保留两套 model 并做临时 JSON 桥接。
- 独立 workspace `tools/mcp-auth-test-server` 升级到 `rmcp 3.1.4`。它通过 HTTP 协议与 Jaco 交互，不共享 Rust
  类型；升级后必须证明 RMCP 2.2 client 与 RMCP 3.1.4 server 的 legacy protocol negotiation、OAuth 与 tool
  call 兼容。

### D-205-04：Diesel 与 SQLite sys crate 原子升级

- `diesel 2.3.11 -> 2.3.12`。
- `libsqlite3-sys 0.37.0 -> 0.38.2`，继续启用 `bundled-windows`。
- Diesel 2.3.12 允许 `<0.39.0`；最终图只能有一个 `links = "sqlite3"` package。
- 本批不改 schema、migration 或 repository 语义；任何数据差异都视为回归。

### D-205-05：lockfile 是单次、可审阅的生成结果

- 根 workspace 与独立工具分别维护自己的 lockfile。
- 先修改并验证直接依赖要求，再由 Cargo 完整重解；不得逐行手改，也不得覆盖 Lestty 已加入 workspace 的现有
  用户改动。
- 根 lock 的 dry-run 当前预计更新约 285 个 package；普通 transitive 更新按 dependency cohort 审阅，native、
  TLS、parser、数据库、媒体和 proc-macro 变化必须单独检查。

### D-205-06：工具链基线不静默漂移

- 仓库当前说明为 Rust `1.95+`，CI 安装 `stable`，没有 `rust-toolchain.toml`；当前机器是 Rust 1.97.0。
- component 目标在上游用 Rust 1.97.1 验证。本批先以 1.97.1 作为参考验证工具链，同时运行一次 1.95
  compatibility check。
- 如果 1.95 仍通过，README/AGENTS 的支持基线保持不变；如果 target GPUI 实际要求更高版本，必须把首次失败
  证据、首个通过版本、README、AGENTS 与 CI 一起纳入同一变更，不能只依赖开发机的新版编译器。

### D-205-07：删除 repo-local gpui-tokio fork

- Zed target 的 `gpui_tokio 0.1.0` 公开 `init`、`init_from_handle`、`Tokio::spawn`、`Tokio::handle` 和
  `JoinError`，覆盖本地 crate 的完整 API，并以同样的 Task-drop abort 语义管理 Tokio task。
- 根 dependency key 可继续为 `gpui-tokio`，但通过 `package = "gpui_tokio"` 指向与 `gpui` 相同的 Zed Git
  source；Jaco/HTTP Client 的 Rust import `gpui_tokio` 不需要 compatibility facade。
- 消费者 cancellation/network/time 回归通过后，从 workspace members 删除 `crates/gpui-tokio`，并删除本地
  `Cargo.toml` 与 `src/`；保留 `docs/dev/issue-205` 作为退役证据。上游新增的 `anyhow`、`gpui_util`
  transitive 由同一 Zed source 拥有。
- 上游 bridge 自身只开启 Tokio `rt,rt-multi-thread`；Jaco、HTTP Client 及未来 Lestty 必须在各自 manifest
  明确声明各自实际使用的 `net/io-util/sync/time/fs/process` 子集，不能依赖 workspace 中无关 consumer 的
  feature union。
- 若上游 API 或 drop-cancel 行为不能通过 parity tests，停止删除并先更新
  [上游能力复用审计](upstream-reuse-audit.md)，不得保留两套同名 bridge。

### D-205-08：GPUI 相关 skill 与 component 目标提交同步

- `.agents/skills/gpui/**` 不是根据 Zed 源码在本仓手工维护的摘要，而是
  `longbridge/gpui-component` 仓库 `skills/gpui/**` 的上游镜像。本批以同一 component target
  `5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3` 的完整目录为权威，不从 Zed checkout 拼装或向
  镜像文件中追加 repo-local 规则。
- 目标 `skills/gpui/**` 与当前上游基线的 23 个文件无增删和语义变化；本仓副本仅有 5 个文件
  的文件末空行差异。实施仍要记录 target SHA、源路径和 normalized directory diff，不得因
  “本批没有内容变化”跳过同步证据。
- `.agents/skills/gpui-component-usage` 是本仓的消费者 skill，不是上游
  `skills/gpui-component/**` 的纯镜像。其 `SKILL.md`、`references/rules/**` 和组件选择索引保持
  repo-local ownership；`references/components/**` 中的上游文档快照则从 target
  `website/docs/components/**` 完整刷新，并明确排除本仓自有索引。
- target 没有 `skills/gpui-base` 目录。不得将 `website/base/**` 伪装成上游 skill；Lestty
  的 base-only 路由和索引由本仓所有，内容以同一 target 的 `website/base/**`、`crates/base/**`
  和实际 API 交叉核对。

### D-205-09：拒绝 `arrayref 0.3.10` 供应链载荷

- 2026-08-20 的完整 lock refresh 曾解析到当天发布的 `arrayref 0.3.10`。该版本新增正常依赖
  `proc-macro1 1.0.107`；后者的 `build.rs` 禁用 TLS 证书验证、从裸 IP 下载平台载荷并后台执行。
- 构建在下载阶段失败；检查系统临时目录未发现 `rust-setup*` 载荷。Cargo registry 中两个恶意包的
  source/archive cache 与共享 `target` 中已编译的 build-script artifacts 已删除，且最终图不得再出现
  `proc-macro1`。
- `arrayref 0.3.9` 的 crates.io 发布已被 yank，不能只靠一次性的 yanked lock resolution。根
  `[patch.crates-io]` 改用原项目 `https://github.com/droundy/arrayref` 的
  `f8d0299d863922db6c409d08098941e833b70d69`；该提交 manifest 为 `0.3.9`，只含原有 dev dependency，
  不含 build script 或 `proc-macro1`。
- 所有后续 Cargo 命令使用 `--locked`。完成门包括 `cargo tree --workspace --locked -i arrayref` 只显示上述
  Git commit，以及 `cargo tree --workspace --locked -i proc-macro1` 返回 package-not-found。

### 耦合 skill 与文档产物

| Dependency / target | Coupled artifact | Ownership / provenance | Required synchronization | Expected add/change/delete | Evidence / completion gate |
| --- | --- | --- | --- | --- | --- |
| `gpui-component@5e5a1a30` 验证的 GPUI | `.agents/skills/gpui/**` | 上游 `skills/gpui/**` 完整镜像 | 从精确 target 比较全部目录，仅允许 LF/文件末规范化差异 | 23 个文件；预期 0 add / 0 semantic change / 0 delete | 记录 source SHA/path；normalized hash manifest 无内容差异；旧 SHA 无残留 |
| `gpui-component@5e5a1a30` | `.agents/skills/gpui-component-usage/references/components/**` | 上游 `website/docs/components/**` 文档快照；本仓索引除外 | 同步完整 upstream doc set，更新 attribution 的 SHA、路径、许可证和排除项 | 新增 `command.md`、`textarea.md`；刷新 `chart`、`dropdown_button`、`editor`、`input`、`list`、`notification`、`scrollable`、`tabs`、`text-view`、`title-bar`；`index.md` 三方合并 | 目标 63 份文档无漏项；上游快照文件与 target normalized diff 为零；attribution 指向 `website/docs/components` 并刷新 2024–2026 license |
| `gpui-component@5e5a1a30` | `.agents/skills/gpui-component-usage/{SKILL.md,references/rules/**,references/components/index.md}` | repo-local 消费者路由与行为规则 | 对照上游 `skills/gpui-component/**`、component 源码/story/test 和刷新后文档，手工重放本仓适配与上游文档 errata | 增加 `Command`、`Textarea`、`AppMenuBar`、TitleBar helper 和 base/component 边界路由；移除本地旧 API 事实 | repo-local rules 不被上游 compact skill 覆盖；所有路由可解析；已知 upstream doc 漂移有 source-backed errata |
| `gpui-base@5e5a1a30` | `.agents/skills/gpui-base-usage/**` 与 `.agents/skills/gpui-app-development/SKILL.md` 路由 | repo-local adaptation；上游仅提供 `website/base/**` 和 `crates/base/**` | 新建独立 base-only skill，将 Lestty 与完整 component 路由分开，记录 base 只提供行为 primitive、不提供完整视觉/a11y 保证 | 新增 `SKILL.md` 及 architecture/boundary、primitives、input-textarea-editor、accessibility、text-selection 精简参考；不复制一个不存在的 upstream skill | Lestty/base-only 任务路由到 `gpui-base-usage`，其他 app 仍路由到 `gpui-component-usage`；每个 API 事实可追溯到 target path/test |

## 依赖目标清单

### Registry 兼容更新

| Dependency | Source | Target | Owners |
| --- | --- | --- | --- |
| `async-compression` | 0.4.42 | 0.4.43 | http-client、http-client-test-server |
| `async-trait` | 0.1.91 | 0.1.92 | jaco、jaco-agent |
| `base64` | 0.23.0 | 0.23.1 | http-client、http-client-test-server、jaco-agent |
| `bytemuck` | 1.25.0 | 1.25.2 | http-client |
| `bytes` | 1.12.0 | 1.12.1 | http-client、http-client-test-server |
| `clap` | 4.6.4 | 4.6.6 | xtask |
| `diesel` | 2.3.11 | 2.3.12 | jaco-db |
| `futures` / `futures-util` | 0.3.33 | 0.3.34 | novel-download、jaco-agent、http-client、http-client-test-server |
| `globset` | 0.4.19 | 0.4.20 | jaco-agent |
| `http` | 1.4.2 | 1.5.0 | jaco、jaco-agent、http-client |
| `http-body-util` | 0.1.3 | 0.1.5 | http-client-test-server |
| `hyper` | 1.10.1 | 1.11.0 | http-client-test-server |
| `ignore` | 0.4.31 | 0.4.33 | jaco-agent |
| `similar` | 3.1.1 | 3.2.0 | jaco-agent |
| `thiserror` | 2.0.19 | 2.0.20 | all direct owners listed in their owner plans |
| `time` | 0.3.54 | 0.3.55 | jaco、jaco-agent、jaco-core、jaco-db |
| `trybuild` | 1.0.118 | 1.0.120 | gpui-form、gpui-form-macros |
| `uuid` | 1.24.0 | 1.24.1 | jaco-core |
| `xcap` | 0.9.7 | 0.9.8 | jaco |

另将 Jaco build dependency 的宽范围 `winresource = "0.1"` 规范为当前完整版本 `0.1.31`，满足本仓库新增
依赖必须使用完整版本号的规则。

### Breaking、独立 major 与保留项

| Dependency | Source | Target | 分类 | 关闭条件 |
| --- | --- | --- | --- | --- |
| `rig` | 0.41.0 | 0.42.0 | breaking migration | streaming/unary、hooks、tool identity、持久化与错误测试通过 |
| root `rmcp` | 2.2.0 | 2.2.0 | compatibility pin | Rig 自身升级到 RMCP 3，且 Jaco/Rig 可共享同一 3.x type universe |
| tool `rmcp` | 2.2.0 | 3.1.4 | independent breaking migration | tool 单测 + Jaco 2.2 client 端到端通过 |
| `libsqlite3-sys` | 0.37.0 | 0.38.2 | breaking manifest range | 单一 sqlite link + 三平台 build + DB tests |
| `rodio` | =0.22.2 | =0.22.2 | intentional exact pin / current | 音频 owner 另行决定 |

### 已审计、当前无需改 manifest 的直接依赖

2026-08-20 在线 `cargo upgrade --dry-run --incompatible allow --pinned allow` 未给出以下依赖的新目标；实施
当天必须再次运行同一命令，结果作为 PR 证据，不能把本表当永久“最新”声明：

`async-channel`、`async-compat`、`async-stream`、`block2`、`dirs`、`dirs-next`、`duckdb`、
`encoding_rs`、`fluent-bundle`、`garde`、`get-selected-text`、`global-hotkey`、`grep-matcher`、
`grep-regex`、`grep-searcher`、`hayro`、`hex`、`hyper-util`、`image`、`material-color-utils`、`mime`、
`mime_guess`、`nom`、`notify-debouncer-full`、全部 `objc2*`、`pinyin`、`plist`、`proc-macro2`、`quote`、
`r2d2`、`raw-window-handle`、`regex`、`reqwest`、`rust-embed`、`scraper`、`serde`、`serde_json`、`sha2`、
`smol`、`syn`、`sys-locale`、`tauri-bundler`、`tauri-utils`、`tempfile`、`tokio`、`tokio-util`、`toml`、
`tracing`、`tracing-subscriber`、`unic-langid`、`unicode-segmentation`、`url`、`walkdir`、`which`、全部
`windows*`。独立工具的 `anyhow`、`axum` 与 `schemars` 也在此类。

## 已知源码迁移

### GPUI / gpui-component / gpui-base

1. `crates/app-theme/src/lib.rs` 与 `app/jaco/src/state/theme.rs` 在
   `Theme::global_mut(cx).apply_config(...)` 后调用 `Theme::sync_base(cx)`，使 scrollbar/resizable 等移入
   gpui-base 的主题投影同步。
2. HTTP Client 的 response viewer、HTTP text body 与 Jaco prompt dialog 不再把多行/代码编辑器建模为旧
   `InputState`：普通多行迁到 `TextareaState` / `Textarea`，代码编辑迁到 `EditorState` / `Editor`。
3. `gpui-component` 目标会统一启用 GPUI `profiler` feature；接受新增 `hdrhistogram` transitive，并检查启动、
   包体和性能影响。
4. 根 `[profile.dev.package]` 增加 `gpui-base = { opt-level = 3 }`，避免已迁入 base 的 input、scroll、
   virtual-list 行为在开发构建中退回未优化代码。
5. 按 [a11y 与 Command 复用审计](accessibility-and-command-reuse-audit.md) 区分底层 AccessKit、逐组件语义和
   app-owned a11y；Jaco 会话搜索使用 target `Command` 做适配式替代，但保留数据库与 operation owner，并关闭
   Command 的二次本地过滤。
6. 按 [上游能力复用审计](upstream-reuse-audit.md) 执行 deletion-first 迁移：删除本地 `gpui-tokio` 和 Jaco
   AppMenuBar fork；TitleBar 统一使用 helper；Picker、系统通知、timeline 跟尾和 cursor animation 保持
   `Defer`，只记录解除条件和现状回归，不能把相似 API 当成等价替代。

### Rig 0.42

以 Rig 0.42 自带 `MIGRATING.md` 为上游权威，至少核对：

- `OneOrMany<T>` 移除并改为 `Vec<T>`；所有依赖“非空”的位置显式验证。
- `ToolOutput::content(Vec<_>)` 变为 fallible；错误不能被映射成成功的空 tool result。
- tool call 的内部 correlation ID 与 provider ID 被拆分；持久化和 replay 必须保留正确 provenance。
- streamed parts 有稳定 identity/lifecycle；streaming 与 unary 最终状态、取消和 incomplete turn 保持一致。
- provider response status/body/header/request ID 的错误分类变化不得泄露敏感响应正文。
- 按 [上游能力复用审计](upstream-reuse-audit.md) 删除私有 response-id metadata/probe 与重复完成 side channel；
  精确 0.42 WebSocket unary helper 没有填充 `raw`，在上游修复前使用 tested public-event drain 保留 reasoning
  context，不能为追求删除量丢 continuation 数据。
- `similar 3.2` 默认 Myers 输出会变化；jaco-agent 为保持既有 diff text contract 显式使用 `RawMyers`。

### RMCP 3 独立测试服务

- `StreamableHttpServerConfig::with_stateful_mode(false)` 迁到
  `with_legacy_session_mode(false)`。
- 直接复用 RMCP 3 默认 loopback Host 防 DNS-rebinding guard；验证合法/恶意 Host，不复制或关闭该保护。
- 保留 legacy protocol version negotiation；Jaco RMCP 2.2 client 必须能初始化、完成 OAuth、列出并调用
  echo tool、刷新 token、取消和正常关闭。
- 工具使用自己的 lockfile；它的 RMCP 3 不得出现在根 workspace 的 `cargo tree`。

## Owner 计划索引

| Owner | Owner plan | 本批责任 |
| --- | --- | --- |
| `app/jaco` | [plan](../../../app/jaco/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI/component、Rig consumer、registry patch、主题/Input 迁移 |
| `app/feiwen` | [plan](../../../app/feiwen/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI/component 与 app 回归 |
| `app/http-client` | [plan](../../../app/http-client/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI/component、Input split、HTTP/media patch |
| `app/novel-download` | [plan](../../../app/novel-download/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI/component、futures/thiserror |
| `app/lestty` | [plan](../../../app/lestty/docs/dev/issue-205/dependency-upgrade-plan.md) | 唯一 normal/runtime app gpui-base direct consumer 与 graph assertion |
| `crates/app-assets` | [plan](../../../crates/app-assets/docs/dev/issue-205/dependency-upgrade-plan.md) | component assets 与 source identity |
| `crates/app-assets-macros` | [plan](../../../crates/app-assets-macros/docs/dev/issue-205/dependency-upgrade-plan.md) | proc-macro cluster current audit |
| `crates/app-theme` | [plan](../../../crates/app-theme/docs/dev/issue-205/dependency-upgrade-plan.md) | base theme projection |
| `crates/gpui-form` | [plan](../../../crates/gpui-form/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI dev surface、trybuild |
| `crates/gpui-form-gpui-component` | [plan](../../../crates/gpui-form-gpui-component/docs/dev/issue-205/dependency-upgrade-plan.md) | component adapter API |
| `crates/gpui-form-macros` | [plan](../../../crates/gpui-form-macros/docs/dev/issue-205/dependency-upgrade-plan.md) | trybuild/proc-macro fixtures |
| `crates/gpui-operation` | [plan](../../../crates/gpui-operation/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI async/task tests |
| `crates/gpui-store` | [plan](../../../crates/gpui-store/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI Entity/Store tests |
| `crates/gpui-tokio` | [retirement plan](../../../crates/gpui-tokio/docs/dev/issue-205/dependency-upgrade-plan.md) | 过渡 owner；以 Zed `gpui_tokio` 替代，删除本地 member/source 并保留退役文档 |
| `crates/http-client-test-server` | [plan](../../../crates/http-client-test-server/docs/dev/issue-205/dependency-upgrade-plan.md) | Hyper/HTTP/compression batch |
| `crates/jaco-agent` | [plan](../../../crates/jaco-agent/docs/dev/issue-205/dependency-upgrade-plan.md) | Rig 0.42 breaking migration 与 RMCP 2 pin |
| `crates/jaco-conversation` | [plan](../../../crates/jaco-conversation/docs/dev/issue-205/dependency-upgrade-plan.md) | thiserror patch |
| `crates/jaco-core` | [plan](../../../crates/jaco-core/docs/dev/issue-205/dependency-upgrade-plan.md) | time/uuid 与 persisted domain regression |
| `crates/jaco-db` | [plan](../../../crates/jaco-db/docs/dev/issue-205/dependency-upgrade-plan.md) | Diesel/SQLite 原子升级 |
| `crates/platform-ext` | [plan](../../../crates/platform-ext/docs/dev/issue-205/dependency-upgrade-plan.md) | native bindings audit 与三平台验证 |
| `crates/window-ext` | [plan](../../../crates/window-ext/docs/dev/issue-205/dependency-upgrade-plan.md) | GPUI window API 与 native regression |
| `crates/xtask` | [plan](../../../crates/xtask/docs/dev/issue-205/dependency-upgrade-plan.md) | Clap、打包依赖和 bundle smoke |
| `tools/mcp-auth-test-server` | [plan](../../../tools/mcp-auth-test-server/docs/dev/issue-205/dependency-upgrade-plan.md) | 独立 RMCP 3 server migration |

## 实施工作包

### WP-205-00：冻结基线与目标

1. 保存 `git status --short`、现有 Lestty member diff、两个 lockfile hash 和当前 compiler 版本。
2. 在线重跑两个 workspace 的 `cargo upgrade --dry-run --incompatible allow --pinned allow --verbose`。
3. 核对 target Git commits、crate versions、licenses、MSRV、feature diff 和 upstream migration guide。
4. 若 upstream 在实施日已有更新，不自动改目标；先更新本文与 owner plans，再开始 manifest 修改。

### WP-205-10：根依赖图与 Lestty gpui-base edge

1. 在 `[workspace.dependencies]` 新增与 component 同源的 `gpui-base`。
2. 将 `gpui-tokio` dependency key 映射到同一 Zed source 的 `gpui_tokio` package；先跑 Jaco/HTTP Client
   cancellation/network/time parity tests，再移除本地 workspace member、manifest 与源码，保留退役文档。
3. 在 Lestty 只增加 `gpui`、`gpui_platform`、`gpui-base`；不接 app-theme/app-assets/component。
4. 更新 `[profile.dev.package]` 的 gpui-base 优化配置。
5. 固定 GPUI/component families 的精确 lock SHA，并验证单一 source identity、无本地 gpui-tokio path edge。

### WP-205-20：GPUI/component API 迁移

1. 先迁移 shared `app-theme`，补 theme-to-base 同步测试。
2. 迁移 Jaco theme 与 editor，迁移 HTTP Client textarea/editor。
3. 按 `D-205-R03/R04` 将 Jaco 会话搜索的通用交互层适配到 `Command`，保留外部搜索 authority，并完成
   Command 与自绘控件的 a11y 验收。
4. 按上游复用审计删除 Jaco AppMenuBar fork、统一 TitleBar options；Picker 只冻结 `Defer` contract inventory，
   不在依赖 blocker 中进行大规模产品重构。
5. 编译并验证所有当前 GPUI owner surface，包括被删除 bridge 的两个消费者；现有 app 不改变组件库边界。

### WP-205-25：GPUI skill 与文档产物同步

以 [skill 同步计划](skill-sync-plan.md) 为唯一实施细则：

1. 核对 `5e5a1a30/skills/gpui/**` 到 `.agents/skills/gpui/**` 的完整镜像；同步上游增删，不向
   镜像目录添加 Zed 或本仓自有补充。
2. 将 component docs 快照从旧 `docs/docs/components` 迁到 target `website/docs/components`；新增
   `Command`/`Textarea`，刷新 10 份已变文档，对本仓 `index.md` 做三方合并，同步 provenance
   和 license。
3. 在 repo-local `gpui-component-usage` 中重放 Command 外部搜索、Textarea/Editor 分层、AppMenuBar、
   TitleBar helper、system notification 和 a11y 边界；不用上游 compact skill 整目录覆盖。
4. 新建 repo-local `gpui-base-usage` 并更新 `gpui-app-development` 路由；Lestty/base-only 任务不再
   路由到完整 component skill。
5. 运行目录/hash、旧 SHA/旧路径/旧 API 残留、Markdown 链接、frontmatter、UTF-8/LF 门禁；
   实际符号以 target source/story/test 为准。

### WP-205-30：兼容 registry 批次

按 owner 表更新精确 manifest requirement；先 HTTP/compression、再 async/search、再 proc-macro/test、最后通用
`thiserror/time`。每个 cohort 完成 focused tests 后才能进入下一 cohort，以便定位 lock churn。

### WP-205-40：Rig 0.42 breaking migration

1. 更新 root Rig requirement，保持 root RMCP 2.2。
2. 以 compile errors + Rig migration guide 盘点 jaco-agent runtime、hooks、stream、tool、persistence、providers。
3. 先用公开 raw/identity/model-turn finish 收回旧 side channel，再固定 typed conversions 与 persisted JSON
   compatibility；禁止用 lossy JSON round-trip 规避类型变更。
4. 精确 0.42 WebSocket unary raw 缺口使用 public-event drain 或经验证的上游修复，不直接调用会丢 reasoning
   context 的路径。
5. 补 streaming/unary、empty content、tool identity、cancel、provider error、continuation fallback 与旧记录读取测试。

### WP-205-50：Diesel / libsqlite3-sys

原子更新两个 requirement 和 lock resolution，执行 migration/repository tests、只读/写入回归与三平台 native build；
不得产生 schema 或 migration diff。

### WP-205-60：独立 MCP test server

在独立 workspace 升级 RMCP 3.1.4、迁移 server config API、生成独立 lockfile；先完成 tool tests，再由 Jaco
RMCP 2.2 client 完成静态 bearer 与 OAuth 两条端到端路径。

### WP-205-70：完整 lock refresh、打包与文档

1. Cargo 完整重解两个 lockfile，审阅新增/移除 package、native/TLS feature 与 duplicate versions。
2. 运行 workspace 全量 build/test/clippy、四个既有 app 的 bundle smoke 和三平台 CI。
3. 若 Rust 1.95 不再通过，按 D-205-06 同步工具链基线；否则不改支持声明。
4. 在本计划和 owner plans 回填命令、结果、CI run、最终 SHA 和 residual risks 后才可标记 `Done`。

## 顺序与提交边界

```text
WP-00 evidence
  -> WP-10 Git graph + Lestty base
  -> WP-20 GPUI/component API
  -> WP-25 GPUI skills + vendored docs
  -> WP-30 compatible registry cohort
  -> WP-40 Rig
  -> WP-50 Diesel/SQLite
  -> WP-60 independent RMCP server
  -> WP-70 full lock + packaging + CI
```

推荐按上述边界保留可审阅提交；lockfile 只在各 cohort 需要时由 Cargo 更新，并在最终 WP-70 统一收束。若某个
breaking cohort 失败，只回退该 cohort 的 manifest/source/lock diff，不得用 `git reset --hard` 或覆盖用户已有改动。

## 验证矩阵

### 依赖图与 lock

```powershell
cargo metadata --locked --format-version 1
cargo tree --workspace --locked -d
cargo tree -p lestty --locked
cargo tree --workspace --locked -i gpui-base
cargo tree --workspace --locked -i gpui-component
cargo tree -p jaco-agent --locked -i rmcp
cargo tree -p jaco-db --locked -i libsqlite3-sys
cargo metadata --manifest-path tools/mcp-auth-test-server/Cargo.toml --locked --format-version 1
cargo tree --manifest-path tools/mcp-auth-test-server/Cargo.toml --locked -i rmcp
```

人工断言：Lestty tree 不含 `gpui-component`；根 graph 只有 RMCP 2.x；独立工具只有 RMCP 3.1.4；GPUI 和
longbridge families 各只有一个 Git SHA；SQLite sys crate 只有一个版本。

### Skill 与文档产物

- `gpui` 镜像的文件集与 normalized content 必须等于
  `gpui-component@5e5a1a30/skills/gpui/**`；仅 LF 和文件末规范化可作为明示例外。
- component 快照必须覆盖 target 63 份 component docs，上游文档内容 normalized diff 为零，
  本仓 `index.md` 和 rules 另行验证。
- 残留搜索不得再命中旧 snapshot SHA/path、`.multi_line(...)` / `.code_editor(...)` 或将
  Command-like 面板无条件路由到 Combobox 的旧规则。target `title-bar.md` 自身仍有错误的
  `AppMenuBar::new(window, cx)` 示例，因快照字节完整性保留，但 repo-local errata 必须路由到源码真实
  签名 `AppMenuBar::new(cx)`。`TitleBar::title_bar_options()` 仍是有效 API；新建 `WindowOptions` 时优先
  `TitleBar::window_options()`。
- `Command`、`Textarea`、`Editor`、base/full init 边界都必须能从 skill 入口逐步路由；
  Lestty 必须命中 `gpui-base-usage`，不命中 `gpui-component-usage`。

### Focused 与 workspace

各 owner 先运行自己的计划命令，随后运行：

```powershell
cargo build --workspace --all-features --locked
cargo test --workspace --all-features --locked --no-fail-fast
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
cargo test --manifest-path tools/mcp-auth-test-server/Cargo.toml --locked
```

Lestty 当前只有脚手架，因此本批只要求 `cargo check -p lestty --all-features --locked` 和依赖图断言；终端行为
测试属于后续 Issue #205 实施计划。

### 平台与人工 smoke

| 平台 | 必须证明 |
| --- | --- |
| Windows | GPUI/component 构建、SQLite bundled-windows、window-ext/platform-ext、Jaco/HTTP Client 启动 |
| macOS | GPUI/component 构建、ObjC bindings、窗口/主题、严格 Clippy、四个既有 app bundle |
| Linux | bootstrap 脚本仍完整、X11/Wayland、音频/native libs、四个既有 app build |
| MCP E2E | RMCP 2.2 Jaco client 对 RMCP 3.1.4 test server 的 bearer、OAuth、refresh、tool call、shutdown |

## 风险与失败处理

| Risk | 预防/检测 | 失败处理 |
| --- | --- | --- |
| GPUI 双类型宇宙 | source SHA tree assertion | 恢复 canonical URL，重新生成 lock；禁止 adapter/cast |
| component 与更新 GPUI 不兼容 | 采用上游验证的 e093 target | 保持 e093；另立证据后才前移 |
| Lestty 间接拉入完整组件 | negative cargo-tree assertion | 移除 app-theme/app-assets/component edge，设计 app-owned 替代 |
| Rig 改变 persisted tool identity | 旧 fixture + round-trip + replay | 停止升级，不自动丢字段或重写历史数据 |
| RMCP 2/3 协议不互通 | 独立 server E2E | 工具暂 pin 2.2 并记录阻断；主 workspace 不变 |
| SQLite native link 冲突 | inverse tree + 三平台 build | Diesel/sys 一起回退，不保留双 links package |
| 本地 gpui-tokio 删除后取消语义漂移 | drop-to-abort、panic/join、network/time consumer tests | 停止删除并修正上游映射；不保留同名双 bridge |
| skill 镜像与本地适配相互覆盖 | 镜像/快照/local rules 分层；目录 diff 与 residual gate | 恢复精确上游镜像，再仅在 repo-local 层重放适配；不在镜像里打补丁 |
| 大规模 lock churn 难审阅 | cohort commits、cargo tree -d | 回退当前 cohort，保留已验证 cohort |
| Rust 基线静默提高 | 1.95 compatibility + 1.97.1 reference | 显式更新工具链文档/CI，或 pin 最后兼容依赖 |

## 实施证据

### 本地自动化结果（Windows，Rust/Cargo 1.97.1）

| 范围 | 实际执行 | 结果 |
| --- | --- | --- |
| 格式与静态 diff | `cargo fmt --all -- --check`；`git diff --check` | 通过 |
| Root lock/metadata | `cargo metadata --locked --offline --format-version 1`；source/residual scans | 通过；Zed 仅 `e0931d5a...`，longbridge 仅 `5e5a1a30...` |
| Workspace build | `cargo build --workspace --all-features --locked` | 通过 |
| Workspace tests | `cargo test --workspace --all-features --locked --no-fail-fast` | 通过；全部 unit、integration、trybuild 与 doc tests 通过 |
| Workspace Clippy | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 通过 |
| 独立 RMCP 3 工具 | 在 `tools/mcp-auth-test-server` 执行 fmt、build、test、strict Clippy | 通过；独立 lock 只解析 `rmcp 3.1.4` |
| 关键依赖图 | Lestty/component/base、root/tool RMCP、SQLite inverse tree 与 duplicate tree | 通过；Lestty normal 图仅直连 `gpui`、`gpui_platform`、`gpui-base` |
| Windows bundle smoke | 依次执行 `cargo run -p xtask --locked -- bundle {jaco,feiwen,http-client,novel-download}` | 四条命令均成功生成 MSI；共享输出目录最终保留最后一次 Novel Download 产物 |
| Lock 安全审阅 | registry checksum/yanked、Git SHA、build script/proc-macro、native `links`、`arrayref` 回归扫描 | 通过；无 `proc-macro1`/`arrayref 0.3.10`，无重复 native links owner |

Windows bundler 对四个应用均输出非阻断的 `__TAURI_BUNDLE_TYPE variable not found in binary` 警告；MSI 仍成功
生成。该警告影响未来 Tauri updater 识别，不影响本批 bundle smoke，保留为后续 packaging 风险。

### 尚未执行的外部门禁

- macOS/Linux CI 与 macOS 四应用 bundle；当前任务未推送分支，也没有可回填的 CI run/link。
- Narrator、VoiceOver、Orca/AT-SPI，以及各应用真实窗口、titlebar、主题和 native window 人工 smoke。
- Jaco RMCP 2.2 client 对独立 RMCP 3.1.4 server 的 bearer、OAuth/refresh、tool call 与 shutdown E2E。

这些门禁需要对应平台、运行中的桌面会话或跨进程测试环境；它们不会被本地 Windows 编译结果冒充为已完成。

## 完成条件

- [x] 当前 22 个 member owner 与独立工具均有审计结论；gpui-tokio 退役文档已记录实际执行证据，
      最终 21-member workspace 的 owner 索引无悬空链接。
- [x] 最终清单中的全部唯一外部直接依赖均有 upgrade、pin 或 confirmed-current 结论，新增
      `gpui-base/gpui_tokio` edge 也已计入审计。
- [x] GPUI/component families 各自解析到单一目标 SHA；Lestty 是唯一 normal/runtime app direct
      `gpui-base` consumer，`app-theme` 仅保留 projection-test dev edge。
- [x] Jaco/HTTP Client 已消费 Zed `gpui_tokio`，本地 `crates/gpui-tokio` member/path edge/源码均无残留。
- [x] 已知 Theme/Input/Editor breaking 调用点全部迁移并有回归测试。
- [x] `gpui` 上游 skill 镜像、component docs 快照、repo-local component/base 路由及 provenance 均按
      [skill 同步计划](skill-sync-plan.md) 验证完成。
- [ ] Rig 0.42、Diesel/SQLite、独立 RMCP 3 的本地 focused/workspace 验证通过；RMCP 2↔3 跨进程 E2E
      尚未执行。
- [x] 两个 lockfile 均由 Cargo 生成且审阅完成，没有覆盖 Lestty 现有用户改动。
- [ ] workspace build/test/clippy/fmt、独立工具测试和四个 Windows app bundle 已通过；macOS/Linux CI 与
      macOS bundle 尚未执行。
- [ ] 最终版本、Git SHA、工具链结果、执行命令和未关闭风险已回填；CI run/link 与人工 smoke 记录尚缺。

满足以上条件后，依赖升级才可视为 Issue #205 终端实现的已完成先决条件；在此之前不得开始 Lestty 生产
backend/UI 实现。
