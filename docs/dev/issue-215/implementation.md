# Issue #215 实施记录

- 授权：2026-09-07 用户要求“按照计划实现”。本记录对应 WP-01–WP-06 及直接相关验证。
- 基线：`main@0dbe80e`；分支：`codex/215-gpui-kit-dependency-upgrade`。
- 当前：代码和资料已落地，受影响自动化验证及本地 Jaco 界面抽查已完成。用户已要求提交、推送并创建面向 main 的 PR；WP-07 本地 CI 对应检查及四应用 release 打包通过，远程三平台 CI 待 PR 触发。
- 上游固定证据：GPUI Kit `v0.6.0` / `94a313a72a2513aee2780240cd322d552b2395f0`，
  registry `gpui-pre` / `gpui-pre-macros` / `gpui-pre-platform` `0.3.3`。

## 依赖和共享边界

| 工作包 | 实施结果 |
| --- | --- |
| WP-01 | 四应用使用 `gpui_kit`、`gpui_kit::component`、`gpui_kit::assets`、`application()`/`init()`；共享 crate 保留 Rust 别名。删除 Zed Git patches；`profile.dev.package` 改为真实包名 `gpui-pre`。 |
| WP-01 features | 保留 Jaco 默认 basic 语言与 opt-in full 语言；HTTP Client 保留全语言。GPUI platform 的 font-kit、x11、wayland、runtime_shaders 在发布包图中存在；未引入 Web/Shell/WebView。 |
| WP-01 lock | `cargo update` 生成锁文件；仅一个 GPUI 类型来源，GPUI 与宏均为 0.3.3；component/kit/assets 均为 0.6.0；剩余 Git 包数为 0。 |
| WP-02 assets | `app-assets::__private` 隐藏导出宏所需 AssetSource、Result、SharedString、IconNamed；宏通过该路径展开。保留 app-local 图标、provider SVG、组件 fallback 顺序。 |
| WP-02 Form | `FormInput` / `FormTextarea` / `FormEditor` 共享 `ControlBinding` 和 `InputEvent::Change`/`Blur` 路由，保留静默投影、动态路径及解绑契约；公共 README/guide 中英文同步。 |
| WP-02 theme | 删除 0.6.0 已移除的 accordion_hover 字段；其他 Material/主题映射保留。 |
| WP-02 Tokio | 保留本地 bridge 与 owned/external runtime、JoinError、drop-to-abort 契约；新增三个对应回归。 |
| WP-05 普通依赖 | 按 [draft §6](draft.md) 固定数值更新直接依赖，配套解析间接依赖；应用调用点通过受影响测试验证。Diesel 2.3.13 与 SQLite 0.38.2 配套，唯一 `libsqlite3-sys`；Feiwen 继续使用 DuckDB。 |
| WP-05 Rig/MCP | Rig / rig-core 0.42.0、RMCP 2.2.0 保留；独立 `tools/mcp-auth-test-server` 的 manifest/lock 不变。 |
| WP-05 Windows | bindgen 0.66.0 使用 `--no-allow`，删除生成后字符串裁剪。0.100.0 移除既有参数并要求不同 runtime API，未采用。保留 windows-core 0.62.2 / windows-future 0.3.2；没有 Windows 实机证据。 |

## 应用迁移与删除边界

应用 `src` 下普通差异为 import/初始化入口迁移；行为改动集中在以下文件。

| Owner/文件 | 结果与业务边界 |
| --- | --- |
| Jaco `features/settings/prompts/dialog.rs` | 多行正文改为 FormTextarea/Textarea，名字保留 FormInput/Input；保存、校验、rebase 与保存中关闭保护保留。 |
| HTTP `features/request/body/http_text.rs`、`response/viewer.rs` | 请求文本使用 FormEditor/EditorState；响应使用只读 Editor，支持选择与复制。 |
| Jaco `features/home/sidebar/search.rs` | Command 负责输入/匹配列表/键盘/焦点。关闭本地二次过滤；DB/Operation 负责标题、项目和正文搜索、取消、过期结果和错误恢复。确认捕获与该次 Command model 相同的业务 ID 快照。删除旧 delegate 和键盘选择样板。 |
| Jaco `app/title_bar_menu.rs` 及 home/settings/about | 改用 AppMenuBar；保留菜单定义和应用图标 leading 外壳，删除本地菜单状态机。 |
| Jaco `components/picker.rs` 及 Form/run settings/new conversation | 改用 ComboboxState 与上游 SearchableVec/SearchableGroup；只保留领域选项、禁用策略和 Change 订阅。按完整目录中的稳定值投影并恢复 query；取消 Confirm 不写入。旧列表、controlled-open 协调和无消费者的 popover helper 删除。 |
| Jaco `components/chat/detail.rs` / `detail/*` | Message/Bubble/Attachment/Marker 承载展示；业务行键、文本状态、附件访问、复制失败与审批入口保留。真实活跃 Running 且不等待审批时启用 Shimmer。 |
| Jaco `components/chat/detail.rs` | MessageScrollerState 承担列表 splice/remeasure/follow；页面观察状态以更新跳转按钮。采用向上阅读暂停、回到底部恢复的上游语义；en-US/zh-CN 增加 jump-to-latest 标签。 |
| Jaco `components/chat/input/composer_editor*` | 用限帧 GPUI Animation 替换 BlinkCursor entity/timer；用 UndoHistory 保存 before/after 编辑事务。原子 token、选区、IME 组合提交与撤销后的新分支保留。 |
| Jaco `components/hotkey_input.rs` | 显示委托 Kbd::format；录制、校验和序列化保留。 |
| Jaco `app.rs`、Feiwen `main.rs` | 自绘 TitleBar 窗口采用 `TitleBar::window_options()`；保留应用标题和 bounds。Jaco temporary 窗口没有 TitleBar，保持原原生配置。 |
| Feiwen `features/query/advanced/render.rs` | Combobox trigger 使用公开 `selection()`；动态 Form、查询重排和 Fetch 业务保留。 |

## 资料同步

- GPUI references：22 篇，从 `skills/gpui-kit/references/gpui/*.md` 同步。
- 组件 references：68 篇，从 `website/docs/components/*.md` 同步；其中新增 8 篇
  Attachment、Bubble、Command、Marker、MessageScroller、Message、Shimmer、Textarea。
- 两个官方文件集合及正文逐文件比较通过；没有删除项。保留本地组件 index、rules 和两个独立 skill 入口。
- 更新来源仓库、路径、release、SHA 与 Apache license 副本；同步本地导航、入口、资产和主题缓存版本说明。
- 官方快照原文保留 5 处末尾空行：action、async、event、focus-handle、global。
  `git diff --check` 会提示这些上游空行；本地手写变更单独检查，不修改官方正文来消除提示。

## 已执行验证

命令中的 `-p` 组合用于一次覆盖关联契约；没有在同一状态上叠加全 workspace 最终门禁。

| 命令/检查 | 结果 |
| --- | --- |
| `cargo check -p gpui-form-gpui-component -p app-assets --locked` | 通过。 |
| `cargo check -p jaco -p http-client -p novel-download --locked` | 通过。 |
| `cargo check --workspace --all-targets --locked` | 首次发现 Feiwen 的私有 `selection` 字段访问；已改为 `selection()`，后续 Feiwen 测试编译与运行通过。未重复整个检查。 |
| `cargo test -p jaco -p gpui-form-gpui-component -p gpui-tokio -p app-assets -p app-assets-macros -p jaco-agent -p jaco-db --locked` | app-assets 7、adapter 单元 2/集成 17 通过；Jaco 580 通过、2 ignored、1 旧定位断言失败；定位修正后单独重跑通过。后续包因该失败未执行，移至下一命令。 |
| `cargo test -p gpui-tokio -p jaco-agent -p jaco-db -p http-client -p feiwen -p novel-download -p gpui-form-macros -p gpui-heatmap -p xtask --locked` | 通过：Tokio 3、Jaco Agent 164、Jaco DB 87、HTTP 163、Feiwen 95、Novel Download 41、Form macros UI harness 1、Heatmap 10、xtask 12；对应 doc-tests 通过。 |
| `cargo test -p jaco composer_context_occupancy_precedes_the_model_selector --locked` | 修正旧 Picker 定位后通过。 |
| `cargo test -p jaco features::home::sidebar::search --locked` | 通过；验证项目独有命中不被组件过滤、查询替换、空结果、失败重试和键盘确认正确业务 ID。测试显式导入以避免 GPUI test 宏递归，窗口使用实际组件 Root。 |
| `cargo tree --locked --offline -p http-client -e features -i gpui-pre-platform` | font-kit/x11/wayland/runtime_shaders 全部存在。 |
| `cargo tree --locked --offline -p jaco -e features -i gpui-kit` 和 lock 解析 | 默认 basic 语言映射、单一 GPUI/宏/SQLite/Rig/RMCP 来源通过。 |
| 官方集合/逐文件字节比较、本地 skill/index 链接 | 22/68 篇 exact match，导航链接通过。 |
| changed Rust 文件 `rustfmt --edition 2024 --config skip_children=true` | 已格式化当前修改范围。 |

Jaco 测试链接器报告 debug `__eh_frame` 超过 compact unwind 编码范围的性能警告；
Cargo 另报告依赖 `block 0.1.6` 的 future-incompatibility 提示。测试没有因此失败。

## 平台与运行观察

- `cargo run -p xtask --locked -- bundle jaco`：通过；未使用 `--install`。
- 产物：`target/release/bundle/macos/Jaco.app`；可执行文件 SHA-256：
  `b80155bbfd9a45af3e5a64aa1083f5e2a842468ac0bcc363ebe634106ff748e4`。
  按绝对路径连接，启动进程及合成项目记录确认产物身份。
- 隔离：`JACO_CONFIG_DIR` / `JACO_LOG_DIR` 指向本轮 `/private/tmp/gpui215-ui-jch7v8n_`；
  使用测试数据库、两个合成模型和 loopback Ollama mock，无真实 provider 凭据。
- 模型 Combobox：中文搜索正确筛出备用模型，Escape 取消后保留原模型。
- 消息：中文 Markdown、代码高亮、用户 Bubble、静态 Marker、运行中 Shimmer 和结束状态正常显示。
  上翻后新内容继续生成且阅读位置保持；跳转按钮可回到最新内容。
  通过滚动到实际底部恢复自动跟随，随后无滚动操作时视口由第 53 段推进至第 69 段。
  长消息重新测量高度时一次大幅滚动可能未到实际底部，继续滚到底部后跳转按钮消失并恢复跟随。
- 提示词 Textarea：多行输入、撤销、重做、保存成功；重新打开后两行正文完整保留。
- 测试应用已退出，mock 已停止，隔离数据已清理；日志及产物身份记录保留在
  `/tmp/gpui215-runtime-evidence`，构建日志为 `/tmp/gpui215-jaco-bundle.log`。
- Windows/Linux 原生生成、窗口拖动/菜单实机验证、MCP E2E 与远程三平台 CI：未执行。四应用 macOS release 打包结果见下文。
- 上述为本轮定向界面观察；附件打开、审批交互、IME 实机输入、其他应用界面未作手工覆盖。
  自动化回归及这些局部观察不代表全应用 UI 验收。

## PR 阶段验证（2026-09-07）

- 用户授权：提交、推送并创建面向 `main` 的普通 PR。
- 已刷新 `origin/main`；分支与远程基线相同，无额外提交或待合入更新。
- `cargo fmt --all -- --check`、`cargo build --workspace --locked`：通过。
- 全 workspace 测试发现主题颜色哈希仍包含上游已删除的 `accordion.hover.background`。
  对比新旧 ThemeConfigColors 字段，并按原顺序补回旧颜色后，浅色/深色旧哈希均匹配；
  已移除临时对照代码，仅更新新版字段集合对应的两个哈希。其余颜色值保持不变。
- `cargo test --workspace --locked`：通过，55 个测试目标合计 1,409 passed、2 ignored、0 failed，含 doc-tests。
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`：通过。
- 当前本地文档 392 个链接目标检查通过；手写差异 whitespace 检查通过，官方快照的 5 处末尾空行按原文保留。
- 用户确认一并提交当前全部改动，包含此前 AGENTS.md 与 skill 精简整理。
- `cargo run -p xtask --locked -- bundle http-client`、`bundle feiwen`、`bundle novel-download`：全部通过。
  加上此前本轮相同运行源码的 Jaco bundle，四应用 macOS release 打包均通过；未安装。
  日志：`/tmp/gpui215-final-bundle-{http-client,feiwen,novel-download}.log`。
- 本地最终日志：`/tmp/gpui215-final-{build,test,clippy}.log`。Windows/Linux 和远程 CI 结果由 PR 后续检查确认。

## PR #216：测试遗留路径清理（2026-09-07）

- Linux/Windows 的生产 build 已通过；test 编译失败于仅非 macOS 测试引用的旧 `ConfirmDialog`。
  删除旧 action 引用，改用真实 Enter 按键；保留异步完成前不关闭、成功后关闭的断言，并让测试在 macOS 同样编译运行。
- 对四应用生产目标及依赖使用 `--force-warn dead_code` 排查，结合调用点区分生命周期持有字段、测试设施与无生产入口的实现。
- 删除仅被测试调用的 `model_select_groups` 旧 Select 分组适配及其专用测试，保留生产使用的 `model_sections`。
- 删除仅被测试设置的 OpenAI `with_mode`、mode 字段及序列化分支；生产配置一直使用 None，effort/context/store 行为保留。
  同步移除 WebSocket 测试夹具中的旧字段；保留针对实际生产映射的断言。
- 未删除仍服务生产删除/归档流程的异步确认逻辑，也未删除用于维持订阅和控制器生命周期的持有字段。
- 定向验证通过：`cargo test -p jaco --locked components::delete_confirm::tests`（4）、
  `cargo test -p jaco --locked components::chat::model_picker::tests`（7）、
  `cargo test -p jaco-agent --locked providers::openai`（18）。
- `cargo clippy -p jaco -p jaco-agent --all-targets --all-features --locked -- -D warnings` 及变更差异检查通过。
  Linux/Windows 运行结果仍需更新后的 PR CI 确认。
