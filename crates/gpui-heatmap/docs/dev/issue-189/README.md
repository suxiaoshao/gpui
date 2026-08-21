# gpui-heatmap：Issue #189 活动热力图组件

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Settings activity heatmap](../../../../../docs/dev/issue-189/settings-usage-activity-heatmap-plan.md)
- Owner directory：`crates/gpui-heatmap`
- Owner status：`Implemented`（`WP-601` production component与聚焦自动化已完成；GUI人工矩阵、workspace-wide gates与三平台CI待执行）
- Assigned WP：`WP-601`
- Owns：产品无关的连续日期series、Monday activity grid、month labels、0 + 4级linear max色阶、gpui-component Plot tooltip、theme、size、scroll、caption/legend、whole-chart accessibility与组件测试
- Does not own：Jaco数据库查询、时区/rolling range、Fluent、Token业务语义、费用、provider/model、Settings Entity/Operation/GroupBox或empty/error状态

## Owner-local 证据与决定

- `E-601`：锁定的gpui-component `57a9903`没有heatmap/calendar-activity组件；Calendar是日期选择器，不是二维activity visualization。
- `E-602`：`Plot` + `#[derive(IntoPlot)]`提供stable-id、plot-relative hit test和即时pointer tooltip；`Tooltip`继承popover theme。
- `E-603`：`IntoPlot`生成的Element请求`Size::full()`，自身不实现`Styled`/`Sizable`/`InteractiveElement`，必须由明确宽高parent div承载。
- `E-604`：`ElementId`支持`From<(ElementId, T)>` child id；private plot可使用`(component_id, "plot")`避免同级冲突。
- `E-605`：`ActiveTheme`提供`secondary`、`chart_1`、`border`、`muted_foreground`和radius；无需crate palette。
- `E-606`：`TooltipState`只携带index/cross_line/dots，series index足以回查日期和值；无需per-cell state。
- `E-607`：`ScrollableElement` convenience API用source callsite构造scroll state/layer id；公开`Scrollbar::id(...)`和GPUI `window.use_keyed_state(...)`可以用caller component id建立多实例隔离。
- `D-601`：public data是start date + contiguous exact u64 values；拒绝empty/range overflow。
- `D-602`：week start固定Monday，不在首版公开unused定制API；月份label从该月第一个Monday所在week column开始。
- `D-603`：private Plot只绘制cell与处理hit/tooltip；month labels、caption、legend、Role::Image和style由外层RenderOnce组件负责。
- `D-604`：tooltip即时pointer显示date + exact value；不使用HoverCard、Entity、Task、timer、FocusHandle或action。
- `D-605`：主题每次render/paint读取；不缓存palette、不订阅theme、不开放自定义颜色API。
- `D-606`：整图一个image AX summary，无365个Tab stop/accessibility child。
- `D-607`：scroll只使用caller-id keyed `ScrollHandle`作为本地状态，显式渲染可设id的gpui-component `Scrollbar`；不使用callsite-keyed convenience wrapper。

## 文件与 ownership tree

```text
crates/gpui-heatmap/
├── Cargo.toml                         # F-601 [Modify] time + test-support
├── README.md                          # F-602 [Modify] stable API/example
├── README.zh-CN.md                    # F-603 [Modify] bilingual parity
├── src/
│   ├── lib.rs                         # F-604 [Modify] module/exports
│   └── activity.rs                    # F-605 [Add] data/component/plot/tests
└── docs/dev/
    ├── README.md                      # F-606 [Modify] Ready entry
    └── issue-189/
        └── README.md                  # F-607 [Modify] this owner plan
```

不新增`mod.rs`、example app、asset、Fluent、Store、Global或business-owned Entity type；scroll的keyed `ScrollHandle`不暴露为公开状态。

## `C-23`：公开API

`src/lib.rs`只导出稳定调用面：

```rust
mod activity;

pub use activity::{
    ActivityHeatmap,
    ActivityHeatmapLabels,
    ActivityHeatmapSeries,
    ActivityHeatmapSeriesError,
};
```

`src/activity.rs`落地：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityHeatmapSeries {
    start_date: Date,
    end_date: Date,
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
        start_date: Date,
        values: Vec<u64>,
    ) -> Result<Self, ActivityHeatmapSeriesError>;
    pub fn start_date(&self) -> Date;
    pub fn end_date(&self) -> Date;
    pub fn values(&self) -> &[u64];
    pub fn max_value(&self) -> u64;
}

#[derive(Clone)]
pub struct ActivityHeatmapLabels {
    pub months: [SharedString; 12],
    pub less: SharedString,
    pub more: SharedString,
    pub value: SharedString,
}

#[derive(IntoElement)]
pub struct ActivityHeatmap {
    id: ElementId,
    series: ActivityHeatmapSeries,
    labels: ActivityHeatmapLabels,
    accessible_summary: SharedString,
    caption: Option<SharedString>,
    size: gpui_component::Size,
    style: StyleRefinement,
    date_labels: Vec<SharedString>,
    value_labels: Vec<SharedString>,
}

impl ActivityHeatmap {
    pub fn new(
        id: impl Into<ElementId>,
        series: ActivityHeatmapSeries,
        labels: ActivityHeatmapLabels,
        accessible_summary: impl Into<SharedString>,
    ) -> Self;
    pub fn caption(self, caption: impl Into<SharedString>) -> Self;
    pub fn format_date(
        self,
        formatter: impl Fn(Date) -> SharedString,
    ) -> Self;
    pub fn format_value(
        self,
        formatter: impl Fn(u64) -> SharedString,
    ) -> Self;
}

// ActivityHeatmap implements Sizable, Styled, and RenderOnce.
```

### Constructor与failure contract

- `try_new`接收owned values；empty返回`Empty`。
- `len - 1`以checked转换到`i64`，再用`Date::checked_add(Duration::days(...))`计算end；失败返回`RangeOverflow`。
- `max_value`在构造时计算；不对值求和，避免组件发明overflow语义。
- error手写`fmt::Display + std::error::Error`；不增加`thiserror`。
- constructor预生成ISO `Date::to_string()`与完整十进制`u64::to_string()` label vectors。`format_date`/`format_value`立即遍历series并替换owned labels，不保存caller closure或要求`'static`；Jaco可借用当前I18n后安全返回组件。
- `ActivityHeatmap::new`要求accessible summary，避免出现无文字等价信息的组件实例。

## Layout contract

### Calendar grid

```text
leading_rows = start.weekday().number_days_from_monday()
slot_count   = leading_rows + values.len()
columns      = ceil(slot_count / 7)
row          = slot % 7          # Monday = 0
column       = slot / 7
```

- trailing padding补到完整最后一列；padding不映射series index。
- day index反向映射为`slot - leading_rows`；任何negative/out-of-range返回None。
- `ActivityHeatmapLabels.months`要求caller提供compact labels。月份label放在该月第一个Monday（`weekday == Monday && day <= 7`）所在column，且从该column到月末在范围内至少覆盖两个week columns；因此跨月混合周仍归前一个月，首个partial month与final one-column month不生成label，避免右侧裁切。
- 不显示weekday gutter。月份label、grid、footer共享固定content width并整体horizontal scroll。

### Size contract

| `gpui_component::Size` | cell | gap |
| --- | ---: | ---: |
| `XSmall` | 8px | 2px |
| `Small` | 10px | 2px |
| `Medium`（default） | 12px | 3px |
| `Large` | 14px | 4px |
| `Size(cell)` | `clamp(finite cell, 4px, 32px)` | `clamp(cell * 0.25, 2px, 4px)` |

non-finite custom pixels按4px处理，再应用4–32px clamp。`grid_width = columns * cell + (columns - 1) * gap`，`grid_height = 7 * cell + 6 * gap`。private Plot放入精确`.w(grid_width).h(grid_height)`的parent div；`Styled`只应用到component外层。cell radius为`min(theme.radius, cell / 4)`。

footer允许wrap：optional caption在左，`Less + level 0..4 swatches + More`在右。caption是caller提供的presentation string，component不计算业务合计。

## Level、theme与paint contract

```rust
fn level(value: u64, max: u64) -> u8 {
    if value == 0 { return 0; }
    let level = (u128::from(value) * 4)
        .div_ceil(u128::from(max.max(1)))
        .clamp(1, 4);
    level as u8
}
```

level只使用u128整数运算，按series最大值线性归一化且不使用最小值；tooltip/caption/AX由exact u64文本产生。颜色每次`render`/`paint`/`tooltip`读取当前`cx.theme()`：

| level | color |
| --- | --- |
| 0 | `secondary` |
| 1 | `chart_1.opacity(0.25)` |
| 2 | `chart_1.opacity(0.45)` |
| 3 | `chart_1.opacity(0.70)` |
| 4 | `chart_1` |

outline使用`border`，month/legend/caption使用`muted_foreground`和正常foreground层级。`Tooltip::row`必须传resolved `Hsla`；不传`ThemeToken`或gradient `Background`。

## Plot、tooltip与accessibility contract

```rust
#[derive(gpui_component::plot::IntoPlot)]
struct ActivityHeatmapPlot {
    plot_id: ElementId,
    // immutable layout/series/formatters
}
```

- render时以`let plot_id: ElementId = (component_id.clone(), "plot").into();`生成private plot id，`Plot::id()`原样返回。
- `paint`只画真实cells；padding留空。每格使用`window.paint_quad(gpui::quad(cell_bounds, radius, level_hsla, px(1.), border, BorderStyle::default()))`；几何与hit test复用同一个纯layout结果，禁止两套坐标计算。
- scroll不直接调用`.overflow_x_scrollbar()`或`.horizontal_scrollbar()`；两个convenience API的state/layer id来自source callsite，同一render callsite下的多实例无法仅靠输入Div的id隔离。
- render以caller `component_id`派生`scroll-state`/`scroll-root`/`scroll-area`/`scroll-content`/`scrollbar`稳定child id。`window.use_keyed_state(scroll_state_id, cx, |_, _| ScrollHandle::default())`创建唯一的组件本地状态；scroll area显式`.track_scroll(&scroll_handle).overflow_x_scroll()`，固定宽content是其唯一chart child。
- scroll root使用relative overlay结构；非inspector picking时显式渲染`Scrollbar::horizontal(&scroll_handle).id(scrollbar_id)`，从而复用gpui-component theme、hover、drag与fade行为。组件不自定义scrollbar样式或复制其交互。
- `tooltip_state(position, bounds, cx)`先拒绝bounds外，再按stride定位column/row并拒绝gap/padding；返回series index，以cell中心填必需的`cross_line`字段并使用空dots。
- `tooltip`以index回查date/value，返回`Tooltip::new(cursor, bounds.size).title(formatted_date).row(level_hsla, labels.value, formatted_exact_value)`；不画cross-line或dots。
- outer stateful div持有caller id、`Role::Image`和required accessible summary；Plot不持有role/aria。
- component无`.tab_stop()`、FocusHandle、action/keybinding、mouse/click/hover state、业务Entity、Task、自定义timer或subscription；唯一keyed state是上述`ScrollHandle`，gpui-component `Scrollbar`内部的标准fade timer不在本crate重复实现。

### Scroll tooltip风险

Plot tooltip的deferred box仍继承horizontal scroll content mask。未滚动和滚动到最右时，可视区域左右端的tooltip必须完整显示。若人工/GPUI验证发现裁切，`WP-601`必须在component内部修正tooltip定位/overlay边界；Jaco consumer不得加局部兜底。

## Cargo与dependency contract

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
time = "0.3.54"

[dev-dependencies]
gpui = { workspace = true, features = ["test-support"] }
```

无新UI/chart/calendar/formatting/error dependency。normal + dev的gpui声明沿用`crates/gpui-store`等当前workspace已经验证的同一模式；不得把`test-support`并入production feature或引入test helper crate。

## `WP-601`：实现活动热力图（Implemented）

1. 修改manifest和`lib.rs`，建立`activity.rs`与`C-23`。
2. 实现series validation、pure layout、size mapping与level mapping。
3. 实现outer RenderOnce、month labels、caller-id keyed `ScrollHandle`、显式id的gpui-component `Scrollbar`、caption、legend与theme。
4. 实现private Plot paint/hit test/Tooltip和single-image AX。
5. 更新双语README，给出product-neutral构造示例和caller responsibilities。
6. 完成focused automated/manual validation，记录实际API差异和证据。

依赖：当前crate骨架。可与`jaco-db/WP-204`并行；完成后解锁`app/jaco/WP-504`。

## Tests

| ID | 类型 | 场景 |
| --- | --- | --- |
| `T-601` | pure | empty/range overflow/end date/max cache |
| `T-602` | pure | Monday layout、53-week、cross-year、leap day、padding |
| `T-603` | pure | month first-Monday columns、跨月混合周归前月、partial first与final one-column omission |
| `T-604` | pure | all-zero/single/equal/outlier/u64::MAX levels |
| `T-605` | pure | all Size variants与custom clamp/grid bounds |
| `T-606` | pure | cell/gap/padding/outside hit test，zero cell可命中 |
| `T-607` | GPUI | stable child plot/scroll ids与render smoke |
| `T-608` | GPUI | tooltip date/value exact，theme colors在rerender读取 |
| `T-609` | GPUI/AX | 同一source callsite的两实例scroll offset隔离并在rerender后各自保留；outer只有一个Role::Image summary且cells不进Tab sequence |

## Focused validation

```sh
cargo fmt
cargo test -p gpui-heatmap
cargo clippy -p gpui-heatmap --all-targets --all-features -- -D warnings
cargo check -p gpui-heatmap
git diff --check -- crates/gpui-heatmap Cargo.toml Cargo.lock
```

人工阻断矩阵：light/dark/system、en-US/zh-CN caller labels、常规/窄宽度、未滚动两端、滚动最右后的可视两端tooltip、Role::Image/Tab序列。

## 完成条件

- `C-23`、layout、level、theme、tooltip与AX contract全部落地，无app/business dependency。
- `T-601`–`T-609`通过；滚动tooltip阻断矩阵无裁切。
- README与owner/root plans同步实际稳定API。
- 记录implementation commit/PR和未执行的workspace/three-platform gates。

## 实施证据

- 2026-08-21：workspace member、crate基础文件与owner文档骨架已建立；`cargo check -p gpui-heatmap`通过。
- 2026-08-21：`ActivityHeatmapSeries`、公开组件、Monday-first布局、月份标记、精确tooltip、theme、caption/legend、single-image AX及caller-id keyed滚动已实现，API与`C-23`一致。
- `cargo test -p gpui-heatmap`通过10项；`cargo check -p gpui-heatmap`、`cargo clippy -p gpui-heatmap --all-targets --all-features -- -D warnings`与scoped diff check通过。
- GPUI测试覆盖render/theme/tooltip smoke和同一callsite双实例滚动隔离；完整窗口中的横向滚动两端tooltip与Accessibility Inspector矩阵因当前macOS图形会话锁定而待执行。
- implementation commit / PR、workspace-wide gates与三平台CI：`Pending`。
