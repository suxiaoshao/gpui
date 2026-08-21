# gpui-heatmap

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-heatmap` 是 workspace 中可复用 GPUI 热力图组件的归属 crate。

本 crate 只负责热力图的展示与交互。数据查询与聚合、日期范围和时区选择，以及 Token 用量、费用等产品语义继续由调用方负责。

## 活动热力图

`ActivityHeatmap` 将一段连续日期的精确每日 `u64` 数值渲染为周一开周的日历网格，提供：

- 零值加四个按序列最大值线性归一化的活动等级；
- 来自 `gpui-component` 的颜色、圆角、文字、tooltip、尺寸和滚动条行为；
- 使用调用方格式化日期及精确数值的即时 pointer tooltip；
- 固定可读 cell，窄宽度下横向滚动；
- 一个由调用方提供的整图无障碍摘要，不为每日 cell 添加键盘停靠点。

```rust,no_run
use gpui::{SharedString, px};
use gpui_component::Sizable as _;
use gpui_heatmap::{ActivityHeatmap, ActivityHeatmapLabels, ActivityHeatmapSeries};
use time::{Date, Month};

let start = Date::from_calendar_date(2025, Month::January, 1)?;
let series = ActivityHeatmapSeries::try_new(start, vec![0, 12, 340])?;
let labels = ActivityHeatmapLabels {
    months: std::array::from_fn(|month| SharedString::from(format!("{}月", month + 1))),
    less: "较少".into(),
    more: "较多".into(),
    value: "事件".into(),
};

let heatmap = ActivityHeatmap::new(
    "yearly-activity",
    series,
    labels,
    "1月1日至1月3日的活动：352个事件",
)
.caption("352个事件")
.format_date(|date| date.to_string().into())
.format_value(|value| value.to_string().into())
.with_size(px(12.));
# Ok::<_, Box<dyn std::error::Error>>(heatmap)
```

调用方负责日期范围和时区转换、本地化月份与图例文案、精确数字格式、caption，以及完整的整图无障碍摘要。组件要求输入非空的连续日期序列，并明确返回日期范围越界错误。

## 开发

- [开发计划索引](docs/dev/README.md)
- [Issue #189 owner 计划](docs/dev/issue-189/README.md)
