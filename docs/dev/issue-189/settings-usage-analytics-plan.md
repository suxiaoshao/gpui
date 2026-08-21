# Issue #189 执行计划：设置页时间范围使用统计

> 后续替换说明（2026-08-21）：本计划仍是已实施的period selector、selected-range summary、provider/model bucket/query/label join、精简breakdown Table、page-local Operation、fresh-schema index及其证据的authority。finite daily LineChart及“年度heatmap为非目标”的部分，已由[设置页 Token 用量活动热力图计划](settings-usage-activity-heatmap-plan.md)定向替代；整个本计划不回写为`Superseded`，`WP-203`/`WP-503`的历史实施证据继续保留。

## 状态与范围

- 状态：`Implemented`
- 关联 issue：[#189](https://github.com/suxiaoshao/gpui/issues/189)
- Plan ID：`issue-189`
- 根计划：[Issue #189 总计划](README.md)
- 分支：`codex/189-jaco-show-context-usage`
- 受影响 owner：`crates/jaco-db`、`app/jaco`
- Release gate：无外部 release gate
- 最近证据刷新：2026-08-20
- 实施引用：当前分支工作树；implementation commit / PR `Pending`

### 目标

在 Jaco Settings 中新增 Usage 页面，以每一条 persisted `usage_events` 作为一次 completed provider request，按 Today、This week、This month、This year、All time 五个固定周期查询并展示：请求覆盖情况、六个 token 维度的精确合计、有限周期的本地日历每日趋势，以及按稳定 provider/model ID 分组的明细。查询、分组与覆盖率计算全部在 repository 边界完成；GPUI 只消费 typed snapshot。

### 范围

- `crates/jaco-db` 新增时间范围、aggregate、daily、provider/model bucket 与 snapshot 类型。
- `FreshRepository` 新增一次返回完整 analytics snapshot 的聚合查询。
- 在当前 fresh schema 中新增 `usage_events(created_at)` 索引；`SCHEMA_VERSION` 保持 `1`，不新增 migration 或自动升级路径。
- `app/jaco` 新增 Settings Usage 页面、周期选择器、refresh Operation、summary、daily line chart、provider/model table、loading/empty/error/degraded 状态。
- 新增 `en-US`、`zh-CN` Fluent 文案与 Settings 导航/search 入口。

### 非目标

- Agent 消息单次 request usage 与 composer context occupancy；它们继续由前两份执行文档负责。
- Cost、pricing、预算、quota、TTFT、TPS、latency、throughput 或 provider billing 对账。
- 自定义日期范围、最近 N 天、年度 heatmap、stacked 多指标图、导出或远程 telemetry。
- IANA 时区历史规则或 DST 历史重建；每次刷新只使用当时捕获的固定 `UtcOffset`。
- 按 conversation、project、prompt、run、user message 或 agent turn 分组。
- 把一轮对话的多个 provider steps 合并成一次请求；每条 usage event 都独立计数。
- 修改 `usage_events` 写入基数、既有 usage JSON、provider runtime 或 conversation live publication。
- 在 app/render 层加载全部 usage events 后聚合。

### 用户决定

- #189 在同一个 issue 中交付消息、composer 和 Settings 三个独立产品面，并分别维护可执行计划。
- Settings 提供 Today、This week、This month、This year、All time 五个固定周期。
- 默认周期为 This month，不持久化选择；每次打开新的 Settings 窗口重新使用 This month。
- 首次进入 Usage 页面及切换周期时自动查询；重新选择 Usage 页面可刷新。
- 查询失败提供 Retry；不增加常驻 Refresh 按钮、不轮询、不在窗口 focus 时自动刷新。
- 有限周期使用当前本地固定 offset 的日历边界；This week 从周一开始。
- 当前没有真实用户数据库，`SCHEMA_VERSION` 保持 `1`；fresh schema直接包含新索引，已有本地数据库由使用者自行重建或手工加索引，app不增加兼容/修复分支。

## 高影响变更摘要

| 审计门 | 结果 | 权威 IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | [Add] 仅 `jaco-db` 增加 analytics repository projection，`app/jaco` 增加 Settings 页面；不新增 crate | `D-34`、`C-21`、`WP-203`、`WP-503` |
| Public or cross-owner contracts | [Cross-owner] `jaco-db` 导出 `UsageAnalyticsRange` 与 `UsageAnalyticsSnapshot`，app 只读消费 | `C-21` |
| Global/shared authority | [Add] `UsageSettingsPage` Entity 是周期选择、active query 与 Operation 的唯一 owner；`SettingsView`维护actual active page lifecycle；不新增 Store/Global | `D-39`、`ST-21` |
| Persistence, data, configuration, or credentials | [Modify] 当前fresh schema增加单列index，schema version仍为1；不改usage rows/JSON、credential或用户配置，不实现旧库迁移 | `D-38`、`DB-23` |
| Runtime, concurrency, performance, or shutdown | [Modify] 单次 repository read transaction 执行 summary/daily/group queries；Operation-owned Task 负责取消 | `D-36`、`ST-21` |
| Security, privacy, or external access | No change；只聚合本地 normalized counts 与稳定 IDs，不访问网络、不展示 raw payload/secret | `D-45` |
| Dependencies, toolchains, generated, or vendored artifacts | None；复用 `time`、`gpui-operation`、`Select`、`LineChart`、`Table`、`Alert`、`Skeleton` | `D-46` |
| Platform, packaging, CI, or release | No packaging 变化；既有三平台 CI 仍是最终门 | `S-16` |
| User-visible compatibility, defaults, or removals | [Add] Settings 导航新增 Usage；默认 This month；旧 usage rows 自动进入统计，不回填或改写 | `D-40`、`R-41`–`R-56` |

## 适用性

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定或负面理由 | Owner / WP |
| --- | --- | --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Applicable | authority/query 在 DB，页面/状态在 Jaco | 只增加 `WP-203` 与 `WP-503`；core/conversation/agent 无第三 WP | `D-34` |
| `S-02` | GPUI components, layout, interaction, and accessibility | Applicable | Settings 已有 page frame；component checkout 提供 Select/LineChart/Table/Alert/Skeleton | 复用组件并提供 chart 的文字等价信息 | `D-42`–`D-44`、`WP-503` |
| `S-03` | Entity, Store, Global, identity, and projections | Applicable | Usage 只由一个 Settings page 消费 | 页面 Entity 直接拥有 Operation，不建 Store/Global 或第二份 snapshot cache | `D-39`、`ST-21` |
| `S-04` | Actions, events, subscriptions, focus, and windows | Applicable | 周期由 `SelectEvent::Confirm` 改变；Settings 页面有 navigation/search/DB/i18n transitions | Select 使用原生键鼠语义；actual active page统一解析，离开Usage或窗口关闭都取消Task | `ST-21`、`WP-503` |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Applicable | `SessionDatabaseExecutor::execute` 与 refresh Operation 已是 app 模式 | 每页至多一个查询 Task；切周期、离开Usage、DB unavailable或window close取消旧Task | `D-39`–`D-41`、`ST-21` |
| `S-06` | Data acquisition and Operation state | Applicable | 当前无 analytics state；Projects 已使用 refresh lifecycle | 明确 Idle/Loading/Ready/Refreshing/Unavailable/Retrying/Degraded 映射 | `ST-21`、`ERR-21` |
| `S-07` | Forms and editable state | N/A | 只有固定枚举选择，无草稿/save/validation | 不引入 Form 或 DatePicker | — |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | app 已直接依赖 jaco-db；usage 已归一化持久化 | 固定 `C-21`；不调用 provider/Rig/MCP/API | `C-21` |
| `S-09` | Error identity, propagation, recovery, and error UI | Applicable | DB、offset、calendar 与 stored-data 错误需要区分内部原因 | typed problem 保留 source，UI 只显示安全本地化文案并提供 Retry | `ERR-21`、`D-45` |
| `S-10` | Database, persistence, and migrations | Applicable | 当前fresh schema索引不支持全局 `created_at` range scan | 直接修改0001 fresh schema增加单列索引，schema version保持1且无migration；增加read transaction与query-plan test | `DB-21`–`DB-23`、`WP-203` |
| `S-11` | Generated, synchronized, copied, or vendored content | N/A | 全部目标是 handwritten Rust/SQL/Fluent/Markdown | 无生成文件或 vendor 内容 | — |
| `S-12` | Icons and assets | Applicable | app-local `ChartNoAxesColumn` 已存在 | Settings nav 复用 typed icon，不新增 SVG/asset | `D-42` |
| `S-13` | Fluent i18n and bundle localization | Applicable | Settings 文案集中在两份 `main.ftl` | 两 locale 同 key parity；不改 macOS bundle localization | `D-44`、`WP-503` |
| `S-14` | Security, privacy, and credentials | No change | query 只读 token counts、IDs 与当前显示标签 | 不记录 raw SQL values、prompt、response、metadata 或 secret | `D-45` |
| `S-15` | Observability and diagnostics | Applicable | 当前 executor 能传播 DB error | 内部错误按 period/range 分类记录；不记录逐 event 数据或新增 telemetry | `ERR-21` |
| `S-16` | Packaging, platform behavior, and CI/release | No change | 无 bundle resource 或平台分支 | 既有 macOS/Linux/Windows CI 为最终验证 | `R-56` |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | No change | 所需时间、Operation、chart/table API 已在依赖中 | 不改 Cargo manifests/features/lockfile | `D-46` |
| `S-18` | Owner documentation, indexes, and ADRs | Applicable | Settings 骨架与 owner 映射尚未完成 | 同步 root、DB、app owner plans/index；无需 ADR | `WP-203`、`WP-503` |
| `S-19` | Validation and completion evidence | Applicable | 行为跨schema/query/time/Operation/GPUI | `T-41`–`T-57` 覆盖自动化、人工与 CI | 全部 WPs |

## 实施前证据

### 实施前流程

1. `FreshRepository::complete_provider_step_with_usage` 在 provider step 完成事务中插入一条以 `provider_step_id` 唯一的 `usage_events`。
2. `usage_events` 已保存 conversation/provider/model identity、六个 normalized token columns、完整 `usage_json` 与 UTC `created_at`。
3. 当前 `date_key` 由 `now_utc().date()` 生成，表达 UTC 日期，不能回答本地日历 Today/This week/This month/This year。
4. repository 目前只提供按 provider step 与 conversation 读取 usage，没有全局范围 aggregate API。
5. Settings 当前有八个 page key；`DatabaseSettingsPages` 持有 database-backed page entities，并由 `SettingsView` 负责导航、DB readiness 与 render dispatch。
6. `database::ready_executor`/`SessionDatabaseExecutor::execute` 已提供离开 GPUI 线程执行 repository closure、并在 session draining 时拒绝新任务的边界。
7. `gpui_operation::refresh::Operation` 已表达 load/refresh/retry/cancel/degraded；`Select`、`LineChart`、simple `Table`、`Alert`、`Skeleton` 均在当前 gpui-component checkout 中可用。

### 证据登记

| E-ID | 分类 | 结论 | 证据 | 计划后果 |
| --- | --- | --- | --- | --- |
| `E-31` | Current fact | 每个 completed provider step 最多一条 usage event | `usage_events` unique provider-step index；`complete_provider_step_with_usage` | request unit 固定为 event，不按 turn/message 去重 |
| `E-32` | Current fact | 六个 token 字段与 `created_at` 已是窄查询所需的全部事实 | `crates/jaco-db/src/migrations.rs`、`records/agent.rs::UsageEventRecord` | 不解析 `usage_json` 或新增统计表 |
| `E-33` | Current fact | `date_key` 是 UTC 日期且现有 index 前导列是 conversation | `repository/agent.rs`、`idx_usage_events_conversation_date` | range filter 与 daily bucket 使用 `created_at`；增加单列索引 |
| `E-34` | Current fact | DB 没有 time-range aggregation；app 已直接依赖 jaco-db | `repository/agent.rs`、`app/jaco/Cargo.toml` | typed query projection留在 jaco-db，无 core WP |
| `E-35` | Current fact | Settings 页面/导航/DB overlay 均由 `SettingsView` 管理 | `features/settings.rs`、`features/settings/layout.rs` | Usage 作为独立 database-backed page entity接入 |
| `E-36` | Current fact | executor 已处理后台 DB work 与 draining | `database.rs::ready_executor`、`database/session.rs` | 不新增 runtime/channel |
| `E-37` | Current fact | refresh Operation 的 Task 字段是取消 authority | `gpui-operation/src/refresh.rs`、`state/projects.rs` | 页面不维护平行 loading/error bool |
| `E-38` | Upstream fact | `Select`、`LineChart`、simple `Table`、`Alert`、`Skeleton` 满足页面 | 当前 gpui-component checkout docs/source | 不新增 UI dependency 或自制通用组件 |
| `E-39` | Upstream fact | LineChart quantitative axis 只接受 `f64` 或 Decimal | current `plot::scale::sealed` | chart 使用 f64 visual projection，所有文字保持 u64 exact |
| `E-40` | Current fact | `time` 已在 app 启用 local-offset，DB 已启用 parsing/formatting | 两个 Cargo manifests | 不改 feature/lockfile |
| `E-41` | User/current fact | 当前schema version为1且没有真实用户；既有本地数据库可由使用者自行处理 | 用户决定；`SCHEMA_VERSION`/`CREATE_FRESH_SCHEMA_SQL` | 保持一个0001 fresh schema，不新增upgrade/compatibility path |
| `E-42` | Current fact | token columns 是 signed SQLite INTEGER，schema 没有 nonnegative CHECK | `usage_events` DDL 与 SQL row mapping | 聚合查询显式拒绝负数并 checked-convert |
| `E-43` | User/issue decision | 五个周期、本地日历、DB aggregate、summary/trend/breakdown 与状态已固定 | #189 正文及本轮确认 | 无待确认产品分叉 |

## 设计决定

| D-ID | 决定 | 依据 | 放弃的方案 | 后果 / owner |
| --- | --- | --- | --- | --- |
| `D-31` | analytics universe 是范围内全部 persisted usage events；一 event 等于一 completed provider request | `E-31`、#189 | 按 message/turn/run 去重；只统计最终成功 run | tool-loop 的每个 completed request独立计数 |
| `D-32` | 有限周期每次请求捕获一个 `now_utc` 与一个当前 fixed `UtcOffset`，从该快照计算本地日历边界 | `E-40`、用户决定 | UTC calendar、持续读取系统 offset、IANA/DST backfill | 一个 snapshot 的 filter 与 bucket 使用同一 offset |
| `D-33` | finite query 始终按 `created_at >= start_utc AND created_at < end_utc`；daily 也用同一 offset；忽略 `date_key` | `E-33` | `BETWEEN`、`date_key`、app 分桶 | 边界无重复，正/负/半小时 offset可测 |
| `D-34` | analytics types 定义在 `jaco-db::records::analytics`，不新增 jaco-core domain type | `E-34` | 为单一 SQL projection 扩展 core；app 自定义重复 DTO | DB 是唯一 producer，app 是唯一 consumer |
| `D-35` | reported = 六字段任一非零；unreported = 六字段全零；total-covered = `total_tokens > 0`；partial = reported - total-covered | #189、既有 coverage 语义 | metadata presence、从 details 重建 total、把 all-zero 当 empty | summary/daily/group使用同一 SQL predicate |
| `D-36` | summary、daily、provider/model 三类 query 在一个 read transaction 中完成；repository填充有限周期缺失日期并验证交叉合计 | snapshot consistency | 三次独立 connection query；app补日期/校验 | reload得到一个一致 typed snapshot |
| `D-37` | provider/model 按 event 中稳定 `(provider_id, model_id)` 分组；聚合后 left join当前显示标签，缺失/空标签由 app 回退 ID | 历史 identity 不能随 catalog label 改变 | 按显示名分组；要求 catalog 行存在 | 改名只影响标签，不合并历史 bucket |
| `D-38` | 在`CREATE_FRESH_SCHEMA_SQL`的0001中增加`idx_usage_events_created_at`；`SCHEMA_VERSION`保持1，`MIGRATIONS`仍只有0001 | `E-33`、`E-41` | 0002自动迁移、runtime schema修复、先加宽复合索引 | fresh DB直接有索引；已有本地DB由使用者重建或手工建索引 |
| `D-39` | `UsageSettingsPage` Entity 直接拥有 selected period、active query、Select state、refresh Operation 与 subscriptions；`SettingsView`维护actual active page lifecycle | `E-35`–`E-37` | Global/Store、detached Task、多份 loading/error state、render side effect | owner唯一；隐藏页面不保留running query |
| `D-40` | 默认 This month；direct initial/actual进入/reselect Usage 与周期改变自动查询；error仅提供Retry；无轮询/focus refresh/常驻Refresh | 用户决定、Alma参考 | 持久化周期、30秒轮询、focus revalidate | 新 request在下一次激活/切周期/Retry后出现 |
| `D-41` | period/range变化先取消旧Task再启动新查询；actual active page离开Usage时取消并清除active range；旧data只有与active exact range相等时才可作为stale data展示 | calendar boundary与offset可能在相同period名下变化 | 只比较enum period；让隐藏query完成；显示其他范围旧数据 | 旧结果不闪回或冒充当前周期 |
| `D-42` | 页面使用 header Select、一个summary section、finite LineChart section、provider/model Table section；导航复用 `ChartNoAxesColumn` | `E-38`、Settings layout | DatePicker、卡片堆、DataTable、定制icon | 保持既有Settings版式与组件语义 |
| `D-43` | daily chart仅画 `total_tokens`，按日期 dense；u64转f64只用于线条，不启用依赖精确浮点值的交互tooltip | `E-39`、#189 | stacked six-series、f64作为展示authority | summary/table/accessibility始终显示exact integer |
| `D-44` | 全部文案走 Fluent；Select/Alert/Button保留组件交互，app为Select focus wrapper、summary、status与Table文字补显式AX名称；chart容器暴露 image role与本地化文字摘要 | workspace UI/i18n规则与当前Label/Table AX边界 | hard-coded text、自制focus/keybindings、假设视觉Label会自动成为AX文字 | 键盘用户可操作period/retry，汇总与表格精确值可被读取 |
| `D-45` | internal problem保留 offset/calendar/DB source并记录 period/range；UI只显示安全本地化错误 | error/privacy边界 | 显示 raw SQL/row/payload；错误转零snapshot | empty与error不混淆，不泄露数据 |
| `D-46` | 不增加 dependency、Cargo feature、telemetry、bundle asset或macOS localization | `E-38`、`E-40` | 新数字/图表/日期库 | 复用当前 workspace 能力 |

## 目标设计

### 文件、模块与 owner 边界

```text
crates/jaco-db/
├── src/
│   ├── migrations.rs                         # F-221 [Modify] schema v1 fresh index
│   ├── records.rs                            # F-222 [Modify] analytics module/export
│   ├── records/analytics.rs                  # F-223 [Add] C-21 typed projection
│   ├── repository.rs                         # F-224 [Modify] analytics module
│   ├── repository/analytics.rs               # F-225 [Add] DB-21/DB-22 queries
│   ├── tests.rs                              # F-226 [Modify] analytics test module
│   └── tests/analytics.rs                    # F-227 [Add] range/aggregate/index tests
└── docs/dev/
    ├── README.md                              # F-228 [Modify] owner index
    └── issue-189/README.md                    # F-229 [Modify] WP-203

app/jaco/
├── src/
│   ├── features/settings.rs                  # F-531 [Modify] entity/nav/render/DB lifecycle
│   ├── features/settings/layout.rs           # F-532 [Modify] Usage page key
│   └── features/settings/usage.rs            # F-533 [Add] period/range/Operation/UI/tests
├── locales/
│   ├── en-US/main.ftl                        # F-534 [Modify] Usage keys
│   └── zh-CN/main.ftl                        # F-535 [Modify] parity keys
└── docs/dev/
    ├── README.md                              # F-536 [Modify] owner index
    └── issue-189/README.md                    # F-537 [Modify] WP-503
```

不修改 `crates/jaco-core`、`crates/jaco-conversation`、`crates/jaco-agent`、`usage_events` rows/JSON、Diesel `schema.rs`、Cargo manifests、`Cargo.lock`、runtime assets或bundle assets。SQLite index不进入Diesel table schema，因此 `schema.rs` 无变化。

### `C-21`：跨 owner analytics projection

Authority：`crates/jaco-db/src/records/analytics.rs`。Producer：`FreshRepository`。Consumer：`UsageSettingsPage`。这是 additive Rust API，不是 persisted/serialized contract。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsFiniteRange {
    start_utc: time::OffsetDateTime,
    end_utc: time::OffsetDateTime,
    local_offset: time::UtcOffset,
}

impl UsageAnalyticsFiniteRange {
    pub fn new(
        start_utc: time::OffsetDateTime,
        end_utc: time::OffsetDateTime,
        local_offset: time::UtcOffset,
    ) -> Option<Self>;
    pub fn start_utc(&self) -> time::OffsetDateTime;
    pub fn end_utc(&self) -> time::OffsetDateTime;
    pub fn local_offset(&self) -> time::UtcOffset;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageAnalyticsRange {
    Finite(UsageAnalyticsFiniteRange),
    AllTime,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageAnalyticsAggregate {
    pub request_count: u64,
    pub reported_request_count: u64,
    pub unreported_request_count: u64,
    pub total_covered_request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}

impl UsageAnalyticsAggregate {
    pub fn is_empty(&self) -> bool;
    pub fn partial_request_count(&self) -> Option<u64>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsDailyBucket {
    pub local_date: time::Date,
    pub aggregate: UsageAnalyticsAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsProviderModelBucket {
    pub provider_id: jaco_core::ProviderId,
    pub model_id: jaco_core::ProviderModelId,
    pub provider_label: Option<String>,
    pub model_label: Option<String>,
    pub aggregate: UsageAnalyticsAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsSnapshot {
    pub range: UsageAnalyticsRange,
    pub summary: UsageAnalyticsAggregate,
    pub daily: Vec<UsageAnalyticsDailyBucket>,
    pub provider_models: Vec<UsageAnalyticsProviderModelBucket>,
}

impl FreshRepository {
    pub fn usage_analytics(
        &self,
        range: UsageAnalyticsRange,
    ) -> jaco_db::Result<UsageAnalyticsSnapshot>;
}
```

`UsageAnalyticsFiniteRange::new` 先把两端规范为`UtcOffset::UTC`保存，只接受 `start_utc < end_utc`，并要求两端转换到`local_offset`后都是00:00的完整本地日历边界。字段可公开读取但不能绕过构造器制造空、反向或partial-day范围。`partial_request_count()` 使用 checked subtraction；repository产出的合法aggregate总是返回`Some`，手工构造的invalid aggregate不会发生underflow。

### 时间范围契约

app-local pure function：

```rust
fn usage_analytics_range(
    period: UsageAnalyticsPeriod,
    now_utc: OffsetDateTime,
    local_offset: Option<UtcOffset>,
) -> Result<UsageAnalyticsRange, UsageAnalyticsProblemSource>;
```

- `AllTime` 直接返回 `AllTime`，不读取或要求 local offset。
- finite request在一次 refresh context 中只捕获一次 `now_utc = OffsetDateTime::now_utc()`，并以同一 instant 调用 `UtcOffset::local_offset_at(now_utc)`；失败成为 typed problem，不回退 UTC。不得改用会在内部再次取当前时刻的 `UtcOffset::current_local_offset()`。
- `local_now = now_utc.to_offset(local_offset)`；所有 calendar arithmetic 只使用它的 local `Date`。
- local midnight以该 fixed offset `assume_offset` 后转回 UTC，构成 half-open range。

| Period | Local start | Local end |
| --- | --- | --- |
| Today | 当前 local date 00:00 | 下一 local date 00:00 |
| This week | 向前减 `weekday.number_days_from_monday()` 天后的周一 00:00 | start + 7 days |
| This month | 当前年月 1 日 00:00 | 下一月 1 日 00:00 |
| This year | 当前年 1 月 1 日 00:00 | 下一年 1 月 1 日 00:00 |
| All time | 无 start | 无 end |

所有 date/month/year arithmetic 使用 checked API；不可表示的边界成为 calendar problem。一个 snapshot 内即使系统 offset 随后变化，filter、daily bucket与显示range仍使用捕获值。

### `DB-21`：范围与 daily bucket SQL

finite query的唯一范围条件：

```sql
usage_events.created_at >= ?1
AND usage_events.created_at < ?2
```

All time省略 `WHERE`。禁止使用 `BETWEEN` 或 `date_key`。

start/end使用与`schema.rs`一致的`TimestamptzSqlite` bind传入`OffsetDateTime`，不把时间手工格式化进SQL字符串。

finite daily group key：

```sql
strftime(
    '%Y-%m-%d',
    usage_events.created_at,
    printf('%+d seconds', ?3)
)
```

`?3` 绑定 `local_offset.whole_seconds()` 的 SQLite Integer；不手工拼接 `+08:00` 字符串，因此正、负与非整小时 offset走同一路径。返回的 `YYYY-MM-DD` 使用既有 `time` parsing能力解析为 `Date`。SQL rows按 local date升序；repository从 local start遍历到 local end，补齐没有事件的零 bucket。`AllTime` 固定返回空 `daily`，页面不渲染趋势图。

### `DB-22`：覆盖率、合计与分组 SQL

summary、daily、provider/model query复用相同 predicate：

```sql
COUNT(*) AS request_count,
SUM(CASE WHEN
    input_tokens = 0
    AND output_tokens = 0
    AND cached_input_tokens = 0
    AND cache_write_input_tokens = 0
    AND reasoning_tokens = 0
    AND total_tokens = 0
    THEN 1 ELSE 0 END) AS unreported_request_count,
SUM(CASE WHEN
    input_tokens != 0
    OR output_tokens != 0
    OR cached_input_tokens != 0
    OR cache_write_input_tokens != 0
    OR reasoning_tokens != 0
    OR total_tokens != 0
    THEN 1 ELSE 0 END) AS reported_request_count,
SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END)
    AS total_covered_request_count
```

所有`SUM`只用`COALESCE(..., 0)`处理零匹配row产生的SQL NULL。六个 token columns分别 `SUM`；`total_tokens` 直接求和，禁止把input/output/cache/reasoning再次组合成total。query还计算任一token `< 0` 的 invalid row count；非零时返回 `DbError::Invariant`。SQLite integer sum overflow保留为 query error；所有 i64 count/sum 到 u64 使用 checked conversion，不能 saturate、wrap 或把错误变成零。

三类 query在同一 connection 的 read transaction 内执行：

1. summary一行；empty range返回全零aggregate。
2. finite daily按 local date分组；AllTime不执行daily query。
3. provider/model先按 `usage_events.provider_id, usage_events.model_id` 聚合，再 left join `providers` 与唯一 `(provider_id, model_id)` catalog row获取当前label。label不参与 `GROUP BY`。

provider/model rows固定排序：`total_tokens DESC, provider_id ASC, model_id ASC`。repository验证：

- `reported + unreported == request_count`。
- `total_covered <= reported`。
- finite daily各字段合计等于summary。
- provider/model各字段合计等于summary。

任一 invariant 不成立返回错误，不能向UI交付部分snapshot。现存 usage events 均进入范围：包括同一turn多个steps、最终run后来失败、soft-deleted/archived conversation仍保留的rows；物理级联删除后不存在的row不在universe中。

### `DB-23`：Fresh schema 与索引

```sql
CREATE INDEX idx_usage_events_conversation_date ON usage_events(conversation_id, date_key);
CREATE INDEX idx_usage_events_created_at ON usage_events(created_at);
CREATE UNIQUE INDEX idx_usage_events_provider_step ON usage_events(provider_step_id);
```

- 上述新单列index直接加入`CREATE_FRESH_SCHEMA_SQL`的usage index区；`SCHEMA_VERSION`保持1，`MIGRATIONS`仍只有`0001_create_fresh_schema`。
- 不新增`0002`、旧schema检测、自动建索引、backfill、repair或兼容错误。
- fresh数据库创建时直接得到目标index；现有本地数据库即使没有index，查询语义仍正确但可能全表扫描，由使用者自行删除重建数据库或手工执行同一`CREATE INDEX`。
- app启动不检查或改写现有数据库的index状态。
- `EXPLAIN QUERY PLAN` 对生产同形 finite range predicate断言 `SEARCH usage_events USING INDEX idx_usage_events_created_at`；SQLite为group/order建立临时B-tree不算失败。
- 初版不增加 `(created_at, provider_id, model_id)` 等宽索引；若实际 query plan/benchmark显示需要，另行用证据扩展。

### `ST-21`：状态、刷新与生命周期

app-local owner：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsageAnalyticsPeriod {
    Today,
    ThisWeek,
    ThisMonth,
    ThisYear,
    AllTime,
}

struct UsageAnalyticsData {
    period: UsageAnalyticsPeriod,
    snapshot: UsageAnalyticsSnapshot,
}

struct UsageAnalyticsProblem {
    period: UsageAnalyticsPeriod,
    range: Option<UsageAnalyticsRange>,
    source: UsageAnalyticsProblemSource,
}

type UsageAnalyticsOperation = refresh::Operation<
    UsageAnalyticsData,
    UsageAnalyticsProblem,
    Task<()>,
>;

struct UsageSettingsPage {
    selected_period: UsageAnalyticsPeriod,
    active_range: Option<UsageAnalyticsRange>,
    period_select: Entity<SelectState<Vec<UsagePeriodOption>>>,
    operation: UsageAnalyticsOperation,
    _subscriptions: Vec<Subscription>,
}
```

`selected_period` 是产品选择 authority；Select只是本地化interaction projection。`active_range` 是本次请求identity，用于防止相同period名下跨日/月/offset变化时展示旧范围。snapshot只存在于 Operation data中。

`SettingsView` 另外保存 `active_page: Option<SettingsPageKey>`，它表示搜索过滤后当前实际渲染的页面，并由唯一的非render transition helper维护；`selected_page`仍是用户导航选择。helper用同一套pure resolver计算initial、导航、搜索和i18n变化后的actual active page，避免页面生命周期依赖render side effect。

Lifecycle：

1. `UsageSettingsPage::new` 建立 This month select与Idle operation，不在用户进入页面前查询。
2. `SettingsView::new_with_page` 完成页面实体和数据库页面初始化后，显式执行一次actual-active-page transition；因此直接以Usage为initial target且DB已经exact Ready时会立即`activate`，不等待render、重复点击或DB状态变化。
3. 导航选择统一经过`select_page`，搜索`InputEvent`与可能改变搜索匹配结果的i18n更新也经过同一transition helper。actual active page从非Usage进入Usage且DB exact Ready时调用`activate`；每次明确重新选择Usage也强制`activate`；running时不并发。
4. actual active page从Usage离开时调用`deactivate`：向running Operation发送`Cancel`并清除`active_range`；已settled data仍留在Operation中，仅供以后重新进入且新range完全相同时作为stale data。页面Entity即使仍被`DatabaseSettingsPages`持有，也不会让隐藏查询继续发布UI更新。
5. `activate` 为当前period捕获一次 `now_utc`，finite range以`local_offset_at(now_utc)`取得同一instant的offset；Idle发`Load`，Ready/Degraded发`Refresh`，Unavailable发`Retry`。AllTime不调用offset API。
6. `SelectEvent::Confirm` 只在值实际改变时更新authority；先向running operation发`Cancel`，再建立新range并开始请求。
7. query通过当前 `SessionDatabaseExecutor` 执行；Task只持有weak page entity，完成后在UI线程发送`Complete(result)`并notify。
8. DB离开exact Ready时调用同一取消路径并清除active range；外层既有database resource view负责整页不可用。DB恢复且actual active page为Usage时重新activate。
9. window/page entity释放会drop Operation-owned Task；没有detached task、generation timer、polling或usage-event subscription。
10. I18n变化重建localized select items，并按`selected_period`重新选中；不会发业务Confirm。只有搜索匹配变化导致actual active page发生转换时，才通过统一helper执行对应activate/deactivate。

render只消费已解析并保存的`active_page`和Operation状态；不得在render中调用resolver后启动Task，也不得用render次数补偿缺失的初始化或导航事件。

Render state：

| Operation / range relation | UI |
| --- | --- |
| Idle（只可能尚未激活） | 不渲染旧数据；进入页面立即触发load |
| Loading / Retrying，无匹配data | Skeleton + loading文案 |
| Ready，`request_count == 0` | 明确empty state；不渲染零值dashboard |
| Ready，有events | summary + finite trend + breakdown |
| Refreshing，旧data range与active range完全相同 | 保留data并显示小型refreshing状态 |
| Degraded，旧data range与active range完全相同 | 保留stale data，显示warning + Retry |
| Refreshing/Degraded，旧data range与active range不同 | 隐藏旧data；显示loading或blocking error |
| Unavailable | error Alert + Retry |

全零usage event仍使 `request_count > 0`，因此属于Ready dashboard中的unreported数据，不属于empty。

### GPUI 设置页与数据展示

- `SettingsPageKey::Usage` 插在 Provider 后；page spec数组从8增至9。
- navigation：标题 `settings-page-usage`，icon `IconName::ChartNoAxesColumn`，search terms覆盖 usage/token/statistics/analytics/request/provider/model/cache 及中文/拼音。
- 继续使用默认 `SettingsPageFrame` 外层vertical scroll与960px内容宽度，不使用 `no_outer_body_scroll()`。
- header说明统计单位为completed provider request；右侧放small `Select`，固定五个option，不可clear、不启用search。
- summary使用一个 `GroupBox`，内部以可换行grid展示 request、reported、unreported、total-covered counts，以及input/output/cache read/cache write/reasoning/total六个exact token合计。
- 数字复用 `foundation::conversation_format::format_token_count` 的逗号分组exact formatter；不使用message action-row的compact `k/M` formatter。
- finite trend使用 `LineChart::new(daily).x(localized_date).y(total_tokens_as_f64).linear()`；Today/Week/Month启用`.dot()`，This year不画逐日dot；四者tick margin固定为1/1/5/30。line chart不设置interactive `.id()`/tooltip，避免把f64近似显示成权威整数。
- chart容器有稳定debug ID、240px高度、localized标题/说明与role Image accessibility label；summary exact total和table是等价文字信息。
- AllTime隐藏trend section。
- breakdown使用compact simple `Table`，外包horizontal scroll；列按 Model、Provider、Requests、Input、Output、Cache read、Cache write 排列。Reported、Unreported、Total covered、Reasoning、Total 仍由summary保留，不在明细表重复展示。
- provider/model label经trim后为空或missing时显示稳定ID；不把label作为row key。顺序直接使用repository结果。

### 本地化与可访问性

两份locale保持完全相同key集合，至少包括：

```text
settings-page-usage
settings-usage-description
settings-usage-period-label
settings-usage-period-today
settings-usage-period-this-week
settings-usage-period-this-month
settings-usage-period-this-year
settings-usage-period-all-time
settings-usage-loading
settings-usage-refreshing
settings-usage-load-error-title
settings-usage-load-error-description
settings-usage-refresh-error-description
settings-usage-retry
settings-usage-empty-title
settings-usage-empty-description
settings-usage-summary-title
settings-usage-summary-accessible
settings-usage-metric-accessible
settings-usage-requests
settings-usage-reported-requests
settings-usage-unreported-requests
settings-usage-total-covered-requests
settings-usage-input-tokens
settings-usage-output-tokens
settings-usage-cached-input-tokens
settings-usage-cache-write-input-tokens
settings-usage-reasoning-tokens
settings-usage-total-tokens
settings-usage-trend-title
settings-usage-trend-description
settings-usage-trend-accessible
settings-usage-date-value
settings-usage-breakdown-title
settings-usage-provider
settings-usage-model
```

`settings-usage-trend-accessible` 使用range、total、covered、requests变量形成简短文字摘要；不逐点朗读366个值。Select与Retry Button沿组件原生Tab/Enter/Space/focus语义；Select外层以同一个focus handle暴露ComboBox label/value。summary group包含十个本地化exact指标，status提供同文案aria label，Table每个header/cell提供唯一ID与显式可访问名称；不注册app-local keybindings。

### `ERR-21`：错误、空状态与诊断

```rust
enum UsageAnalyticsProblemSource {
    LocalOffset(time::error::IndeterminateOffset),
    CalendarRange,
    Database(jaco_db::DbError),
}
```

`UsageAnalyticsProblem` 实现 `Display` 与 `std::error::Error` 以满足refresh Operation；UI不直接渲染其`Display`。

| Source | Operation result | User UI | Diagnostic |
| --- | --- | --- | --- |
| local offset unavailable | Unavailable或Degraded | 本地化load/refresh error + Retry | period；不伪造UTC range |
| date boundary不可表示 | Unavailable或Degraded | 同上 | period + calendar category |
| executor unavailable/draining | 由Settings DB overlay或DB error处理 | resource unavailable或Retry | session state，不含secret |
| Diesel/query/overflow | Unavailable或Degraded | 通用安全错误 | period + finite bounds/all-time + error chain |
| negative token / aggregate invariant | Unavailable或Degraded | 通用安全错误 | invariant category；不记录逐row token |
| zero matched events | Ready empty | empty title/description | 无error log |

UI不显示 `DbError::to_string()`、SQL、provider payload、prompt、response、credential或逐event数值。Retry重新捕获当前now/offset，不复用失败的range。

## Requirements

| R-ID | Requirement |
| --- | --- |
| `R-41` | 默认This month且不持久化；五个period顺序与文案固定 |
| `R-42` | request unit为每条usage event；一个turn多个provider steps分别计数 |
| `R-43` | finite范围只捕获一次`now_utc`并以`local_offset_at(now_utc)`取得同一instant的fixed offset；使用Monday week和UTC half-open bounds |
| `R-44` | filter/daily忽略UTC `date_key`，正负与非整小时offset使用同一路径 |
| `R-45` | reported/unreported/total-covered/partial与六个token sum严格按`D-35`，total不重建 |
| `R-46` | summary/daily/provider-model在一个read transaction内生成并通过cross-total invariants |
| `R-47` | provider/model按稳定IDs分组，label只用于显示，缺失时回退ID，排序deterministic |
| `R-48` | finite daily dense补零；AllTime不返回或渲染daily trend |
| `R-49` | negative/overflow/invalid range明确失败，不saturate、不wrap、不转empty |
| `R-50` | fresh schema直接包含`idx_usage_events_created_at`且schema version保持1；无0002、旧库检测、自动repair或backfill |
| `R-51` | app/render不加载全量events、不聚合、不解析usage JSON |
| `R-52` | Usage page只有一个Operation-owned Task；切周期、actual active page离开Usage、DB unavailable或window close都取消旧task |
| `R-53` | `new_with_page`直接打开、actual active page进入Usage、明确reselect、period change与Retry触发查询；render不启动查询，且无轮询/focus refresh/常驻Refresh |
| `R-54` | loading/empty/ready/refreshing/unavailable/degraded以及range mismatch均有确定UI |
| `R-55` | summary/table保持exact integers；chart的f64只作视觉投影且有文字等价信息 |
| `R-56` | nav/search/existing typed icon、两locale parity、键盘操作与accessibility有测试；Cargo/lock/assets无无关diff |

## 工作包

### `WP-203` — jaco-db analytics projection、fresh index 与 queries（Implemented）

1. 增加 `F-223` 的 `C-21` 类型、constructor/getters与aggregate helpers，并从 `records.rs` 导出。
2. 在 `F-221` 的0001 fresh schema增加单列created-at index，保持schema version 1与单一migration；不实现旧库升级。
3. 在 `F-225` 实现一个read transaction中的summary/daily/provider-model queries、dense date fill、label join与invariant validation。
4. 增加`DB-21`–`DB-23`的range、coverage、overflow/corruption、determinism、fresh-schema index与query-plan tests。
5. 确认 `schema.rs`、usage JSON、Cargo manifests与`Cargo.lock`无diff。

依赖：现有 `usage_events` authority。解锁 `WP-503`。

### `WP-503` — Jaco Settings Usage page 与 Operation（Implemented）

1. 增加 `SettingsPageKey::Usage`、第九个page spec、导航icon、search terms、database-backed page entity与render dispatch。
2. 在 `usage.rs` 实现period enum、pure range builder、localized Select projection、Operation owner与DB executor query。
3. 实现`ST-21`的activate/period switch/cancel/DB readiness/i18n/range-match lifecycle。
4. 用现有组件实现summary、finite daily LineChart、provider/model Table、Alert/Skeleton/Retry及exact formatting。
5. 增加两locale keys、nav/icon/i18n/range/operation/projection/GPUI layout tests与人工场景证据。

依赖：`WP-203` 的稳定 `C-21`。

### 执行顺序

```text
WP-203 -> WP-503 -> focused validation -> manual Settings matrix -> workspace/CI gates
```

两者的文档可同时细化；生产实现按依赖顺序进行，避免app临时定义第二套projection。

## 测试

| T-ID | Owner | Proposed test / 覆盖 |
| --- | --- | --- |
| `T-41` | db | finite range constructor拒绝equal/reversed/misaligned bounds；AllTime合法 |
| `T-42` | app | Today与正/负/半小时offset跨UTC日边界；finite activation以同一`now_utc`解析`local_offset_at` |
| `T-43` | app | Monday week、month/year rollover与leap day的half-open范围 |
| `T-44` | db | start inclusive、end exclusive；故意不一致的`date_key`不影响filter/bucket |
| `T-45` | db | 同一turn两个completed provider steps计为两个requests；run最终failed仍按events统计 |
| `T-46` | db | all-zero、partial、total-covered三类coverage counts与六字段独立sum |
| `T-47` | db | cache字段不二次加入total；large合法integer保持exact |
| `T-48` | db | negative token/invariant与SQLite sum overflow返回error，绝不返回zero snapshot |
| `T-49` | db | finite daily按捕获offset分桶、顺序稳定并补齐zero days；AllTime daily empty |
| `T-50` | db | provider/model稳定ID不串组；rename/missing label不改identity且fallback-ready |
| `T-51` | db | summary/daily/group cross-total相等；empty range为合法zero snapshot |
| `T-52` | db | fresh DB保持schema version1与一个0001 migration，并创建`idx_usage_events_created_at`；无upgrade path |
| `T-53` | db | EXPLAIN finite query使用`idx_usage_events_created_at` |
| `T-54` | app | default month、period order、nav第九项、icon、英文/中文/拼音search与locale parity |
| `T-55` | app | DB已Ready时direct initial Usage激活、search-derived进入、reselect refresh、period switch cancel、导航/搜索离开取消、running dedupe、Retry与DB unavailable/recovery lifecycle |
| `T-56` | app | exact-range stale data可见，mismatched-range旧数据隐藏；all-zero不是empty |
| `T-57` | app GPUI | summary exact values、AllTime无chart、finite chart、horizontal table、loading/empty/error/degraded与AX label |

## 聚焦验证

实施时按本轮直接改动执行一次最小充分验证：

```sh
cargo fmt
cargo test -p jaco-db usage_analytics
cargo test -p jaco-db bootstrap
cargo test -p jaco features::settings::usage::tests
cargo test -p jaco features::settings::tests
cargo test -p jaco i18n
cargo check -p jaco
cargo clippy -p jaco-db -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

文档阶段只执行：

```sh
git diff --check
```

### 人工场景

1. 打开Settings；进入Usage时默认This month并出现loading后正常数据。
2. 切换Today/This week/This month/This year/All time；有限周期显示趋势，All time不留空图框。
3. 有数据时核对summary exact totals、coverage counts与provider/model rows；长数字不压缩为`k/M`。
4. 空数据库显示empty；只有all-zero usage event时显示Ready且unreported=1。
5. 查询过程中快速切period，旧范围数据不闪回；离开/关闭窗口无后台UI更新。
6. 模拟DB error后看到blocking error或same-range stale warning与Retry；恢复后Retry成功。
7. 英文/中文切换后Select、表头、状态与chart summary全部更新；键盘可操作Select与Retry。
8. 新完成一次provider request后重新选择Usage或切周期，request/tokens增加一条event对应的值。

## 完成条件

- `R-41`–`R-56` 与 `T-41`–`T-57` 全部有实现和证据。
- `WP-203`、`WP-503` 状态更新为 `Implemented`，并记录实际files/commands/results。
- fresh DB在schema version1下直接拥有created-at index；实现中不存在0002、旧库检测、自动repair或backfill。
- Settings UI没有全事件加载、app-side aggregation、轮询、常驻refresh、cost或custom range。
- 消息request usage与composer context occupancy的行为和测试无回归。
- root/owner计划与索引同步；最终workspace gates、人工矩阵和三平台CI有明确结果或未验证说明。

## 完成证据

| 证据 | 当前结果 |
| --- | --- |
| `WP-203` | `Implemented`；新增 DB-owned range/snapshot contract、同一 read transaction 的 summary/daily/provider-model 聚合、schema version 1 fresh created-at index、dense dates、label fallback-ready projection与invariant checks |
| `WP-503` | `Implemented`；新增第九个 Settings Usage 页面、五周期范围、page-local refresh Operation、actual-active/DB readiness lifecycle、summary/trend/table、完整状态、双语与显式 AX 文本 |
| Automated validation | `cargo test -p jaco-db usage_analytics` 11 passed；`cargo test -p jaco-db bootstrap` 5 passed；Usage module 15 passed（含实际 Entity/Window 的 single-flight、cancel、stale completion 与compact breakdown列顺序）；Settings module 16 passed；i18n 11 passed；`cargo check -p jaco`、两 package strict clippy、`cargo fmt` 与 `git diff --check` 通过 |
| Manual Settings matrix | `Partial`；当前本地bundle + 隔离 `JACO_CONFIG_DIR` 已验证fresh empty、reselect刷新、五个period、keyboard Select、finite trend、AllTime无trend、exact summary/provider-model rows、AX tree以及search-derived进入/离开；横向表格滚动、error/degraded、运行中快速切换和现场语言切换仍待验证 |
| Workspace / three-platform CI | `Pending` |
| Bundle / implementation commit / PR | `cargo run -p xtask -- bundle jaco`通过；`actool`因本机CoreSimulator不可用而跳过Liquid Glass图标注入并保留普通图标；commit / PR `Pending` |

## 执行交接审计

- [x] 目标、范围、非目标与用户决定完整。
- [x] `S-01`–`S-19` 全部判定。
- [x] 当前证据、权威来源与 owner 边界已核对。
- [x] cross-owner type、repository method、SQL filter/group/order/invariants已固定。
- [x] fresh index、schema version 1与明确的无upgrade行为已固定。
- [x] period/default/refresh/cancel/stale/error/empty状态已固定。
- [x] GPUI component、layout、exactness、i18n与accessibility已固定。
- [x] requirements、tests、work packages、validation与done conditions可追踪。
- [x] 无剩余用户确认问题；计划可直接实施。
