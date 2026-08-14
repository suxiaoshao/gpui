# Feiwen：Form、Operation、Store 与数据库资源完整迁移计划

## 状态与范围

- 状态：`Done`。Query/Fetch Form 与私有 Transition、QueryCatalog Store/Operation、数据库
  resource/repair 路径及 workspace/titlebar 汇合均已落地；实际 UI 操作测试按本轮要求未执行。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 子任务 ID：`FEI-199-01`
- Feiwen 子任务索引：[Issue #199：Feiwen 子任务跟踪](README.md)
- 决策来源：[应用迁移调研与待确认问题](../../../../../docs/dev/issue-199/application-migration-decisions.md)
- Form producer plan：
  [FORM-199-02](../../../../../crates/gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- 所有者：`app/feiwen`
- 本地 ID 范围：`E/D/F/L/ST/DB/ERR/R/T-700..899`、`WP-700..799`

本文是 Feiwen 在 Issue #199 中的完整 owner plan，不是单独的 form migration。它同时负责：

- Query 与 Fetch 的 typed Form、不可变运行快照和私有 `Transition`；
- application-global QueryCatalog `gpui-store` + `refresh::Operation`；
- application-global DuckDB resource Store、打开失败页面、重新打开、备份后重建与文件回滚；
- 高级查询 recursive typed tree、runtime item identity、catalog options 投影与字段级验证；
- workspace/titlebar/UI、本地化、既有产品文档和完整迁移验证。

目录 `README.md` 只跟踪多轮子任务、状态与本文链接，不承载执行细节。

## 目标

1. 把可编辑草稿、单次运行状态、全局资源状态和 native component state 分成独立 owner。
2. Query 每次提交立即清空旧结果，同时冻结完整 `QueryDraft` 与执行用 `QuerySpec`，运行中允许
   编辑/reset Form，使用专门 Cancel；失败只显示原因与“载入表单”。
3. Fetch 冻结 `FetchRequest`，运行中允许编辑下一次 Fresh Form；Fresh 禁止重入并清空上轮，Resume/Retry
   固定原 snapshot，从失败/中断位置继续，始终只有一个 run。
4. 用全局 QueryCatalog Store 管理 tags/authors Data、phase、problem 与 invalidation generation；启动时
   eager Load，连续数据库提交合并刷新。
5. 用全局数据库资源 Store 让打开失败也成为可观察 `Unavailable`，只有精确 `Ready` 才发放新
   database job；资源错误与修复入口不再散落在 Query/Fetch。
6. 把高级查询改成无 form-only ID 的 recursive typed tree；dynamic item identity、typed path、UI key
   与 stale callback protection由 Form runtime提供。
7. 只在对应字段旁显示 validation issue；catalog phase/problem 与数据库 problem 不伪装成字段错误。

## 非目标

- 本轮不迁移 HTTP Client 或 Novel Download。
- 不处理 Jaco Conversation、Jaco MCP runtime 或其他 app 状态机。
- 不改变 Feiwen 抓取协议、网页解析算法、SQL查询语义、DuckDB schema或数据格式。
- 不改变 `Tag::tags_with_id` 排除 `id IS NULL` 的既有 SQL contract。
- 不增加运行历史、并行 Fetch、detached producer、RunId、跨进程 repair lease或 PID 文件。
- 不让 Query/Fetch 使用预定义 `refresh::Operation`/`repair::Operation`；它们使用 Feiwen 私有 Transition。
- 不在 QueryCatalog Store 中保存 Form draft、native control entity或页面 subscription。
- 不在数据库 `Ready` 页面显示或接受 Reopen/BackupAndRebuild。

## 适用范围

| 表面 | 目标 owner | 技术选择 |
| --- | --- | --- |
| Query 可编辑条件 | `Entity<Form<QueryDraft>>` | Form vNext recursive typed tree |
| Query 单次运行 | `QueryView` 内私有 `QueryRun` | Feiwen `Transition<QueryMessage>` |
| Fetch 可编辑请求 | `Entity<Form<FetchDraft>>` | Form vNext flat form |
| Fetch 单次运行/日志 | workspace-owned `Store<FetchRun>` | Feiwen `Transition<FetchMessage>` |
| tags/authors catalog | application-global `Store<QueryCatalogState>` | `refresh::Operation` + generation |
| DuckDB pool/resource | application-global `Store<DatabaseResource>` | Feiwen私有 database Transition |
| Query results table | QueryView/table delegate | 当前run的页面投影，不进global Store |
| native inputs/selects | Form adapter / advanced renderer | entity、focus、subscription仍归UI owner |

## 实施前证据

| ID | 当前事实 | 主要位置 | 迁移影响 |
| --- | --- | --- | --- |
| `E-700` | `QueryView` 同时持有 AdvancedQueryState、options、validation error、Search Task和table | `src/features/query.rs` | 拆成Form、QueryRun、Catalog与table projection |
| `E-701` | Query start设置loading但不先清旧rows，运行期间锁整个advanced editor | `query.rs::start_search` | 开始即清表；只禁用submit/catalog controls，不锁普通Form |
| `E-702` | QueryOptions在页面创建/Reset时同步从Db读取 | `advanced/options.rs`、`query.rs` | 移到global Catalog；Reset只reset Form |
| `E-703` | AdvancedQueryState混合递归业务draft、u64 ID、native entities、subscriptions和error | `advanced/state.rs` | typed tree与adapter/runtime identity拆开 |
| `E-704` | `FetchTaskState`混合URL/页码/Cookie、status/logs和Task | `features/fetch.rs` | 拆成Fetch Form + Store<FetchRun> |
| `E-705` | Fetch Resume/Retry重新读取当前可编辑字段 | `fetch.rs::start_fetch_from` | 终态保存immutable snapshot并固定续跑范围 |
| `E-706` | 当前fetch唯一spawned runner串行投递全部事件，没有detached producer | `fetch.rs::Runner` | 维持唯一Task；本轮不加RunId |
| `E-707` | 每个Novel save独立事务；后续失败不回滚此前成功写入 | `store/service/novel.rs`、fetch runner | 每次成功commit推进catalog invalidation generation |
| `E-708` | `Db` 是一次性Global；打开失败只日志后return，consumer仍假设global存在 | `src/store.rs` | 始终安装Database Store并统一resource UI |
| `E-709` | DuckDB依赖为duckdb-rs `1.10505.0`，对应DuckDB `1.5.5` | `app/feiwen/Cargo.toml`、crate metadata | repair协议按DuckDB 1.5.5验证 |
| `E-710` | workspace强持有一个FetchTaskState并让Query观察它 | `app/workspace.rs` | 改观察FetchRun Store selection，不让Query持有fetch draft |
| `E-711` | titlebar Query Reset/Search都受searching gate，尚无Cancel | `app/titlebar.rs` | Reset保持可用；新增Cancel；Search按DB/Catalog/Run gate |

## 跨所有者契约

| ID | Producer | Feiwen消费 | Gate |
| --- | --- | --- | --- |
| `C-700` | Form `C-500` | QueryDraft/FetchDraft、validator、prepare/replace/reset、typed paths | public contract与compile fixture通过 |
| `C-701` | Form `C-501` | recursive items/case、runtime identity、field issue、stale callback | topology/validation tests通过 |
| `C-702` | Form `C-502` | Input/Select/Combobox/IntegerInput、PathKey与row callback | adapter tests通过 |
| `C-703` | Feiwen DB | exact Ready job gate和resource problem | Query/Fetch/Catalog不再clone裸Global pool |
| `C-704` | Feiwen Catalog | phase/data/options/invalidation generation | advanced adapter和Query submit只消费Store projection |
| `C-705` | Feiwen Query/Fetch | snapshot与唯一Task lifecycle | UI、runner、table/log没有第二份authority |

## 架构决定

### `D-700`：迁移分成两个可并行通道

通道 A 不依赖未实现 Form，可先完成数据库资源 Store、QueryCatalog Store/Operation与其定向tests。
通道 B 等 `C-700`–`C-702` producer-ready 后迁移 Query/Fetch Form与高级查询adapter。最终workspace、
titlebar、本地化和删除旧state在两通道汇合后一次完成。

### `D-701`：Query 使用私有状态机，不使用预定义 Operation

目标状态与消息语义固定为：

```rust,ignore
enum QueryRun {
    Idle,
    Running { snapshot: QueryDraft, task: Task<()> },
    Succeeded { count: usize },
    Failed { snapshot: QueryDraft, problem: QueryProblem },
}

enum QueryMessage {
    ClearTerminal,
    Start { snapshot: QueryDraft, task: Task<()> },
    Complete(Result<QueryResult, QueryProblem>),
    Cancel,
}
```

- Query command先清空table与旧terminal state，再执行Form prepare；校验失败保持空table并只展示字段错误。
- `Start` 只从非Running接受；同步安装 `Running` 后runner才会被event loop poll。
- completion只从 `Running` 接受。成功在同一UI turn安装rows并进入Succeeded；失败保持空table，保留
  snapshot并进入Failed。
- `Cancel` 只从Running接受，drop唯一lifecycle Task并回到Idle；不reset Form、不恢复旧table。
- UI在Running禁用Search并显示独立Cancel；Reset和普通Form字段仍可用。
- 非法第二个Start或非Running completion保留原状态、记录bug并丢弃，不排队。
- runner不得把background job的结果直接投递给view；只能由Running持有的父Task await后投递。drop父Task
  后即使同步DuckDB工作稍后结束，也没有completion route，因此本轮不引入QueryRunId。
- 父Task闭包独立持有从同一份 prepared draft 编译出的 `QuerySpec`；Transition保留完整 `QueryDraft`
  snapshot，避免 relation 当前未使用的operand在失败恢复时丢失。

### `D-702`：Query Form与snapshot恢复

- `QueryView`强持有一个 `Entity<Form<QueryDraft>>`；Form不拥有QueryRun或table。
- Search在DB与Catalog精确Ready、非Running时调用prepare，并纯函数转换为 `QuerySpec`。
- 运行中编辑只影响下次Search；Reset只调用Form reset，不触碰QueryRun、snapshot、Task或table。
- Failed UI只显示错误原因与“载入表单”；按钮直接用提交时冻结的 `QueryDraft` whole-model replace，
  明确覆盖用户当前草稿。不提供“复制错误详情”。
- `QueryDraft -> QuerySpec` 必须保持可执行查询的排序、negation、group顺序与当前有效 literal；失败恢复
  不再依赖反向猜测，而是直接使用完整 draft snapshot，因此未被当前 relation 使用的operand也不会丢失。

### `D-703`：Fetch Form与唯一运行Store

```rust,ignore
struct FetchDraft {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
}

enum FetchRun {
    Idle,
    Running { snapshot: FetchRequest, progress: FetchProgress, logs: Vec<FetchPageLog>, task: Task<()> },
    Interrupted { snapshot: FetchRequest, progress: FetchProgress, logs: Vec<FetchPageLog> },
    Failed { snapshot: FetchRequest, progress: FetchProgress, logs: Vec<FetchPageLog>, failure: FetchFailure },
    Succeeded { snapshot: FetchRequest, progress: FetchProgress, logs: Vec<FetchPageLog> },
}
```

- workspace安装一个 `Store<FetchRun>`；FetchView持有 `Entity<Form<FetchDraft>>`，两者不复制字段。
- Fresh只从非Running接受，prepare生成snapshot，清空上轮logs/terminal state并安装唯一Task。
- Running收到第二个Fresh保留原状态并拒绝；UI按钮同时禁用。Form字段本身保持可编辑。
- Interrupt drop唯一Task并保留snapshot/progress/logs；Resume从last success下一页继续。
- RetryFailed从失败页继续；Resume/Retry始终使用原URL/Cookie/start/end，不读取当前Form。
- Resume/Retry保留本轮logs；新的Fresh覆盖上轮终态并清空logs；不保存run history。
- runner不得detach会继续投递progress/completion的子Task。所有事件只能经当前Running持有的唯一Task；
  因此不加RunId。未来若需要并行或detached producer，必须重新打开本决定。
- success只更新FetchRun，不replace/reset/rebase Form。

### `D-704`：Fetch snapshot UI

- 只要FetchRun含snapshot，UI显示URL、原始页码范围、当前/失败/下一页和Cookie“已设置/未设置”。
- 不显示、临时揭示、复制或记录Cookie原文；日志与error diagnostics同样不得包含它。
- “载入表单”一次性把完整snapshot转回FetchDraft并whole-model replace；不是剪贴板复制，也不改变
  当前run snapshot。

### `D-705`：QueryCatalog 使用 global Store + refresh Operation

```rust,ignore
struct QueryCatalogState {
    operation: refresh::Operation<QueryCatalogData, QueryCatalogProblem, CatalogTask>,
    invalidation_generation: u64,
    covered_generation: u64,
}

struct CatalogTask {
    target_generation: u64,
    task: Task<()>,
}
```

- Data包含tags/authors的typed options，不保存native Select/Combobox entity。
- DB初始Ready后立即Load，不等待Query页面；首次load仍是异步，页面必须处理Loading。
- 每次Load/Refresh/Retry捕获 `target_generation = invalidation_generation`。
- 每个影响tags/authors的成功Novel事务只推进invalidation generation；不为每本记录立即spawn Refresh。
- 成功安装Data后才把covered推进到target。若仍有未覆盖generation、DB Ready且无active task，最多自动
  再启动一次Refresh。
- failure/cancel不推进covered；pending generation保留，由合法的显式Load/Retry/Refresh继续消费。
- DB不Ready时catalog必须离开精确Ready并取消active task；repair恢复Ready后不自动load。Query页面提供
  独立“重新加载目录”，owner按当前phase映射Load/Retry/Refresh。

### `D-706`：Catalog phase与UI/表单投影

- 精确Ready：catalog controls可编辑，Query可在其他gate满足时submit。
- Loading/Unavailable：catalog controls禁用，无可提交options；普通非catalog Form字段仍可编辑。
- Refreshing/Degraded：last-known options保持可见但只读，Query按钮禁用，显示phase/problem。
- missing current value不被清除或fallback；显示非阻塞“当前目录中不存在”。Catalog精确Ready时仍允许
  提交该typed literal，非Ready由page gate阻止提交。
- Data更新顺序固定为：发布Store Data -> 更新仍挂载native items -> 从Form读当前typed value并静默
  重投影selection -> 替换validator context -> 显式dynamic validation。
- Reset只reset Form，不触发Catalog refresh。

### `D-707`：高级查询使用无业务NodeId的recursive typed tree

建议业务形状固定为：

```rust,ignore
struct QueryDraft {
    #[form(child)]
    root: FilterGroupDraft,
    #[form(items)]
    sorts: Vec<SortDraft>,
}

struct FilterGroupDraft {
    relation: GroupRelation,
    negated: bool,
    #[form(items)]
    children: Vec<FilterNodeDraft>,
}

struct FilterNodeDraft {
    #[form(child)]
    kind: FilterNodeKind,
}

enum FilterNodeKind {
    Condition(FilterConditionDraft),
    Group(FilterGroupDraft),
}
```

- QueryDraft/QuerySpec不保存纯表单ID；Form runtime的ItemPath/PathKey负责递归定位、remove/move、UI key、
  validation与binding。
- static `.then(...)` 不需要Form；进入enum/optional payload时只调用已确认的
  `.try_case(query_form.read(cx), CaseDef)` / `.try_some(query_form.read(cx))`。返回path捕获当前
  incarnation但不持有entity；Feiwen不接收或缓存 `TopologyIndex`/snapshot。
- condition field使用Rust enum区分Text/Number/Bool/Tags/Author payload；切换field type原子替换variant，
  清空旧relation/value并退休旧subtree。
- relation只是同一field payload内的运算符；改变relation不清除field-owned value。
- number payload稳定保存single/min/max operands；author payload稳定保存text/single/multiple；relation只决定
  哪些operand当前显示、校验并编译进QuerySpec，未使用值保留且不报错。
- group All/Any只改运算符，不修改children。
- validation issue只在对应字段展示，不生成condition/group/page汇总。

### `D-708`：数据库资源使用私有 Transition，不预建 Retiring

本轮可达状态只有：

```rust,ignore
enum DatabaseResource {
    Loading { task: Task<()> },
    Ready { pool: DbConn },
    Unavailable { problem: DatabaseProblem },
    Repairing { repair: DatabaseRepair, problem: DatabaseProblem, task: Task<()> },
}

enum DatabaseRepair {
    Reopen,
    BackupAndRebuild { backup_dir: PathBuf },
}
```

- 应用启动始终安装Store；初次打开/建schema成功进入Ready，失败进入Unavailable。
- consumer只能通过 `DatabaseStore::with_ready_job`/等价gate取得本次job所需pool clone；不再读取裸`Db`
  Global。若非Ready，入口同步拒绝并由resource problem负责UI。
- 普通Query/Fetch数据库错误只结束各自run，不自动把DB从Ready改为Unavailable。
- Ready UI没有repair入口，其他event route也拒绝repair，因此本轮不存在Ready->Retiring/Repairing。
- 从Unavailable发起repair时没有可用pool；Reopen直接Repairing，BackupAndRebuild先二次确认和选择backup
  path，再投递Repairing。Repairing拒绝重复repair。
- 因当前没有任何实际资源级producer要求让一个仍被job持有的Ready pool退役，本轮不实现不可达
  `Retiring`、active-job drain或consumer cancel-all。未来出现真实Ready pool替换需求时另开设计，不为
  状态对称预留空phase。

### `DB-700`：DuckDB 备份后重建文件协议

本协议针对当前 duckdb-rs `1.10505.0` / DuckDB `1.5.5`。DuckDB官方说明：WAL包含崩溃后恢复所需
数据，`CHECKPOINT`把WAL同步进主文件；而本动作从无法安全打开的 `Unavailable` 开始，不能依赖先成功
checkpoint。因此备份必须保留原始主文件和存在的 `.wal`，不能只复制 `data.duckdb`。

依据：

- [DuckDB CHECKPOINT](https://duckdb.org/docs/current/sql/statements/checkpoint)
- [DuckDB crashes / WAL recovery](https://duckdb.org/docs/current/guides/troubleshooting/crashes)
- [DuckDB files created](https://duckdb.org/docs/current/operations_manual/footprint_of_duckdb/files_created_by_duckdb)
- [DuckDB COPY FROM DATABASE](https://duckdb.org/docs/current/sql/statements/copy)

`BackupAndRebuild` 固定执行：

1. UI二次确认后让用户选择一个尚不存在的backup目录；取消选择不投递message。
2. Transition同步把Unavailable替换为Repairing并持有唯一Task。再次检查没有pool保存在resource中。
3. 创建backup目录；逐个复制存在的 `data.duckdb`、`data.duckdb.wal` 到该目录，并对每个目标文件
   `sync_all`，最后同步backup目录。一个artifact复制/同步失败即返回Unavailable，绝不进入重建。
4. 如果主文件与WAL都不存在，视为backup失败；不以空目录伪装成功。
5. 在live数据库同一父目录创建唯一staging目录/文件，确保后续rename不跨filesystem。用新的DuckDB
   connection创建staging，执行现有 `initialize_schema`、`CHECKPOINT`，drop全部connection，再重新
   open并验证必需tables/indexes；最终再次checkpoint并关闭全部connection。验证失败不碰live artifacts。
6. 把现有live主文件和WAL分别rename到同一父目录的唯一rollback目录；任一步失败立即按反向顺序恢复。
7. 把staging数据库的完整artifact集合rename为固定live名称，先同步父目录，再open/checkout/验证schema。
   rename、同步或验证任一步失败时先关闭新connection，把失败的新artifacts移回staging/quarantine，
   再把rollback artifacts恢复到原名并同步父目录，最终回到Unavailable；rollback自身失败时同时报告
   primary与rollback错误以及backup path。
8. 新live数据库验证成功且父目录同步成功是不可逆commit point。此后安装唯一新pool并进入Ready，发布
   backup path成功通知；backup目录永不自动删除、导入或恢复。
9. commit后尽力删除同目录rollback临时artifacts并同步父目录。清理失败只记录残留路径和诊断，留待后续
   定向清理；不得因为旧artifacts残留而撤销已经验证并发布的live数据库，也不得自动删除用户backup。

该协议不使用 `COPY FROM DATABASE` 生成“新数据库”，因为产品决定是备份旧文件后建立空schema，不是
把可能损坏的数据复制进新库。`COPY FROM DATABASE` 只作为未来可正常打开数据库时的逻辑复制参考。

### `D-709`：数据库resource UI与repair后的consumer

- Ready只渲染正常Workspace，不显示Reopen/BackupAndRebuild。
- Loading渲染统一loading resource page；Unavailable渲染problem + Reopen + BackupAndRebuild；Repairing
  显示运行状态并禁用两个按钮。
- Reopen重新打开固定路径的现有数据库并验证schema，不创建替代路径、不清数据。
- repair成功只恢复DB Ready。Query/Fetch重新允许用户操作但不自动重跑；QueryCatalog保持非Ready，
  必须点击自己的“重新加载目录”。
- repair失败回Unavailable并保留新的problem；Query/Fetch/Catalog不各自复制repair按钮。

## 文件与所有权

| ID | 文件 | 动作 | 责任 |
| --- | --- | --- | --- |
| `F-700` | `app/feiwen/Cargo.toml` | 添加workspace `gpui-form`、adapter、`gpui-store`、`gpui-operation` | 不新增外部版本，不手改lockfile |
| `F-701` | `src/store.rs` | 删除一次性`Db` Global consumer API；保留schema/query/service exports | 数据模块入口 |
| `F-702` | `src/store/database.rs` | 新增Database Store、Transition、Ready gate、open/reopen/rebuild protocol | DB唯一resource owner |
| `F-703` | `src/main.rs` | 按顺序安装DB Store、Catalog Store和app | startup wiring |
| `F-704` | `src/app/resource.rs` | 新增数据库loading/problem/repair页面与confirm/path picker | 仅Unavailable显示repair |
| `F-705` | `src/app/workspace.rs` | 创建Query/Fetch Form session、安装/观察FetchRun Store、按DB phase路由resource page | workspace composition |
| `F-706` | `src/app/titlebar.rs` | Query Reset/Search/Cancel、Catalog/DB gate、Fetch Fresh gate | intent入口，不持有状态副本 |
| `F-707` | `src/features/query.rs` | 重写为QueryRun Transition、snapshot、table effect、error/recovery UI | Query单次run owner |
| `F-708` | `src/store/catalog.rs` | 新增global Catalog Store、Operation、generation、selectors/intents | catalog唯一owner |
| `F-709` | `src/features/query/form.rs` | 新增QueryDraft typed tree、validator、Prepared->QuerySpec与逆转换 | 纯业务draft/compile |
| `F-710` | `src/features/query/advanced.rs` | 改为advanced Form renderer/adapter入口 | 不持有business state副本 |
| `F-711` | `src/features/query/advanced/{state,spec}.rs` | 已迁移tests并删除 | 旧mixed owner退出 |
| `F-712` | `src/features/query/advanced/{controller,options,render,sort}.rs` | 拆成typed path controller、静态choices、native adapter与renderer | runtime PathKey定位并按key复用未受影响controls |
| `F-713` | `src/features/query/advanced/components.rs`、`components/numeric_range_input.rs` | 已删除，由typed number operands与Form adapter替代 | 不保存业务value副本 |
| `F-714` | `src/features/query/results_table.rs` | 只保留rows/sort/render projection | 不保存QueryRun authority |
| `F-715` | `src/features/fetch.rs` | 收缩为feature facade/page composition | 删除FetchTaskState mixed owner |
| `F-716` | `src/features/fetch/form.rs` | 新增FetchDraft、validator、snapshot转换 | 可编辑Form owner |
| `F-717` | `src/features/fetch/run.rs` | 新增FetchRun/Message/Transition、Store selectors/intents | 唯一run authority |
| `F-718` | `src/features/fetch/runner.rs` | page runner、snapshot-only续跑、commit后Catalog invalidation | 唯一Task producer |
| `F-719` | `locales/{zh-CN,en-US}/main.ftl` | 新增Query/Fetch snapshot、Cancel、Catalog phase、DB repair等同构keys | 用户可见文案双语 |
| `F-720` | `docs/advanced-query-prd.md`、`docs/fetch-workflow-prd.md`及受影响feature docs | 实施后同步已确认产品语义 | 稳定产品文档 |
| `F-721` | `docs/dev/issue-199/README.md`与本文 | README只索引；本文登记实施/验证 | 中文开发文档 |

新增Rust module一律使用同名 `.rs` 入口，不新增 `mod.rs`。

`store/query.rs` 与 `store/service/{novel,tag}.rs` 的SQL/schema默认No change。runner只在现有
`Novel::save` 返回成功后投递invalidation；除非实现证明需要返回commit outcome，不能顺手重写service。

## 生命周期与数据流

### `L-700`：启动

```text
install DatabaseStore(Loading)
  -> open/create/check schema
  -> Ready: install/start Catalog Load
  -> Unavailable: render DB resource page，Catalog保持dependency problem
  -> create Workspace/Form sessions
```

Catalog首次Load只在启动期DB第一次Ready后自动触发。repair后的Ready不走startup hook。

### `L-701`：Query submit

```text
Search intent
  -> 同步检查 !Running + DB Ready + Catalog Ready
  -> 立即清空旧table/terminal result
  -> Form.prepare + map(QuerySpec)
  -> 创建唯一parent Task
  -> Transition Start(snapshot, task)
  -> background checkout/query
  -> parent Task投递Complete
  -> Transition state + table effect同turn发布
```

Form validation失败不进入QueryProblem；DB/catalog gate失败不伪装成字段issue。

### `L-702`：Fetch Fresh/Resume/Retry

Fresh从Form prepare生成snapshot；Resume/Retry只从FetchRun终态读取snapshot。runner逐页更新当前Store；每个
Novel transaction成功后只推进Catalog generation。Interrupt/drop view/drop workspace都通过唯一Task
终止completion route，不detach producer。

### `L-703`：Catalog invalidation

```text
Novel commit Ok
  -> generation += 1
  -> catalog idle/Ready && DB Ready: start one Refresh(target=generation)
  -> running: only retain higher generation
  -> success: covered=target; if generation>covered start one follow-up Refresh
  -> failure/cancel/nonReady: covered unchanged，等待显式合法intent
```

### `L-704`：数据库repair

repair intent只从Unavailable进入Repairing。由于本轮没有Ready->repair路径，repair开始前resource里不存在
可用pool，也没有需要取消的consumer job。Task完成后一次Transition安装Ready或新的Unavailable；文件
操作全部在blocking executor完成，UI thread只归约state/effect。

## 状态与错误契约

### `ST-700`：Query

- `QueryProblem`只表示本轮runtime/I/O/DB job错误；字段validation不进入它。
- Running是Task唯一owner；取消/状态替换drop Task。
- results table不是第二状态机；只接收Transition effect。

### `ST-701`：Fetch

- FetchRun是snapshot/progress/logs/failure/Task唯一owner。
- 非法phase message保留原状态并记录bug；不默认修复、不排队、不spawn第二run。
- Cookie不得进入Display/Debug/tracing字段、用户日志或snapshot文本。

### `ST-702`：Catalog

- Operation是phase/Data/problem/Task唯一owner；generation是Operation外唯一control metadata。
- last-known Data可以在Refreshing/Degraded显示，但只有精确Ready有提交能力。
- DB dependency problem只让Catalog退出Ready；权威数据库错误和repair仍由DatabaseStore展示。

### `ST-703`：Database

- `DatabaseProblem`区分Open、Reopen、Backup、BuildStaging、Swap、Validate、Rollback。
- backup失败不得执行staging/swap；staging失败不得碰live。
- commit point前的swap/validate/sync失败必须rollback；rollback自身失败时保持Unavailable，同时在problem
  中包含primary与rollback错误，并明确backup path；不得发布Ready或静默创建第二个target。
- commit point后的rollback临时目录清理失败不改变Ready；记录残留路径，用户backup始终保留。

### `ERR-700`：Form

Form `ResolveError/MutationError/PrepareError`留在表单/adapter边界。stale dynamic callback是生命周期
no-op；用户validation只在对应field展示。

### `ERR-701`：资源与运行

Query/Fetch本轮错误可引用“数据库不可用”，但不复制repair action。Catalog missing literal是非阻塞hint；
Catalog phase、DB phase和network/database runtime failure均不写成field issue。

## 风险

| ID | 风险 | 防护 |
| --- | --- | --- |
| `R-700` | 把Feiwen计划缩成Form迁移，遗漏Operation/Store/DB | 本文完整owner范围与WP通道 |
| `R-701` | Form签名未定时应用造shim | `C-700`–`C-702` release gate |
| `R-702` | Query取消后background completion污染新run | 唯一parent Task route + controllable cancellation test；无direct callback |
| `R-703` | Fetch Resume读取当前Form | 终态snapshot-only API与Form编辑并发test |
| `R-704` | 每本Novel触发refresh storm或漏掉partial commits | generation coalescing与partial failure integration test |
| `R-705` | catalog刷新清除missing typed value | options->selection->validator顺序与nonblocking hint test |
| `R-706` | dynamic u64 ID在renderer/action中残留 | PathKey/ItemPath residual scan |
| `R-707` | raw copy遗漏DuckDB WAL | `DB-700` artifact fixture与官方WAL contract |
| `R-708` | staging失败后live已被删除 | staging先验证；rename到rollback；任何失败反向恢复 |
| `R-709` | 为未来对称性实现不可达Retiring | 本轮状态枚举/residual明确不含Retiring |
| `R-710` | Ready仍出现repair按钮 | phase rendering/action route测试 |
| `R-711` | Cookie泄露到UI/log/problem | snapshot formatter与tracing residual/test |

## 测试契约

| ID | 层级 | 场景 | 验收 |
| --- | --- | --- | --- |
| `T-700` | plain Rust | QueryDraft递归compile/restore、field type/relation变化 | typed tree等价；field切换清空；relation切换保留value |
| `T-701` | GPUI | Query start/reset/cancel/complete/fail/recover | start清表；Reset只Form；cancel无completion；失败只原因+载入 |
| `T-702` | GPUI | Query运行中编辑、Catalog/DB gate |普通字段可编辑；Search/catalog controls按phase禁用 |
| `T-703` | plain Rust | Fetch Fresh/Resume/Retry page计算与snapshot转换 | 固定URL/Cookie/range；Fresh清logs；无history/RunId |
| `T-704` | GPUI | Fetch运行中编辑、重入拒绝、snapshot载入 | 当前run不变；Fresh disabled+rejected；success不改Form |
| `T-705` | lifecycle | Query/Fetch唯一Task取消与late producer | drop后无direct completion；非法event不改state |
| `T-706` | Store/Operation | Catalog Load/Refresh/Retry/phase UI | precise Ready gate；last-known options只读 |
| `T-707` | generation | active refresh期间连续invalidation、success/failure/cancel | 最多一次follow-up；covered只在success推进 |
| `T-708` | adapter GPUI | catalog options更新、missing value、recursive row reorder/delete | value不丢；hint非阻塞；错误/控件不串row |
| `T-709` | DB integration | initial open success/failure、Reopen、Ready action gate | Store始终存在；Ready无repair；Unavailable两动作 |
| `T-710` | DB filesystem | backup main+WAL、copy/sync failure、empty artifacts | backup失败前不建staging，不遗漏WAL |
| `T-711` | DB filesystem | staging/build/validate、每个rename/sync/open/cleanup注入失败 | commit前失败回滚并保持Unavailable；commit后cleanup失败保持Ready且记录残留路径；backup保留 |
| `T-712` | Fetch/DB/Catalog integration | partial page commit后失败/中断 | 已提交数据保留且generation推进，不等最终Success |
| `T-713` | i18n/security | 两个locale key/变量集合、Cookie格式与logs | key parity；无secret明文 |
| `T-714` | residual | active Feiwen source/tests | mixed state、裸Db Global、u64 form ID、运行期全表单disabled零残留 |

## 工作包

### `WP-700`：固定owner边界与依赖

- 刷新本文 `E-700`–`E-711` inventory；确认 Form producer状态、Operation/Store当前API和DuckDB 1.5.5。
- 添加四个workspace依赖，建立新module入口和compile-only types；不同时迁移业务流程。
- 验收：两个实施通道可独立编译；没有旧/新owner双写。

### `WP-701`：数据库资源 Store与repair UI

- 实现 `F-701`–`F-704`、`D-708/D-709`、`DB-700`。
- 先完成initial open/Ready gate/resource page，再做Reopen，最后做BackupAndRebuild failure injection。
- 不实现Retiring/active-job drain；Ready repair在Transition和UI两层拒绝。
- 验收：`T-709`–`T-711`；`C-703` producer-ready。
- 依赖：`WP-700`。

### `WP-702`：QueryCatalog global Store与generation

- 实现 `F-708`、`D-705/D-706`；startup只在initial DB Ready时eager Load。
- 提供Store selectors/intents与显式“重新加载目录”；先通过纯state/generation tests再接native controls。
- 验收：`T-706/T-707`；`C-704` producer-ready。
- 依赖：`WP-701`。

### `WP-703`：高级Query typed Form与adapters

- Form `C-700`–`C-702` producer-ready后实现 `F-709`–`F-713` 与 `D-702/D-707`；不得重新引入纯
  `.case/.some` 或Feiwen-local topology resolver。
- 先建立pure QueryDraft/validator/QuerySpec双向转换，再迁recursive renderer/controls，最后接Catalog options。
- 删除业务u64 ID与mixed `AdvancedQueryState`；旧tests迁到new owner后才删除文件。
- 验收：`T-700/T-708`；依赖 `WP-700`、`WP-702`、Form producer gates。

### `WP-704`：Query私有Transition与UI

- 实现 `F-707/F-714`、`D-701`，拆Search/Reset/Cancel与table effects。
- start/validation都先清旧table；运行中只禁用Search和catalog相关controls，Reset保持可用。
- 增加失败snapshot载入Form；不加错误详情复制按钮。
- 验收：`T-701/T-702/T-705` Query部分。
- 依赖：`WP-703`、`C-703/C-704`。

### `WP-705`：Fetch Form、Store Transition与runner

- 实现 `F-715`–`F-718`、`D-703/D-704`。
- Fresh/Resume/Retry只通过FetchRun messages；runner只持snapshot与唯一Task route。
- 每个Novel save成功后推进Catalog generation；不等待整轮Succeeded。
- 验收：`T-703`–`T-705` Fetch部分、`T-712`。
- 依赖：`WP-700`、`WP-702`、Form `C-700/C-702`。

### `WP-706`：workspace/titlebar/resource汇合

- 实现 `F-703`、`F-705/F-706`，移除Query对FetchTaskState的直接依赖。
- 按DB phase路由normal workspace/resource page；接Query/Fetch/Catalog selectors与全部button gates。
- 验收：DB Ready/Unavailable、Catalog nonReady、Query Running、Fetch Running组合状态不出现矛盾按钮。
- 依赖：`WP-701`–`WP-705`。

### `WP-707`：本地化、产品文档与旧实现删除

- 同步 `F-719/F-720`；开发README只更新状态/链接，详细结果仍回本文。
- 删除旧SearchState/QueryEvent mixed职责、FetchTaskState、AdvancedQueryState/business IDs与裸Db Global。
- 保持HTTP Client、Novel Download、SQL/schema、assets和bundle localization No change。
- 验收：`T-713/T-714`。
- 依赖：`WP-706`。

### `WP-708`：定向自动化验证与 UI 边界记录

- 按下方顺序执行最小充分Cargo命令；运行Query、Fetch、Catalog与DB recovery定向自动化场景。
- destructive DB tests全部使用临时目录与fixture，不操作用户真实config database。
- 记录每个WP/T-ID、实际commit/PR、UI覆盖与未执行边界；本轮按用户要求不做实际 UI 操作测试。
- 依赖：`WP-707`。

## 验证

实现阶段按需先运行新owner的定向tests，最终只执行一次：

```text
cargo fmt --all
cargo test -p feiwen --locked
cargo check -p feiwen --all-targets --locked
cargo clippy -p feiwen --all-targets --all-features --locked -- -D warnings
git diff --check
```

Form producer tests由 `FORM-199-02` 负责；Feiwen只运行应用集成fixture。数据库filesystem tests使用
临时目录，不触碰真实 `data.duckdb`。以下实际 UI smoke 场景按用户本轮要求未执行：

1. DB startup failure -> Unavailable page -> Reopen失败/成功；
2. BackupAndRebuild取消确认、取消path picker、backup失败、成功后Ready且Catalog仍需显式reload；
3. Query运行中编辑/reset/cancel、失败snapshot载入、每次start不见旧table；
4. Catalog loading/refreshing/degraded/missing literal、generation follow-up；
5. Fetch运行中编辑、Fresh重入禁用、interrupt/resume、failure/retry、snapshot载入与Cookie脱敏。

Residual scan至少覆盖：

```text
struct Db(
global::<Db>
FetchTaskState
AdvancedQueryState
next_id: u64
group_id: u64
node_id: u64
set_disabled(true)
SearchState::Task
QueryEvent::Reset
```

命中需逐项分类；真实数据库/author/tag领域ID不属于form-only identity，不能误删。

## 实施结果（2026-08-05）

| 工作包 | 结果 |
| --- | --- |
| `WP-700` | 已完成依赖、owner 边界与最终 Form producer gate 对齐。 |
| `WP-701` | 已完成全局 Database Store、Ready/Unavailable/Repairing Transition、Reopen 与 BackupAndRebuild；Ready 不显示修复入口。 |
| `WP-702` | 已完成 application-global QueryCatalog Store、`refresh::Operation`、generation coverage 与启动加载。 |
| `WP-703` | 已完成 recursive typed `QueryDraft`、Form runtime identity、typed validation、relation operand保留与catalog missing-value投影；结构变化按 `PathKey` 复用未受影响native controls，业务模型不保存 form-only ID。 |
| `WP-704` | 已完成 Query 私有 Transition、start 清表、Cancel、运行中 Form 可编辑/reset 与精确 `QueryDraft` 失败 snapshot 载入；Catalog data与phase/problem分开观察。 |
| `WP-705` | 已完成 Fetch Form、单一 `FetchRun` 枚举状态机、Fresh/Resume/Retry snapshot、唯一 Task 与逐条 catalog invalidation。 |
| `WP-706` | 已完成 workspace/titlebar 的 DB、Catalog、Query、Fetch gate 汇合。 |
| `WP-707` | 已完成中英文 locale，并同步高级查询、抓取配置、路由与对应测试说明；旧 mixed run/裸 DB/form-only u64 identity 已删除，HTTP Client、Novel Download 保持 No change。 |
| `WP-708` | 自动化门禁与临时目录 DB 场景通过；实际 UI 操作测试按用户要求未执行。 |

验证结果：

- `cargo test -p feiwen --bin feiwen --locked`：通过，89 tests。
- 数据库临时目录 tests 覆盖成功备份重建、缺失 artifact 拒绝、WAL staging 失败恢复 live 数据库；未操作用户真实数据库。
- Query/Fetch tests 覆盖运行中第二次 start 丢弃、snapshot 续跑计算、catalog generation 与状态投影。
- `gpui-operation`、三个 Form crate、Jaco 与 Feiwen 的聚合 Clippy 严格门禁、workspace
  all-target/all-feature check 与 `git diff --check`：通过。
- Residual 扫描未发现 `FetchTaskState`、裸 `Db` Global、`next_id/group_id/node_id: u64` 或 `Retiring`；
  Query/Fetch 的业务数据库 ID 不在 form-only identity 范围内。
- 实际 UI 操作测试未执行；这不等同于通过 UI smoke。

## 完成与交接

Form `C-700`–`C-702` 与 Feiwen `C-703`–`C-705` 已达到当前实现 gate，代码状态为 `Done`。
实现已纳入本次 Issue #199 实施提交；实际 UI 操作测试被明确排除，PR 尚未请求。HTTP Client、Novel
Download、SQL/schema、抓取协议、资源与 bundle localization 保持 No change。
