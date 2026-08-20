use crate::{
    database,
    foundation::{I18n, conversation_format::format_token_count},
};
use fluent_bundle::FluentArgs;
use gpui::*;
use gpui_component::{
    ActiveTheme, IndexPath, Sizable, StyledExt,
    alert::Alert,
    button::Button,
    chart::LineChart,
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectItem, SelectState},
    skeleton::Skeleton,
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    v_flex,
};
use gpui_operation::{Cancel, Complete, Load, Refresh, Retry, Transition, refresh};
use jaco_db::{
    UsageAnalyticsAggregate, UsageAnalyticsFiniteRange, UsageAnalyticsProviderModelBucket,
    UsageAnalyticsRange, UsageAnalyticsSnapshot,
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

    fn chart_tick_margin(self) -> usize {
        match self {
            Self::Today | Self::ThisWeek => 1,
            Self::ThisMonth => 5,
            Self::ThisYear => 30,
            Self::AllTime => 1,
        }
    }

    fn chart_has_dots(self) -> bool {
        matches!(self, Self::Today | Self::ThisWeek | Self::ThisMonth)
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
}

#[derive(Debug)]
struct UsageAnalyticsProblem {
    period: UsageAnalyticsPeriod,
    range: Option<UsageAnalyticsRange>,
    source: UsageAnalyticsProblemSource,
}

#[derive(Debug)]
enum UsageAnalyticsProblemSource {
    LocalOffset(time::error::IndeterminateOffset),
    CalendarRange,
    Database(jaco_db::DbError),
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
        }
    }
}

impl std::error::Error for UsageAnalyticsProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            UsageAnalyticsProblemSource::LocalOffset(error) => Some(error),
            UsageAnalyticsProblemSource::CalendarRange => None,
            UsageAnalyticsProblemSource::Database(error) => Some(error),
        }
    }
}

type UsageAnalyticsOperation =
    refresh::Operation<UsageAnalyticsData, UsageAnalyticsProblem, Task<()>>;
type UsagePeriodSelectState = SelectState<Vec<UsagePeriodOption>>;

pub(super) struct UsageSettingsPage {
    selected_period: UsageAnalyticsPeriod,
    active_range: Option<UsageAnalyticsRange>,
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
            active_range: None,
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
        cancel_active_query(&mut self.operation, &mut self.active_range);
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
        cancel_active_query(&mut self.operation, &mut self.active_range);
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
        let range = current_usage_range(period, now_utc);
        self.active_range = range.as_ref().ok().copied();
        let completion_range = self.active_range;
        let page = cx.entity().downgrade();

        let task = match range {
            Ok(range) => match database::ready_executor(cx) {
                Ok(executor) => window.spawn(cx, async move |cx| {
                    let result = executor
                        .execute(move |repository| repository.usage_analytics(range))
                        .await
                        .map(|snapshot| UsageAnalyticsData { period, snapshot })
                        .map_err(|source| UsageAnalyticsProblem {
                            period,
                            range: Some(range),
                            source: UsageAnalyticsProblemSource::Database(source),
                        });
                    complete_query(page, period, completion_range, result, cx);
                }),
                Err(source) => window.spawn(cx, async move |cx| {
                    complete_query(
                        page,
                        period,
                        completion_range,
                        Err(UsageAnalyticsProblem {
                            period,
                            range: Some(range),
                            source: UsageAnalyticsProblemSource::Database(source),
                        }),
                        cx,
                    );
                }),
            },
            Err(problem) => window.spawn(cx, async move |cx| {
                complete_query(page, period, completion_range, Err(problem), cx);
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
            .filter(|data| usage_data_matches(self.selected_period, self.active_range, data))
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

    fn render_trend(
        &self,
        data: &UsageAnalyticsData,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if data.period == UsageAnalyticsPeriod::AllTime {
            return None;
        }
        let i18n = cx.global::<I18n>();
        let points = data
            .snapshot
            .daily
            .iter()
            .map(|bucket| DailyChartPoint {
                label: localized_date(bucket.local_date, i18n).into(),
                total_tokens: bucket.aggregate.total_tokens as f64,
            })
            .collect::<Vec<_>>();
        let mut args = FluentArgs::new();
        args.set("range", i18n.t(data.period.i18n_key()));
        args.set(
            "total",
            format_token_count(data.snapshot.summary.total_tokens),
        );
        args.set(
            "covered",
            format_token_count(data.snapshot.summary.total_covered_request_count),
        );
        args.set(
            "requests",
            format_token_count(data.snapshot.summary.request_count),
        );
        let accessible = i18n.t_with_args("settings-usage-trend-accessible", &args);
        let chart = LineChart::new(points)
            .x(|point| point.label.clone())
            .y(|point| point.total_tokens)
            .linear()
            .tick_margin(data.period.chart_tick_margin());
        let chart = if data.period.chart_has_dots() {
            chart.dot()
        } else {
            chart
        };

        Some(
            v_flex()
                .id("settings-usage-trend-section")
                .w_full()
                .gap_2()
                .child(
                    Label::new(i18n.t("settings-usage-trend-title"))
                        .text_sm()
                        .font_medium(),
                )
                .child(
                    Label::new(i18n.t("settings-usage-trend-description"))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .id("settings-usage-trend-chart")
                        .debug_selector(|| "settings-usage-trend-chart".into())
                        .role(Role::Image)
                        .aria_label(accessible)
                        .w_full()
                        .h(px(240.))
                        .child(chart),
                )
                .into_any_element(),
        )
    }

    fn render_breakdown(
        &self,
        buckets: &[UsageAnalyticsProviderModelBucket],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let i18n = cx.global::<I18n>();
        let headers = USAGE_BREAKDOWN_COLUMNS;
        let table = Table::new()
            .small()
            .min_w(px(960.))
            .child(
                TableHeader::new().child(
                    TableRow::new().children(headers.into_iter().enumerate().map(
                        |(column, (key, numeric))| {
                            let text: SharedString = i18n.t(key).into();
                            let head = TableHead::new().child(accessible_table_text(
                                format!("settings-usage-column-{column}"),
                                text,
                            ));
                            if numeric { head.text_right() } else { head }
                        },
                    )),
                ),
            )
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
        v_flex()
            .id("settings-usage-dashboard")
            .w_full()
            .gap_4()
            .child(self.render_summary(&data.snapshot.summary, cx))
            .children(self.render_trend(data, cx))
            .child(self.render_breakdown(&data.snapshot.provider_models, cx))
            .into_any_element()
    }

    fn render_data(&self, data: &UsageAnalyticsData, cx: &mut Context<Self>) -> AnyElement {
        if data.snapshot.summary.is_empty() {
            self.render_empty(cx)
        } else {
            self.render_dashboard(data, cx)
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
    range: Option<UsageAnalyticsRange>,
    result: Result<UsageAnalyticsData, UsageAnalyticsProblem>,
    cx: &mut AsyncWindowContext,
) {
    let _ = page.update_in(cx, |page, _window, cx| {
        if page.selected_period != period
            || page.active_range != range
            || !page.operation.is_running()
        {
            return;
        }
        if let Err(problem) = &result {
            event!(
                Level::ERROR,
                period = ?problem.period,
                range = ?problem.range,
                error = ?problem.source,
                "load settings usage analytics failed"
            );
        }
        page.operation.transition(Complete(result));
        cx.notify();
    });
}

fn cancel_active_query(
    operation: &mut UsageAnalyticsOperation,
    active_range: &mut Option<UsageAnalyticsRange>,
) {
    if operation.is_running() {
        operation.transition(Cancel);
    }
    *active_range = None;
}

fn usage_data_matches(
    selected_period: UsageAnalyticsPeriod,
    active_range: Option<UsageAnalyticsRange>,
    data: &UsageAnalyticsData,
) -> bool {
    data.period == selected_period && Some(data.snapshot.range) == active_range
}

fn current_usage_range(
    period: UsageAnalyticsPeriod,
    now_utc: OffsetDateTime,
) -> Result<UsageAnalyticsRange, UsageAnalyticsProblem> {
    if period == UsageAnalyticsPeriod::AllTime {
        return Ok(UsageAnalyticsRange::AllTime);
    }
    let local_offset =
        UtcOffset::local_offset_at(now_utc).map_err(|source| UsageAnalyticsProblem {
            period,
            range: None,
            source: UsageAnalyticsProblemSource::LocalOffset(source),
        })?;
    usage_range_for_offset(period, now_utc, local_offset).ok_or(UsageAnalyticsProblem {
        period,
        range: None,
        source: UsageAnalyticsProblemSource::CalendarRange,
    })
}

fn usage_range_for_offset(
    period: UsageAnalyticsPeriod,
    now_utc: OffsetDateTime,
    local_offset: UtcOffset,
) -> Option<UsageAnalyticsRange> {
    if period == UsageAnalyticsPeriod::AllTime {
        return Some(UsageAnalyticsRange::AllTime);
    }
    // `time`'s large-date representation can reach years where offset
    // conversion trips its internal standard-range assertions. One year of
    // headroom covers every legal UTC offset in both directions.
    if !(-9_998..=9_998).contains(&now_utc.year()) {
        return None;
    }
    let today = now_utc.checked_to_offset(local_offset)?.date();
    let (start_date, end_date) = match period {
        UsageAnalyticsPeriod::Today => (today, today.next_day()?),
        UsageAnalyticsPeriod::ThisWeek => {
            let days = i64::from(today.weekday().number_days_from_monday());
            let start = today.checked_sub(Duration::days(days))?;
            (start, start.checked_add(Duration::days(7))?)
        }
        UsageAnalyticsPeriod::ThisMonth => {
            let start = Date::from_calendar_date(today.year(), today.month(), 1).ok()?;
            let (year, month) = next_month(start.year(), start.month())?;
            let end = Date::from_calendar_date(year, month, 1).ok()?;
            (start, end)
        }
        UsageAnalyticsPeriod::ThisYear => {
            let start = Date::from_calendar_date(today.year(), Month::January, 1).ok()?;
            let end =
                Date::from_calendar_date(today.year().checked_add(1)?, Month::January, 1).ok()?;
            (start, end)
        }
        UsageAnalyticsPeriod::AllTime => unreachable!(),
    };
    let start_utc = start_date
        .midnight()
        .assume_offset(local_offset)
        .checked_to_offset(UtcOffset::UTC)?;
    let end_utc = end_date
        .midnight()
        .assume_offset(local_offset)
        .checked_to_offset(UtcOffset::UTC)?;
    UsageAnalyticsFiniteRange::new(start_utc, end_utc, local_offset)
        .map(UsageAnalyticsRange::Finite)
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

struct DailyChartPoint {
    label: SharedString,
    total_tokens: f64,
}

#[cfg(test)]
mod tests {
    use super::{
        USAGE_BREAKDOWN_COLUMNS, UsageAnalyticsData, UsageAnalyticsOperation, UsageAnalyticsPeriod,
        UsageSettingsPage, cancel_active_query, complete_query, display_label, localized_date,
        summary_accessible_label, usage_data_matches, usage_range_for_offset,
    };
    use crate::{database, foundation::I18n};
    use gpui::{AppContext as _, Task, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_operation::{Load, Transition, refresh};
    use jaco_db::{UsageAnalyticsAggregate, UsageAnalyticsRange, UsageAnalyticsSnapshot};
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
        let UsageAnalyticsRange::Finite(range) =
            usage_range_for_offset(period, now, offset).unwrap()
        else {
            panic!("expected a finite range")
        };
        range
    }

    #[test]
    fn usage_periods_keep_product_order_and_default_chart_policy() {
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
        assert_eq!(UsageAnalyticsPeriod::Today.chart_tick_margin(), 1);
        assert_eq!(UsageAnalyticsPeriod::ThisWeek.chart_tick_margin(), 1);
        assert_eq!(UsageAnalyticsPeriod::ThisMonth.chart_tick_margin(), 5);
        assert_eq!(UsageAnalyticsPeriod::ThisYear.chart_tick_margin(), 30);
        assert!(UsageAnalyticsPeriod::ThisMonth.chart_has_dots());
        assert!(!UsageAnalyticsPeriod::ThisYear.chart_has_dots());
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
    fn all_time_does_not_need_a_local_offset() {
        assert_eq!(
            usage_range_for_offset(
                UsageAnalyticsPeriod::AllTime,
                OffsetDateTime::UNIX_EPOCH,
                UtcOffset::UTC,
            ),
            Some(UsageAnalyticsRange::AllTime)
        );
    }

    #[test]
    fn cancellation_clears_active_range_and_restores_previous_operation_state() {
        let mut operation = UsageAnalyticsOperation::new();
        operation.transition(Load(Task::ready(())));
        let mut active_range = Some(UsageAnalyticsRange::AllTime);

        cancel_active_query(&mut operation, &mut active_range);

        assert_eq!(operation.phase(), refresh::Phase::Idle);
        assert_eq!(active_range, None);
    }

    #[gpui::test]
    fn entity_activation_is_single_flight_and_deactivation_cancels_it(cx: &mut TestAppContext) {
        let _dir = init_usage_settings_test(cx);
        let window = open_usage_settings_window(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let page = window.root(&mut cx).expect("usage settings page");

        let first_range = cx.update(|window, cx| {
            page.update(cx, |page, cx| page.activate(window, cx));
            let first_range = page.read(cx).active_range;
            page.update(cx, |page, cx| page.activate(window, cx));
            assert_eq!(page.read(cx).operation.phase(), refresh::Phase::Loading);
            assert_eq!(page.read(cx).active_range, first_range);
            first_range
        });
        assert!(first_range.is_some());

        cx.update(|_window, cx| {
            page.update(cx, |page, cx| page.deactivate(cx));
            assert_eq!(page.read(cx).operation.phase(), refresh::Phase::Idle);
            assert_eq!(page.read(cx).active_range, None);
        });
        cx.run_until_parked();
        assert_eq!(
            page.read_with(&cx, |page, _| page.operation.phase()),
            refresh::Phase::Idle
        );
        assert_eq!(page.read_with(&cx, |page, _| page.active_range), None);
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
            window
                .spawn(cx, async move |cx| {
                    complete_query(
                        weak_page,
                        UsageAnalyticsPeriod::ThisMonth,
                        Some(UsageAnalyticsRange::AllTime),
                        Ok(marked_usage_data(
                            UsageAnalyticsPeriod::ThisMonth,
                            UsageAnalyticsRange::AllTime,
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
    fn entity_rejects_stale_completion_for_range_mismatch(cx: &mut TestAppContext) {
        let _dir = init_usage_settings_test(cx);
        let range_window = open_usage_settings_window(cx);
        let mut range_cx = VisualTestContext::from_window(range_window.into(), cx);
        let range_page = range_window
            .root(&mut range_cx)
            .expect("range mismatch page");
        range_cx.update(|window, cx| {
            let weak_page = range_page.downgrade();
            window
                .spawn(cx, async move |cx| {
                    complete_query(
                        weak_page,
                        UsageAnalyticsPeriod::ThisMonth,
                        Some(UsageAnalyticsRange::AllTime),
                        Ok(marked_usage_data(
                            UsageAnalyticsPeriod::ThisMonth,
                            UsageAnalyticsRange::AllTime,
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
            window
                .spawn(cx, async move |cx| {
                    complete_query(
                        weak_page,
                        UsageAnalyticsPeriod::ThisMonth,
                        None,
                        Ok(marked_usage_data(
                            UsageAnalyticsPeriod::ThisMonth,
                            UsageAnalyticsRange::AllTime,
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
            idle_page.read_with(&idle_cx, |page, _| page.active_range),
            None
        );
    }

    #[test]
    fn stale_data_requires_exact_period_and_range_and_all_zero_usage_is_not_empty() {
        let data = UsageAnalyticsData {
            period: UsageAnalyticsPeriod::AllTime,
            snapshot: UsageAnalyticsSnapshot {
                range: UsageAnalyticsRange::AllTime,
                summary: UsageAnalyticsAggregate {
                    request_count: 1,
                    unreported_request_count: 1,
                    ..Default::default()
                },
                daily: Vec::new(),
                provider_models: Vec::new(),
            },
        };

        assert!(usage_data_matches(
            UsageAnalyticsPeriod::AllTime,
            Some(UsageAnalyticsRange::AllTime),
            &data,
        ));
        assert!(!usage_data_matches(
            UsageAnalyticsPeriod::ThisMonth,
            Some(UsageAnalyticsRange::AllTime),
            &data,
        ));
        assert!(!usage_data_matches(
            UsageAnalyticsPeriod::AllTime,
            None,
            &data,
        ));
        assert!(!data.snapshot.summary.is_empty());
    }

    #[test]
    fn labels_use_current_name_or_stable_id_and_dates_are_localized() {
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
                usage_range_for_offset(UsageAnalyticsPeriod::Today, now, offset)
            });
            assert!(result.is_ok(), "range construction must not panic");
            assert_eq!(result.unwrap(), None);
        }
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
            "settings-usage-trend-title",
            "settings-usage-trend-description",
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
            for key in [
                "settings-usage-trend-accessible",
                "settings-usage-date-value",
                "settings-usage-summary-accessible",
                "settings-usage-metric-accessible",
            ] {
                assert_ne!(
                    i18n.t_with_args(key, &args),
                    key,
                    "missing {key} in {locale}"
                );
            }
        }
    }

    fn marked_usage_data(
        period: UsageAnalyticsPeriod,
        range: UsageAnalyticsRange,
    ) -> UsageAnalyticsData {
        UsageAnalyticsData {
            period,
            snapshot: UsageAnalyticsSnapshot {
                range,
                summary: UsageAnalyticsAggregate {
                    request_count: 999,
                    ..Default::default()
                },
                daily: Vec::new(),
                provider_models: Vec::new(),
            },
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
                    .summary
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
