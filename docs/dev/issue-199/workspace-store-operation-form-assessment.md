# Issue #199：workspace Store、Operation 与 Form 适用性调研

## 1. 文档状态与范围

- 文档类型：全仓只读调研快照，不是实施计划，也不分配 work package。
- 调研基线：`codex/199-adopt-gpui-store-form-operation` 分支，commit `24d4249`。
- 调研范围：`app/jaco`、`app/feiwen`、`app/http-client`、
  `app/novel-download` 当前源码与 manifest。
- 本文只判断现有状态应该继续保留、改用预定义 Operation、只实现自定义
  `Transition<Message>`、迁入 `gpui-store`，还是暂缓；不修改任何应用代码或 crate API。
- Jaco 在 Issue #177 中已经完成的大部分 Store / Operation 迁移属于当前基线，本文不会把它们
  重复列为新任务。
- 审阅状态：除 Jaco MCP runtime 相关候选外，其余调研结论已由用户审阅。
- 用户决定：Jaco MCP runtime 的自定义 Transition 与 Store 发布快照均暂缓；在用户再次明确要求前，
  不为这两项建立实施文档，也不开始代码改造。该决定不否定下文的技术判断，只调整执行顺序。
- “已审阅”只表示本文可作为后续拆分 owner 文档的调研依据，不代表已经授权实施任何迁移。

本轮的核心结论是：

1. 当前最明确的新预定义 `refresh::Operation` 候选是 Feiwen 的查询执行和查询选项目录。
2. 当前没有新的业务可以直接、完整地套用 `repair::Operation`；Jaco Config 已经是正确示例，
   其余 repair 候选都依赖未来产品提供明确的用户修复动作。
3. Feiwen 多页抓取、novel-download 下载流程和 Jaco MCP runtime 的状态拓扑超出了两个预定义
   family，更适合应用自己定义消息并实现 `Transition<Message>`；其中 Jaco MCP runtime 已按用户
   决定暂缓，只保留调研结论。
4. `gpui-store` 最明确的新迁移目标是 Feiwen 的共享抓取运行状态；Feiwen 查询选项目录也适合在
   抽离后进入 Store。Jaco 只适合把 MCP 状态、快捷键诊断等纯快照从服务 owner 中拆出，不能把
   manager、平台 backend 或 `Task` 整体迁入 Store；其中 MCP 状态快照迁移已暂缓。
5. 除 Jaco 外，Feiwen、HTTP Client、Novel Download 当前都没有接入 `gpui-form`。Feiwen 抓取
   配置和 HTTP 请求草稿是明确候选；Novel Download 当前只有一个即时 URL 输入，不应为了形式
   统一而强行包装成 Form。

## 2. 判断标准

### 2.1 何时使用预定义 Operation

`gpui-operation` 当前提供两个完整 runtime enum：

| 类型 | 适用条件 | 不适用条件 |
| --- | --- | --- |
| `refresh::Operation<Data, Problem, Task>` | 同一份读取可反复执行；首次失败没有 Data；刷新失败时旧 Data 仍然有效；恢复方式只是再次读取 | 有不可重复副作用、流式部分提交、多条并发子任务，或失败后旧 Data 已失效 |
| `repair::Operation<Data, Problem, Repair, Task>` | Problem 需要调用方从明确的修复动作中选择一个，再执行修复 | 普通 Retry、局部输入校验、一次性通知，或尚未定义修复语义 |

两个 family 的契约见 `crates/gpui-operation/README.md:5-17`、`45-119`。Operation 只保存并转换
状态；Task 的构造、运行、completion route、通知、持久化和 repair 选择仍归应用 owner
（`crates/gpui-operation/README.md:165-179`）。

### 2.2 何时只实现 `Transition<Message>`

当业务确实有复杂状态转换，但状态图不是“读取 / 刷新”或“问题 / 显式修复”时，应用应在自己的
数据类型或 runtime enum 上实现 `Transition<Message>`。典型信号包括：

- 多阶段流程包含逐步进度、部分提交、停止、续跑或迟到 completion；
- 一个 owner 同时协调多个 keyed operation；
- 合法转换依赖业务 identity、generation 或 commit point；
- 需要把“接收消息并决定状态”与“执行 I/O / 平台副作用”分开。

不能因为代码已经有 `EventEmitter` 或消息 enum，就机械地改成 `Transition`。只有直接赋值、控件
编辑或一次性命令时，普通方法和 `Entity::update` 更清楚。

### 2.3 何时使用 Store

`gpui-store` 适合一份需要被多个独立消费者读取、选择和观察的共享内存事实。它不负责文件、
数据库、网络、平台资源或持久化（`crates/gpui-store/README.md:5-12`、`89-95`）。

本轮继续采用已经确认的边界：

- 不给 Store 新增公共 dispatch / message API；
- 复杂状态机由应用数据类型实现 `Transition<Message>`，调用方在 `Store::update` / `update_if`
  中投递消息，或构造完整新状态后调用 `Store::set`；
- Form draft、焦点、popup、控件 Entity、窗口 handle、service、repository、平台 backend 和
  `Task` 不因“共享”一词自动进入 Store；
- 如果一个现有 Global / Entity 同时混合服务与可观察数据，只有拆出的纯数据 snapshot 才是
  Store 候选，原服务 owner 继续持有副作用资源。

现有 `set`、`update`、`update_if` 已足以发布应用自己完成的状态转换，见
`crates/gpui-store/src/store.rs:111-152`。

### 2.4 何时使用 Form

`gpui-form` 适合有 typed draft、字段或跨字段校验、dirty / revision、rebase 和明确提交边界的
编辑会话。一次性搜索词、只有一个即时执行输入的命令栏，以及纯控件显示状态不必强行迁移。
Form runtime 继续由页面或 controller 持有，不存入 Store；异步业务执行继续由页面或 Operation
owner 持有。

## 3. 当前接入基线

manifest 与源码扫描结果如下：

| App | `gpui-operation` | `gpui-store` | `gpui-form` | 当前判断 |
| --- | --- | --- | --- | --- |
| Jaco | 已接入 | 已接入 | 已接入 | Issue #177 与 #199 已形成当前基线；只评估剩余边界 |
| Feiwen | 未接入 | 未接入 | **未接入** | 三类 crate 都有实际候选 |
| HTTP Client | 未接入 | 未接入 | **未接入** | Form 有明确候选；Operation / Store 等待真实请求运行时 |
| Novel Download | 未接入 | 未接入 | **未接入** | 下载状态适合自定义 Transition；当前没有直接 Store / Form 迁移必要 |

依赖证据：Jaco 在 `app/jaco/Cargo.toml:60-63` 声明三个 crate；其余三个 app 的
`Cargo.toml` 均未声明这些依赖，当前源码也没有相关 API 使用。

## 4. Jaco

### 4.1 已有正确基线

Jaco 已经覆盖大部分典型组合，后续不应重复迁移或降级为另一套状态模型：

| 现有表面 | 当前设计 | 判断 |
| --- | --- | --- |
| Config | `repair::Operation` + `Store`，支持 Reload / Reset 等显式 repair | 保留；这是预定义 repair family 的正确应用（`app/jaco/src/state/config.rs:46-67`、`656-707`） |
| Provider / Project / Prompt / Shortcut catalogs | `refresh::Operation` + `Store`；Ready Data 通过领域消息更新 | 保留；例如 Provider 的类型与消息在 `app/jaco/src/state/providers.rs:16-18`、`174-196` |
| Conversation catalog、conversation model、temporary search、sidebar search、skill catalog、runtime recovery | 预定义 `refresh::Operation` | 保留；这些都是可重复读取或 recovery fetch |
| Database | 应用自定义 `DatabaseOperation` + `Transition<DatabaseMessage>` + `Store` | 保留；refresh 失败后必须先 `Retiring` session 再进入 `Unavailable`，预定义 repair family 无法表达该中间态（`app/jaco/src/database/operation.rs:8-61`、`135-223`） |
| App shutdown | `Store<AppShutdownPhase>` | 保留；只有 `Running -> Draining` 的单向事实，不需要再引入 Operation（`app/jaco/src/app.rs:35-43`） |

### 4.2 剩余 Transition 候选

| 候选 | 分类 | 判断 |
| --- | --- | --- |
| MCP runtime | **暂缓（用户决定）** | 技术判断仍是自定义 `Transition`，且实施前必须先拆分状态与服务：当前一个 Entity 同时持有 session manager、四类 keyed Task、server statuses、OAuth target 和全局错误；连接测试、授权、凭据写入、断连和 runtime event 分散修改多张 map（`app/jaco/src/state/mcp.rs:63-73`、`154-206`、`209-289`、`359-390`、`393-652`）。多服务器并发与 OAuth 子流程都不符合单个 refresh / repair enum。当前只保留该调研结论，不建立实施文档或修改代码。 |
| Conversation active run | **后续评估自定义 `Transition`** | 每个 conversation 有 `Idle / Running / Stopping`、run key、取消 token、approval broker、运行 Task 和迟到 completion 防护（`app/jaco/src/features/conversation/runtime.rs:24-66`、`138-196`、`199-285`、`490-555`）。自定义消息可以集中合法转换，但必须保留 runtime owner 对任务、会话和取消资源的控制。这里不是 Data fetch，也不是 repair；现有 recovery `refresh::Operation` 继续独立保留。 |
| Settings 页 save / delete / fetch task | **保持现状** | Prompt、Shortcut、Provider、MCP editor 的任务只是页面或 dialog 局部互斥，并与 form revision、通知、关闭和 catalog command 绑定；底层 catalog 已有 Operation。当前 `Option<Task<()>>` 比再包一层 runtime enum 更直接，例如 `app/jaco/src/features/settings/provider.rs:245-300`、`930-1040`。 |
| Layout、Theme、temporary window、screenshot overlay | **保持现状** | 它们分别是带 debounce persistence 的 UI cache、订阅驱动的主题副作用、窗口 handle 生命周期和局部拖拽 / capture 交互，不是可重复业务 Operation。 |

### 4.3 Store 候选

| 候选 | 分类 | Store 中只保存什么 | Store 外保留什么 |
| --- | --- | --- | --- |
| MCP runtime 发布快照 | **暂缓（用户决定）** | 若未来恢复该项，只保存 server status、tool / auth snapshot、pending phase、last error，以及 UI 所需的派生 row 输入 | `McpSessionManager`、event listener、OAuth / connect / disconnect Task、网络与 keychain effect；当前不实施拆分或 Store 迁移 |
| Hotkey runtime diagnostics | **适合拆出** | `ShortcutRuntimeDiagnostics` 中的 temporary hotkey、registered shortcuts、registration errors、last pressed；该 snapshot 已经独立存在并被 General / Shortcut settings 多处读取（`app/jaco/src/state/hotkey.rs:179-192`、`1155-1168`） | `GlobalHotKeyManager` backend、系统注册 / 注销、副作用 Task 和事件监听（`app/jaco/src/state/hotkey.rs:144-155`、`356-394`） |
| Conversation runtime | **不整体迁移** | 无立即迁移项；未来只有确实出现多个独立消费者的纯 run summary 才另行评估 | active run、Task、cancellation token、approval broker、OpenAI session pool 继续由 Entity service owner 持有 |
| Layout / Theme / temporary window / screenshot | **保持现状** | 无 | 当前 owner 已与窗口、平台资源或副作用生命周期一致；换 Store 不会增加新的权威数据边界 |

MCP runtime 发布快照已经暂缓；若未来恢复评估，其边界仍是“拆出发布快照”，不是把名为
`McpRuntimeStore` 的现有 Entity 机械换成 `gpui_store::Store<S>`。Hotkey runtime diagnostics
候选同样只拆纯数据快照。类型名中有 `Store` 不代表它符合 `gpui-store` 的数据职责。

### 4.4 Form 状态

Jaco 已完成 Issue #199 的 form API 迁移。当前 Provider、Prompt、MCP、Shortcut、ChatInput 和
RunSettings 都是现有契约的消费方，本次调研没有发现需要建立第二套 form runtime 或把 form 放入
Store 的理由。

## 5. Feiwen

### 5.1 Operation / Transition

| 候选 | 分类 | 判断 |
| --- | --- | --- |
| 查询执行 `SearchState` | **直接采用 `refresh::Operation`** | 第一次查询对应 `Load`，已有结果后重查对应 `Refresh`；首次失败进入 `Unavailable`，已有结果后的失败进入 `Degraded` 并保留旧表格数据。输入校验错误仍归查询编辑器（未来可由 Form 接管），不应变成 Operation Problem。当前状态与执行分散在 `app/feiwen/src/features/query.rs:28-49`、`234-321`。 |
| `QueryOptions` 标签 / 作者目录 | **抽离后采用 `refresh::Operation`** | 目录可由同一来源重复加载，失败后旧 options 仍可展示；适合与后述 `Store<QueryCatalog>` 组合。当前加载入口在 `app/feiwen/src/features/query/advanced/options.rs:191-227` 和 `app/feiwen/src/features/query.rs:108-120`、`177-188`。 |
| 多页抓取 | **采用自定义 `Transition`，不使用预定义 family** | 状态包含逐页开始、写库成功、累计总数、限制日志、失败页、停止 / 续跑和最终结果；抓取还有持续写数据库的副作用（`app/feiwen/src/features/fetch.rs:87-104`、`204-335`、`371-497`、`708-845`）。应由 `FetchMessage` 归约纯运行状态，Runner 根据 effect 执行抓取和持久化。旧数据不是 refresh 意义下的可保留 Data。 |
| 数据库初始化失败 | **暂缓 repair** | 当前只记录错误且不安装 DB Global（`app/feiwen/src/store.rs:40-54`）。只有产品以后提供“重新打开 / 重建数据库”等明确、可选择的恢复动作时，才建立 `repair::Operation`。 |
| router、结果排序、条件控件编辑 | **保持现状** | 都是同步页面局部交互，没有异步 phase、Task 或恢复语义。 |

### 5.2 Store

| 候选 | 分类 | 判断 |
| --- | --- | --- |
| 抓取运行状态 | **直接采用 workspace-local `Store<FetchRunState>`** | 当前 `FetchTaskState` 由 Fetch 页面写入，又被 Query 页面与 titlebar 读取，是明确的共享权威状态（`app/feiwen/src/app/workspace.rs:32-76`、`app/feiwen/src/app/titlebar.rs:138-162`）。迁移 Form 后，URL / start / end / cookie draft 从中移出，Store 只保留运行 phase、进度、日志与任务 identity。 |
| Query catalog | **抽离后采用 `Store<QueryCatalog>`** | 标签 / 作者 options 是多个查询条件控件消费的目录数据；I/O completion 后发布新 snapshot，catalog 更新不能自动覆盖用户正在编辑的选择。 |
| `Db` | **保持 Global service** | 它是连接池 / repository 服务，不是共享内存业务 snapshot（`app/feiwen/src/store.rs:21-44`）。 |
| Workspace router、QueryView、table sort、advanced control Entity | **保持局部 Entity** | 这些 owner 只服务对应页面或控件，放入 Store 会扩大可变状态范围。 |

### 5.3 Form

Feiwen **尚未接入 `gpui-form`**。候选分为两级：

- **直接候选：抓取配置 Form。** 将 URL、`start_page: u32`、`end_page: u32`、cookie 建成 typed
  draft；统一验证 URL、页码下界和 `start_page <= end_page`。当前字段和裸 Input 同步见
  `app/feiwen/src/features/fetch.rs:33-39`、`500-674`。抓取执行不进入 Form。
- **暂缓：高级查询 Form。** 当前是带订阅的递归异构 Entity 树，条件类型变化会重建 relation / value
  控件（`app/feiwen/src/features/query/advanced/state.rs:36-135`、`278-345`）。应先抽出纯
  `QuerySpec` 编辑模型，再评估动态 array / nested group；不能直接把现有控件树塞入 Form 或 Store。

## 6. HTTP Client

### 6.1 Operation / Transition

当前 Send 事件的订阅仍是 TODO；应用没有 HTTP Task、response、retry 或 error runtime
（`app/http-client/src/features/request.rs:17-20`、`97-99`）。因此当前没有可直接迁移的 Operation：

| 候选 | 分类 | 判断 |
| --- | --- | --- |
| 未来请求执行 / resend | **功能落地后采用 `refresh::Operation<ResponseData, RequestProblem, Task>`** | 初次 Send 是 Load；已有 response 后 resend 是 Refresh；失败时分别进入 Unavailable / Degraded。Operation owner 应是 request page 或未来 request tab。当前不能为不存在的运行时预建状态。 |
| `HttpFormEvent` / `HttpBodyEvent` | **保持普通消息 / 方法** | 现有分支只是 method、URL、header、body 类型和文本的直接编辑，没有 phase、非法 completion 或 Task（`app/http-client/src/features/request.rs:101-127`、`app/http-client/src/features/request/body.rs:52-60`、`156-184`）。 |
| URL 与 Params 投影 | **交给 Form 建模，不用 Transition** | URL 应是唯一权威字段，Params 是 parse / rewrite 投影；parse failure 是字段校验问题，不是 repair。 |

当前也没有明确 `repair::Operation` 候选。只有以后出现认证 challenge、用户选择证书、代理修复等
显式恢复流程时，再按具体问题建模。

### 6.2 Store

当前没有直接 Store 迁移目标：

- `HttpForm` 只在唯一 request page 及其子视图间共享；`HttpBodyForm` 也只属于 body editor，都是
  页面局部 Entity（`app/http-client/src/features/request.rs:34-65`、
  `app/http-client/src/features/request/body.rs:46-75`）。
- I18n 初始化后只读，普通 Global 已足够。
- 请求历史、收藏、environment、auth 配置、cookie jar，以及多 tab catalog 都尚未实现。未来这些
  数据若被多个 tab / pane / window 共同消费，再分别建立 Store，不能提前创建空的全局状态。

### 6.3 Form

HTTP Client **尚未接入 `gpui-form`**，但请求编辑器是明确迁移候选。建议先统一一个 typed
`RequestDraft`，包含 method、URL、headers 和 body：

- 当前 `HttpForm` 只持有 method、URL 与由 `InputState` 组成的 header 行，业务值没有统一进入
  model（`app/http-client/src/features/request.rs:34-42`、
  `app/http-client/src/features/request/headers.rs:13-18`）；
- body 又在独立 `HttpBodyForm` 中保存 type、text 与 x-form
  （`app/http-client/src/features/request/body.rs:46-60`）；
- URL 是唯一 source of truth，Params 只做解析与编辑投影；
- URL、header 和 body 结构校验进入 Form，真正发送请求、Task、response 和 retry 留在 controller /
  Operation owner；
- multipart FormData 当前还是空视图
  （`app/http-client/src/features/request/body/form_data.rs:3-17`），不能把未实现行为写成已确认字段。

## 7. Novel Download

### 7.1 Operation / Transition

| 候选 | 分类 | 判断 |
| --- | --- | --- |
| 下载页面状态 | **采用自定义 `Transition`，但先确认并发模型** | `WorkspaceEvent` 和 `FetchState` 已经形成消息与状态雏形，但归约分散在页面方法中（`app/novel-download/src/features/workspace.rs:20-50`、`192-245`）。应集中 `Start / Progress / Succeed / Fail / Cancel` 等领域消息；页面 owner 继续构造 Task、写文件和通知。实现前必须决定单活跃下载、可取消单任务或队列。 |
| 小说元数据读取 | **暂缓 `refresh::Operation`** | 如果以后先展示元数据预览，同一 URL 的解析 / 拉取可用 refresh，失败后可保留旧预览。当前结果只是下载内部中间值，没有独立 UI owner（`app/novel-download/src/crawler/implement/zgzl/novel.rs:68-89`）。 |
| 输出目录 / 部分文件恢复 | **暂缓 `repair::Operation`** | 只有以后提供“更换目录 / 覆盖 / 从已完成章节续传”等用户选择，并先定义 commit point、去重和续传语义后，才适合 repair。 |
| 整个下载主流程 | **不能使用预定义 refresh / repair** | 流程会逐步请求并 append 写文件，重试可能重复已写内容，也没有可作为 Ready / Degraded Data 的完整旧结果（`app/novel-download/src/crawler.rs:33-53`、`app/novel-download/src/crawler/implement/zgzl/novel.rs:103-163`）。 |
| 单页网络 retry | **保持内部控制流** | 它是下载任务内部的短暂重试，不应为每一页创建 UI Operation（`app/novel-download/src/crawler/implement.rs:37-55`）。 |

当前 `FetchState::Fetching` 没有被 `loading()` 当作 loading，但启动事件会先进入该状态
（`app/novel-download/src/features/workspace.rs:86-92`、`203-205`）。这进一步说明当前模型尚未明确
重入 / 并发语义；自定义 Transition 应在该产品决定之后落地。

### 7.2 Store

当前没有直接 Store 迁移目标。一个 `WorkspaceView` 持有一个 `Entity<Workspace>`，Runner 通过弱
实体回传事件，没有多个独立消费者（`app/novel-download/src/features/workspace.rs:176-190`）。

如果未来形成后台下载中心、队列、历史、跨窗口详情或可恢复任务，才建立
`Store<DownloadCenterState>`；Store 保存共享运行事实和队列 snapshot，crawler、文件 I/O 与 Task
仍留在 service / owner 中。I18n 继续保持只读 Global。

### 7.3 Form

Novel Download **尚未接入 `gpui-form`**。当前只有一个 URL / ID `InputState`，输入后立即执行，
没有多字段 draft、validation report、rebase 或独立 submit preparation，因此本轮判断为保持现状
（`app/novel-download/src/features/workspace.rs:176-190`、`282-292`）。

当下载请求增加来源、输出目录、起止章节 / 页、覆盖策略或续传策略后，再建立
`DownloadRequestForm`；届时 Form 只负责 typed request 与验证，下载状态机仍由自定义 Transition
owner 负责。

## 8. 汇总结论

### 8.1 建议采用顺序（只表示设计成熟度，不是实施计划）

| 成熟度 | 候选 |
| --- | --- |
| 边界已经清楚 | Feiwen 查询 `refresh::Operation`；Feiwen 抓取自定义 Transition；Feiwen `Store<FetchRunState>`；Feiwen 抓取 Form；HTTP `RequestDraft` Form |
| 先拆分 owner / data | Feiwen QueryCatalog 的 refresh + Store；Jaco HotkeyDiagnostics Store |
| 先做产品或领域决定 | Jaco conversation active run Transition；HTTP request runtime；Novel Download 并发 / 取消 / 续传状态机；Novel Download richer Form |
| 用户决定暂缓 | Jaco MCP runtime 的 custom Transition 与 status Store；只保留本文调研结论，等待用户再次明确要求 |
| 保持现状 | Jaco settings 局部任务与平台生命周期；HTTP 当前页面局部 Entity；Novel Download 当前单 Workspace；各 app 的 service / repository / I18n Global |

### 8.2 明确不改变的公共契约

- `gpui-operation` 不需要新增第三个预定义 family；非 refresh / repair 拓扑由应用自定义
  `Transition<Message>`。
- `gpui-store` 不需要新增消息接口。应用在自己的 `S` 上实现 Transition，并通过现有
  `update` / `update_if` / `set` 发布结果。
- Store 不拥有 persistence、repository、service、平台 handle 或 effect runtime。
- Form 不进入 Store，也不接管业务 Task；Form submit 只产生经过验证的 typed input。
- Jaco MCP runtime 的 custom Transition 与 status Store 不进入当前后续实施范围；恢复这两项必须由
  用户再次明确提出。
- 本文不授权开始任何迁移。后续若实施，应在同一个 Issue #199 下为对应 app 建立 owner 文档，
  再把候选转换成依赖、消息、状态、effect、Store snapshot 与验证清单。
