# Issue #189 执行计划：设置页 Token 用量活动热力图

## 状态与范围

- 状态：`Implemented`
- 关联 issue：[#189](https://github.com/suxiaoshao/gpui/issues/189)
- 父 issue：[#159](https://github.com/suxiaoshao/gpui/issues/159)
- Plan ID：`issue-189-settings-usage-activity-heatmap`
- Root hub：[Issue #189](README.md)
- 前置实施记录：[Settings usage analytics](settings-usage-analytics-plan.md)
- 分支：`codex/189-jaco-show-context-usage`
- 最近更新：2026-08-21
- 执行状态：热力图已替换每日折线图，精简provider/model Table已保留；`WP-601`、`WP-204`与`WP-504`均已实施，人工主题/滚动/AX矩阵与workspace-wide gates待最终验收

### 目标

在 Jaco Settings Usage 页面中，用独立 `gpui-heatmap` crate 提供的活动热力图替换有限周期的“每日总 Token”折线图，并保留已简化的provider/model明细表。热力图固定显示包含今天在内的最近365个本地日历日，以每日`total_tokens`着色；现有周期选择器继续控制selected summary与provider/model Table。

页面的一次刷新必须在同一个数据库 read transaction 与同一个 page-local refresh Operation 中取得两种语义不同但时间一致的数据：

1. 用户所选 Today、This week、This month、This year 或 All time 的 summary/provider-model breakdown；
2. 固定最近 365 个本地日历日的 activity summary/daily。

### 范围

- `crates/gpui-heatmap` 定义产品无关的连续日期序列、活动热力图组件、日期网格、0 + 4 个非零色阶、`gpui-component` Plot tooltip、主题、图例、整图 accessibility summary 与组件测试。
- `crates/jaco-db` 将现有analytics query改为selected range + activity range的单次typed query，保留selected provider/model聚合，删除失去consumer的selected daily snapshot，并在一个read transaction中返回经过交叉校验的selected与activity投影。
- `app/jaco` 计算固定本地offset下的selected range与最近365天activity range，继续使用一个Operation-owned Task，移除LineChart并接入热力图，同时保留精简breakdown Table、双语、空状态、accessibility与生命周期测试。
- workspace manifests 将 `gpui-heatmap` 暴露为 workspace dependency，并让 Jaco 消费；组件 crate 增加精确版本的 `time = "0.3.54"`，不增加新的外部包。

### 非目标

- 费用、定价、预算、quota、账单、provider/model 费用排名或价格历史。
- 自定义活动日期范围、跟随周期缩短热力图、多个 heatmap 指标、输入/输出/缓存分层着色。
- 单日点击、选择、钻取、跳转到 conversation、拖拽、缩放或导出。
- 为 365 个日期创建 FocusHandle、Tab stop、Entity、Task、timer、subscription 或 app-local hover state。
- 在组件 crate 中读取系统时区、Fluent、Jaco repository、usage event 或业务 period。
- 新 schema、migration、schema version、旧库兼容层或 usage event 回填。
- 本轮费用统计；现有 provider/model identity 仍保留在 `usage_events`，后续费用设计另行决定价格来源与价格版本快照。

### 已确认的用户决定

- 热力图固定显示最近 365 个本地日历日并包含今天，不跟随 Settings 周期选择器。
- 热力图替换“每日总 Token”折线图；精简provider/model Table继续保留。
- 周期选择器继续控制selected summary与provider/model Table，热力图不跟随周期变化。
- 每个有效日期 cell 的 pointer tooltip 只显示本地化日期和精确总 Token；详情不做 `k`/`M` 压缩。
- 热力图只提供整图 accessibility summary，不为 365 个日期创建独立键盘焦点或键盘打开路径。
- 周一为每周第一行；首尾不足一周使用不可交互 padding cell。
- 色阶为零值 + 四个非零等级，按当前 365 天最大值进行确定性线性归一化；不使用最小值或对数变换。
- cell 维持固定可读尺寸；窄页面使用横向滚动，不继续压缩格子。
- 组件复用 `gpui-component` 的 Plot tooltip、theme、size 与 scroll 约定，不新增热力图或数字格式化库。
- 调用方负责 fixed offset、本地日期、Fluent 文案、日期与精确数值格式；组件不持有 Jaco 语义。

## 高影响变更摘要

| Surface | 变更 | Authority / work package |
| --- | --- | --- |
| Workspace/crate topology | [Modify] 已建立的 `gpui-heatmap` workspace member从骨架进入稳定公开组件；Jaco增加workspace path dependency | `C-23`、`WP-601`、`WP-504` |
| Public workspace API | [Modify] `jaco-db` analytics query/snapshot改为selected + activity；保留provider/model bucket，删除selected daily snapshot | `C-22`、`WP-204` |
| Database runtime | [Modify] 一个deferred read transaction内执行selected summary/provider-model与activity summary/daily | `DB-31`–`DB-33`、`WP-204` |
| GPUI state/lifecycle | [Modify] `UsageSettingsPage` 的exact query identity包含两个range；仍由一个refresh Operation拥有唯一Task | `ST-61`、`WP-504` |
| UI/accessibility | [Modify] 有限周期LineChart替换为固定365天活动热力图；精简provider/model Table保留；pointer tooltip + whole-chart image summary | `HM-61`–`HM-63`、`WP-601`、`WP-504` |
| i18n | [Modify] 两个Jaco Fluent locale增加activity、legend、caption、month和empty keys，删除失去consumer的trend keys并保留breakdown keys | `L-61`、`WP-504` |
| Dependencies | [Modify] 无新外部package；`gpui-heatmap`直接使用已解析的`time 0.3.54`，Jaco增加workspace path dependency | `D-76`、`WP-601`、`WP-504` |
| Schema/migration | None | `D-68` |

## 适用性矩阵

| ID | Surface | 适用性 | 当前证据 | 计划结论 |
| --- | --- | --- | --- | --- |
| `S-01` | Workspace、文件、模块和owner边界 | Applicable | crate骨架已存在；DB与app已有owner plans | 三个owner分别执行`WP-601`、`WP-204`、`WP-504` |
| `S-02` | GPUI组件、布局、交互和accessibility | Applicable | 当前gpui-component无heatmap；Plot/Tooltip/GroupBox/scroll/theme可复用 | 自定义最小日期网格，复用组件库交互和主题 |
| `S-03` | Entity、Store、Global与状态权威 | Applicable | Usage page已有独立Entity和refresh Operation | 热力图保持RenderOnce/Plot值组件，不新增状态authority |
| `S-04` | Actions、events、focus与window | Applicable | 需求只有pointer hover | 不注册action/keybinding/FocusHandle；整图非Tab stop |
| `S-05` | Async、Task、取消和过期completion | Applicable | 当前page以Operation-owned Task查询 | query identity扩展后继续single-flight/cancel/stale拒绝 |
| `S-06` | Error、empty、degraded和retry | Applicable | 当前Operation已覆盖完整phase | 两个投影作为一个result成功或失败，不做partial fallback |
| `S-07` | 数据模型与跨owner契约 | Applicable | 当前snapshot只有selected range和provider_models | 以`C-22`和`C-23`固定typed boundary |
| `S-08` | Provider/runtime API | No change | authority仍为normalized usage_events | 不改provider parsing或usage capture |
| `S-09` | Serialization/compatibility | No change | analytics类型不序列化持久化 | workspace内部breaking API原子迁移，不设兼容层 |
| `S-10` | Database、persistence和migration | Applicable | created_at索引与dense daily query已存在 | 复用schema/index；替换projection/SQL/tests |
| `S-11` | Security、privacy和redaction | No change | 只显示本地聚合token与日期 | 不暴露prompt、response、provider payload或credentials |
| `S-12` | Icons/assets | No change | 热力图由绘制与主题token构成 | 不新增SVG、runtime asset或bundle asset |
| `S-13` | Fluent与bundle localization | Applicable | Jaco运行时文案使用两份main.ftl | app生成所有labels/tooltip/caption/AX文本；crate不依赖Fluent |
| `S-14` | Packaging、platform与window sizing | Applicable | Settings内容宽度有限 | 固定cell + horizontal scrollbar；不改bundle配置 |
| `S-15` | Observability/telemetry | No change | 当前只在query error记录结构化日志 | 不记录hover或usage telemetry |
| `S-16` | Generated/vendor artifacts | No change | 无codegen/vendor | Cargo.lock仅允许正常workspace package/dependency解析变化 |
| `S-17` | Dependencies/framework/toolchain | Applicable | time、GPUI、gpui-component均已解析 | 不引入新库；直接复用锁定API |
| `S-18` | Owner docs/index/ADR | Applicable | root和三个owner已有计划入口 | 同步root execution plan与owner扩展；无需ADR |
| `S-19` | 测试、人工验证和CI | Applicable | DB/app已有focused harness；新crate为空 | 纯逻辑、GPUI window、DB、app lifecycle、人工主题/滚动矩阵 |

## 实施前证据

### 当前代码事实

1. `UsageAnalyticsDailyBucket` 已提供 `time::Date + UsageAnalyticsAggregate`，`total_tokens` 是精确 `u64`。
2. `load_daily` 已按调用方提供的fixed offset分桶，并为范围内缺失日期生成零aggregate。
3. 前置`UsageAnalyticsSnapshot`同时包含selected summary/daily与provider/model buckets；折线图是selected daily的唯一consumer，Table是provider/model projection的consumer。
4. 当前 Settings `ThisYear` 是本地自然年，不能表达rolling 365 days；AllTime路径当前不获取local offset。
5. `UsageSettingsPage` 已有一个 `refresh::Operation<UsageAnalyticsData, UsageAnalyticsProblem, Task<()>>`，离开页面、DB失效和Entity/window drop会取消其Task。
6. 当前 `render_data` 在selected summary为空时隐藏整个dashboard；接入固定activity后必须区分selected empty与global empty。
7. 当前gpui-component checkout `57a9903` 没有heatmap/calendar-activity组件；`Plot` + `#[derive(IntoPlot)]`提供stable-id pointer tooltip，`Tooltip`使用组件主题。`ScrollableElement::overflow_x_scrollbar()`的状态id来自source callsite；可复用组件需使用caller id派生的keyed `ScrollHandle`，再显式渲染可指定id的`gpui_component::scroll::Scrollbar`。
8. `ActiveTheme` 暴露`secondary`、`chart_1`、`border`、`muted_foreground`等语义token；无需维护独立亮/暗色板。
9. `format_token_count` 已在Jaco以整数算法输出精确千分位值；activity caption、tooltip和AX可直接复用。
10. provider/model identity持久化在`usage_events`，当前精简Table仍需要该聚合投影；费用设计仍需独立的价格来源与版本快照。

### 证据登记

| ID | 类型 | 事实 | 来源 | 设计影响 |
| --- | --- | --- | --- | --- |
| `E-61` | Local source | daily bucket已是本地Date和exact aggregate | `crates/jaco-db/src/records/analytics.rs` | DB到app无需传原始events |
| `E-62` | Local source | finite daily query dense补零并验证sum | `crates/jaco-db/src/repository/analytics.rs` | activity复用同一assembler |
| `E-63` | Upstream source | checkout无heatmap；Plot支持stable-id tooltip；Scrollbar可显式设id，Scrollable convenience wrapper使用caller id | gpui-component `plot::{Plot, tooltip}` / `scroll::{ScrollableElement, Scrollbar}` | `gpui-heatmap`补二维网格，并用component id隔离滚动状态 |
| `E-64` | Upstream source | `ActiveTheme`提供semantic/chart tokens | gpui-component `theme` | 色阶每次render/paint从主题解析 |
| `E-65` | Local source | page已有Operation-owned Task与exact-range stale guard | `app/jaco/src/features/settings/usage.rs` | 扩展identity，不创建第二Operation |
| `E-66` | Local source | `ThisYear`为自然年，AllTime不取offset | `usage_range_for_offset` / `current_usage_range` | 新建一次捕获offset的query builder |
| `E-67` | Local source | LineChart是selected daily唯一app consumer；Table仍消费provider/model projection | `render_trend`、`render_breakdown`及analytics imports/tests | 删除LineChart/selected daily，保留Table/provider-model projection |
| `E-68` | Local source | exact formatter无外部dependency | `foundation/conversation_format.rs` | tooltip/caption保持完整整数 |

## 设计决定

| ID | 决定 | 理由 | 排除方案 | 可验证结果 |
| --- | --- | --- | --- | --- |
| `D-61` | Jaco activity固定为本地今天向前364天到明日本地午夜的UTC半开区间 | 精确表达“包含今天的最近365个本地日历日” | 当前自然年、最近12个月、跟随period | leap day和跨年均恰好365个bucket |
| `D-62` | 一次捕获`now_utc`与`local_offset_at(now_utc)`，selected和activity共用该offset | 避免一个snapshot出现两个日历视图 | 分别读取系统offset | query identity可重放、跨午夜可识别 |
| `D-63` | `C-22`包含selected range与activity range；repository在一个read transaction内完成两个projection | 保持snapshot consistency | 两个Task/两个connection/后补activity | 页面只处理一个成功或失败结果 |
| `D-64` | 删除selected daily snapshot与finite selected daily查询；保留provider/model bucket、SQL、label join与精简Table | 热力图替代每日趋势后selected daily失去consumer，Table仍提供按模型明细 | 同时保留LineChart；删除Table | selected只查询summary/provider-model，activity查询summary/daily |
| `D-65` | component输入是“start_date + 连续u64 values”，不接受任意date-value pairs | 在类型层消除排序、重复和缺口歧义 | HashMap、可重复Vec<(Date,u64)> | index与日期一一对应 |
| `D-66` | 组件支持任意非空连续范围；Jaco consumer负责精确365天 | crate保持产品无关且可复用 | crate硬编码365/系统时区 | 组件单测可覆盖短序列，app测试覆盖365 |
| `D-67` | 周一为默认week start；Jaco不覆盖；首尾padding不可hover | 与Settings周范围一致 | locale自动改变week start | row 0恒为Monday，padding命中返回None |
| `D-68` | 不改schema/version/migration，不添加兼容层 | authority和索引已足够 | activity cache table或旧snapshot adapter | fresh/current DB使用相同数据 |
| `D-69` | 0为独立等级；正值按`ceil(value/max(max, 1)*4)`映射1..4 | 与Alma语义一致，计算简单且当前Token分布保持可读差异 | 对数、分位数、固定token阈值 | 全零、四分位边界、单点、u64::MAX无panic且等级稳定 |
| `D-70` | authority、文本与视觉等级都保留整数语义；level计算临时扩展为u128 | 避免展示精度丢失与乘4溢出 | 把chart f64当显示值 | tooltip/caption/AX逐字保留整数，tier边界用整数测试 |
| `D-71` | default Medium为12px cell/3px gap；custom cell限制4–32px；窄宽度使用`(component_id, "scroll-state")` keyed `ScrollHandle` + 显式id的gpui-component `Scrollbar` | 53列保持可读，同一source callsite的多个组件不共享scroll offset/scrollbar state | 无限压缩、wrap周列、直接调用callsite-keyed `overflow_x_scrollbar()` | 小窗口保持cell尺寸；多实例滚动隔离且rerender保留各自offset |
| `D-72` | zero用`secondary`；四级用`chart_1`的25%/45%/70%/100% opacity；边框/文字用`border`/`muted_foreground` | 完整继承gpui-component主题 | hard-coded Alma绿、crate自建dark palette | 亮暗主题切换无需重建data |
| `D-73` | 私有Plot使用`(component_id, "plot")` child ElementId；tooltip由Plot即时pointer命中 | 复用组件库tooltip overlay和theme并保持同级唯一id | 365个Div HoverCard/Entity/Task/timer | gap/padding/outside不显示tooltip |
| `D-74` | tooltip仅日期title +“总Token”exact row；零值cell同样可hover显示0 | 与用户确认范围一致 | input/output/cache详情、点击固定 | 每个真实日期均有确定tooltip |
| `D-75` | 外层只有一个`Role::Image`和调用方提供的完整aria label；无cell焦点/键盘action | 365个Tab stop不可用 | 每格button、隐藏焦点网格 | AX可读range/total/active days/peak且Tab序列不增长 |
| `D-76` | 不增加外部库；crate直接依赖`time = "0.3.54"`并复用workspace GPUI/gpui-component | 当前API足够 | calendar/heatmap/formatting新库 | Cargo.lock无新第三方package |

## 目标设计

### 文件与 owner 边界

```text
Cargo.toml                                      # F-061 [Modify] workspace dependency path
Cargo.lock                                      # F-062 [Modify] 仅正常workspace package记录
docs/dev/issue-189/
├── README.md                                   # F-063 [Modify] 四个执行文档与状态
├── settings-usage-analytics-plan.md             # F-064 [Modify] 定向supersede说明
└── settings-usage-activity-heatmap-plan.md      # F-065 [Add] 本执行计划

crates/gpui-heatmap/
├── Cargo.toml                                  # F-601 [Modify] time + test-support
├── README.md                                   # F-602 [Modify] 稳定API/用法
├── README.zh-CN.md                             # F-603 [Modify] 双语一致
├── src/
│   ├── lib.rs                                  # F-604 [Modify] exports
│   └── activity.rs                             # F-605 [Add] series/component/plot/tests
└── docs/dev/
    ├── README.md                               # F-606 [Modify] Ready入口
    └── issue-189/README.md                     # F-607 [Modify] WP-601 owner plan

crates/jaco-db/
├── src/records/analytics.rs                    # F-231 [Modify] query/activity/snapshot types
├── src/repository/analytics.rs                 # F-232 [Modify] selected + activity transaction
├── src/repository.rs                           # F-233 [Modify] test SQL re-export集合
├── src/tests/analytics.rs                      # F-234 [Modify] projection/query-plan tests
└── docs/dev/issue-189/README.md                # F-235 [Modify] WP-204

app/jaco/
├── Cargo.toml                                  # F-541 [Modify] gpui-heatmap workspace dep
├── src/features/settings.rs                    # F-542 [Modify] Usage search terms/test
├── src/features/settings/usage.rs              # F-543 [Modify] query/range/UI/tests
├── locales/en-US/main.ftl                      # F-544 [Modify] activity keys/remove dead keys
├── locales/zh-CN/main.ftl                      # F-545 [Modify] parity
└── docs/dev/issue-189/README.md                # F-546 [Modify] WP-504
```

禁止新增 `mod.rs`。`activity.rs`由`lib.rs`以`mod activity; pub use activity::{...};`导出。

### `C-22`：jaco-db → Jaco analytics query/snapshot

`crates/jaco-db/src/records/analytics.rs` 将跨owner API固定为：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsQuery {
    pub selected_range: UsageAnalyticsRange,
    pub activity_range: UsageAnalyticsFiniteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsActivity {
    pub range: UsageAnalyticsFiniteRange,
    pub summary: UsageAnalyticsAggregate,
    pub daily: Vec<UsageAnalyticsDailyBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsSnapshot {
    pub selected_range: UsageAnalyticsRange,
    pub selected_summary: UsageAnalyticsAggregate,
    pub provider_models: Vec<UsageAnalyticsProviderModelBucket>,
    pub activity: UsageAnalyticsActivity,
}

impl FreshRepository {
    pub fn usage_analytics(
        &self,
        query: UsageAnalyticsQuery,
    ) -> Result<UsageAnalyticsSnapshot>;
}
```

`UsageAnalyticsProviderModelBucket`及`provider_models` snapshot字段继续保留原有稳定ID、显示label与排序语义。`selected_daily`字段删除且不保留deprecated alias或兼容constructor；`UsageAnalyticsAggregate`、`UsageAnalyticsDailyBucket`、`UsageAnalyticsRange`与`UsageAnalyticsFiniteRange`保持现有语义。

### `DB-31`：单一一致性transaction

`usage_analytics(query)`只获取一个pooled connection，并在一个deferred read transaction中按固定顺序执行：

1. `selected_range` summary；
2. `selected_range` provider/model buckets；
3. `activity_range` summary；
4. `activity_range` dense daily。

两个范围即使相等也独立执行，不增加range-equality缓存分支。任何SQL、decode、overflow、invalid negative value、dense fill或invariant错误使整个query失败；app不显示新selected结果配旧activity结果。

### `DB-32`：范围、dense buckets与invariants

- summary/daily复用现有`created_at >= start AND created_at < end`与fixed-offset `strftime` SQL。
- `load_daily`继续接收任意合法finite range并逐本地日补零；DB层不硬编码365。
- `provider_models`逐字段checked sum等于`selected_summary`，并继续以稳定ID分组、当前label投影与确定性顺序返回。
- `activity.daily`逐字段checked sum等于`activity.summary`，所有日期严格递增且首尾与`activity.range`半开边界一致。
- repository返回前验证没有重复、范围外bucket、负值、i64→u64失败或checked sum overflow。

### `DB-33`：索引与query plan

- 继续使用schema version 1中的`idx_usage_events_created_at`；migration/schema/Cargo feature不变。
- focused EXPLAIN直接复用生产`SUMMARY_FINITE_SQL`、`DAILY_FINITE_SQL`与provider/model SQL，证明finite selected/activity范围走created-at index并保留label joins；AllTime查询保持table scan是预期行为。

### `C-23`：`gpui-heatmap`公开组件契约

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityHeatmapSeries {
    start_date: time::Date,
    end_date: time::Date,
    values: Vec<u64>,
    max_value: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityHeatmapSeriesError {
    Empty,
    RangeOverflow,
}

impl ActivityHeatmapSeries {
    pub fn try_new(
        start_date: time::Date,
        values: Vec<u64>,
    ) -> Result<Self, ActivityHeatmapSeriesError>;
    pub fn start_date(&self) -> time::Date;
    pub fn end_date(&self) -> time::Date;
    pub fn values(&self) -> &[u64];
    pub fn max_value(&self) -> u64;
}

#[derive(Clone)]
pub struct ActivityHeatmapLabels {
    pub months: [SharedString; 12], // January..December
    pub less: SharedString,
    pub more: SharedString,
    pub value: SharedString,
}

#[derive(IntoElement)]
pub struct ActivityHeatmap {
    // private fields
}

impl ActivityHeatmap {
    pub fn new(
        id: impl Into<ElementId>,
        series: ActivityHeatmapSeries,
        labels: ActivityHeatmapLabels,
        aria_label: impl Into<SharedString>,
    ) -> Self;
    pub fn caption(self, caption: impl Into<SharedString>) -> Self;
    pub fn format_date(
        self,
        formatter: impl Fn(time::Date) -> SharedString,
    ) -> Self;
    pub fn format_value(
        self,
        formatter: impl Fn(u64) -> SharedString,
    ) -> Self;
}

// ActivityHeatmap implements Sizable, Styled, and RenderOnce.
```

`try_new`接收owned values并缓存end date/max；拒绝空values、`len - 1`不能转换为`i64`或`start_date + len - 1 days`越界；error手写`Display + Error`，不增加`thiserror`。序列一旦构造成功，index `i`唯一对应`start_date + i days`。component在构造时预生成ISO date与完整十进制value labels；两个formatter builder立即重建owned label vectors而不保存closure，因此Jaco可直接借用当前I18n。week start在本期公开API中固定为Monday，不暴露unused定制入口。

### `HM-61`：布局与尺寸

- week column计算：`leading = start.weekday`相对week start的偏移，`columns = ceil((leading + len) / 7)`；row 0为week start。
- leading/trailing padding只参与布局，不映射series index，不绘制active fill且tooltip hit test返回`None`。
- 调用方提供短月份label。label放在该月第一个Monday（`weekday == Monday && day <= 7`）所在column，且从该column到月末在可见范围至少覆盖两个week columns；跨月混合周继续归前一个月，开头不完整月份和结尾只占一列的月份不标，避免误标或右侧裁切。
- 第一版不显示weekday gutter；Monday仍决定row顺序。月份、grid和footer作为同一固定宽度内容横向滚动。
- scroll root从caller component id派生`scroll-state`/`scroll-area`/`scroll-content`/`scrollbar`四个child id。render通过`window.use_keyed_state(scroll-state, ..., ScrollHandle::default)`取得handle，scroll area使用`.track_scroll(&handle).overflow_x_scroll()`，并显式子元素`Scrollbar::horizontal(&handle).id(scrollbar_id)`；不直接调用callsite-keyed `.overflow_x_scrollbar()`/`.horizontal_scrollbar()`。这个keyed `ScrollHandle`是组件唯一的本地交互状态，scrollbar的颜色、hover/drag/fade继续由gpui-component实现。
- 尺寸固定如下：`XSmall = 8px/2px gap`、`Small = 10px/2px`、`Medium = 12px/3px`、`Large = 14px/4px`；`Size::Size(cell)`先把non-finite/小于4px/大于32px限制到4–32px，再将gap限制在2–4px。
- cell圆角取当前theme radius与`cell / 4`的较小值；grid宽高由columns、7 rows、cell和gap精确推导。外层接受`Styled` refinement，但只作用于外层容器，不得覆盖内部hit-test几何。
- footer左侧显示optional caption，右侧显示`Less + 5 swatches + More`；允许flex wrap，不截断exact caption。

### `HM-62`：色阶与主题

对每个value使用：

```text
value == 0                       -> level 0
value > 0                        -> ceil(value / max(max, 1) * 4), clamp 1..4
```

视觉计算使用u128整数除法向上取整，避免乘4溢出且不损失exact u64；series、caption、tooltip与aria文本继续使用exact u64。level颜色每次render/paint从当前`ActiveTheme`读取：

```text
0 -> secondary
1 -> chart_1 @ 25%
2 -> chart_1 @ 45%
3 -> chart_1 @ 70%
4 -> chart_1 @ 100%
```

cell outline使用`border`，月份/legend/caption使用`muted_foreground`与正常foreground层级。crate不保存theme、颜色cache或theme subscription；light/dark/system theme切换由下一次render自然生效。

### `HM-63`：Plot tooltip与accessibility

- 私有`ActivityHeatmapPlot`使用`#[derive(gpui_component::plot::IntoPlot)]`并实现`Plot`。derive生成的Element请求`Size::full()`且本身不实现`Styled`/`Sizable`/`InteractiveElement`，因此必须放入宽高与grid几何完全一致的parent div。
- 外层stateful div保存caller id、`Role::Image`、aria label和style。私有Plot使用GPUI child id：`let plot_id: ElementId = (component_id.clone(), "plot").into();`，`id()`返回该稳定、同级唯一id。
- `tooltip_state`把plot-relative pointer坐标反解为column/row，只在cell内部命中；gap、padding、label、legend与bounds外返回`None`。
- `TooltipState.index`保存series index，`cross_line`填cell中心但不传给Tooltip渲染，`dots`为空；`tooltip`返回`plot::tooltip::Tooltip::new(cursor, bounds.size).title(date).row(level_hsla, labels.value, exact_value)`，不增加HoverCard或延迟Task；row颜色显式传`Hsla`，不传ThemeToken/Background。
- zero cell是有效日期，显示0；padding不显示。
- component最外层设置`Role::Image`和构造时必填的aria label；不设置`.tab_stop()`，不注册action/keybinding，不给cell创建accessibility child。

### `ST-61`：Jaco query identity与Operation lifecycle

`UsageSettingsPage`改为：

```rust
struct UsageSettingsPage {
    selected_period: UsageAnalyticsPeriod,
    active_query: Option<UsageAnalyticsQuery>,
    period_select: Entity<UsagePeriodSelectState>,
    operation: UsageAnalyticsOperation,
    _subscriptions: Vec<Subscription>,
}
```

`current_usage_query(period, now_utc)`即使period为AllTime也先获取一次`UtcOffset::local_offset_at(now_utc)`，然后调用纯函数`usage_query_for_offset`：

1. guard `now_utc.year()`在time offset转换安全headroom内；
2. `today = now_utc.checked_to_offset(offset)?.date()`；
3. `activity_start = today.checked_sub(Duration::days(364))?`；
4. `activity_end = today.next_day()?`；
5. selected/activity每个local boundary在`assume_offset`前再次guard其year为`-9998..=9998`，再通过checked offset转换生成`UsageAnalyticsFiniteRange`；
6. selected range保持现有五个period定义，AllTime只影响selected side。

一次`start_query`只创建一个`UsageAnalyticsQuery`、一个executor request、一个Operation Task。`active_query`是唯一completion identity；matching data要求：

```text
data.period == selected_period
snapshot.selected_range == active_query.selected_range
snapshot.activity.range == active_query.activity_range
operation is in the expected running/settled phase
```

周期切换、离开Usage、DB离开Ready、window/entity drop继续取消同一个Task。重新进入或跨本地午夜产生不同query时不显示旧activity；同query refresh/degraded允许继续显示settled snapshot。不增加第二Operation、cache、generation counter、polling或focus refresh。

### `L-61`：Settings页面组合与本地化

页面顺序：

```text
[Description                                  Period Select]
[Selected-period Summary]
[Rolling 365-day Activity GroupBox]
[Selected-period provider/model breakdown Table]
```

- 删除`render_trend`、LineChart imports、tick/dot helpers与`DailyChartPoint`；恢复/保留`render_breakdown`、Table imports、精简列、label fallback与table AX helpers。
- Usage page search terms同时覆盖provider/model与activity/heatmap/calendar/daily/year，包括对应中文与拼音；page位置、icon与navigation不变。
- Activity section使用现有`GroupBox::outline()`承载本地化title/description，内部放`ActivityHeatmap::new(...).with_size(Size::Medium)`。
- DB completion在发送Operation `Complete`前通过`UsageAnalyticsData::try_new`将`activity.daily`验证并适配为`ActivityHeatmapSeries(start_date, total_tokens...)`，同时缓存active days/peak；任何空vector、非365长度、日期不连续或构造失败视为typed activity error。render只消费已验证view data，不在render中发起fallible transition或静默隐藏。
- caption使用activity summary exact total与固定365天文案；tooltip value复用exact formatter；日期复用`settings-usage-date-value`。
- month labels由一个Fluent select key按January..December生成；无需在crate中读locale。
- aria label至少包含start、end、exact total、active day count、peak date和exact peak token；全零时peak使用本地化“无活动”分支，不伪造日期。
- `selected_summary`非空：显示summary、activity与provider/model Table。
- selected为空但activity非空：显示紧凑selected-period empty status与activity，不渲染空Table。
- selected非空但activity为空：显示summary、全零activity grid与provider/model Table，明确最近365天无活动。
- selected与activity都为空：保留现有全局empty页面，不渲染空grid。
- refreshing/degraded只有snapshot与完整active query匹配时继续显示这些内容。

两份`main.ftl`至少新增并保持parity：

```text
settings-usage-selected-period-empty
settings-usage-activity-title
settings-usage-activity-description
settings-usage-activity-caption
settings-usage-activity-less
settings-usage-activity-more
settings-usage-activity-month-label
settings-usage-activity-accessible
settings-usage-activity-accessible-no-peak
```

`settings-usage-total-tokens`、`settings-usage-date-value`、`settings-usage-breakdown-title`、`settings-usage-provider`与`settings-usage-model`继续复用。删除LineChart后，以`rg`确认无consumer再删除`settings-usage-trend-title`、`settings-usage-trend-description`与`settings-usage-trend-accessible`；不改macOS bundle localization。

变量契约：`selected-period-empty`使用`$range`；`activity-caption`使用exact `$total`与固定`$days = 365`；`month-label`使用January..December枚举值`$month`；两个accessible key使用`$start`、`$end`、`$total`、`$activeDays`，有peak版本再使用`$peakDate`与`$peakTokens`。全部数字变量先在Rust格式化为完整整数文本。

### `ERR-61`：失败与诊断

| 来源 | typed结果 | UI | 日志 |
| --- | --- | --- | --- |
| local offset不可用 | `UsageAnalyticsProblemSource::LocalOffset` | 现有load/retry error | period + error，不记录usage内容 |
| 日期减法、midnight或offset转换越界 | `CalendarRange` | 现有load/retry error | period + query none |
| DB SQL/decode/invariant | `Database(DbError)` | 现有load/degraded/retry映射 | selected/activity range + error |
| app收到非365、非连续或范围不匹配的activity | `UsageAnalyticsProblemSource::Activity(UsageActivityInvariant)` | 整个refresh失败；旧exact-query data可degraded显示 | 记录variant/range/length，不记录provider payload |
| component series构造失败 | `ActivityHeatmapSeriesError`在app适配处转typed problem | 同上 | error variant + range |

不以空grid、旧provider table、部分snapshot或默认UTC掩盖错误。

## Requirements

| ID | Requirement |
| --- | --- |
| `R-61` | activity恰好覆盖包含今天的最近365个本地日历日 |
| `R-62` | selected period只控制summary与provider/model Table，activity固定rolling 365 days |
| `R-63` | selected/activity共用一次捕获的now与fixed offset |
| `R-64` | repository在一个read transaction返回完整typed snapshot |
| `R-65` | provider/model buckets与selected summary、activity daily与activity summary分别逐字段checked一致 |
| `R-66` | selected daily/LineChart/dead trend i18n完整删除；provider/model bucket、SQL、Table与breakdown i18n保留 |
| `R-67` | series类型拒绝empty/range overflow并保证连续日期 |
| `R-68` | Monday布局、padding、month positions与365天列数确定 |
| `R-69` | 0 + 4级linear max mapping覆盖all-zero、四分位边界、single/outlier/u64::MAX且不溢出 |
| `R-70` | tooltip只命中真实cell并显示本地日期与exact total token |
| `R-71` | caption、tooltip与AX不使用f64或compact formatter |
| `R-72` | 颜色只来自当前gpui-component theme tokens |
| `R-73` | 组件只保留由caller id隔离的keyed `ScrollHandle`；不实现业务/hover Entity、Task、timer、FocusHandle、action、keybinding或subscription |
| `R-74` | 整图Role::Image提供完整文字等价信息且不增加365个Tab stop |
| `R-75` | 窄页面横向滚动，cell不低于所选Size映射 |
| `R-76` | selected empty/activity nonempty仍可查看过去一年活动 |
| `R-77` | AllTime仍返回rolling activity并在offset失败时走typed error |
| `R-78` | 无schema/migration/version/兼容层或新外部package |
| `R-79` | en-US/zh-CN keys parity并在现场语言/主题变化后重建当前文本/颜色 |
| `R-80` | Usage search terms同时反映activity heatmap与provider/model明细 |

## 工作包

### `WP-601` — `gpui-heatmap`活动组件（Implemented）

Owner：[crate plan](../../../crates/gpui-heatmap/docs/dev/issue-189/README.md)

1. 落地`C-23` series/error/labels/component exports与双语README。
2. 实现Monday日期布局、month labels、固定Size、横向内容宽度，以caller id keyed `ScrollHandle` + 显式gpui-component `Scrollbar`隔离多实例scroll state，再完成caption/legend。
3. 实现linear max levels、ActiveTheme colors、私有Plot绘制与pointer Tooltip命中。
4. 提供whole-chart Role::Image，不增加keyboard/cell state。
5. 补纯逻辑与GPUI window tests并通过crate focused gates。

依赖：已建立的crate骨架与锁定GPUI/gpui-component。可与`WP-204`并行；解锁`WP-504`。

### `WP-204` — `jaco-db` selected + activity projection（Implemented）

Owner：[DB plan](../../../crates/jaco-db/docs/dev/issue-189/README.md)

1. 落地`C-22` query/activity/snapshot breaking API。
2. 保留provider/model projection、SQL、label joins、exports和测试，删除selected daily snapshot/query。
3. 在一个transaction查询selected summary/provider-model与activity summary/daily。
4. 保留dense fill、两组checked totals与created-at index proof，补365/offset/AllTime/error tests。

依赖：现有`WP-203` authority/index。可与`WP-601`并行；解锁`WP-504`。

### `WP-504` — Jaco range、Operation与Settings UI接入（Implemented）

Owner：[Jaco plan](../../../app/jaco/docs/dev/issue-189/README.md)

1. 增加workspace dependency，迁移query builder与`active_query` identity。
2. 适配`C-22`和`C-23`，保持单Operation/Task/cancel/degraded语义。
3. 删除LineChart与dead trend helpers/keys，保留精简Table并加入activity GroupBox、caption、tooltip与AX文本。
4. 修正selected/global empty组合、更新Usage search terms，并补range、lifecycle、render、Table与i18n测试。
5. 完成人工主题、语言、横向scroll、hover和生命周期矩阵。

依赖：`WP-601`与`WP-204`。不改message/composer surfaces。

### 执行顺序

```text
WP-601 ─┐
        ├─> WP-504 -> focused validation -> manual matrix -> workspace/CI gates
WP-204 ─┘
```

## 测试计划

| ID | Owner | 自动化场景 |
| --- | --- | --- |
| `T-61` | heatmap | series拒绝empty、len/date overflow；end date精确 |
| `T-62` | heatmap | Monday布局覆盖365天、跨年、leap day、leading/trailing padding |
| `T-63` | heatmap | month labels只落在该月第一个Monday columns；跨月混合周归前月，partial first与final one-column month不标 |
| `T-64` | heatmap | all-zero、single positive、outlier、equal values、u64::MAX等级稳定 |
| `T-65` | heatmap | Size四档与custom size的cell/gap/grid bounds精确 |
| `T-66` | heatmap | hit test区分cell、gap、padding、labels、outside；zero cell可命中 |
| `T-67` | heatmap GPUI | tooltip使用stable id、exact labels、当前theme；无focus/action |
| `T-68` | heatmap GPUI | 同一source callsite渲染两个不同component id，滚动其中一个不改变另一个且rerender各自保留offset；wrapper有唯一Role::Image/aria label且Tab序列不含cells |
| `T-69` | DB | finite selected provider-model + 365 activity在同snapshot各自sum一致 |
| `T-70` | DB | AllTime仍返回provider-model聚合与365个activity dense buckets，snapshot无selected daily |
| `T-71` | DB | positive/negative/half-hour offset、cross-year/leap range日期正确 |
| `T-72` | DB | invalid/overflow使整个transaction失败，无partial snapshot |
| `T-73` | DB | production finite summary/daily/provider-model SQL走created-at index并保留label joins |
| `T-74` | app | query builder对Today/Week/Month/Year/AllTime均生成同offset 365 activity |
| `T-75` | app | Date边界、rolling减法后的boundary year与极端offset无panic并返回CalendarRange |
| `T-76` | app lifecycle | duplicate activate single-flight；leave/DB unavailable/drop取消唯一Task |
| `T-77` | app lifecycle | period/query/range/running mismatch completion不发布；exact refresh可degraded显示 |
| `T-78` | app UI | selected empty/activity nonempty、selected nonempty/activity zero、both empty三种组合 |
| `T-79` | app UI | LineChart不存在；精简Table列/AX保留；activity caption/tooltip/AX使用exact formatter |
| `T-80` | i18n | 两locale key parity、month select分支与新增变量完整 |
| `T-81` | app settings | Usage搜索同时命中activity/heatmap与provider/model的中英文及拼音词 |

## 聚焦验证

实现阶段只运行与本轮受影响owner直接相关的最小充分门禁：

```sh
cargo fmt
cargo test -p gpui-heatmap
cargo clippy -p gpui-heatmap --all-targets --all-features -- -D warnings

cargo test -p jaco-db usage_analytics
cargo clippy -p jaco-db --all-targets --all-features -- -D warnings

cargo test -p jaco features::settings::usage::tests --no-fail-fast
cargo test -p jaco features::settings::tests --no-fail-fast
cargo test -p jaco i18n --no-fail-fast
cargo check -p jaco
cargo clippy -p jaco --all-targets --all-features -- -D warnings

git diff --check
```

workspace-wide `cargo build`、`cargo test`、strict clippy与三平台CI留到Issue #189聚合最终门禁，不在每个owner工作包重复运行。

### 人工场景

1. en-US/zh-CN与light/dark/system theme分别检查month labels、zero/四级颜色、legend、caption和tooltip。
2. 365天跨年且包含leap day时，随机核对日期column/row与DB每日exact total。
3. pointer从cell快速移动到gap、padding和另一cell，tooltip不滞留、不显示错误日期。
4. Settings常规宽度无需滚动；缩窄窗口后出现horizontal scrollbar且cell/tooltip仍对齐。分别在未滚动左右端、滚动到最右后的可视区域左右端hover，tooltip必须完整显示且不被scroll content mask裁切；失败即阻断验收并在组件内部修正定位。
5. Today/Week/Month/Year/AllTime切换只改变summary/Table，activity start/end/total保持同一次刷新语义。
6. selected period无数据但过去365天有数据时仍显示activity；两者都为空时显示全局empty。
7. refresh失败显示exact-query stale data + warning；离开Usage再返回会重新查询且旧Task不发布。
8. 用accessibility inspector确认整图只暴露一个有完整summary的image node，Tab不会逐格停留。

## 实施证据

- `WP-601`：`gpui-heatmap`稳定API、Monday-first layout、月份标记、0 + 4级linear max色阶、theme、精确Plot tooltip、caller-id keyed滚动与single-image AX已落地；`cargo test -p gpui-heatmap`通过10项，crate check与strict clippy通过。
- `WP-204`：最终snapshot为`selected_range + selected_summary + provider_models + activity`；selected daily已删除，selected provider/model与activity daily分别完成checked cross-total。`cargo test -p jaco-db usage_analytics --no-fail-fast`通过12项，`cargo test -p jaco-db --no-fail-fast`通过67项，strict clippy与格式检查通过。
- `WP-504`：最终页面为selected summary/empty status → rolling 365-day activity → selected provider/model Table；LineChart、trend helpers与dead Fluent keys已删除，精简7列表格、双语/搜索/AX已保留。Usage tests通过19项，Settings tests通过16项，i18n tests通过11项，`cargo check -p jaco`与strict clippy通过。
- 澄清前的中间实现曾通过`cargo run -p xtask -- bundle jaco`生成并签名`target/release/bundle/macos/Jaco.app`；该bundle早于最终“移除LineChart并保留Table”的修正，不计作最终UI证据。Liquid Glass图标注入因CoreSimulator不可用而跳过，普通图标bundle成功。
- 中间bundle曾以临时`JACO_CONFIG_DIR`/`JACO_LOG_DIR`启动并确认未读取用户真实数据库；当前前台图形会话为`loginwindow`，最终实现的light/dark、双语、窄宽滚动两端tooltip、degraded refresh与AX Inspector矩阵未执行，临时目录已清理。
- `WP-601`不受本次澄清影响；implementation commit / PR、workspace-wide gates、修正后bundle人工矩阵与三平台CI仍为`Pending`。

## 完成条件

- `WP-601`、`WP-204`、`WP-504`全部达到`Implemented`并记录实际文件、命令和结果。
- `C-22`与`C-23`在producer/consumer中完全一致，无selected daily/LineChart dead path，provider/model Table链路完整。
- `R-61`–`R-80`与`T-61`–`T-81`有自动化或明确人工证据。
- crate README、root plan、三个owner plan与索引同步到实际稳定API。
- 记录实际implementation commit/PR、未执行的workspace/CI/manual边界；只有Issue #189最终不再修改时才执行一次完整最终门禁。

## 执行交接审计

- 计划前仍需用户决定的问题：`0`。范围、交互、时区、week start、色阶、responsive与AX均已确认。
- 执行时必须保留精简provider/model Table；不得重新引入LineChart、第二Operation、兼容层、费用估算或硬编码颜色。
- 若当前gpui-component锁定API发生变化，只允许在`WP-601`内按同等语义调整实现形状；不得改变`C-23`的产品无关输入、exact tooltip、theme与AX契约。
- 若发现`usage_events`无法重建某日total，停止并报告authority缺口；不得用summary平均值、当前catalog或猜测回填daily cell。
