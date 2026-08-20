# Jaco：在 Agent message action row 展示单次请求用量

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)
- Owner directory：`app/jaco`
- Owner status：`In progress`
- 消费 root IDs：`C-01`–`C-03`、`D-05`、`D-06`、`D-08`、`ST-01`、`R-01`–`R-13`
- Assigned WP：`WP-501`
- Owns：Conversation effect消费、timeline row projection、request usage formatter、原生HoverCard/DescriptionList、typed icon、Fluent、UI tests/manual validation
- Does not own：DB association、usage authority/聚合、provider raw parsing、composer context、Settings analytics

## Owner-local 证据与决定

- `E-501`：`ConversationDetailPage::sync_timeline` 以Ready `Conversation` 构造rows；页面已能按run ID精确update/remeasure。
- `E-502`：`timeline::agent_turn_row` 是 `AgentTurnRow` 唯一构造入口。
- `E-503`：`agent_action_row` 当前为24px高、760px max width、Copy + hover time。
- `E-504`：`CopyButton` 已采用ghost/xsmall/icon/tooltip模式。
- `E-505`：当前gpui-component `HoverCard`原生负责window overlay、trigger/content hover、默认600ms open delay、300ms close delay与timer cancellation；`DescriptionList`适合单列key-value；HoverCard明确是pointer-only。
- `D-501`：request usage以plain cloned projection传入row；app不为详情新增open/hover/pinned/focus Entity、Store、Global、cache、Task或subscription。
- `D-502`：新增专用 `request_usage.rs`，不抽象为composer/settings共用组件。
- `D-503`：usage图标始终可hover；只有Reported total摘要与完成时间采用相同字体、字号、颜色与message group-hover时机，摘要以`k`/`M`等紧凑形式显示且位于trigger之外，不触发详情；只有图标按HoverCard组件默认延迟展开保留完整整数的Alma式详情；partial/unreported/unavailable不显示未知total或0，只保留图标与详情状态。
- `D-504`：详情使用原生pointer-only HoverCard；不提供click固定、Escape、键盘打开或focus return，app不重复组件已有的hover timing/state/Task。

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
- workspace build/test/strict clippy、`cargo fmt` 与 `git diff --check` 通过；chat form、Settings、TTFT/TPS与provider metadata均未接入。
- 最终HoverCard交互已由用户检查确认符合预期。真实provider请求与三平台CI尚未执行，因此owner状态保留`In progress`。
