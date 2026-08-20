# Jaco：Issue #189 message usage、composer context 与 Settings analytics UI

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)、[Composer context occupancy](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md)、[Settings usage analytics](../../../../../docs/dev/issue-189/settings-usage-analytics-plan.md)
- Owner directory：`app/jaco`
- Owner status：`In progress`（`WP-501`、`WP-502`、`WP-503` 已 `Implemented`；root-level workspace/known-provider/CI gates待做）
- 消费 root IDs：`C-01`–`C-03`、`C-12`、`C-21`、`D-05`、`D-06`、`D-08`、`D-19`–`D-21`、`D-32`、`D-39`–`D-46`、`ST-01`、`ST-11`、`ST-21`、`ERR-21`、`R-01`–`R-13`、`R-27`–`R-34`、`R-41`、`R-43`、`R-52`–`R-56`
- Assigned WP：`WP-501`、`WP-502`、`WP-503`
- Owns：Conversation effect消费、timeline message usage、composer projection/footer、Settings Usage period/range/Operation/UI、shared formatter、typed icons、Fluent、GPUI tests/manual validation
- Does not own：DB association/selection、usage authority/aggregate SQL、analytics fresh-schema index、provider raw parsing、capability persistence或#194 editor

## Owner-local 证据与决定

- `E-501`：`ConversationDetailPage::sync_timeline` 以Ready `Conversation` 构造rows；页面已能按run ID精确update/remeasure。
- `E-502`：`timeline::agent_turn_row` 是 `AgentTurnRow` 唯一构造入口。
- `E-503`：`agent_action_row` 当前为24px高、760px max width、Copy + hover time。
- `E-504`：`CopyButton` 已采用ghost/xsmall/icon/tooltip模式。
- `E-505`：当前gpui-component `HoverCard`原生负责window overlay、trigger/content hover、默认600ms open delay、300ms close delay与timer cancellation；`DescriptionList`适合单列key-value；HoverCard明确是pointer-only。
- `E-506`：Settings由`SettingsView`持有page entities与数据库overlay；`SettingsPageFrame`默认提供外层滚动。
- `E-507`：current gpui-component已有Select、LineChart、simple Table、Alert与Skeleton；LineChart numeric axis使用f64。
- `D-501`：request usage以plain cloned projection传入row；app不为详情新增open/hover/pinned/focus Entity、Store、Global、cache、Task或subscription。
- `D-502`：新增专用 `request_usage.rs`，不抽象为composer/settings共用组件。
- `D-503`：usage图标始终可hover；只有Reported total摘要与完成时间采用相同字体、字号、颜色与message group-hover时机，摘要以`k`/`M`等紧凑形式显示且位于trigger之外，不触发详情；只有图标按HoverCard组件默认延迟展开保留完整整数的Alma式详情；partial/unreported/unavailable不显示未知total或0，只保留图标与详情状态。
- `D-504`：详情使用原生pointer-only HoverCard；不提供click固定、Escape、键盘打开或focus return，app不重复组件已有的hover timing/state/Task。
- `D-505`：Settings analytics用独立`UsageSettingsPage` Entity直接拥有refresh Operation；不复用conversation projection，也不增加Store/Global。

## 文件与 ownership tree

```text
app/jaco/
├── src/
│   ├── components/chat/
│   │   ├── detail.rs                         # F-501 [Modify] module、effect handler、precise row update
│   │   └── detail/
│   │       ├── timeline.rs                   # F-502 [Modify] projection传递/update/tests
│   │       ├── message.rs                    # F-503 [Modify] AgentTurnRow/action row插入trigger
│   │       └── request_usage.rs              # F-504 [Add] formatter、纯icon trigger、HoverCard、DescriptionList/tests
│   └── foundation/assets.rs                  # F-505 [Modify] ChartNoAxesColumn typed icon/test
├── locales/
│   ├── en-US/main.ftl                        # F-506 [Modify] request usage keys
│   └── zh-CN/main.ftl                        # F-507 [Modify] parity keys
└── docs/dev/
    ├── README.md                              # F-508 [Modify] owner index
    └── issue-189/README.md                    # F-509 [Add] 本计划
```

不修改chat form/composer、Settings、Cargo manifest/lock、bundle assets、macOS localization或provider logos。

## GPUI contracts

### L-501：Timeline request usage projection

`ConversationTimelineRows` 增加精确更新方法：

```rust
impl ConversationTimelineRows {
    pub(super) fn update_agent_request_usage(
        &mut self,
        agent_run_id: &AgentRunId,
        request_usage: AgentMessageRequestUsage,
    ) -> Option<TimelineRowKey>;
}
```

- `build_rows` 从 `Conversation.agent_message_request_usages` 建立borrowed `AgentRunId -> projection` map，并传给 `agent_turn_row`。
- `AgentTurnRow` 新增 `request_usage: Option<AgentMessageRequestUsage>`。
- update只replace matching Agent row；找不到row返回None并由page执行full `sync_timeline`。
- sibling rows、expanded state、tool preview/copy state均不变。

`ConversationDetailPage::apply_conversation_effect` 对 `AgentMessageRequestUsageChanged { agent_run_id }`：

1. 从Ready Conversation按exact run ID取projection clone。
2. 调用 `L-501`。
3. 成功则 `remeasure_timeline_row`；失败或projection缺失则full sync。

不得在UI按entry/step重新join或查询DB。

### L-502：RequestUsageDisclosure + 原生HoverCard

`F-504` target（抽象契约，不绑定具体构造器签名）：

- `RequestUsageDisclosure`是无状态`RenderOnce`组合：读取projection并组装纯icon trigger、原生HoverCard、详情内容与trigger外的可选Reported total摘要。
- trigger持有stable element/debug identity，只渲染`ChartNoAxesColumn`与localized accessible label；它不包含Reported total摘要，也不注册click/key/focus handler。
- `RequestUsageDisclosure`在HoverCard trigger之外渲染可选Reported total摘要；摘要与完成时间共享message group-hover reveal，但没有HoverCard hover/click处理。
- open/close timer、trigger/content hover与cancellation全部由gpui-component `HoverCardState`拥有；app不镜像这些状态。

Composition：

```text
RequestUsageDisclosure (stateless RenderOnce)
├── HoverCard("conversation-request-usage-hover-card-{step_id}")
│   ├── trigger: icon div ("conversation-request-usage-{step_id}")
│   │   ├── always-available IconName::ChartNoAxesColumn
│   │   └── localized accessible label
│   └── content
│       ├── localized title
│       └── DescriptionList::horizontal()
│           .columns(1)
│           .bordered(false)
│           .small()
└── optional Reported total summary (message group-hover reveal; outside trigger)
```

- ElementId由stable provider step ID构成，并处在agent row parent scope；禁止index。
- Reported usage才在trigger右侧显示紧凑 total token；partial/unreported/unavailable只显示图标，未知值保留在详情状态中。
- HoverCard使用现有appearance/theme与组件默认open/close delay，不硬编码light/dark颜色或app timing。
- 只有trigger图标hover可打开；total摘要和message空白不打开；content hover与延迟关闭由HoverCard内部处理；click和键盘不属于本交互。
- content不新增业务/UI Entity、Store、Global、subscription或Task，并且不得执行association/format之外的重工作。

### L-503：Field projection与formatter

`F-504` private pure contracts：

```rust
fn request_usage_fields(
    request_usage: &AgentMessageRequestUsage,
    i18n: &I18n,
) -> RequestUsageContent;

fn format_token_count(value: u64) -> String;
fn format_rate(value: f64) -> String;
```

`RequestUsageContent` 只在该module内部表示 `Unavailable | Unreported | Fields(Vec<(String, String)>)`。

- `format_token_count` 输出ASCII decimal grouping：0、999、1,000、24,716、u64::MAX均正确，不使用locale-sensitive float。
- `format_rate` 把ratio乘100并保留一位小数；非finite值不进入contract。
- Reported/Partial/Unreported/Missing字段规则严格引用root UI contract，不显示provider metadata。
- Reported total token由action row显示`k`/`M`等紧凑摘要，文本样式与完成时间一致；HoverCard详情继续显示完整千分位整数。Partial的action row不显示total，详情中使用 `conversation-request-usage-unknown-value`；Unavailable/Unreported显示单一状态文案。
- cache read/write/reasoning为0时省略；input/output/known total保留0值。

### L-504：Action row integration

`AgentActionRow` 增加 `request_usage: Option<AgentMessageRequestUsage>`。

`agent_action_row` 顺序固定为：

1. Copy button；
2. optional usage cluster：始终显示纯icon HoverCard trigger，Reported时在trigger外右侧显示紧凑total token；
3. 现有hover completion time，与 usage cluster 采用同一 message group-hover reveal。

只有usage cluster的Reported total摘要与time共享message group-hover reveal；摘要是trigger之外的普通文本，不触发HoverCard。trigger图标始终可hover，按HoverCard组件默认延迟打开；不提供click或键盘操作。

## 图标与 Fluent

### Icon

`F-505`：

```rust
ChartNoAxesColumn => "chart-no-axes-column",
```

扩展现有 `declared_icons_have_lucide_paths`，断言 `icons/chart-no-axes-column.svg`。不提交新SVG。

### Runtime localization

`F-506`/`F-507` 增加root执行文档列出的13个 `conversation-request-usage-*` keys，名称与集合完全一致。

- title/tooltip/state/field label全部通过 `I18n::t`。
- unknown value用同一个localized key，不在Rust硬编码英文。
- bundle `InfoPlist.strings` 不变。

## 状态与lifecycle

### ST-501：UI projection

- **Authority：** Ready `Conversation.agent_message_request_usages`
- **Owner/lifetime：** plain value cloned into `TimelineRow::Agent`；随timeline rebuild丢弃
- **Readers：** `L-502`/`L-503`
- **Mutation：** `L-501` only，来源为Conversation effect后的authority
- **Persistence：** 业务projection None；app不持有open/hover state或缓存formatted strings；HoverCard内部临时state由组件keyed lifecycle拥有
- **Reset：** reload/full sync重建；delete清空timeline
- **Task/subscription：** 复用现有ConversationModel subscription；app不新增业务/runtime/UI Task或subscription；HoverCard默认timing Task由gpui-component内部拥有
- **Window：** HoverCard overlay与trigger/content hover lifecycle由gpui-component拥有

## WP-501：实现action row与HoverCard

1. 在 `F-504` 先实现 `L-503` pure formatting/field tests。
2. 增加typed icon与path test、两locale keys与parity test。
3. 实现`L-502`，按无状态`RequestUsageDisclosure` + pure-icon trigger + trigger外total摘要 + 原生HoverCard/DescriptionList契约编译。
4. 在 `F-503` 接入 `L-504`。
5. 在 `F-502` 接入build/update projection与row tests。
6. 在 `F-501` 消费root `C-03` effect并精确remeasure。
7. 运行focused tests/check后，用隔离数据执行pointer/reload人工场景。

### Tests

| T-ID | Proposed test |
| --- | --- |
| `T-501` | `request_usage_fields_render_reported_partial_unreported_and_unavailable` |
| `T-502` | `token_count_and_cache_rate_formatting_preserve_exact_values` |
| `T-503` | `timeline_attaches_request_usage_to_only_its_agent_run` |
| `T-504` | `request_usage_update_remeasures_only_owning_agent_row` |
| `T-505` | `tool_loop_row_uses_final_entry_request_usage_without_sum` |
| `T-506` | `request_usage_hover_card_uses_only_the_icon_and_component_delays` |
| `T-507` | `declared_icons_have_lucide_paths` extension |
| `T-508` | existing/new locale parity test包含全部request usage keys |

### Focused validation

```sh
cargo fmt
cargo test -p jaco request_usage
cargo test -p jaco timeline
cargo test -p jaco i18n
cargo check -p jaco
git diff --check
```

人工验证：

1. reported普通请求：chart icon始终可hover，message group-hover以和时间一致的文本样式显示其右侧紧凑total token但摘要不触发详情；只有icon按HoverCard默认延迟打开，详情字段/完整千分位正确。
2. tool loop：只显示final step。
3. partial/all-zero/missing fixtures：unknown/unreported/unavailable可区分。
4. OpenAI-style/Anthropic-style/Ollama-style cache fixtures：rate正确或省略。
5. 鼠标快速掠过、持续hover、trigger到content与离开延迟关闭；click和键盘不属于本交互。
6. 重开conversation后同一消息值、按钮identity与live一致。

完成条件：`L-501`–`L-504`、`ST-501` 与 `T-501`–`T-508` 通过；人工场景有记录；chat form、Settings与excluded metrics无diff。

## 实施证据（2026-08-20）

- 已实现Conversation effect精确更新、timeline projection、agent action-row projection、原生HoverCard/DescriptionList、分离的exact/compact formatter、typed icon及en-US/zh-CN keys；当前UI语义为始终可hover的纯图标、与时间样式一致的trigger外group-hover紧凑total摘要、详情完整整数与组件默认延迟。
- `cargo test -p jaco request_usage`：8 passed（包含摘要compact/详情exact边界、真实GPUI window的icon-only trigger、快速掠过取消、默认延迟与trigger/content过渡）；`cargo test -p jaco timeline`：11 passed；`cargo test -p jaco request_usage_update`：2 passed；两个typed icon focused tests通过。
- `cargo fmt` 与 selected-package combined strict clippy 通过；workspace-wide `cargo build`、`cargo test`、`cargo clippy`、known/provider 场景与三平台 CI 未执行；chat form、Settings、TTFT/TPS与provider metadata均未接入。
- 最终HoverCard交互已由用户检查确认符合预期；release bundle 构建成功。隔离配置下 fresh no-model `Gauge —`、AX label、默认延迟 HoverCard、详情内容与 footer 布局已验证。

## Composer extension — `WP-502`（Implemented）

本节登记 [composer 执行文档](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md) 的 app owner contract。`WP-501` 的 action-row视觉与pointer交互不得改变。

### Owner-local 文件与边界

```text
app/jaco/src/
├── state/providers.rs                         # F-526 [Modify] persisted capability projection
├── components/chat.rs                         # F-511 [Modify] module declaration
├── components/chat/context_occupancy.rs       # F-512 [Add] projection/UI/tests
├── components/chat/detail.rs                  # F-513 [Modify] reload/effect sync
├── components/chat/input.rs                   # F-514 [Modify] singular fact owner
├── components/chat/form.rs                    # F-515 [Modify] footer builder slot
├── components/chat/detail/request_usage.rs    # F-516 [Modify] shared formatter consumer
├── components/chat/model_picker.rs            # F-521 [Modify] capability fixture fallout
├── features/conversation.rs                   # F-522 [Modify] capability/domain fixture fallout
├── features/conversation/attachments.rs       # F-523 [Modify] capability fixture fallout
├── features/conversation/model.rs             # F-524 [Modify] Conversation fixture fallout
├── features/home/sidebar.rs                   # F-525 [Modify] Conversation fixture fallout
├── foundation/conversation_format.rs          # F-517 [Modify] exact/compact token helpers
└── foundation/assets.rs                       # F-518 [Modify] typed Gauge + path test

app/jaco/locales/
├── en-US/main.ftl                             # F-519 [Modify] context occupancy keys
└── zh-CN/main.ftl                             # F-520 [Modify] parity keys
```

- 不新增Entity/Store/Global/Operation/Task、overlay state、timer、subscription或数字格式化dependency。
- `ChatForm`保持pure visual shell；不读取Conversation/repository/provider global。
- composer UI不复用message action-row component，也不展示request input/output/cache/reasoning明细。

### `L-511`：Raw fact ownership 与纯派生

- `ConversationDetailPage` 在initial/reload、composer context effect与conversation clear后，把 `Conversation.latest_context_request_usage.clone()` 同步给 `ChatInputController`；effect可来自accepted或ignored replay。
- controller singular-fact setter先做`PartialEq`等值判断，只有raw fact实际变化时才notify。
- controller只保存该plain optional fact；form/current catalog沿既有observe/notify路径触发render。
- provider catalog 从持久化record原样派生 `ProviderModelChoice`，不做读取时补全；升级前缺capability的缓存保持unknown，用户执行provider refresh后消费新持久化值。
- `context_occupancy.rs` 始终构造projection；无model selection时current choice为None并派生typed unknown，其余情况按root固定顺序派生capability、exact identity match与coverage。
- percentage用`u128`整数计算到十分位；1%、37.5%、125%均保留准确文本，不clamp。
- model切换不保存per-model history；切回时仍只检查conversation singular latest fact。

### `L-512`：Footer trigger 与 HoverCard

Footer插槽固定为既有flex spacer之后、model selector之前：

```text
[left controls] [flex spacer] [Gauge 37.5%] [model selector] [send/stop]
```

- 常驻 `IconName::Gauge + text_xs percentage`；unknown为`Gauge + —`。
- icon/text同为muted foreground、nowrap、shrink-0，无button chrome、fill、progress ring或threshold color。
- 整个cluster是原生 `HoverCard` trigger，使用默认600ms open/300ms close delay与`Anchor::BottomRight`。
- pointer-only；不注册click、keydown、focus handle或tab stop。
- known详情用exact formatter列出used/window/occupancy/provider/model/request time；unknown详情显示typed reason与仍可确定的metadata。

### `L-513`：Formatter、Fluent 与 accessibility

- 把 `request_usage.rs` 当前exact/compact helpers原样迁移到 `foundation/conversation_format.rs`，保留WP-501输出/tests。
- `Gauge` 走app-local `IconName`与既有Lucide runtime asset；扩展typed path test。
- root计划列出的全部Fluent keys在`en-US`/`zh-CN` parity出现。
- cluster暴露一个localized、非可聚焦的image-role accessibility label；known label含percentage，unknown label含状态。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-511` | `composer_context_projection_classifies_all_known_and_unknown_states` |
| `T-512` | `composer_context_percentage_formats_integer_decimal_and_over_capacity` |
| `T-513` | `composer_context_model_switch_uses_only_latest_conversation_request` |
| `T-514` | `composer_footer_places_gauge_before_model_without_shrinking_primary_controls` |
| `T-515` | `composer_context_hover_card_uses_whole_cluster_and_component_default_delays` |
| `T-516` | `shared_token_formatters_preserve_request_usage_exact_and_compact_output` |
| `T-517` | typed Gauge path、locale parity与accessible label coverage |
| `T-518` | `composer_context_fact_setter_deduplicates_replayed_or_ignored_effects` |
| `T-519` | 升级前缺context-window的GPT-5.6缓存保持unknown直至provider refresh；已持久化的discovered/manual/official-doc capability与provenance原样进入choice |

```sh
cargo fmt
cargo test -p jaco context_occupancy
cargo test -p jaco request_usage
cargo test -p jaco i18n
cargo check -p jaco
git diff --check
```

完成条件：root `ST-11`、`R-27`–`R-34`与`T-511`–`T-519`通过；用户参考图对应的`Gauge + percentage/—`常驻footer，默认HoverCard详情和WP-501 action row均有回归证据。

## Composer 实施证据（2026-08-20）

- `WP-502` 已 `Implemented`；provider state 4、`context_occupancy` 7、`request_usage` 8、`i18n` 11、`context_request_usage_setter` 1 passed，`cargo check -p jaco` 通过。
- `cargo fmt` 与 `cargo clippy -p jaco -p jaco-agent -p jaco-db --all-targets --all-features -- -D warnings` 通过；隔离配置的fresh no-model `Gauge —`、AX label、默认 HoverCard/details/layout已在此前UI构建中验证；移除读取时兼容补全后的最终bundle尚未重建。
- 现场provider refresh/新请求、完整人工矩阵、workspace-wide build/test/clippy、三平台 CI 与 implementation commit/PR 仍 `Pending`。

## Settings extension — `WP-503`（Implemented）

本节只登记 [Settings analytics 执行文档](../../../../../docs/dev/issue-189/settings-usage-analytics-plan.md) 的 app owner contract。`WP-501`/`WP-502` 的message/composer UI和证据保持不变。

### Owner-local 文件与边界

```text
app/jaco/
├── src/
│   ├── features/settings.rs                  # F-531 [Modify] entity/nav/render/DB lifecycle
│   ├── features/settings/layout.rs           # F-532 [Modify] Usage page key
│   └── features/settings/usage.rs            # F-533 [Add] period/range/Operation/UI/tests
├── locales/
│   ├── en-US/main.ftl                        # F-534 [Modify] settings usage keys
│   └── zh-CN/main.ftl                        # F-535 [Modify] parity keys
└── docs/dev/
    ├── README.md                              # F-536 [Modify] owner index
    └── issue-189/README.md                    # F-537 [Modify] this owner plan
```

- `usage.rs` 是独立Settings page module；不放入chat/conversation/state全局模块。
- 页面只调用 `FreshRepository::usage_analytics`；不读取/解析/聚合 `UsageEventRecord`。
- 不新增Store、Global、Form、DatePicker、app-local action/keybinding、detached Task、timer、polling或dependency。

### `L-531`：Settings navigation 与 page ownership

- `SettingsPageKey` 增加 `Usage`，位于`Provider`之后；`settings_page_specs_for_i18n`从8项改为9项。
- spec使用`settings-page-usage`、`IconName::ChartNoAxesColumn`与usage/token/statistics/analytics/request/provider/model/cache及中文/拼音search terms。
- `DatabaseSettingsPages`新增`usage: Entity<UsageSettingsPage>`；`database_page`和render dispatch增加exact Usage branch。
- `SettingsView`保存搜索过滤后的`active_page: Option<SettingsPageKey>`，并以唯一的非render transition helper维护它；收口现有导航赋值为`select_page`，让initial target、导航、search input与可能改变搜索匹配的i18n更新共享同一个pure resolver。render只消费已保存的active page，不启动query。
- `new_with_page`在页面与数据库页面实体初始化完成后显式同步一次actual active page；因此DB已经exact Ready时直接打开Usage会立即`activate`。actual active page从非Usage进入Usage时激活；每次明确重新选择Usage也强制刷新。
- actual active page从Usage离开时调用`deactivate`，取消running Operation并清除active range；settled data可保留供下一次exact-range stale展示。database observer离开Ready时走相同取消路径；恢复Ready且actual active page为Usage时重新activate。整体DB loading/error仍由既有`settings_database_resource_view`呈现。

### `L-532`：Period 与 exact local range

`UsageAnalyticsPeriod`固定顺序：Today、This week、This month、This year、All time；default为This month，不写入`app_settings`。

pure range helper显式接收`now_utc`与optional fixed local offset：

- finite period只捕获一次`now_utc`，并通过`UtcOffset::local_offset_at(now_utc)`取得同一instant的current offset，再计算local midnight half-open bounds；不得调用会再次取当前时刻的`current_local_offset()`。
- week使用Monday start；month/year使用checked next boundary。
- `AllTime`不调用local offset。
- local offset失败或date越界成为typed `UsageAnalyticsProblemSource`，不fallback UTC。

`UsagePeriodOption`实现`SelectItem<Value = UsageAnalyticsPeriod>`。I18n变化通过`set_items`和`set_selected_value`重建交互projection，不发业务selection事件。

### `ST-531`：Page-local refresh Operation

```rust
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

- `new`创建Idle + default month；自身不query。`SettingsView::new_with_page`完成owner初始化后的actual-page transition负责direct initial Usage load。
- `activate`重新捕获now/offset/range：Idle -> Load，Ready/Degraded -> Refresh，Unavailable -> Retry；running时不并发。
- `deactivate`向running Operation发送Cancel并清除active range；settled data保留，但只有下一次activate产生的range完全匹配时才可作为stale data展示。
- period Confirm值未改变时无动作；改变时先Cancel running Task，再更新authority/range并发起query。
- range构造结果与repository query都通过Operation-owned Task回到同一个`Complete`；Task只捕获weak page entity。
- `active_range`是当前request identity。Operation保留的data只有其snapshot range与active range完全相等时才可显示为stale；跨日/月/year或offset变化即使period enum相同也隐藏旧data。
- actual active page离开Usage、DB unavailable与Entity/window drop都取消Task；离开时同时清除active range。没有generation counter；取消的Task不能脱离Operation发布旧result。

UI状态严格映射root `ST-21`：initial/loading/retrying使用Skeleton；zero requests为empty；Ready为dashboard；same-range Refreshing保留data并显示轻量进度；same-range Degraded显示warning+Retry；range mismatch隐藏旧data并显示loading/blocking error；Unavailable显示Alert+Retry。

### `L-533`：Settings Usage composition

使用默认可vertical-scroll的`SettingsPageFrame`：

```text
[Title / description]                         [Period Select]

[Usage summary: coverage counts + six exact token totals]

[Daily total-token LineChart]                 # finite only

[Provider/model exact breakdown Table]
```

- summary是一个GroupBox，内部使用可换行grid，避免为每个数字建立装饰card。
- exact counts复用`foundation::conversation_format::format_token_count`；不得使用chat action row的compact formatter。
- LineChart使用dense daily local-date labels和`total_tokens as f64` visual projection；`.linear()`并放在240px高容器中，Today/Week/Month启用`.dot()`，This year不画逐日dot，不设置interactive id/tooltip。四个finite period的tick margin为1/1/5/30。
- chart容器提供localizedrole-Image summary；exact summary/table是文字等价信息。AllTime完全省略trend section。
- compact simple Table置于horizontal scroll中；列固定为Model、Provider、Requests、Input、Output、Cache read、Cache write。Reported、Unreported、Total covered、Reasoning与Total继续由上方summary展示，不在明细表重复。
- label missing或trim为空时回退stable ID；row identity仍为 `(provider_id, model_id)`。
- `request_count == 0`才empty；all-zero event显示Ready/unreported，不隐藏dashboard。

### `L-534`：Fluent、accessibility 与安全错误

- root列出的`settings-usage-*` keys在`en-US`/`zh-CN`完全parity；日期也通过Fluent变量格式化。
- Select、Alert、Button保留组件原生focus/keyboard；Select外层跟踪同一focus handle并暴露ComboBox label/value。summary、loading/refreshing status以及每个Table header/cell补显式本地化AX名称与唯一ID，不自制焦点状态。
- Retry是error state唯一动作；正常状态无Refresh按钮。
- UI只显示本地化通用load/refresh error；内部problem保留period、optional range与offset/calendar/DB source供诊断，绝不渲染raw `DbError`、SQL、payload或逐event数据。
- nav复用已经存在并有path test的typed `ChartNoAxesColumn`；Settings spec测试断言该variant，不修改`foundation/assets.rs`或新增SVG。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-531` | default month、period order与no persisted setting |
| `T-532` | Today positive/negative/sub-hour offset range |
| `T-533` | Monday week、month/year rollover、leap day、AllTime no offset |
| `T-534` | Settings ninth spec、position、icon与English/Chinese/pinyin search |
| `T-535` | DB已Ready时direct initial Usage activate、search-derived进入、reselect refresh与running dedupe |
| `T-536` | period change cancels old Task and queries exact new range |
| `T-537` | navigation/search-derived departure、DB unavailable/window drop cancellation and ready reactivation |
| `T-538` | same-range stale data retained; mismatched-range data hidden |
| `T-539` | empty/all-zero distinction与six exact summary fields |
| `T-540` | finite chart/AllTime no chart/provider-model table composition |
| `T-541` | error/degraded Retry and safe user-visible message |
| `T-542` | Fluent parity、i18n Select reprojection、AX chart summary与existing typed icon selection |

```sh
cargo fmt
cargo test -p jaco features::settings::usage::tests
cargo test -p jaco features::settings::tests
cargo test -p jaco i18n
cargo check -p jaco
cargo clippy -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

完成条件：root `ST-21`、`ERR-21`、`R-41`、`R-43`、`R-52`–`R-56`与`T-531`–`T-542`通过；五个period、summary/trend/breakdown和全部Operation states有自动化及人工证据，message/composer UI无回归。

## Settings 完成证据

- `WP-503`：`Implemented`。第九个Usage页面、五周期local-range builder、page-local refresh Operation、actual-active/DB readiness lifecycle、summary/trend/breakdown、状态映射、双语与显式AX文本已落地。
- `cargo test -p jaco features::settings::usage::tests --no-fail-fast`：15 passed，包含实际Usage page Entity/Window的single-flight、cancel、period/range/running stale-completion拒绝与compact breakdown列顺序；`cargo test -p jaco features::settings::tests --no-fail-fast`：16 passed；`cargo test -p jaco i18n --no-fail-fast`：11 passed。
- `cargo check -p jaco`、strict jaco clippy、`cargo fmt`与`git diff --check`通过。
- `cargo run -p xtask -- bundle jaco`通过；以临时`JACO_CONFIG_DIR`启动该bundle并预置4条隔离usage event，已验证fresh empty、reselect刷新、五个period、keyboard Select、finite trend、AllTime无trend、exact summary/provider-model rows、AX tree及search-derived进入/离开。
- 横向table scroll、error/degraded、运行中快速切period、现场语言切换、workspace-wide gates、three-platform CI与implementation commit/PR：`Pending`。隔离临时目录已删除；`actool`因本机CoreSimulator不可用跳过Liquid Glass图标注入，普通图标bundle成功。
