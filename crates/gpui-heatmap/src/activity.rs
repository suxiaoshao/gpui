use std::{error::Error, fmt};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, BorderStyle, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, RenderOnce, Role, ScrollHandle, SharedString,
    Size as GpuiSize, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, point,
    px, quad,
};
use gpui_component::{
    ActiveTheme as _, Sizable, Size, StyledExt as _,
    plot::{
        IntoPlot, Plot,
        tooltip::{Tooltip, TooltipState},
    },
    scroll::Scrollbar,
    v_flex,
};
use time::{Date, Duration};

const ROWS: usize = 7;
const LEVEL_COUNT: usize = 5;
const MAX_ACTIVITY_LEVEL: u8 = (LEVEL_COUNT - 1) as u8;
const MONTH_LABEL_HEIGHT: Pixels = px(20.);
const FOOTER_GAP: Pixels = px(8.);

/// A contiguous series of daily activity values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityHeatmapSeries {
    start_date: Date,
    end_date: Date,
    values: Vec<u64>,
    max_value: u64,
}

/// Validation errors for [`ActivityHeatmapSeries`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityHeatmapSeriesError {
    /// At least one day is required.
    Empty,
    /// The number of values cannot be represented as a date range starting at the given date.
    RangeOverflow,
}

impl fmt::Display for ActivityHeatmapSeriesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("activity heatmap series cannot be empty"),
            Self::RangeOverflow => {
                formatter.write_str("activity heatmap series exceeds the supported date range")
            }
        }
    }
}

impl Error for ActivityHeatmapSeriesError {}

impl ActivityHeatmapSeries {
    /// Constructs a contiguous series beginning at `start_date`.
    pub fn try_new(start_date: Date, values: Vec<u64>) -> Result<Self, ActivityHeatmapSeriesError> {
        let day_offset = values
            .len()
            .checked_sub(1)
            .ok_or(ActivityHeatmapSeriesError::Empty)?;
        let day_offset =
            i64::try_from(day_offset).map_err(|_| ActivityHeatmapSeriesError::RangeOverflow)?;
        let end_date = start_date
            .checked_add(Duration::days(day_offset))
            .ok_or(ActivityHeatmapSeriesError::RangeOverflow)?;
        let max_value = values.iter().copied().max().unwrap_or_default();

        Ok(Self {
            start_date,
            end_date,
            values,
            max_value,
        })
    }

    pub fn start_date(&self) -> Date {
        self.start_date
    }

    pub fn end_date(&self) -> Date {
        self.end_date
    }

    pub fn values(&self) -> &[u64] {
        &self.values
    }

    pub fn max_value(&self) -> u64 {
        self.max_value
    }
}

/// Caller-owned labels used by [`ActivityHeatmap`].
#[derive(Clone)]
pub struct ActivityHeatmapLabels {
    pub months: [SharedString; 12],
    pub less: SharedString,
    pub more: SharedString,
    pub value: SharedString,
}

/// A reusable, product-neutral calendar activity heatmap.
#[derive(IntoElement)]
pub struct ActivityHeatmap {
    id: ElementId,
    series: ActivityHeatmapSeries,
    labels: ActivityHeatmapLabels,
    accessible_summary: SharedString,
    caption: Option<SharedString>,
    size: Size,
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
    ) -> Self {
        let date_labels = series_dates(&series)
            .map(|date| date.to_string().into())
            .collect();
        let value_labels = series
            .values()
            .iter()
            .map(|value| value.to_string().into())
            .collect();

        Self {
            id: id.into(),
            series,
            labels,
            accessible_summary: accessible_summary.into(),
            caption: None,
            size: Size::default(),
            style: StyleRefinement::default(),
            date_labels,
            value_labels,
        }
    }

    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Formats every date immediately and stores only the resulting owned labels.
    pub fn format_date(mut self, formatter: impl Fn(Date) -> SharedString) -> Self {
        self.date_labels = series_dates(&self.series).map(formatter).collect();
        self
    }

    /// Formats every exact value immediately and stores only the resulting owned labels.
    pub fn format_value(mut self, formatter: impl Fn(u64) -> SharedString) -> Self {
        self.value_labels = self
            .series
            .values()
            .iter()
            .copied()
            .map(formatter)
            .collect();
        self
    }
}

impl Styled for ActivityHeatmap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for ActivityHeatmap {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for ActivityHeatmap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let metrics = HeatmapMetrics::for_size(self.size);
        let layout = HeatmapLayout::new(&self.series, metrics);
        let plot_id = child_id(&self.id, "plot");
        let scroll_state_id = child_id(&self.id, "scroll-state");
        let scroll_root_id = child_id(&self.id, "scroll-root");
        let scroll_area_id = child_id(&self.id, "scroll-area");
        let scroll_content_id = child_id(&self.id, "scroll-content");
        let scrollbar_id = child_id(&self.id, "scrollbar");

        let scroll_state =
            window.use_keyed_state(scroll_state_id, cx, |_, _| ScrollHandle::default());
        let scroll_handle = scroll_state.read(cx).clone();
        let month_markers = month_markers(&self.series, &layout);
        let month_labels = self.labels.months.clone();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let palette = level_palette(cx);
        let legend_cell = metrics.cell.min(px(12.));
        let legend_radius = cx.theme().radius.min(legend_cell / 4.);

        let plot = ActivityHeatmapPlot {
            id: plot_id,
            series: self.series.clone(),
            layout: layout.clone(),
            date_labels: self.date_labels,
            value_labels: self.value_labels,
            value_label: self.labels.value.clone(),
        };

        let months = div()
            .relative()
            .w(layout.grid_width)
            .h(MONTH_LABEL_HEIGHT)
            .children(month_markers.into_iter().map(|marker| {
                div()
                    .absolute()
                    .left(layout.column_x(marker.column))
                    .top_0()
                    .text_xs()
                    .text_color(muted)
                    .child(month_labels[marker.month_index].clone())
            }));

        let grid = div().w(layout.grid_width).h(layout.grid_height).child(plot);

        let legend = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(muted)
            .child(self.labels.less)
            .children(palette.into_iter().map(|color| {
                div()
                    .size(legend_cell)
                    .rounded(legend_radius)
                    .border_1()
                    .border_color(border)
                    .bg(color)
            }))
            .child(self.labels.more);

        let footer = div()
            .w(layout.grid_width)
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(FOOTER_GAP)
            .text_xs()
            .text_color(muted)
            .when_some(self.caption, |this, caption| this.child(caption))
            .child(legend);

        let content = v_flex()
            .id(scroll_content_id)
            .w(layout.grid_width)
            .flex_none()
            .gap_2()
            .child(months)
            .child(grid)
            .child(footer);

        let scroll_area = div()
            .id(scroll_area_id)
            .w_full()
            .flex()
            .flex_row()
            .track_scroll(&scroll_handle)
            .overflow_x_scroll()
            .child(content);

        let scroll_root = div()
            .id(scroll_root_id)
            .relative()
            .w_full()
            .overflow_hidden()
            .child(scroll_area)
            .when(!window.is_inspector_picking(cx), |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .child(Scrollbar::horizontal(&scroll_handle).id(scrollbar_id)),
                )
            });

        div()
            .id(self.id)
            .role(Role::Image)
            .aria_label(self.accessible_summary)
            .w_full()
            .refine_style(&self.style)
            .child(scroll_root)
    }
}

fn child_id(parent: &ElementId, child: &'static str) -> ElementId {
    (parent.clone(), child).into()
}

fn series_dates(series: &ActivityHeatmapSeries) -> impl Iterator<Item = Date> + '_ {
    (0..series.values.len()).map(|index| {
        let offset = i64::try_from(index).expect("validated series length fits i64");
        series
            .start_date
            .checked_add(Duration::days(offset))
            .expect("validated series dates remain in range")
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HeatmapMetrics {
    cell: Pixels,
    gap: Pixels,
}

impl HeatmapMetrics {
    fn for_size(size: Size) -> Self {
        match size {
            Size::XSmall => Self::new(8., 2.),
            Size::Small => Self::new(10., 2.),
            Size::Medium => Self::new(12., 3.),
            Size::Large => Self::new(14., 4.),
            Size::Size(cell) => {
                let cell = if cell.as_f32().is_finite() {
                    cell.as_f32().clamp(4., 32.)
                } else {
                    4.
                };
                Self::new(cell, (cell * 0.25).clamp(2., 4.))
            }
        }
    }

    fn new(cell: f32, gap: f32) -> Self {
        Self {
            cell: px(cell),
            gap: px(gap),
        }
    }

    fn stride(self) -> Pixels {
        self.cell + self.gap
    }
}

#[derive(Clone, Debug, PartialEq)]
struct HeatmapLayout {
    leading_rows: usize,
    columns: usize,
    values_len: usize,
    metrics: HeatmapMetrics,
    grid_width: Pixels,
    grid_height: Pixels,
}

impl HeatmapLayout {
    fn new(series: &ActivityHeatmapSeries, metrics: HeatmapMetrics) -> Self {
        let leading_rows = usize::from(series.start_date.weekday().number_days_from_monday());
        let slot_count = leading_rows + series.values.len();
        let columns = slot_count.div_ceil(ROWS);
        let grid_width = dimension(columns, metrics);
        let grid_height = dimension(ROWS, metrics);
        Self {
            leading_rows,
            columns,
            values_len: series.values.len(),
            metrics,
            grid_width,
            grid_height,
        }
    }

    fn column_x(&self, column: usize) -> Pixels {
        self.metrics.stride() * column as f32
    }

    fn row_y(&self, row: usize) -> Pixels {
        self.metrics.stride() * row as f32
    }

    fn slot_for_index(&self, index: usize) -> usize {
        self.leading_rows + index
    }

    fn cell_position(&self, index: usize) -> (usize, usize) {
        let slot = self.slot_for_index(index);
        (slot / ROWS, slot % ROWS)
    }

    fn cell_bounds(&self, index: usize, origin: Point<Pixels>) -> Bounds<Pixels> {
        let (column, row) = self.cell_position(index);
        Bounds {
            origin: origin + point(self.column_x(column), self.row_y(row)),
            size: GpuiSize::new(self.metrics.cell, self.metrics.cell),
        }
    }

    fn hit_test(&self, position: Point<Pixels>, plot_size: GpuiSize<Pixels>) -> Option<usize> {
        if position.x < px(0.)
            || position.y < px(0.)
            || position.x >= plot_size.width
            || position.y >= plot_size.height
        {
            return None;
        }
        let stride = self.metrics.stride().as_f32();
        let x = position.x.as_f32();
        let y = position.y.as_f32();
        let column = (x / stride).floor() as usize;
        let row = (y / stride).floor() as usize;
        if column >= self.columns || row >= ROWS {
            return None;
        }
        if x - column as f32 * stride >= self.metrics.cell.as_f32()
            || y - row as f32 * stride >= self.metrics.cell.as_f32()
        {
            return None;
        }
        let slot = column * ROWS + row;
        let index = slot.checked_sub(self.leading_rows)?;
        (index < self.values_len).then_some(index)
    }
}

fn dimension(count: usize, metrics: HeatmapMetrics) -> Pixels {
    if count == 0 {
        return px(0.);
    }
    metrics.cell * count as f32 + metrics.gap * count.saturating_sub(1) as f32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MonthMarker {
    month_index: usize,
    column: usize,
}

fn month_markers(series: &ActivityHeatmapSeries, layout: &HeatmapLayout) -> Vec<MonthMarker> {
    series_dates(series)
        .enumerate()
        .filter(|(_, date)| date.weekday() == time::Weekday::Monday && date.day() <= 7)
        .filter_map(|(index, date)| {
            let start_column = layout.cell_position(index).0;
            let month_index = usize::from(u8::from(date.month()) - 1);
            let next_month = if date.month() == time::Month::December {
                Date::from_calendar_date(date.year().checked_add(1)?, time::Month::January, 1).ok()
            } else {
                Date::from_calendar_date(date.year(), date.month().next(), 1).ok()
            };
            let visible_end = next_month
                .and_then(Date::previous_day)
                .unwrap_or(series.end_date)
                .min(series.end_date);
            let visible_days = (visible_end - date).whole_days();
            let end_index = index.checked_add(usize::try_from(visible_days).ok()?)?;
            let end_column = layout.cell_position(end_index).0;
            (end_column > start_column).then_some(MonthMarker {
                month_index,
                column: start_column,
            })
        })
        .collect()
}

fn activity_level(value: u64, max_value: u64) -> u8 {
    if value == 0 {
        return 0;
    }

    let level = (u128::from(value) * u128::from(MAX_ACTIVITY_LEVEL))
        .div_ceil(u128::from(max_value.max(1)))
        .clamp(1, u128::from(MAX_ACTIVITY_LEVEL));
    level as u8
}

fn level_palette(cx: &App) -> [Hsla; LEVEL_COUNT] {
    let active = cx.theme().chart_1;
    [
        cx.theme().secondary,
        active.opacity(0.25),
        active.opacity(0.45),
        active.opacity(0.70),
        active,
    ]
}

#[derive(IntoPlot)]
struct ActivityHeatmapPlot {
    id: ElementId,
    series: ActivityHeatmapSeries,
    layout: HeatmapLayout,
    date_labels: Vec<SharedString>,
    value_labels: Vec<SharedString>,
    value_label: SharedString,
}

impl Plot for ActivityHeatmapPlot {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let palette = level_palette(cx);
        let radius = cx.theme().radius.min(self.layout.metrics.cell / 4.);
        for (index, value) in self.series.values().iter().copied().enumerate() {
            let level = usize::from(activity_level(value, self.series.max_value()));
            window.paint_quad(quad(
                self.layout.cell_bounds(index, bounds.origin),
                radius,
                palette[level],
                px(1.),
                cx.theme().border,
                BorderStyle::default(),
            ));
        }
    }

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _cx: &App,
    ) -> Option<TooltipState> {
        let index = self.layout.hit_test(position, bounds.size)?;
        let cell = self.layout.cell_bounds(index, point(px(0.), px(0.)));
        Some(TooltipState::new(index, cell.center(), Vec::new()))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let value = *self.series.values().get(state.index)?;
        let level = usize::from(activity_level(value, self.series.max_value()));
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(self.date_labels.get(state.index)?.clone())
                .row(
                    level_palette(cx)[level],
                    self.value_label.clone(),
                    self.value_labels.get(state.index)?.clone(),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Entity, Render, TestAppContext, VisualTestContext, Window, rgb};
    use gpui_component::Theme;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).unwrap()
    }

    fn series(start: Date, values: Vec<u64>) -> ActivityHeatmapSeries {
        ActivityHeatmapSeries::try_new(start, values).unwrap()
    }

    #[test]
    fn series_validates_range_and_caches_end_and_max() {
        assert_eq!(
            ActivityHeatmapSeries::try_new(date(2024, Month::January, 1), Vec::new()),
            Err(ActivityHeatmapSeriesError::Empty)
        );
        assert_eq!(
            ActivityHeatmapSeries::try_new(Date::MAX, vec![1, 2]),
            Err(ActivityHeatmapSeriesError::RangeOverflow)
        );
        let series = series(date(2024, Month::February, 28), vec![2, 9, 4]);
        assert_eq!(series.end_date(), date(2024, Month::March, 1));
        assert_eq!(series.max_value(), 9);
        assert_eq!(series.values(), &[2, 9, 4]);
    }

    #[test]
    fn monday_layout_handles_cross_year_leap_days_and_padding() {
        let yearly_series = series(date(2023, Month::December, 31), vec![0; 365]);
        let layout = HeatmapLayout::new(&yearly_series, HeatmapMetrics::for_size(Size::Medium));
        assert_eq!(layout.leading_rows, 6);
        assert_eq!(layout.columns, 53);
        assert_eq!(layout.cell_position(0), (0, 6));
        assert_eq!(layout.cell_position(1), (1, 0));
        assert_eq!(yearly_series.end_date(), date(2024, Month::December, 29));

        let leap = series(date(2024, Month::February, 28), vec![0; 3]);
        assert_eq!(leap.end_date(), date(2024, Month::March, 1));
        let leap_layout = HeatmapLayout::new(&leap, HeatmapMetrics::for_size(Size::Medium));
        assert_eq!(leap_layout.leading_rows, 2);
        assert_eq!(leap_layout.cell_position(1), (0, 3));
    }

    #[test]
    fn month_markers_start_on_the_first_monday_inside_the_month() {
        let partial = series(date(2024, Month::January, 15), vec![0; 48]);
        let layout = HeatmapLayout::new(&partial, HeatmapMetrics::for_size(Size::Medium));
        let markers = month_markers(&partial, &layout);
        assert_eq!(markers.len(), 1);
        assert_eq!(
            markers[0].month_index,
            usize::from(u8::from(Month::February) - 1)
        );
        assert_eq!(markers[0].column, layout.cell_position(21).0);

        let mixed_week = series(date(2026, Month::July, 20), vec![0; 48]);
        let layout = HeatmapLayout::new(&mixed_week, HeatmapMetrics::for_size(Size::Medium));
        let markers = month_markers(&mixed_week, &layout);
        let august = markers
            .iter()
            .find(|marker| marker.month_index == usize::from(u8::from(Month::August) - 1))
            .expect("August marker should be visible");
        assert_eq!(august.column, layout.cell_position(14).0);
        assert_ne!(august.column, layout.cell_position(12).0);

        let one_column = series(date(2024, Month::March, 4), vec![0; 3]);
        let layout = HeatmapLayout::new(&one_column, HeatmapMetrics::for_size(Size::Medium));
        assert!(month_markers(&one_column, &layout).is_empty());
    }

    #[test]
    fn linear_levels_use_maximum_quarter_bands_without_overflow() {
        assert_eq!(activity_level(0, 0), 0);
        assert_eq!(activity_level(1, 1), 4);
        assert_eq!(activity_level(7, 7), 4);
        assert_eq!(activity_level(25, 100), 1);
        assert_eq!(activity_level(26, 100), 2);
        assert_eq!(activity_level(50, 100), 2);
        assert_eq!(activity_level(51, 100), 3);
        assert_eq!(activity_level(75, 100), 3);
        assert_eq!(activity_level(76, 100), 4);
        assert_eq!(activity_level(100, 100), 4);
        assert_eq!(activity_level(1, 1_000_000), 1);
        assert_eq!(activity_level(u64::MAX / 4, u64::MAX), 1);
        assert_eq!(activity_level(u64::MAX / 4 + 1, u64::MAX), 2);
        assert_eq!(activity_level(u64::MAX, u64::MAX), 4);
    }

    #[test]
    fn sizes_map_and_custom_values_clamp() {
        for (size, expected) in [
            (Size::XSmall, HeatmapMetrics::new(8., 2.)),
            (Size::Small, HeatmapMetrics::new(10., 2.)),
            (Size::Medium, HeatmapMetrics::new(12., 3.)),
            (Size::Large, HeatmapMetrics::new(14., 4.)),
        ] {
            assert_eq!(HeatmapMetrics::for_size(size), expected);
            let layout =
                HeatmapLayout::new(&series(date(2024, Month::January, 1), vec![0; 8]), expected);
            assert_eq!(layout.grid_width, expected.cell * 2. + expected.gap);
            assert_eq!(layout.grid_height, expected.cell * 7. + expected.gap * 6.);
        }
        assert_eq!(
            HeatmapMetrics::for_size(Size::Size(px(1.))),
            HeatmapMetrics::new(4., 2.)
        );
        assert_eq!(
            HeatmapMetrics::for_size(Size::Size(px(100.))),
            HeatmapMetrics::new(32., 4.)
        );
        assert_eq!(
            HeatmapMetrics::for_size(Size::Size(px(f32::NAN))),
            HeatmapMetrics::new(4., 2.)
        );

        let layout = HeatmapLayout::new(
            &series(date(2024, Month::January, 1), vec![0; 8]),
            HeatmapMetrics::new(12., 3.),
        );
        assert_eq!(layout.grid_width, px(27.));
        assert_eq!(layout.grid_height, px(102.));
    }

    #[test]
    fn hit_test_rejects_gaps_padding_and_outside_but_accepts_zero_cells() {
        let series = series(date(2024, Month::January, 3), vec![0, 5]);
        let layout = HeatmapLayout::new(&series, HeatmapMetrics::new(12., 3.));
        let size = GpuiSize::new(layout.grid_width, layout.grid_height);
        assert_eq!(layout.hit_test(point(px(1.), px(31.)), size), Some(0));
        assert_eq!(layout.hit_test(point(px(1.), px(46.)), size), Some(1));
        assert_eq!(layout.hit_test(point(px(1.), px(1.)), size), None);
        assert_eq!(layout.hit_test(point(px(1.), px(61.)), size), None);
        assert_eq!(layout.hit_test(point(px(12.5), px(31.)), size), None);
        assert_eq!(layout.hit_test(point(px(-1.), px(31.)), size), None);
        assert_eq!(
            layout.hit_test(point(layout.grid_width, px(31.)), size),
            None
        );
    }

    #[test]
    fn formatter_builders_eagerly_replace_owned_labels() {
        let labels = ActivityHeatmapLabels {
            months: std::array::from_fn(|index| format!("M{}", index + 1).into()),
            less: "Less".into(),
            more: "More".into(),
            value: "Value".into(),
        };
        let prefix = String::from("date");
        let heatmap = ActivityHeatmap::new(
            "heatmap",
            series(date(2024, Month::January, 1), vec![3]),
            labels,
            "summary",
        )
        .format_date(|date| format!("{prefix}:{date}").into())
        .format_value(|value| format!("{value} exact").into());
        drop(prefix);
        assert_eq!(heatmap.date_labels, vec!["date:2024-01-01"]);
        assert_eq!(heatmap.value_labels, vec!["3 exact"]);
    }

    fn labels() -> ActivityHeatmapLabels {
        ActivityHeatmapLabels {
            months: std::array::from_fn(|index| format!("M{}", index + 1).into()),
            less: "Less".into(),
            more: "More".into(),
            value: "Tokens".into(),
        }
    }

    fn test_heatmap(id: &'static str) -> ActivityHeatmap {
        ActivityHeatmap::new(
            id,
            series(date(2024, Month::January, 1), vec![1; 365]),
            labels(),
            format!("{id} summary"),
        )
        .format_date(|date| format!("date:{date}").into())
        .format_value(|value| format!("exact:{value}").into())
    }

    #[derive(Default)]
    struct HeatmapRenderTest {
        left_scroll: Option<Entity<ScrollHandle>>,
        right_scroll: Option<Entity<ScrollHandle>>,
    }

    impl Render for HeatmapRenderTest {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.left_scroll = Some(window.use_keyed_state(
                child_id(&ElementId::from("left-heatmap"), "scroll-state"),
                cx,
                |_, _| ScrollHandle::default(),
            ));
            self.right_scroll = Some(window.use_keyed_state(
                child_id(&ElementId::from("right-heatmap"), "scroll-state"),
                cx,
                |_, _| ScrollHandle::default(),
            ));
            v_flex()
                .w(px(180.))
                .children(["left-heatmap", "right-heatmap"].map(|id| {
                    div()
                        .w(px(180.))
                        .h(px(160.))
                        .overflow_hidden()
                        .child(test_heatmap(id))
                }))
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    fn set_scroll_x(cx: &mut VisualTestContext, state: &Entity<ScrollHandle>, offset: Pixels) {
        cx.update(|_, cx| state.read(cx).set_offset(point(offset, px(0.))));
    }

    fn scroll_x(cx: &mut VisualTestContext, state: &Entity<ScrollHandle>) -> Pixels {
        cx.update(|_, cx| state.read(cx).offset().x)
    }

    #[gpui::test]
    fn gpui_render_smoke_uses_current_theme_and_exact_tooltip_index(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        cx.update(|cx| {
            let original = level_palette(cx);
            Theme::global_mut(cx).chart_1 = rgb(0x12_34_56).into();
            let changed = level_palette(cx);
            assert_ne!(original[4], changed[4]);
            assert_eq!(changed[1], changed[4].opacity(0.25));

            let heatmap_series = series(date(2024, Month::January, 3), vec![0, 42]);
            let layout =
                HeatmapLayout::new(&heatmap_series, HeatmapMetrics::for_size(Size::Medium));
            let plot = ActivityHeatmapPlot {
                id: child_id(&ElementId::from("tooltip-test"), "plot"),
                series: heatmap_series,
                layout: layout.clone(),
                date_labels: vec!["first-date".into(), "second-date".into()],
                value_labels: vec!["0".into(), "42".into()],
                value_label: "Tokens".into(),
            };
            let bounds = Bounds {
                origin: point(px(0.), px(0.)),
                size: GpuiSize::new(layout.grid_width, layout.grid_height),
            };
            let state = plot
                .tooltip_state(point(px(1.), px(46.)), bounds, cx)
                .unwrap();
            assert_eq!(state.index, 1);
            assert_eq!(plot.date_labels[state.index], "second-date");
            assert_eq!(plot.value_labels[state.index], "42");
        });

        let (_, cx) = cx.add_window_view(|_, _| HeatmapRenderTest::default());
        draw(cx);
    }

    #[gpui::test]
    fn two_instances_keep_keyed_scroll_offsets_isolated_across_rerender(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|_, _| HeatmapRenderTest::default());
        draw(cx);

        let (left, right) = cx.update(|_, cx| {
            let view = view.read(cx);
            (
                view.left_scroll.clone().unwrap(),
                view.right_scroll.clone().unwrap(),
            )
        });
        set_scroll_x(cx, &left, px(-64.));
        assert_eq!(scroll_x(cx, &left), px(-64.));
        assert_eq!(scroll_x(cx, &right), px(0.));

        view.update(cx, |_, cx| cx.notify());
        draw(cx);
        assert_eq!(scroll_x(cx, &left), px(-64.));
        assert_eq!(scroll_x(cx, &right), px(0.));
    }

    #[test]
    fn stable_child_ids_are_unique_per_component() {
        let left = ElementId::from("left");
        let right = ElementId::from("right");
        assert_eq!(child_id(&left, "plot"), child_id(&left, "plot"));
        assert_ne!(child_id(&left, "plot"), child_id(&right, "plot"));
        assert_ne!(
            child_id(&left, "scroll-state"),
            child_id(&left, "scrollbar")
        );
    }
}
