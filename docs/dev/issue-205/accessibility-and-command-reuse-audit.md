# Issue #205 依赖升级：a11y 与 Command 复用审计草稿

## 文档状态

- 状态：`Draft`
- 关联 issue：[#205](https://github.com/suxiaoshao/gpui/issues/205)
- Root hub：[Issue #205](README.md)
- Canonical dependency plan：[全 workspace 依赖升级计划](dependency-upgrade-plan.md)
- General reuse audit：[上游能力复用审计](upstream-reuse-audit.md)
- Jaco owner plan：[Jaco 依赖升级计划](../../../app/jaco/docs/dev/issue-205/dependency-upgrade-plan.md)
- 最近证据刷新：`2026-08-20`

本文只记录依赖升级带来的程序化 accessibility 基线变化，以及新 `Command` 对现有搜索 UI 的复用审计。
它不属于 Lestty 的终端内核、配置与主题选型草稿；也不把“AccessKit 已连接”扩张为“产品无障碍已完成”。

## 结论摘要

1. 当前锁定的 GPUI `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` 已默认接入 AccessKit。
   各应用使用 `gpui_platform::application()`，没有调用 `Application::new_inaccessible`，所以无需等待本次升级
   才“打开” a11y。
2. `gpui-component` 的 a11y 是逐组件实现，不是全局初始化后自动推导。目标
   `5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3` 扩大了 role、value 和 accessible action 覆盖，但应用自绘
   控件、icon-only 控件以及 Lestty 的终端 surface 仍需显式建模和验证。
3. 目标版本新增的 `gpui_component::command::Command` 适合**适配式替代** Jaco 会话搜索弹窗的通用交互层；
   它不能替代 Jaco 的数据库搜索、取消、错误、重试、陈旧响应门和结果 identity。
4. Jaco 必须使用 `Command::filterable(false)`，让数据库查询成为唯一过滤 authority。Command 不能注入自定义
   matcher；`keywords` 只能扩展内置 substring 规则，无法正确表达项目和消息正文命中。
5. `Command` 只存在于完整 `gpui-component`，不在 `gpui-base`。Jaco 升级后可用；Lestty 不应为了它破坏
   `gpui-base`-only 的已固定依赖边界。

## a11y 分层事实

| 层 | 当前事实 | 升级后的变化 | 仍由应用负责 |
| --- | --- | --- | --- |
| GPUI / platform | `gpui_platform::application()` 构造正常 `Application`；native window 按系统请求建立 AccessKit adapter 和 tree update。 | target 继续使用同一模型，并提供 role、aria、accessible action、synthetic children 与 a11y tree debug API。 | 不使用 `new_inaccessible`；稳定 element ID；实机验证各平台 adapter。 |
| `gpui-component` | 当前版本已给 Button、List、Table 等部分控件写入语义，但覆盖不完整。 | 更多行为和语义下沉到 `gpui-base`；完整 `gpui_component::init` 内部调用 `gpui_base::init`。 | 正确选择带语义的组件，并为 label、状态和自定义 content 补齐契约。 |
| workspace apps | 本轮 `rg` 未发现 app/crates 对 `role`、`aria_*`、`on_a11y_action` 或 `accessibility_label` 的显式调用；没有 a11y 验收测试。 | 现有完整组件 consumer 会继承上游新增覆盖。 | 自绘交互、icon-only 按钮、复合控件、焦点顺序、状态播报和 screen-reader smoke。 |
| Lestty | 当前只有 crate 脚手架，没有 UI 或终端语义树。 | `gpui-base::init` 不会替自绘 terminal `Element` 生成语义；base 层裸 Input 也不等于完整 UI Input 的 a11y wrapper。 | 候选 `Role::Terminal`、可见文本的 synthetic `TextRun`、光标/selection、滚动与可访问操作的独立 spike。 |

上游证据：

- GPUI 的 [accessibility guide](https://github.com/zed-industries/zed/blob/e0931d5a9dbf4f781b336fdf448739e74a2ac0b5/crates/gpui/src/_accessibility.rs)
  明确说明它通过 AccessKit 暴露 UI tree，并要求节点同时具有稳定 ID 和 role。
- 当前 pin 的 [`Application::new_inaccessible`](https://github.com/zed-industries/zed/blob/1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba/crates/gpui/src/app.rs#L183-L195)
  是显式关闭路径；仓库应用使用的
  [`gpui_platform::application`](https://github.com/zed-industries/zed/blob/1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba/crates/gpui_platform/src/gpui_platform.rs#L13-L15)
  走正常可访问 Application。
- target 的 [`gpui_component::init`](https://github.com/longbridge/gpui-component/blob/5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3/crates/ui/src/lib.rs#L120-L141)
  初始化组件全局状态和 keybindings，并调用 `gpui_base::init`；它不是一个能替所有应用节点补语义的 a11y
  注册器。

一个已验证的应用反例是 Jaco 的 icon-only copy Button：它有 icon 和 tooltip，但没有 label。Button role 可以来自
组件，tooltip 却不自动成为 accessible name。因此升级后仍需按调用点审计，而不是把组件依赖视为 blanket guarantee。

macOS 还有一个窄风险：完整 `gpui_component::Root` 会额外把 `NSWindow.accessibilityHitTest` 转发给
content view，而 `gpui-base` 的初始化没有 Window 参数。GPUI/AccessKit 自身已经提供核心 tree adapter，所以当前
不要求 Lestty 调用上游 `doc(hidden)` helper；实现时用 VoiceOver 键盘导航和 Accessibility Inspector 点选分别
验收，只有 point hit-testing 失败时才向上游确认公开契约。

## Command 能力与限制

目标 `Command` 不是纯视觉容器。它已拥有：

- query、focus、selection、滚动和 loading interaction state；
- Up/Down 循环选择、跳过 disabled、Enter confirm、Escape cancel；
- item/group/separator、Action keybinding hint、custom row、header/footer/empty；
- `on_query`、`on_select`、`on_confirm` 在 state lease 释放后回调；
- `v_virtual_list` viewport paint 和动态行高测量；
- 适合外部异步搜索的 `filterable(false)`。

它不拥有 Task、debounce、取消、错误、重试、结果 identity 或数据库 authority。每次 entries 变化时仍会测量所有
扁平 row；Jaco 的 50 条上限使这个成本可接受，但不能据此把 Command 当作海量结果 engine。

### 过滤模型

Command 当前没有 `filter_with`、matcher delegate 或 scorer callback：

| 配置 | 行为 | 适用范围 |
| --- | --- | --- |
| 默认 | 对 `label + keywords` 做 case-insensitive substring | 静态、本地、简单命令列表 |
| `filterable(false)` | 保留搜索框和 `on_query`，不对 supplied entries 做二次过滤 | 数据库、网络、fuzzy engine 或其他外部 authority |
| `searchable(false)` | 隐藏搜索框并关闭 query callback | 纯 quick actions |

Jaco 数据库搜索会命中会话标题、项目以及消息正文；仓库测试也覆盖了“标题不命中、正文命中”的会话。若保留
Command 默认过滤，这些正确结果会因为 row label 不含 query 而再次消失。把正文全文塞进 `keywords` 会复制搜索
authority、扩大内存与隐私暴露面，也仍不能表达未来的排序规则，因此不是可接受方案。

### Command 自身的 a11y 现状

target 已给 list container 写入 `Role::ListBox`，给 row 写入稳定 ID、`Role::ListBoxOption` 和
`aria_selected`；query Input 也有比当前 pin 更完整的 role/value/SetValue 支持。

但它尚不能被视为 a11y 完成：

- searchable 模式下真实焦点留在 query Input，selected row 没有使用 GPUI 的 `aria_active_descendant`；
- option 自身没有显式 accessible name，默认 `SharedString` child 也不能从源码证明会生成可命名的 a11y node；
- disabled、loading、result count 和 empty 没有完整的状态/live-region 语义；
- command 模块没有针对 accessibility tree 或 screen reader 的测试。

因此 Command 的交互复用与 a11y 验收要分开判断：它可以替代手写交互层，但不能作为“升级后自动完成 a11y”的
证据。若 target 上实测不能播报当前高亮项，应优先向上游补 option name、active descendant 和对应测试，避免在
Jaco 内复制一套 Command。

## 上游复用决定表

| D-ID | 本地实现 | 上游能力与差异 | 决定 | 删除/保留边界 | 验证 |
| --- | --- | --- | --- | --- | --- |
| `D-205-R01` | 每个 app 的 platform startup | GPUI 已默认创建 AccessKit adapter。 | `Reuse directly` | 不新增 app-local adapter 或第二套 a11y runtime。 | 三平台 adapter/screen-reader smoke。 |
| `D-205-R02` | app 自绘和 component 调用点 | target 扩大逐组件语义，但无法自动命名 icon-only/custom elements。 | `Adapt` | 复用组件 role/actions；应用只补产品语义、label 和自绘节点。 | tree inspection + keyboard + screen reader。 |
| `D-205-R03` | [Jaco 会话搜索弹窗](../../../app/jaco/src/features/home/sidebar/search.rs)的 Input、List、delegate、row、键盘和选择 helpers | Command 覆盖通用交互，支持 external async results；Escape 和选择保留策略有行为差异。 | `Adapt` | 删除通用交互层，保留薄 owner 和双行 custom row。 | query/selection/confirm/Escape/scroll/a11y。 |
| `D-205-R04` | Jaco `refresh::Operation`、数据库 search、stale gate、retry 和 ConversationId 映射 | Command 不拥有业务异步与 identity。 | `Retain` | Jaco 仍是唯一业务 owner；Command 只消费当前 results。 | 取消、乱序响应、错误重试、正文命中。 |
| `D-205-R05` | Lestty 将来的 app-local search/command UI | Command 只在完整 UI crate，违反已固定 base-only graph。 | `Retain/Defer` | 不给 Lestty 加 `gpui-component`，也不复制 Command；未来审计上游是否下沉。 | `cargo tree -p lestty` negative assertion。 |

仓库中其他名为 search 的 UI 不在本决定中：Jaco temporary navigation、controlled picker、Settings 页面过滤和
Feiwen 查询页都有不同的 state/value/route authority，不应因为组件名相似而一并迁移。

## Jaco 适配后的 owner 形状

建议保留：

- `Entity<CommandState>`；
- `Vec<Rc<SidebarSearchResult>>` 和 `query`；
- `refresh::Operation`、workspace observation、i18n、error/retry；
- `IndexPath.row -> results[row].conversation.id` 的显式映射；
- Dialog 的打开、关闭和结果确认策略。

建议删除：

- `ConversationSearchDelegate` 与 `Entity<ListState<_>>`；
- app-local query `InputState` 及其订阅；
- MoveUp、MoveDown、Enter handlers；
- `select_first_if_any`、`move_selected`、`confirm_selected`、`item_count`；
- 只为证明 ListState callback defer 而存在的本地 re-lock 测试，由 Command 的上游 defer 契约替代。

渲染时重建 `Command` entries，使用 `.filterable(false)`、`.on_query(...)` 和 `.on_confirm(...)`。异步开始/结束时
同步 `CommandState::set_loading`；error/retry 保留在 Jaco-owned header 或外围状态区。初次空查询仍由 owner 主动
加载，因为 Command 不会在创建时自行调用 `on_query`。

实施前还要固定两个行为：

1. 接受 Command 的两段 Escape：query 非空时第一次清空，第二次关闭 Dialog；当前面板会把 Escape 传播给
   Dialog。
2. 动态 entries 默认按数值 `IndexPath` 保留选择，而不是按 ConversationId。外部查询变化应重选第一条；仅
   workspace refresh 时是否按 identity remap，需通过产品行为测试确定。

## 验证门

自动化或结构检查：

```text
cargo test -p jaco features::home::sidebar::search::tests --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
cargo tree -p lestty --locked
rg -n "aria_active_descendant|aria_label|Role::ListBox" <target command sources>
```

必须补充的行为覆盖：

- 输入变化取消旧请求，乱序完成不能覆盖新 query；
- title、project 和 message-body 三种命中都保持可见；
- 上下键 wrap、Enter、鼠标 click、50 条滚动、empty、loading、error/retry；
- 非空/空 query 的两段 Escape；
- entries 更新后的 selection policy 与 ConversationId 映射。

手工 a11y 门：Windows Narrator、macOS VoiceOver、Linux Orca/AT-SPI 至少确认搜索框名称和值、结果总览、方向键
高亮播报、disabled/empty/loading/error 状态、confirm 与 focus return。Lestty 后续还要单独验证
`Role::Terminal`、可见文本、selection/caret、滚动和 point hit-testing；这些不由本次依赖升级自动完成。
