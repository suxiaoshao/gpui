# Issue #205 依赖升级：上游能力复用审计草稿

## 文档状态

- 状态：`Draft`
- 关联 issue：[#205](https://github.com/suxiaoshao/gpui/issues/205)
- Root hub：[Issue #205](README.md)
- Canonical dependency plan：[全 workspace 依赖升级计划](dependency-upgrade-plan.md)
- 专项审计：[a11y 与 Command 复用审计](accessibility-and-command-reuse-audit.md)
- GPUI 基线：`1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`
- GPUI 目标：`e0931d5a9dbf4f781b336fdf448739e74a2ac0b5`
- gpui-component 基线：`57a9903f48160845aabc8b92a1e2f5348c80d439`
- gpui-component 目标：`5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3`
- Registry 证据：正式发布包与本仓 target 表，刷新日期 `2026-08-20`

本文是本轮依赖升级的 deletion-first 复用审计：先找出上游已经承担的通用行为，再决定本地代码应直接删除、
缩成领域 adapter、继续保留或延后。它不是 Lestty 的终端内核、配置或主题草稿，也不授权把现有应用迁到
`gpui-base`。

## 判定规则

| 结论 | 含义 | 实施约束 |
| --- | --- | --- |
| `Reuse directly` | 上游公开 API 已覆盖本地通用行为 | 删除重复实现和重复测试，只保留调用点与产品回归 |
| `Adapt` | 上游覆盖交互/协议骨架，但领域 authority 仍在本仓 | 本地只保留 identity、持久化、错误、策略或投影 adapter |
| `Retain` | 上游能力与本地语义不同 | 保留实现，记录为什么相似 API 不能替代 |
| `Defer` | API 接近，但仍有已知语义、a11y 或稳定性缺口 | 不在升级批次强迁；写清重新审计的解除条件 |

每项还必须注明能力来源：

- `Target-new`：基线到目标之间新增，是真正的升级收益；
- `Target-hardened`：基线已有，但目标修复了阻止复用的行为或补了公开 accessor；
- `Pre-existing`：基线已有，只是本仓仍维护重复实现，不能误称为目标新增。

只有能定位原始目的、全部 consumer、等价上游契约和 focused validation 时，才允许删除本地实现。名字相似不等于
语义相同。

## 结论摘要

最值得在本批执行的删除或收薄工作是：

1. 删除本仓 `crates/gpui-tokio` fork，直接依赖同一 Zed revision 的上游 `gpui_tokio`。
2. Jaco 会话搜索使用新 `Command`，删除手写 Input/List/delegate/键盘/选择样板，保留数据库查询和 operation。
3. Jaco 标题栏菜单改用上游 `menu::AppMenuBar`，删除本地整套菜单状态机和递归 popup 构造，只保留 app icon
   与 titlebar leading wrapper。
4. Jaco、Feiwen 的自绘标题栏窗口以新 `TitleBar::window_options()` 为基线，删除重复的 titlebar options 和
   `app_owns_titlebar_drag` 手写组合。
5. Jaco 主题切换把应用主题同步给新的 native window appearance API。
6. `jaco-agent` 消费 Rig 0.42 的标准 `raw`、`response_id`、`provider_request_id` 和 `ResponseIdentity`，删除
   私有 response-id metadata 注入及旧字段形状 probe；WebSocket pool、live stream 和持久化仍归 Jaco。

Picker、系统通知、时间线跟尾和 cursor animation 都有未决的产品语义，只记录解除条件，不作为本批删除目标。

`ComposerEditor`、`HotkeyInput`、`window-ext`、`platform-ext`、嵌套滚动链、OAuth 测试服务和
Feiwen 的领域重排逻辑没有等价替代，不应为了减少行数而迁移。

## 总决定表

| ID | 上游能力与来源 | 本地目标 | 决定 | 删除与保留边界 |
| --- | --- | --- | --- | --- |
| `UR-205-01` | `Command` + `filterable(false)`；`Target-new` | Jaco `ConversationSearchView` | `Adapt` | 删除 Input/List/delegate/actions/helpers；保留 DB search、`refresh::Operation`、stale/error/retry、i18n 与 ConversationId 映射 |
| `UR-205-02` | `menu::AppMenuBar`；`Pre-existing`，基线已含 a11y/action-context 修复，目标仅再加 axis lock | Jaco `app/title_bar_menu.rs` | `Adapt` | 删除本地 actions、menu entities、popup 递归和 trigger；仅保留 app icon/chrome wrapper |
| `UR-205-03` | `Combobox`；`Pre-existing`，目标新增 query accessor 并修 trigger/focus | Jaco `components/picker.rs` | `Defer` | 先另立 follow-up；typed projection、read-only reason 与 arbitrary-content/controlled-open seam 尚无等价 API |
| `UR-205-04` | `TitleBar::window_options()`；`Target-new` | Jaco 三个窗口、Feiwen 主窗口 | `Reuse directly` | 以 helper 作为 struct update base，只保留 owner-specific bounds、size、position 和 traffic-light override |
| `UR-205-05` | component system notification bridge + GPUI app identity；`Target-new` | Jaco global-hotkey background failures | `Defer` | component delivery 仍从某个 Window 的 NotificationList 发出，不能直接删除无 Root 分支；先固定无窗口投递与点击语义 |
| `UR-205-06` | `ListItem: InteractiveElement`；`Target-new` | Feiwen advanced sort row | `Defer` | 可评估用 ListItem 收回 row shell；`PathKey`、form fields、drag preview 和 reorder mutation 必须保留 |
| `UR-205-07` | Input/Textarea/Editor split、readonly、transaction undo、context-menu builder；`Target-new` | 普通多行/代码输入 | `Adapt` | 完成 required API migration，并保留各 form/viewer owner 的 validation、readonly 与内容 authority |
| `UR-205-08` | unified editor engine；`Target-new` | Jaco `ComposerEditor` | `Retain` | atomic skill token、history、attachments、completion、IME/selection coupling 没有上游等价契约 |
| `UR-205-09` | TextView link/table/selection/local-image/large-parse 能力；`Target-new` | Jaco Markdown timeline | `Retain` | 直接继承新增能力，但没有等价本地 workaround 可删；timeline source/observer/remeasure 继续保留 |
| `UR-205-10` | component `Clipboard`；`Pre-existing` | Jaco timed copy button | `Retain` | 上游未覆盖本地写后校验/失败提示，icon-only accessible name 也未闭合 |
| `UR-205-11` | AccessKit 与 target component roles/actions | app-owned custom UI | `Adapt` | 复用已有 semantic primitives；accessible name、自绘复合控件和 screen-reader 验收不能删除 |
| `UR-205-12` | GPUI native window/scroll/drag/test additions；`Target-new` | `window-ext`、`platform-ext`、nested scroll | `Retain` | 新 API 是补充或测试能力，未覆盖本地 native window level/hide、accent observer 或 residual scroll chaining |
| `UR-205-13` | `ListState::pause_following_tail()`；`Target-new` | Jaco timeline 两处 `set_follow_mode(Normal)` | `Defer` | helper 能冻结当前位置并在回到底部后恢复 Tail；先确认是否要改变当前“永久 Normal 到下次提交”的语义 |
| `UR-205-14` | `App::set_window_appearance`；`Target-new` | Jaco explicit/system theme binding | `Adapt` | Light/Dark 投影到 native appearance，System 传 `None`；系统主题观察与 `platform-ext` accent 仍保留 |
| `UR-205-15` | synced repeating animation + max FPS；`Target-new` | Jaco custom cursor blink task | `Defer` | 可用 `repeat_synced().with_max_fps(2)` 收回递归 500ms timer；保留输入后 300ms 常亮恢复策略并先做功耗/节拍测试 |
| `UR-205-16` | Zed `gpui_tokio`；`Pre-existing` | 本仓 `crates/gpui-tokio` fork | `Reuse directly` | 根依赖改为同一 Zed Git source；删除本地 member、manifest 与源码，保留 owner docs 作为退役证据；imports 保持 `gpui_tokio` |
| `UR-205-17` | raw-window-handle 0.6 `HasWindowHandle`；`Pre-existing` | `window-ext` deprecated compatibility layer | `Adapt` | 删除 crate-wide deprecated allow 与旧 `HasRawWindowHandle` path；native window capability 本身继续保留 |
| `UR-205-18` | `gpui-base::TextSelection`；`Target-new` | Lestty future terminal selection | `Defer` | 先做 mapping spike；terminal grid/cell/line/block selection authority 仍属于 terminal core |
| `UR-205-19` | gpui-base behavior primitives；`Target-new` | Lestty app chrome | `Reuse directly` | 复用 Button/Dialog/Popover/Input/Textarea/Editor/Scrollbar/AutoScroll/VirtualList/Tabs 等行为；presentation、theme 和 terminal surface 归 Lestty |
| `UR-205-20` | Rig 0.42 normalized response、raw payload、response identity；`Target-new` | Jaco provider response side channels/probes | `Adapt` | 删除私有 response-id metadata 与旧字段 probe；持久化 schema 通过显式边界转换保持稳定 |
| `UR-205-21` | Rig OpenAI Responses WebSocket normalization；`Target-hardened` | Jaco OpenAI WebSocket adapter | `Adapt` | 复用公开 RawStreamingChoice/normalize_stream/ModelTurnFinished；保留 event decoder、pool、expiry、continuation recovery 与 persistence policy |
| `UR-205-22` | `similar 3.2` `TextMerge`、`WhitespaceMode`、`RawMyers` | filesystem tool unified diff | `Adapt` | owner 已要求 diff text 不变，显式选择 `RawMyers` 并补复杂/重复行 snapshot；新三方 merge 无删除目标 |
| `UR-205-23` | RMCP 3.1.4；独立 breaking target | test server transport | `Adapt` | 采用 renamed legacy-session builder 与默认 loopback Host guard，并做 2.x client/3.x server E2E |
| `UR-205-24` | RMCP auth client/resource-server surface | OAuth authorization-server fixture | `Retain` | RMCP 不提供本地 authorization server；保留 routes/state/token fixture |
| `UR-205-25` | 其余 registry patch/minor API | 各 owner | `Retain` | 仅做 compatibility/behavior regression；没有证据时不发明 wrapper 替代项目 |

## GPUI 与 gpui-component 详细决定

### UR-205-01：Command 只替代会话搜索的通用交互层

`Command` 拥有 query、selection、滚动、loading、Up/Down、Enter、Escape、group、disabled item、custom row 和
虚拟列表。它不拥有 Task、debounce、数据库、取消、错误、重试或业务 identity。

Jaco 数据库还会匹配项目和消息正文，因此必须使用 `.filterable(false)`，以数据库作为唯一过滤 authority；
`Command` 没有可注入的 matcher/scorer callback。精确交互、过滤和 a11y 边界由
[专项审计](accessibility-and-command-reuse-audit.md)拥有。

### UR-205-02：删除 Jaco 的 AppMenuBar fork

`app/jaco/src/app/title_bar_menu.rs` 与上游 `menu::AppMenuBar` 重复拥有：菜单 reload、左右键、Escape、焦点
恢复、hover 切换、`OwnedMenu` 到 popup/submenu 的构造、dismiss 和 trigger。

基线 `AppMenuBar` 已经比本地 fork 多出：

- `Role::MenuBar`；
- mouse-down toggle 与 click 去重，避免一次鼠标操作切换两次；
- 复用 popup 时重新写入 action context，并带回归测试；
- titlebar child 阻止窗口拖动的处理。

目标 revision 在这些既有能力上只新增横向滚动 axis lock。实施价值主要来自删除本地重复状态机，不能把基线
已经存在的 a11y/action-context 修复记成本次升级新增。

实施时 `title_bar_leading` 改为接收 `Entity<gpui_component::menu::AppMenuBar>`；保留 16px app icon、间距和
leading 区域的 drag propagation guard。Home、About、Settings 三个 consumer 改用上游类型和 `reload`，删除
本地 `init` 调用及整份状态机。

### UR-205-03：Picker 具备收薄潜力，但本批 Defer

Jaco `components/picker.rs` 当前重复实现 sections、query filtering、selected index、List delegate、键盘选择、
Popover、empty、disabled 和 custom row。上游 Combobox 已覆盖这些通用职责，并允许自定义 searchable item、
group、trigger、footer 和 empty view。目标新增的 `query()` / `set_query()` 使 catalog replacement 后保持 query
成为可表达行为，trigger dismiss/focus 修复也降低替换风险。

不能直接删除的边界：

- form/draft 拥有的 typed selected value；
- `replace_projection` 的 catalog + selection 原子投影；
- per-item selectable 与 control-level read-only reason；
- 领域 item 的双行/状态渲染与 confirm mapping；
- `picker_content_popover` 的任意内容模式；若它不是 searchable selection，应继续直接使用 Popover，而不是
  强塞入 Combobox。

这项约 940 行的产品重构不作为依赖升级 blocker。独立 follow-up 应先做一个 consumer vertical slice，证明
query、selection identity、catalog replacement、read-only tooltip 和 focus return 后再删除 generic shell；
Issue #205 只保存 contract inventory。

### UR-205-04：统一自绘标题栏 WindowOptions

目标 `TitleBar::window_options()` 同时设置透明 titlebar options 与 `app_owns_titlebar_drag = true`，正是当前
Jaco/Feiwen 多处手写组合。所有渲染 component `TitleBar` 的窗口都以该 helper 为 base，再覆盖 app-specific
字段。Feiwen 仍可覆盖 traffic-light position；Jaco temporary/screenshot/native-titlebar 窗口不应误用 helper。

目标 TitleBar 还会按 window manager/server-side decorations 和窗口可 minimizable/resizable 状态决定是否绘制
控制按钮。应用不复制这些平台分支，只做 Windows/macOS/Linux drag、double-click、按钮可见性与关闭回归。

### UR-205-05：系统通知是新能力，但不能直接删除 Root-window hunt

Jaco 全局快捷键可能在没有主窗口时失败；当前 `GlobalHotkeyState::push_notification` 会遍历 active/all windows
寻找 `Root`，找不到就只写错误日志。目标 component 可以把 `NotificationList` 收到的通知桥接到系统通知中心，
并保留 click 激活/双投递语义；但 `NotificationList::push_system` 本身仍绑定 posting window 和 list entity，
所以它不能直接解决“进程没有 Root window”的现有失败分支。

若后续产品决定补系统级后台通知，应先选定一种公开边界：保留/创建 notification host window，或直接使用 GPUI
system-notification API 并自行设计 tag、点击和激活。随后再考虑 `.system()` / `.in_app_and_system()`：

- `gpui_component::init` 已注册 app-global system-notification response handler，应用不能再注册第二个 handler
  覆盖它；若走 direct GPUI tag，必须与 component handler 的 ownership/ignore 规则一致；
- Windows 在 app 初始化早期设置稳定 app identity；
- macOS bundle/授权限制纳入 bundle smoke，不能用 `cargo run` 成功作为验收；
- Linux 没有 notification daemon 时保留诊断日志；
- 明确 click 是激活已有窗口、打开临时窗口还是只 dismiss。

普通前台保存成功/表单错误 toast 继续 in-app；不以升级为由制造双重通知。Issue #205 的依赖升级只记录平台
前置与现状回归，不把这项产品行为作为强制迁移。

### UR-205-06 至 UR-205-12：明确保留边界

- Feiwen drag sort：ListItem 现在可直接挂 drag handlers，但上游没有领域 reorder state machine。只有当
  `ListItem` 能容纳完整 form row 时才删除 row hover/border shell。
- 普通编辑器：HTTP Client response/body 与 Jaco prompt 按目标 API 分别迁到 `Editor` / `Textarea`；这是 breaking
  API 适配，不代表删除 form/viewer 的 validation、readonly 和内容 owner。
- Jaco ComposerEditor：统一输入 engine 没有 atomic inline skill-token/decorator contract，也不拥有附件、历史、
  completion/detail popup 和提交语义。transaction undo 不能单独恢复 token state，Issue #205 不迁。
- TextView：新 link/table/source-copy/local-image/large-parse 能力直接消费；现有 timeline 的 streaming source、
  append/replace 和重测 owner 不是重复 parser，继续保留。
- Clipboard：上游 timed copied-state 很接近本地 CopyButton，但本地还会读回校验并显示失败。上游 callback 目前
  不是 fallible，且 icon-only Button accessible name 仍需修复，暂不直接替换。
- HotkeyInput：它捕获并规范化组合键，不是文本 Input 或 Command，保留。
- `window-ext`：native hide/show-without-activation、window level、crosshair cursor、Quick Look 等没有被目标 GPUI
  覆盖；`platform-ext` 的 system-accent observer 也不等于 `set_window_appearance`。
- nested scroll：目标 axis lock 和 div scrolling 改进不提供父容器 residual delta chaining，Jaco 的定向链路保留。

另外有三个值得单独排期的小型收薄点：

- Jaco timeline 当前两处用 `set_follow_mode(Normal)` 停止自动跟尾。目标新增的
  `pause_following_tail()` 会在离开底部时冻结、手动回到底部后自动恢复 Tail。它会改变当前
  “直到下次提交才恢复”的行为；确认产品语义并补展开/折叠、手动回底测试后再替换。
- 目标 GPUI 新增 `App::set_window_appearance`。Jaco 明确 Light/Dark 主题时同步 native appearance，System 模式
  传 `None`，可修正 macOS 原生边框/titlebar；`WindowThemeBinding` 和系统 accent observer 仍是不同职责。
- target 的 synced repeating animation 与 max-FPS 可让多个 cursor blink 共用 2 FPS phase，而不是每个 entity
  递归持有 500ms Task。先证明能保留输入后 300ms 常亮恢复和窗口 inactive 行为，再删除 `blink_cursor` timer。

### UR-205-16 至 UR-205-19：删除本地 bridge，并保留正确的 native/terminal authority

Zed target 自带 `gpui_tokio 0.1.0`，公开的 `init`、`init_from_handle`、`Tokio::spawn`、`Tokio::handle` 和
`JoinError` 覆盖本仓同名 crate 的完整 API，Task drop 时 abort Tokio task 的语义也相同；上游还增加
`Tokio::spawn_result`。本地额外的 `AbortOnDrop` 只是对上游 `gpui_util::defer` 的重复实现。

根 workspace 应把现有 dependency key 映射到同一 Zed Git source 的 package `gpui_tokio`，从 members 删除
`crates/gpui-tokio`，随后删除本地 manifest 与 `src/`。`docs/dev/issue-205` 作为退役决策、执行结果和最终证据
保留，不允许删除后让 root owner map 悬空。Jaco、HTTP Client 仍可使用 crate name `gpui_tokio`，无需引入兼容
facade。

删除前后必须验证：runtime shutdown、GPUI Task drop 触发 Tokio cancellation、normal/panic/JoinError、HTTP/MCP
网络 I/O 和 timer。上游 package 只显式启用 Tokio `rt,rt-multi-thread`；Jaco/HTTP Client 必须各自在自己的
manifest 声明实际使用的 feature 子集（Jaco 当前为 `io-util/net/sync/time`，HTTP Client 为
`fs/io-util/time`，测试另需 `net`），不能依赖另一个 workspace member 的 feature union。
上游新增的 `anyhow`/`gpui_util` 由同一 Zed graph 拥有；最终 lock 只允许一个 GPUI source identity。

`window-ext` 仍须保留，但可以删除一层历史兼容：用 raw-window-handle 0.6 的
`HasWindowHandle::window_handle(...).as_raw()` 替换旧 `HasRawWindowHandle`，移除 crate-wide deprecated allow。
该能力在当前 GPUI 已存在，并非目标新增；target `TestWindow` 的 raw handle 仍会 panic，测试不能伪造调用该
native path。

Lestty 应直接消费新 gpui-base behavior primitives。`TextSelection` 可承担 window-level gesture、UTF-8 range
projection、copy 与 auto-scroll，但它不知道 terminal grid、wide cell、wrapped line、block selection 或 alternate
screen；先做 terminal coordinates 到 text layer 的 mapping spike，不能把 terminal-core selection authority
直接移走。

## 其他依赖详细决定

### UR-205-20/21：Rig 0.42 先删除旧 provider side channel，再迁类型

Rig 0.42 的 normalized `CompletionResponse` 和 `StreamFinal` 已公开：

- `raw: serde_json::Value`；
- `response_id` 与 `provider_request_id`；
- `identity()`；
- normalized choice/finish reason/usage；
- stable streaming part identity/lifecycle。

因此 `jaco-agent` 不再把 OpenAI response ID 塞进私有 `__jaco_response_id` reasoning metadata，也不继续在
`provider_step.rs` 探测旧字段形状。provider raw audit 和 continuation identity 优先消费 Rig 的公开字段，再在
Jaco persistence 边界做显式、可测试的 domain conversion。

但 Rig 0.41 已经包含 OpenAI Responses WebSocket session，本仓也已经基于它实现 adapter；不能把整个
`providers/openai/websocket.rs` 误判为 0.42 可删除。0.42 的 session 仍没有直接返回 live partial
`StreamingCompletionResponse` 的高层方法，因此以下职责继续归 Jaco：

- per-conversation session pool、API key/expiry/eviction；
- persisted previous-response lookup、失效与 rejected-continuation full-history fallback；
- live `send + next_event` 到 Jaco stream 的 adapter；
- persistence attempt coordination、document/history suppression 和 product reasoning policy。

精确 `0.42.0` 还有一个必须规避的契约缺口：WebSocket `session.completion()` 返回 normalized response，却没有像
HTTP model 一样把 captured provider JSON 写入 `CompletionResponse.raw`，会丢 reasoning context。因此当前不能用
它直接替换本地 blocking path。可执行的 0.42 路线是：保留 Jaco 的 public `send + next_event` 事件解码器，让它
产出公开 `RawStreamingChoice<OpenAiStreamingResponse>`，再经公开 `normalize_stream` 和
`StreamingCompletionResponse::stream` 收束；前者捕获 terminal raw，后者用共享 `PartsAccumulator` 组装 tool
fragments 并 mint internal correlation ID。这样不需要调用或复制私有 `RawChoiceAccumulator` / `ResponsesAdapter`。

在这个前提下可删除/合并的是：旧 blocking 双循环、`__jaco_response_id` 注入及 probe、blocking/streaming 两套
attempt-complete side channel、runtime `final_raw_response` 和 stream-finish hook；使用 Rig 0.42 公开的
`ModelTurnFinished`（同时提供 content、usage、identity、finish reason 与 raw）统一成功提交，并从
`identity.response_id + raw` 构造 continuation。非 fallback transport error 还必须 evict 已标记 failed 的 session，
避免复用死连接。

event decoder、transport/pool 和产品 persistence policy 继续保留。删除量以公开 API 实际可表达和 parity
fixtures 为准；若 `RawStreamingChoice -> normalize_stream -> StreamingCompletionResponse` 这条公开链路不能通过
tool/reasoning/raw parity tests，就停止删除 finish side channel，而不是复制 Rig 私有实现。

迁移还要按 0.42 的统一流语法删除本地 `tool_call_internal_ids` minting，让 shared normalization 拥有内部 tool
identity；item mapper 本身仍保留。`ResponseIncomplete` 不再一概转 error，而要按 Length/ContentFilter 成功
收束并保存 partial output/usage；新增 `Unknown` frame 要进入 raw audit，不能进入 assistant aggregate，也不能
被静默丢弃。

### UR-205-22：similar 3.2 是行为选择，不是三方合并重构

新 `TextMerge`、`WhitespaceMode` 可以支持未来 diff3/忽略空白需求，但仓库当前没有自定义三方文本 merge；唯一
consumer 只是 filesystem tool 的 `TextDiff::from_lines(...).unified_diff()`，所以没有删除目标。

3.2 的默认 Myers 采用 Git 风格 bounded split，输出可能与 3.1 不同；`RawMyers` 才保留旧 shortest-edit-script
行为。jaco-agent owner 已固定“diff text 不变”，因此把 `TextDiff::from_lines(...)` 改为
`TextDiff::configure().algorithm(Algorithm::RawMyers).diff_lines(...)`，并补复杂/重复行 unified-diff snapshot。
若产品以后希望改用更接近 Git 的输出，应另立行为变更，而不是在依赖升级里静默接受 churn。

### UR-205-23 至 UR-205-25：RMCP 与其余 registry 更新

- 独立 `mcp-auth-test-server` 升 RMCP 3.1.4，直接采用 renamed legacy-session builder 和默认 loopback Host
  防 DNS-rebinding guard；其自定义 OAuth authorization server 仍是产品测试 fixture，RMCP 的
  client/resource-server auth 能力不能替代它。
- 主 workspace 仍精确 pin RMCP 2.2，与 Rig 0.42 保持单一 Rust type universe；不引入 JSON 桥接。
- `http 1.5` 新增 `Method::QUERY`，只代表 HTTP Client 将来可扩展方法表，不替代现有 method/body/request UI。
- `http-body-util 0.1.5` 新增 `inspect_frame/inspect_err/into_stream/fuse`，但本仓没有自写 Body poll wrapper；
  response decoder 的多层编码、大小上限、字符集和错误分类仍是产品逻辑。
- Diesel 2.3.12 的生产修复针对 MySQL `NaiveTime` 微秒，而 Jaco 使用 SQLite；`libsqlite3-sys` 只是 bundled
  SQLite 更新，没有 repository/migration wrapper 可删。
- xcap 0.9.8 的生产差异位于 Linux Wayland recorder connection，本仓 capture 只在 Windows/macOS cfg 使用，
  不产生替代目标。
- 其余 async、HTTP、compression、search、database、macro、time 与 platform patch/minor 更新，本轮 source/API
  scan 没有映射到现有自定义实现。它们仍须执行 owner compatibility 与行为测试，但不为了“用上新功能”新增
  产品范围。

## Owner 工作包

### WP-UR-205-10：低风险直接复用

1. 根依赖切到 Zed `gpui_tokio`，Jaco/HTTP Client 通过 cancellation/network/time tests 后删除本地 member。
2. Jaco 删除本地 AppMenuBar fork，三个窗口切换到上游实体，保留 leading wrapper。
3. Jaco/Feiwen 将 component TitleBar 窗口改用 `TitleBar::window_options()` base。
4. 运行菜单键盘/鼠标/子菜单/focus return 与三平台 titlebar 回归；完成后做 residual scan。

### WP-UR-205-20：有领域 owner 的 UI 适配

1. 按专项审计迁移 Jaco conversation search 到 `Command`。
2. Picker 只保存 projection/query/selection/read-only/controlled-open contract inventory；未另立 follow-up 前保持
   `Defer`，本依赖批次不运行 picker 迁移或删除验证。
3. picker 任意内容 popover、temporary navigation、settings page filter 保持原 owner，不做批量替换。

### WP-UR-205-30：主题、跟尾与通知产品边界

1. 把 Jaco Light/Dark/System 主题投影到 native window appearance，并验证系统模式的观察链。
2. timeline 跟尾与系统通知保持 `Defer`；本批只验证当前 `set_follow_mode(Normal)` 与 Root-window notification
   行为不回归，并把产品决定留给后续计划。
3. 只有后续先固定“手动回到底部是否自动恢复 Tail”，才评估 `pause_following_tail()`。
4. 只有后续先定义有/无窗口的 notification delivery owner 与点击语义，才启用系统通知并验证无主窗口、已有
   主窗口、点击、关闭、重复 ID、无 daemon/未授权场景。

### WP-UR-205-40：Rig deletion-first migration

1. 先用 Rig 0.42 raw/identity API 替换私有 metadata/probe，再处理 Vec、hook 和 tagged content breaking changes。
2. live adapter 保留 event decoder，以公开 `RawStreamingChoice`、`normalize_stream`、
   `StreamingCompletionResponse` 归一化；blocking drain 复用同一链路，不能调用会丢 `raw` 的 0.42 unary helper。
3. 公开链路通过 raw/tool correlation parity 后，才删除 internal-id minting、旧 finish shuttle 与重复 commit path。
4. 用 unary/streaming/tool/reasoning/incomplete/error/continuation fallback fixtures 证明删除前后 parity。

### WP-UR-205-50：保留项认证

对 ComposerEditor、HotkeyInput、window/platform extensions、nested scroll、Feiwen reorder、OAuth
fixture 和 similar diff policy逐项留下 test 或 residual evidence，避免未来再次仅凭名称做同一轮审计。

## 验证与 residual gates

```text
rg -n "TitleBarAppMenuBar|CancelTitleBarMenu|popup_menu_from_owned_items" app/jaco/src
rg -n "ConversationSearchDelegate|ListState<ConversationSearchDelegate" app/jaco/src
rg -n "__jaco_response_id|tool_call_internal_ids|final_raw_response|on_stream_response_finish" crates/jaco-agent/src
rg -n 'path = "\./crates/gpui-tokio"|"crates/gpui-tokio"' Cargo.toml

cargo test -p jaco features::home::sidebar::search::tests --locked
cargo test -p jaco titlebar --locked
cargo test -p jaco-agent --all-features --locked
cargo test -p feiwen advanced --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Residual 不是盲目零命中：`title_bar_leading`、domain Picker adapter、Jaco live stream wrapper 等明确保留项仍可存在，
但旧通用状态机、私有 response identity side channel 和已迁 consumer 不得残留。

手工矩阵至少覆盖：菜单的 mouse/keyboard/submenu/focus；Windows/macOS/Linux titlebar；Narrator/VoiceOver/Orca
的 Command 和 icon-only controls；Rig continuation rejection、stream incomplete、tool-bearing turn 与 provider
error。Picker、timeline 新跟尾语义和系统通知是 `Defer`，本批只验证现有行为不回归。

## 完成条件

- 所有 `Reuse directly` 项的重复实现已删除，调用点不保留 compatibility wrapper。
- 所有 `Adapt` 项只保留表中明确的领域 authority，并有 parity/behavior tests。
- `Retain/Defer` 项有具体缺口和解除条件，不以“以后可能有用”作为理由。
- owner plans、root dependency plan、repo-local GPUI/component skill 与最终实现一致。
- 目标 SHA 或 registry target 变化时先刷新本审计，再继续实施。
