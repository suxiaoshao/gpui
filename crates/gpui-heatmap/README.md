# gpui-heatmap

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-heatmap` is the workspace-owned home for reusable heatmap components built with GPUI.

The crate owns presentation and interaction only. Applications remain responsible for querying and aggregating values, selecting date ranges and time zones, and defining product-specific meaning such as token usage or cost.

## Activity heatmap

`ActivityHeatmap` renders a contiguous series of exact daily `u64` values as a Monday-first calendar grid. It provides:

- zero plus four linear activity levels normalized to the series maximum;
- colors, radius, typography, tooltip, sizing, and scrollbar behavior from `gpui-component`;
- immediate pointer tooltips with caller-formatted dates and exact values;
- fixed-size cells with horizontal scrolling at narrow widths;
- one caller-provided accessible image summary, without per-day keyboard stops.

```rust,no_run
use gpui::{SharedString, px};
use gpui_component::Sizable as _;
use gpui_heatmap::{ActivityHeatmap, ActivityHeatmapLabels, ActivityHeatmapSeries};
use time::{Date, Month};

let start = Date::from_calendar_date(2025, Month::January, 1)?;
let series = ActivityHeatmapSeries::try_new(start, vec![0, 12, 340])?;
let labels = ActivityHeatmapLabels {
    months: std::array::from_fn(|month| SharedString::from(format!("M{}", month + 1))),
    less: "Less".into(),
    more: "More".into(),
    value: "Events".into(),
};

let heatmap = ActivityHeatmap::new(
    "yearly-activity",
    series,
    labels,
    "Activity from January 1 to January 3: 352 events",
)
.caption("352 events")
.format_date(|date| date.to_string().into())
.format_value(|value| value.to_string().into())
.with_size(px(12.));
# Ok::<_, Box<dyn std::error::Error>>(heatmap)
```

The caller owns the date range and time-zone conversion, localized month and legend labels, exact number formatting, caption, and complete accessibility summary. The component requires a non-empty contiguous series and reports date-range overflow explicitly.

## Development

- [Development plan index](docs/dev/README.md)
- [Issue #189 owner plan](docs/dev/issue-189/README.md)
