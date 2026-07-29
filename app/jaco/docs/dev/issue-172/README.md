# Issue #172：依赖更新、上游同步与 GPT-5.6 适配计划

关联 issue：[suxiaoshao/gpui#172](https://github.com/suxiaoshao/gpui/issues/172)

## 当前结论

本文档集同时保留实施设计和 2026-07-29 的执行记录。`rig`、`rig-core`、
`rig-agent` 0.41.0 已同步发布到 crates.io，并由正式 `v0.41.0` release/tag 指向同一
源码；依赖 gate 已满足，WP-00 至 WP-90 已按顺序落地。

当前执行状态：

- [x] WP-00～WP-60：registry/Git/submodule 更新、Rig 0.41/RMCP 2.2 迁移；
- [x] WP-70：`gpui` 精确镜像、`chart.md`/attribution 同步、`gpui-form` metadata 校正；
- [x] WP-80：fresh provider-step 生命周期、continuation 列、usage 原子事务和删除 gate；
- [x] WP-90：GPT-5.6 capability/typed reasoning、官方 OpenAI Responses WebSocket、
  运行时 continuation、一次性 full-history fallback 和 session ownership；
- [x] 无凭据自动验证：workspace check、全 workspace clippy、core/DB/agent/Jaco tests、
  独立 MCP test-server、dependency tree、skill validator 和 diff checks；
- [ ] 真实 OpenAI API smoke：需要显式测试凭据，不属于无凭据 CI；不得在完成记录中伪报。

实现仍不包含 `safety_identifier`、用户可控 `reasoning.context`、产品 `pro` 选项、
Programmatic Tool Calling、多 agent beta、compaction 或新的缓存设置。

实施仍严格遵守“依赖先行”：

1. 先更新 registry crate、`gpui-component`/Zed GPUI、Lucide、Rig/RMCP，并完成每批
   breaking change、上游复用和 lockfile 审查。
2. 再同步复制型 skill/组件文档，并仅检查现有 `gpui-form` skill 是否因 adapter API
   变化而失真。
3. 最后才实施 GPT-5.6 capability、reasoning policy、provider-step schema、Responses
   WebSocket 和 `previous_response_id` continuation。

Rig 0.41 已经直接提供 GPT-5.6 model constants、typed reasoning、event-specific
`AgentHook`、structured tool output、RMCP registration 和 Responses WebSocket session。
计划因此删除旧草案中会重复实现这些能力的部分，只在 Jaco 保留产品策略、持久化、
审批、运行时 session pool 和 WebSocket event 到 Rig stream 的薄适配。

## 文档地图

- [dependency-evidence.md](dependency-evidence.md)：本次调查快照、release/changelog/
  migration/API 证据和风险分类。
- [dependency-refresh.md](dependency-refresh.md)：全部依赖更新的工作包、文件范围、
  Rig/RMCP breaking changes、命令、失败边界和验收。
- [upstream-reuse-audit.md](upstream-reuse-audit.md)：更新后应直接复用、保留或删除的
  本地实现清单。
- [skill-sync.md](skill-sync.md)：`gpui` 镜像、组件文档 A/M/D 同步，以及现有
  `gpui-form` skill 的窄范围 API 漂移检查和 metadata 小修。
- [gpt-5-6-adaptation.md](gpt-5-6-adaptation.md)：GPT-5.6、reasoning、WebSocket、
  continuation、数据库和运行时的实施级设计。

## 实施工作包

依赖关系：

`WP-00 -> WP-10 -> WP-20 -> WP-25 -> WP-30 -> WP-35 -> WP-40 -> WP-50 -> WP-60
-> WP-70 -> WP-80 -> WP-90`

`WP-10` 到 `WP-60` 都是依赖或上游内容工作包。按顺序执行可让每个 manifest/lockfile/API
变化都有单一来源；完成 `WP-60` 的 HTTP/SSE 等价迁移后，才进入 skill、数据库和
GPT-5.6 产品适配。以下段落保留执行前设计与验收依据，实际结果以上方状态和各专题文档的
“实施记录”为准。

### WP-00：执行前刷新窄范围证据

- 负责人输入：本计划和实施时仓库 HEAD。
- 操作：只重新确认计划已列出的 registry crate、Rig/RMCP 正式版本、目标
  `gpui-component` commit、其 lockfile 中的 Zed SHA 和 Lucide 稳定 tag；若目标变动，
  先更新证据表和影响分析再改 manifest。
- 禁止：重新运行一次无边界的“全部升级后再看哪里坏了”。
- 完成条件：每个目标都有确切版本/SHA、官方或包内证据、MSRV/features 和受影响文件。

### WP-10：公共 compatible registry 批次

- 执行 [dependency-refresh.md](dependency-refresh.md) DR-10。
- 范围：Serde/JSON/thiserror、async-trait/futures/Tokio、time/TOML/UUID 的全部 direct
  declarations；包含独立 `tools/mcp-auth-test-server` workspace 和 lockfile。
- 不改 public API、任务所有权、wire format、DB schema、UI、icon 或 i18n。
- 完成条件：所有 owner feature 原样、serialization/time/config/runtime focused tests
  通过，Rig/RMCP 保持旧版本。

### WP-20：proc-macro / Syn 3 breaking cluster

- 执行 DR-20：`proc-macro2 1.0.107`、`quote 1.0.47`、`syn 3.0.3`。
- owner 限定为 `crates/app-assets-macros` 与 `crates/gpui-form-macros`；保持宏输入、生成
  trait/method 和 compile-error 契约。
- 当前源码未命中 Syn 3 已知重命名/移除 API；先以 compiler、unit/trybuild 和展开结果
  证明，无错误时不修改 Rust 源码。

### WP-25：app asset/capture、搜索栈与 xtask

- 执行 DR-25A/B/C/D：`regex`、`rust-embed`、`xcap`，Jaco
  `globset/grep-matcher/grep-searcher/ignore` 原子栈，以及
  `clap/plist/tauri-bundler/which`；DR-25D 单独更新 MCP test-server 的
  `anyhow/axum/schemars`。
- 直接采用 rust-embed 重复资源/cross-compile 修复、ignore traversal 修复和 bundler
  AppImage 修复；不复制上游实现，不采用没有产品需求的新宏/compression/API。
- focused tests 覆盖 asset lookup、capture、`.gitignore`/glob/context/range/order、
  plist/AppImage 和命令探测。

### WP-30：`base64 0.23`

- owner 仅 `crates/jaco-agent`；显式使用
  `{ version = "0.23.0", default-features = false, features = ["std"] }`。
- 保留 scalar `STANDARD.encode`，不启用 0.23 默认新增但本地未使用的 `simd-unsafe`。
- image/document attachment 的 padded Base64 输出必须与 0.22 完全相同。

### WP-35：DuckDB、Diesel 与 SQLite native

- 依次独立更新 `duckdb 1.10505.0`、`diesel 2.3.11`；`libsqlite3-sys` 因 Diesel
  `<0.38.0` 与唯一 `links=sqlite3` 约束明确保留 `0.37.0`，分别审查 lockfile。
- Feiwen 已避开 DuckDB 的 `Statement::step`/Arrow breaking API，`Value` matches 已有
  catch-all；仍需 schema/query/fetch/native tests。
- Jaco 保留 Diesel features 和 Windows `bundled-windows`，检查唯一 `links=sqlite3`。
- 本工作包不修改任何 schema/migration；后续 provider-step fresh schema 不能混入。

### WP-40：`gpui-component` 与 Zed GPUI cluster

- 当前仓库 lockfile 基线是
  `gpui-component 5b45bcb26b9343d91a123a4d5ed8a654360512e5` +
  `Zed GPUI 1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- 2026-07-29 已审计目标是
  `gpui-component 57a9903f48160845aabc8b92a1e2f5348c80d439`；其 lockfile 仍解析同一个
  Zed GPUI `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- 根 `Cargo.toml` 已统一使用未固定 rev 的 git source；本批只更新 `Cargo.lock` 中三个
  gpui-component package 到同一目标 commit，不制造无效的 manifest SHA 改动，也不回退
  Zed GPUI。
- 先修复 GPUI/API 编译，再执行 [upstream-reuse-audit.md](upstream-reuse-audit.md)，
  最后才能同步 skill 文档。
- 如果实施时 `gpui-component/main` 已越过目标 commit，不得自动追到新 HEAD；先为新的
  commit 重做 compare、story/docs、Zed lockfile 和复用审计。

### WP-50：Lucide submodule

- 当前：`third_party/lucide` 精确 tag `1.21.0`；本地已获取的最新同系列稳定 tag 为
  `1.25.0`。
- 更新后逐个验证 app-local `IconName` slug；只更新运行时 Lucide 资源，不触碰
  `app/jaco/build-assets/icon/app-icon.png`。
- 无 UI 布局、用户文案或 i18n 变化。

### WP-60：Rig 0.41 / RMCP 2.2 迁移

- 用 `rig 0.41.0` facade 取代直接 `rig-core 0.39.0`，并将 `rmcp` 固定到 Rig 使用的
  `2.2.0`，避免并存两个 major。
- 先完成 core/agent split、`AgentRunner` builder、`AgentHook`、`DynamicTool`/
  `ToolContext`/`ToolOutput` 和 RMCP tool registration 的编译迁移。
- 本工作包不接入 WebSocket continuation；目标是让现有 HTTP/SSE 行为在新库上等价通过，
  并据此确认哪些本地 adapter 可以删除。
- 根 workspace 和独立 MCP auth test-server workspace 都更新到 RMCP 2.2；两份
  lockfile 不得残留 1.8 或引入 3.x。
- 具体 manifest、features、调用点和测试见
  [dependency-refresh.md](dependency-refresh.md)。

### WP-70：复制内容同步与 `gpui-form` 窄范围校正

- 所有 crate/Git cluster 更新完成后再同步，避免 skill 描述旧 API。
- `gpui` skill 对目标 `gpui-component/skills/gpui` 做完整镜像。
- `gpui-component-usage` 只镜像上游组件正文，本地索引、规则和 attribution 按所有权维护。
- `.agents/skills/gpui-form/` 已由其他提交完成，本 issue 不重构、不拆 references。
  仅在更新后公开 adapter API 使现有说明失真时定位修改 `SKILL.md`；可顺手修正
  `agents/openai.yaml` 中与当前架构不符的旧 “form draft” metadata。
- 完成条件详见 [skill-sync.md](skill-sync.md)。

### WP-80：provider step 生命周期与 fresh schema

- 先修复当前 streaming provider step 只在整次 run 末尾结束、tool loop 中间 step 可能
  长期保持 `running` 的问题。
- 直接修改初始 SQLite schema，加入 typed continuation 列、约束和索引；同步
  `jaco-core` payload/domain、`jaco-db` models/records/repository/schema/validation 和
  `AgentPersistence`。
- `provider_steps` 五态的 timestamp/response/error 组合一次约束完整；usage 与 step 定义
  为一对一，通过 `complete_provider_step_with_usage` transaction 与 continuation 原子提交。
- 应用尚未发布，不新增兼容 migration，不保留重复 JSON continuation；开发数据库需要
  删除后按 fresh schema 重建。

### WP-90：GPT-5.6 reasoning 与 Responses WebSocket

- 增加 GPT-5.6 family capability：`none/low/medium/high/xhigh/max`，默认 `medium`。
- `reasoning.context` 由运行时管理；`pro` 只有底层 typed mapping，产品/UI 不发送。
- 当前本地 BYOK 架构明确不生成、持久化或发送 `safety_identifier`；也不恢复旧 `user`
  字段。只有未来共享托管 API key/多用户代理架构才重新评估。
- 不设置 `prompt_cache_key`/retention；图片继续 `ImageDetail::Auto`，PDF 保持 typed
  document，不新增用户设置。
- 仅 OpenAI 官方 endpoint 的 GPT-5.6 family 选择 Responses WebSocket；自定义 base URL
  继续走现有 HTTP/SSE。
- 正常 continuation 只发送增量 input；ID 不可解析时基于结构化 provider error 做一次
  full-history 回退，不对含糊的 transport error 自动重放。
- 取消已发送但未 terminal 的请求时，先从 pool 移除并关闭 socket，再写 canceled 终态；
  55 分钟重连显式使用 run state 最新 response ID。
- 删除按钮保持可点击；数据库 transaction 发现 queued/running agent run 时返回 typed
  error，UI warning 要求先停止再删除，不切 route、不取消 run、不关闭 session。
- 详细类型、方法、session pool、数据流和测试见
  [gpt-5-6-adaptation.md](gpt-5-6-adaptation.md)。

## 系统面决策表

| 系统面 | 本轮决定 | 具体位置/边界 |
| --- | --- | --- |
| 文件/模块 | 依赖/skill 更新后，新增 `jaco-agent/providers/openai{,/websocket}.rs` 并同步 core/DB/runtime owner | 完整文件表见 GPT 计划；仍禁止 `mod.rs` |
| UI 组件 | 复用更新后的 `gpui-component`；GPT-5.6 不新增设置 UI | reasoning 沿用现有 picker；active-run 删除复用 `NotificationType::Warning`，按钮保持可点击 |
| 自定义类型/trait/method | 新增 typed continuation、reasoning policy、transport/session pool/WebSocket model+decoder，以及 session/connector test seam | 明确实现完整 `CompletionModel`/streaming bounds 和 native-output override；不重构现有 form skill |
| 数据流 | Rig AgentRunner 驱动 model/tool turn；Jaco adapter 负责逐 provider request 持久化和 continuation shaping | provider terminal + usage + continuation 先本地原子 commit，再前移 run state；WebSocket 只发送新 input；删除由 DB typed error 要求先停止再重试 |
| 全局状态 | 不新增 GPUI `Global`；`ConversationRuntimeStore` 持有进程内 OpenAI session pool | 每会话仍只允许一个 active run；gpui-form 不持有 locale/global，validation message 继续由 app 渲染时本地化 |
| 数据库 | 直接重写 fresh `provider_steps` 完整五态约束；usage 一对一；soft delete 增加 active-run gate | complete+usage+continuation 同事务；queued/running delete 返回 `ConversationHasActiveRun` 且零写入 |
| 数据获取 | 依赖证据来自 crates.io、正式 release/tag、发布包 source/lockfile；模型仍从 provider `/models` 获取 | GPT-5.6 alias/family 由现有模型列表自然进入 UI，capability 按 model id 补齐 |
| icon | 仅校验 Lucide slug | 不新增 icon，不改 app icon |
| i18n | reasoning 档位已有全部 key；新增 active-run 删除 warning 的中英文 title/message | 修改 `locales/{en-US,zh-CN}/main.ftl`；不改 macOS localization |
| 新依赖 | `rig 0.41.0`、`async-stream 0.3.6`；直接 `rmcp` 更新为 `2.2.0` | 不直接新增 `tokio-tungstenite`，由 Rig `websocket` feature 管理 |

## 总体验收

- 每个依赖目标均可追溯到 [dependency-evidence.md](dependency-evidence.md)，并记录无
  migration guide 时采用的替代证据。
- Cargo 图只出现一个预期 Zed GPUI SHA；`gpui-component`/assets/macros 来自同一 SHA。
- `libsqlite3-sys` 明确保留 Diesel 兼容的 `0.37.0`，native feature/唯一 links 经三平台
  CI 验证，数据库 schema 无无关 diff。
- Lucide 所有本地 icon slug 可解析。
- 上游复用审计逐项给出 Adopt/Adapt/Retain/Remove 结论，没有“以后看看能不能删”。
- skill 同步有确切 A/M/D 清单、attribution SHA 和链接检查；`gpui-form` 只有在可证明的
  adapter API 漂移或 stale metadata 存在时才做定位修改。
- Rig/RMCP 只解析到计划版本，Jaco 不再直接导入 `rig_core`；HTTP/SSE 回归通过后才进入
  WebSocket 工作包；独立 MCP test-server lockfile 同样解析 RMCP 2.2。
- 每次实际 provider request 对应一个终态 provider step；continuation selection、
  过期/失效、55 分钟重连、取消关闭和一次性回退均有指定文件/函数/fixture/断言的测试。
- provider 已成功但本地 complete transaction 失败时不重放 provider request、不执行
  tool，并关闭 WebSocket；step/usage 不出现半提交。
- GPT-5.6 alias/Sol/Terra/Luna 的 capability、reasoning request/response metadata、
  WebSocket tool loop、usage 和 unknown provider output 都有覆盖。
- active conversation 删除返回 typed error 和 warning，停止并达到终态后才能再次删除；
  `safety_identifier` 在当前 BYOK 架构中保持明确不发送。
