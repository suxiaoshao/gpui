use crate::{
    database,
    foundation::{I18n, conversation_format::format_token_count},
};
use fluent_bundle::FluentArgs;
use gpui::*;
use gpui_component::{
    ActiveTheme, IndexPath, Sizable, Size, StyledExt,
    alert::Alert,
    button::Button,
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectItem, SelectState},
    skeleton::Skeleton,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use gpui_heatmap::{
    ActivityHeatmap, ActivityHeatmapLabels, ActivityHeatmapSeries, ActivityHeatmapSeriesError,
};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use jaco_db::{
    UsageAnalyticsAggregate, UsageAnalyticsFiniteRange, UsageAnalyticsProviderModelBucket,
    UsageAnalyticsQuery, UsageAnalyticsRange, UsageAnalyticsSnapshot,
};
use std::fmt;
use time::{Date, Duration, Month, OffsetDateTime, UtcOffset};
use tracing::{Level, event};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UsageAnalyticsPeriod {
    Today,
    ThisWeek,
    ThisMonth,
    ThisYear,
    AllTime,
}

impl UsageAnalyticsPeriod {
    const ALL: [Self; 5] = [
        Self::Today,
        Self::ThisWeek,
        Self::ThisMonth,
        Self::ThisYear,
        Self::AllTime,
    ];

    fn i18n_key(self) -> &'static str {
        match self {
            Self::Today => "settings-usage-period-today",
            Self::ThisWeek => "settings-usage-period-this-week",
            Self::ThisMonth => "settings-usage-period-this-month",
            Self::ThisYear => "settings-usage-period-this-year",
            Self::AllTime => "settings-usage-period-all-time",
        }
    }
}

#[derive(Clone)]
struct UsagePeriodOption {
    period: UsageAnalyticsPeriod,
    label: SharedString,
}

impl SelectItem for UsagePeriodOption {
    type Value = UsageAnalyticsPeriod;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.period
    }
}

struct UsageAnalyticsData {
    period: UsageAnalyticsPeriod,
    snapshot: UsageAnalyticsSnapshot,
    activity: UsageActivityViewData,
}

impl UsageAnalyticsData {
    fn try_new(
        period: UsageAnalyticsPeriod,
        snapshot: UsageAnalyticsSnapshot,
    ) -> Result<Self, UsageActivityInvariant> {
        let activity = UsageActivityViewData::try_new(&snapshot)?;
        Ok(Self {
            period,
            snapshot,
            activity,
        })
    }
}

struct UsageActivityViewData {
    series: ActivityHeatmapSeries,
    active_days: u64,
    peak: Option<(Date, u64)>,
}

impl UsageActivityViewData {
    fn try_new(snapshot: &UsageAnalyticsSnapshot) -> Result<Self, UsageActivityInvariant> {
        let daily = &snapshot.activity.daily;
        if daily.len() != ACTIVITY_DAY_COUNT {
            return Err(UsageActivityInvariant::BucketCount {
                actual: daily.len(),
            });
        }

        let range = snapshot.activity.range;
        let offset = range.local_offset();
        let expected_start = range
            .start_utc()
            .checked_to_offset(offset)
            .ok_or(UsageActivityInvariant::RangeStart)?
            .date();
        let expected_end = range
            .end_utc()
            .checked_to_offset(offset)
            .ok_or(UsageActivityInvariant::RangeEnd)?
            .date()
            .previous_day()
            .ok_or(UsageActivityInvariant::RangeEnd)?;

        if daily.first().map(|bucket| bucket.local_date) != Some(expected_start) {
            return Err(UsageActivityInvariant::RangeStart);
        }
        if daily.last().map(|bucket| bucket.local_date) != Some(expected_end) {
            return Err(UsageActivityInvariant::RangeEnd);
        }
        for (index, buckets) in daily.windows(2).enumerate() {
            if buckets[0].local_date.next_day() != Some(buckets[1].local_date) {
                return Err(UsageActivityInvariant::NonContiguous { index: index + 1 });
            }
        }

        let values = daily
            .iter()
            .map(|bucket| bucket.aggregate.total_tokens)
            .collect::<Vec<_>>();
        let active_count = values.iter().filter(|value| **value > 0).count();
        let active_days =
            u64::try_from(active_count).map_err(|_| UsageActivityInvariant::BucketCount {
                actual: active_count,
            })?;
        let peak = daily
            .iter()
            .filter(|bucket| bucket.aggregate.total_tokens > 0)
            .max_by(|left, right| {
                left.aggregate
                    .total_tokens
                    .cmp(&right.aggregate.total_tokens)
                    .then_with(|| right.local_date.cmp(&left.local_date))
            })
            .map(|bucket| (bucket.local_date, bucket.aggregate.total_tokens));
        let series = ActivityHeatmapSeries::try_new(expected_start, values)
            .map_err(UsageActivityInvariant::Series)?;

        Ok(Self {
            series,
            active_days,
            peak,
        })
    }
}

#[derive(Debug)]
struct UsageAnalyticsProblem {
    period: UsageAnalyticsPeriod,
    query: Option<UsageAnalyticsQuery>,
    source: UsageAnalyticsProblemSource,
}

#[derive(Debug)]
enum UsageAnalyticsProblemSource {
    LocalOffset(time::error::IndeterminateOffset),
    CalendarRange,
    Database(Box<jaco_db::DbError>),
    Activity(UsageActivityInvariant),
}

#[derive(Debug)]
enum UsageActivityInvariant {
    BucketCount { actual: usize },
    RangeStart,
    RangeEnd,
    NonContiguous { index: usize },
    Series(ActivityHeatmapSeriesError),
}

impl fmt::Display for UsageActivityInvariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BucketCount { actual } => {
                write!(
                    f,
                    "activity requires {ACTIVITY_DAY_COUNT} buckets, received {actual}"
                )
            }
            Self::RangeStart => f.write_str("activity start does not match its range"),
            Self::RangeEnd => f.write_str("activity end does not match its range"),
            Self::NonContiguous { index } => {
                write!(f, "activity bucket {index} is not contiguous")
            }
            Self::Series(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for UsageActivityInvariant {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Series(error) => Some(error),
            Self::BucketCount { .. }
            | Self::RangeStart
            | Self::RangeEnd
            | Self::NonContiguous { .. } => None,
        }
    }
}

impl fmt::Display for UsageAnalyticsProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            UsageAnalyticsProblemSource::LocalOffset(error) => {
                write!(f, "local offset is unavailable: {error}")
            }
            UsageAnalyticsProblemSource::CalendarRange => {
                f.write_str("usage analytics calendar range is not representable")
            }
            UsageAnalyticsProblemSource::Database(error) => error.fmt(f),
            UsageAnalyticsProblemSource::Activity(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for UsageAnalyticsProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            UsageAnalyticsProblemSource::LocalOffset(error) => Some(error),
            UsageAnalyticsProblemSource::CalendarRange => None,
            UsageAnalyticsProblemSource::Database(error) => Some(error),
            UsageAnalyticsProblemSource::Activity(error) => Some(error),
        }
    }
}

type UsageAnalyticsOperation =
    refresh::Operation<UsageAnalyticsData, UsageAnalyticsProblem, Task<()>>;
type UsagePeriodSelectState = SelectState<Vec<UsagePeriodOption>>;

pub(super) struct UsageSettingsPage {
    selected_period: UsageAnalyticsPeriod,
    active_query: Option<UsageAnalyticsQuery>,
    period_select: Entity<UsagePeriodSelectState>,
    operation: UsageAnalyticsOperation,
    _subscriptions: Vec<Subscription>,
}

impl UsageSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected_period = UsageAnalyticsPeriod::ThisMonth;
        let period_select = cx.new(|cx| {
            SelectState::new(
                localized_period_options(cx.global::<I18n>()),
                Some(IndexPath::new(period_index(selected_period))),
                window,
                cx,
            )
        });
        let period_subscription = cx.subscribe_in(
            &period_select,
            window,
            |page, _, event: &SelectEvent<Vec<UsagePeriodOption>>, window, cx| {
                let SelectEvent::Confirm(period) = event;
                if let Some(period) = period {
                    page.select_period(*period, window, cx);
                }
            },
        );

        Self {
            selected_period,
            active_query: None,
            period_select,
            operation: UsageAnalyticsOperation::new(),
            _subscriptions: vec![period_subscription],
        }
    }

    pub(super) fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.is_running() || !database::is_ready(cx) {
            return;
        }
        self.start_query(window, cx);
    }

    pub(super) fn deactivate(&mut self, cx: &mut Context<Self>) {
        cancel_active_query(&mut self.operation, &mut self.active_query);
        cx.notify();
    }

    pub(super) fn sync_i18n(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options = localized_period_options(cx.global::<I18n>());
        self.period_select.update(cx, |select, cx| {
            select.set_items(options, window, cx);
            select.set_selected_value(&self.selected_period, window, cx);
        });
        cx.notify();
    }

    fn select_period(
        &mut self,
        period: UsageAnalyticsPeriod,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if period == self.selected_period {
            return;
        }
        cancel_active_query(&mut self.operation, &mut self.active_query);
        self.selected_period = period;
        if database::is_ready(cx) {
            self.start_query(window, cx);
        } else {
            cx.notify();
        }
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(window, cx);
    }

    fn start_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.is_running() {
            return;
        }

        let period = self.selected_period;
        let now_utc = OffsetDateTime::now_utc();
        let query = current_usage_query(period, now_utc);
        self.active_query = query.as_ref().ok().copied();
        let completion_query = self.active_query;
        let page = cx.entity().downgrade();

        let task = match query {
            Ok(query) => match database::ready_executor(cx) {
                Ok(executor) => window.spawn(cx, async move |cx| {
                    let result = executor
                        .execute(move |repository| repository.usage_analytics(query))
                        .await
                        .map_err(|source| UsageAnalyticsProblem {
                            period,
                            query: Some(query),
                            source: UsageAnalyticsProblemSource::Database(Box::new(source)),
                        })
                        .and_then(|snapshot| {
                            UsageAnalyticsData::try_new(period, snapshot).map_err(|source| {
                                UsageAnalyticsProblem {
                                    period,
                                    query: Some(query),
                                    source: UsageAnalyticsProblemSource::Activity(source),
                                }
                            })
                        });
                    complete_query(page, period, completion_query, result, cx);
                }),
                Err(source) => window.spawn(cx, async move |cx| {
                    complete_query(
                        page,
                        period,
                        completion_query,
                        Err(UsageAnalyticsProblem {
                            period,
                            query: Some(query),
                            source: UsageAnalyticsProblemSource::Database(Box::new(source)),
                        }),
                        cx,
                    );
                }),
            },
            Err(problem) => window.spawn(cx, async move |cx| {
                complete_query(page, period, completion_query, Err(problem), cx);
            }),
        };

        match self.operation.phase() {
            refresh::Phase::Idle => self.operation.transition(Load(task)),
            refresh::Phase::Ready | refresh::Phase::Degraded => {
                self.operation.transition(Refresh(task))
            }
            refresh::Phase::Unavailable => self.operation.transition(Retry(task)),
            refresh::Phase::Loading
            | refresh::Phase::Refreshing
            | refresh::Phase::Retrying
            | refresh::Phase::RefreshingDegraded => return,
        }
        cx.notify();
    }

    fn matching_data(&self) -> Option<&UsageAnalyticsData> {
        self.operation
            .data()
            .filter(|data| usage_data_matches(self.selected_period, self.active_query, data))
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<I18n>();
        h_flex()
            .w_full()
            .items_start()
            .justify_between()
            .gap_4()
            .flex_wrap()
            .child(
                Label::new(i18n.t("settings-usage-description"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .flex_1()
                    .min_w(px(260.)),
            )
            .child(
                h_flex()
                    .id("settings-usage-period-select")
                    .role(Role::ComboBox)
                    .aria_label(i18n.t("settings-usage-period-label"))
                    .aria_value(i18n.t(self.selected_period.i18n_key()))
                    .track_focus(&self.period_select.focus_handle(cx))
                    .flex_none()
                    .gap_2()
                    .child(
                        Label::new(i18n.t("settings-usage-period-label"))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(Select::new(&self.period_select).small().w(px(180.))),
            )
            .into_any_element()
    }

    fn render_loading(&self, cx: &mut Context<Self>) -> AnyElement {
        let loading = cx.global::<I18n>().t("settings-usage-loading");
        v_flex()
            .id("settings-usage-loading")
            .role(Role::Status)
            .aria_label(loading.clone())
            .w_full()
            .gap_3()
            .child(
                Label::new(loading)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(Skeleton::new().h_8())
            .child(Skeleton::new().secondary().h_24())
            .child(Skeleton::new().h_8())
            .child(Skeleton::new().secondary().h_16())
            .into_any_element()
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<I18n>();
        v_flex()
            .id("settings-usage-empty")
            .w_full()
            .min_h(px(220.))
            .items_center()
            .justify_center()
            .gap_2()
            .child(Label::new(i18n.t("settings-usage-empty-title")).font_medium())
            .child(
                Label::new(i18n.t("settings-usage-empty-description"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .into_any_element()
    }

    fn render_problem(&self, degraded: bool, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let description_key = if degraded {
            "settings-usage-refresh-error-description"
        } else {
            "settings-usage-load-error-description"
        };
        let alert = if degraded {
            Alert::warning("settings-usage-refresh-error", i18n.t(description_key))
        } else {
            Alert::error("settings-usage-load-error", i18n.t(description_key))
        }
        .title(i18n.t("settings-usage-load-error-title"));

        v_flex()
            .w_full()
            .gap_2()
            .child(alert)
            .child(
                h_flex().justify_end().child(
                    Button::new("settings-usage-retry")
                        .label(i18n.t("settings-usage-retry"))
                        .small()
                        .on_click(cx.listener(|page, _, window, cx| page.retry(window, cx))),
                ),
            )
            .into_any_element()
    }

    fn render_summary(
        &self,
        aggregate: &UsageAnalyticsAggregate,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let metrics = usage_summary_metrics(aggregate);

        let title = i18n.t("settings-usage-summary-title");
        let group =
            GroupBox::new()
                .outline()
                .title(Label::new(title.clone()).text_sm())
                .child(h_flex().items_start().flex_wrap().gap_4().children(
                    metrics.into_iter().map(|(key, value)| {
                        v_flex()
                            .min_w(px(150.))
                            .gap_1()
                            .child(
                                Label::new(i18n.t(key))
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                Label::new(format_token_count(value))
                                    .text_sm()
                                    .font_medium(),
                            )
                    }),
                ));
        div()
            .id("settings-usage-summary")
            .role(Role::Group)
            .aria_label(summary_accessible_label(aggregate, i18n))
            .child(group)
            .into_any_element()
    }

    fn render_selected_period_empty(
        &self,
        data: &UsageAnalyticsData,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let mut args = FluentArgs::new();
        args.set("range", i18n.t(data.period.i18n_key()));
        let text = i18n.t_with_args("settings-usage-selected-period-empty", &args);

        div()
            .id("settings-usage-selected-period-empty")
            .role(Role::Status)
            .aria_label(text.clone())
            .w_full()
            .child(
                Label::new(text)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .into_any_element()
    }

    fn render_activity(&self, data: &UsageAnalyticsData, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let total = format_token_count(data.snapshot.activity.summary.total_tokens);
        let mut caption_args = FluentArgs::new();
        caption_args.set("total", total.clone());
        caption_args.set("days", ACTIVITY_DAY_COUNT.to_string());
        let caption = i18n.t_with_args("settings-usage-activity-caption", &caption_args);

        let start = localized_date(data.activity.series.start_date(), i18n);
        let end = localized_date(data.activity.series.end_date(), i18n);
        let mut accessible_args = FluentArgs::new();
        accessible_args.set("start", start);
        accessible_args.set("end", end);
        accessible_args.set("total", total);
        accessible_args.set("activeDays", format_token_count(data.activity.active_days));
        let accessible = if let Some((peak_date, peak_tokens)) = data.activity.peak {
            accessible_args.set("peakDate", localized_date(peak_date, i18n));
            accessible_args.set("peakTokens", format_token_count(peak_tokens));
            i18n.t_with_args("settings-usage-activity-accessible", &accessible_args)
        } else {
            i18n.t_with_args(
                "settings-usage-activity-accessible-no-peak",
                &accessible_args,
            )
        };
        let labels = ActivityHeatmapLabels {
            months: localized_month_labels(i18n),
            less: i18n.t("settings-usage-activity-less").into(),
            more: i18n.t("settings-usage-activity-more").into(),
            value: i18n.t("settings-usage-total-tokens").into(),
        };
        let heatmap = ActivityHeatmap::new(
            "settings-usage-activity",
            data.activity.series.clone(),
            labels,
            accessible,
        )
        .caption(caption)
        .format_date(|date| localized_date(date, i18n).into())
        .format_value(|value| format_token_count(value).into())
        .with_size(Size::Medium);

        GroupBox::new()
            .id("settings-usage-activity-section")
            .outline()
            .title(Label::new(i18n.t("settings-usage-activity-title")).text_sm())
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(
                        Label::new(i18n.t("settings-usage-activity-description"))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(heatmap),
            )
            .into_any_element()
    }

    fn render_breakdown(
        &self,
        buckets: &[UsageAnalyticsProviderModelBucket],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let table = Table::new()
            .small()
            .min_w(px(960.))
            .child(TableHeader::new().child(
                TableRow::new().children(USAGE_BREAKDOWN_COLUMNS.into_iter().enumerate().map(
                    |(column, (key, numeric))| {
                        let text: SharedString = i18n.t(key).into();
                        let head = TableHead::new().child(accessible_table_text(
                            format!("settings-usage-column-{column}"),
                            text,
                        ));
                        if numeric { head.text_right() } else { head }
                    },
                )),
            ))
            .child(
                TableBody::new().children(buckets.iter().enumerate().map(|(row, bucket)| {
                    let aggregate = &bucket.aggregate;
                    let model = display_label(bucket.model_label.as_deref(), &bucket.model_id);
                    let provider =
                        display_label(bucket.provider_label.as_deref(), &bucket.provider_id);
                    TableRow::new()
                        .child(TableCell::new().child(accessible_table_text(
                            format!("settings-usage-cell-{row}-0"),
                            model,
                        )))
                        .child(TableCell::new().child(accessible_table_text(
                            format!("settings-usage-cell-{row}-1"),
                            provider,
                        )))
                        .child(numeric_cell(row, 2, aggregate.request_count))
                        .child(numeric_cell(row, 3, aggregate.input_tokens))
                        .child(numeric_cell(row, 4, aggregate.output_tokens))
                        .child(numeric_cell(row, 5, aggregate.cached_input_tokens))
                        .child(numeric_cell(row, 6, aggregate.cache_write_input_tokens))
                })),
            );

        v_flex()
            .id("settings-usage-breakdown-section")
            .w_full()
            .gap_2()
            .child(
                Label::new(i18n.t("settings-usage-breakdown-title"))
                    .text_sm()
                    .font_medium(),
            )
            .child(
                div()
                    .id("settings-usage-breakdown-table-scroll")
                    .role(Role::Group)
                    .aria_label(i18n.t("settings-usage-breakdown-title"))
                    .w_full()
                    .overflow_x_scrollbar()
                    .child(table),
            )
            .into_any_element()
    }

    fn render_dashboard(&self, data: &UsageAnalyticsData, cx: &mut Context<Self>) -> AnyElement {
        let selected_empty = matches!(
            usage_content_composition(&data.snapshot),
            UsageContentComposition::SelectedEmptyActivityReady
        );
        let mut dashboard = v_flex().id("settings-usage-dashboard").w_full().gap_4();
        if selected_empty {
            dashboard = dashboard.child(self.render_selected_period_empty(data, cx));
        } else {
            dashboard = dashboard.child(self.render_summary(&data.snapshot.selected_summary, cx));
        }
        dashboard = dashboard.child(self.render_activity(data, cx));
        if !selected_empty {
            dashboard = dashboard.child(self.render_breakdown(&data.snapshot.provider_models, cx));
        }
        dashboard.into_any_element()
    }

    fn render_data(&self, data: &UsageAnalyticsData, cx: &mut Context<Self>) -> AnyElement {
        match usage_content_composition(&data.snapshot) {
            UsageContentComposition::GlobalEmpty => self.render_empty(cx),
            UsageContentComposition::SelectedEmptyActivityReady
            | UsageContentComposition::SelectedReadyActivityEmpty
            | UsageContentComposition::SelectedReadyActivityReady => {
                self.render_dashboard(data, cx)
            }
        }
    }

    fn render_refresh_warning(&self, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<I18n>();
        Alert::warning(
            "settings-usage-refresh-warning",
            i18n.t("settings-usage-refresh-error-description"),
        )
        .title(i18n.t("settings-usage-load-error-title"))
        .into_any_element()
    }
}

impl Render for UsageSettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = self.operation.phase();
        let matching_data = self.matching_data();
        let content = match (phase, matching_data) {
            (refresh::Phase::Ready, Some(data)) => self.render_data(data, cx),
            (refresh::Phase::Refreshing, Some(data)) => {
                let refreshing = cx.global::<I18n>().t("settings-usage-refreshing");
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div()
                            .id("settings-usage-refreshing-status")
                            .role(Role::Status)
                            .aria_label(refreshing.clone())
                            .child(
                                Label::new(refreshing)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(self.render_data(data, cx))
                    .into_any_element()
            }
            (refresh::Phase::RefreshingDegraded, Some(data)) => {
                let refreshing = cx.global::<I18n>().t("settings-usage-refreshing");
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(self.render_refresh_warning(cx))
                    .child(
                        div()
                            .id("settings-usage-refreshing-degraded-status")
                            .role(Role::Status)
                            .aria_label(refreshing.clone())
                            .child(
                                Label::new(refreshing)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(self.render_data(data, cx))
                    .into_any_element()
            }
            (refresh::Phase::Degraded, Some(data)) => v_flex()
                .w_full()
                .gap_3()
                .child(self.render_problem(true, cx))
                .child(self.render_data(data, cx))
                .into_any_element(),
            (refresh::Phase::Unavailable, _)
            | (refresh::Phase::Degraded, _)
            | (refresh::Phase::RefreshingDegraded, _) => self.render_problem(false, cx),
            (refresh::Phase::Idle, _)
            | (refresh::Phase::Loading, _)
            | (refresh::Phase::Retrying, _)
            | (refresh::Phase::Refreshing, _)
            | (refresh::Phase::Ready, None) => self.render_loading(cx),
        };

        v_flex()
            .w_full()
            .gap_4()
            .child(self.render_header(cx))
            .child(content)
    }
}

fn complete_query(
    page: WeakEntity<UsageSettingsPage>,
    period: UsageAnalyticsPeriod,
    query: Option<UsageAnalyticsQuery>,
    result: Result<UsageAnalyticsData, UsageAnalyticsProblem>,
    cx: &mut AsyncWindowContext,
) {
    let _ = page.update_in(cx, |page, _window, cx| {
        if page.selected_period != period
            || page.active_query != query
            || !page.operation.is_running()
            || !query_result_matches(period, query, &result)
        {
            return;
        }
        if let Err(problem) = &result {
            event!(
                Level::ERROR,
                period = ?problem.period,
                query = ?problem.query,
                error = ?problem.source,
                "load settings usage analytics failed"
            );
        }
        page.operation.transition(Complete(result));
        cx.notify();
    });
}

fn query_result_matches(
    period: UsageAnalyticsPeriod,
    query: Option<UsageAnalyticsQuery>,
    result: &Result<UsageAnalyticsData, UsageAnalyticsProblem>,
) -> bool {
    match result {
        Ok(data) => usage_data_matches(period, query, data),
        Err(problem) => problem.period == period && problem.query == query,
    }
}

fn cancel_active_query(
    operation: &mut UsageAnalyticsOperation,
    active_query: &mut Option<UsageAnalyticsQuery>,
) {
    if operation.is_running() {
        operation.transition(Cancel);
    }
    *active_query = None;
}

fn usage_data_matches(
    selected_period: UsageAnalyticsPeriod,
    active_query: Option<UsageAnalyticsQuery>,
    data: &UsageAnalyticsData,
) -> bool {
    data.period == selected_period
        && active_query.is_some_and(|query| {
            data.snapshot.selected_range == query.selected_range
                && data.snapshot.activity.range == query.activity_range
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsageContentComposition {
    GlobalEmpty,
    SelectedEmptyActivityReady,
    SelectedReadyActivityEmpty,
    SelectedReadyActivityReady,
}

fn usage_content_composition(snapshot: &UsageAnalyticsSnapshot) -> UsageContentComposition {
    match (
        snapshot.selected_summary.is_empty(),
        snapshot.activity.summary.is_empty(),
    ) {
        (true, true) => UsageContentComposition::GlobalEmpty,
        (true, false) => UsageContentComposition::SelectedEmptyActivityReady,
        (false, true) => UsageContentComposition::SelectedReadyActivityEmpty,
        (false, false) => UsageContentComposition::SelectedReadyActivityReady,
    }
}

fn current_usage_query(
    period: UsageAnalyticsPeriod,
    now_utc: OffsetDateTime,
) -> Result<UsageAnalyticsQuery, UsageAnalyticsProblem> {
    let local_offset =
        UtcOffset::local_offset_at(now_utc).map_err(|source| UsageAnalyticsProblem {
            period,
            query: None,
            source: UsageAnalyticsProblemSource::LocalOffset(source),
        })?;
    usage_query_for_offset(period, now_utc, local_offset).ok_or(UsageAnalyticsProblem {
        period,
        query: None,
        source: UsageAnalyticsProblemSource::CalendarRange,
    })
}

fn usage_query_for_offset(
    period: UsageAnalyticsPeriod,
    now_utc: OffsetDateTime,
    local_offset: UtcOffset,
) -> Option<UsageAnalyticsQuery> {
    // `time`'s large-date representation can reach years where offset
    // conversion trips its internal standard-range assertions. One year of
    // headroom covers every legal UTC offset in both directions.
    if !(-9_998..=9_998).contains(&now_utc.year()) {
        return None;
    }
    let today = now_utc.checked_to_offset(local_offset)?.date();
    let activity_start = today.checked_sub(Duration::days((ACTIVITY_DAY_COUNT - 1) as i64))?;
    let activity_end = today.next_day()?;
    let activity_range = finite_range_for_dates(activity_start, activity_end, local_offset)?;

    let selected_range = match period {
        UsageAnalyticsPeriod::Today => UsageAnalyticsRange::Finite(finite_range_for_dates(
            today,
            today.next_day()?,
            local_offset,
        )?),
        UsageAnalyticsPeriod::ThisWeek => {
            let days = i64::from(today.weekday().number_days_from_monday());
            let start = today.checked_sub(Duration::days(days))?;
            UsageAnalyticsRange::Finite(finite_range_for_dates(
                start,
                start.checked_add(Duration::days(7))?,
                local_offset,
            )?)
        }
        UsageAnalyticsPeriod::ThisMonth => {
            let start = Date::from_calendar_date(today.year(), today.month(), 1).ok()?;
            let (year, month) = next_month(start.year(), start.month())?;
            let end = Date::from_calendar_date(year, month, 1).ok()?;
            UsageAnalyticsRange::Finite(finite_range_for_dates(start, end, local_offset)?)
        }
        UsageAnalyticsPeriod::ThisYear => {
            let start = Date::from_calendar_date(today.year(), Month::January, 1).ok()?;
            let end =
                Date::from_calendar_date(today.year().checked_add(1)?, Month::January, 1).ok()?;
            UsageAnalyticsRange::Finite(finite_range_for_dates(start, end, local_offset)?)
        }
        UsageAnalyticsPeriod::AllTime => UsageAnalyticsRange::AllTime,
    };

    Some(UsageAnalyticsQuery {
        selected_range,
        activity_range,
    })
}

fn finite_range_for_dates(
    start_date: Date,
    end_date: Date,
    local_offset: UtcOffset,
) -> Option<UsageAnalyticsFiniteRange> {
    if !(-9_998..=9_998).contains(&start_date.year())
        || !(-9_998..=9_998).contains(&end_date.year())
    {
        return None;
    }
    let start_utc = start_date
        .midnight()
        .assume_offset(local_offset)
        .checked_to_offset(UtcOffset::UTC)?;
    let end_utc = end_date
        .midnight()
        .assume_offset(local_offset)
        .checked_to_offset(UtcOffset::UTC)?;
    UsageAnalyticsFiniteRange::new(start_utc, end_utc, local_offset)
}

fn next_month(year: i32, month: Month) -> Option<(i32, Month)> {
    if month == Month::December {
        Some((year.checked_add(1)?, Month::January))
    } else {
        Some((year, month.next()))
    }
}

fn localized_period_options(i18n: &I18n) -> Vec<UsagePeriodOption> {
    UsageAnalyticsPeriod::ALL
        .into_iter()
        .map(|period| UsagePeriodOption {
            period,
            label: i18n.t(period.i18n_key()).into(),
        })
        .collect()
}

fn period_index(period: UsageAnalyticsPeriod) -> usize {
    UsageAnalyticsPeriod::ALL
        .iter()
        .position(|candidate| *candidate == period)
        .expect("every usage period has a select option")
}

fn localized_date(date: Date, i18n: &I18n) -> String {
    let mut args = FluentArgs::new();
    args.set("year", date.year().to_string());
    args.set("month", format!("{:02}", u8::from(date.month())));
    args.set("day", format!("{:02}", date.day()));
    i18n.t_with_args("settings-usage-date-value", &args)
}

fn localized_month_labels(i18n: &I18n) -> [SharedString; 12] {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    MONTHS.map(|month| {
        let mut args = FluentArgs::new();
        args.set("month", month);
        i18n.t_with_args("settings-usage-activity-month-label", &args)
            .into()
    })
}

fn display_label(label: Option<&str>, fallback: &str) -> SharedString {
    label
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or(fallback)
        .to_owned()
        .into()
}

const USAGE_BREAKDOWN_COLUMNS: [(&str, bool); 7] = [
    ("settings-usage-model", false),
    ("settings-usage-provider", false),
    ("settings-usage-requests", true),
    ("settings-usage-input-tokens", true),
    ("settings-usage-output-tokens", true),
    ("settings-usage-cached-input-tokens", true),
    ("settings-usage-cache-write-input-tokens", true),
];

fn usage_summary_metrics(aggregate: &UsageAnalyticsAggregate) -> [(&'static str, u64); 10] {
    [
        ("settings-usage-requests", aggregate.request_count),
        (
            "settings-usage-reported-requests",
            aggregate.reported_request_count,
        ),
        (
            "settings-usage-unreported-requests",
            aggregate.unreported_request_count,
        ),
        (
            "settings-usage-total-covered-requests",
            aggregate.total_covered_request_count,
        ),
        ("settings-usage-input-tokens", aggregate.input_tokens),
        ("settings-usage-output-tokens", aggregate.output_tokens),
        (
            "settings-usage-cached-input-tokens",
            aggregate.cached_input_tokens,
        ),
        (
            "settings-usage-cache-write-input-tokens",
            aggregate.cache_write_input_tokens,
        ),
        (
            "settings-usage-reasoning-tokens",
            aggregate.reasoning_tokens,
        ),
        ("settings-usage-total-tokens", aggregate.total_tokens),
    ]
}

fn summary_accessible_label(aggregate: &UsageAnalyticsAggregate, i18n: &I18n) -> String {
    let metrics = usage_summary_metrics(aggregate)
        .into_iter()
        .map(|(key, value)| {
            let mut args = FluentArgs::new();
            args.set("label", i18n.t(key));
            args.set("value", format_token_count(value));
            i18n.t_with_args("settings-usage-metric-accessible", &args)
        })
        .collect::<Vec<_>>()
        .join("; ");
    let mut args = FluentArgs::new();
    args.set("metrics", metrics);
    i18n.t_with_args("settings-usage-summary-accessible", &args)
}

fn accessible_table_text(id: impl Into<ElementId>, text: SharedString) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text.clone())
        .child(Label::new(text))
        .into_any_element()
}

fn numeric_cell(row: usize, column: usize, value: u64) -> TableCell {
    let text: SharedString = format_token_count(value).into();
    TableCell::new().text_right().child(accessible_table_text(
        format!("settings-usage-cell-{row}-{column}"),
        text,
    ))
}

const ACTIVITY_DAY_COUNT: usize = 365;

#[cfg(test)]
mod tests {
    use super::{
        ACTIVITY_DAY_COUNT, USAGE_BREAKDOWN_COLUMNS, UsageActivityInvariant, UsageAnalyticsData,
        UsageAnalyticsOperation, UsageAnalyticsPeriod, UsageContentComposition, UsageSettingsPage,
        cancel_active_query, complete_query, display_label, localized_date, localized_month_labels,
        query_result_matches, summary_accessible_label, usage_content_composition,
        usage_data_matches, usage_query_for_offset,
    };
    use crate::{database, foundation::I18n};
    use gpui::{AppContext as _, Task, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_operation::{Load, Transition, refresh};
    use jaco_db::{
        UsageAnalyticsActivity, UsageAnalyticsAggregate, UsageAnalyticsDailyBucket,
        UsageAnalyticsFiniteRange, UsageAnalyticsQuery, UsageAnalyticsRange,
        UsageAnalyticsSnapshot,
    };
    use tempfile::{TempDir, tempdir};
    use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

    fn utc(year: i32, month: Month, day: u8, hour: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .unwrap()
            .with_time(Time::from_hms(hour, 0, 0).unwrap())
            .assume_utc()
    }

    fn finite(
        period: UsageAnalyticsPeriod,
        now: OffsetDateTime,
        offset: UtcOffset,
    ) -> jaco_db::UsageAnalyticsFiniteRange {
        let UsageAnalyticsRange::Finite(range) = usage_query_for_offset(period, now, offset)
            .unwrap()
            .selected_range
        else {
            panic!("expected a finite range")
        };
        range
    }

    fn query(
        period: UsageAnalyticsPeriod,
        now: OffsetDateTime,
        offset: UtcOffset,
    ) -> UsageAnalyticsQuery {
        usage_query_for_offset(period, now, offset).expect("representable usage query")
    }

    #[test]
    fn usage_periods_keep_product_order() {
        assert_eq!(
            UsageAnalyticsPeriod::ALL,
            [
                UsageAnalyticsPeriod::Today,
                UsageAnalyticsPeriod::ThisWeek,
                UsageAnalyticsPeriod::ThisMonth,
                UsageAnalyticsPeriod::ThisYear,
                UsageAnalyticsPeriod::AllTime,
            ]
        );
    }

    #[test]
    fn today_uses_positive_negative_and_half_hour_local_boundaries() {
        for offset in [
            UtcOffset::from_hms(8, 0, 0).unwrap(),
            UtcOffset::from_hms(-7, 0, 0).unwrap(),
            UtcOffset::from_hms(5, 30, 0).unwrap(),
        ] {
            let range = finite(
                UsageAnalyticsPeriod::Today,
                utc(2026, Month::August, 20, 20),
                offset,
            );
            assert_eq!(range.local_offset(), offset);
            assert_eq!(range.start_utc().to_offset(offset).time(), Time::MIDNIGHT);
            assert_eq!(range.end_utc().to_offset(offset).time(), Time::MIDNIGHT);
            assert_eq!((range.end_utc() - range.start_utc()).whole_hours(), 24);
        }
    }

    #[test]
    fn every_period_uses_the_same_rolling_365_day_activity_range() {
        let now = utc(2024, Month::February, 29, 8);
        let offset = UtcOffset::from_hms(5, 30, 0).unwrap();
        let queries = UsageAnalyticsPeriod::ALL.map(|period| query(period, now, offset));
        let expected = queries[0].activity_range;

        for query in queries {
            assert_eq!(query.activity_range, expected);
        }
        assert_eq!(
            expected.start_utc().to_offset(offset).date(),
            Date::from_calendar_date(2023, Month::March, 2).unwrap()
        );
        assert_eq!(
            expected.end_utc().to_offset(offset).date(),
            Date::from_calendar_date(2024, Month::March, 1).unwrap()
        );
        assert_eq!(expected.local_offset(), offset);
    }

    #[test]
    fn week_month_and_leap_year_ranges_are_half_open_calendar_periods() {
        let offset = UtcOffset::from_hms(8, 0, 0).unwrap();
        let week = finite(
            UsageAnalyticsPeriod::ThisWeek,
            utc(2026, Month::August, 20, 8),
            offset,
        );
        assert_eq!(
            week.start_utc().to_offset(offset).date(),
            Date::from_calendar_date(2026, Month::August, 17).unwrap()
        );
        assert_eq!(
            week.end_utc().to_offset(offset).date(),
            Date::from_calendar_date(2026, Month::August, 24).unwrap()
        );

        let month = finite(
            UsageAnalyticsPeriod::ThisMonth,
            utc(2024, Month::February, 29, 8),
            offset,
        );
        assert_eq!(
            month.start_utc().to_offset(offset).date(),
            Date::from_calendar_date(2024, Month::February, 1).unwrap()
        );
        assert_eq!(
            month.end_utc().to_offset(offset).date(),
            Date::from_calendar_date(2024, Month::March, 1).unwrap()
        );

        let year = finite(
            UsageAnalyticsPeriod::ThisYear,
            utc(2024, Month::February, 29, 8),
            offset,
        );
        assert_eq!(
            year.end_utc().to_offset(offset).date(),
            Date::from_calendar_date(2025, Month::January, 1).unwrap()
        );
    }

    #[test]
    fn all_time_keeps_the_rolling_activity_range() {
        let query = query(
            UsageAnalyticsPeriod::AllTime,
            OffsetDateTime::UNIX_EPOCH,
            UtcOffset::UTC,
        );
        assert_eq!(query.selected_range, UsageAnalyticsRange::AllTime);
        assert_eq!(
            query.activity_range.start_utc().date(),
            Date::from_calendar_date(1969, Month::January, 2).unwrap()
        );
        assert_eq!(
            query.activity_range.end_utc().date(),
            Date::from_calendar_date(1970, Month::January, 2).unwrap()
        );
    }

    #[test]
    fn cancellation_clears_active_query_and_restores_previous_operation_state() {
        let mut operation = UsageAnalyticsOperation::new();
        operation.transition(Load(Task::ready(())));
        let mut active_query = Some(query(
            UsageAnalyticsPeriod::AllTime,
            OffsetDateTime::UNIX_EPOCH,
            UtcOffset::UTC,
        ));

        cancel_active_query(&mut operation, &mut active_query);

        assert_eq!(operation.phase(), refresh::Phase::Idle);
        assert_eq!(active_query, None);
    }

    #[gpui::test]
    fn entity_activation_is_single_flight_and_deactivation_cancels_it(cx: &mut TestAppContext) {
        let _dir = init_usage_settings_test(cx);
        let window = open_usage_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).expect("usage settings page");

        let first_query = cx.update(|window, cx| {
            page.update(cx, |page, cx| page.activate(window, cx));
            let first_query = page.read(cx).active_query;
            page.update(cx, |page, cx| page.activate(window, cx));
            assert_eq!(page.read(cx).operation.phase(), refresh::Phase::Loading);
            assert_eq!(page.read(cx).active_query, first_query);
            first_query
        });
        assert!(first_query.is_some());

        cx.update(|_window, cx| {
            page.update(cx, |page, cx| page.deactivate(cx));
            assert_eq!(page.read(cx).operation.phase(), refresh::Phase::Idle);
            assert_eq!(page.read(cx).active_query, None);
        });
        cx.run_until_parked();
        assert_eq!(
            page.read_with(&cx, |page, _| page.operation.phase()),
            refresh::Phase::Idle
        );
        assert_eq!(page.read_with(&cx, |page, _| page.active_query), None);
    }

    #[gpui::test]
    fn entity_rejects_stale_completion_for_period_mismatch(cx: &mut TestAppContext) {
        let _dir = init_usage_settings_test(cx);
        let period_window = open_usage_settings_window(cx);
        let mut period_cx = VisualTestContext::from_window(period_window.into(), cx);
        let period_page = period_window
            .root(&mut period_cx)
            .expect("period mismatch page");
        period_cx.update(|window, cx| {
            let weak_page = period_page.downgrade();
            let stale_query = query(
                UsageAnalyticsPeriod::ThisMonth,
                utc(2026, Month::August, 20, 8),
                UtcOffset::UTC,
            );
            window
                .spawn(cx, async move |cx| {
                    complete_query(
                        weak_page,
                        UsageAnalyticsPeriod::ThisMonth,
                        Some(stale_query),
                        Ok(marked_usage_data(
                            UsageAnalyticsPeriod::ThisMonth,
                            stale_query,
                        )),
                        cx,
                    );
                })
                .detach();
            period_page.update(cx, |page, cx| {
                page.select_period(UsageAnalyticsPeriod::AllTime, window, cx)
            });
        });
        period_cx.run_until_parked();
        assert_fresh_empty_result(&period_page, UsageAnalyticsPeriod::AllTime, &period_cx);
    }

    #[gpui::test]
    fn entity_rejects_stale_completion_for_query_mismatch(cx: &mut TestAppContext) {
        let _dir = init_usage_settings_test(cx);
        let range_window = open_usage_settings_window(cx);
        let mut range_cx = VisualTestContext::from_window(range_window.into(), cx);
        let range_page = range_window
            .root(&mut range_cx)
            .expect("range mismatch page");
        range_cx.update(|window, cx| {
            let weak_page = range_page.downgrade();
            let stale_query = query(
                UsageAnalyticsPeriod::ThisMonth,
                utc(2026, Month::August, 20, 8),
                UtcOffset::UTC,
            );
            window
                .spawn(cx, async move |cx| {
                    complete_query(
                        weak_page,
                        UsageAnalyticsPeriod::ThisMonth,
                        Some(stale_query),
                        Ok(marked_usage_data(
                            UsageAnalyticsPeriod::ThisMonth,
                            stale_query,
                        )),
                        cx,
                    );
                })
                .detach();
            range_page.update(cx, |page, cx| page.activate(window, cx));
        });
        range_cx.run_until_parked();
        assert_fresh_empty_result(&range_page, UsageAnalyticsPeriod::ThisMonth, &range_cx);
    }

    #[gpui::test]
    fn entity_rejects_completion_when_no_query_is_running(cx: &mut TestAppContext) {
        let _dir = init_usage_settings_test(cx);
        let idle_window = open_usage_settings_window(cx);
        let mut idle_cx = VisualTestContext::from_window(idle_window.into(), cx);
        let idle_page = idle_window.root(&mut idle_cx).expect("idle page");
        idle_cx.update(|window, cx| {
            let weak_page = idle_page.downgrade();
            let stale_query = query(
                UsageAnalyticsPeriod::ThisMonth,
                utc(2026, Month::August, 20, 8),
                UtcOffset::UTC,
            );
            window
                .spawn(cx, async move |cx| {
                    complete_query(
                        weak_page,
                        UsageAnalyticsPeriod::ThisMonth,
                        None,
                        Ok(marked_usage_data(
                            UsageAnalyticsPeriod::ThisMonth,
                            stale_query,
                        )),
                        cx,
                    );
                })
                .detach();
        });
        idle_cx.run_until_parked();
        assert_eq!(
            idle_page.read_with(&idle_cx, |page, _| page.operation.phase()),
            refresh::Phase::Idle
        );
        assert_eq!(
            idle_page.read_with(&idle_cx, |page, _| page.active_query),
            None
        );
    }

    #[test]
    fn stale_data_requires_exact_period_and_query_and_all_zero_usage_is_not_empty() {
        let active_query = query(
            UsageAnalyticsPeriod::AllTime,
            OffsetDateTime::UNIX_EPOCH,
            UtcOffset::UTC,
        );
        let data = marked_usage_data(UsageAnalyticsPeriod::AllTime, active_query);

        assert!(usage_data_matches(
            UsageAnalyticsPeriod::AllTime,
            Some(active_query),
            &data,
        ));
        assert!(!usage_data_matches(
            UsageAnalyticsPeriod::ThisMonth,
            Some(active_query),
            &data,
        ));
        assert!(!usage_data_matches(
            UsageAnalyticsPeriod::AllTime,
            None,
            &data,
        ));
        let activity_only_mismatch = query(
            UsageAnalyticsPeriod::AllTime,
            OffsetDateTime::UNIX_EPOCH + time::Duration::days(1),
            UtcOffset::UTC,
        );
        assert_eq!(
            activity_only_mismatch.selected_range,
            active_query.selected_range
        );
        assert_ne!(
            activity_only_mismatch.activity_range,
            active_query.activity_range
        );
        assert!(!usage_data_matches(
            UsageAnalyticsPeriod::AllTime,
            Some(activity_only_mismatch),
            &data,
        ));
        assert!(!query_result_matches(
            UsageAnalyticsPeriod::AllTime,
            Some(active_query),
            &Ok(marked_usage_data(
                UsageAnalyticsPeriod::AllTime,
                activity_only_mismatch,
            )),
        ));
        assert!(!data.snapshot.selected_summary.is_empty());
    }

    #[test]
    fn labels_use_current_name_or_stable_id_and_dates_and_months_are_localized() {
        assert_eq!(display_label(Some("  OpenAI  "), "provider-id"), "OpenAI");
        assert_eq!(display_label(Some("  "), "provider-id"), "provider-id");
        assert_eq!(display_label(None, "provider-id"), "provider-id");
        let date = Date::from_calendar_date(2026, Month::August, 20).unwrap();
        assert_eq!(
            localized_date(date, &I18n::for_locale_tag("en-US")),
            "2026-08-20"
        );
        assert_eq!(
            localized_date(date, &I18n::for_locale_tag("zh-CN")),
            "2026年08月20日"
        );
        assert_eq!(
            localized_month_labels(&I18n::for_locale_tag("en-US"))[0],
            "Jan"
        );
        assert_eq!(
            localized_month_labels(&I18n::for_locale_tag("zh-CN"))[11],
            "12月"
        );
    }

    #[test]
    fn breakdown_columns_keep_the_compact_requested_order() {
        assert_eq!(
            USAGE_BREAKDOWN_COLUMNS,
            [
                ("settings-usage-model", false),
                ("settings-usage-provider", false),
                ("settings-usage-requests", true),
                ("settings-usage-input-tokens", true),
                ("settings-usage-output-tokens", true),
                ("settings-usage-cached-input-tokens", true),
                ("settings-usage-cache-write-input-tokens", true),
            ]
        );
    }

    #[test]
    fn summary_accessibility_includes_every_localized_exact_metric_and_value() {
        let aggregate = UsageAnalyticsAggregate {
            request_count: 1,
            reported_request_count: 2,
            unreported_request_count: 3,
            total_covered_request_count: 4,
            input_tokens: 5,
            output_tokens: 6,
            cached_input_tokens: 7,
            cache_write_input_tokens: 8,
            reasoning_tokens: 9,
            total_tokens: 10,
        };
        let accessible = summary_accessible_label(&aggregate, &I18n::for_locale_tag("en-US"));

        for metric in [
            "Requests: 1",
            "Reported: 2",
            "Unreported: 3",
            "Requests with total: 4",
            "Input tokens: 5",
            "Output tokens: 6",
            "Cache read: 7",
            "Cache write: 8",
            "Reasoning tokens: 9",
            "Total tokens: 10",
        ] {
            assert!(
                accessible.contains(metric),
                "missing {metric}: {accessible}"
            );
        }
    }

    #[test]
    fn extreme_dates_and_offsets_fail_without_panicking() {
        let positive = UtcOffset::from_hms(23, 59, 59).unwrap();
        let negative = UtcOffset::from_hms(-23, -59, -59).unwrap();
        let minimum = Date::MIN.midnight().assume_utc();
        let maximum = Date::MAX.with_time(Time::MAX).assume_utc();

        for (now, offset) in [(minimum, positive), (maximum, negative)] {
            let result = std::panic::catch_unwind(|| {
                usage_query_for_offset(UsageAnalyticsPeriod::Today, now, offset)
            });
            assert!(result.is_ok(), "range construction must not panic");
            assert_eq!(result.unwrap(), None);
        }
    }

    #[test]
    fn activity_adapter_validates_dense_range_and_uses_earliest_peak() {
        let query = query(
            UsageAnalyticsPeriod::ThisMonth,
            utc(2026, Month::August, 20, 8),
            UtcOffset::UTC,
        );
        let snapshot = UsageAnalyticsSnapshot {
            selected_range: query.selected_range,
            selected_summary: UsageAnalyticsAggregate::default(),
            provider_models: Vec::new(),
            activity: activity_for_range(query.activity_range, &[(4, 99), (7, 99), (10, 1)]),
        };
        let data = UsageAnalyticsData::try_new(UsageAnalyticsPeriod::ThisMonth, snapshot)
            .expect("valid dense activity");

        assert_eq!(data.activity.series.values().len(), ACTIVITY_DAY_COUNT);
        assert_eq!(data.activity.active_days, 3);
        assert_eq!(
            data.activity.peak,
            Some((
                data.activity
                    .series
                    .start_date()
                    .checked_add(time::Duration::days(4))
                    .unwrap(),
                99,
            ))
        );
    }

    #[test]
    fn activity_adapter_rejects_wrong_length_and_noncontiguous_dates() {
        let query = query(
            UsageAnalyticsPeriod::ThisMonth,
            utc(2026, Month::August, 20, 8),
            UtcOffset::UTC,
        );
        let mut short_activity = activity_for_range(query.activity_range, &[]);
        short_activity.daily.pop();
        let short = UsageAnalyticsData::try_new(
            UsageAnalyticsPeriod::ThisMonth,
            UsageAnalyticsSnapshot {
                selected_range: query.selected_range,
                selected_summary: UsageAnalyticsAggregate::default(),
                provider_models: Vec::new(),
                activity: short_activity,
            },
        );
        assert!(matches!(
            short,
            Err(UsageActivityInvariant::BucketCount { actual: 364 })
        ));

        let mut gapped_activity = activity_for_range(query.activity_range, &[]);
        gapped_activity.daily[12].local_date =
            gapped_activity.daily[12].local_date.next_day().unwrap();
        let gapped = UsageAnalyticsData::try_new(
            UsageAnalyticsPeriod::ThisMonth,
            UsageAnalyticsSnapshot {
                selected_range: query.selected_range,
                selected_summary: UsageAnalyticsAggregate::default(),
                provider_models: Vec::new(),
                activity: gapped_activity,
            },
        );
        assert!(matches!(
            gapped,
            Err(UsageActivityInvariant::NonContiguous { index: 12 })
        ));
    }

    #[test]
    fn selected_and_activity_empty_states_are_distinct() {
        let query = query(
            UsageAnalyticsPeriod::ThisMonth,
            utc(2026, Month::August, 20, 8),
            UtcOffset::UTC,
        );
        let snapshot = |selected_requests, activity_requests| UsageAnalyticsSnapshot {
            selected_range: query.selected_range,
            selected_summary: UsageAnalyticsAggregate {
                request_count: selected_requests,
                ..Default::default()
            },
            provider_models: Vec::new(),
            activity: UsageAnalyticsActivity {
                range: query.activity_range,
                summary: UsageAnalyticsAggregate {
                    request_count: activity_requests,
                    ..Default::default()
                },
                daily: Vec::new(),
            },
        };

        assert_eq!(
            usage_content_composition(&snapshot(0, 0)),
            UsageContentComposition::GlobalEmpty
        );
        assert_eq!(
            usage_content_composition(&snapshot(0, 1)),
            UsageContentComposition::SelectedEmptyActivityReady
        );
        assert_eq!(
            usage_content_composition(&snapshot(1, 0)),
            UsageContentComposition::SelectedReadyActivityEmpty
        );
        assert_eq!(
            usage_content_composition(&snapshot(1, 1)),
            UsageContentComposition::SelectedReadyActivityReady
        );
    }

    #[test]
    fn usage_locale_keys_exist_in_both_languages() {
        let keys = [
            "settings-page-usage",
            "settings-usage-description",
            "settings-usage-period-label",
            "settings-usage-period-today",
            "settings-usage-period-this-week",
            "settings-usage-period-this-month",
            "settings-usage-period-this-year",
            "settings-usage-period-all-time",
            "settings-usage-loading",
            "settings-usage-refreshing",
            "settings-usage-load-error-title",
            "settings-usage-load-error-description",
            "settings-usage-refresh-error-description",
            "settings-usage-retry",
            "settings-usage-empty-title",
            "settings-usage-empty-description",
            "settings-usage-summary-title",
            "settings-usage-requests",
            "settings-usage-reported-requests",
            "settings-usage-unreported-requests",
            "settings-usage-total-covered-requests",
            "settings-usage-input-tokens",
            "settings-usage-output-tokens",
            "settings-usage-cached-input-tokens",
            "settings-usage-cache-write-input-tokens",
            "settings-usage-reasoning-tokens",
            "settings-usage-total-tokens",
            "settings-usage-activity-title",
            "settings-usage-activity-description",
            "settings-usage-activity-less",
            "settings-usage-activity-more",
            "settings-usage-breakdown-title",
            "settings-usage-provider",
            "settings-usage-model",
        ];
        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            for key in keys {
                assert_ne!(i18n.t(key), key, "missing {key} in {locale}");
            }
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("range", "This month");
            args.set("total", "1,000");
            args.set("covered", "2");
            args.set("requests", "3");
            args.set("year", "2026");
            args.set("month", "08");
            args.set("day", "20");
            args.set("metrics", "Requests: 1");
            args.set("label", "Requests");
            args.set("value", "1");
            args.set("days", "365");
            args.set("start", "2025-08-22");
            args.set("end", "2026-08-21");
            args.set("activeDays", "2");
            args.set("peakDate", "2026-08-20");
            args.set("peakTokens", "1,000");
            args.set("month", "January");
            for key in [
                "settings-usage-date-value",
                "settings-usage-summary-accessible",
                "settings-usage-metric-accessible",
                "settings-usage-selected-period-empty",
                "settings-usage-activity-caption",
                "settings-usage-activity-month-label",
                "settings-usage-activity-accessible",
                "settings-usage-activity-accessible-no-peak",
            ] {
                assert_ne!(
                    i18n.t_with_args(key, &args),
                    key,
                    "missing {key} in {locale}"
                );
            }
            for month in [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ] {
                args.set("month", month);
                let label = i18n.t_with_args("settings-usage-activity-month-label", &args);
                assert_ne!(label, "settings-usage-activity-month-label");
                assert!(
                    !label.is_empty(),
                    "empty month label for {month} in {locale}"
                );
            }
        }
    }

    fn marked_usage_data(
        period: UsageAnalyticsPeriod,
        query: UsageAnalyticsQuery,
    ) -> UsageAnalyticsData {
        UsageAnalyticsData::try_new(
            period,
            UsageAnalyticsSnapshot {
                selected_range: query.selected_range,
                selected_summary: UsageAnalyticsAggregate {
                    request_count: 999,
                    ..Default::default()
                },
                provider_models: Vec::new(),
                activity: activity_for_range(query.activity_range, &[]),
            },
        )
        .expect("valid marked activity projection")
    }

    fn activity_for_range(
        range: UsageAnalyticsFiniteRange,
        nonzero: &[(usize, u64)],
    ) -> UsageAnalyticsActivity {
        let start = range.start_utc().to_offset(range.local_offset()).date();
        let daily = (0..ACTIVITY_DAY_COUNT)
            .map(|index| {
                let total_tokens = nonzero
                    .iter()
                    .find_map(|(candidate, value)| (*candidate == index).then_some(*value))
                    .unwrap_or(0);
                UsageAnalyticsDailyBucket {
                    local_date: start
                        .checked_add(time::Duration::days(index as i64))
                        .expect("activity fixture date"),
                    aggregate: UsageAnalyticsAggregate {
                        request_count: u64::from(total_tokens > 0),
                        total_tokens,
                        ..Default::default()
                    },
                }
            })
            .collect::<Vec<_>>();
        UsageAnalyticsActivity {
            range,
            summary: UsageAnalyticsAggregate {
                request_count: nonzero.len() as u64,
                total_tokens: nonzero.iter().map(|(_, value)| value).sum(),
                ..Default::default()
            },
            daily,
        }
    }

    fn assert_fresh_empty_result(
        page: &gpui::Entity<UsageSettingsPage>,
        expected_period: UsageAnalyticsPeriod,
        cx: &VisualTestContext,
    ) {
        page.read_with(cx, |page, _| {
            assert_eq!(page.selected_period, expected_period);
            assert_eq!(page.operation.phase(), refresh::Phase::Ready);
            assert_eq!(
                page.matching_data()
                    .expect("fresh matching usage data")
                    .snapshot
                    .selected_summary
                    .request_count,
                0
            );
        });
    }

    fn init_usage_settings_test(cx: &mut TestAppContext) -> TempDir {
        let dir = tempdir().expect("temporary usage settings database");
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(I18n::english_for_test());
            database::install_for_test(cx, dir.path());
        });
        cx.run_until_parked();
        dir
    }

    fn open_usage_settings_window(cx: &mut TestAppContext) -> WindowHandle<UsageSettingsPage> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| UsageSettingsPage::new(window, cx))
            })
            .expect("open usage settings window")
        })
    }
}
