# Coding Guides 对齐与功能 crate 拆分

状态：实现与本地验证完成，可试用。

依据：[Issue #255](https://github.com/suxiaoshao/gpui/issues/255)、[官方 Coding Guides](https://gpui-kit.com/docs/coding-guides/) 与 2026-10-03 检查的 `main@aade6a2e`。本计划落实本轮确认的功能拆分、文本与集合所有权、生命周期、布局，以及文档和 skills 同步。官方 skills 通过 npx skills 管理，不手工编辑其内容。下列目录和能力边界已迁移到源码，验证记录见文末。

## 目标与交付边界

- 应用入口保留在 `app/<name>/src/main.rs`，负责运行时初始化、窗口、原生菜单、导航及功能组合；业务能力进入应用专属 crate。
- 功能 crate 包含本能力的模型、服务、视图、命令、对话框与测试，通过窄公共接口协作；功能不依赖 shell，跨功能不读取私有实现。
- 持久 UI 文本、集合与异步状态有明确 owner，渲染避免不必要的深复制；保留可编辑文本、原始响应和用户数据的语义。
- 应用布局遵循 rem 尺度和主题语义，键盘、焦点、滚动与异步关闭行为随迁移一起验证。
- 每批更新对应使用文档和项目自有 skills；官方技能通过管理工具更新，保持上游文件原样。

保持现有应用功能、包名、启动命令、数据库 schema、数据目录和打包入口。迁移允许改变仓库内部 Rust 接口；现役共享库的外部公共契约不随文件移动无收益地改写。迁移时的依赖版本为 `gpui-kit 0.7.0` / `gpui-pre 0.3.7`，官方文档和 skill 不能代替锁定源码的 API 核对。

本计划包含三个现役应用及必要的共享能力调整。#248 的共享库深层契约工作仅在本轮实际依赖处复核和复用，不重复承接已迁出的 Gupi 或退役应用任务。性能收益只在实际测量后表述；减少复制或拆分 crate 本身不等同于性能验收。

## 目标目录和依赖

```text
app/
├── feiwen/
│   ├── Cargo.toml                  # 原有二进制包与 bundle 元数据
│   ├── src/                        # shell、启动、窗口及导航
│   ├── locales/                    # 应用拥有的 Fluent 与原生 bundle 文案
│   └── crates/
│       ├── feiwen-data/            # 数据模型、数据库、查询及目录服务
│       ├── feiwen-fetch/           # 抓取、解析、写入工作流和对应 UI
│       └── feiwen-query/           # 查询表单、结果、排序和对应 UI
├── http-client/
│   ├── Cargo.toml
│   ├── src/                        # shell、启动和窗口组合
│   ├── locales/
│   └── crates/
│       ├── http-client-core/       # 执行契约、传输、响应原始数据及捕获
│       ├── http-client-request/    # 编辑、表单校验、准备及发送工作流 UI
│       └── http-client-response/   # 预览、文本/PDF/媒体、保存和对应 UI
└── novel-download/
    ├── Cargo.toml
    ├── src/                        # shell、启动和窗口
    ├── locales/
    └── crates/
        └── novel-download-feature/ # 表单、下载、源解析、输出及对应 UI

crates/
├── app-i18n/                       # 三个应用共用的 Fluent 运行时
├── app-theme/
└── …                               # 现有跨应用框架和能力
```

新增 workspace member 明确列入根 manifest，不创建嵌套 workspace；内部 crate 使用 `publish = false`，遵循根 manifest 的依赖别名、完整版本与 `{module}.rs` 约定。应用专属 crate 不进入根 `crates/`，根 AGENTS 与架构文档同步说明新归属。

| 依赖者 | 允许依赖的业务能力 | 关键边界 |
| --- | --- | --- |
| Feiwen shell | data、fetch、query | 处理路由及跨功能协调 |
| feiwen-fetch | feiwen-data | 抓取状态由 fetch 持有，按意图发布事件 |
| feiwen-query | feiwen-data | 不依赖 shell 或 fetch 的内部运行状态 |
| HTTP shell | request、response、core | 组合功能，协调请求结果与响应视图 |
| http-client-request | http-client-core | 表单草稿与编辑行为归 request |
| http-client-response | http-client-core | 不依赖 RequestView 或请求表单 |
| Novel Download shell | novel-download-feature | 初始化并组合下载功能 |

功能可依赖有实际用途的现有 GPUI/Form/Operation/Store/Tokio 能力，以及共享 `app-i18n`。不创建用于转运所有类型的兜底 shared crate。

## 必要设计决定

### Feiwen：数据服务、抓取与查询

`store/` 中的小说/作者/标签模型、数据库连接与资源生命周期、查询和目录能力进入 `feiwen-data`。数据库的准备、错误、备份等状态属于数据服务；shell 组合对应资源页面和通知，避免数据服务反向调用 shell。纯后端解析能力保留清晰接口，数据库路径由应用启动配置传入，data 不读取 shell 的 `APP_NAME`。

合并现在 `fetch/` 与 `features/fetch/` 的同一抓取能力到 `feiwen-fetch`；抓取表单、运行状态、网络解析、进度和日志的所有权一并迁移。`feiwen-query` 聚合查询表单、条件控件、执行状态与结果表格。

Query 不再持有 `Entity<Workspace>` 或 `Store<FetchRun>`。导航/抓取请求通过语义事件交给 shell；如果现有行为确实需要抓取状态，提供只包含所需语义的公开状态投影，由 shell 传递，不暴露完整 FetchRun。与数据变化有关的查询刷新归数据服务事件，而非借用抓取 UI 状态。迁移保持现有禁用条件、刷新时机、取消与重试行为。

查询结果只保留一份权威集合，显示顺序由索引表达；排序比较借用文本，不在每次比较中分配或复制字符串。保留默认顺序恢复、稳定排序、缺失值位置和已有选择语义，行身份使用小说业务 ID，不使用当前排序位置。

### HTTP Client：执行核心与两个独立功能

将 PreparedRequest 等执行输入、传输错误/进度、ResponseData/StoredBody、响应捕获和传输所需解码能力移到 `http-client-core`。core 不依赖 GPUI View、Fluent 或 feature；属于运行时适配的 GPUI task/channel 协调留在请求功能。纯后端的 bytes、流和不可变 Arc 数据继续沿用，不为了 UI 文本规则引入框架依赖。

请求表单、draft、字段控件、业务校验和把草稿编译成执行输入的逻辑归 `http-client-request`。原始响应与编辑模型分离，RequestDraft 的可变文本保留 `String`。

响应功能从依赖 `Context<RequestView>` 的 ResponsePane 转为自身 `Entity<ResponseView>`：控件订阅、预览任务、PDF/媒体状态、保存任务和代际标记归响应 owner。对外只暴露接收响应/进度、清理与必要意图事件的窄接口。shell 通过事件订阅或明确的更新接口协调 request 与 response，不复制响应正文，也不成为全部业务状态的 owner。发送前清理、执行失败、取消和迟到结果保持当前行为。

### Novel Download：聚合一个完整能力

将 `crawler/` 与 `features/workspace/` 合为 `novel-download-feature`，保留 DownloadBackend、源解析、暂存输出、清理与 DownloadRuntime 的现有职责。下载目录/配置由 shell 注入；窗口初始化和日志目录留在 shell。WorkspaceView 按实际职责命名为下载视图，不仅为了与其他应用目录一致创建额外后端/源/PDF 等 crate。

### 本地化、资源与官方 skills

提取三个应用重复的 Fluent locale 检测、bundle 和翻译运行时到 `crates/app-i18n`，由 shell 注册应用目录中的资源。Feature 引用共享运行时，不引用 shell 类型。保留应用各自的 key、插值、locale 和原生 bundle 文件位置；Form 业务校验消息映射归所属 feature，shell 文案归 shell。新 crate 只处理本地化，不吸纳图标、主题或通用工具。

持久 UI 文本在合适的模型/展示入口转换为 `SharedString`；临时借用用 `&str`，编辑/格式化过程用 `String`。纯后端 API 数据保留传输含义，在 UI 边界产生持久共享文本；已依赖 GPUI 且 UI 持有的不可变 JSON/API 文本按官方指南在反序列化边界处理。翻译结果的跨帧保留与语言变更失效一致，不为了消除每次格式化建立无依据缓存。

官方 `gpui-kit`、`gpui-kit-design-guides` 使用 npx skills 的项目级安装/更新流程，保留来源和上游文件。已通过 `npx skills add longbridge/gpui-kit --skill gpui-kit gpui-kit-design-guides --agent codex --copy --yes` 接管这两个项目级官方 skills，来源记录在根 `skills-lock.json`。CLI 按当前上游同步了内容和文件列表。安装后的 gpui-kit 摘要仍使用“一律禁止公开字段”的概括；官网允许明确的记录类型公开字段。项目代码遵循官网的记录例外、非穷尽约束和构造入口，不手工修正官方副本。

项目自有 `gpui-app-development` 写入真实能力边界、依赖方向和 ownership；`gpui-i18n`、图标、Form、Store、Operation 和调试 skills 只更新实际受影响的路径、使用契约和例子。

## 实施顺序与每批交付

每批同时完成实现、消费者、必要文档/自有 skill 和受影响验证，交付可运行应用后继续下一批。目录调整优先解决所有权，不保留已失效的重复模块或无价值转发层。

| 批次 | 工作 | 完成条件 |
| --- | --- | --- |
| 1. 管理与基础边界 | 核对并按 npx 管理官方 skills；提取 app-i18n，迁移三应用调用；更新应用专属 crate 约定 | 官方目录无手工补丁；本地化 runtime 与 shell 解耦；三应用构建、本地化关键测试通过 |
| 2. Feiwen | 先迁移 data，再 fetch、query 和 shell 协调；调整结果文本/排序所有权，迁移相关布局 | Query 不引用 shell/FetchRun；各 crate 可单独构建测试；抓取、查询、排序和数据库状态关键流程可用 |
| 3. HTTP Client | 先迁移 core 契约/捕获，再独立 ResponseView，最后迁移 request 与 shell 协调；调整展示文本和布局 | core 不依赖 UI；Response 不依赖 RequestView；现有执行/捕获/预览/保存/取消关键回归通过 |
| 4. Novel Download | 聚合下载 feature，迁移配置注入与 shell；调整持久展示文本和布局 | 下载、取消、失败清理及提交输出的现有回归通过；应用可启动使用 |
| 5. 集成收尾 | 检查 workspace 依赖图、过时目录/规则、所有有效文档链接和 skills；完成 workspace 集成检查 | 无反向依赖、重复权威状态或失效规则；符合现有 hooks/CI；验证结果和实际限制记录完整 |

### 每个应用批次都覆盖的规则

- 轻量展示组件默认评估 `RenderOnce`；有独立生命周期、任务或订阅的功能使用 `Entity` + `Render`。保留有实际用途的原有实体，不按文件长度机械改类型。
- 父强持有子时子回引用 WeakEntity；订阅与持续 task 由明确 owner 保留。独立一次性任务按用途决定 detach；处理 owner/window 关闭与过期代际结果。
- Form 继续持有编辑权威，原生控件持有焦点/查询/滚动；外部同步不生成用户修改事件。保留既有 Form/Operation 状态机，避免第二套 loading/selection 权威。
- 固定布局宽度、间距、控件与图标几何迁移为 rem helper/语义 token；直接像素值按平台边界、测量、媒体/数据几何和主题定义逐项说明。表格列宽也纳入核对，不只搜索显式 `px(...)`。
- 尺寸缓存随 rem、宽度、字体、主题或内容变化失效；全高列 stretch、flex 最小尺寸释放和滚动 viewport/inset 分层正确。
- 同一命令按钮/菜单/快捷键共用 Action 或 owner 方法；正确注册菜单快捷键，验证改动的 Tab、焦点可见性、弹层关闭恢复与 disabled 状态。
- 主题继续通过受支持的更新入口同步颜色、tokens 和 Base 投影；自定义 spacing/elevation 如需保留，由明确设计系统 owner 管理。
- 行为公共状态默认私有；适用的记录型公开字段用 `#[non_exhaustive]` 并提供构造入口。已有公共接口改动核对实际消费者与指南适用范围，不全量改名或重写共享库。

## 文档归属与验证

根 README/AGENTS 描述 workspace、入口和应用专属 crate 位置；应用 README 描述窗口组合和功能依赖；新增 crate README 描述其公共能力、状态与生命周期。稳定用法归 README/指南，本计划只记录此次设计、实施和必要验证。已有有效开发文档中的路径随迁移更新；删除失效结论，不复制多份进度或指南。根开发索引登记本计划。

验证采用现有 Rust 工具链和 Nix 环境，按实际 hooks 与 `.github/workflows/ci.yml` 执行。

| 范围 | 必要验证 |
| --- | --- |
| 每批代码 | 受影响 package 及消费者的 `cargo build -p … --locked`、`cargo test -p … --locked`、Clippy；格式检查。沿用已有测试，测试随 owner 移动 |
| Feiwen | 数据库 schema/读取语义、本地化文本、排序/恢复/缺失值/行身份、查询取消和抓取状态协调；必要启动检查 |
| HTTP Client | 既有测试服务器覆盖的传输/重定向/解码/存储限额；原始正文完整性、预览代际、媒体/PDF 和保存的关键回归；受改动的原生交互 |
| Novel Download | 既有假后端测试的事件、取消、清理、暂存和最终输出；必要启动检查 |
| 布局与交互 | 受改动页面默认与放大基础字号、窄窗口和长文本；键盘、焦点、弹层和滚动。不为每个未改控件追加全量 UI 审计 |
| 文档/skills | 本地链接存在，资源/include 路径、例子和最终源码一致；Fluent key/参数一致；管理工具只触及选定官方技能 |
| workspace 集成 | `cargo fmt --all -- --check`、`cargo build --workspace --locked`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`；遵循现有平台 CI |

仅为具体未覆盖回归补测试；纯文件移动不新增镜像实现的测试。core 可脱离 UI 独立测试，但不把 crate 拆分称为已测得编译加速。应用位置、包名、bundle 元数据和原生 locale 位置保留，因此打包只核对受影响路径；实际打包行为改变时才运行对应 xtask 覆盖和必要打包检查。

实施记录分别报告本地自动化、原生启动/交互和 CI 的实际结果；未实测平台与未测量性能明确注明。迭代不以全平台实机验收作为每批试用的前置条件。

## 实施与验证记录

- 官方 skills 已经 npx 管理；项目自有应用结构和本地化 skills、README、AGENTS 与源码归属同步。
- 三应用的功能 crate 及共享 app-i18n 已迁移。Query 不依赖 shell/FetchRun；HTTP core 无 GPUI/Fluent/feature 依赖，ResponseView 独立持有订阅和任务；DownloadView 接收 shell 的下载目录。
- Feiwen 结果使用单一数据集合、排序索引与小说 ID，持久展示文本使用 SharedString。HTTP 保留可编辑 String 与原始 bytes，持久预览文本使用 SharedString。
- 固定布局迁移 rem；表格列宽在窗口 rem 改变时刷新，原生标题栏和媒体测量保留像素。
- Feiwen 的可选 `FEIWEN_DATA_DIR` 用于隔离启动；默认数据目录和 schema 保持不变。缺失配置目录通过数据库资源错误页处理。
- 已通过本地 `cargo build --workspace --locked`、`cargo test --workspace --locked` 和 `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`。新增回归覆盖排序后选中同一本小说、默认顺序恢复，以及新请求失效旧预览但保留独立保存任务。
- macOS 原生验证：HTTP 本地 health 请求显示响应、保存面板取消和清除响应；Feiwen 使用隔离数据库启动、导航与无条件空结果查询；Novel Download 启动与必填校验反馈。测试进程、临时 app wrapper 和隔离数据库已清理。未触及真实 Cookie、抓取站点或下载输出。
- 字号缩放、窄窗口、长文本及全部键盘/弹层组合未作全面原生验收；Windows/Linux 实机与远程 CI 未运行，未测量性能。自动化保留已有传输、解码、媒体/PDF、数据库修复、取消和暂存输出的覆盖。
- `cargo-shear` 未发现受影响包的无用依赖。自有文档链接存在；官方 `references/usage.md` 的上游 examples 链接未随 skills 安装进本仓库，保持管理工具的原样副本。
- 最后持久日志/查询错误文本和请求回调清理已完成定向复测：feiwen-fetch、feiwen-query、http-client-request 全部通过。格式检查、文档空白检查通过。验证使用 Nix devShell 和临时构建目录 `/tmp/gpui-255-target`，保留原 workspace 的 target 配置。
