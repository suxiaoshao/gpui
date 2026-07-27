# Issue #177：Resource UI 组合与订阅边界重构计划

关联 issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)

状态：**设计草案，等待用户审阅**。

本文只规划 Jaco 在已经接入 `gpui-store` 与 `gpui-operation` 后的 UI 组合、订阅和生命周期
重构。本文不会修改 `gpui-store`、`gpui-operation` 的公共 API，也不会取代
[issue #177 总计划](README.md)。

本轮只记录已经确认的设计和实施顺序，不执行最终完整性检查。用户审阅通过后再实施或补充
尚未确认的细节。

## 1. 目标

1. Resource 的 `Operation` 仍由现有 Store 或局部 owner 持有，不增加第二套 UI 状态机。
2. 一个组件树内由已经拥有状态生命周期的最高层组件订阅一次；直接展示子组件不重复订阅。
3. 父组件可以把本次 render 所需的轻量展示数据直接交给子组件；禁止的是跨多层转发状态，
   不是正常的一层父子参数。
4. 无局部状态、无订阅、无 Task 的展示组件优先实现 `RenderOnce`。
5. `JacoRoot`、`SettingsView`、`TemporaryWindow` 和各设置页直接 match 自己拥有的 Resource；
   它们根据 Operation 变体构造 Loading、Problem、Repair 或正常内容。
6. 页面负责展示当前资源问题和修复方法；具体业务操作是否允许，由执行该操作的按钮或控件
   负责表达。
7. 所有 command 在执行时仍重新验证精确 Operation 变体。按钮禁用是交互反馈，command
   验证是正确性边界。
8. Config、Database、Provider、Project、Prompt、Shortcut、Conversation Index 和 Skill
   的所有可达状态都有明确 UI，不用默认值、空列表或静默关闭掩盖 Problem。

## 2. 非目标

- 不新增通用 Resource trait、通用依赖图、`AppAvailability` Store 或另一套 phase enum。
- 不让 `gpui-store` 感知 Operation、I/O、修复或按钮能力。
- 不让 `gpui-operation` 感知 GPUI Entity、Store、通知或页面。
- 不把所有按钮改成会自行订阅任意 Global Store 的“智能按钮”。
- 不为消除一层正常参数传递而引入父子消息、event bus 或重复 Selection。
- 不修改 About 的产品内容、Provider/MCP 协议、数据库 schema、配置格式或 Skill 来源规则。
- 不在本计划阶段执行应用 UI 自动化或最终验收；用户继续负责最终人工 UI 测试。

## 3. 已确认的职责边界

### 3.1 Resource owner

Resource owner 负责：

- 保存唯一的 `Operation<Data, Problem, ...>`；
- 保存运行中的 Task；
- 构造 load、refresh、retry 或 repair attempt；
- 接收 `Complete`、`Cancel` 和业务 message；
- 通过 Store 或 Entity 发布变化；
- 在 command 入口匹配精确 Operation 变体。

Resource owner 不负责具体窗口布局。

### 3.2 状态订阅 owner

一个组件树内，最靠上的现有状态 owner 负责订阅它实际需要的 Store/Selection：

- 它保存 `StoreSelection` 或 `Subscription`；
- Selection 变化时通知自身；
- render 时读取 Operation 或已选出的轻量状态；
- 直接构造对应展示组件。

直接子级是无状态展示组件时，不得再次订阅同一 Resource。

只有满足以下任一条件时，子 Entity 才建立自己的订阅：

1. 子 Entity 可以脱离父组件独立存在；
2. 子 Entity 有不同生命周期；
3. 父组件不消费该 Resource；
4. Resource 更新只应重绘该子 Entity，而父组件不应重绘；
5. 子 Entity 自己就是该局部 Operation 的 owner。

### 3.3 `RenderOnce` 展示组件

`RenderOnce` 适合：

- Loading 页面；
- Problem 与 Repair 页面；
- 只读/过时状态提示；
- 列表行、状态条和按钮组合；
- 父组件已经读取状态后构造的一次性 UI。

`RenderOnce: 'static`，因此不能保存从父组件借用的 `&Operation`、`&Data` 或 `&Problem`。
父组件应在同一次 render 中生成最小的 owned 展示输入，例如：

```rust
struct ResourceProblemPresentation {
    title: SharedString,
    message: SharedString,
    running: bool,
    actions: Vec<ResourceProblemAction>,
}
```

该类型只存在于本次 element tree 中，不写回 Store，不保存为长期状态，也不镜像完整
Operation。

若某处为了 `RenderOnce` 必须复制大型 Data，则不使用该 presentation；改为由父组件在 Store
借用范围内调用普通 `render_*` 函数，或者继续渲染已有 Entity。

### 3.4 普通父子参数

允许：

```text
订阅 Resource 的父组件
  -> 直接子 RenderOnce 的 owned 展示输入
```

禁止：

```text
Window -> Page -> Section -> Control -> Button
  逐层传 phase/problem/loading/disabled
```

如果 Section 已经是独立 Entity 并且真正拥有该 Resource 的消费生命周期，就由 Section
订阅；否则由上一级已订阅的 owner 直接构造展示树。

### 3.5 按钮与 command

父组件读取 Operation 后，可以直接为本次 render 构造按钮的 `disabled`、`loading`、tooltip
和可见性，不要求按钮再次订阅。

按钮点击后调用 Resource command。Resource command 必须重新匹配当前真实状态：

- Config/Database repair 只从对应 problem-bearing variant 启动；
- refresh/retry 只从合法 settled variant 启动；
- Provider、Project、Prompt、Shortcut 和 Conversation mutation 只允许精确 `Ready`；
- conversation send/run 重新验证 Config、Database、Provider Model 和 Runtime。

这不是重复 UI 逻辑，而是防止快捷键、延迟事件、旧 element tree 或其他调用路径绕过约束。

## 4. 共享展示组件

在 Jaco app 内新增 Resource UI 展示模块。目标文件布局在实施时以现有模块树为准，计划中的
候选位置为：

```text
app/jaco/src/components/resource.rs
app/jaco/src/components/resource/loading.rs
app/jaco/src/components/resource/problem.rs
app/jaco/src/components/resource/stale.rs
```

不新增 `mod.rs`。

计划提供以下 app-local 组件：

### 4.1 `CriticalResourcesView`

实现 `RenderOnce`，只接收一次性 presentation。它不保存 Store、不订阅、不持有 Task。

它负责组合：

- Config Loading；
- Config Problem 与可用 Repair；
- Database Loading；
- Database Problem 与可用 Repair；
- AppSession 初始化失败与 Retry；
- Refreshing/Repairing 进度；
- 有旧内容时的 Problem/Repair layer。

修复按钮可以直接调用 `state::config`、`database` 或 `app::session` command，不通过
`JacoRoot` 消息回传。

### 4.2 普通 Resource 状态组件

Provider、Project、Prompt、Shortcut、Conversation Index、Skill 等局部 Resource 共用视觉
原语，但不使用一个抹平类型差异的通用 runtime adapter：

- `ResourceLoadingView`
- `ResourceProblemView`
- `ResourceStaleNotice`
- `ResourceRefreshButton`

父页面根据自己的 Operation family 和 Problem 构造这些组件。具体 Refresh/Retry/Repair
command 仍由对应 Resource module 提供。

### 4.3 不持久化 presentation

所有 presentation 都由当前 Operation 当次计算：

- 不安装到 Global；
- 不放入 Store；
- 不保存 revision；
- 不用 observer 同步；
- 不参与 Resource command。

## 5. 主窗口

### 5.1 订阅 owner

`JacoRoot` 保存并订阅：

- Config 的关键展示 Selection；
- Database 的 binding/phase/problem 展示 Selection；
- AppSessionStore；
- AppShutdownStore；
- 主窗口专属 Theme binding。

这些订阅只负责让 `JacoRoot` 在相关状态变化时重新 render。`CriticalResourcesView` 不再重复
订阅。

### 5.2 render 组合

`JacoRoot::render` 直接 match Config、Database 和 AppSession 的当前状态，不保存新的
`AppStartupState` 或 `CriticalState`。

优先级：

```text
Config
  -> Database
     -> AppSession
        -> HomeView
```

映射：

| 当前事实 | 主窗口展示 |
| --- | --- |
| Config Idle/Loading，无 Data | `CriticalResourcesView` 的 Config Loading |
| Config Unavailable/RepairingUnavailable | Config Problem/Repair |
| Config Refreshing/Degraded/RepairingDegraded，有旧 Data | 保留匹配旧 Session 的 Home，并叠加 Config Problem/Repair layer |
| Config Ready，Database Awaiting/Loading | Database Loading |
| Database Unavailable/RepairingUnavailable | Database Problem/Repair |
| Database Refreshing/Degraded/RepairingDegraded，binding 未变 | 保留匹配 binding 的 Home，并叠加 Database Problem/Repair layer |
| Database binding 已失效或变化 | 不复用旧 Home；展示当前 Database Loading/Problem |
| Database Ready，AppSession Awaiting | Session Loading |
| AppSession Failed | Session Problem/Retry |
| 三者精确 Ready 且 binding 匹配 | `HomeView` |

`HomeView` 继续是 `Entity + Render`，因为它持有交互状态、子 Entity、焦点和运行时引用，不改为
`RenderOnce`。

### 5.3 删除旧职责

从 `JacoRoot` 删除：

- 重复的 Loading/Alert/Button UI 构造；
- 为子组件准备并逐层传递的 phase/problem/running 参数；
- 修复 command 的中转消息；
- 与 Resource 生命周期无关的 task/session 初始化。

保留：

- 主窗口组合；
- `HomeView` 与当前 binding 的对应关系；
- 主窗口焦点、菜单和窗口级订阅；
- 对共享 Store 的一次订阅与 render-time match。

## 6. 设置窗口

### 6.1 窗口级 Resource

`SettingsView` 直接订阅 Config，因为 Config 决定设置值、数据目录和持久化能力。

映射：

| Config 状态 | Settings 展示 |
| --- | --- |
| 无 Data 的 Loading/Unavailable/Repairing | `CriticalResourcesView`，替代正常设置内容 |
| 有旧 Data 的 Refreshing/Degraded/Repairing | 保留 Settings shell，并叠加 Config Problem/Repair layer |
| Ready | 正常 Settings shell |

`CriticalResourcesView` 接收 `SettingsView` 当次构造的 presentation，不再订阅 Config。

Database 不再作为整个 Settings 窗口的创建门禁。即使 Database 不可用，General、
Appearance、Skills 和不依赖数据库的 MCP 设置仍可打开。

### 6.2 子页面 Resource

| 子页面 | 状态 owner | 处理范围 |
| --- | --- | --- |
| General | `SettingsView`/General 当前 Config 消费 owner | Config 由窗口级状态处理；临时 hotkey 错误只影响对应控件 |
| Appearance | `AppearanceSettingsPage` | Theme 编辑；Config 持久化能力由父级状态决定 |
| Skills | `SkillsSettingsPage` | 页面局部 Skill Operation 的 Loading/Problem/Refresh |
| MCP | `McpSettingsPage` | Config 中的 MCP 定义和 MCP Runtime；不依赖 Database catalog |
| Providers | `ProviderSettingsPage` | Provider Operation、Secret side effect、远程 model fetch |
| Projects | `ProjectsSettingsPage` | Project Operation |
| Prompts | `PromptsSettingsPage` | Prompt Operation |
| Shortcuts | `ShortcutsSettingsPage` | Shortcut、Prompt、Provider Operation 与 Hotkey diagnostics |

每个页面已有 StoreSelection/Subscription 时，由页面读取一次并构造 RenderOnce 子组件；列表
行、错误视图和按钮不重复订阅。

Shortcuts 需要多个 Resource 时，页面分别保存已有具名 Selection。不得建立把多个 Operation
复制进另一 Store 的组合状态。

### 6.3 Database 与 Session 页面生命周期

数据库相关页面按当前 AppSession binding 延迟创建：

- Database/AppSession 未就绪时，选中该页只展示对应 Loading/Problem/Repair；
- binding 就绪时创建该 binding 对应的页面 Entity；
- binding 变化时丢弃旧页面 Entity，General/Appearance/Skills/MCP 保持；
- Database 有匹配旧 Data 时，页面继续展示旧内容和 Problem/Refresh UI；
- 页面内 mutation 按钮只在所需 Resource 精确 `Ready` 时启用。

这一步同时删除 `SettingsView::new_with_page` 中“Database 必须精确 Ready”以及一次性急切创建
全部数据库页面的前提。

## 7. About 窗口

About 只消费：

- 编译期 metadata；
- app icon；
- I18n；
- Theme；
- 窗口菜单与焦点。

它不订阅 Config、Database、AppSession 或 catalog，不显示 Critical Resource UI，也不因这些
Resource 失败而关闭。

Theme 和 I18n 在 Config 无 Data 时继续使用应用 bootstrap/default 值。About 只保留现有
WindowThemeBinding 和应用级 I18n/Theme 刷新路径。

## 8. 临时窗口

### 8.1 窗口生命周期边界

`TemporaryWindow` 绑定创建时的 Database binding 和 Conversation Runtime。

- binding 相同但 Database 正在 Refresh/Degraded：窗口保留；
- binding 消失或变化：旧 Runtime、Conversation page 和 command route 已失效，关闭临时窗口；
- 全局故障需要用户处理时，通过 `show_or_create_main_window` 保证修复 UI 有承载窗口。

关闭只用于生命周期身份失效，不用于普通 Project、Conversation、Provider 或 Skill 加载失败。

### 8.2 窗口已有状态 owner

`TemporaryWindow` 已持有：

- `search_operation`；
- Database binding subscription；
- Conversation Runtime；
- 当前 route；
- conversation page Entity；
- new conversation pane Entity。

因此：

- Temporary list 的 Loading/Problem/旧数据直接由 `TemporaryWindow` match
  `search_operation` 后交给 RenderOnce/List delegate；
- 列表行和错误提示不再订阅同一个 search Operation；
- Database 有匹配旧 Data 时，由 `TemporaryWindow` 构造一次性的 Database
  Problem/Repair presentation；
- 不把 database phase 逐层传给 ConversationDetail 或 ChatInput。

### 8.3 局部依赖

| 区域/操作 | 依赖不可用时 |
| --- | --- |
| 左侧临时对话列表 | Project/Conversation Index Loading 或错误在列表区域显示，并提供各自 Refresh |
| New Conversation | Provider/Model、Config chat settings、Skill 状态由 ChatInput owner 处理 |
| Send | Send 按钮根据当前 Config、Database、Provider Model 和 Runtime 状态显示禁用原因 |
| Conversation Detail | 具体 conversation load/run Problem 在详情区域显示 |
| Project/Conversation 有旧 Data | 继续浏览旧列表；对应 mutation/submit 按钮自行禁用 |

Project、Conversation Index 或 Provider 加载失败不会关闭临时窗口。

## 9. Selector 与订阅规则

1. 多处复用的 Resource 投影使用具名 `Select`。
2. 同一父组件已经持有 Selection 时，RenderOnce 子组件只接收 Selection 输出或当次 owned
   presentation。
3. 不为了少传一个一层参数，让按钮、行和提示分别建立相同 Selection。
4. 不为了复用 presentation，把它保存为第二个 Store。
5. 多 Resource consumer 保存多个独立 Selection；不在 `Select` 内读取其他 Store。
6. 只需要一次 render-time 读取的数据不建立 Subscription。
7. 页面局部 Operation 由页面 Entity 自己保存和通知，不提升为 Global。

## 10. 实施工作包

### UI-10：建立无状态 Resource 展示组件

**范围**

- 新增 Resource Loading、Problem、Stale 和 Critical 展示模块；
- `CriticalResourcesView` 实现 `RenderOnce`；
- 定义最小 owned presentation；
- 迁移当前 `JacoRoot` 内重复的 Config/Database/Session UI 构造，但暂不改变状态订阅。

**完成条件**

- RenderOnce 组件不保存 Store、Entity Subscription 或 Task；
- presentation 不镜像完整 Operation；
- repair/refresh 按钮直接调用 Resource command；
- 中英文文案继续使用现有 I18n key，新增 key 时同步两种 locale。

### UI-20：重构 `JacoRoot`

**依赖**：UI-10。

**范围**

- 保留 JacoRoot 对 Config、Database、AppSession 和 Shutdown 的一次订阅；
- render 直接 match 当前状态并构造 `CriticalResourcesView` 或 `HomeView`；
- 删除 JacoRoot 内重复 UI 与 command 中转；
- 保留 binding 与 Home 生命周期对应关系；
- 覆盖有旧 Data 的 Problem/Repair layer。

**完成条件**

- CriticalResourcesView 没有重复订阅；
- JacoRoot 没有第二套持久化 startup/critical 状态；
- 每个 Config/Database/AppSession 可达状态都有明确主窗口输出；
- binding 变化后不能继续使用旧 Home。

### UI-30：拆分 Settings 窗口与页面依赖

**依赖**：UI-10。

**范围**

- SettingsView 只统一处理 Config 的窗口级展示；
- 删除打开 Settings 必须 Database Ready 的门禁；
- General/Appearance/Skills/MCP 与 database-backed 页面分离生命周期；
- database-backed 页面按 AppSession binding 延迟创建和替换；
- 页面使用已有 Selection 一次构造 RenderOnce 子树；
- 每个业务按钮只表达自身所需 Resource 的可用性。

**完成条件**

- Database 不可用时仍能打开不依赖数据库的设置页；
- Config Problem 在 Settings 内直接显示修复，不要求主窗口存在；
- Provider/Project/Prompt/Shortcut Problem 只影响对应页面或控件；
- 不存在 Window 到 Button 的多层 phase/problem/disabled 传递。

### UI-40：确认 About 独立性

**依赖**：UI-10 无强依赖，可并行实施。

**范围**

- 删除任何误加的 Config/Database/AppSession Gate；
- 保留 I18n、Theme、菜单、focus 和 metadata；
- 确认 bootstrap/default Theme 与 I18n 足以渲染。

**完成条件**

- Config 或 Database 无 Data 时仍能创建和渲染 About；
- About 不持有无关 Store handle 或 subscription。

### UI-50：细化 TemporaryWindow Resource UI

**依赖**：UI-10、UI-20 的 Critical 展示契约。

**范围**

- binding 身份失效与普通 catalog Problem 分开处理；
- TemporaryWindow 继续作为 search_operation 的唯一 owner；
- Project/Conversation Index Problem 在列表区域显示 Refresh；
- Provider/Model 问题由 NewConversation/ChatInput owner 展示；
- Database 有匹配旧 Data 时保留窗口并展示 Problem/Repair；
- binding 变化时关闭旧窗口并确保主窗口修复入口存在。

**完成条件**

- 普通 Project/Conversation/Provider 失败不会关闭窗口；
- 列表、详情和发送操作只受自己真实依赖影响；
- 旧 binding 的 Runtime 和页面不会被继续使用；
- Temporary 子 RenderOnce 不重复订阅父级状态。

### UI-60：收紧业务按钮与 command 边界

**依赖**：UI-20、UI-30、UI-50。

**范围**

- 核对 send、select model、project/prompt/shortcut mutation、provider mutation、
  refresh/retry/repair 按钮；
- 按钮展示 disabled/loading/reason；
- command 重新验证精确 Operation 变体；
- 删除页面级一刀切 `database::is_ready()` 遮罩和不必要的统一禁用。

**完成条件**

- 每个按钮只依赖执行该动作真正需要的 Resource；
- UI 旧状态不能绕过 command guard；
- Degraded/Refreshing Data 可展示，但 mutation 不会落到非 Ready Operation；
- 不新增 capability mirror Store。

### UI-70：定向测试与旧路径清理

**依赖**：UI-20、UI-30、UI-40、UI-50、UI-60。

**范围**

- 删除被新 RenderOnce 组件替代的重复 Loading/Alert/Button；
- 删除 Settings/Temporary 的整体 `database::is_ready()` 页面遮罩；
- 删除不再需要的重复 selector closure 和 duplicate subscription；
- 增加状态到 presentation、binding 生命周期和 command guard 的定向测试；
- 保留用户人工 UI 测试作为最终可见行为确认。

**计划验证**

```text
cargo fmt --all
cargo check -p jaco --locked
cargo test -p jaco <resource presentation tests> --locked
cargo test -p jaco <settings dependency tests> --locked
cargo test -p jaco <temporary lifecycle tests> --locked
git diff --check
```

只有用户确认本文不再修改并要求最终检查时，才执行一次跨工作包最终验收；实施过程中不重复运行
已经被后续门禁覆盖的同类检查。

## 11. 工作包顺序

```text
UI-10
├── UI-20
├── UI-30
├── UI-40
└── UI-50 (同时消费 UI-20 的 Critical 展示契约)

UI-20 + UI-30 + UI-50
└── UI-60

UI-20 + UI-30 + UI-40 + UI-50 + UI-60
└── UI-70
```

各工作包只修改自己拥有的组件和状态映射。不得在 UI-70 重新设计 Resource owner、Operation
family、Store 生命周期或页面产品策略。

## 12. 审阅重点

用户审阅本文时重点确认：

1. `CriticalResourcesView` 是否只作为无状态 `RenderOnce` presentation；
2. Config 是否是唯一阻断整个 Settings 内容的 Resource；
3. Database 不可用时，General/Appearance/Skills/MCP 是否应继续可用；
4. database-backed Settings 页面是否按 AppSession binding 延迟创建；
5. TemporaryWindow 是否只在 binding 身份失效时关闭；
6. Database Degraded 且 binding 未变时，TemporaryWindow 是否直接显示修复入口；
7. UI-60 的按钮级能力边界是否符合“页面展示问题，按钮决定操作”的职责划分。
